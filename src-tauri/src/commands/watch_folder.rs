use crate::managers::watch_folder::{
    self, WatchFolderConfig, WatchFolderManager, WatchFolderStatus,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Get all configured watch folders
#[tauri::command]
#[specta::specta]
pub fn get_watch_folders(app: AppHandle) -> Vec<WatchFolderConfig> {
    watch_folder::get_watch_folders(&app)
}

/// Add a new watch folder
#[tauri::command]
#[specta::specta]
pub fn add_watch_folder(
    app: AppHandle,
    manager: State<'_, Arc<WatchFolderManager>>,
    path: String,
    recursive: bool,
) -> Result<WatchFolderConfig, String> {
    let config =
        watch_folder::add_watch_folder(&app, path, recursive).map_err(|e| e.to_string())?;

    // Start watching if enabled
    if config.enabled {
        manager.start_watching(&config).map_err(|e| e.to_string())?;
    }

    Ok(config)
}

/// Remove a watch folder
#[tauri::command]
#[specta::specta]
pub fn remove_watch_folder(
    app: AppHandle,
    manager: State<'_, Arc<WatchFolderManager>>,
    folder_id: String,
) -> Result<(), String> {
    // Stop watching first
    manager
        .stop_watching(&folder_id)
        .map_err(|e| e.to_string())?;

    // Remove from settings
    watch_folder::remove_watch_folder(&app, &folder_id).map_err(|e| e.to_string())
}

/// Update a watch folder configuration
#[tauri::command]
#[specta::specta]
pub fn update_watch_folder(
    app: AppHandle,
    manager: State<'_, Arc<WatchFolderManager>>,
    config: WatchFolderConfig,
) -> Result<(), String> {
    let was_enabled = {
        let folders = watch_folder::get_watch_folders(&app);
        folders
            .iter()
            .find(|f| f.id == config.id)
            .map(|f| f.enabled)
            .unwrap_or(false)
    };

    // Update settings
    watch_folder::update_watch_folder(&app, config.clone()).map_err(|e| e.to_string())?;

    // Handle watching state changes
    if config.enabled && !was_enabled {
        // Start watching
        manager.start_watching(&config).map_err(|e| e.to_string())?;
    } else if !config.enabled && was_enabled {
        // Stop watching
        manager
            .stop_watching(&config.id)
            .map_err(|e| e.to_string())?;
    } else if config.enabled {
        // Restart with new config (e.g., recursive changed)
        manager
            .stop_watching(&config.id)
            .map_err(|e| e.to_string())?;
        manager.start_watching(&config).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Get the status of all watch folders
#[tauri::command]
#[specta::specta]
pub fn get_watch_folder_status(
    manager: State<'_, Arc<WatchFolderManager>>,
) -> Vec<WatchFolderStatus> {
    manager.get_all_status()
}

/// Start watching a specific folder
#[tauri::command]
#[specta::specta]
pub fn start_watch_folder(
    app: AppHandle,
    manager: State<'_, Arc<WatchFolderManager>>,
    folder_id: String,
) -> Result<(), String> {
    let folders = watch_folder::get_watch_folders(&app);
    let config = folders
        .iter()
        .find(|f| f.id == folder_id)
        .ok_or_else(|| format!("Watch folder not found: {}", folder_id))?;

    manager.start_watching(config).map_err(|e| e.to_string())
}

/// Stop watching a specific folder
#[tauri::command]
#[specta::specta]
pub fn stop_watch_folder(
    manager: State<'_, Arc<WatchFolderManager>>,
    folder_id: String,
) -> Result<(), String> {
    manager.stop_watching(&folder_id).map_err(|e| e.to_string())
}
