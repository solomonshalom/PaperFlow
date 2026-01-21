//! Meeting Mode Manager
//!
//! Handles long-form recording with chunked transcription, auto-summarization,
//! and action item extraction.

use anyhow::Result;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::managers::audio::AudioRecordingManager;
use crate::managers::history::HistoryManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::get_settings;
use crate::tray::{change_tray_icon, TrayIconState};
use crate::utils;

/// State of the meeting mode
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "state")]
pub enum MeetingState {
    /// No meeting in progress
    Idle,
    /// Meeting is currently recording
    Recording {
        meeting_id: String,
        started_at: i64,
        chunk_count: u32,
        /// The binding_id used to start the recording (needed to stop it correctly)
        binding_id: String,
    },
    /// Meeting is being processed (transcription/summarization)
    Processing { meeting_id: String },
}

/// A single chunk of meeting audio
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MeetingChunk {
    pub chunk_id: u32,
    pub start_time_ms: u64,
    pub end_time_ms: u64,
    pub audio_path: Option<String>,
    pub transcription: Option<String>,
}

/// A complete meeting session
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MeetingSession {
    pub meeting_id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub chunks: Vec<MeetingChunk>,
    pub full_transcript: Option<String>,
    pub summary: Option<String>,
    pub action_items: Option<Vec<String>>,
    pub duration_seconds: Option<i64>,
}

/// Event emitted when meeting state changes
#[derive(Debug, Clone, Serialize, Type)]
pub struct MeetingStateEvent {
    pub state: MeetingState,
    pub elapsed_seconds: Option<u64>,
    pub chunk_count: Option<u32>,
}

/// Event emitted when a chunk is transcribed
#[derive(Debug, Clone, Serialize, Type)]
pub struct MeetingChunkEvent {
    pub meeting_id: String,
    pub chunk_id: u32,
    pub transcription: String,
}

/// Internal state for the meeting manager
struct MeetingManagerInner {
    state: MeetingState,
    current_session: Option<MeetingSession>,
    recording_start: Option<Instant>,
    last_chunk_time: Option<Instant>,
    pending_audio: Vec<f32>,
}

impl Default for MeetingManagerInner {
    fn default() -> Self {
        Self {
            state: MeetingState::Idle,
            current_session: None,
            recording_start: None,
            last_chunk_time: None,
            pending_audio: Vec::new(),
        }
    }
}

/// Manager for meeting mode functionality
#[derive(Clone)]
pub struct MeetingManager {
    inner: Arc<Mutex<MeetingManagerInner>>,
    app_handle: AppHandle,
}

impl MeetingManager {
    /// Create a new MeetingManager
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let manager = Self {
            inner: Arc::new(Mutex::new(MeetingManagerInner::default())),
            app_handle: app_handle.clone(),
        };

        // Check for crash recovery on startup
        if let Ok(Some(session)) = manager.recover_from_crash() {
            info!(
                "Recovered meeting session from crash: {}",
                session.meeting_id
            );
            // Emit recovery event
            let _ = app_handle.emit("meeting-recovery-available", &session);
        }

        Ok(manager)
    }

    /// Get the current meeting state
    pub fn get_meeting_state(&self) -> MeetingState {
        let inner = self.inner.lock().unwrap();
        inner.state.clone()
    }

    /// Get the current session if any
    pub fn get_current_session(&self) -> Option<MeetingSession> {
        let inner = self.inner.lock().unwrap();
        inner.current_session.clone()
    }

    /// Get elapsed time in seconds since meeting started
    pub fn get_elapsed_seconds(&self) -> Option<u64> {
        let inner = self.inner.lock().unwrap();
        inner.recording_start.map(|start| start.elapsed().as_secs())
    }

    /// Start a new meeting
    pub fn start_meeting(&self, binding_id: &str) -> Result<String> {
        let mut inner = self.inner.lock().unwrap();

        // Check if already recording
        if !matches!(inner.state, MeetingState::Idle) {
            return Err(anyhow::anyhow!("Meeting already in progress"));
        }

        let settings = get_settings(&self.app_handle);
        if !settings.meeting_mode_enabled {
            return Err(anyhow::anyhow!("Meeting mode is not enabled"));
        }

        // Generate meeting ID
        let meeting_id = Uuid::new_v4().to_string();
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        info!("Starting meeting: {}", meeting_id);

        // Create new session
        let session = MeetingSession {
            meeting_id: meeting_id.clone(),
            started_at,
            ended_at: None,
            chunks: Vec::new(),
            full_transcript: None,
            summary: None,
            action_items: None,
            duration_seconds: None,
        };

        // Start audio recording
        let rm = self.app_handle.state::<Arc<AudioRecordingManager>>();
        if !rm.start_meeting_recording(binding_id) {
            return Err(anyhow::anyhow!("Failed to start audio recording"));
        }

        // Update state
        inner.state = MeetingState::Recording {
            meeting_id: meeting_id.clone(),
            started_at,
            chunk_count: 0,
            binding_id: binding_id.to_string(),
        };
        inner.current_session = Some(session);
        inner.recording_start = Some(Instant::now());
        inner.last_chunk_time = Some(Instant::now());
        inner.pending_audio.clear();

        // Emit state change event
        let _ = self.app_handle.emit(
            "meeting-state-changed",
            MeetingStateEvent {
                state: inner.state.clone(),
                elapsed_seconds: Some(0),
                chunk_count: Some(0),
            },
        );

        // Save recovery data
        self.save_recovery_data(&inner.current_session);

        // Start heartbeat timer for UI updates (elapsed time display)
        let manager_clone = self.clone();
        let meeting_id_clone = meeting_id.clone();
        std::thread::spawn(move || {
            manager_clone.heartbeat_loop(meeting_id_clone);
        });

        Ok(meeting_id)
    }

    /// Stop the current meeting and process results
    pub fn stop_meeting(&self, meeting_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();

        // Verify meeting ID matches and extract binding_id
        let binding_id = match &inner.state {
            MeetingState::Recording {
                meeting_id: current_id,
                binding_id,
                ..
            } => {
                if current_id != meeting_id {
                    return Err(anyhow::anyhow!("Meeting ID mismatch"));
                }
                binding_id.clone()
            }
            _ => return Err(anyhow::anyhow!("No meeting in progress")),
        };

        info!("Stopping meeting: {}", meeting_id);

        // Stop audio recording using the original binding_id
        let rm = self.app_handle.state::<Arc<AudioRecordingManager>>();
        let final_samples = rm.stop_recording(&binding_id);

        // Add final samples to pending audio
        if let Some(samples) = final_samples {
            inner.pending_audio.extend(samples);
        }

        // Update state to processing
        inner.state = MeetingState::Processing {
            meeting_id: meeting_id.to_string(),
        };

        // Calculate duration
        let duration = inner.recording_start.map(|s| s.elapsed().as_secs() as i64);
        if let Some(ref mut session) = inner.current_session {
            session.ended_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            );
            session.duration_seconds = duration;
        }

        // Emit state change
        let _ = self.app_handle.emit(
            "meeting-state-changed",
            MeetingStateEvent {
                state: inner.state.clone(),
                elapsed_seconds: duration.map(|d| d as u64),
                chunk_count: inner
                    .current_session
                    .as_ref()
                    .map(|s| s.chunks.len() as u32),
            },
        );

        // Process remaining audio
        let pending_audio = std::mem::take(&mut inner.pending_audio);
        let session = inner.current_session.clone();
        drop(inner); // Release lock before async processing

        // Spawn async task for final processing
        let manager_clone = self.clone();
        let meeting_id_clone = meeting_id.to_string();
        tauri::async_runtime::spawn(async move {
            manager_clone
                .finalize_meeting(meeting_id_clone, pending_audio, session)
                .await;
        });

        Ok(())
    }

    /// Cancel the current meeting without processing
    pub fn cancel_meeting(&self, meeting_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();

        // Verify meeting ID matches and extract binding_id
        let binding_id = match &inner.state {
            MeetingState::Recording {
                meeting_id: current_id,
                binding_id,
                ..
            } => {
                if current_id != meeting_id {
                    return Err(anyhow::anyhow!("Meeting ID mismatch"));
                }
                binding_id.clone()
            }
            _ => return Err(anyhow::anyhow!("No meeting in progress")),
        };

        info!("Cancelling meeting: {}", meeting_id);

        // Stop audio recording without processing using the original binding_id
        let rm = self.app_handle.state::<Arc<AudioRecordingManager>>();
        let _ = rm.stop_recording(&binding_id);

        // Reset state
        inner.state = MeetingState::Idle;
        inner.current_session = None;
        inner.recording_start = None;
        inner.last_chunk_time = None;
        inner.pending_audio.clear();

        // Clear recovery data
        self.clear_recovery_data();

        // Emit state change
        let _ = self.app_handle.emit(
            "meeting-state-changed",
            MeetingStateEvent {
                state: MeetingState::Idle,
                elapsed_seconds: None,
                chunk_count: None,
            },
        );

        Ok(())
    }

    /// Recover from a crash - returns session if recovery data exists
    pub fn recover_from_crash(&self) -> Result<Option<MeetingSession>> {
        let hm = match self.app_handle.try_state::<Arc<HistoryManager>>() {
            Some(hm) => hm,
            None => {
                debug!("HistoryManager not available for recovery check");
                return Ok(None);
            }
        };

        match hm.load_meeting_recovery() {
            Ok(Some(session_json)) => match serde_json::from_str::<MeetingSession>(&session_json) {
                Ok(session) => {
                    info!("Found recoverable meeting session: {}", session.meeting_id);
                    Ok(Some(session))
                }
                Err(e) => {
                    warn!("Failed to parse recovery data, clearing: {}", e);
                    let _ = hm.clear_meeting_recovery();
                    Ok(None)
                }
            },
            Ok(None) => Ok(None),
            Err(e) => {
                warn!("Failed to load recovery data: {}", e);
                Ok(None)
            }
        }
    }

    /// Emit periodic heartbeat events for UI updates (elapsed time display)
    fn heartbeat_loop(&self, meeting_id: String) {
        loop {
            std::thread::sleep(Duration::from_secs(1));

            let inner = self.inner.lock().unwrap();

            // Check if still recording this meeting
            match &inner.state {
                MeetingState::Recording {
                    meeting_id: current_id,
                    chunk_count,
                    ..
                } => {
                    if current_id != &meeting_id {
                        debug!("Meeting ID changed, stopping heartbeat loop");
                        return;
                    }

                    // Emit heartbeat with elapsed time
                    let elapsed = inner.recording_start.map(|s| s.elapsed().as_secs());
                    let _ = self.app_handle.emit(
                        "meeting-heartbeat",
                        MeetingStateEvent {
                            state: inner.state.clone(),
                            elapsed_seconds: elapsed,
                            chunk_count: Some(*chunk_count),
                        },
                    );
                }
                _ => {
                    debug!("Meeting stopped, exiting heartbeat loop");
                    return;
                }
            }
        }
    }

    /// Reset state to Idle and cleanup UI - called on error or completion
    fn reset_to_idle(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.state = MeetingState::Idle;
            inner.current_session = None;
            inner.recording_start = None;
            inner.last_chunk_time = None;
        }

        // Clear recovery data
        self.clear_recovery_data();

        // Hide overlay and reset tray
        utils::hide_recording_overlay(&self.app_handle);
        change_tray_icon(&self.app_handle, TrayIconState::Idle);

        // Emit state change
        let _ = self.app_handle.emit(
            "meeting-state-changed",
            MeetingStateEvent {
                state: MeetingState::Idle,
                elapsed_seconds: None,
                chunk_count: None,
            },
        );
    }

    /// Finalize the meeting with summarization and action items
    async fn finalize_meeting(
        &self,
        meeting_id: String,
        final_audio: Vec<f32>,
        session: Option<MeetingSession>,
    ) {
        let Some(mut session) = session else {
            error!("No session to finalize - resetting state");
            self.reset_to_idle();
            return;
        };

        let settings = get_settings(&self.app_handle);
        let chunk_duration_samples = settings.meeting_chunk_duration_seconds as usize * 16000; // 16kHz sample rate

        // Handle empty audio case
        if final_audio.is_empty() {
            warn!("No audio recorded for meeting {}", meeting_id);
            self.reset_to_idle();
            return;
        }

        // Split audio into chunks and transcribe each
        let tm = self.app_handle.state::<Arc<TranscriptionManager>>();
        let total_samples = final_audio.len();
        let mut offset: usize = 0;
        let mut chunk_id: u32 = 0;

        info!(
            "Processing {} samples ({:.1}s) in chunks of {}s",
            total_samples,
            total_samples as f64 / 16000.0,
            settings.meeting_chunk_duration_seconds
        );

        while offset < total_samples {
            let end = (offset + chunk_duration_samples).min(total_samples);
            let chunk_audio = final_audio[offset..end].to_vec();
            let chunk_samples = chunk_audio.len();

            // Calculate timing
            let start_time_ms = (offset as u64 * 1000) / 16000;
            let end_time_ms = (end as u64 * 1000) / 16000;

            // Transcribe this chunk using spawn_blocking to avoid blocking async runtime
            let tm_clone = Arc::clone(&*tm);
            let transcription_result =
                tokio::task::spawn_blocking(move || tm_clone.transcribe(chunk_audio)).await;

            match transcription_result {
                Ok(Ok(transcription)) => {
                    if !transcription.is_empty() {
                        info!(
                            "Chunk {} transcribed: {} chars ({}ms - {}ms)",
                            chunk_id,
                            transcription.len(),
                            start_time_ms,
                            end_time_ms
                        );

                        let chunk = MeetingChunk {
                            chunk_id,
                            start_time_ms,
                            end_time_ms,
                            audio_path: None,
                            transcription: Some(transcription.clone()),
                        };
                        session.chunks.push(chunk);

                        // Emit chunk event
                        let _ = self.app_handle.emit(
                            "meeting-chunk-transcribed",
                            MeetingChunkEvent {
                                meeting_id: meeting_id.clone(),
                                chunk_id,
                                transcription,
                            },
                        );
                    }
                }
                Ok(Err(e)) => {
                    warn!(
                        "Failed to transcribe chunk {} ({} samples): {}",
                        chunk_id, chunk_samples, e
                    );
                }
                Err(e) => {
                    error!("Transcription task panicked for chunk {}: {}", chunk_id, e);
                }
            }

            offset = end;
            chunk_id += 1;
        }

        // Combine all transcriptions
        let full_transcript: String = session
            .chunks
            .iter()
            .filter_map(|c| c.transcription.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");

        session.full_transcript = Some(full_transcript.clone());

        info!(
            "Meeting {} finalized with {} chunks, {} chars total",
            meeting_id,
            session.chunks.len(),
            full_transcript.len()
        );

        let settings = get_settings(&self.app_handle);

        // Generate summary if enabled
        if settings.meeting_auto_summarize && !full_transcript.is_empty() {
            match self.generate_summary(&full_transcript, &settings).await {
                Ok(summary) => {
                    session.summary = Some(summary);
                    debug!("Generated meeting summary");
                }
                Err(e) => {
                    warn!("Failed to generate summary: {}", e);
                }
            }
        }

        // Extract action items if enabled
        if settings.meeting_extract_action_items && !full_transcript.is_empty() {
            match self.extract_action_items(&full_transcript, &settings).await {
                Ok(items) => {
                    session.action_items = Some(items);
                    debug!("Extracted action items");
                }
                Err(e) => {
                    warn!("Failed to extract action items: {}", e);
                }
            }
        }

        // Handle case where no chunks were successfully transcribed
        if full_transcript.is_empty() {
            warn!(
                "No transcript generated for meeting {} - all chunks failed",
                meeting_id
            );
            // Still save to history so user knows meeting was attempted
            self.save_meeting_to_history(&session);
            self.reset_to_idle();
            return;
        }

        // Clear recovery data before saving (meeting is complete)
        self.clear_recovery_data();

        // Save to history first (this emits meeting-history-updated)
        self.save_meeting_to_history(&session);

        // Emit completion event
        let _ = self.app_handle.emit("meeting-completed", &session);

        // Copy transcript to clipboard and paste it, then cleanup UI
        let app_handle = self.app_handle.clone();
        let app_handle_for_closure = app_handle.clone();
        let transcript_to_paste = full_transcript.clone();
        let session_duration = session.duration_seconds;
        let chunk_count = session.chunks.len() as u32;

        let _ = app_handle.run_on_main_thread(move || {
            // Paste the transcript
            match utils::paste(transcript_to_paste, app_handle_for_closure.clone()) {
                Ok(()) => info!("Meeting transcript pasted successfully"),
                Err(e) => error!("Failed to paste meeting transcript: {}", e),
            }

            // Now that paste is complete, hide overlay and reset tray
            utils::hide_recording_overlay(&app_handle_for_closure);
            change_tray_icon(&app_handle_for_closure, TrayIconState::Idle);

            // Emit final state change on main thread
            let _ = app_handle_for_closure.emit(
                "meeting-state-changed",
                MeetingStateEvent {
                    state: MeetingState::Idle,
                    elapsed_seconds: session_duration.map(|d| d as u64),
                    chunk_count: Some(chunk_count),
                },
            );
        });

        // Update internal state (can happen before paste completes, but UI won't change until main thread runs)
        {
            let mut inner = self.inner.lock().unwrap();
            inner.state = MeetingState::Idle;
            inner.current_session = None;
            inner.recording_start = None;
            inner.last_chunk_time = None;
        }
    }

    /// Generate a summary using the configured LLM
    async fn generate_summary(
        &self,
        transcript: &str,
        settings: &crate::settings::AppSettings,
    ) -> Result<String> {
        let prompt = settings
            .meeting_summary_prompt
            .replace("${transcript}", transcript);

        let provider = settings.active_post_process_provider().ok_or_else(|| {
            anyhow::anyhow!("No post-processing provider configured for summarization")
        })?;

        let api_key = settings
            .post_process_api_keys
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();

        let model = settings
            .post_process_models
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();

        if api_key.is_empty() || model.is_empty() {
            return Err(anyhow::anyhow!("LLM not configured for summarization"));
        }

        match crate::llm_client::send_chat_completion(provider, api_key, &model, prompt).await {
            Ok(Some(content)) => Ok(content),
            Ok(None) => Err(anyhow::anyhow!("Empty response from LLM")),
            Err(e) => Err(anyhow::anyhow!("LLM request failed: {}", e)),
        }
    }

    /// Extract action items using the configured LLM
    async fn extract_action_items(
        &self,
        transcript: &str,
        settings: &crate::settings::AppSettings,
    ) -> Result<Vec<String>> {
        let prompt = settings
            .meeting_action_items_prompt
            .replace("${transcript}", transcript);

        let provider = settings.active_post_process_provider().ok_or_else(|| {
            anyhow::anyhow!("No post-processing provider configured for action items")
        })?;

        let api_key = settings
            .post_process_api_keys
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();

        let model = settings
            .post_process_models
            .get(&provider.id)
            .cloned()
            .unwrap_or_default();

        if api_key.is_empty() || model.is_empty() {
            return Err(anyhow::anyhow!("LLM not configured for action items"));
        }

        match crate::llm_client::send_chat_completion(provider, api_key, &model, prompt).await {
            Ok(Some(content)) => {
                // Parse the response as a list of action items
                let items: Vec<String> = content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| {
                        // Remove common list prefixes
                        line.trim()
                            .trim_start_matches(|c: char| {
                                c.is_numeric() || c == '.' || c == '-' || c == '*'
                            })
                            .trim()
                            .to_string()
                    })
                    .filter(|s| !s.is_empty())
                    .collect();
                Ok(items)
            }
            Ok(None) => Err(anyhow::anyhow!("Empty response from LLM")),
            Err(e) => Err(anyhow::anyhow!("LLM request failed: {}", e)),
        }
    }

    /// Save recovery data to database
    fn save_recovery_data(&self, session: &Option<MeetingSession>) {
        let Some(session) = session else {
            return;
        };

        let hm = match self.app_handle.try_state::<Arc<HistoryManager>>() {
            Some(hm) => hm,
            None => {
                debug!("HistoryManager not available for recovery save");
                return;
            }
        };

        match serde_json::to_string(session) {
            Ok(session_json) => {
                if let Err(e) = hm.save_meeting_recovery(&session_json) {
                    warn!("Failed to save recovery data: {}", e);
                } else {
                    debug!("Saved recovery data for meeting: {}", session.meeting_id);
                }
            }
            Err(e) => {
                warn!("Failed to serialize session for recovery: {}", e);
            }
        }
    }

    /// Clear recovery data from database
    fn clear_recovery_data(&self) {
        let hm = match self.app_handle.try_state::<Arc<HistoryManager>>() {
            Some(hm) => hm,
            None => {
                debug!("HistoryManager not available for recovery clear");
                return;
            }
        };

        if let Err(e) = hm.clear_meeting_recovery() {
            warn!("Failed to clear recovery data: {}", e);
        } else {
            debug!("Cleared recovery data");
        }
    }

    /// Save meeting to history after completion
    fn save_meeting_to_history(&self, session: &MeetingSession) {
        let hm = match self.app_handle.try_state::<Arc<HistoryManager>>() {
            Some(hm) => hm,
            None => {
                warn!("HistoryManager not available, meeting not saved to history");
                return;
            }
        };

        // Serialize action items as JSON if present
        let action_items_json = session
            .action_items
            .as_ref()
            .and_then(|items| serde_json::to_string(items).ok());

        if let Err(e) = hm.save_meeting(
            &session.meeting_id,
            session.started_at,
            session.ended_at.unwrap_or(0),
            session.duration_seconds.unwrap_or(0),
            session.full_transcript.as_deref().unwrap_or(""),
            session.summary.as_deref(),
            action_items_json.as_deref(),
            session.chunks.len() as u32,
        ) {
            error!("Failed to save meeting to history: {}", e);
        } else {
            info!("Meeting {} saved to history", session.meeting_id);

            // Save individual chunks
            for chunk in &session.chunks {
                if let Err(e) = hm.save_meeting_chunk(
                    &session.meeting_id,
                    chunk.chunk_id,
                    chunk.start_time_ms,
                    chunk.end_time_ms,
                    chunk.transcription.as_deref().unwrap_or(""),
                    chunk.audio_path.as_deref(),
                ) {
                    warn!("Failed to save meeting chunk {}: {}", chunk.chunk_id, e);
                }
            }
        }
    }
}
