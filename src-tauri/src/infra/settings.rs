use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::AppHandle;

use crate::infra::paths::{resolve_project_data_dir, resolve_settings_file_path};
use crate::DEFAULT_HOTKEY;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    normalized
        .chars()
        .last()
        .filter(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch as u32)
        .unwrap_or('Z' as u32)
}

pub(crate) fn default_settings(project_data_dir: &Path) -> AppSettings {
    AppSettings {
        dict_dir: project_data_dir.to_string_lossy().to_string(),
        hotkey: DEFAULT_HOTKEY.to_string(),
    }
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

    let patch: AppSettingsPatch =
        serde_json::from_str(&text).map_err(|err| format!("解析设置失败: {err}"))?;
    let dict_dir = patch
        .dict_dir
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| project_data_dir.to_string_lossy().to_string());
    let hotkey = normalize_hotkey(patch.hotkey.as_deref().unwrap_or(DEFAULT_HOTKEY));
    Ok(AppSettings { dict_dir, hotkey })
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
    if settings_path.exists() {
        fs::remove_file(&settings_path).map_err(|err| format!("替换设置文件失败: {err}"))?;
    }
    fs::rename(temp_path, settings_path).map_err(|err| format!("保存设置文件失败: {err}"))?;
    Ok(())
}
