mod actions;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod apple_intelligence;
mod audio_feedback;
pub mod audio_toolkit;
mod clipboard;
mod commands;
mod context;
mod groq_transcription;
mod helpers;
mod input;
mod llm_client;
mod managers;
mod overlay;
mod settings;
mod shortcut;
mod signal_handle;
mod tray;
mod tray_i18n;
mod utils;
use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri_specta::{collect_commands, Builder};

use env_filter::Builder as EnvFilterBuilder;
use managers::audio::AudioRecordingManager;
use managers::diarization::DiarizationManager;
use managers::file_transcription::FileTranscriptionManager;
use managers::history::HistoryManager;
use managers::live_preview::LivePreviewManager;
use managers::meeting::MeetingManager;
use managers::model::ModelManager;
use managers::system_audio::SystemAudioManager;
use managers::transcription::TranscriptionManager;
use managers::watch_folder::WatchFolderManager;
#[cfg(unix)]
use signal_hook::consts::SIGUSR2;
#[cfg(unix)]
use signal_hook::iterator::Signals;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use tauri::image::Image;

use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_log::{Builder as LogBuilder, RotationStrategy, Target, TargetKind};

use crate::settings::get_settings;

// Global atomic to store the file log level filter
// We use u8 to store the log::LevelFilter as a number
pub static FILE_LOG_LEVEL: AtomicU8 = AtomicU8::new(log::LevelFilter::Debug as u8);

fn level_filter_from_u8(value: u8) -> log::LevelFilter {
    match value {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Trace,
    }
}

fn build_console_filter() -> env_filter::Filter {
    let mut builder = EnvFilterBuilder::new();

    match std::env::var("RUST_LOG") {
        Ok(spec) if !spec.trim().is_empty() => {
            if let Err(err) = builder.try_parse(&spec) {
                log::warn!(
                    "Ignoring invalid RUST_LOG value '{}': {}. Falling back to info-level console logging",
                    spec,
                    err
                );
                builder.filter_level(log::LevelFilter::Info);
            }
        }
        _ => {
            builder.filter_level(log::LevelFilter::Info);
        }
    }

    builder.build()
}

#[derive(Default)]
struct ShortcutToggleStates {
    // Map: shortcut_binding_id -> is_active
    active_toggles: HashMap<String, bool>,
}

type ManagedToggleState = Mutex<ShortcutToggleStates>;

fn show_main_window(app: &AppHandle) {
    if let Some(main_window) = app.get_webview_window("main") {
        // First, ensure the window is visible
        if let Err(e) = main_window.show() {
            log::error!("Failed to show window: {}", e);
        }
        // Then, bring it to the front and give it focus
        if let Err(e) = main_window.set_focus() {
            log::error!("Failed to focus window: {}", e);
        }
        // Optional: On macOS, ensure the app becomes active if it was an accessory
        #[cfg(target_os = "macos")]
        {
            if let Err(e) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
                log::error!("Failed to set activation policy to Regular: {}", e);
            }
        }
    } else {
        log::error!("Main window not found.");
    }
}

fn initialize_core_logic(app_handle: &AppHandle) {
    // Note: Enigo (keyboard/mouse simulation) is NOT initialized here.
    // The frontend is responsible for calling the `initialize_enigo` command
    // after onboarding completes. This avoids triggering permission dialogs
    // on macOS before the user is ready.

    // Initialize the managers
    let recording_manager = Arc::new(
        AudioRecordingManager::new(app_handle).expect("Failed to initialize recording manager"),
    );
    let model_manager =
        Arc::new(ModelManager::new(app_handle).expect("Failed to initialize model manager"));
    let transcription_manager = Arc::new(
        TranscriptionManager::new(app_handle, model_manager.clone())
            .expect("Failed to initialize transcription manager"),
    );
    let history_manager =
        Arc::new(HistoryManager::new(app_handle).expect("Failed to initialize history manager"));
    let meeting_manager =
        Arc::new(MeetingManager::new(app_handle).expect("Failed to initialize meeting manager"));
    let live_preview_manager = Arc::new(LivePreviewManager::new(
        app_handle,
        transcription_manager.clone(),
    ));
    let file_transcription_manager = Arc::new(
        FileTranscriptionManager::new(app_handle, transcription_manager.clone())
            .expect("Failed to initialize file transcription manager"),
    );
    let watch_folder_manager = Arc::new(
        WatchFolderManager::new(app_handle).expect("Failed to initialize watch folder manager"),
    );
    let diarization_manager = Arc::new(
        DiarizationManager::new(app_handle).expect("Failed to initialize diarization manager"),
    );
    let system_audio_manager = Arc::new(
        SystemAudioManager::new(app_handle).expect("Failed to initialize system audio manager"),
    );

    // Wire up the live preview manager to the recording manager
    recording_manager.set_live_preview_manager(live_preview_manager.clone());

    // Add managers to Tauri's managed state
    app_handle.manage(recording_manager.clone());
    app_handle.manage(model_manager.clone());
    app_handle.manage(transcription_manager.clone());
    app_handle.manage(history_manager.clone());
    app_handle.manage(meeting_manager.clone());
    app_handle.manage(live_preview_manager.clone());
    app_handle.manage(file_transcription_manager.clone());
    app_handle.manage(watch_folder_manager.clone());
    app_handle.manage(diarization_manager.clone());
    app_handle.manage(system_audio_manager.clone());

    // Start watching all enabled folders
    if let Err(e) = watch_folder_manager.start_all() {
        log::error!("Failed to start watch folders: {}", e);
    }

    // Initialize the shortcuts
    shortcut::init_shortcuts(app_handle);

    #[cfg(unix)]
    let signals = Signals::new(&[SIGUSR2]).unwrap();
    // Set up SIGUSR2 signal handler for toggling transcription
    #[cfg(unix)]
    signal_handle::setup_signal_handler(app_handle.clone(), signals);

    // Apply macOS Accessory policy if starting hidden
    #[cfg(target_os = "macos")]
    {
        let settings = settings::get_settings(app_handle);
        if settings.start_hidden {
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }
    // Get the current theme to set the appropriate initial icon
    let initial_theme = tray::get_current_theme(app_handle);

    // Choose the appropriate initial icon based on theme
    let initial_icon_path = tray::get_icon_path(initial_theme, tray::TrayIconState::Idle);

    let icon_path = app_handle
        .path()
        .resolve(initial_icon_path, tauri::path::BaseDirectory::Resource)
        .expect("Failed to resolve tray icon path - ensure resources are bundled correctly");

    let icon_image = Image::from_path(&icon_path)
        .expect("Failed to load tray icon image - ensure icon file exists and is valid");

    let tray = TrayIconBuilder::new()
        .icon(icon_image)
        .show_menu_on_left_click(true)
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                show_main_window(app);
            }
            "check_updates" => {
                let settings = settings::get_settings(app);
                if settings.update_checks_enabled {
                    show_main_window(app);
                    let _ = app.emit("check-for-updates", ());
                }
            }
            "copy_last_transcript" => {
                tray::copy_last_transcript(app);
            }
            "cancel" => {
                use crate::utils::cancel_current_operation;

                // Use centralized cancellation that handles all operations
                cancel_current_operation(app);
            }
            "quit" => {
                // Clean up watch folder manager before exit
                if let Some(watch_manager) = app.try_state::<Arc<WatchFolderManager>>() {
                    watch_manager.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        })
        .build(app_handle)
        .expect("Failed to build tray icon - check system tray availability");
    app_handle.manage(tray);

    // Initialize tray menu with idle state
    utils::update_tray_menu(app_handle, &utils::TrayIconState::Idle, None);

    // Get the autostart manager and configure based on user setting
    let autostart_manager = app_handle.autolaunch();
    let settings = settings::get_settings(&app_handle);

    if settings.autostart_enabled {
        // Enable autostart if user has opted in
        let _ = autostart_manager.enable();
    } else {
        // Disable autostart if user has opted out
        let _ = autostart_manager.disable();
    }

    // Create the recording overlay window (hidden by default)
    utils::create_recording_overlay(app_handle);
}

#[tauri::command]
#[specta::specta]
fn trigger_update_check(app: AppHandle) -> Result<(), String> {
    let settings = settings::get_settings(&app);
    if !settings.update_checks_enabled {
        return Ok(());
    }
    app.emit("check-for-updates", ())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Parse console logging directives from RUST_LOG, falling back to info-level logging
    // when the variable is unset
    let console_filter = build_console_filter();

    let specta_builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        shortcut::change_binding,
        shortcut::reset_binding,
        shortcut::change_ptt_setting,
        shortcut::change_audio_feedback_setting,
        shortcut::change_audio_feedback_volume_setting,
        shortcut::change_sound_theme_setting,
        shortcut::change_start_hidden_setting,
        shortcut::change_autostart_setting,
        shortcut::change_translate_to_english_setting,
        shortcut::change_selected_language_setting,
        shortcut::change_overlay_position_setting,
        shortcut::change_debug_mode_setting,
        shortcut::change_word_correction_threshold_setting,
        shortcut::change_paste_method_setting,
        shortcut::change_clipboard_handling_setting,
        shortcut::change_post_process_enabled_setting,
        shortcut::change_post_process_base_url_setting,
        shortcut::change_post_process_api_key_setting,
        shortcut::change_post_process_model_setting,
        shortcut::set_post_process_provider,
        shortcut::fetch_post_process_models,
        shortcut::add_post_process_prompt,
        shortcut::update_post_process_prompt,
        shortcut::delete_post_process_prompt,
        shortcut::set_post_process_selected_prompt,
        shortcut::update_custom_words,
        shortcut::suspend_binding,
        shortcut::resume_binding,
        shortcut::change_mute_while_recording_setting,
        shortcut::change_append_trailing_space_setting,
        shortcut::change_app_language_setting,
        shortcut::change_update_checks_setting,
        shortcut::change_snippets_enabled_setting,
        shortcut::update_snippets,
        shortcut::change_auto_format_enabled_setting,
        shortcut::change_auto_format_lists_setting,
        shortcut::change_verbal_commands_enabled_setting,
        shortcut::change_tone_adjustment_enabled_setting,
        shortcut::change_default_tone_setting,
        shortcut::update_app_tone_mappings,
        shortcut::change_developer_mode_setting,
        shortcut::change_preserve_code_syntax_setting,
        shortcut::update_developer_dictionary,
        shortcut::change_correction_detection_enabled_setting,
        shortcut::change_context_awareness_enabled_setting,
        shortcut::update_context_per_app_permissions,
        shortcut::change_groq_transcription_api_key_setting,
        shortcut::change_multilingual_mode_enabled_setting,
        shortcut::change_primary_language_setting,
        shortcut::change_secondary_language_setting,
        shortcut::change_language_detection_sensitivity_setting,
        shortcut::change_show_meeting_menu_setting,
        shortcut::change_meeting_mode_enabled_setting,
        shortcut::change_meeting_chunk_duration_setting,
        shortcut::change_meeting_auto_summarize_setting,
        shortcut::change_meeting_extract_action_items_setting,
        shortcut::change_meeting_summary_prompt_setting,
        shortcut::change_meeting_action_items_prompt_setting,
        shortcut::change_live_preview_enabled_setting,
        shortcut::change_live_preview_interval_setting,
        shortcut::change_whisper_mode_enabled_setting,
        shortcut::change_vad_threshold_setting,
        commands::meeting::get_meeting_state,
        commands::meeting::get_current_meeting_session,
        commands::meeting::get_meeting_elapsed_seconds,
        commands::meeting::start_meeting,
        commands::meeting::stop_meeting,
        commands::meeting::cancel_meeting,
        commands::meeting::get_meeting_history,
        commands::meeting::delete_meeting,
        trigger_update_check,
        commands::cancel_operation,
        commands::get_app_dir_path,
        commands::get_app_settings,
        commands::get_default_settings,
        commands::get_log_dir_path,
        commands::set_log_level,
        commands::open_recordings_folder,
        commands::open_log_dir,
        commands::open_app_data_dir,
        commands::check_apple_intelligence_available,
        commands::initialize_enigo,
        commands::models::get_available_models,
        commands::models::get_model_info,
        commands::models::download_model,
        commands::models::delete_model,
        commands::models::cancel_download,
        commands::models::set_active_model,
        commands::models::get_current_model,
        commands::models::get_transcription_model_status,
        commands::models::is_model_loading,
        commands::models::has_any_models_available,
        commands::models::has_any_models_or_downloads,
        commands::models::get_recommended_first_model,
        commands::audio::update_microphone_mode,
        commands::audio::get_microphone_mode,
        commands::audio::get_available_microphones,
        commands::audio::set_selected_microphone,
        commands::audio::get_selected_microphone,
        commands::audio::get_available_output_devices,
        commands::audio::set_selected_output_device,
        commands::audio::get_selected_output_device,
        commands::audio::play_test_sound,
        commands::audio::check_custom_sounds,
        commands::audio::set_clamshell_microphone,
        commands::audio::get_clamshell_microphone,
        commands::audio::is_recording,
        commands::audio::get_system_audio_info,
        commands::audio::is_native_system_audio_available,
        commands::audio::start_system_audio_capture,
        commands::audio::stop_system_audio_capture,
        commands::audio::is_capturing_system_audio,
        commands::diarization::get_diarization_status,
        commands::diarization::change_diarization_enabled_setting,
        commands::diarization::get_diarization_model_info,
        commands::diarization::download_diarization_models,
        commands::transcription::set_model_unload_timeout,
        commands::transcription::get_model_load_status,
        commands::transcription::unload_model_manually,
        commands::history::get_history_entries,
        commands::history::toggle_history_entry_saved,
        commands::history::get_audio_file_path,
        commands::history::delete_history_entry,
        commands::history::update_history_limit,
        commands::history::update_recording_retention_period,
        commands::file_transcription::get_supported_file_extensions,
        commands::file_transcription::queue_file_for_transcription,
        commands::file_transcription::queue_files_for_transcription,
        commands::file_transcription::process_next_file,
        commands::file_transcription::process_all_files,
        commands::file_transcription::cancel_file_transcription,
        commands::file_transcription::cancel_file_transcription_job,
        commands::file_transcription::get_file_transcription_jobs,
        commands::file_transcription::get_file_transcription_job,
        commands::file_transcription::clear_completed_file_jobs,
        commands::file_transcription::remove_file_transcription_job,
        commands::file_transcription::is_file_transcription_processing,
        commands::export::export_transcript,
        commands::export::export_transcript_to_file,
        commands::export::get_export_file_extension,
        commands::export::get_available_export_formats,
        commands::watch_folder::get_watch_folders,
        commands::watch_folder::add_watch_folder,
        commands::watch_folder::remove_watch_folder,
        commands::watch_folder::update_watch_folder,
        commands::watch_folder::get_watch_folder_status,
        commands::watch_folder::start_watch_folder,
        commands::watch_folder::stop_watch_folder,
        helpers::clamshell::is_laptop,
    ]);

    #[cfg(debug_assertions)] // <- Only export on non-release builds
    specta_builder
        .export(
            Typescript::default().bigint(BigIntExportBehavior::Number),
            "../src/bindings.ts",
        )
        .expect("Failed to export typescript bindings");

    let mut builder = tauri::Builder::default().plugin(
        LogBuilder::new()
            .level(log::LevelFilter::Trace) // Set to most verbose level globally
            .max_file_size(500_000)
            .rotation_strategy(RotationStrategy::KeepOne)
            .clear_targets()
            .targets([
                // Console output respects RUST_LOG environment variable
                Target::new(TargetKind::Stdout).filter({
                    let console_filter = console_filter.clone();
                    move |metadata| console_filter.enabled(metadata)
                }),
                // File logs respect the user's settings (stored in FILE_LOG_LEVEL atomic)
                Target::new(TargetKind::LogDir {
                    file_name: Some("paperflow".into()),
                })
                .filter(|metadata| {
                    let file_level = FILE_LOG_LEVEL.load(Ordering::Relaxed);
                    metadata.level() <= level_filter_from_u8(file_level)
                }),
            ])
            .build(),
    );

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_macos_permissions::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .manage(Mutex::new(ShortcutToggleStates::default()))
        .setup(move |app| {
            let settings = get_settings(&app.handle());
            let tauri_log_level: tauri_plugin_log::LogLevel = settings.log_level.into();
            let file_log_level: log::Level = tauri_log_level.into();
            // Store the file log level in the atomic for the filter to use
            FILE_LOG_LEVEL.store(file_log_level.to_level_filter() as u8, Ordering::Relaxed);
            let app_handle = app.handle().clone();

            initialize_core_logic(&app_handle);

            // Show main window only if not starting hidden
            if !settings.start_hidden {
                if let Some(main_window) = app_handle.get_webview_window("main") {
                    main_window.show().unwrap();
                    main_window.set_focus().unwrap();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _res = window.hide();
                #[cfg(target_os = "macos")]
                {
                    let res = window
                        .app_handle()
                        .set_activation_policy(tauri::ActivationPolicy::Accessory);
                    if let Err(e) = res {
                        log::error!("Failed to set activation policy: {}", e);
                    }
                }
            }
            tauri::WindowEvent::ThemeChanged(theme) => {
                log::info!("Theme changed to: {:?}", theme);
                // Update tray icon to match new theme, maintaining idle state
                utils::change_tray_icon(&window.app_handle(), utils::TrayIconState::Idle);
            }
            _ => {}
        })
        .invoke_handler(specta_builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
