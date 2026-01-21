use crate::managers::diarization::{DiarizationManager, DiarizationModelStatus};
use crate::settings::{get_settings, write_settings};
use log::warn;
use serde::Serialize;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Diarization status response
#[derive(Serialize, Type)]
pub struct DiarizationStatus {
    pub available: bool,
    pub enabled: bool,
    pub models_downloaded: bool,
    pub download_progress: Option<f32>,
    pub error: Option<String>,
}

/// Get the current diarization status
#[tauri::command]
#[specta::specta]
pub fn get_diarization_status(app: AppHandle) -> DiarizationStatus {
    let settings = get_settings(&app);

    if let Some(dm) = app.try_state::<Arc<DiarizationManager>>() {
        let status = dm.get_status();
        match status {
            DiarizationModelStatus::NotDownloaded => DiarizationStatus {
                available: false,
                enabled: settings.diarization_enabled,
                models_downloaded: false,
                download_progress: None,
                error: None,
            },
            DiarizationModelStatus::Downloading { progress } => DiarizationStatus {
                available: false,
                enabled: settings.diarization_enabled,
                models_downloaded: false,
                download_progress: Some(progress),
                error: None,
            },
            DiarizationModelStatus::Ready => DiarizationStatus {
                available: true,
                enabled: settings.diarization_enabled,
                models_downloaded: true,
                download_progress: None,
                error: None,
            },
            DiarizationModelStatus::Error(err) => DiarizationStatus {
                available: false,
                enabled: settings.diarization_enabled,
                // Model might be downloaded but failed to initialize
                models_downloaded: false,
                download_progress: None,
                error: Some(err),
            },
        }
    } else {
        DiarizationStatus {
            available: false,
            enabled: settings.diarization_enabled,
            models_downloaded: false,
            download_progress: None,
            error: Some("Diarization manager not initialized".to_string()),
        }
    }
}

/// Enable or disable diarization
#[tauri::command]
#[specta::specta]
pub fn change_diarization_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    // Only allow enabling if models are downloaded and ready
    if enabled {
        if let Some(dm) = app.try_state::<Arc<DiarizationManager>>() {
            if !dm.is_available() {
                return Err(
                    "Cannot enable diarization: models not downloaded or initialization failed"
                        .to_string(),
                );
            }
        } else {
            return Err("Diarization manager not available".to_string());
        }
    }

    let mut settings = get_settings(&app);
    settings.diarization_enabled = enabled;
    write_settings(&app, settings);
    Ok(())
}

/// Model info for display in UI
#[derive(Serialize, Type)]
pub struct DiarizationModelInfo {
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
}

/// Get information about required models
#[tauri::command]
#[specta::specta]
pub fn get_diarization_model_info() -> Vec<DiarizationModelInfo> {
    DiarizationManager::get_model_info()
        .into_iter()
        .map(|(name, description, size)| DiarizationModelInfo {
            name: name.to_string(),
            description: description.to_string(),
            size_bytes: size,
        })
        .collect()
}

/// Download diarization models
#[tauri::command]
#[specta::specta]
pub async fn download_diarization_models(app: AppHandle) -> Result<(), String> {
    let dm = app
        .try_state::<Arc<DiarizationManager>>()
        .ok_or_else(|| "Diarization manager not initialized".to_string())?;

    // Check if already downloading
    let status = dm.get_status();
    if matches!(status, DiarizationModelStatus::Downloading { .. }) {
        warn!("Download already in progress, ignoring duplicate request");
        return Ok(());
    }

    // Check if already ready
    if matches!(status, DiarizationModelStatus::Ready) {
        return Ok(());
    }

    dm.download_models()
        .await
        .map_err(|e| format!("Failed to download models: {}", e))
}
