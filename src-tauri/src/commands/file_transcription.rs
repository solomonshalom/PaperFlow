use crate::managers::file_transcription::{FileTranscriptionJob, FileTranscriptionManager};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Get list of supported file extensions
#[tauri::command]
#[specta::specta]
pub fn get_supported_file_extensions() -> Vec<String> {
    FileTranscriptionManager::get_supported_extensions()
}

/// Queue a file for transcription
#[tauri::command]
#[specta::specta]
pub async fn queue_file_for_transcription(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
    file_path: String,
) -> Result<FileTranscriptionJob, String> {
    file_manager
        .queue_file(&file_path)
        .map_err(|e| e.to_string())
}

/// Queue multiple files for transcription
#[tauri::command]
#[specta::specta]
pub async fn queue_files_for_transcription(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
    file_paths: Vec<String>,
) -> Result<Vec<FileTranscriptionJob>, String> {
    file_manager
        .queue_files(&file_paths)
        .map_err(|e| e.to_string())
}

/// Process the next queued file
#[tauri::command]
#[specta::specta]
pub async fn process_next_file(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
) -> Result<Option<String>, String> {
    file_manager.process_next().map_err(|e| e.to_string())
}

/// Process all queued files (runs on background thread to avoid blocking UI)
#[tauri::command]
#[specta::specta]
pub async fn process_all_files(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
) -> Result<(), String> {
    let manager = file_manager.inner().clone();

    // Spawn on background thread to avoid blocking the main/UI thread
    tokio::task::spawn_blocking(move || {
        if let Err(e) = manager.process_all() {
            log::error!("Error processing files: {}", e);
        }
    });

    // Return immediately - progress updates come via events
    Ok(())
}

/// Cancel the current file transcription
#[tauri::command]
#[specta::specta]
pub fn cancel_file_transcription(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
) {
    file_manager.cancel_current();
}

/// Cancel a specific job by ID
#[tauri::command]
#[specta::specta]
pub fn cancel_file_transcription_job(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
    job_id: String,
) -> Result<(), String> {
    file_manager.cancel_job(&job_id).map_err(|e| e.to_string())
}

/// Get all transcription jobs
#[tauri::command]
#[specta::specta]
pub fn get_file_transcription_jobs(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
) -> Vec<FileTranscriptionJob> {
    file_manager.get_jobs()
}

/// Get a specific job by ID
#[tauri::command]
#[specta::specta]
pub fn get_file_transcription_job(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
    job_id: String,
) -> Option<FileTranscriptionJob> {
    file_manager.get_job(&job_id)
}

/// Clear completed, failed, and cancelled jobs
#[tauri::command]
#[specta::specta]
pub fn clear_completed_file_jobs(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
) {
    file_manager.clear_completed();
}

/// Remove a specific job
#[tauri::command]
#[specta::specta]
pub fn remove_file_transcription_job(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
    job_id: String,
) -> Result<(), String> {
    file_manager.remove_job(&job_id).map_err(|e| e.to_string())
}

/// Check if file transcription is currently processing
#[tauri::command]
#[specta::specta]
pub fn is_file_transcription_processing(
    _app: AppHandle,
    file_manager: State<'_, Arc<FileTranscriptionManager>>,
) -> bool {
    file_manager.is_processing()
}
