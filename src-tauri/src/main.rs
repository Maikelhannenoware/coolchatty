#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod history;
mod hotkeys;
mod paste;
mod settings;
mod state;
mod websocket;

use audio::RecorderService;
use history::HistoryStore;
use hotkeys::HotkeyManager;
use paste::PasteManager;
use settings::SettingsStore;
use state::AppState;
use tauri::Manager;

fn main() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install AWS-LC crypto provider");
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
            let hotkeys = HotkeyManager::new(&initial_settings.hotkey);
            let history = tauri::async_runtime::block_on(HistoryStore::new())?;

            let state = AppState::new(recorder, history, paste, hotkeys, settings_store);
            app.manage(state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run CoolChatty");
}
