use crate::audio_toolkit::{
    apply_corrections, apply_custom_words, apply_formatting, filter_transcription_output,
    FormattingRules,
};
use crate::groq_transcription;
use crate::managers::diarization::DiarizationManager;
use crate::managers::model::{EngineType, ModelManager};
use crate::managers::snippets::apply_snippets;
use crate::settings::{get_settings, ModelUnloadTimeout};
use anyhow::Result;
use log::{debug, error, info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use tokio::runtime::Handle;
use transcribe_rs::{
    engines::{
        moonshine::{ModelVariant, MoonshineEngine, MoonshineModelParams},
        parakeet::{
            ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity,
        },
        whisper::{WhisperEngine, WhisperInferenceParams},
    },
    TranscriptionEngine,
};

#[derive(Clone, Debug, Serialize)]
pub struct ModelStateEvent {
    pub event_type: String,
    pub model_id: Option<String>,
    pub model_name: Option<String>,
    pub error: Option<String>,
}

/// Event emitted during CoreML model compilation (first-run takes 3-5 minutes)
#[derive(Clone, Debug, Serialize)]
pub struct CoreMLCompilationEvent {
    pub event_type: String, // "started", "completed", "failed"
    pub model_id: String,
    pub estimated_time_seconds: Option<u32>,
    pub error: Option<String>,
}

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetEngine),
    Moonshine(MoonshineEngine),
    GroqCloud { model_id: String },
}

#[derive(Clone)]
pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    app_handle: AppHandle,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    shutdown_signal: Arc<AtomicBool>,
    watcher_handle: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}

impl TranscriptionManager {
    pub fn new(app_handle: &AppHandle, model_manager: Arc<ModelManager>) -> Result<Self> {
        let manager = Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            app_handle: app_handle.clone(),
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            )),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            watcher_handle: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
        };

        // Start the idle watcher
        {
            let app_handle_cloned = app_handle.clone();
            let manager_cloned = manager.clone();
            let shutdown_signal = manager.shutdown_signal.clone();
            let handle = thread::spawn(move || {
                while !shutdown_signal.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_secs(10)); // Check every 10 seconds

                    // Check shutdown signal again after sleep
                    if shutdown_signal.load(Ordering::Relaxed) {
                        break;
                    }

                    let settings = get_settings(&app_handle_cloned);
                    let timeout_seconds = settings.model_unload_timeout.to_seconds();

                    if let Some(limit_seconds) = timeout_seconds {
                        // Skip polling-based unloading for immediate timeout since it's handled directly in transcribe()
                        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately {
                            continue;
                        }

                        let last = manager_cloned.last_activity.load(Ordering::Relaxed);
                        let now_ms = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;

                        if now_ms.saturating_sub(last) > limit_seconds * 1000 {
                            // idle -> unload
                            if manager_cloned.is_model_loaded() {
                                let unload_start = std::time::Instant::now();
                                debug!("Starting to unload model due to inactivity");

                                if let Ok(()) = manager_cloned.unload_model() {
                                    let _ = app_handle_cloned.emit(
                                        "model-state-changed",
                                        ModelStateEvent {
                                            event_type: "unloaded".to_string(),
                                            model_id: None,
                                            model_name: None,
                                            error: None,
                                        },
                                    );
                                    let unload_duration = unload_start.elapsed();
                                    debug!(
                                        "Model unloaded due to inactivity (took {}ms)",
                                        unload_duration.as_millis()
                                    );
                                }
                            }
                        }
                    }
                }
                debug!("Idle watcher thread shutting down gracefully");
            });
            *manager.watcher_handle.lock().unwrap() = Some(handle);
        }

        Ok(manager)
    }

    pub fn is_model_loaded(&self) -> bool {
        let engine = self.engine.lock().unwrap();
        engine.is_some()
    }

    pub fn unload_model(&self) -> Result<()> {
        let unload_start = std::time::Instant::now();
        debug!("Starting to unload model");

        {
            let mut engine = self.engine.lock().unwrap();
            if let Some(ref mut loaded_engine) = *engine {
                match loaded_engine {
                    LoadedEngine::Whisper(ref mut e) => e.unload_model(),
                    LoadedEngine::Parakeet(ref mut e) => e.unload_model(),
                    LoadedEngine::Moonshine(ref mut e) => e.unload_model(),
                    LoadedEngine::GroqCloud { .. } => {
                        // Cloud models have no local state to unload
                    }
                }
            }
            *engine = None; // Drop the engine to free memory
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = None;
        }

        // Emit unloaded event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "unloaded".to_string(),
                model_id: None,
                model_name: None,
                error: None,
            },
        );

        let unload_duration = unload_start.elapsed();
        debug!(
            "Model unloaded manually (took {}ms)",
            unload_duration.as_millis()
        );
        Ok(())
    }

    /// Unloads the model immediately if the setting is enabled and the model is loaded
    pub fn maybe_unload_immediately(&self, context: &str) {
        let settings = get_settings(&self.app_handle);
        if settings.model_unload_timeout == ModelUnloadTimeout::Immediately
            && self.is_model_loaded()
        {
            info!("Immediately unloading model after {}", context);
            if let Err(e) = self.unload_model() {
                warn!("Failed to immediately unload model: {}", e);
            }
        }
    }

    pub fn load_model(&self, model_id: &str) -> Result<()> {
        let load_start = std::time::Instant::now();
        debug!("Starting to load model: {}", model_id);

        // Emit loading started event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_started".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: None,
                error: None,
            },
        );

        let model_info = self
            .model_manager
            .get_model_info(model_id)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_id))?;

        if !model_info.is_downloaded {
            let error_msg = "Model not downloaded";
            let _ = self.app_handle.emit(
                "model-state-changed",
                ModelStateEvent {
                    event_type: "loading_failed".to_string(),
                    model_id: Some(model_id.to_string()),
                    model_name: Some(model_info.name.clone()),
                    error: Some(error_msg.to_string()),
                },
            );
            return Err(anyhow::anyhow!(error_msg));
        }

        // Get model path for local models (skip for cloud models)
        let model_path = if model_info.engine_type != EngineType::GroqCloud {
            Some(self.model_manager.get_model_path(model_id)?)
        } else {
            None
        };

        // Create appropriate engine based on model type
        let loaded_engine = match model_info.engine_type {
            EngineType::Whisper => {
                let path = model_path.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Model path missing for Whisper engine '{}'", model_id)
                })?;

                // Check if CoreML is enabled and available (macOS only)
                #[cfg(target_os = "macos")]
                let using_coreml = {
                    let settings = get_settings(&self.app_handle);
                    if settings.coreml_enabled {
                        // Check if CoreML model is downloaded
                        if let Some(coreml_path) =
                            self.model_manager.get_coreml_model_path(model_id)
                        {
                            info!(
                                "CoreML model found at {:?}, will use Apple Neural Engine acceleration",
                                coreml_path
                            );

                            // Check if this might be the first compilation using a marker file
                            // ANECompilerService caches compiled models, so first-run is slow
                            let compiled_marker = coreml_path.join(".compiled");
                            let is_first_run = !compiled_marker.exists();

                            if is_first_run {
                                info!(
                                    "First-time CoreML compilation detected, this may take 3-5 minutes"
                                );
                                let _ = self.app_handle.emit(
                                    "coreml-compilation-status",
                                    CoreMLCompilationEvent {
                                        event_type: "started".to_string(),
                                        model_id: model_id.to_string(),
                                        estimated_time_seconds: Some(240), // ~4 minutes
                                        error: None,
                                    },
                                );
                            }

                            true
                        } else {
                            info!(
                                "CoreML enabled but model not downloaded, using Metal acceleration"
                            );
                            false
                        }
                    } else {
                        info!("CoreML disabled, using Metal acceleration");
                        false
                    }
                };

                #[cfg(not(target_os = "macos"))]
                let using_coreml = false;

                let mut engine = WhisperEngine::new();
                let load_result = engine.load_model(path);

                // Emit CoreML compilation completed event if we were using CoreML
                #[cfg(target_os = "macos")]
                if using_coreml {
                    // Create marker file to indicate compilation is done
                    if let Some(coreml_path) = self.model_manager.get_coreml_model_path(model_id) {
                        let compiled_marker = coreml_path.join(".compiled");
                        let _ = std::fs::write(&compiled_marker, "");
                    }

                    let _ = self.app_handle.emit(
                        "coreml-compilation-status",
                        CoreMLCompilationEvent {
                            event_type: "completed".to_string(),
                            model_id: model_id.to_string(),
                            estimated_time_seconds: None,
                            error: None,
                        },
                    );
                }

                load_result.map_err(|e| {
                    let error_msg = format!("Failed to load whisper model {}: {}", model_id, e);
                    let _ = self.app_handle.emit(
                        "model-state-changed",
                        ModelStateEvent {
                            event_type: "loading_failed".to_string(),
                            model_id: Some(model_id.to_string()),
                            model_name: Some(model_info.name.clone()),
                            error: Some(error_msg.clone()),
                        },
                    );
                    anyhow::anyhow!(error_msg)
                })?;

                LoadedEngine::Whisper(engine)
            }
            EngineType::Parakeet => {
                let path = model_path.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Model path missing for Parakeet engine '{}'", model_id)
                })?;
                let mut engine = ParakeetEngine::new();
                engine
                    .load_model_with_params(path, ParakeetModelParams::int8())
                    .map_err(|e| {
                        let error_msg =
                            format!("Failed to load parakeet model {}: {}", model_id, e);
                        let _ = self.app_handle.emit(
                            "model-state-changed",
                            ModelStateEvent {
                                event_type: "loading_failed".to_string(),
                                model_id: Some(model_id.to_string()),
                                model_name: Some(model_info.name.clone()),
                                error: Some(error_msg.clone()),
                            },
                        );
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Parakeet(engine)
            }
            EngineType::Moonshine => {
                let path = model_path.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Model path missing for Moonshine engine '{}'", model_id)
                })?;
                let mut engine = MoonshineEngine::new();
                engine
                    .load_model_with_params(path, MoonshineModelParams::variant(ModelVariant::Base))
                    .map_err(|e| {
                        let error_msg =
                            format!("Failed to load moonshine model {}: {}", model_id, e);
                        let _ = self.app_handle.emit(
                            "model-state-changed",
                            ModelStateEvent {
                                event_type: "loading_failed".to_string(),
                                model_id: Some(model_id.to_string()),
                                model_name: Some(model_info.name.clone()),
                                error: Some(error_msg.clone()),
                            },
                        );
                        anyhow::anyhow!(error_msg)
                    })?;
                LoadedEngine::Moonshine(engine)
            }
            EngineType::GroqCloud => {
                // Cloud models don't need local loading - just store the model ID
                info!("Setting up Groq cloud model: {}", model_id);
                LoadedEngine::GroqCloud {
                    model_id: model_id.to_string(),
                }
            }
        };

        // Update the current engine and model ID
        {
            let mut engine = self.engine.lock().unwrap();
            *engine = Some(loaded_engine);
        }
        {
            let mut current_model = self.current_model_id.lock().unwrap();
            *current_model = Some(model_id.to_string());
        }

        // Emit loading completed event
        let _ = self.app_handle.emit(
            "model-state-changed",
            ModelStateEvent {
                event_type: "loading_completed".to_string(),
                model_id: Some(model_id.to_string()),
                model_name: Some(model_info.name.clone()),
                error: None,
            },
        );

        let load_duration = load_start.elapsed();
        debug!(
            "Successfully loaded transcription model: {} (took {}ms)",
            model_id,
            load_duration.as_millis()
        );
        Ok(())
    }

    /// Kicks off the model loading in a background thread if it's not already loaded
    pub fn initiate_model_load(&self) {
        let mut is_loading = self.is_loading.lock().unwrap();
        if *is_loading || self.is_model_loaded() {
            return;
        }

        *is_loading = true;
        let self_clone = self.clone();
        thread::spawn(move || {
            let settings = get_settings(&self_clone.app_handle);
            if let Err(e) = self_clone.load_model(&settings.selected_model) {
                error!("Failed to load model: {}", e);
            }
            let mut is_loading = self_clone.is_loading.lock().unwrap();
            *is_loading = false;
            self_clone.loading_condvar.notify_all();
        });
    }

    pub fn get_current_model(&self) -> Option<String> {
        let current_model = self.current_model_id.lock().unwrap();
        current_model.clone()
    }

    pub fn transcribe(&self, audio: Vec<f32>) -> Result<String> {
        // Update last activity timestamp
        self.last_activity.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            Ordering::Relaxed,
        );

        let st = std::time::Instant::now();

        debug!("Audio vector length: {}", audio.len());

        if audio.is_empty() {
            debug!("Empty audio vector");
            self.maybe_unload_immediately("empty audio");
            return Ok(String::new());
        }

        // Check if model is loaded, if not try to load it
        {
            // If the model is loading, wait for it to complete.
            let mut is_loading = self.is_loading.lock().unwrap();
            while *is_loading {
                is_loading = self.loading_condvar.wait(is_loading).unwrap();
            }

            let engine_guard = self.engine.lock().unwrap();
            if engine_guard.is_none() {
                return Err(anyhow::anyhow!("Model is not loaded for transcription."));
            }
        }

        // Get current settings for configuration
        let settings = get_settings(&self.app_handle);

        // Clone audio for diarization if enabled (before transcription consumes it)
        let audio_for_diarization = if settings.diarization_enabled {
            Some(audio.clone())
        } else {
            None
        };

        // Perform transcription with the appropriate engine
        let result = {
            let mut engine_guard = self.engine.lock().unwrap();
            let engine = engine_guard.as_mut().ok_or_else(|| {
                anyhow::anyhow!(
                    "Model failed to load after auto-load attempt. Please check your model settings."
                )
            })?;

            match engine {
                LoadedEngine::Whisper(whisper_engine) => {
                    // Multilingual mode: use language=None for auto-detection (handles code-switching)
                    // Single language mode: use selected_language
                    let whisper_language = if settings.multilingual_mode_enabled {
                        // Whisper Large v3 handles code-switching natively when language is None
                        debug!("Multilingual mode enabled, using auto language detection for code-switching");
                        None
                    } else if settings.selected_language == "auto" {
                        None
                    } else {
                        // Normalize language code for Whisper
                        // Convert zh-Hans and zh-Hant to zh since Whisper uses ISO 639-1 codes
                        let normalized = if settings.selected_language == "zh-Hans"
                            || settings.selected_language == "zh-Hant"
                        {
                            "zh".to_string()
                        } else {
                            settings.selected_language.clone()
                        };
                        Some(normalized)
                    };

                    let params = WhisperInferenceParams {
                        language: whisper_language,
                        translate: settings.translate_to_english,
                        ..Default::default()
                    };

                    whisper_engine
                        .transcribe_samples(audio, Some(params))
                        .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?
                }
                LoadedEngine::Parakeet(parakeet_engine) => {
                    if settings.multilingual_mode_enabled {
                        warn!("Multilingual mode is not supported by Parakeet model. Using English only.");
                    }
                    let params = ParakeetInferenceParams {
                        timestamp_granularity: TimestampGranularity::Segment,
                        ..Default::default()
                    };
                    parakeet_engine
                        .transcribe_samples(audio, Some(params))
                        .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?
                }
                LoadedEngine::Moonshine(moonshine_engine) => {
                    if settings.multilingual_mode_enabled {
                        warn!("Multilingual mode is not supported by Moonshine model. Using English only.");
                    }
                    moonshine_engine
                        .transcribe_samples(audio, None)
                        .map_err(|e| anyhow::anyhow!("Moonshine transcription failed: {}", e))?
                }
                LoadedEngine::GroqCloud { model_id } => {
                    // Cloud transcription - make async API call
                    let api_key = settings.groq_transcription_api_key.clone();
                    let language = if settings.multilingual_mode_enabled {
                        // In multilingual mode, don't specify a language - let Groq auto-detect
                        None
                    } else if settings.selected_language == "auto" {
                        None
                    } else {
                        Some(settings.selected_language.clone())
                    };
                    let model_id_clone = model_id.clone();
                    let audio_clone = audio.to_vec();

                    // Get multilingual settings for prompt hint
                    let primary_lang = settings.primary_language.clone();
                    let secondary_lang = settings.secondary_language.clone();
                    let is_multilingual = settings.multilingual_mode_enabled;

                    // Use block_in_place to avoid deadlock when called from async context
                    let result = tokio::task::block_in_place(|| {
                        Handle::current().block_on(async {
                            if is_multilingual {
                                groq_transcription::transcribe_multilingual(
                                    &api_key,
                                    &model_id_clone,
                                    &audio_clone,
                                    primary_lang.as_deref(),
                                    secondary_lang.as_deref(),
                                )
                                .await
                            } else {
                                groq_transcription::transcribe(
                                    &api_key,
                                    &model_id_clone,
                                    &audio_clone,
                                    language.as_deref(),
                                )
                                .await
                            }
                        })
                    });

                    let text = result
                        .map_err(|e| anyhow::anyhow!("Groq cloud transcription failed: {}", e))?;

                    // Return a result structure compatible with local engines
                    transcribe_rs::TranscriptionResult {
                        text,
                        segments: None,
                    }
                }
            }
        };

        // Apply speaker diarization if enabled and audio is available
        let diarized_text = if let Some(audio_samples) = audio_for_diarization {
            info!("Diarization enabled, attempting to run speaker diarization...");
            // Try to get the diarization manager and run diarization
            if let Some(dm) = self.app_handle.try_state::<Arc<DiarizationManager>>() {
                info!(
                    "Got diarization manager, is_available: {}",
                    dm.is_available()
                );
                if dm.is_available() {
                    info!(
                        "Running speaker diarization on {} samples...",
                        audio_samples.len()
                    );
                    match dm.diarize(&audio_samples) {
                        Ok(diarization_segments) => {
                            info!(
                                "Diarization returned {} segments",
                                diarization_segments.len()
                            );
                            if !diarization_segments.is_empty() {
                                // Check if we have transcription segments with timestamps
                                info!("Transcription has segments: {}", result.segments.is_some());
                                if let Some(ref segments) = result.segments {
                                    // Convert transcription segments to the format expected by diarization
                                    let transcription_segments: Vec<(u64, u64, String)> = segments
                                        .iter()
                                        .map(|s| {
                                            (
                                                (s.start * 1000.0) as u64,
                                                (s.end * 1000.0) as u64,
                                                s.text.clone(),
                                            )
                                        })
                                        .collect();

                                    // Assign speakers to transcription segments
                                    let labeled_segments =
                                        DiarizationManager::assign_speakers_to_segments(
                                            &transcription_segments,
                                            &diarization_segments,
                                        );

                                    // Format output with speaker labels
                                    let mut formatted_output = String::new();
                                    let mut last_speaker: Option<String> = None;

                                    for (_start, _end, text, speaker) in labeled_segments {
                                        let current_speaker =
                                            speaker.unwrap_or_else(|| "Unknown".to_string());

                                        // Only add speaker label when speaker changes
                                        if last_speaker.as_ref() != Some(&current_speaker) {
                                            if !formatted_output.is_empty() {
                                                formatted_output.push('\n');
                                            }
                                            formatted_output
                                                .push_str(&format!("[{}]: ", current_speaker));
                                            last_speaker = Some(current_speaker);
                                        }

                                        formatted_output.push_str(&text);
                                    }

                                    debug!(
                                        "Diarization applied: {} speakers detected",
                                        diarization_segments
                                            .iter()
                                            .map(|s| s.speaker.clone())
                                            .collect::<std::collections::HashSet<_>>()
                                            .len()
                                    );
                                    Some(formatted_output)
                                } else {
                                    // No transcription segments - apply diarization to the whole text
                                    // Just prepend the dominant speaker
                                    if let Some(first_segment) = diarization_segments.first() {
                                        debug!("No transcription segments, using first diarization speaker");
                                        Some(format!(
                                            "[{}]: {}",
                                            first_segment.speaker, result.text
                                        ))
                                    } else {
                                        None
                                    }
                                }
                            } else {
                                debug!("No diarization segments found (audio may be too short)");
                                None
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Diarization failed: {}. Continuing without speaker labels.",
                                e
                            );
                            None
                        }
                    }
                } else {
                    debug!("Diarization manager not available");
                    None
                }
            } else {
                debug!("Diarization manager not found in app state");
                None
            }
        } else {
            None
        };

        // Use diarized text if available, otherwise use original transcription
        let text_for_processing = diarized_text.unwrap_or(result.text);

        // Apply word correction if custom words are configured
        let corrected_result = if !settings.custom_words.is_empty() {
            apply_custom_words(
                &text_for_processing,
                &settings.custom_words,
                settings.word_correction_threshold,
            )
        } else {
            text_for_processing
        };

        // Filter out filler words and hallucinations
        let filtered_result = filter_transcription_output(&corrected_result);

        // Apply correction detection if enabled
        let corrected_text = if settings.correction_detection_enabled {
            apply_corrections(&filtered_result)
        } else {
            filtered_result
        };

        // Apply voice snippets if enabled
        let snippets_result = if settings.snippets_enabled && !settings.snippets.is_empty() {
            apply_snippets(&corrected_text, &settings.snippets)
        } else {
            corrected_text
        };

        // Apply auto-formatting if enabled
        let formatted_result = if settings.auto_format_enabled {
            let rules = FormattingRules {
                auto_lists: settings.auto_format_lists,
                verbal_commands: settings.verbal_commands_enabled,
            };
            apply_formatting(&snippets_result, &rules)
        } else {
            snippets_result
        };

        let et = std::time::Instant::now();
        let translation_note = if settings.translate_to_english {
            " (translated)"
        } else {
            ""
        };
        info!(
            "Transcription completed in {}ms{}",
            (et - st).as_millis(),
            translation_note
        );

        let final_result = formatted_result;

        if final_result.is_empty() {
            info!("Transcription result is empty");
        } else {
            info!("Transcription result: {}", final_result);
        }

        self.maybe_unload_immediately("transcription");

        Ok(final_result)
    }

    /// Check if the current model is a cloud-based model (requires network)
    pub fn is_cloud_model(&self) -> bool {
        match self.engine.try_lock() {
            Ok(guard) => matches!(guard.as_ref(), Some(LoadedEngine::GroqCloud { .. })),
            Err(_) => false, // Assume local if we can't check
        }
    }

    /// Transcribe audio for live preview (skips post-processing for speed)
    ///
    /// This method is designed for real-time preview during recording.
    /// It skips custom word correction, filtering, and other post-processing
    /// to minimize latency.
    ///
    /// IMPORTANT: This method uses try_lock to avoid blocking the main transcription.
    /// If the engine is busy, it returns an error immediately rather than waiting.
    pub fn transcribe_partial(&self, audio: Vec<f32>) -> Result<String> {
        // Update last activity timestamp
        self.last_activity.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            Ordering::Relaxed,
        );

        if audio.is_empty() {
            return Ok(String::new());
        }

        // Use try_lock to avoid blocking the main transcription
        // If the engine is busy (e.g., final transcription starting), skip this preview
        let mut engine_guard = self
            .engine
            .try_lock()
            .map_err(|_| anyhow::anyhow!("Engine busy - skipping preview transcription"))?;

        let engine = engine_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Model not loaded for partial transcription"))?;

        // Get current settings for language configuration
        let settings = get_settings(&self.app_handle);

        // Perform transcription with the appropriate engine
        let result = match engine {
            LoadedEngine::Whisper(whisper_engine) => {
                let whisper_language = if settings.multilingual_mode_enabled {
                    None
                } else if settings.selected_language == "auto" {
                    None
                } else {
                    let normalized = if settings.selected_language == "zh-Hans"
                        || settings.selected_language == "zh-Hant"
                    {
                        "zh".to_string()
                    } else {
                        settings.selected_language.clone()
                    };
                    Some(normalized)
                };

                let params = WhisperInferenceParams {
                    language: whisper_language,
                    translate: false, // Skip translation for speed
                    ..Default::default()
                };

                whisper_engine
                    .transcribe_samples(audio, Some(params))
                    .map_err(|e| anyhow::anyhow!("Whisper partial transcription failed: {}", e))?
            }
            LoadedEngine::Parakeet(parakeet_engine) => {
                let params = ParakeetInferenceParams {
                    timestamp_granularity: TimestampGranularity::Segment,
                    ..Default::default()
                };
                parakeet_engine
                    .transcribe_samples(audio, Some(params))
                    .map_err(|e| anyhow::anyhow!("Parakeet partial transcription failed: {}", e))?
            }
            LoadedEngine::Moonshine(moonshine_engine) => moonshine_engine
                .transcribe_samples(audio, None)
                .map_err(|e| anyhow::anyhow!("Moonshine partial transcription failed: {}", e))?,
            LoadedEngine::GroqCloud { model_id } => {
                // Cloud transcription for partial preview
                let api_key = settings.groq_transcription_api_key.clone();
                let language = if settings.multilingual_mode_enabled {
                    None
                } else if settings.selected_language == "auto" {
                    None
                } else {
                    Some(settings.selected_language.clone())
                };
                let model_id_clone = model_id.clone();

                // Drop the engine lock before making network request
                // This allows final transcription to proceed if user stops recording
                drop(engine_guard);

                // Handle both tokio and non-tokio thread contexts
                let result = if let Ok(handle) = Handle::try_current() {
                    // We're in a tokio context, use block_in_place
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            groq_transcription::transcribe(
                                &api_key,
                                &model_id_clone,
                                &audio,
                                language.as_deref(),
                            )
                            .await
                        })
                    })
                } else {
                    // Not in tokio context (e.g., live preview worker thread)
                    // Create a temporary runtime for this request
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to create runtime: {}", e))?;

                    rt.block_on(async {
                        groq_transcription::transcribe(
                            &api_key,
                            &model_id_clone,
                            &audio,
                            language.as_deref(),
                        )
                        .await
                    })
                };

                let text = result.map_err(|e| {
                    anyhow::anyhow!("Groq cloud partial transcription failed: {}", e)
                })?;

                return Ok(text.trim().to_string());
            }
        };

        // Return raw text without post-processing for speed
        Ok(result.text.trim().to_string())
    }
}

impl Drop for TranscriptionManager {
    fn drop(&mut self) {
        debug!("Shutting down TranscriptionManager");

        // Signal the watcher thread to shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // Wait for the thread to finish gracefully
        if let Some(handle) = self.watcher_handle.lock().unwrap().take() {
            if let Err(e) = handle.join() {
                warn!("Failed to join idle watcher thread: {:?}", e);
            } else {
                debug!("Idle watcher thread joined successfully");
            }
        }
    }
}
