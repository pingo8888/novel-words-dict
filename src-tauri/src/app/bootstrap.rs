use std::fs;
use std::path::PathBuf;
use tauri::Manager;

use crate::app::platform::start_hotkey_listener;
#[cfg(desktop)]
use crate::app::platform::setup_tray_icon;
use crate::app::state::{AppState, HotkeyState, SettingsState};
use crate::infra::paths::{resolve_entries_file_path, sync_bundled_dict_to_install_dir};
use crate::infra::settings::{load_app_settings, normalize_hotkey, persist_app_settings};

pub(crate) fn setup_app<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>>
where
    tauri::AppHandle<R>: Send + 'static,
{
    let app_handle = app.handle();
    sync_bundled_dict_to_install_dir(app_handle);
    let loaded_settings = load_app_settings(app_handle).map_err(std::io::Error::other)?;
    let dict_dir = PathBuf::from(loaded_settings.dict_dir.as_str());
    fs::create_dir_all(&dict_dir)
        .map_err(|err| std::io::Error::other(format!("创建词库目录失败: {err}")))?;
    let data_path = resolve_entries_file_path(&dict_dir);
    let app_state = app.state::<AppState>();
    if let Ok(mut store) = app_state.store.lock() {
        store
            .load(app_handle, data_path)
            .map_err(std::io::Error::other)?;
    } else {
        return Err(std::io::Error::other("状态锁不可用").into());
    }

    let settings_state = app.state::<SettingsState>();
    if let Ok(mut guard) = settings_state.0.lock() {
        *guard = Some(loaded_settings.clone());
    } else {
        return Err(std::io::Error::other("设置状态锁不可用").into());
    }

    let hotkey_state = app.state::<HotkeyState>();
    if let Ok(mut guard) = hotkey_state.0.lock() {
        *guard = normalize_hotkey(&loaded_settings.hotkey);
    } else {
        return Err(std::io::Error::other("快捷键状态锁不可用").into());
    }

    if let Err(err) = persist_app_settings(app_handle, &loaded_settings) {
        eprintln!("{err}");
    }

    start_hotkey_listener(app_handle.clone());

    #[cfg(desktop)]
    setup_tray_icon(app)?;

    Ok(())
}
