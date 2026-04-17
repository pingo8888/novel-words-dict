use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::infra::settings::AppSettings;
use crate::store::EntryStore;

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) store: Mutex<EntryStore>,
}

#[derive(Default)]
pub(crate) struct SettingsState(pub(crate) Mutex<Option<AppSettings>>);

#[derive(Default)]
pub(crate) struct EditorSeed(pub(crate) Mutex<Option<String>>);

#[derive(Default)]
pub(crate) struct HotkeyState(pub(crate) Mutex<String>);

#[derive(Default, Clone)]
pub(crate) struct HotkeyShutdown(pub(crate) Arc<AtomicBool>);

impl HotkeyShutdown {
    pub(crate) fn request_shutdown(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub(crate) struct HotkeyEnabled(pub(crate) Arc<AtomicBool>);

impl Default for HotkeyEnabled {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}

impl HotkeyEnabled {
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.0.store(enabled, Ordering::Relaxed);
    }
}
