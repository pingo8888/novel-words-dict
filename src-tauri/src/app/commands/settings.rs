use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::app::state::{AppState, HotkeyState, SettingsState};
use crate::infra::paths::{normalize_dict_dir, resolve_entries_file_path, resolve_project_data_dir};
use crate::infra::settings::{
    default_settings, normalize_hotkey, persist_app_settings, AppSettings,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSettingsRequest {
    dict_dir: String,
    hotkey: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsResponse {
    dict_dir: String,
    hotkey: String,
    project_data_dir: String,
}

fn build_settings_response(settings: &AppSettings, project_data_dir: &Path) -> SettingsResponse {
    SettingsResponse {
        dict_dir: settings.dict_dir.clone(),
        hotkey: settings.hotkey.clone(),
        project_data_dir: project_data_dir.to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub(crate) fn get_app_settings(
    app: AppHandle,
    settings_state: State<SettingsState>,
) -> Result<SettingsResponse, String> {
    let project_data_dir = resolve_project_data_dir(&app)?;
    let settings = settings_state
        .0
        .lock()
        .map_err(|_| "读取设置失败：状态锁不可用".to_string())?
        .clone()
        .unwrap_or_else(|| default_settings(&project_data_dir));
    Ok(build_settings_response(&settings, &project_data_dir))
}

#[tauri::command]
pub(crate) fn save_app_settings(
    app: AppHandle,
    state: State<AppState>,
    settings_state: State<SettingsState>,
    hotkey_state: State<HotkeyState>,
    request: SaveSettingsRequest,
) -> Result<SettingsResponse, String> {
    let project_data_dir = resolve_project_data_dir(&app)?;
    let normalized_hotkey = normalize_hotkey(&request.hotkey);
    let dict_dir_path = normalize_dict_dir(&request.dict_dir, &project_data_dir);
    fs::create_dir_all(&dict_dir_path).map_err(|err| format!("创建词库目录失败: {err}"))?;

    let data_path = resolve_entries_file_path(&dict_dir_path);
    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "保存设置失败：词库状态锁不可用".to_string())?;
        store.load(&app, data_path)?;
    }

    let normalized_settings = AppSettings {
        dict_dir: dict_dir_path.to_string_lossy().to_string(),
        hotkey: normalized_hotkey.clone(),
    };
    persist_app_settings(&app, &normalized_settings)?;

    {
        let mut settings_guard = settings_state
            .0
            .lock()
            .map_err(|_| "保存设置失败：设置状态锁不可用".to_string())?;
        *settings_guard = Some(normalized_settings.clone());
    }
    {
        let mut hotkey_guard = hotkey_state
            .0
            .lock()
            .map_err(|_| "保存设置失败：快捷键状态锁不可用".to_string())?;
        *hotkey_guard = normalized_hotkey;
    }

    let _ = app.emit_to("main", "entry-updated", String::new());
    Ok(build_settings_response(
        &normalized_settings,
        &project_data_dir,
    ))
}
