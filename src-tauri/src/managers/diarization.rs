use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use pyannote_rs::{get_segments, EmbeddingExtractor, EmbeddingManager, Segment};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

/// Minimum audio duration in seconds for diarization to be meaningful
const MIN_AUDIO_DURATION_SECS: f32 = 1.0;

/// Sample rate required by pyannote models
const SAMPLE_RATE: u32 = 16000;

/// Similarity threshold for speaker matching (lower = more lenient, groups similar voices together)
/// 0.5 is too strict and splits same speaker into multiple. 0.25 is more lenient.
const SPEAKER_SIMILARITY_THRESHOLD: f32 = 0.25;

/// Segment with speaker identification
#[derive(Clone, Debug)]
pub struct DiarizedSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: String,
}

/// Status of diarization models
#[derive(Clone, Debug, PartialEq)]
pub enum DiarizationModelStatus {
    NotDownloaded,
    Downloading { progress: f32 },
    Ready,
    Error(String),
}

/// Diarization models holder
struct DiarizationModels {
    segmentation_path: PathBuf,
    embedding_extractor: EmbeddingExtractor,
}

/// Manager for speaker diarization using pyannote-rs
///
/// Pyannote provides more accurate speaker diarization (~10-11% DER vs ~15-20% for Sortformer)
/// and supports unlimited speakers (vs 4-speaker limit in Sortformer).
pub struct DiarizationManager {
    app_handle: AppHandle,
    model_status: Arc<Mutex<DiarizationModelStatus>>,
    models: Arc<Mutex<Option<DiarizationModels>>>,
}

impl DiarizationManager {
    /// Pyannote segmentation model for detecting speech segments
    /// Uses the official pyannote-rs model (compatible with the library)
    const SEGMENTATION_MODEL: &'static str = "segmentation-3.0.onnx";
    const SEGMENTATION_URL: &'static str =
        "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/segmentation-3.0.onnx";
    const SEGMENTATION_SIZE: u64 = 6_000_000; // ~6MB

    /// WeSpeaker embedding model for speaker identification
    /// Uses the official pyannote-rs compatible model
    const EMBEDDING_MODEL: &'static str = "wespeaker_en_voxceleb_CAM++.onnx";
    const EMBEDDING_URL: &'static str =
        "https://github.com/thewh1teagle/pyannote-rs/releases/download/v0.1.0/wespeaker_en_voxceleb_CAM++.onnx";
    const EMBEDDING_SIZE: u64 = 27_000_000; // ~27MB

    /// Total model size for progress reporting
    const TOTAL_MODEL_SIZE: u64 = Self::SEGMENTATION_SIZE + Self::EMBEDDING_SIZE; // ~33MB

    pub fn new(app: &AppHandle) -> Result<Self> {
        let manager = Self {
            app_handle: app.clone(),
            model_status: Arc::new(Mutex::new(DiarizationModelStatus::NotDownloaded)),
            models: Arc::new(Mutex::new(None)),
        };

        // Check if models are already available
        if manager.check_models_exist() {
            if let Err(e) = manager.initialize_models() {
                warn!("Failed to initialize pyannote diarizer: {}", e);
                if let Ok(mut status) = manager.model_status.lock() {
                    *status = DiarizationModelStatus::Error(e.to_string());
                }
            }
        }

        Ok(manager)
    }

    fn get_models_dir(&self) -> Result<PathBuf> {
        // Use app data directory (writable) instead of resources (read-only)
        self.app_handle
            .path()
            .app_data_dir()
            .map(|p| p.join("models/diarization"))
            .map_err(|e| anyhow!("Failed to resolve models directory: {}", e))
    }

    fn get_segmentation_model_path(&self) -> Result<PathBuf> {
        Ok(self.get_models_dir()?.join(Self::SEGMENTATION_MODEL))
    }

    fn get_embedding_model_path(&self) -> Result<PathBuf> {
        Ok(self.get_models_dir()?.join(Self::EMBEDDING_MODEL))
    }

    fn check_models_exist(&self) -> bool {
        self.get_segmentation_model_path()
            .map(|p| p.exists())
            .unwrap_or(false)
            && self
                .get_embedding_model_path()
                .map(|p| p.exists())
                .unwrap_or(false)
    }

    fn initialize_models(&self) -> Result<()> {
        let segmentation_path = self.get_segmentation_model_path()?;
        let embedding_path = self.get_embedding_model_path()?;

        info!("Checking diarization model paths:");
        info!(
            "  Segmentation: {:?} (exists: {})",
            segmentation_path,
            segmentation_path.exists()
        );
        info!(
            "  Embedding: {:?} (exists: {})",
            embedding_path,
            embedding_path.exists()
        );

        if !segmentation_path.exists() {
            return Err(anyhow!(
                "Segmentation model not found at {:?}",
                segmentation_path
            ));
        }

        if !embedding_path.exists() {
            return Err(anyhow!("Embedding model not found at {:?}", embedding_path));
        }

        info!("Initializing pyannote diarization models...");

        // Initialize the embedding extractor
        info!("Loading embedding extractor from {:?}", embedding_path);
        let embedding_extractor = EmbeddingExtractor::new(&embedding_path).map_err(|e| {
            warn!("Failed to create embedding extractor: {}", e);
            anyhow!("Failed to create embedding extractor: {}", e)
        })?;
        info!("Embedding extractor loaded successfully");

        let models = DiarizationModels {
            segmentation_path,
            embedding_extractor,
        };

        // Update state with proper error handling
        match self.models.lock() {
            Ok(mut guard) => *guard = Some(models),
            Err(e) => return Err(anyhow!("Failed to acquire models lock: {}", e)),
        }

        match self.model_status.lock() {
            Ok(mut guard) => *guard = DiarizationModelStatus::Ready,
            Err(e) => return Err(anyhow!("Failed to acquire status lock: {}", e)),
        }

        info!("Pyannote diarization models initialized successfully");
        Ok(())
    }

    /// Get the current status of diarization models
    pub fn get_status(&self) -> DiarizationModelStatus {
        self.model_status
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or(DiarizationModelStatus::Error(
                "Failed to acquire lock".to_string(),
            ))
    }

    /// Check if diarization is available
    pub fn is_available(&self) -> bool {
        matches!(self.get_status(), DiarizationModelStatus::Ready)
    }

    /// Convert f32 samples to i16 for pyannote-rs
    fn convert_f32_to_i16(samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&s| {
                // Clamp to [-1.0, 1.0] and scale to i16 range
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * i16::MAX as f32) as i16
            })
            .collect()
    }

    /// Perform speaker diarization on audio samples
    ///
    /// # Arguments
    /// * `samples` - Audio samples at 16kHz sample rate (f32, mono)
    ///
    /// # Returns
    /// A vector of diarized segments with speaker labels.
    /// Returns an empty vector if audio is too short for meaningful diarization.
    pub fn diarize(&self, samples: &[f32]) -> Result<Vec<DiarizedSegment>> {
        let duration_secs = samples.len() as f32 / SAMPLE_RATE as f32;

        // Check minimum duration
        if duration_secs < MIN_AUDIO_DURATION_SECS {
            debug!(
                "Audio too short for diarization ({:.2}s < {:.2}s minimum)",
                duration_secs, MIN_AUDIO_DURATION_SECS
            );
            return Ok(Vec::new());
        }

        // Convert f32 samples to i16 for pyannote-rs
        let samples_i16 = Self::convert_f32_to_i16(samples);

        let mut models_guard = self
            .models
            .lock()
            .map_err(|e| anyhow!("Failed to acquire models lock: {}", e))?;

        let models = models_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Models not initialized. Download the models first."))?;

        debug!(
            "Starting diarization on {} samples ({:.2}s)",
            samples.len(),
            duration_secs
        );

        // Run pyannote segmentation to detect speech segments
        info!(
            "Running segmentation with model: {:?}",
            models.segmentation_path
        );
        let segments_iter = get_segments(&samples_i16, SAMPLE_RATE, &models.segmentation_path)
            .map_err(|e| {
                warn!("Segmentation model failed: {}", e);
                anyhow!("Segmentation failed: {}", e)
            })?;

        // Collect segments (consuming the iterator), log any errors
        let mut segments = Vec::new();
        let mut segment_errors = 0;
        for result in segments_iter {
            match result {
                Ok(segment) => segments.push(segment),
                Err(e) => {
                    segment_errors += 1;
                    if segment_errors <= 3 {
                        warn!("Segment extraction error: {}", e);
                    }
                }
            }
        }

        if segment_errors > 0 {
            warn!("Had {} segment extraction errors", segment_errors);
        }

        if segments.is_empty() {
            info!(
                "No speech segments found in audio (duration: {:.2}s, errors: {})",
                duration_secs, segment_errors
            );
            return Ok(Vec::new());
        }

        info!("Found {} speech segments for diarization", segments.len());

        // Create embedding manager for speaker clustering
        let mut speaker_manager = EmbeddingManager::new(100); // Support up to 100 speakers

        // Process each segment
        let mut diarized_segments = Vec::with_capacity(segments.len());

        for segment in &segments {
            // Compute embedding for this segment
            let embedding_result = models.embedding_extractor.compute(&segment.samples);

            let speaker_id = match embedding_result {
                Ok(embedding_iter) => {
                    let embedding: Vec<f32> = embedding_iter.collect();

                    // Try to match to existing speaker or create new one
                    match speaker_manager
                        .search_speaker(embedding.clone(), SPEAKER_SIMILARITY_THRESHOLD)
                    {
                        Some(id) => id,
                        None => {
                            // New speaker - get best match will add it
                            speaker_manager
                                .get_best_speaker_match(embedding)
                                .unwrap_or(0)
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to compute embedding for segment: {}", e);
                    0 // Default to speaker 0 on error
                }
            };

            diarized_segments.push(DiarizedSegment {
                start_ms: (segment.start * 1000.0) as u64,
                end_ms: (segment.end * 1000.0) as u64,
                speaker: format!("Speaker {}", speaker_id),
            });
        }

        debug!(
            "Diarization complete: {} segments, {} unique speakers",
            diarized_segments.len(),
            speaker_manager.get_all_speakers().len()
        );

        Ok(diarized_segments)
    }

    /// Download diarization models from HuggingFace
    pub async fn download_models(&self) -> Result<()> {
        let models_dir = self.get_models_dir()?;
        std::fs::create_dir_all(&models_dir)?;

        // Check if both models already exist
        if self.check_models_exist() {
            info!("Pyannote models already exist, initializing...");
            return self.initialize_models();
        }

        // Update status
        if let Ok(mut status) = self.model_status.lock() {
            *status = DiarizationModelStatus::Downloading { progress: 0.0 };
        }

        let mut total_downloaded: u64 = 0;

        // Download segmentation model
        let segmentation_path = models_dir.join(Self::SEGMENTATION_MODEL);
        if !segmentation_path.exists() {
            info!(
                "Downloading segmentation model from {}",
                Self::SEGMENTATION_URL
            );
            total_downloaded += self
                .download_single_model(
                    Self::SEGMENTATION_URL,
                    &segmentation_path,
                    Self::SEGMENTATION_SIZE,
                    total_downloaded,
                )
                .await?;
        } else {
            total_downloaded += Self::SEGMENTATION_SIZE;
        }

        // Download embedding model
        let embedding_path = models_dir.join(Self::EMBEDDING_MODEL);
        if !embedding_path.exists() {
            info!("Downloading embedding model from {}", Self::EMBEDDING_URL);
            self.download_single_model(
                Self::EMBEDDING_URL,
                &embedding_path,
                Self::EMBEDDING_SIZE,
                total_downloaded,
            )
            .await?;
        }

        info!("Downloaded pyannote models successfully");

        // Initialize after successful download
        self.initialize_models()
    }

    /// Download a single model file with progress tracking
    async fn download_single_model(
        &self,
        url: &str,
        path: &PathBuf,
        expected_size: u64,
        already_downloaded: u64,
    ) -> Result<u64> {
        use futures_util::StreamExt;
        use std::io::Write;

        let temp_path = path.with_extension("tmp");

        let response = reqwest::get(url)
            .await
            .map_err(|e| anyhow!("Failed to connect to download server: {}", e))?;

        if !response.status().is_success() {
            let error_msg = format!("Download failed: HTTP {}", response.status());
            if let Ok(mut status) = self.model_status.lock() {
                *status = DiarizationModelStatus::Error(error_msg.clone());
            }
            return Err(anyhow!(error_msg));
        }

        let _total_size = response.content_length().unwrap_or(expected_size);
        let mut downloaded: u64 = 0;

        // Write to temp file first
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| anyhow!("Failed to create temp file: {}", e))?;

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow!("Download interrupted: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| anyhow!("Failed to write to temp file: {}", e))?;
            downloaded += chunk.len() as u64;

            // Update overall progress
            let total_progress =
                (already_downloaded + downloaded) as f32 / Self::TOTAL_MODEL_SIZE as f32;

            if let Ok(mut status) = self.model_status.lock() {
                *status = DiarizationModelStatus::Downloading {
                    progress: total_progress.min(1.0),
                };
            }
        }

        drop(file);

        // Rename temp file to final path
        std::fs::rename(&temp_path, path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            anyhow!("Failed to finalize model file: {}", e)
        })?;

        Ok(downloaded)
    }

    /// Get the required model files and their expected sizes
    pub fn get_model_info() -> Vec<(&'static str, &'static str, u64)> {
        vec![
            (
                Self::SEGMENTATION_MODEL,
                "Pyannote segmentation model (speech detection)",
                Self::SEGMENTATION_SIZE,
            ),
            (
                Self::EMBEDDING_MODEL,
                "Pyannote embedding model (speaker identification)",
                Self::EMBEDDING_SIZE,
            ),
        ]
    }

    /// Get total model size for display
    pub fn get_total_model_size() -> u64 {
        Self::TOTAL_MODEL_SIZE
    }

    /// Assign speakers to transcription segments based on diarization results
    ///
    /// This function takes transcription segments with timestamps and assigns
    /// speaker labels by finding the best matching diarization segment for each.
    ///
    /// # Arguments
    /// * `transcription_segments` - Segments with (start_ms, end_ms, text)
    /// * `diarization_segments` - Segments from diarize() with speaker labels
    ///
    /// # Returns
    /// Segments with (start_ms, end_ms, text, speaker)
    pub fn assign_speakers_to_segments(
        transcription_segments: &[(u64, u64, String)],
        diarization_segments: &[DiarizedSegment],
    ) -> Vec<(u64, u64, String, Option<String>)> {
        if diarization_segments.is_empty() {
            // No diarization data - return segments without speaker labels
            return transcription_segments
                .iter()
                .map(|(start, end, text)| (*start, *end, text.clone(), None))
                .collect();
        }

        transcription_segments
            .iter()
            .map(|(start_ms, end_ms, text)| {
                // Find the diarization segment with the most overlap
                let speaker = diarization_segments
                    .iter()
                    .filter_map(|diar| {
                        // Calculate overlap between transcription and diarization segments
                        let overlap_start = (*start_ms).max(diar.start_ms);
                        let overlap_end = (*end_ms).min(diar.end_ms);

                        if overlap_start < overlap_end {
                            let overlap_duration = overlap_end - overlap_start;
                            Some((overlap_duration, &diar.speaker))
                        } else {
                            None
                        }
                    })
                    .max_by_key(|(overlap, _)| *overlap)
                    .map(|(_, speaker)| speaker.clone());

                (*start_ms, *end_ms, text.clone(), speaker)
            })
            .collect()
    }
}
