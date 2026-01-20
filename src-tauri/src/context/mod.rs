//! Context-awareness features for improved transcription.
//!
//! This module provides functionality to understand the user's context:
//! - Active application detection
//! - Developer mode detection
//! - Window context reading (optional, privacy-sensitive)

pub mod active_app;
pub mod window_context;

pub use active_app::{
    categorize_app, get_active_app, is_developer_context, ActiveAppInfo, AppCategory, ToneStyle,
};
pub use window_context::{extract_context_terms, get_window_context, WindowContext};
