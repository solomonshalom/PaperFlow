use anyhow::{anyhow, Result};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::managers::transcription::TranscriptionManager;

/// Supported audio file extensions
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "m4a", "ogg", "opus", "flac", "aac", "wma", "aiff", "webm",
];

/// Supported video file extensions (audio will be extracted)
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv", "flv"];

/// Status of a file transcription job
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileTranscriptionStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

/// A single file transcription job
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct FileTranscriptionJob {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub status: FileTranscriptionStatus,
    pub progress: f32,
    pub transcription: Option<String>,
    pub error: Option<String>,
    pub duration_seconds: Option<f64>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

/// Event emitted during file transcription
#[derive(Clone, Debug, Serialize)]
pub struct FileTranscriptionEvent {
    pub job_id: String,
    pub status: FileTranscriptionStatus,
    pub progress: f32,
    pub transcription: Option<String>,
    pub error: Option<String>,
}

/// Manager for handling file-based transcription
pub struct FileTranscriptionManager {
    app_handle: AppHandle,
    transcription_manager: Arc<TranscriptionManager>,
    jobs: Arc<Mutex<Vec<FileTranscriptionJob>>>,
    cancel_flag: Arc<AtomicBool>,
    is_processing: Arc<AtomicBool>,
    current_job_id: Arc<Mutex<Option<String>>>,
}

impl FileTranscriptionManager {
    pub fn new(
        app_handle: &AppHandle,
        transcription_manager: Arc<TranscriptionManager>,
    ) -> Result<Self> {
        let manager = Self {
            app_handle: app_handle.clone(),
            transcription_manager,
            jobs: Arc::new(Mutex::new(Vec::new())),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            is_processing: Arc::new(AtomicBool::new(false)),
            current_job_id: Arc::new(Mutex::new(None)),
        };

        // Recovery: Reset any stuck "processing" jobs from previous session
        // (This handles the case where the app crashed mid-transcription)
        manager.recover_stuck_jobs();

        Ok(manager)
    }

    /// Reset any jobs that were stuck in "processing" state (from app crash)
    fn recover_stuck_jobs(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        for job in jobs.iter_mut() {
            if job.status == FileTranscriptionStatus::Processing {
                warn!("Recovering stuck job {} - resetting to queued", job.id);
                job.status = FileTranscriptionStatus::Queued;
                job.progress = 0.0;
                job.error = None;
            }
        }
    }

    /// Check if a file path has a supported audio extension
    pub fn is_supported_audio_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    /// Check if a file path has a supported video extension
    pub fn is_supported_video_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    /// Check if a file is supported (audio or video)
    pub fn is_supported_file(path: &Path) -> bool {
        Self::is_supported_audio_file(path) || Self::is_supported_video_file(path)
    }

    /// Get list of supported file extensions
    pub fn get_supported_extensions() -> Vec<String> {
        let mut extensions: Vec<String> = AUDIO_EXTENSIONS
            .iter()
            .chain(VIDEO_EXTENSIONS.iter())
            .map(|s| s.to_string())
            .collect();
        extensions.sort();
        extensions.dedup();
        extensions
    }

    /// Add a file to the transcription queue
    pub fn queue_file(&self, file_path: &str) -> Result<FileTranscriptionJob> {
        let path = Path::new(file_path);

        // Validate file exists
        if !path.exists() {
            return Err(anyhow!("File does not exist: {}", file_path));
        }

        // Validate file extension
        if !Self::is_supported_file(path) {
            return Err(anyhow!(
                "Unsupported file format. Supported formats: {}",
                Self::get_supported_extensions().join(", ")
            ));
        }

        // Get file metadata
        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();

        // Check file size limit (4GB max to prevent memory issues)
        const MAX_FILE_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4GB
        if file_size > MAX_FILE_SIZE {
            return Err(anyhow!(
                "File too large ({:.2} GB). Maximum supported size is 4 GB.",
                file_size as f64 / (1024.0 * 1024.0 * 1024.0)
            ));
        }

        // Warn about large files
        if file_size > 500 * 1024 * 1024 {
            // > 500MB
            warn!(
                "Large file ({:.2} MB) - transcription may take a while",
                file_size as f64 / (1024.0 * 1024.0)
            );
        }

        // Generate unique job ID
        let job_id = format!(
            "job_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0000")
        );

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let job = FileTranscriptionJob {
            id: job_id.clone(),
            file_path: file_path.to_string(),
            file_name,
            file_size,
            status: FileTranscriptionStatus::Queued,
            progress: 0.0,
            transcription: None,
            error: None,
            duration_seconds: None,
            created_at: chrono::Utc::now().timestamp(),
            completed_at: None,
        };

        // Add to queue
        {
            let mut jobs = self.jobs.lock().unwrap();
            jobs.push(job.clone());
        }

        // Emit queued event
        self.emit_job_event(&job);

        info!("Queued file for transcription: {} ({})", file_path, job_id);

        Ok(job)
    }

    /// Queue multiple files at once
    pub fn queue_files(&self, file_paths: &[String]) -> Result<Vec<FileTranscriptionJob>> {
        let mut jobs = Vec::new();
        let mut errors = Vec::new();

        for path in file_paths {
            match self.queue_file(path) {
                Ok(job) => jobs.push(job),
                Err(e) => errors.push(format!("{}: {}", path, e)),
            }
        }

        if !errors.is_empty() && jobs.is_empty() {
            return Err(anyhow!("Failed to queue files:\n{}", errors.join("\n")));
        }

        Ok(jobs)
    }

    /// Process the next job in the queue
    pub fn process_next(&self) -> Result<Option<String>> {
        // Check if already processing
        if self.is_processing.load(Ordering::SeqCst) {
            return Ok(None);
        }

        // Find next queued job
        let job_to_process = {
            let jobs = self.jobs.lock().unwrap();
            jobs.iter()
                .find(|j| j.status == FileTranscriptionStatus::Queued)
                .cloned()
        };

        let job = match job_to_process {
            Some(j) => j,
            None => return Ok(None),
        };

        // Mark as processing
        self.is_processing.store(true, Ordering::SeqCst);
        self.cancel_flag.store(false, Ordering::SeqCst);
        {
            let mut current = self.current_job_id.lock().unwrap();
            *current = Some(job.id.clone());
        }

        let job_id = job.id.clone();
        self.update_job_status(&job_id, FileTranscriptionStatus::Processing, None, None);

        // Process the file
        let result = self.process_file(&job);

        // Update final status
        match result {
            Ok(transcription) => {
                self.update_job_status(
                    &job_id,
                    FileTranscriptionStatus::Completed,
                    Some(transcription),
                    None,
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("cancelled") {
                    self.update_job_status(
                        &job_id,
                        FileTranscriptionStatus::Cancelled,
                        None,
                        Some(error_msg),
                    );
                } else {
                    self.update_job_status(
                        &job_id,
                        FileTranscriptionStatus::Failed,
                        None,
                        Some(error_msg),
                    );
                }
            }
        }

        // Clear processing state
        self.is_processing.store(false, Ordering::SeqCst);
        {
            let mut current = self.current_job_id.lock().unwrap();
            *current = None;
        }

        Ok(Some(job_id))
    }

    /// Process all queued jobs
    pub fn process_all(&self) -> Result<Vec<String>> {
        let mut processed_ids = Vec::new();

        loop {
            match self.process_next()? {
                Some(id) => processed_ids.push(id),
                None => break,
            }

            // Check for cancellation between jobs
            if self.cancel_flag.load(Ordering::SeqCst) {
                break;
            }
        }

        Ok(processed_ids)
    }

    /// Process a single file and return the transcription
    fn process_file(&self, job: &FileTranscriptionJob) -> Result<String> {
        let path = Path::new(&job.file_path);

        info!("Processing file: {}", job.file_path);

        // Ensure model is loaded before processing
        if !self.transcription_manager.is_model_loaded() {
            info!("Model not loaded, initiating load for file transcription");
            self.transcription_manager.initiate_model_load();

            // Wait for model to load (with timeout)
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120); // 2 minute timeout

            while !self.transcription_manager.is_model_loaded() {
                if start.elapsed() > timeout {
                    return Err(anyhow!(
                        "Model loading timed out. Please ensure a model is downloaded and selected."
                    ));
                }

                // Check for cancellation while waiting
                if self.cancel_flag.load(Ordering::SeqCst) {
                    return Err(anyhow!("Transcription cancelled while waiting for model"));
                }

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        // Load and decode the audio file
        let audio_samples = self.load_audio_file(path)?;

        // Check for cancellation
        if self.cancel_flag.load(Ordering::SeqCst) {
            return Err(anyhow!("Transcription cancelled"));
        }

        // Update progress
        self.update_job_progress(&job.id, 0.5);

        // Transcribe
        let transcription = self.transcription_manager.transcribe(audio_samples)?;

        // Update progress
        self.update_job_progress(&job.id, 1.0);

        Ok(transcription)
    }

    /// Load an audio file and return samples at 16kHz mono
    fn load_audio_file(&self, path: &Path) -> Result<Vec<f32>> {
        use symphonia::core::audio::SampleBuffer;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        let file = std::fs::File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a hint to help the format registry guess the format
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        // Probe the media source
        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;

        let mut format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow!("No audio track found in file"))?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or_else(|| {
            warn!("No sample rate in file metadata, assuming 44100 Hz");
            44100
        });
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count())
            .unwrap_or_else(|| {
                warn!("No channel count in file metadata, assuming stereo");
                2
            });

        // Validate channels
        if channels == 0 {
            return Err(anyhow!("Invalid audio file: 0 channels"));
        }

        // Create a decoder
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;

        let mut all_samples: Vec<f32> = Vec::new();

        // Decode all packets
        loop {
            // Check for cancellation periodically
            if self.cancel_flag.load(Ordering::SeqCst) {
                return Err(anyhow!("Transcription cancelled during audio loading"));
            }

            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => return Err(anyhow!("Error reading packet: {}", e)),
            };

            // Skip packets from other tracks
            if packet.track_id() != track_id {
                continue;
            }

            // Decode the packet
            let decoded = match decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(symphonia::core::errors::Error::DecodeError(e)) => {
                    warn!("Decode error: {}, skipping packet", e);
                    continue;
                }
                Err(e) => return Err(anyhow!("Error decoding packet: {}", e)),
            };

            // Convert to f32 samples
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;

            let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
            sample_buf.copy_interleaved_ref(decoded);

            let samples = sample_buf.samples();

            // Convert to mono if needed
            if channels > 1 {
                for chunk in samples.chunks(channels) {
                    let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    all_samples.push(mono);
                }
            } else {
                all_samples.extend_from_slice(samples);
            }
        }

        // Validate we got some audio
        if all_samples.is_empty() {
            return Err(anyhow!("No audio data found in file"));
        }

        // Resample to 16kHz if needed
        let target_sample_rate = 16000;
        if sample_rate != target_sample_rate {
            all_samples = self.resample(&all_samples, sample_rate, target_sample_rate)?;
        }

        // Final validation
        if all_samples.is_empty() {
            return Err(anyhow!("Resampling produced no audio data"));
        }

        let duration_seconds = all_samples.len() as f64 / target_sample_rate as f64;
        info!(
            "Loaded {} samples ({:.2}s) from {}",
            all_samples.len(),
            duration_seconds,
            path.display()
        );

        // Warn about very long files (> 2 hours)
        if duration_seconds > 7200.0 {
            warn!(
                "Very long audio file ({:.1} hours) - transcription may take a while",
                duration_seconds / 3600.0
            );
        }

        Ok(all_samples)
    }

    /// Resample audio from one sample rate to another
    fn resample(&self, samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
        use rubato::{
            Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
            WindowFunction,
        };

        // Handle edge cases
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        if from_rate == to_rate {
            return Ok(samples.to_vec());
        }

        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        // Ensure chunk size is at least 64 samples for the resampler
        let chunk_size = samples.len().min(1024).max(64);

        let mut resampler = SincFixedIn::<f32>::new(
            to_rate as f64 / from_rate as f64,
            2.0,
            params,
            chunk_size,
            1,
        )?;

        let mut output = Vec::new();
        let chunk_size = resampler.input_frames_max();

        for chunk in samples.chunks(chunk_size) {
            // Pad last chunk if needed
            let input = if chunk.len() < chunk_size {
                let mut padded = chunk.to_vec();
                padded.resize(chunk_size, 0.0);
                vec![padded]
            } else {
                vec![chunk.to_vec()]
            };

            let resampled = resampler.process(&input, None)?;
            if !resampled.is_empty() {
                output.extend_from_slice(&resampled[0]);
            }
        }

        Ok(output)
    }

    /// Cancel the current transcription job
    pub fn cancel_current(&self) {
        info!("Cancelling current file transcription");
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Cancel a specific job by ID
    pub fn cancel_job(&self, job_id: &str) -> Result<()> {
        // Check if this is the current job - release lock before potentially acquiring jobs lock
        let is_current_job = {
            let current = self.current_job_id.lock().unwrap();
            current.as_deref() == Some(job_id)
        };

        if is_current_job {
            // Cancel the current job
            self.cancel_current();
        } else {
            // Remove from queue if not yet processing
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                if job.status == FileTranscriptionStatus::Queued {
                    job.status = FileTranscriptionStatus::Cancelled;
                    let job_clone = job.clone();
                    drop(jobs); // Release lock before emitting
                    self.emit_job_event(&job_clone);
                } else {
                    return Err(anyhow!(
                        "Cannot cancel job that is not queued or already completed"
                    ));
                }
            } else {
                return Err(anyhow!("Job not found: {}", job_id));
            }
        }

        Ok(())
    }

    /// Get all jobs
    pub fn get_jobs(&self) -> Vec<FileTranscriptionJob> {
        self.jobs.lock().unwrap().clone()
    }

    /// Get a specific job by ID
    pub fn get_job(&self, job_id: &str) -> Option<FileTranscriptionJob> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .find(|j| j.id == job_id)
            .cloned()
    }

    /// Clear completed and failed jobs
    pub fn clear_completed(&self) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| {
            j.status != FileTranscriptionStatus::Completed
                && j.status != FileTranscriptionStatus::Failed
                && j.status != FileTranscriptionStatus::Cancelled
        });
    }

    /// Remove a specific job
    pub fn remove_job(&self, job_id: &str) -> Result<()> {
        let mut jobs = self.jobs.lock().unwrap();
        let initial_len = jobs.len();
        jobs.retain(|j| j.id != job_id);

        if jobs.len() == initial_len {
            return Err(anyhow!("Job not found: {}", job_id));
        }

        Ok(())
    }

    /// Check if currently processing
    pub fn is_processing(&self) -> bool {
        self.is_processing.load(Ordering::SeqCst)
    }

    /// Update job status
    fn update_job_status(
        &self,
        job_id: &str,
        status: FileTranscriptionStatus,
        transcription: Option<String>,
        error: Option<String>,
    ) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.status = status.clone();
            if transcription.is_some() {
                job.transcription = transcription.clone();
            }
            if error.is_some() {
                job.error = error.clone();
            }
            if status == FileTranscriptionStatus::Completed
                || status == FileTranscriptionStatus::Failed
                || status == FileTranscriptionStatus::Cancelled
            {
                job.completed_at = Some(chrono::Utc::now().timestamp());
                job.progress = if status == FileTranscriptionStatus::Completed {
                    1.0
                } else {
                    job.progress
                };
            }

            let job_clone = job.clone();
            drop(jobs); // Release lock before emitting
            self.emit_job_event(&job_clone);
        }
    }

    /// Update job progress
    fn update_job_progress(&self, job_id: &str, progress: f32) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.progress = progress;
            let job_clone = job.clone();
            drop(jobs);
            self.emit_job_event(&job_clone);
        }
    }

    /// Emit job event to frontend
    fn emit_job_event(&self, job: &FileTranscriptionJob) {
        let event = FileTranscriptionEvent {
            job_id: job.id.clone(),
            status: job.status.clone(),
            progress: job.progress,
            transcription: job.transcription.clone(),
            error: job.error.clone(),
        };

        if let Err(e) = self.app_handle.emit("file-transcription-update", event) {
            error!("Failed to emit file transcription event: {}", e);
        }
    }
}
