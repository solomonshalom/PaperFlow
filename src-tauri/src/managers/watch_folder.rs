use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use super::file_transcription::FileTranscriptionManager;
use crate::settings::{get_settings, write_settings};

/// Supported audio/video extensions for watch folder
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "m4a", "flac", "ogg", "aac", "wma", "aiff", "mp4", "mkv", "avi", "mov", "webm",
];

/// Configuration for a watched folder
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct WatchFolderConfig {
    pub id: String,
    pub path: String,
    pub enabled: bool,
    pub recursive: bool,
    pub auto_process: bool,
}

/// Status of a watch folder
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct WatchFolderStatus {
    pub folder_id: String,
    pub is_watching: bool,
    pub last_error: Option<String>,
    pub files_processed: u32,
}

/// Event payload for watch folder file detection
#[derive(Clone, Debug, Serialize, Type)]
pub struct WatchFolderFileDetected {
    pub folder_id: String,
    pub file_path: String,
    pub file_name: String,
}

/// Internal state for a single watcher
struct WatcherState {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    config: WatchFolderConfig,
    is_watching: bool,
    last_error: Option<String>,
    files_processed: u32,
}

/// Manager for watch folder functionality
pub struct WatchFolderManager {
    app_handle: AppHandle,
    watchers: Arc<Mutex<HashMap<String, WatcherState>>>,
    /// Track recently processed files to avoid duplicates (path -> timestamp)
    recent_files: Arc<Mutex<HashMap<String, Instant>>>,
    /// Debounce duration in seconds
    debounce_seconds: u64,
}

impl WatchFolderManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let manager = Self {
            app_handle: app_handle.clone(),
            watchers: Arc::new(Mutex::new(HashMap::new())),
            recent_files: Arc::new(Mutex::new(HashMap::new())),
            debounce_seconds: 5,
        };

        Ok(manager)
    }

    /// Start watching all enabled folders from settings
    pub fn start_all(&self) -> Result<()> {
        let settings = get_settings(&self.app_handle);
        let folders = settings.watch_folders.unwrap_or_default();

        for folder in folders {
            if folder.enabled {
                if let Err(e) = self.start_watching(&folder) {
                    error!("Failed to start watching folder {}: {}", folder.path, e);
                }
            }
        }

        Ok(())
    }

    /// Start watching a specific folder
    pub fn start_watching(&self, config: &WatchFolderConfig) -> Result<()> {
        let path = PathBuf::from(&config.path);
        if !path.exists() {
            let err = format!("Folder does not exist: {}", config.path);
            self.set_folder_error(&config.id, Some(err.clone()));
            return Err(anyhow!(err));
        }

        if !path.is_dir() {
            let err = format!("Path is not a directory: {}", config.path);
            self.set_folder_error(&config.id, Some(err.clone()));
            return Err(anyhow!(err));
        }

        // Check read permissions by attempting to read the directory
        match std::fs::read_dir(&path) {
            Ok(_) => {}
            Err(e) => {
                let err = format!("Cannot read folder (permission denied?): {}", e);
                self.set_folder_error(&config.id, Some(err.clone()));
                return Err(anyhow!(err));
            }
        }

        // Check if already watching
        {
            let watchers = self
                .watchers
                .lock()
                .map_err(|e| anyhow!("Lock error: {}", e))?;
            if let Some(state) = watchers.get(&config.id) {
                if state.is_watching {
                    debug!("Already watching folder: {}", config.path);
                    return Ok(());
                }
            }
        }

        let app_handle = self.app_handle.clone();
        let recent_files = self.recent_files.clone();
        let debounce_seconds = self.debounce_seconds;
        let folder_id = config.id.clone();
        let auto_process = config.auto_process;

        // Create the watcher with event handler
        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| match result {
                Ok(event) => {
                    Self::handle_event(
                        &app_handle,
                        &recent_files,
                        debounce_seconds,
                        &folder_id,
                        auto_process,
                        event,
                    );
                }
                Err(e) => {
                    error!("Watch error: {}", e);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| {
            let err = format!("Failed to create watcher: {}", e);
            self.set_folder_error(&config.id, Some(err.clone()));
            anyhow!(err)
        })?;

        // Start watching the path
        let recursive_mode = if config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher.watch(&path, recursive_mode).map_err(|e| {
            let err = format!("Failed to watch path: {}", e);
            self.set_folder_error(&config.id, Some(err.clone()));
            anyhow!(err)
        })?;

        info!(
            "Started watching folder: {} (recursive: {})",
            config.path, config.recursive
        );

        // Clear any previous error on successful start
        self.set_folder_error(&config.id, None);

        // Store the watcher state
        let state = WatcherState {
            watcher,
            config: config.clone(),
            is_watching: true,
            last_error: None,
            files_processed: 0,
        };

        let mut watchers = self
            .watchers
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;
        watchers.insert(config.id.clone(), state);

        Ok(())
    }

    /// Stop watching a specific folder
    pub fn stop_watching(&self, folder_id: &str) -> Result<()> {
        let mut watchers = self
            .watchers
            .lock()
            .map_err(|e| anyhow!("Lock error: {}", e))?;

        if let Some(state) = watchers.get_mut(folder_id) {
            state.is_watching = false;
            info!("Stopped watching folder: {}", state.config.path);
        }

        // Remove the watcher entirely to stop it
        watchers.remove(folder_id);

        Ok(())
    }

    /// Handle file system events
    fn handle_event(
        app_handle: &AppHandle,
        recent_files: &Arc<Mutex<HashMap<String, Instant>>>,
        debounce_seconds: u64,
        folder_id: &str,
        auto_process: bool,
        event: Event,
    ) {
        // We only care about file creation and modification events
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {}
            _ => return,
        }

        for path in event.paths {
            // Check if it's a file with supported extension
            if !path.is_file() {
                continue;
            }

            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }

            let path_str = path.to_string_lossy().to_string();

            // Check debounce
            {
                let mut recent = match recent_files.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        error!("Failed to lock recent files: {}", e);
                        continue;
                    }
                };

                let now = Instant::now();

                // Clean up old entries
                recent.retain(|_, instant| {
                    now.duration_since(*instant).as_secs() < debounce_seconds * 2
                });

                // Check if we've recently processed this file
                if let Some(last_processed) = recent.get(&path_str) {
                    if now.duration_since(*last_processed).as_secs() < debounce_seconds {
                        debug!("Debouncing file: {}", path_str);
                        continue;
                    }
                }

                // Mark as processed
                recent.insert(path_str.clone(), now);
            }

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            info!(
                "Watch folder detected file: {} in folder {}",
                file_name, folder_id
            );

            // Emit event to frontend
            let payload = WatchFolderFileDetected {
                folder_id: folder_id.to_string(),
                file_path: path_str.clone(),
                file_name: file_name.clone(),
            };

            if let Err(e) = app_handle.emit("watch-folder-file-detected", &payload) {
                error!("Failed to emit watch folder event: {}", e);
            }

            // Increment files processed counter
            if let Some(manager) = app_handle.try_state::<Arc<WatchFolderManager>>() {
                manager.increment_files_processed(folder_id);
            }

            // Auto-process if enabled
            if auto_process {
                if let Some(file_manager) = app_handle.try_state::<Arc<FileTranscriptionManager>>()
                {
                    match file_manager.queue_file(&path_str) {
                        Ok(job) => {
                            info!(
                                "Auto-queued file for transcription: {} (job {})",
                                file_name, job.id
                            );
                        }
                        Err(e) => {
                            warn!("Failed to auto-queue file: {}", e);
                            // Record the error
                            if let Some(manager) = app_handle.try_state::<Arc<WatchFolderManager>>()
                            {
                                manager.set_folder_error(
                                    folder_id,
                                    Some(format!("Failed to queue file: {}", e)),
                                );
                            }
                        }
                    }
                } else {
                    warn!("FileTranscriptionManager not available for auto-processing");
                    if let Some(manager) = app_handle.try_state::<Arc<WatchFolderManager>>() {
                        manager.set_folder_error(
                            folder_id,
                            Some("Transcription manager not available".to_string()),
                        );
                    }
                }
            }
        }
    }

    /// Get the status of all watch folders
    pub fn get_all_status(&self) -> Vec<WatchFolderStatus> {
        let watchers = match self.watchers.lock() {
            Ok(guard) => guard,
            Err(e) => {
                error!("Failed to lock watchers: {}", e);
                return vec![];
            }
        };

        watchers
            .values()
            .map(|state| WatchFolderStatus {
                folder_id: state.config.id.clone(),
                is_watching: state.is_watching,
                last_error: state.last_error.clone(),
                files_processed: state.files_processed,
            })
            .collect()
    }

    /// Stop all watchers and clean up resources
    /// Should be called on app shutdown
    pub fn shutdown(&self) {
        info!("Shutting down watch folder manager...");
        let mut watchers = match self.watchers.lock() {
            Ok(guard) => guard,
            Err(e) => {
                error!("Failed to lock watchers during shutdown: {}", e);
                return;
            }
        };

        let folder_ids: Vec<String> = watchers.keys().cloned().collect();
        for folder_id in folder_ids {
            if let Some(state) = watchers.get(&folder_id) {
                info!("Stopping watcher for folder: {}", state.config.path);
            }
            watchers.remove(&folder_id);
        }

        // Clear recent files tracking
        if let Ok(mut recent) = self.recent_files.lock() {
            recent.clear();
        }

        info!("Watch folder manager shutdown complete");
    }

    /// Record an error for a specific folder
    pub fn set_folder_error(&self, folder_id: &str, error: Option<String>) {
        if let Ok(mut watchers) = self.watchers.lock() {
            if let Some(state) = watchers.get_mut(folder_id) {
                state.last_error = error;
            }
        }
    }

    /// Increment the files processed counter for a folder
    pub fn increment_files_processed(&self, folder_id: &str) {
        if let Ok(mut watchers) = self.watchers.lock() {
            if let Some(state) = watchers.get_mut(folder_id) {
                state.files_processed += 1;
            }
        }
    }
}

/// Get watch folders from settings
pub fn get_watch_folders(app_handle: &AppHandle) -> Vec<WatchFolderConfig> {
    let settings = get_settings(app_handle);
    settings.watch_folders.unwrap_or_default()
}

/// Add a new watch folder
pub fn add_watch_folder(
    app_handle: &AppHandle,
    path: String,
    recursive: bool,
) -> Result<WatchFolderConfig> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(anyhow!("Folder does not exist: {}", path));
    }

    if !path_buf.is_dir() {
        return Err(anyhow!("Path is not a directory: {}", path));
    }

    let mut settings = get_settings(app_handle);
    let mut folders = settings.watch_folders.unwrap_or_default();

    // Check for duplicates
    if folders.iter().any(|f| f.path == path) {
        return Err(anyhow!("Folder is already being watched: {}", path));
    }

    let config = WatchFolderConfig {
        id: uuid::Uuid::new_v4().to_string(),
        path,
        enabled: true,
        recursive,
        auto_process: true,
    };

    folders.push(config.clone());
    settings.watch_folders = Some(folders);
    write_settings(app_handle, settings);

    Ok(config)
}

/// Remove a watch folder
pub fn remove_watch_folder(app_handle: &AppHandle, folder_id: &str) -> Result<()> {
    let mut settings = get_settings(app_handle);
    let mut folders = settings.watch_folders.unwrap_or_default();

    let original_len = folders.len();
    folders.retain(|f| f.id != folder_id);

    if folders.len() == original_len {
        return Err(anyhow!("Watch folder not found: {}", folder_id));
    }

    settings.watch_folders = Some(folders);
    write_settings(app_handle, settings);

    Ok(())
}

/// Update a watch folder configuration
pub fn update_watch_folder(app_handle: &AppHandle, config: WatchFolderConfig) -> Result<()> {
    let mut settings = get_settings(app_handle);
    let mut folders = settings.watch_folders.unwrap_or_default();

    let folder = folders
        .iter_mut()
        .find(|f| f.id == config.id)
        .ok_or_else(|| anyhow!("Watch folder not found: {}", config.id))?;

    *folder = config;

    settings.watch_folders = Some(folders);
    write_settings(app_handle, settings);

    Ok(())
}
