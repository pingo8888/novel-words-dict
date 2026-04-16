use tauri::AppHandle;

#[cfg(target_os = "windows")]
use tauri::Emitter;
#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(target_os = "windows")]
use crate::app::commands::set_editor_seed_value;
#[cfg(target_os = "windows")]
use crate::app::state::{HotkeyShutdown, HotkeyState};
#[cfg(target_os = "windows")]
use crate::infra::settings::hotkey_virtual_key;
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
    // Prefer capture success even when clipboard is not plain text.
    let backup_text = clipboard.get_text().ok();
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

    let cleaned = captured.trim().to_string();
    if let Some(text) = backup_text {
        let _ = clipboard.set_text(text);
    } else if cleaned.is_empty() {
        let should_clear_marker = clipboard
            .get_text()
            .map(|text| text == marker)
            .unwrap_or(false);
        if should_clear_marker {
            let _ = clipboard.set_text(String::new());
        }
    }

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
    let shutdown = app.state::<HotkeyShutdown>().0.clone();
    std::thread::spawn(move || {
        use std::sync::atomic::Ordering;
        use std::thread;
        use std::time::Duration;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            RegisterHotKey, UnregisterHotKey, MOD_ALT,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            PeekMessageW, MSG, PM_NOREMOVE, PM_REMOVE, WM_HOTKEY,
        };

        const HOTKEY_ID: i32 = 1104;
        let mut current_hotkey = String::new();
        let mut is_registered = false;
        let mut refresh_counter: u8 = 0;
        let mut msg: MSG = unsafe { std::mem::zeroed() };

        // Ensure current thread has a message queue before registering hotkey.
        let _ = unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };

        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            if refresh_counter == 0 {
                let desired_hotkey = app
                    .state::<HotkeyState>()
                    .0
                    .lock()
                    .map(|value| value.clone())
                    .unwrap_or_else(|_| {
                        eprintln!("读取快捷键状态失败：状态锁已中毒（poisoned）");
                        DEFAULT_HOTKEY.to_string()
                    });

                if desired_hotkey != current_hotkey {
                    if is_registered {
                        let _ = unsafe { UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID) };
                        is_registered = false;
                    }

                    let vk = hotkey_virtual_key(&desired_hotkey);
                    if unsafe { RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, MOD_ALT, vk) } == 0
                    {
                        eprintln!(
                            "注册全局快捷键 {} 失败，可能已被其他程序占用",
                            desired_hotkey
                        );
                    } else {
                        is_registered = true;
                    }
                    current_hotkey = desired_hotkey;
                }
                refresh_counter = 10;
            } else {
                refresh_counter -= 1;
            }

            while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
                if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as usize {
                    let selected = capture_selected_text_from_system().unwrap_or_default();
                    if let Err(err) = set_editor_seed_value(&app, &selected) {
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

        if is_registered {
            let _ = unsafe { UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID) };
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_hotkey_listener<R: tauri::Runtime>(_app: AppHandle<R>) {}
