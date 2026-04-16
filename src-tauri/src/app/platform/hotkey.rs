use tauri::AppHandle;

#[cfg(target_os = "windows")]
use tauri::Emitter;
#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(target_os = "windows")]
use crate::app::commands::set_editor_seed_value;
#[cfg(target_os = "windows")]
use crate::app::state::HotkeyState;
#[cfg(target_os = "windows")]
use crate::infra::settings::{hotkey_virtual_key, normalize_hotkey};
#[cfg(target_os = "windows")]
use crate::DEFAULT_HOTKEY;

#[cfg(target_os = "windows")]
fn trigger_copy_shortcut() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_MENU,
    };

    unsafe {
        // Global hotkey includes Alt, ensure Alt is released before sending Ctrl+C.
        keybd_event(VK_MENU as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(b'C', 0, 0, 0);
        keybd_event(b'C', 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(target_os = "windows")]
fn capture_selected_text_from_system() -> Option<String> {
    use arboard::Clipboard;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;

    let mut clipboard = Clipboard::new().ok()?;
    // Only proceed when clipboard text can be restored later.
    let backup_text = clipboard.get_text().ok()?;
    let marker = format!(
        "__name_dict_marker_{}__",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

    clipboard.set_text(marker.clone()).ok()?;
    let marker_sequence = unsafe { GetClipboardSequenceNumber() };
    thread::sleep(Duration::from_millis(35));
    trigger_copy_shortcut();

    let mut captured = String::new();
    let mut last_sequence = marker_sequence;
    for _ in 0..16 {
        thread::sleep(Duration::from_millis(15));
        let sequence = unsafe { GetClipboardSequenceNumber() };
        if sequence == last_sequence {
            continue;
        }
        last_sequence = sequence;
        if let Ok(text) = clipboard.get_text() {
            if text != marker {
                captured = text;
                break;
            }
        }
    }
    if captured.is_empty() {
        for _ in 0..8 {
            thread::sleep(Duration::from_millis(40));
            if let Ok(text) = clipboard.get_text() {
                if text != marker {
                    captured = text;
                    break;
                }
            }
        }
    }

    let _ = clipboard.set_text(backup_text);

    let cleaned = captured.trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn start_hotkey_listener<R: tauri::Runtime>(app: AppHandle<R>)
where
    AppHandle<R>: Send + 'static,
{
    std::thread::spawn(move || unsafe {
        use std::thread;
        use std::time::Duration;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            RegisterHotKey, UnregisterHotKey, MOD_ALT,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY,
        };

        const HOTKEY_ID: i32 = 1104;
        let mut current_hotkey = String::new();
        let mut is_registered = false;
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let desired_hotkey = app
                .state::<HotkeyState>()
                .0
                .lock()
                .map(|value| normalize_hotkey(value.as_str()))
                .unwrap_or_else(|_| DEFAULT_HOTKEY.to_string());

            if desired_hotkey != current_hotkey {
                if is_registered {
                    let _ = UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
                    is_registered = false;
                }

                let vk = hotkey_virtual_key(&desired_hotkey);
                if RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, MOD_ALT, vk) == 0 {
                    eprintln!(
                        "注册全局快捷键 {} 失败，可能已被其他程序占用",
                        desired_hotkey
                    );
                } else {
                    is_registered = true;
                }
                current_hotkey = desired_hotkey;
            }

            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as usize {
                    let selected = capture_selected_text_from_system().unwrap_or_default();
                    if let Err(err) = set_editor_seed_value(&app, selected.clone()) {
                        eprintln!("{err}");
                        continue;
                    }
                    if let Err(err) = app.emit_to("main", "editor-open-request", selected) {
                        eprintln!("发送快捷键编辑事件失败: {err}");
                    }
                }
            }

            thread::sleep(Duration::from_millis(20));
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_hotkey_listener<R: tauri::Runtime>(_app: AppHandle<R>) {}
