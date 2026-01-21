use crate::audio_feedback;
use crate::audio_toolkit::audio::{list_input_devices, list_output_devices};
use crate::managers::audio::{AudioRecordingManager, MicrophoneMode};
use crate::managers::system_audio::SystemAudioManager;
use crate::settings::{get_settings, write_settings};
use log::warn;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Type)]
pub struct CustomSounds {
    start: bool,
    stop: bool,
}

/// Device type classification for audio devices
#[derive(Serialize, Deserialize, Debug, Clone, Type, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AudioDeviceType {
    Microphone,
    SystemLoopback,
    VirtualDevice,
    Unknown,
}

/// Detect the type of audio device based on its name
fn detect_device_type(name: &str) -> AudioDeviceType {
    let lower = name.to_lowercase();

    // macOS: BlackHole, Soundflower, Loopback (by Rogue Amoeba)
    // Windows: Stereo Mix, What U Hear, WASAPI Loopback
    // Linux: .monitor, "Monitor of X"
    if lower.contains("blackhole")
        || lower.contains("soundflower")
        || lower.contains("loopback")
        || lower.contains("stereo mix")
        || lower.contains("what u hear")
        || lower.contains("wasapi")
        || lower.contains(".monitor")
        || lower.contains("monitor of")
    {
        return AudioDeviceType::SystemLoopback;
    }

    // Virtual devices (general)
    if lower.contains("virtual")
        || lower.contains("vb-audio")
        || lower.contains("voicemeeter")
        || lower.contains("cable")
    {
        return AudioDeviceType::VirtualDevice;
    }

    // Default to microphone for unrecognized input devices
    AudioDeviceType::Microphone
}

/// Information about system audio capture capabilities
#[derive(Serialize, Type)]
pub struct SystemAudioInfo {
    /// Whether system audio capture is currently available (via virtual devices or native)
    pub available: bool,
    /// Whether native system audio capture is available (no drivers needed)
    pub native_available: bool,
    /// Whether additional setup is required
    pub requires_setup: bool,
    /// Platform-specific setup instructions
    pub setup_instructions: String,
    /// List of detected system audio devices (virtual drivers)
    pub devices: Vec<String>,
    /// Platform-specific info about native capture
    pub native_info: String,
}

/// Get information about system audio capture for the current platform
#[tauri::command]
#[specta::specta]
pub fn get_system_audio_info(app: AppHandle) -> SystemAudioInfo {
    let devices = list_input_devices()
        .unwrap_or_default()
        .into_iter()
        .filter(|d| {
            let device_type = detect_device_type(&d.name);
            device_type == AudioDeviceType::SystemLoopback
                || device_type == AudioDeviceType::VirtualDevice
        })
        .map(|d| d.name)
        .collect::<Vec<_>>();

    let virtual_available = !devices.is_empty();

    // Check native capture availability
    let (native_available, native_info) =
        if let Some(sam) = app.try_state::<Arc<SystemAudioManager>>() {
            let is_available = sam.is_available();
            let info = sam.get_platform_info();
            (is_available, info)
        } else {
            (false, "Native capture not initialized".to_string())
        };

    // Available if either native or virtual devices are present
    let available = native_available || virtual_available;

    #[cfg(target_os = "macos")]
    let (requires_setup, setup_instructions) = if native_available {
        // Native capture available on macOS 14.2+
        (false, String::new())
    } else if virtual_available {
        (false, String::new())
    } else {
        (
            true,
            "Native system audio capture requires macOS 14.2 or later.\n\n\
             For older macOS versions, install a virtual audio driver:\n\n\
             1. Download BlackHole (free, open-source): https://existential.audio/blackhole/\n\
             2. Install and restart your computer\n\
             3. Create a Multi-Output Device in Audio MIDI Setup\n\
             4. Select the new device here to record system audio"
                .to_string(),
        )
    };

    #[cfg(target_os = "windows")]
    let (requires_setup, setup_instructions) = if native_available {
        // Native WASAPI loopback available
        (false, String::new())
    } else if virtual_available {
        (false, String::new())
    } else {
        (
            true,
            "Native system audio capture via WASAPI loopback is available.\n\n\
             Alternatively, to use virtual devices:\n\n\
             1. Open Sound Settings\n\
             2. Go to Recording tab\n\
             3. Right-click and enable 'Show Disabled Devices'\n\
             4. Enable 'Stereo Mix' if available\n\
             5. If not available, install VB-Audio Virtual Cable"
                .to_string(),
        )
    };

    #[cfg(target_os = "linux")]
    let (requires_setup, setup_instructions) = if virtual_available {
        (false, String::new())
    } else {
        (
            true,
            "To capture system audio on Linux:\n\n\
             With PipeWire: Monitor devices should appear automatically\n\
             With PulseAudio: Use 'pactl load-module module-loopback'\n\n\
             Look for devices ending in '.monitor' or 'Monitor of'"
                .to_string(),
        )
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let (requires_setup, setup_instructions) = (
        true,
        "System audio capture may not be available on this platform.".to_string(),
    );

    SystemAudioInfo {
        available,
        native_available,
        requires_setup,
        setup_instructions,
        devices,
        native_info,
    }
}

/// Check if native system audio capture is available
#[tauri::command]
#[specta::specta]
pub fn is_native_system_audio_available(app: AppHandle) -> bool {
    app.try_state::<Arc<SystemAudioManager>>()
        .map(|sam| sam.is_available())
        .unwrap_or(false)
}

/// Start native system audio capture
#[tauri::command]
#[specta::specta]
pub async fn start_system_audio_capture(app: AppHandle) -> Result<(), String> {
    let sam = app
        .try_state::<Arc<SystemAudioManager>>()
        .ok_or_else(|| "System audio manager not initialized".to_string())?;

    sam.start_capture()
        .map_err(|e| format!("Failed to start system audio capture: {}", e))
}

/// Stop native system audio capture and return samples
#[tauri::command]
#[specta::specta]
pub async fn stop_system_audio_capture(app: AppHandle) -> Result<Vec<f32>, String> {
    let sam = app
        .try_state::<Arc<SystemAudioManager>>()
        .ok_or_else(|| "System audio manager not initialized".to_string())?;

    sam.stop_capture()
        .map_err(|e| format!("Failed to stop system audio capture: {}", e))
}

/// Check if currently capturing system audio
#[tauri::command]
#[specta::specta]
pub fn is_capturing_system_audio(app: AppHandle) -> bool {
    app.try_state::<Arc<SystemAudioManager>>()
        .map(|sam| sam.is_capturing())
        .unwrap_or(false)
}

fn custom_sound_exists(app: &AppHandle, sound_type: &str) -> bool {
    app.path()
        .resolve(
            format!("custom_{}.wav", sound_type),
            tauri::path::BaseDirectory::AppData,
        )
        .map_or(false, |path| path.exists())
}

#[tauri::command]
#[specta::specta]
pub fn check_custom_sounds(app: AppHandle) -> CustomSounds {
    CustomSounds {
        start: custom_sound_exists(&app, "start"),
        stop: custom_sound_exists(&app, "stop"),
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AudioDevice {
    pub index: String,
    pub name: String,
    pub is_default: bool,
    pub device_type: AudioDeviceType,
}

#[tauri::command]
#[specta::specta]
pub fn update_microphone_mode(app: AppHandle, always_on: bool) -> Result<(), String> {
    // Update settings
    let mut settings = get_settings(&app);
    settings.always_on_microphone = always_on;
    write_settings(&app, settings);

    // Update the audio manager mode
    let rm = app.state::<Arc<AudioRecordingManager>>();
    let new_mode = if always_on {
        MicrophoneMode::AlwaysOn
    } else {
        MicrophoneMode::OnDemand
    };

    rm.update_mode(new_mode)
        .map_err(|e| format!("Failed to update microphone mode: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn get_microphone_mode(app: AppHandle) -> Result<bool, String> {
    let settings = get_settings(&app);
    Ok(settings.always_on_microphone)
}

#[tauri::command]
#[specta::specta]
pub fn get_available_microphones() -> Result<Vec<AudioDevice>, String> {
    let devices =
        list_input_devices().map_err(|e| format!("Failed to list audio devices: {}", e))?;

    let mut result = vec![AudioDevice {
        index: "default".to_string(),
        name: "Default".to_string(),
        is_default: true,
        device_type: AudioDeviceType::Microphone,
    }];

    result.extend(devices.into_iter().map(|d| {
        let device_type = detect_device_type(&d.name);
        AudioDevice {
            index: d.index,
            name: d.name,
            is_default: false, // The explicit default is handled separately
            device_type,
        }
    }));

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn set_selected_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_microphone = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    write_settings(&app, settings);

    // Update the audio manager to use the new device
    let rm = app.state::<Arc<AudioRecordingManager>>();
    rm.update_selected_device()
        .map_err(|e| format!("Failed to update selected device: {}", e))?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_selected_microphone(app: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app);
    Ok(settings
        .selected_microphone
        .unwrap_or_else(|| "default".to_string()))
}

#[tauri::command]
#[specta::specta]
pub fn get_available_output_devices() -> Result<Vec<AudioDevice>, String> {
    let devices =
        list_output_devices().map_err(|e| format!("Failed to list output devices: {}", e))?;

    let mut result = vec![AudioDevice {
        index: "default".to_string(),
        name: "Default".to_string(),
        is_default: true,
        device_type: AudioDeviceType::Unknown,
    }];

    result.extend(devices.into_iter().map(|d| AudioDevice {
        index: d.index,
        name: d.name,
        is_default: false, // The explicit default is handled separately
        device_type: AudioDeviceType::Unknown,
    }));

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub fn set_selected_output_device(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.selected_output_device = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_selected_output_device(app: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app);
    Ok(settings
        .selected_output_device
        .unwrap_or_else(|| "default".to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn play_test_sound(app: AppHandle, sound_type: String) {
    let sound = match sound_type.as_str() {
        "start" => audio_feedback::SoundType::Start,
        "stop" => audio_feedback::SoundType::Stop,
        _ => {
            warn!("Unknown sound type: {}", sound_type);
            return;
        }
    };
    audio_feedback::play_test_sound(&app, sound);
}

#[tauri::command]
#[specta::specta]
pub fn set_clamshell_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    settings.clamshell_microphone = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    write_settings(&app, settings);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_clamshell_microphone(app: AppHandle) -> Result<String, String> {
    let settings = get_settings(&app);
    Ok(settings
        .clamshell_microphone
        .unwrap_or_else(|| "default".to_string()))
}

#[tauri::command]
#[specta::specta]
pub fn is_recording(app: AppHandle) -> bool {
    let audio_manager = app.state::<Arc<AudioRecordingManager>>();
    audio_manager.is_recording()
}
