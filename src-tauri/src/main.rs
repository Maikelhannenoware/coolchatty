#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod errors;
mod history;
mod hotkey;
mod paste;
mod realtime;
mod settings;
mod state;

use audio::RecorderService;
use history::HistoryStore;
use hotkey::HotkeyManager;
use paste::PasteManager;
use settings::SettingsStore;
use state::AppState;
use tauri::Manager;

fn main() {
    init_crypto();
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::get_history,
            commands::clear_history,
            commands::recorder_status,
            commands::trigger_record_event,
            commands::get_settings,
            commands::save_settings,
        ])
        .setup(|app| {
            let recorder = RecorderService::new();
            let paste = PasteManager::new();
            let settings_store = SettingsStore::load()?;
            let initial_settings = tauri::async_runtime::block_on(settings_store.get());
            let hotkeys = {
                let handle = app.handle();
                HotkeyManager::new(&handle, &initial_settings.hotkey)?
            };
            let history = tauri::async_runtime::block_on(HistoryStore::new())?;

            let state = AppState::new(recorder, history, paste, hotkeys, settings_store);
            app.manage(state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run CoolChatty");
}

fn init_crypto() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install AWS-LC crypto provider");
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("coolchatty=info"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(filter_layer)
        .with_target(false)
        .compact()
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}
