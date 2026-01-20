//! Window context reading for improved transcription accuracy.
//!
//! This module provides optional functionality to read surrounding text
//! from the active window to improve transcription accuracy for names and terms.
//!
//! IMPORTANT: This feature is privacy-sensitive and should be:
//! - Disabled by default
//! - Require explicit user opt-in
//! - Process data locally only (never send to cloud)
//! - Provide clear user notification when active

use serde::{Deserialize, Serialize};

/// Context information from the active window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowContext {
    /// Name of the active application
    pub app_name: String,
    /// Currently selected text (if any)
    pub selected_text: Option<String>,
    /// Text near the cursor/selection (surrounding paragraph)
    pub nearby_text: Option<String>,
}

/// Gets context from the active window.
///
/// This function attempts to read the selected text and surrounding context
/// from the currently active window using platform-specific accessibility APIs.
///
/// Returns None if:
/// - Context reading is not available on the platform
/// - The user hasn't granted accessibility permissions
/// - No relevant context could be obtained
#[cfg(target_os = "macos")]
pub fn get_window_context() -> Option<WindowContext> {
    use std::process::Command;

    // Get active app name using AppleScript
    let app_name = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
        ])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })?;

    // Try to get selected text using AppleScript
    // This requires accessibility permissions
    let selected_text = Command::new("osascript")
        .args([
            "-e",
            r#"
            tell application "System Events"
                set frontApp to first application process whose frontmost is true
                set appName to name of frontApp

                try
                    tell frontApp
                        set selectedText to value of attribute "AXSelectedText" of (first UI element whose value of attribute "AXFocused" is true)
                        if selectedText is not "" then
                            return selectedText
                        end if
                    end tell
                end try
            end tell
            return ""
            "#,
        ])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let text = String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                text
            } else {
                None
            }
        });

    Some(WindowContext {
        app_name,
        selected_text,
        nearby_text: None, // Full nearby text reading requires more complex accessibility API usage
    })
}

#[cfg(target_os = "windows")]
pub fn get_window_context() -> Option<WindowContext> {
    use std::process::Command;

    // Get active window info using PowerShell
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"
            Add-Type @"
                using System;
                using System.Runtime.InteropServices;
                using System.Text;
                public class WindowInfo {
                    [DllImport("user32.dll")]
                    public static extern IntPtr GetForegroundWindow();

                    [DllImport("user32.dll")]
                    public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

                    [DllImport("user32.dll")]
                    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

                    public static string GetActiveWindowInfo() {
                        IntPtr hwnd = GetForegroundWindow();
                        StringBuilder title = new StringBuilder(256);
                        GetWindowText(hwnd, title, 256);

                        uint processId;
                        GetWindowThreadProcessId(hwnd, out processId);

                        try {
                            var process = System.Diagnostics.Process.GetProcessById((int)processId);
                            return process.ProcessName;
                        } catch {
                            return title.ToString();
                        }
                    }
                }
"@
            [WindowInfo]::GetActiveWindowInfo()
            "#,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let app_name = String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())?;

    if app_name.is_empty() {
        return None;
    }

    // Note: Getting selected text on Windows requires UI Automation API
    // which is more complex to implement. For now, we return None for selected_text.
    Some(WindowContext {
        app_name,
        selected_text: None,
        nearby_text: None,
    })
}

#[cfg(target_os = "linux")]
pub fn get_window_context() -> Option<WindowContext> {
    use std::process::Command;

    // Try using xdotool to get active window info (X11)
    let app_name = Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // If xdotool fails, try wmctrl
    let app_name = app_name.or_else(|| {
        Command::new("wmctrl")
            .args(["-l"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    // wmctrl -l output format: <window-id> <desktop> <host> <title>
                    // Get the last entry (usually the active window)
                    String::from_utf8(o.stdout).ok().and_then(|s| {
                        s.lines()
                            .last()
                            .and_then(|line| line.split_whitespace().skip(3).next())
                            .map(|s| s.to_string())
                    })
                } else {
                    None
                }
            })
    })?;

    Some(WindowContext {
        app_name,
        selected_text: None, // Getting selected text on Linux requires xclip or similar
        nearby_text: None,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn get_window_context() -> Option<WindowContext> {
    None
}

/// Extracts relevant context terms from window context.
///
/// This function processes the window context to extract names, terms,
/// and other relevant words that might appear in the transcription.
pub fn extract_context_terms(context: &WindowContext) -> Vec<String> {
    let mut terms = Vec::new();

    // Extract terms from selected text
    if let Some(ref selected) = context.selected_text {
        // Extract capitalized words (likely names or proper nouns)
        for word in selected.split_whitespace() {
            let cleaned: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
            if cleaned.len() >= 2
                && cleaned
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
            {
                terms.push(cleaned);
            }
        }
    }

    // Extract terms from nearby text
    if let Some(ref nearby) = context.nearby_text {
        for word in nearby.split_whitespace() {
            let cleaned: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
            if cleaned.len() >= 2
                && cleaned
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
            {
                if !terms.contains(&cleaned) {
                    terms.push(cleaned);
                }
            }
        }
    }

    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_context_terms() {
        let context = WindowContext {
            app_name: "Test App".to_string(),
            selected_text: Some("Meeting with John Smith about the API project".to_string()),
            nearby_text: Some("Sarah mentioned the DatabaseManager class".to_string()),
        };

        let terms = extract_context_terms(&context);
        assert!(terms.contains(&"John".to_string()));
        assert!(terms.contains(&"Smith".to_string()));
        assert!(terms.contains(&"API".to_string()));
        assert!(terms.contains(&"Sarah".to_string()));
        assert!(terms.contains(&"DatabaseManager".to_string()));
    }

    #[test]
    fn test_extract_context_terms_empty() {
        let context = WindowContext {
            app_name: "Test".to_string(),
            selected_text: None,
            nearby_text: None,
        };

        let terms = extract_context_terms(&context);
        assert!(terms.is_empty());
    }
}
