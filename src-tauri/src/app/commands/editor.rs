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
    term: &str,
) -> Result<(), String> {
    let seed_state = app.state::<EditorSeed>();
    let mut guard = seed_state
        .0
        .lock()
        .map_err(|_| "写入编辑词条失败：状态锁已中毒（poisoned）".to_string())?;
    *guard = Some(term.to_string());
    Ok(())
}

#[tauri::command]
pub(crate) fn open_editor_window(app: AppHandle, request: OpenEditorRequest) -> Result<(), String> {
    let term = request.term.unwrap_or_default();
    set_editor_seed_value(&app, &term)?;
    app.emit_to("main", "editor-open-request", term)
        .map_err(|err| format!("发送编辑窗口事件失败: {err}"))?;
    Ok(())
}

#[tauri::command]
pub(crate) fn take_editor_seed(editor_seed: State<EditorSeed>) -> Result<Option<String>, String> {
    let mut guard = editor_seed
        .0
        .lock()
        .map_err(|_| "读取编辑词条失败：状态锁已中毒（poisoned）".to_string())?;
    Ok(guard.take())
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
    set_editor_seed_value(&app, &term)
}

#[tauri::command]
pub(crate) fn set_editor_window_title(app: AppHandle, title: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("editor") {
        window
            .set_title(title.trim())
            .map_err(|err| format!("设置编辑窗口标题失败: {err}"))?;
    }
    Ok(())
}
