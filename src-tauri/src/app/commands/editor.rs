use serde::Deserialize;
use tauri::Emitter;
use tauri::Manager;
use tauri::{AppHandle, State};

use crate::app::state::EditorSeed;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenEditorRequest {
    term: Option<String>,
}

pub(crate) fn set_editor_seed_value<R: tauri::Runtime>(
    app: &AppHandle<R>,
    term: String,
) -> Result<(), String> {
    let seed_state = app.state::<EditorSeed>();
    let mut guard = seed_state
        .0
        .lock()
        .map_err(|_| "写入编辑词条失败：状态锁不可用".to_string())?;
    *guard = Some(term);
    Ok(())
}

#[tauri::command]
pub(crate) fn open_editor_window(app: AppHandle, request: OpenEditorRequest) -> Result<(), String> {
    let term = request.term.unwrap_or_default();
    set_editor_seed_value(&app, term.clone())?;
    app.emit_to("main", "editor-open-request", term)
        .map_err(|err| format!("发送编辑窗口事件失败: {err}"))?;
    Ok(())
}

#[tauri::command]
pub(crate) fn take_editor_seed(editor_seed: State<EditorSeed>) -> Option<String> {
    if let Ok(mut guard) = editor_seed.0.lock() {
        guard.take()
    } else {
        None
    }
}

#[tauri::command]
pub(crate) fn close_editor_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("editor") {
        window
            .hide()
            .map_err(|err| format!("隐藏编辑窗口失败: {err}"))?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_editor_seed(app: AppHandle, term: String) -> Result<(), String> {
    set_editor_seed_value(&app, term)
}
