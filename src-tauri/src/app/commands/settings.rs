use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::app::state::{HotkeyEnabled, HotkeyState, SettingsState};
use crate::infra::paths::{
    resolve_project_data_dir, sanitize_windows_verbatim_prefix,
};
use crate::infra::settings::{
    default_settings, normalize_hotkey, normalize_search_engine, persist_app_settings, AppSettings,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSettingsRequest {
    hotkey: String,
    search_engine: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsResponse {
    dict_dir: String,
    hotkey: String,
    search_engine: String,
    project_data_dir: String,
}

fn build_settings_response(settings: &AppSettings, project_data_dir: &Path) -> SettingsResponse {
    SettingsResponse {
        dict_dir: sanitize_windows_verbatim_prefix(settings.dict_dir.as_str()),
        hotkey: settings.hotkey.clone(),
        search_engine: settings.search_engine.clone(),
        project_data_dir: sanitize_windows_verbatim_prefix(
            project_data_dir.to_string_lossy().as_ref(),
        ),
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
        .map_err(|_| "读取设置失败：状态锁已中毒（poisoned）".to_string())?
        .clone()
        .unwrap_or_else(|| default_settings(&project_data_dir));
    Ok(build_settings_response(&settings, &project_data_dir))
}

#[tauri::command]
pub(crate) fn save_app_settings(
    app: AppHandle,
    settings_state: State<SettingsState>,
    hotkey_state: State<HotkeyState>,
    request: SaveSettingsRequest,
) -> Result<SettingsResponse, String> {
    let project_data_dir = resolve_project_data_dir(&app)?;
    let normalized_hotkey = normalize_hotkey(&request.hotkey);
    let normalized_search_engine =
        normalize_search_engine(request.search_engine.as_deref().unwrap_or("google"));
    let fixed_dict_dir =
        sanitize_windows_verbatim_prefix(project_data_dir.to_string_lossy().as_ref());

    let normalized_settings = AppSettings {
        dict_dir: fixed_dict_dir,
        hotkey: normalized_hotkey.clone(),
        search_engine: normalized_search_engine,
    };
    persist_app_settings(&app, &normalized_settings)?;

    {
        let mut settings_guard = settings_state
            .0
            .lock()
            .map_err(|_| "保存设置失败：设置状态锁已中毒（poisoned）".to_string())?;
        *settings_guard = Some(normalized_settings.clone());
    }
    {
        let mut hotkey_guard = hotkey_state
            .0
            .lock()
            .map_err(|_| "保存设置失败：快捷键状态锁已中毒（poisoned）".to_string())?;
        *hotkey_guard = normalized_hotkey;
    }

    let _ = app.emit_to("main", "entry-updated", String::new());
    Ok(build_settings_response(
        &normalized_settings,
        &project_data_dir,
    ))
}

#[tauri::command]
pub(crate) fn set_hotkey_enabled(
    hotkey_enabled: State<HotkeyEnabled>,
    enabled: bool,
) -> Result<(), String> {
    hotkey_enabled.set_enabled(enabled);
    Ok(())
}
