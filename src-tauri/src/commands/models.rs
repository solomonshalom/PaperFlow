use crate::groq_transcription;
use crate::managers::model::{EngineType, ModelInfo, ModelManager};
use crate::managers::transcription::TranscriptionManager;
use crate::settings::{get_settings, write_settings};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub async fn get_available_models(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    Ok(model_manager.get_available_models())
}

#[tauri::command]
#[specta::specta]
pub async fn get_model_info(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<Option<ModelInfo>, String> {
    Ok(model_manager.get_model_info(&model_id))
}

#[tauri::command]
#[specta::specta]
pub async fn download_model(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .delete_model(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_model(
    app_handle: AppHandle,
    model_manager: State<'_, Arc<ModelManager>>,
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
    model_id: String,
) -> Result<(), String> {
    // Check if model exists and is available
    let model_info = model_manager
        .get_model_info(&model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;

    if !model_info.is_downloaded {
        return Err(format!("Model not downloaded: {}", model_id));
    }

    // Load the model in the transcription manager
    transcription_manager
        .load_model(&model_id)
        .map_err(|e| e.to_string())?;

    // Update settings
    let mut settings = get_settings(&app_handle);
    settings.selected_model = model_id.clone();
    write_settings(&app_handle, settings);

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_current_model(app_handle: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app_handle);
    Ok(settings.selected_model)
}

#[tauri::command]
#[specta::specta]
pub async fn get_transcription_model_status(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<Option<String>, String> {
    Ok(transcription_manager.get_current_model())
}

#[tauri::command]
#[specta::specta]
pub async fn is_model_loading(
    transcription_manager: State<'_, Arc<TranscriptionManager>>,
) -> Result<bool, String> {
    // Check if transcription manager has a loaded model
    let current_model = transcription_manager.get_current_model();
    Ok(current_model.is_none())
}

#[tauri::command]
#[specta::specta]
pub async fn has_any_models_available(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<bool, String> {
    let models = model_manager.get_available_models();
    Ok(models.iter().any(|m| m.is_downloaded))
}

#[tauri::command]
#[specta::specta]
pub async fn has_any_models_or_downloads(
    model_manager: State<'_, Arc<ModelManager>>,
) -> Result<bool, String> {
    let models = model_manager.get_available_models();
    // Return true if any models are downloaded OR if any downloads are in progress
    Ok(models.iter().any(|m| m.is_downloaded))
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_download(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .cancel_download(&model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_recommended_first_model() -> Result<String, String> {
    // Recommend Parakeet V3 model for first-time users - fastest and most accurate
    Ok("parakeet-tdt-0.6b-v3".to_string())
}

/// Download CoreML model for Apple Neural Engine acceleration (macOS only)
#[cfg(target_os = "macos")]
#[tauri::command]
#[specta::specta]
pub async fn download_coreml_model(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .download_coreml_model(&model_id)
        .await
        .map_err(|e| e.to_string())
}

/// Placeholder for non-macOS platforms
#[cfg(not(target_os = "macos"))]
#[tauri::command]
#[specta::specta]
pub async fn download_coreml_model(_model_id: String) -> Result<(), String> {
    Err("CoreML is only available on macOS".to_string())
}

/// Delete CoreML model (macOS only)
#[cfg(target_os = "macos")]
#[tauri::command]
#[specta::specta]
pub async fn delete_coreml_model(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<(), String> {
    model_manager
        .delete_coreml_model(&model_id)
        .map_err(|e| e.to_string())
}

/// Placeholder for non-macOS platforms
#[cfg(not(target_os = "macos"))]
#[tauri::command]
#[specta::specta]
pub async fn delete_coreml_model(_model_id: String) -> Result<(), String> {
    Err("CoreML is only available on macOS".to_string())
}

/// Check if running on macOS (for UI to show/hide CoreML options)
#[tauri::command]
#[specta::specta]
pub fn is_coreml_available() -> bool {
    cfg!(target_os = "macos")
}

/// Validate a Groq API key before using it
/// Returns Ok(()) if valid, or an error message if invalid
#[tauri::command]
#[specta::specta]
pub async fn validate_groq_api_key(api_key: String) -> Result<(), String> {
    groq_transcription::validate_api_key(&api_key).await
}

/// Check if a model requires an API key (cloud models)
#[tauri::command]
#[specta::specta]
pub fn model_requires_api_key(
    model_manager: State<'_, Arc<ModelManager>>,
    model_id: String,
) -> Result<bool, String> {
    let model_info = model_manager
        .get_model_info(&model_id)
        .ok_or_else(|| format!("Model not found: {}", model_id))?;

    Ok(model_info.engine_type == EngineType::GroqCloud)
}

/// Check if Groq API key is configured in settings
#[tauri::command]
#[specta::specta]
pub fn is_groq_api_key_configured(app_handle: AppHandle) -> Result<bool, String> {
    let settings = get_settings(&app_handle);
    Ok(!settings.groq_transcription_api_key.is_empty())
}
