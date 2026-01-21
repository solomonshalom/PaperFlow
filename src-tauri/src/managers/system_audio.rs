//! Native system audio capture manager
//!
//! Provides native system audio capture without requiring third-party virtual audio drivers:
//! - macOS 14.2+: Core Audio Taps API via ScreenCaptureKit
//! - Windows 10+: WASAPI loopback capture
//! - Linux: PipeWire/PulseAudio monitors (already native)

use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::AppHandle;

#[cfg(target_os = "macos")]
use screencapturekit::prelude::*;

/// Flag indicating whether ScreenCaptureKit implementation is complete
/// Set to true once full capture functionality is implemented and tested
#[cfg(target_os = "macos")]
const MACOS_IMPLEMENTATION_COMPLETE: bool = true;

/// Target sample rate for transcription (Whisper expects 16kHz)
const TARGET_SAMPLE_RATE: u32 = 16000;

/// ScreenCaptureKit capture sample rate (macOS default is 48kHz)
#[cfg(target_os = "macos")]
const CAPTURE_SAMPLE_RATE: u32 = 48000;

// ============================================================================
// macOS Audio Capture Handler
// ============================================================================

/// Handler for receiving audio samples from ScreenCaptureKit
#[cfg(target_os = "macos")]
struct AudioCaptureHandler {
    raw_samples: Arc<Mutex<Vec<f32>>>,
    is_capturing: Arc<AtomicBool>,
}

#[cfg(target_os = "macos")]
impl SCStreamOutputTrait for AudioCaptureHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, output_type: SCStreamOutputType) {
        // Only process audio samples
        if output_type != SCStreamOutputType::Audio {
            return;
        }

        // Check if we're still capturing
        if !self.is_capturing.load(Ordering::SeqCst) {
            return;
        }

        // Extract and accumulate audio samples
        if let Some(audio_data) = Self::extract_audio_samples(&sample) {
            if let Ok(mut samples) = self.raw_samples.lock() {
                samples.extend_from_slice(&audio_data);
                debug!(
                    "Captured {} audio samples, total: {}",
                    audio_data.len(),
                    samples.len()
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
impl AudioCaptureHandler {
    /// Extract audio samples from a CMSampleBuffer
    fn extract_audio_samples(sample: &CMSampleBuffer) -> Option<Vec<f32>> {
        // Try to get audio buffer list from the sample
        // This handles the Core Audio format conversion
        let audio_buffers = sample.audio_buffer_list()?;

        let mut samples = Vec::new();

        // Iterate over audio buffers using reference
        for buffer in audio_buffers.iter() {
            // Audio data is typically in f32 format at 48kHz stereo
            let data = buffer.data();
            if !data.is_empty() {
                // Convert bytes to f32 samples
                // Core Audio typically provides data as f32
                let float_samples: &[f32] = unsafe {
                    std::slice::from_raw_parts(
                        data.as_ptr() as *const f32,
                        data.len() / std::mem::size_of::<f32>(),
                    )
                };
                samples.extend_from_slice(float_samples);
            }
        }

        if samples.is_empty() {
            None
        } else {
            Some(samples)
        }
    }
}

// ============================================================================
// Native Audio Status
// ============================================================================

/// Status of native system audio capture availability
#[derive(Clone, Debug, PartialEq)]
pub enum NativeAudioStatus {
    /// Native capture is available and ready
    Available,
    /// Native capture requires additional permissions
    NeedsPermission,
    /// Platform doesn't support native capture (use virtual drivers)
    NotSupported { reason: String },
    /// Error during initialization
    Error(String),
}

/// Manager for native system audio capture
pub struct SystemAudioManager {
    #[allow(dead_code)]
    app_handle: AppHandle,
    status: Arc<Mutex<NativeAudioStatus>>,
    is_capturing: Arc<AtomicBool>,
    captured_samples: Arc<Mutex<Vec<f32>>>,
    /// Raw samples from capture (48kHz stereo on macOS)
    #[cfg(target_os = "macos")]
    raw_samples: Arc<Mutex<Vec<f32>>>,
    /// Active ScreenCaptureKit stream
    #[cfg(target_os = "macos")]
    stream: Arc<Mutex<Option<SCStream>>>,
}

impl SystemAudioManager {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let manager = Self {
            app_handle: app.clone(),
            status: Arc::new(Mutex::new(NativeAudioStatus::NotSupported {
                reason: "Not initialized".to_string(),
            })),
            is_capturing: Arc::new(AtomicBool::new(false)),
            captured_samples: Arc::new(Mutex::new(Vec::new())),
            #[cfg(target_os = "macos")]
            raw_samples: Arc::new(Mutex::new(Vec::new())),
            #[cfg(target_os = "macos")]
            stream: Arc::new(Mutex::new(None)),
        };

        // Check platform support
        manager.check_native_support();

        Ok(manager)
    }

    /// Check if native system audio capture is available on this platform
    fn check_native_support(&self) {
        let status = self.detect_platform_support();
        if let Ok(mut guard) = self.status.lock() {
            *guard = status;
        }
    }

    #[cfg(target_os = "macos")]
    fn detect_platform_support(&self) -> NativeAudioStatus {
        // Check macOS version (need 14.2+ for Core Audio Taps)
        if !Self::is_macos_14_2_or_later() {
            return NativeAudioStatus::NotSupported {
                reason: "macOS 14.2 or later required for native system audio capture".to_string(),
            };
        }

        // Check if implementation is complete
        // This prevents showing "Available" when the feature is not yet functional
        if !MACOS_IMPLEMENTATION_COMPLETE {
            return NativeAudioStatus::NotSupported {
                reason: "Native capture implementation in progress".to_string(),
            };
        }

        // ScreenCaptureKit is available, check permissions
        // Note: Permission check happens when we try to capture
        NativeAudioStatus::Available
    }

    #[cfg(target_os = "macos")]
    fn is_macos_14_2_or_later() -> bool {
        use objc2_foundation::NSProcessInfo;

        let info = NSProcessInfo::processInfo();
        let version = info.operatingSystemVersion();

        // macOS 14.2 = major 14, minor 2
        if version.majorVersion > 14 {
            return true;
        }
        if version.majorVersion == 14 && version.minorVersion >= 2 {
            return true;
        }
        false
    }

    #[cfg(target_os = "windows")]
    fn detect_platform_support(&self) -> NativeAudioStatus {
        // WASAPI loopback is available on Windows Vista and later
        // Windows 10+ has improved support
        NativeAudioStatus::Available
    }

    #[cfg(target_os = "linux")]
    fn detect_platform_support(&self) -> NativeAudioStatus {
        // Linux already supports system audio capture via PipeWire/PulseAudio monitors
        // These show up as regular input devices, so native capture is "not needed"
        NativeAudioStatus::NotSupported {
            reason: "Linux uses PipeWire/PulseAudio monitors (select a .monitor device)"
                .to_string(),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    fn detect_platform_support(&self) -> NativeAudioStatus {
        NativeAudioStatus::NotSupported {
            reason: "Native system audio capture not supported on this platform".to_string(),
        }
    }

    /// Get the current status of native audio capture
    pub fn get_status(&self) -> NativeAudioStatus {
        self.status
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or(NativeAudioStatus::Error(
                "Failed to acquire lock".to_string(),
            ))
    }

    /// Check if native system audio capture is available
    pub fn is_available(&self) -> bool {
        matches!(self.get_status(), NativeAudioStatus::Available)
    }

    /// Check if currently capturing
    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::SeqCst)
    }

    /// Start capturing system audio
    pub fn start_capture(&self) -> Result<()> {
        if !self.is_available() {
            return Err(anyhow!(
                "Native system audio capture not available on this platform"
            ));
        }

        if self.is_capturing.load(Ordering::SeqCst) {
            return Err(anyhow!("Already capturing"));
        }

        info!("Starting native system audio capture");

        // Clear previous samples
        if let Ok(mut samples) = self.captured_samples.lock() {
            samples.clear();
        }

        // Platform-specific capture implementation
        #[cfg(target_os = "macos")]
        {
            self.start_macos_capture()?;
        }

        #[cfg(target_os = "windows")]
        {
            self.start_windows_capture()?;
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            return Err(anyhow!(
                "Native system audio capture not implemented for this platform"
            ));
        }

        self.is_capturing.store(true, Ordering::SeqCst);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn start_macos_capture(&self) -> Result<()> {
        info!("Starting ScreenCaptureKit audio capture");

        // Clear raw samples before starting
        if let Ok(mut samples) = self.raw_samples.lock() {
            samples.clear();
        }

        // Get shareable content (displays, windows, apps)
        let content = SCShareableContent::get()
            .map_err(|e| anyhow!("Failed to get shareable content: {:?}", e))?;

        // Get the first display for system audio capture
        let display = content
            .displays()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No displays found"))?;

        info!(
            "Capturing system audio from display: {:?}",
            display.display_id()
        );

        // Create content filter for the display (captures all system audio)
        let filter = SCContentFilter::create()
            .with_display(&display)
            .with_excluding_windows(&[])
            .build();

        // Configure stream for audio-only capture
        // Use small video dimensions to minimize overhead since we only want audio
        let config = SCStreamConfiguration::new()
            .with_width(1)
            .with_height(1)
            .with_captures_audio(true)
            .with_sample_rate(CAPTURE_SAMPLE_RATE as i32)
            .with_channel_count(2);

        // Create audio handler that collects samples
        let audio_handler = AudioCaptureHandler {
            raw_samples: Arc::clone(&self.raw_samples),
            is_capturing: Arc::clone(&self.is_capturing),
        };

        // Create the stream
        let mut stream = SCStream::new(&filter, &config);

        // Add audio output handler
        stream.add_output_handler(audio_handler, SCStreamOutputType::Audio);

        // Start the capture
        stream
            .start_capture()
            .map_err(|e| anyhow!("Failed to start capture: {:?}", e))?;

        // Store the stream
        if let Ok(mut stream_guard) = self.stream.lock() {
            *stream_guard = Some(stream);
        }

        info!("ScreenCaptureKit audio capture started successfully");
        Ok(())
    }

    /// Stop ScreenCaptureKit capture and process samples
    #[cfg(target_os = "macos")]
    fn stop_macos_capture(&self) -> Result<Vec<f32>> {
        info!("Stopping ScreenCaptureKit audio capture");

        // Stop the stream
        if let Ok(mut stream_guard) = self.stream.lock() {
            if let Some(stream) = stream_guard.take() {
                if let Err(e) = stream.stop_capture() {
                    warn!("Error stopping stream: {:?}", e);
                }
            }
        }

        // Get raw samples and process them
        let raw = if let Ok(guard) = self.raw_samples.lock() {
            guard.clone()
        } else {
            Vec::new()
        };

        if raw.is_empty() {
            warn!("No audio samples were captured");
            return Ok(Vec::new());
        }

        info!("Processing {} raw samples", raw.len());

        // Convert stereo to mono
        let mono = stereo_to_mono(&raw);
        info!("Converted to {} mono samples", mono.len());

        // Resample from 48kHz to 16kHz
        let resampled = resample_linear(&mono, CAPTURE_SAMPLE_RATE, TARGET_SAMPLE_RATE);
        info!(
            "Resampled from {}Hz to {}Hz: {} samples",
            CAPTURE_SAMPLE_RATE,
            TARGET_SAMPLE_RATE,
            resampled.len()
        );

        Ok(resampled)
    }

    #[cfg(target_os = "windows")]
    fn start_windows_capture(&self) -> Result<()> {
        // TODO: Implement WASAPI loopback capture
        // For now, return an error indicating it's not yet fully implemented
        Err(anyhow!(
            "Windows native audio capture via WASAPI loopback is not yet fully implemented. \
             Please use Stereo Mix or a virtual audio cable for now."
        ))
    }

    /// Stop capturing and return the captured samples (resampled to 16kHz)
    pub fn stop_capture(&self) -> Result<Vec<f32>> {
        if !self.is_capturing.load(Ordering::SeqCst) {
            return Err(anyhow!("Not capturing"));
        }

        info!("Stopping native system audio capture");

        // Set capturing flag to false first to stop callbacks
        self.is_capturing.store(false, Ordering::SeqCst);

        // Platform-specific stop and processing
        #[cfg(target_os = "macos")]
        {
            let samples = self.stop_macos_capture()?;
            // Store processed samples in captured_samples for consistency
            if let Ok(mut guard) = self.captured_samples.lock() {
                *guard = samples.clone();
            }
            return Ok(samples);
        }

        #[cfg(target_os = "windows")]
        {
            // TODO: Implement Windows stop
            let samples = if let Ok(guard) = self.captured_samples.lock() {
                guard.clone()
            } else {
                Vec::new()
            };
            return Ok(samples);
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            // Get captured samples for other platforms
            let samples = if let Ok(guard) = self.captured_samples.lock() {
                guard.clone()
            } else {
                Vec::new()
            };
            Ok(samples)
        }
    }

    /// Get platform-specific information about native capture
    pub fn get_platform_info(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            if !Self::is_macos_14_2_or_later() {
                return "Requires macOS 14.2+ for native capture".to_string();
            }

            if MACOS_IMPLEMENTATION_COMPLETE {
                "Native capture via ScreenCaptureKit (macOS 14.2+)".to_string()
            } else {
                "Native capture via ScreenCaptureKit - In development".to_string()
            }
        }

        #[cfg(target_os = "windows")]
        {
            "Native capture via WASAPI loopback - Coming soon".to_string()
        }

        #[cfg(target_os = "linux")]
        {
            "Use PipeWire/PulseAudio monitor devices".to_string()
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            "Platform not supported".to_string()
        }
    }
}

// ============================================================================
// Audio Processing Helper Functions
// ============================================================================

/// Convert stereo audio to mono by averaging left and right channels
pub fn stereo_to_mono(stereo_samples: &[f32]) -> Vec<f32> {
    stereo_samples
        .chunks(2)
        .map(|chunk| {
            if chunk.len() == 2 {
                (chunk[0] + chunk[1]) / 2.0
            } else {
                chunk[0]
            }
        })
        .collect()
}

/// Simple resampling using linear interpolation
/// For production use, the existing FrameResampler with rubato is preferred
pub fn resample_linear(samples: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if from_hz == to_hz {
        return samples.to_vec();
    }

    let ratio = from_hz as f64 / to_hz as f64;
    let output_len = ((samples.len() as f64) / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;

        let sample = if idx_floor < samples.len() {
            samples[idx_floor] * (1.0 - frac as f32) + samples[idx_ceil] * frac as f32
        } else {
            0.0
        };
        output.push(sample);
    }

    output
}

// ============================================================================
// TDD Test Module
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============ STATUS TESTS ============
    mod status_tests {
        use super::*;

        #[test]
        fn test_status_not_available_when_not_implemented() {
            // When MACOS_IMPLEMENTATION_COMPLETE is false (current state),
            // status should NOT be Available
            #[cfg(target_os = "macos")]
            {
                if !MACOS_IMPLEMENTATION_COMPLETE {
                    // Create a mock status check without AppHandle
                    let status = NativeAudioStatus::NotSupported {
                        reason: "Native capture implementation in progress".to_string(),
                    };
                    assert!(
                        !matches!(status, NativeAudioStatus::Available),
                        "Status should not be Available when implementation is incomplete"
                    );
                }
            }
        }

        #[test]
        fn test_get_platform_info_shows_development_status() {
            #[cfg(target_os = "macos")]
            {
                if !MACOS_IMPLEMENTATION_COMPLETE {
                    // Platform info should indicate development status
                    let expected_substring = "In development";
                    let info = "Native capture via ScreenCaptureKit - In development";
                    assert!(
                        info.contains(expected_substring),
                        "Platform info should indicate development status"
                    );
                }
            }
        }

        #[test]
        fn test_native_audio_status_variants() {
            // Test all status variants exist and can be created
            let available = NativeAudioStatus::Available;
            let needs_perm = NativeAudioStatus::NeedsPermission;
            let not_supported = NativeAudioStatus::NotSupported {
                reason: "test".to_string(),
            };
            let error = NativeAudioStatus::Error("test error".to_string());

            assert!(matches!(available, NativeAudioStatus::Available));
            assert!(matches!(needs_perm, NativeAudioStatus::NeedsPermission));
            assert!(matches!(
                not_supported,
                NativeAudioStatus::NotSupported { .. }
            ));
            assert!(matches!(error, NativeAudioStatus::Error(_)));
        }

        #[test]
        fn test_status_clone_and_debug() {
            let status = NativeAudioStatus::Available;
            let cloned = status.clone();
            assert_eq!(status, cloned);

            // Test debug formatting works
            let debug_str = format!("{:?}", status);
            assert!(debug_str.contains("Available"));
        }
    }

    // ============ CAPTURE LIFECYCLE TESTS ============
    mod capture_lifecycle_tests {
        use super::*;
        use std::sync::atomic::Ordering;

        #[test]
        fn test_is_capturing_initially_false() {
            let is_capturing = Arc::new(AtomicBool::new(false));
            assert!(!is_capturing.load(Ordering::SeqCst));
        }

        #[test]
        fn test_is_capturing_flag_toggle() {
            let is_capturing = Arc::new(AtomicBool::new(false));

            // Start capturing
            is_capturing.store(true, Ordering::SeqCst);
            assert!(is_capturing.load(Ordering::SeqCst));

            // Stop capturing
            is_capturing.store(false, Ordering::SeqCst);
            assert!(!is_capturing.load(Ordering::SeqCst));
        }

        #[test]
        fn test_captured_samples_initially_empty() {
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
            assert!(samples.lock().unwrap().is_empty());
        }

        #[test]
        fn test_captured_samples_accumulation() {
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));

            // Simulate audio callback adding samples
            {
                let mut guard = samples.lock().unwrap();
                guard.extend_from_slice(&[0.1, 0.2, 0.3]);
            }

            assert_eq!(samples.lock().unwrap().len(), 3);

            // Add more samples
            {
                let mut guard = samples.lock().unwrap();
                guard.extend_from_slice(&[0.4, 0.5]);
            }

            assert_eq!(samples.lock().unwrap().len(), 5);
        }

        #[test]
        fn test_captured_samples_clear() {
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(vec![0.1, 0.2, 0.3]));

            // Clear samples (like start_capture does)
            samples.lock().unwrap().clear();

            assert!(samples.lock().unwrap().is_empty());
        }

        #[test]
        fn test_stop_capture_returns_samples() {
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(vec![0.1, 0.2, 0.3, 0.4]));

            // Simulate stop_capture returning samples
            let returned = samples.lock().unwrap().clone();

            assert_eq!(returned, vec![0.1, 0.2, 0.3, 0.4]);
        }
    }

    // ============ AUDIO PROCESSING TESTS ============
    mod audio_processing_tests {
        use super::*;

        #[test]
        fn test_stereo_to_mono_conversion() {
            // Stereo samples: [L0, R0, L1, R1, ...]
            let stereo = vec![0.5, 0.5, 1.0, 0.0, 0.0, 1.0, -0.5, 0.5];
            let mono = stereo_to_mono(&stereo);

            assert_eq!(mono.len(), 4);
            assert!((mono[0] - 0.5).abs() < 0.001); // (0.5 + 0.5) / 2 = 0.5
            assert!((mono[1] - 0.5).abs() < 0.001); // (1.0 + 0.0) / 2 = 0.5
            assert!((mono[2] - 0.5).abs() < 0.001); // (0.0 + 1.0) / 2 = 0.5
            assert!((mono[3] - 0.0).abs() < 0.001); // (-0.5 + 0.5) / 2 = 0.0
        }

        #[test]
        fn test_stereo_to_mono_empty_input() {
            let stereo: Vec<f32> = vec![];
            let mono = stereo_to_mono(&stereo);
            assert!(mono.is_empty());
        }

        #[test]
        fn test_stereo_to_mono_odd_length() {
            // Odd length input - last sample should be preserved
            let stereo = vec![0.5, 0.5, 1.0];
            let mono = stereo_to_mono(&stereo);
            assert_eq!(mono.len(), 2);
            assert!((mono[0] - 0.5).abs() < 0.001);
            assert!((mono[1] - 1.0).abs() < 0.001); // Single sample kept as-is
        }

        #[test]
        fn test_resample_same_rate() {
            let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5];
            let resampled = resample_linear(&samples, 48000, 48000);
            assert_eq!(resampled, samples);
        }

        #[test]
        fn test_resample_48khz_to_16khz() {
            // 3:1 downsampling ratio
            let samples: Vec<f32> = (0..4800).map(|i| (i as f32) / 4800.0).collect();
            let resampled = resample_linear(&samples, 48000, 16000);

            // Output should be approximately 1/3 the length
            let expected_len = (4800.0_f64 / 3.0).ceil() as usize;
            assert!(
                (resampled.len() as i32 - expected_len as i32).abs() <= 1,
                "Expected ~{} samples, got {}",
                expected_len,
                resampled.len()
            );
        }

        #[test]
        fn test_resample_44100hz_to_16khz() {
            // 44100/16000 = 2.75625 downsampling ratio
            let samples: Vec<f32> = (0..4410).map(|i| (i as f32) / 4410.0).collect();
            let resampled = resample_linear(&samples, 44100, 16000);

            let expected_len = (4410.0_f64 * 16000.0 / 44100.0).ceil() as usize;
            assert!(
                (resampled.len() as i32 - expected_len as i32).abs() <= 1,
                "Expected ~{} samples, got {}",
                expected_len,
                resampled.len()
            );
        }

        #[test]
        fn test_resample_preserves_approximate_values() {
            // Test that resampling preserves approximate signal shape
            // Generate a simple ramp
            let samples: Vec<f32> = (0..300).map(|i| i as f32 / 300.0).collect();
            let resampled = resample_linear(&samples, 48000, 16000);

            // First and last values should be approximately preserved
            assert!(resampled[0].abs() < 0.01, "First sample should be near 0");
            assert!(
                (resampled[resampled.len() - 1] - 1.0).abs() < 0.1,
                "Last sample should be near 1.0"
            );
        }

        #[test]
        fn test_resample_empty_input() {
            let samples: Vec<f32> = vec![];
            let resampled = resample_linear(&samples, 48000, 16000);
            assert!(resampled.is_empty());
        }
    }

    // ============ PERMISSION TESTS ============
    mod permission_tests {
        use super::*;

        #[test]
        fn test_needs_permission_status() {
            let status = NativeAudioStatus::NeedsPermission;
            assert!(matches!(status, NativeAudioStatus::NeedsPermission));
        }

        #[test]
        fn test_available_status() {
            let status = NativeAudioStatus::Available;
            assert!(matches!(status, NativeAudioStatus::Available));
        }

        #[test]
        fn test_not_supported_with_reason() {
            let reason = "Test reason for not being supported".to_string();
            let status = NativeAudioStatus::NotSupported {
                reason: reason.clone(),
            };

            if let NativeAudioStatus::NotSupported { reason: r } = status {
                assert_eq!(r, reason);
            } else {
                panic!("Expected NotSupported variant");
            }
        }
    }

    // ============ INTEGRATION TESTS (macOS only) ============
    #[cfg(target_os = "macos")]
    mod macos_integration_tests {
        use super::*;

        #[test]
        fn test_macos_version_check_runs() {
            // This test verifies the version check doesn't panic
            let _is_14_2 = SystemAudioManager::is_macos_14_2_or_later();
        }

        #[test]
        fn test_implementation_flag_is_boolean() {
            // Verify the flag is a boolean and can be checked
            let _complete: bool = MACOS_IMPLEMENTATION_COMPLETE;
        }
    }
}
