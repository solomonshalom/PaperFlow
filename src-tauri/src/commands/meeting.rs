//! Meeting mode Tauri commands

use crate::managers::history::HistoryManager;
use crate::managers::meeting::{MeetingManager, MeetingSession, MeetingState};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// A meeting history entry for the frontend
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MeetingHistoryEntry {
    pub id: i64,
    pub meeting_id: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub duration_seconds: i64,
    pub full_transcript: String,
    pub summary: Option<String>,
    pub action_items: Option<Vec<String>>,
    pub chunk_count: u32,
    pub saved: bool,
}

/// Get the current meeting state
#[tauri::command]
#[specta::specta]
pub fn get_meeting_state(app: AppHandle) -> Result<MeetingState, String> {
    let mm = app
        .try_state::<Arc<MeetingManager>>()
        .ok_or("Meeting manager not initialized")?;
    Ok(mm.get_meeting_state())
}

/// Get the current meeting session if any
#[tauri::command]
#[specta::specta]
pub fn get_current_meeting_session(app: AppHandle) -> Result<Option<MeetingSession>, String> {
    let mm = app
        .try_state::<Arc<MeetingManager>>()
        .ok_or("Meeting manager not initialized")?;
    Ok(mm.get_current_session())
}

/// Get elapsed time in seconds since meeting started
#[tauri::command]
#[specta::specta]
pub fn get_meeting_elapsed_seconds(app: AppHandle) -> Result<Option<u64>, String> {
    let mm = app
        .try_state::<Arc<MeetingManager>>()
        .ok_or("Meeting manager not initialized")?;
    Ok(mm.get_elapsed_seconds())
}

/// Start a new meeting
#[tauri::command]
#[specta::specta]
pub fn start_meeting(app: AppHandle, binding_id: String) -> Result<String, String> {
    let mm = app
        .try_state::<Arc<MeetingManager>>()
        .ok_or("Meeting manager not initialized")?;
    mm.start_meeting(&binding_id).map_err(|e| e.to_string())
}

/// Stop the current meeting
#[tauri::command]
#[specta::specta]
pub fn stop_meeting(app: AppHandle, meeting_id: String) -> Result<(), String> {
    let mm = app
        .try_state::<Arc<MeetingManager>>()
        .ok_or("Meeting manager not initialized")?;
    mm.stop_meeting(&meeting_id).map_err(|e| e.to_string())
}

/// Cancel the current meeting without processing
#[tauri::command]
#[specta::specta]
pub fn cancel_meeting(app: AppHandle, meeting_id: String) -> Result<(), String> {
    let mm = app
        .try_state::<Arc<MeetingManager>>()
        .ok_or("Meeting manager not initialized")?;
    mm.cancel_meeting(&meeting_id).map_err(|e| e.to_string())
}

/// Get meeting history entries
#[tauri::command]
#[specta::specta]
pub fn get_meeting_history(app: AppHandle) -> Result<Vec<MeetingHistoryEntry>, String> {
    let hm = app
        .try_state::<Arc<HistoryManager>>()
        .ok_or("History manager not initialized")?;
    hm.get_meeting_entries().map_err(|e| e.to_string())
}

/// Delete a meeting from history
#[tauri::command]
#[specta::specta]
pub fn delete_meeting(app: AppHandle, meeting_id: String) -> Result<(), String> {
    let hm = app
        .try_state::<Arc<HistoryManager>>()
        .ok_or("History manager not initialized")?;
    hm.delete_meeting(&meeting_id).map_err(|e| e.to_string())
}
