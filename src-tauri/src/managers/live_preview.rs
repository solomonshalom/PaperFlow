use crate::managers::transcription::TranscriptionManager;
use crate::settings::get_settings;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// ============================================================================
// Constants
// ============================================================================

/// Maximum audio buffer size in samples (~60 seconds at 16kHz)
const MAX_BUFFER_SAMPLES: usize = 16000 * 60;

/// Minimum audio required for transcription (~0.5 seconds)
const MIN_AUDIO_SAMPLES: usize = 8000;

/// Minimum new audio required to trigger a new transcription (~0.1 seconds)
const MIN_NEW_AUDIO_SAMPLES: usize = 1600;

/// Timeout for worker thread join during stop (ms)
const WORKER_JOIN_TIMEOUT_MS: u64 = 3000;

/// Maximum consecutive failures before disabling live preview
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Minimum interval for cloud providers to avoid rate limits (ms)
const CLOUD_MIN_INTERVAL_MS: u64 = 3000;

/// Base backoff delay on failure (ms)
const BACKOFF_BASE_MS: u64 = 500;

/// Maximum backoff delay (ms)
const BACKOFF_MAX_MS: u64 = 5000;

// ============================================================================
// Events
// ============================================================================

#[derive(Clone, Debug, Serialize)]
pub struct LivePreviewEvent {
    pub text: String,
    pub is_final: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct LivePreviewErrorEvent {
    pub error_type: String,
    pub message: String,
    pub is_fatal: bool,
}

// ============================================================================
// LivePreviewManager
// ============================================================================

/// LivePreviewManager handles real-time streaming transcription display.
///
/// Thread Safety:
/// - start() and stop() should only be called from the main thread
/// - push_audio() can be called from any thread (audio callback thread)
/// - The worker thread runs independently and communicates via atomic flags
pub struct LivePreviewManager {
    app_handle: AppHandle,
    transcription_manager: Arc<TranscriptionManager>,

    // Audio buffer for accumulated samples
    audio_buffer: Arc<Mutex<Vec<f32>>>,

    // Control flags
    is_active: Arc<AtomicBool>,
    is_stopping: Arc<AtomicBool>, // Prevents new work during shutdown
    shutdown_signal: Arc<AtomicBool>,

    // Failure tracking for backoff
    consecutive_failures: Arc<AtomicU32>,

    // Track last transcribed position
    last_transcribed_len: Arc<AtomicU64>,

    // Worker thread handle
    worker_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,

    // Condition variable for signaling
    audio_condvar: Arc<(Mutex<bool>, Condvar)>,
}

impl Clone for LivePreviewManager {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            transcription_manager: self.transcription_manager.clone(),
            audio_buffer: self.audio_buffer.clone(),
            is_active: self.is_active.clone(),
            is_stopping: self.is_stopping.clone(),
            shutdown_signal: self.shutdown_signal.clone(),
            consecutive_failures: self.consecutive_failures.clone(),
            last_transcribed_len: self.last_transcribed_len.clone(),
            worker_handle: self.worker_handle.clone(),
            audio_condvar: self.audio_condvar.clone(),
        }
    }
}

impl LivePreviewManager {
    pub fn new(app_handle: &AppHandle, transcription_manager: Arc<TranscriptionManager>) -> Self {
        Self {
            app_handle: app_handle.clone(),
            transcription_manager,
            audio_buffer: Arc::new(Mutex::new(Vec::with_capacity(MIN_AUDIO_SAMPLES * 4))),
            is_active: Arc::new(AtomicBool::new(false)),
            is_stopping: Arc::new(AtomicBool::new(false)),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            last_transcribed_len: Arc::new(AtomicU64::new(0)),
            worker_handle: Arc::new(Mutex::new(None)),
            audio_condvar: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    /// Check if live preview is enabled in settings
    pub fn is_enabled(&self) -> bool {
        let settings = get_settings(&self.app_handle);
        settings.live_preview_enabled
    }

    /// Start the live preview session
    pub fn start(&self) {
        if !self.is_enabled() {
            debug!("Live preview is disabled in settings");
            return;
        }

        // Check if model is loaded
        if !self.transcription_manager.is_model_loaded() {
            debug!("Model not loaded, skipping live preview");
            return;
        }

        // Atomic compare-and-swap to prevent race conditions
        if self
            .is_active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("Live preview already active");
            return;
        }

        info!("Starting live preview session");

        // Reset all state
        self.is_stopping.store(false, Ordering::SeqCst);
        self.shutdown_signal.store(false, Ordering::SeqCst);
        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.last_transcribed_len.store(0, Ordering::SeqCst);

        // Clear audio buffer
        if let Ok(mut buffer) = self.audio_buffer.lock() {
            buffer.clear();
        }

        // Reset condvar
        if let Ok(mut has_audio) = self.audio_condvar.0.lock() {
            *has_audio = false;
        }

        // Spawn worker thread
        let manager = self.clone();
        let handle = thread::Builder::new()
            .name("live-preview-worker".to_string())
            .spawn(move || {
                manager.run_worker();
            });

        match handle {
            Ok(h) => {
                if let Ok(mut worker) = self.worker_handle.lock() {
                    *worker = Some(h);
                }
            }
            Err(e) => {
                error!("Failed to spawn live preview worker: {}", e);
                self.is_active.store(false, Ordering::SeqCst);
                self.emit_error("spawn_failed", &e.to_string(), true);
            }
        }
    }

    /// Stop the live preview session
    pub fn stop(&self) {
        // Set stopping flag first to prevent new transcriptions
        self.is_stopping.store(true, Ordering::SeqCst);

        // Atomic compare-and-swap to ensure single stop
        if self
            .is_active
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.is_stopping.store(false, Ordering::SeqCst);
            return;
        }

        info!("Stopping live preview session");

        // Signal shutdown
        self.shutdown_signal.store(true, Ordering::SeqCst);

        // Wake worker thread
        self.wake_worker();

        // Wait for worker with timeout
        self.join_worker_with_timeout();

        // Cleanup
        if let Ok(mut buffer) = self.audio_buffer.lock() {
            buffer.clear();
            buffer.shrink_to(MIN_AUDIO_SAMPLES * 4);
        }

        self.is_stopping.store(false, Ordering::SeqCst);
        debug!("Live preview session stopped");
    }

    /// Push audio samples to the buffer (called from audio thread)
    pub fn push_audio(&self, samples: &[f32]) {
        // Quick checks without locking
        if !self.is_active.load(Ordering::Relaxed) {
            return;
        }
        if self.is_stopping.load(Ordering::Relaxed) {
            return;
        }

        // Non-blocking buffer access
        let mut buffer = match self.audio_buffer.try_lock() {
            Ok(b) => b,
            Err(_) => return, // Buffer busy, skip this chunk
        };

        // Sliding window if buffer too large
        if buffer.len() + samples.len() > MAX_BUFFER_SAMPLES {
            let current_len = buffer.len();
            let overflow = current_len + samples.len() - MAX_BUFFER_SAMPLES;
            let drain_count = overflow.min(current_len);
            buffer.drain(0..drain_count);
        }

        buffer.extend_from_slice(samples);

        // Signal new audio (non-blocking)
        if let Ok(mut has_audio) = self.audio_condvar.0.try_lock() {
            *has_audio = true;
            self.audio_condvar.1.notify_one();
        }
    }

    // ========================================================================
    // Private Methods
    // ========================================================================

    fn wake_worker(&self) {
        if let Ok(mut has_audio) = self.audio_condvar.0.lock() {
            *has_audio = true;
            self.audio_condvar.1.notify_all();
        }
    }

    fn join_worker_with_timeout(&self) {
        let worker = self.worker_handle.lock().ok().and_then(|mut w| w.take());

        if let Some(handle) = worker {
            let start = Instant::now();
            let timeout = Duration::from_millis(WORKER_JOIN_TIMEOUT_MS);

            // Spin-wait with short sleeps
            while start.elapsed() < timeout {
                if handle.is_finished() {
                    let _ = handle.join();
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }

            warn!("Live preview worker did not stop within timeout");
            // Thread will be orphaned but will exit on next shutdown check
        }
    }

    fn emit_error(&self, error_type: &str, message: &str, is_fatal: bool) {
        let event = LivePreviewErrorEvent {
            error_type: error_type.to_string(),
            message: message.to_string(),
            is_fatal,
        };
        if let Err(e) = self.app_handle.emit("live-preview-error", event) {
            error!("Failed to emit live preview error event: {}", e);
        }
    }

    fn calculate_backoff(&self) -> Duration {
        let failures = self.consecutive_failures.load(Ordering::Relaxed);
        if failures == 0 {
            return Duration::ZERO;
        }
        let backoff_ms = (BACKOFF_BASE_MS * (1 << failures.min(4))).min(BACKOFF_MAX_MS);
        Duration::from_millis(backoff_ms)
    }

    fn run_worker(&self) {
        debug!("Live preview worker started");

        let is_cloud = self.transcription_manager.is_cloud_model();
        let mut last_transcription = Instant::now();

        loop {
            // === Check shutdown conditions ===
            if self.shutdown_signal.load(Ordering::SeqCst) {
                break;
            }
            if self.is_stopping.load(Ordering::SeqCst) {
                break;
            }

            // === Read settings (respects runtime changes) ===
            let settings = get_settings(&self.app_handle);

            if !settings.live_preview_enabled {
                debug!("Live preview disabled during recording");
                self.emit_error("disabled", "Live preview was disabled", false);
                break;
            }

            // === Calculate interval ===
            let base_interval = Duration::from_millis(settings.live_preview_interval_ms as u64);
            let cloud_interval = Duration::from_millis(CLOUD_MIN_INTERVAL_MS);
            let backoff = self.calculate_backoff();

            let effective_interval = if is_cloud {
                base_interval.max(cloud_interval) + backoff
            } else {
                base_interval + backoff
            };

            // === Wait for audio or timeout ===
            {
                let timeout = effective_interval.saturating_sub(last_transcription.elapsed());
                if timeout > Duration::ZERO {
                    if let Ok(mut has_audio) = self.audio_condvar.0.lock() {
                        if !*has_audio {
                            let result = self
                                .audio_condvar
                                .1
                                .wait_timeout(has_audio, timeout)
                                .unwrap_or_else(|e| e.into_inner());
                            has_audio = result.0;
                        }
                        *has_audio = false;
                    }
                }
            }

            // === Re-check shutdown after wait ===
            if self.shutdown_signal.load(Ordering::SeqCst) {
                break;
            }
            if self.is_stopping.load(Ordering::SeqCst) {
                break;
            }

            // === Check timing ===
            if last_transcription.elapsed() < effective_interval {
                continue;
            }

            // === Get audio samples ===
            let (current_len, audio_samples) = match self.get_audio_for_transcription() {
                Some(data) => data,
                None => continue,
            };

            // === Final shutdown check before transcription ===
            if self.is_stopping.load(Ordering::SeqCst) {
                break;
            }

            // === Perform transcription ===
            debug!(
                "Transcribing {} samples (buffer: {}, failures: {})",
                audio_samples.len(),
                current_len,
                self.consecutive_failures.load(Ordering::Relaxed)
            );

            match self.transcription_manager.transcribe_partial(audio_samples) {
                Ok(text) => {
                    // Success - reset failure count
                    self.consecutive_failures.store(0, Ordering::SeqCst);
                    self.last_transcribed_len
                        .store(current_len as u64, Ordering::SeqCst);

                    if !text.is_empty() {
                        self.emit_preview_text(&text);
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // Don't count "engine busy" as a failure - it's expected
                    if error_msg.contains("Engine busy") {
                        debug!("Engine busy, will retry");
                        continue;
                    }

                    let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
                    warn!(
                        "Live preview transcription failed ({}/{}): {}",
                        failures, MAX_CONSECUTIVE_FAILURES, error_msg
                    );

                    if failures >= MAX_CONSECUTIVE_FAILURES {
                        error!("Live preview disabled due to repeated failures");
                        self.emit_error(
                            "max_failures",
                            "Live preview stopped due to repeated transcription failures",
                            true,
                        );
                        break;
                    }
                }
            }

            last_transcription = Instant::now();
        }

        debug!("Live preview worker stopped");
    }

    fn get_audio_for_transcription(&self) -> Option<(usize, Vec<f32>)> {
        let buffer = self.audio_buffer.lock().ok()?;

        let current_len = buffer.len();
        let last_len = self.last_transcribed_len.load(Ordering::SeqCst) as usize;

        // Not enough new audio
        if current_len <= last_len + MIN_NEW_AUDIO_SAMPLES {
            return None;
        }

        // Not enough total audio
        if current_len < MIN_AUDIO_SAMPLES {
            return None;
        }

        // Get a reasonable window (up to 30 seconds for context)
        let window_size = MAX_BUFFER_SAMPLES / 2;
        let start_pos = current_len.saturating_sub(window_size);
        let samples = buffer[start_pos..].to_vec();

        Some((current_len, samples))
    }

    fn emit_preview_text(&self, text: &str) {
        let event = LivePreviewEvent {
            text: text.to_string(),
            is_final: false,
        };
        if let Err(e) = self.app_handle.emit("live-preview-update", event) {
            error!("Failed to emit live preview event: {}", e);
        }
    }
}
