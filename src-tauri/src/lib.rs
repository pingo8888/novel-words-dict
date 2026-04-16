use std::sync::Mutex;

mod app;
mod core;
mod infra;
mod store;

use crate::app::{
    bootstrap::setup_app,
    commands::{
        close_editor_window, delete_entry, get_app_settings, get_bundled_entry_dict_name, get_entry,
        list_dictionaries, open_editor_window, query_entries, save_app_settings, set_editor_seed,
        take_editor_seed, upsert_entry,
    },
    state::{AppState, EditorSeed, HotkeyState, SettingsState},
};

const DATA_FILE_NAME: &str = "entries.json";
const LEGACY_DATA_FILE_NAME: &str = "entries.ndjson";
const SETTINGS_FILE_NAME: &str = "settings.json";
const DEFAULT_HOTKEY: &str = "Alt+Z";
const BUNDLED_DICT_DIR_NAME: &str = "dict";
const ALL_DICT_ID: &str = "all";
const ALL_DICT_NAME: &str = "所有词库";
const CUSTOM_DICT_ID: &str = "custom";
const CUSTOM_DICT_NAME: &str = "自定词库";
const PAGE_SIZE: usize = 40;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            let label = window.label();
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if label == "main" || label == "editor" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .manage(AppState::default())
        .manage(SettingsState::default())
        .manage(EditorSeed::default())
        .manage(HotkeyState(Mutex::new(DEFAULT_HOTKEY.to_string())))
        .plugin(tauri_plugin_opener::init())
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            query_entries,
            list_dictionaries,
            get_entry,
            get_bundled_entry_dict_name,
            upsert_entry,
            delete_entry,
            get_app_settings,
            save_app_settings,
            open_editor_window,
            take_editor_seed,
            close_editor_window,
            set_editor_seed
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
