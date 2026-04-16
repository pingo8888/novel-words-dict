use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::app::state::AppState;
use crate::core::types::{DictionaryOption, NameEntry};
use crate::store::{QueryRequest, QueryResponse};

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
pub(crate) fn upsert_entry(
    app: AppHandle,
    state: State<AppState>,
    entry: NameEntry,
) -> Result<(), String> {
    let term_for_event = entry.term.trim().to_string();
    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "保存词条失败：状态锁已中毒（poisoned）".to_string())?;
        store.upsert(entry)?;
    }

    let _ = app.emit_to("main", "entry-updated", term_for_event);
    Ok(())
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
