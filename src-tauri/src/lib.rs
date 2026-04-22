use std::sync::Mutex;
use tauri::Manager;

mod app;
mod core;
mod infra;
mod store;

use crate::app::{
    bootstrap::setup_app,
    commands::{
        close_editor_window, delete_entry, get_app_settings, get_bundled_entry,
        get_bundled_entry_dict_name, get_entry, get_last_add_preset, list_dictionaries,
        open_editor_window, query_entries, save_app_settings, set_editor_seed,
        set_editor_window_title, set_hotkey_enabled, take_editor_seed, upsert_entry,
    },
    state::{
        AppState, EditorSeed, HotkeyEnabled, HotkeyShutdown, HotkeyState, LastAddPresetState,
        SettingsState,
    },
};

const DATA_FILE_NAME: &str = "entries.json";
const LEGACY_DATA_FILE_NAME: &str = "entries.ndjson";
const CUSTOM_DB_FILE_NAME: &str = "custom.db";
const BUILTIN_DB_FILE_NAME: &str = "build-in.db";
const SETTINGS_FILE_NAME: &str = "settings.json";
const DEFAULT_HOTKEY: &str = "Alt+D";
const BUNDLED_DICT_DIR_NAME: &str = "dict";
const BUNDLED_DICT_ORDER_FILE_NAME: &str = "dict-orders.json";
const ALL_DICT_ID: &str = "all";
const ALL_DICT_NAME: &str = "所有词库";
const CUSTOM_DICT_ID: &str = "custom";
const CUSTOM_DICT_NAME: &str = "自定词库";
const PAGE_SIZE: usize = 40;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
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
        .manage(LastAddPresetState::default())
        .manage(HotkeyState(Mutex::new(DEFAULT_HOTKEY.to_string())))
        .manage(HotkeyEnabled::default())
        .manage(HotkeyShutdown::default())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
                let _ = main_window.unminimize();
                let _ = main_window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            setup_app(app)
        })
        .invoke_handler(tauri::generate_handler![
            query_entries,
            list_dictionaries,
            get_entry,
            get_bundled_entry,
            get_bundled_entry_dict_name,
            get_last_add_preset,
            upsert_entry,
            delete_entry,
            get_app_settings,
            save_app_settings,
            set_hotkey_enabled,
            open_editor_window,
            take_editor_seed,
            close_editor_window,
            set_editor_seed,
            set_editor_window_title
        ]);

    if let Err(err) = app.run(tauri::generate_context!()) {
        eprintln!("运行应用失败: {err}");
        std::process::exit(1);
    }
}
