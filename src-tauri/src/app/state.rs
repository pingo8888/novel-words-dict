use std::sync::Mutex;

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
