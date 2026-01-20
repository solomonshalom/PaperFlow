//! Active application detection for context-aware features.
//!
//! This module provides functions to detect the currently active (frontmost) application
//! on the user's system. This is used for:
//! - Context-aware tone adjustment (formal for email, casual for messaging)
//! - Developer mode auto-detection (VS Code, terminals, etc.)

use serde::{Deserialize, Serialize};
use specta::Type;

/// Information about the currently active application
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActiveAppInfo {
    /// The application name (e.g., "Visual Studio Code", "Slack")
    pub name: String,
    /// The bundle identifier on macOS (e.g., "com.microsoft.VSCode")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
}

/// Application categories for tone and behavior adjustments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppCategory {
    /// Email clients (Mail, Outlook, Gmail) → Formal tone
    Email,
    /// Messaging apps (Slack, Discord, Messages) → Casual tone
    Messaging,
    /// Document editors (Word, Docs, Notion) → Formal tone
    Documents,
    /// IDEs and code editors → Technical/Developer mode
    Ide,
    /// Terminal and shell applications
    Terminal,
    /// Web browsers → Context-dependent
    Browser,
    /// Other applications → Neutral
    Other,
}

impl AppCategory {
    /// Returns the default tone style for this app category
    pub fn default_tone(&self) -> ToneStyle {
        match self {
            AppCategory::Email => ToneStyle::Formal,
            AppCategory::Messaging => ToneStyle::Casual,
            AppCategory::Documents => ToneStyle::Formal,
            AppCategory::Ide => ToneStyle::Technical,
            AppCategory::Terminal => ToneStyle::Technical,
            AppCategory::Browser => ToneStyle::Neutral,
            AppCategory::Other => ToneStyle::Neutral,
        }
    }
}

/// Tone styles for transcription adjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToneStyle {
    /// Professional, complete sentences (email, documents)
    Formal,
    /// Conversational, contractions allowed (messaging)
    Casual,
    /// Precise, jargon-friendly (IDEs, terminals)
    Technical,
    /// Minimal changes, faithful transcription
    #[default]
    Neutral,
}

// === App Name Lists ===

/// Known email applications
const EMAIL_APPS: &[&str] = &[
    "mail",
    "outlook",
    "gmail",
    "thunderbird",
    "spark",
    "airmail",
    "postbox",
    "mailspring",
    "canary mail",
    "newton mail",
    "superhuman",
    "hey",
    "proton mail",
    "tutanota",
    "fastmail",
];

/// Known messaging applications
const MESSAGING_APPS: &[&str] = &[
    "slack",
    "discord",
    "messages",
    "imessage",
    "telegram",
    "whatsapp",
    "signal",
    "messenger",
    "teams",
    "microsoft teams",
    "zoom",
    "skype",
    "webex",
    "element",
    "mattermost",
    "wire",
    "keybase",
    "wickr",
    "threema",
];

/// Known document editing applications
const DOCUMENT_APPS: &[&str] = &[
    "word",
    "microsoft word",
    "pages",
    "google docs",
    "notion",
    "obsidian",
    "roam",
    "bear",
    "ulysses",
    "ia writer",
    "scrivener",
    "libreoffice writer",
    "openoffice writer",
    "typora",
    "craft",
    "drafts",
    "coda",
    "dropbox paper",
    "quip",
    "confluence",
];

/// Known IDE and code editor applications
const IDE_APPS: &[&str] = &[
    "code",
    "visual studio code",
    "vscode",
    "cursor",
    "intellij idea",
    "intellij",
    "pycharm",
    "webstorm",
    "phpstorm",
    "rider",
    "rubymine",
    "goland",
    "clion",
    "datagrip",
    "android studio",
    "xcode",
    "visual studio",
    "sublime text",
    "atom",
    "vim",
    "neovim",
    "nvim",
    "emacs",
    "nova",
    "bbedit",
    "textmate",
    "fleet",
    "zed",
    "lapce",
    "helix",
    "eclipse",
    "netbeans",
    "brackets",
    "coderunner",
    "codeedit",
];

/// Known terminal applications
const TERMINAL_APPS: &[&str] = &[
    "terminal",
    "iterm",
    "iterm2",
    "hyper",
    "warp",
    "alacritty",
    "kitty",
    "wezterm",
    "windows terminal",
    "powershell",
    "cmd",
    "command prompt",
    "gnome-terminal",
    "konsole",
    "terminator",
    "tilix",
    "xterm",
    "urxvt",
    "st",
    "tabby",
    "terminus",
    "cmder",
    "conemu",
    "mintty",
    "ghostty",
];

/// Known browser applications
const BROWSER_APPS: &[&str] = &[
    "safari",
    "chrome",
    "google chrome",
    "firefox",
    "brave",
    "edge",
    "microsoft edge",
    "opera",
    "vivaldi",
    "arc",
    "zen browser",
    "orion",
    "duckduckgo",
    "tor browser",
    "chromium",
];

/// Categorizes an application based on its name
pub fn categorize_app(app_name: &str) -> AppCategory {
    let name_lower = app_name.to_lowercase();

    // Check each category
    if IDE_APPS.iter().any(|&app| name_lower.contains(app)) {
        return AppCategory::Ide;
    }

    if TERMINAL_APPS.iter().any(|&app| name_lower.contains(app)) {
        return AppCategory::Terminal;
    }

    if EMAIL_APPS.iter().any(|&app| name_lower.contains(app)) {
        return AppCategory::Email;
    }

    if MESSAGING_APPS.iter().any(|&app| name_lower.contains(app)) {
        return AppCategory::Messaging;
    }

    if DOCUMENT_APPS.iter().any(|&app| name_lower.contains(app)) {
        return AppCategory::Documents;
    }

    if BROWSER_APPS.iter().any(|&app| name_lower.contains(app)) {
        return AppCategory::Browser;
    }

    AppCategory::Other
}

/// Returns true if the app is an IDE or terminal (developer context)
pub fn is_developer_context(app_name: &str) -> bool {
    let category = categorize_app(app_name);
    matches!(category, AppCategory::Ide | AppCategory::Terminal)
}

// === Platform-specific implementations ===

/// Gets information about the currently active (frontmost) application.
/// Returns None if the active app cannot be determined.
#[cfg(target_os = "macos")]
pub fn get_active_app() -> Option<ActiveAppInfo> {
    use std::process::Command;

    // Use AppleScript to get the frontmost application
    let script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;

    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if name.is_empty() {
        return None;
    }

    // Try to get bundle identifier as well
    let bundle_script = format!(
        r#"tell application "System Events" to get bundle identifier of first application process whose name is "{}""#,
        name.replace('"', "\\\"")
    );

    let bundle_id = Command::new("osascript")
        .args(["-e", &bundle_script])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !id.is_empty() {
                    Some(id)
                } else {
                    None
                }
            } else {
                None
            }
        });

    Some(ActiveAppInfo { name, bundle_id })
}

#[cfg(target_os = "windows")]
pub fn get_active_app() -> Option<ActiveAppInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0 == 0 {
            return None;
        }

        // Get window title
        let mut buffer = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buffer);
        if len == 0 {
            return None;
        }

        let name = OsString::from_wide(&buffer[..len as usize])
            .to_string_lossy()
            .to_string();

        if name.is_empty() {
            return None;
        }

        Some(ActiveAppInfo {
            name,
            bundle_id: None,
        })
    }
}

#[cfg(target_os = "linux")]
pub fn get_active_app() -> Option<ActiveAppInfo> {
    use std::process::Command;

    // Try xdotool first (X11)
    if let Ok(output) = Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
    {
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return Some(ActiveAppInfo {
                    name,
                    bundle_id: None,
                });
            }
        }
    }

    // Try wmctrl as fallback
    if let Ok(output) = Command::new("wmctrl").args(["-a", ":ACTIVE:"]).output() {
        if output.status.success() {
            // wmctrl -a doesn't return the name directly, we need to parse wmctrl -l
            if let Ok(list_output) = Command::new("wmctrl").arg("-l").output() {
                if list_output.status.success() {
                    // Parse the first line which is usually the active window
                    let list = String::from_utf8_lossy(&list_output.stdout);
                    if let Some(line) = list.lines().next() {
                        // Format: window_id desktop_id client_name window_title
                        let parts: Vec<&str> = line.splitn(4, ' ').collect();
                        if parts.len() >= 4 {
                            return Some(ActiveAppInfo {
                                name: parts[3].to_string(),
                                bundle_id: None,
                            });
                        }
                    }
                }
            }
        }
    }

    None
}

// Fallback for unsupported platforms
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn get_active_app() -> Option<ActiveAppInfo> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_ide_apps() {
        assert_eq!(categorize_app("Visual Studio Code"), AppCategory::Ide);
        assert_eq!(categorize_app("Cursor"), AppCategory::Ide);
        assert_eq!(categorize_app("IntelliJ IDEA"), AppCategory::Ide);
        assert_eq!(categorize_app("Xcode"), AppCategory::Ide);
        assert_eq!(categorize_app("PyCharm"), AppCategory::Ide);
    }

    #[test]
    fn test_categorize_terminal_apps() {
        assert_eq!(categorize_app("Terminal"), AppCategory::Terminal);
        assert_eq!(categorize_app("iTerm2"), AppCategory::Terminal);
        assert_eq!(categorize_app("Warp"), AppCategory::Terminal);
        assert_eq!(categorize_app("Alacritty"), AppCategory::Terminal);
    }

    #[test]
    fn test_categorize_email_apps() {
        assert_eq!(categorize_app("Mail"), AppCategory::Email);
        assert_eq!(categorize_app("Microsoft Outlook"), AppCategory::Email);
        assert_eq!(categorize_app("Spark"), AppCategory::Email);
    }

    #[test]
    fn test_categorize_messaging_apps() {
        assert_eq!(categorize_app("Slack"), AppCategory::Messaging);
        assert_eq!(categorize_app("Discord"), AppCategory::Messaging);
        assert_eq!(categorize_app("Messages"), AppCategory::Messaging);
        assert_eq!(categorize_app("Microsoft Teams"), AppCategory::Messaging);
    }

    #[test]
    fn test_categorize_document_apps() {
        assert_eq!(categorize_app("Microsoft Word"), AppCategory::Documents);
        assert_eq!(categorize_app("Notion"), AppCategory::Documents);
        assert_eq!(categorize_app("Obsidian"), AppCategory::Documents);
    }

    #[test]
    fn test_categorize_browser_apps() {
        assert_eq!(categorize_app("Safari"), AppCategory::Browser);
        assert_eq!(categorize_app("Google Chrome"), AppCategory::Browser);
        assert_eq!(categorize_app("Firefox"), AppCategory::Browser);
        assert_eq!(categorize_app("Arc"), AppCategory::Browser);
    }

    #[test]
    fn test_categorize_unknown_app() {
        assert_eq!(categorize_app("Some Random App"), AppCategory::Other);
    }

    #[test]
    fn test_is_developer_context() {
        assert!(is_developer_context("Visual Studio Code"));
        assert!(is_developer_context("Terminal"));
        assert!(is_developer_context("iTerm2"));
        assert!(!is_developer_context("Safari"));
        assert!(!is_developer_context("Mail"));
    }

    #[test]
    fn test_default_tone() {
        assert_eq!(AppCategory::Email.default_tone(), ToneStyle::Formal);
        assert_eq!(AppCategory::Messaging.default_tone(), ToneStyle::Casual);
        assert_eq!(AppCategory::Ide.default_tone(), ToneStyle::Technical);
        assert_eq!(AppCategory::Other.default_tone(), ToneStyle::Neutral);
    }
}
