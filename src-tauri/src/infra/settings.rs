use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::AppHandle;

use crate::infra::files::replace_file_from_temp;
use crate::infra::paths::{
    resolve_project_data_dir, resolve_settings_file_path, sanitize_windows_verbatim_prefix,
};
use crate::DEFAULT_HOTKEY;
const DEFAULT_SEARCH_ENGINE: &str = "google";

#[derive(Debug, Clone, Copy)]
struct ParsedHotkey {
    ctrl: bool,
    alt: bool,
    shift: bool,
    key: char,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) dict_dir: String,
    pub(crate) hotkey: String,
    pub(crate) search_engine: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsPatch {
    hotkey: Option<String>,
    search_engine: Option<String>,
}

pub(crate) fn normalize_hotkey(input: &str) -> String {
    parse_hotkey(input)
        .or_else(|| parse_hotkey(DEFAULT_HOTKEY))
        .map(format_hotkey)
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
}

pub(crate) fn hotkey_virtual_key(hotkey: &str) -> u32 {
    parse_hotkey(hotkey)
        .or_else(|| parse_hotkey(DEFAULT_HOTKEY))
        .map(|parsed| parsed.key as u32)
        .unwrap_or('D' as u32)
}

pub(crate) fn hotkey_modifier_state(hotkey: &str) -> (bool, bool, bool) {
    let parsed = parse_hotkey(hotkey)
        .or_else(|| parse_hotkey(DEFAULT_HOTKEY))
        .unwrap_or(ParsedHotkey {
            ctrl: false,
            alt: true,
            shift: false,
            key: 'D',
        });
    (parsed.ctrl, parsed.alt, parsed.shift)
}

pub(crate) fn normalize_search_engine(input: &str) -> String {
    match input.trim().to_ascii_lowercase().as_str() {
        "google" => "google".to_string(),
        "bing" => "bing".to_string(),
        "baidu" => "baidu".to_string(),
        _ => DEFAULT_SEARCH_ENGINE.to_string(),
    }
}

pub(crate) fn default_settings(project_data_dir: &Path) -> AppSettings {
    AppSettings {
        dict_dir: sanitize_windows_verbatim_prefix(project_data_dir.to_string_lossy().as_ref()),
        hotkey: DEFAULT_HOTKEY.to_string(),
        search_engine: DEFAULT_SEARCH_ENGINE.to_string(),
    }
}

fn parse_settings_text(text: &str, project_data_dir: &Path) -> Result<AppSettings, String> {
    let patch: AppSettingsPatch =
        serde_json::from_str(text).map_err(|err| format!("解析设置失败: {err}"))?;
    let dict_dir = sanitize_windows_verbatim_prefix(project_data_dir.to_string_lossy().as_ref());
    let hotkey = normalize_hotkey(patch.hotkey.as_deref().unwrap_or(DEFAULT_HOTKEY));
    let search_engine = normalize_search_engine(
        patch
            .search_engine
            .as_deref()
            .unwrap_or(DEFAULT_SEARCH_ENGINE),
    );
    Ok(AppSettings {
        dict_dir,
        hotkey,
        search_engine,
    })
}

fn parse_hotkey(input: &str) -> Option<ParsedHotkey> {
    let compact = input.trim().replace(' ', "").to_ascii_uppercase();
    if compact.is_empty() {
        return None;
    }

    let parts: Vec<&str> = compact.split('+').collect();
    if parts.len() < 2 || parts.len() > 4 {
        return None;
    }

    let key_raw = parts.last().copied().unwrap_or_default();
    if key_raw.len() != 1 {
        return None;
    }
    let key = key_raw.chars().next()?;
    if !key.is_ascii_alphanumeric() {
        return None;
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    for modifier in &parts[..parts.len() - 1] {
        match *modifier {
            "CTRL" => {
                if ctrl {
                    return None;
                }
                ctrl = true;
            }
            "ALT" => {
                if alt {
                    return None;
                }
                alt = true;
            }
            "SHIFT" => {
                if shift {
                    return None;
                }
                shift = true;
            }
            _ => return None,
        }
    }

    let valid_modifier_combo = (alt && !ctrl && !shift) || (ctrl && alt && !shift);
    if !valid_modifier_combo {
        return None;
    }

    Some(ParsedHotkey {
        ctrl,
        alt,
        shift,
        key,
    })
}

fn format_hotkey(parsed: ParsedHotkey) -> String {
    let mut parts = Vec::with_capacity(4);
    if parsed.ctrl {
        parts.push("Ctrl".to_string());
    }
    if parsed.alt {
        parts.push("Alt".to_string());
    }
    if parsed.shift {
        parts.push("Shift".to_string());
    }
    parts.push(parsed.key.to_ascii_uppercase().to_string());
    parts.join("+")
}

pub(crate) fn load_app_settings<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<AppSettings, String> {
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
