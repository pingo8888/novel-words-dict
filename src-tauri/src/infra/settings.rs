use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::AppHandle;

use crate::infra::files::replace_file_from_temp;
use crate::infra::paths::{
    resolve_project_data_dir, resolve_settings_file_path, sanitize_windows_verbatim_prefix,
};
use crate::DEFAULT_HOTKEY;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) dict_dir: String,
    pub(crate) hotkey: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsPatch {
    dict_dir: Option<String>,
    hotkey: Option<String>,
}

pub(crate) fn normalize_hotkey(input: &str) -> String {
    let compact = input.trim().replace(' ', "").to_ascii_uppercase();
    let mut parts = compact.split('+');
    let Some(modifier) = parts.next() else {
        return DEFAULT_HOTKEY.to_string();
    };
    let Some(key) = parts.next() else {
        return DEFAULT_HOTKEY.to_string();
    };
    if parts.next().is_some() || modifier != "ALT" {
        return DEFAULT_HOTKEY.to_string();
    }
    if key.len() != 1 {
        return DEFAULT_HOTKEY.to_string();
    }
    let letter = key.chars().next().unwrap_or('Z');
    if !letter.is_ascii_alphabetic() {
        return DEFAULT_HOTKEY.to_string();
    }
    format!("Alt+{}", letter.to_ascii_uppercase())
}

pub(crate) fn hotkey_virtual_key(hotkey: &str) -> u32 {
    let normalized = normalize_hotkey(hotkey);
    // Current hotkey format only allows Alt+[A-Z], so ASCII letter code equals virtual-key code.
    normalized
        .chars()
        .last()
        .filter(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch as u32)
        .unwrap_or('Z' as u32)
}

pub(crate) fn default_settings(project_data_dir: &Path) -> AppSettings {
    AppSettings {
        dict_dir: sanitize_windows_verbatim_prefix(project_data_dir.to_string_lossy().as_ref()),
        hotkey: DEFAULT_HOTKEY.to_string(),
    }
}

fn parse_settings_text(text: &str, project_data_dir: &Path) -> Result<AppSettings, String> {
    let patch: AppSettingsPatch =
        serde_json::from_str(text).map_err(|err| format!("解析设置失败: {err}"))?;
    let dict_dir = patch
        .dict_dir
        .filter(|value| !value.trim().is_empty())
        .map(|value| sanitize_windows_verbatim_prefix(value.as_str()))
        .unwrap_or_else(|| sanitize_windows_verbatim_prefix(project_data_dir.to_string_lossy().as_ref()));
    let hotkey = normalize_hotkey(patch.hotkey.as_deref().unwrap_or(DEFAULT_HOTKEY));
    Ok(AppSettings { dict_dir, hotkey })
}

pub(crate) fn load_app_settings<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<AppSettings, String> {
    let project_data_dir = resolve_project_data_dir(app)?;
    let settings_path = resolve_settings_file_path(app)?;
    if !settings_path.exists() {
        return Ok(default_settings(&project_data_dir));
    }

    let text = fs::read_to_string(&settings_path).map_err(|err| format!("读取设置失败: {err}"))?;
    if text.trim().is_empty() {
        return Ok(default_settings(&project_data_dir));
    }
    parse_settings_text(&text, &project_data_dir)
}

pub(crate) fn persist_app_settings<R: tauri::Runtime>(
    app: &AppHandle<R>,
    settings: &AppSettings,
) -> Result<(), String> {
    let settings_path = resolve_settings_file_path(app)?;
    let payload =
        serde_json::to_string_pretty(settings).map_err(|err| format!("序列化设置失败: {err}"))?;
    let temp_path = settings_path.with_extension("json.tmp");
    fs::write(&temp_path, payload).map_err(|err| format!("写入设置临时文件失败: {err}"))?;
    replace_file_from_temp(&temp_path, &settings_path)?;
    Ok(())
}

pub(crate) fn should_persist_settings<R: tauri::Runtime>(
    app: &AppHandle<R>,
    settings: &AppSettings,
) -> Result<bool, String> {
    let project_data_dir = resolve_project_data_dir(app)?;
    let settings_path = resolve_settings_file_path(app)?;
    if !settings_path.exists() {
        return Ok(true);
    }

    let text = fs::read_to_string(&settings_path).map_err(|err| format!("读取设置失败: {err}"))?;
    if text.trim().is_empty() {
        return Ok(true);
    }

    let normalized_existing = match parse_settings_text(&text, &project_data_dir) {
        Ok(value) => value,
        Err(_) => return Ok(true),
    };
    Ok(normalized_existing != *settings)
}
