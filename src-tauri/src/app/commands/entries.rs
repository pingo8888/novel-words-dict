use tauri::Emitter;
use tauri::{AppHandle, State};

use serde::Serialize;

use crate::app::state::{AppState, LastAddPreset, LastAddPresetState};
use crate::core::types::{DictionaryOption, GenderType, GenreType, NameEntry, NameType};
use crate::store::{QueryRequest, QueryResponse};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LastAddPresetResponse {
    genre: GenreType,
    group: String,
    name_type: NameType,
    gender_type: GenderType,
}

#[tauri::command]
pub(crate) fn query_entries(
    state: State<AppState>,
    request: QueryRequest,
) -> Result<QueryResponse, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取数据失败：状态锁已中毒（poisoned）".to_string())?;
    Ok(store.query(&request))
}

#[tauri::command]
pub(crate) fn list_dictionaries(state: State<AppState>) -> Result<Vec<DictionaryOption>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取词库失败：状态锁已中毒（poisoned）".to_string())?;
    Ok(store.list_dictionaries())
}

#[tauri::command]
pub(crate) fn get_entry(state: State<AppState>, term: String) -> Result<Option<NameEntry>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取词条失败：状态锁已中毒（poisoned）".to_string())?;
    Ok(store.get_entry(&term))
}

#[tauri::command]
pub(crate) fn get_bundled_entry_dict_name(
    state: State<AppState>,
    term: String,
) -> Result<Option<String>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取词条失败：状态锁已中毒（poisoned）".to_string())?;
    Ok(store.get_bundled_entry_dict_name(&term))
}

#[tauri::command]
pub(crate) fn get_bundled_entry(
    state: State<AppState>,
    term: String,
) -> Result<Option<NameEntry>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取词条失败：状态锁已中毒（poisoned）".to_string())?;
    Ok(store.get_bundled_entry(&term))
}

#[tauri::command]
pub(crate) fn upsert_entry(
    app: AppHandle,
    state: State<AppState>,
    last_add_preset_state: State<LastAddPresetState>,
    entry: NameEntry,
    original_term: Option<String>,
) -> Result<(), String> {
    let is_new_add = original_term
        .as_deref()
        .map(str::trim)
        .map(|value| value.is_empty())
        .unwrap_or(true);
    let term_for_event = entry.term.trim().to_string();
    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "保存词条失败：状态锁已中毒（poisoned）".to_string())?;
        store.upsert(entry.clone(), original_term.as_deref())?;
    }

    if is_new_add {
        let gender_type = if matches!(entry.name_type, NameType::Surname | NameType::Given) {
            entry.gender_type
        } else {
            GenderType::Both
        };
        let mut guard = last_add_preset_state
            .0
            .lock()
            .map_err(|_| "保存词条失败：默认值状态锁已中毒（poisoned）".to_string())?;
        *guard = Some(LastAddPreset {
            genre: entry.genre,
            group: entry.group.trim().to_string(),
            name_type: entry.name_type,
            gender_type,
        });
    }

    let _ = app.emit_to("main", "entry-updated", term_for_event);
    Ok(())
}

#[tauri::command]
pub(crate) fn get_last_add_preset(
    last_add_preset_state: State<LastAddPresetState>,
) -> Result<Option<LastAddPresetResponse>, String> {
    let preset = last_add_preset_state
        .0
        .lock()
        .map_err(|_| "读取默认值失败：状态锁已中毒（poisoned）".to_string())?
        .clone();
    Ok(preset.map(|value| LastAddPresetResponse {
        genre: value.genre,
        group: value.group,
        name_type: value.name_type,
        gender_type: value.gender_type,
    }))
}

#[tauri::command]
pub(crate) fn delete_entry(
    app: AppHandle,
    state: State<AppState>,
    term: String,
) -> Result<(), String> {
    let trimmed_term = term.trim().to_string();
    if trimmed_term.is_empty() {
        return Err("词条不能为空".to_string());
    }

    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "删除词条失败：状态锁已中毒（poisoned）".to_string())?;
        store.delete(&trimmed_term)?;
    }

    let _ = app.emit_to("main", "entry-updated", trimmed_term);
    Ok(())
}
