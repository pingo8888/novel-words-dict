use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[cfg(desktop)]
use crate::app::platform::setup_tray_icon;
use crate::app::platform::start_hotkey_listener;
use crate::app::state::{AppState, HotkeyState, SettingsState};
use crate::infra::paths::{
    normalize_dict_dir, resolve_custom_db_path, resolve_entries_file_path,
    resolve_project_data_dir, sanitize_windows_verbatim_prefix, sync_bundled_db_to_install_dir,
    validate_dict_dir_path,
};
use crate::infra::settings::{
    default_settings, load_app_settings, persist_app_settings, should_persist_settings,
};

pub(crate) fn setup_app<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>>
where
    tauri::AppHandle<R>: Send + 'static,
{
    let app_handle = app.handle();
    sync_bundled_db_to_install_dir(app_handle);
    let project_data_dir = resolve_project_data_dir(app_handle).map_err(std::io::Error::other)?;
    let mut loaded_settings = load_app_settings(app_handle).map_err(std::io::Error::other)?;
    let candidate_dict_dir = normalize_dict_dir(&loaded_settings.dict_dir, &project_data_dir);
    let dict_dir = match validate_dict_dir_path(&candidate_dict_dir, &project_data_dir) {
        Ok(path) => {
            loaded_settings.dict_dir =
                sanitize_windows_verbatim_prefix(path.to_string_lossy().as_ref());
            path
        }
        Err(err) => {
            eprintln!("设置中的词库目录无效，已回退默认目录: {err}");
            let fallback = default_settings(&project_data_dir);
            loaded_settings.dict_dir = fallback.dict_dir;
            PathBuf::from(loaded_settings.dict_dir.as_str())
        }
    };
    fs::create_dir_all(&dict_dir)
        .map_err(|err| std::io::Error::other(format!("创建词库目录失败: {err}")))?;
    let legacy_entries_path = resolve_entries_file_path(&dict_dir);
    let custom_db_path = resolve_custom_db_path(&project_data_dir);
    let app_state = app.state::<AppState>();
    if let Ok(mut store) = app_state.store.lock() {
        store
            .load(app_handle, custom_db_path, legacy_entries_path)
            .map_err(std::io::Error::other)?;
    } else {
        return Err(std::io::Error::other("状态锁已中毒（poisoned）").into());
    }

    let settings_state = app.state::<SettingsState>();
    if let Ok(mut guard) = settings_state.0.lock() {
        *guard = Some(loaded_settings.clone());
    } else {
        return Err(std::io::Error::other("设置状态锁已中毒（poisoned）").into());
    }

    let hotkey_state = app.state::<HotkeyState>();
    if let Ok(mut guard) = hotkey_state.0.lock() {
        *guard = loaded_settings.hotkey.clone();
    } else {
        return Err(std::io::Error::other("快捷键状态锁已中毒（poisoned）").into());
    }

    match should_persist_settings(app_handle, &loaded_settings) {
        Ok(true) => {
            if let Err(err) = persist_app_settings(app_handle, &loaded_settings) {
                eprintln!("{err}");
            }
        }
        Ok(false) => {}
        Err(err) => {
            eprintln!("检查设置持久化状态失败：{err}");
        }
    }

    start_hotkey_listener(app_handle.clone());

    #[cfg(desktop)]
    setup_tray_icon(app)?;

    Ok(())
}
