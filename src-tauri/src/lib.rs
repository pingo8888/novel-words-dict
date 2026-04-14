use pinyin::ToPinyin;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;
use tauri::{AppHandle, State};

const DATA_FILE_NAME: &str = "entries.json";
const LEGACY_DATA_FILE_NAME: &str = "entries.ndjson";
const SETTINGS_FILE_NAME: &str = "settings.json";
const DEFAULT_HOTKEY: &str = "Alt+Z";
const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum NameType {
    Both,
    Surname,
    Given,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum GenderType {
    Both,
    Male,
    Female,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NameEntry {
    term: String,
    group: String,
    name_type: NameType,
    gender_type: GenderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    dict_dir: String,
    hotkey: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsPatch {
    dict_dir: Option<String>,
    hotkey: Option<String>,
}

#[derive(Default)]
struct EntryStore {
    entries: Vec<NameEntry>,
    index: HashMap<String, usize>,
    data_path: Option<PathBuf>,
}

impl EntryStore {
    fn load(&mut self, path: PathBuf) -> Result<(), String> {
        let data_dir = path
            .parent()
            .ok_or_else(|| "数据目录路径无效".to_string())?
            .to_path_buf();
        fs::create_dir_all(&data_dir).map_err(|err| format!("创建数据目录失败: {err}"))?;

        let mut latest: HashMap<String, NameEntry> = HashMap::new();
        let mut loaded_json = false;

        let mut json_files = collect_json_files(&data_dir)?;
        json_files.sort();
        json_files.retain(|candidate| candidate != &path);
        json_files.push(path.clone());

        for file_path in json_files {
            if !file_path.exists() {
                continue;
            }
            match load_entries_from_json_file(&file_path) {
                Ok(entries) => {
                    loaded_json = true;
                    for mut entry in entries {
                        entry.term = entry.term.trim().to_string();
                        if entry.term.is_empty() {
                            continue;
                        }
                        latest.insert(make_term_key(&entry.term), entry);
                    }
                }
                Err(err) => {
                    eprintln!("忽略无效 JSON 文件 {}: {err}", file_path.display());
                }
            }
        }

        if !loaded_json {
            let legacy_path = data_dir
                .parent()
                .map(|dir| dir.join(LEGACY_DATA_FILE_NAME))
                .unwrap_or_else(|| data_dir.join(LEGACY_DATA_FILE_NAME));
            if legacy_path.exists() {
                match load_entries_from_ndjson_file(&legacy_path) {
                    Ok(entries) => {
                        for mut entry in entries {
                            entry.term = entry.term.trim().to_string();
                            if entry.term.is_empty() {
                                continue;
                            }
                            latest.insert(make_term_key(&entry.term), entry);
                        }
                    }
                    Err(err) => {
                        eprintln!("读取旧数据文件失败 {}: {err}", legacy_path.display());
                    }
                }
            }
        }

        self.entries = latest.into_values().collect();
        self.sort_entries();
        self.rebuild_index();
        self.data_path = Some(path.clone());

        if !path.exists() {
            self.persist()?;
        }
        Ok(())
    }

    fn query(&self, request: &QueryRequest) -> QueryResponse {
        let initial = request
            .initial
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_uppercase();
        let name_type = request
            .name_type
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_lowercase();
        let gender_type = request
            .gender_type
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_lowercase();
        let keyword = request
            .keyword
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();

        let mut matched = self
            .entries
            .iter()
            .filter(|entry| matches_initial_filter(&initial, &entry.term))
            .filter(|entry| matches_name_type_filter(&name_type, entry.name_type))
            .filter(|entry| matches_gender_type_filter(&gender_type, entry.gender_type))
            .filter(|entry| matches_keyword_filter(&keyword, entry))
            .cloned()
            .collect::<Vec<_>>();

        matched.sort_by(|a, b| compare_terms(&a.term, &b.term));

        let total = matched.len();
        let page_count = if total == 0 {
            1
        } else {
            total.div_ceil(PAGE_SIZE)
        };
        let page = request.page.unwrap_or(1).max(1).min(page_count);
        let start = (page - 1) * PAGE_SIZE;
        let items = matched.into_iter().skip(start).take(PAGE_SIZE).collect();

        QueryResponse {
            items,
            total,
            total_all: self.entries.len(),
            page,
            page_count,
        }
    }

    fn get_entry(&self, term: &str) -> Option<NameEntry> {
        let key = make_term_key(term);
        self.index
            .get(&key)
            .and_then(|idx| self.entries.get(*idx))
            .cloned()
    }

    fn upsert(&mut self, mut entry: NameEntry) -> Result<(), String> {
        entry.term = entry.term.trim().to_string();
        entry.group = entry.group.trim().to_string();
        if entry.term.is_empty() {
            return Err("词条不能为空".to_string());
        }

        let key = make_term_key(&entry.term);
        if let Some(existing_idx) = self.index.get(&key).copied() {
            self.entries[existing_idx] = entry;
        } else {
            self.entries.push(entry);
        }

        self.sort_entries();
        self.rebuild_index();
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let path = self
            .data_path
            .as_ref()
            .ok_or_else(|| "数据文件路径未初始化".to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("创建数据目录失败: {err}"))?;
        }
        let out = if self.entries.is_empty() {
            "[]".to_string()
        } else {
            let mut lines = Vec::with_capacity(self.entries.len());
            for entry in &self.entries {
                let line =
                    serde_json::to_string(entry).map_err(|err| format!("序列化词条失败: {err}"))?;
                lines.push(line);
            }
            format!("[\n{}\n]", lines.join(",\n"))
        };

        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, out).map_err(|err| format!("写入临时文件失败: {err}"))?;

        if path.exists() {
            fs::remove_file(path).map_err(|err| format!("替换旧数据文件失败: {err}"))?;
        }
        fs::rename(&temp_path, path).map_err(|err| format!("落盘数据文件失败: {err}"))?;
        Ok(())
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| compare_terms(&a.term, &b.term));
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            self.index.insert(make_term_key(&entry.term), idx);
        }
    }
}

fn collect_json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let read_dir = fs::read_dir(dir).map_err(|err| format!("读取数据目录失败: {err}"))?;
    let mut files = Vec::new();
    for item in read_dir {
        let path = item
            .map_err(|err| format!("读取数据目录项失败: {err}"))?
            .path();
        if !path.is_file() {
            continue;
        }
        let is_json = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        if is_json {
            files.push(path);
        }
    }
    Ok(files)
}

fn load_entries_from_json_file(path: &Path) -> Result<Vec<NameEntry>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取文件失败: {err}"))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|err| format!("JSON 解析失败: {err}"))?;
    parse_entries_from_json_value(value)
}

fn parse_entries_from_json_value(value: serde_json::Value) -> Result<Vec<NameEntry>, String> {
    match value {
        serde_json::Value::Array(items) => {
            let mut out = Vec::new();
            let item_count = items.len();
            for item in items {
                if let Ok(entry) = serde_json::from_value::<NameEntry>(item) {
                    out.push(entry);
                }
            }
            if item_count > 0 && out.is_empty() {
                return Err("JSON 数组中未找到有效词条对象".to_string());
            }
            Ok(out)
        }
        serde_json::Value::Object(map) => {
            if let Ok(entry) =
                serde_json::from_value::<NameEntry>(serde_json::Value::Object(map.clone()))
            {
                return Ok(vec![entry]);
            }
            if let Some(entries_value) = map.get("entries") {
                if let serde_json::Value::Array(items) = entries_value {
                    let mut out = Vec::new();
                    let item_count = items.len();
                    for item in items {
                        if let Ok(entry) = serde_json::from_value::<NameEntry>(item.clone()) {
                            out.push(entry);
                        }
                    }
                    if item_count > 0 && out.is_empty() {
                        return Err("entries 数组中未找到有效词条对象".to_string());
                    }
                    return Ok(out);
                }
                return Ok(Vec::new());
            }
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}

fn load_entries_from_ndjson_file(path: &Path) -> Result<Vec<NameEntry>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取旧数据文件失败: {err}"))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<NameEntry>(trimmed) {
            out.push(entry);
        }
    }
    Ok(out)
}

#[derive(Default)]
struct AppState {
    store: Mutex<EntryStore>,
}

#[derive(Default)]
struct SettingsState(Mutex<Option<AppSettings>>);

#[derive(Default)]
struct EditorSeed(Mutex<Option<String>>);

#[derive(Default)]
struct HotkeyState(Mutex<String>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryRequest {
    initial: Option<String>,
    name_type: Option<String>,
    gender_type: Option<String>,
    keyword: Option<String>,
    page: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryResponse {
    items: Vec<NameEntry>,
    total: usize,
    total_all: usize,
    page: usize,
    page_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenEditorRequest {
    term: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveSettingsRequest {
    dict_dir: String,
    hotkey: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsResponse {
    dict_dir: String,
    hotkey: String,
    project_data_dir: String,
}

#[tauri::command]
fn query_entries(state: State<AppState>, request: QueryRequest) -> Result<QueryResponse, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取数据失败：状态锁不可用".to_string())?;
    Ok(store.query(&request))
}

#[tauri::command]
fn get_entry(state: State<AppState>, term: String) -> Result<Option<NameEntry>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "读取词条失败：状态锁不可用".to_string())?;
    Ok(store.get_entry(&term))
}

#[tauri::command]
fn upsert_entry(app: AppHandle, state: State<AppState>, entry: NameEntry) -> Result<(), String> {
    let term_for_event = entry.term.trim().to_string();
    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "保存词条失败：状态锁不可用".to_string())?;
        store.upsert(entry)?;
    }

    let _ = app.emit_to("main", "entry-updated", term_for_event);
    Ok(())
}

#[tauri::command]
fn open_editor_window(app: AppHandle, request: OpenEditorRequest) -> Result<(), String> {
    let term = request.term.unwrap_or_default();
    set_editor_seed_value(&app, term.clone())?;
    app.emit_to("main", "editor-open-request", term)
        .map_err(|err| format!("发送编辑窗口事件失败: {err}"))?;
    Ok(())
}

#[tauri::command]
fn take_editor_seed(editor_seed: State<EditorSeed>) -> Option<String> {
    if let Ok(mut guard) = editor_seed.0.lock() {
        guard.take()
    } else {
        None
    }
}

#[tauri::command]
fn close_editor_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("editor") {
        window
            .close()
            .map_err(|err| format!("关闭编辑窗口失败: {err}"))?;
    }
    Ok(())
}

#[tauri::command]
fn set_editor_seed(app: AppHandle, term: String) -> Result<(), String> {
    set_editor_seed_value(&app, term)
}

#[tauri::command]
fn get_app_settings(
    app: AppHandle,
    settings_state: State<SettingsState>,
) -> Result<SettingsResponse, String> {
    let project_data_dir = resolve_project_data_dir(&app)?;
    let settings = settings_state
        .0
        .lock()
        .map_err(|_| "读取设置失败：状态锁不可用".to_string())?
        .clone()
        .unwrap_or_else(|| default_settings(&project_data_dir));
    Ok(build_settings_response(&settings, &project_data_dir))
}

#[tauri::command]
fn save_app_settings(
    app: AppHandle,
    state: State<AppState>,
    settings_state: State<SettingsState>,
    hotkey_state: State<HotkeyState>,
    request: SaveSettingsRequest,
) -> Result<SettingsResponse, String> {
    let project_data_dir = resolve_project_data_dir(&app)?;
    let normalized_hotkey = normalize_hotkey(&request.hotkey);
    let dict_dir_path = normalize_dict_dir(&request.dict_dir, &project_data_dir);
    fs::create_dir_all(&dict_dir_path).map_err(|err| format!("创建词库目录失败: {err}"))?;

    let data_path = resolve_entries_file_path(&dict_dir_path);
    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "保存设置失败：词库状态锁不可用".to_string())?;
        store.load(data_path)?;
    }

    let normalized_settings = AppSettings {
        dict_dir: dict_dir_path.to_string_lossy().to_string(),
        hotkey: normalized_hotkey.clone(),
    };
    persist_app_settings(&app, &normalized_settings)?;

    {
        let mut settings_guard = settings_state
            .0
            .lock()
            .map_err(|_| "保存设置失败：设置状态锁不可用".to_string())?;
        *settings_guard = Some(normalized_settings.clone());
    }
    {
        let mut hotkey_guard = hotkey_state
            .0
            .lock()
            .map_err(|_| "保存设置失败：快捷键状态锁不可用".to_string())?;
        *hotkey_guard = normalized_hotkey;
    }

    let _ = app.emit_to("main", "entry-updated", String::new());
    Ok(build_settings_response(
        &normalized_settings,
        &project_data_dir,
    ))
}

fn make_term_key(term: &str) -> String {
    normalize_text(term.trim())
}

fn matches_initial_filter(initial: &str, term: &str) -> bool {
    if initial == "ALL" || initial.is_empty() {
        return true;
    }
    let Some(first) = leading_alpha_initial(term) else {
        return false;
    };
    initial.chars().next().is_some_and(|target| target == first)
}

fn matches_name_type_filter(filter: &str, value: NameType) -> bool {
    match filter {
        "all" => true,
        "surname" => value == NameType::Surname || value == NameType::Both,
        "given" => value == NameType::Given || value == NameType::Both,
        "both" => value == NameType::Both,
        _ => true,
    }
}

fn matches_gender_type_filter(filter: &str, value: GenderType) -> bool {
    match filter {
        "all" => true,
        "male" => value == GenderType::Male || value == GenderType::Both,
        "female" => value == GenderType::Female || value == GenderType::Both,
        "both" => value == GenderType::Both,
        _ => true,
    }
}

fn matches_keyword_filter(keyword: &str, entry: &NameEntry) -> bool {
    if keyword.is_empty() {
        return true;
    }
    normalize_text(&entry.term).contains(keyword) || normalize_text(&entry.group).contains(keyword)
}

fn normalize_text(value: &str) -> String {
    value.chars().flat_map(|ch| ch.to_lowercase()).collect()
}

fn leading_alpha_initial(term: &str) -> Option<char> {
    for ch in term.trim().chars() {
        if ch.is_ascii_alphabetic() {
            return Some(ch.to_ascii_uppercase());
        }
        if let Some(pinyin) = ch.to_pinyin() {
            let initial = pinyin
                .plain()
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase());
            if initial.is_some() {
                return initial;
            }
        }
    }
    None
}

fn pinyin_sort_key(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii() {
            out.extend(ch.to_lowercase());
            continue;
        }
        if let Some(pinyin) = ch.to_pinyin() {
            out.push_str(pinyin.plain());
            continue;
        }
        out.extend(ch.to_lowercase());
    }
    out
}

fn compare_terms(left: &str, right: &str) -> Ordering {
    let left_initial = leading_alpha_initial(left);
    let right_initial = leading_alpha_initial(right);
    let left_bucket = if left_initial.is_some() { 0_u8 } else { 1_u8 };
    let right_bucket = if right_initial.is_some() { 0_u8 } else { 1_u8 };

    left_bucket
        .cmp(&right_bucket)
        .then_with(|| left_initial.cmp(&right_initial))
        .then_with(|| pinyin_sort_key(left).cmp(&pinyin_sort_key(right)))
        .then_with(|| left.cmp(right))
}

fn resolve_project_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let project_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("获取项目数据目录失败: {err}"))?;
    fs::create_dir_all(&project_data_dir).map_err(|err| format!("创建项目数据目录失败: {err}"))?;
    Ok(project_data_dir)
}

fn resolve_settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_project_data_dir(app)?.join(SETTINGS_FILE_NAME))
}

fn resolve_entries_file_path(dict_dir: &Path) -> PathBuf {
    dict_dir.join(DATA_FILE_NAME)
}

fn normalize_hotkey(input: &str) -> String {
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

fn hotkey_virtual_key(hotkey: &str) -> u32 {
    let normalized = normalize_hotkey(hotkey);
    normalized
        .chars()
        .last()
        .filter(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch as u32)
        .unwrap_or('Z' as u32)
}

fn default_settings(project_data_dir: &Path) -> AppSettings {
    AppSettings {
        dict_dir: project_data_dir.to_string_lossy().to_string(),
        hotkey: DEFAULT_HOTKEY.to_string(),
    }
}

fn normalize_dict_dir(input: &str, project_data_dir: &Path) -> PathBuf {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        project_data_dir.to_path_buf()
    } else {
        PathBuf::from(trimmed)
    }
}

fn load_app_settings(app: &AppHandle) -> Result<AppSettings, String> {
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

fn persist_app_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
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

fn build_settings_response(settings: &AppSettings, project_data_dir: &Path) -> SettingsResponse {
    SettingsResponse {
        dict_dir: settings.dict_dir.clone(),
        hotkey: settings.hotkey.clone(),
        project_data_dir: project_data_dir.to_string_lossy().to_string(),
    }
}

fn set_editor_seed_value(app: &AppHandle, term: String) -> Result<(), String> {
    let seed_state = app.state::<EditorSeed>();
    let mut guard = seed_state
        .0
        .lock()
        .map_err(|_| "写入编辑词条失败：状态锁不可用".to_string())?;
    *guard = Some(term);
    Ok(())
}

#[cfg(desktop)]
fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.unminimize();
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }
}

#[cfg(desktop)]
fn setup_tray_icon<R: tauri::Runtime>(app: &mut tauri::App<R>) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show_item = MenuItem::with_id(app, "tray_show", "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "tray_quit", "退出程序", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_show" => show_main_window(app),
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(default_icon) = app.default_window_icon() {
        builder = builder.icon(default_icon.clone());
    }

    let _ = builder.build(app)?;
    Ok(())
}

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
    thread::sleep(Duration::from_millis(120));
    trigger_copy_shortcut();

    let mut captured = String::new();
    for _ in 0..12 {
        thread::sleep(Duration::from_millis(50));
        if let Ok(text) = clipboard.get_text() {
            if text != marker {
                captured = text;
                break;
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
fn start_hotkey_listener(app: AppHandle) {
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
                if RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, MOD_ALT as u32, vk) == 0 {
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

            thread::sleep(Duration::from_millis(80));
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn start_hotkey_listener(_app: AppHandle) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if matches!(
                event,
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Focused(false)
            ) && window.is_minimized().unwrap_or(false)
            {
                let _ = window.hide();
            }
        })
        .manage(AppState::default())
        .manage(SettingsState::default())
        .manage(EditorSeed::default())
        .manage(HotkeyState(Mutex::new(DEFAULT_HOTKEY.to_string())))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle();
            let loaded_settings = load_app_settings(&app_handle)?;
            let dict_dir = PathBuf::from(loaded_settings.dict_dir.as_str());
            fs::create_dir_all(&dict_dir).map_err(|err| format!("创建词库目录失败: {err}"))?;
            let data_path = resolve_entries_file_path(&dict_dir);
            let app_state = app.state::<AppState>();
            if let Ok(mut store) = app_state.store.lock() {
                if let Err(err) = store.load(data_path) {
                    return Err(std::io::Error::other(err).into());
                }
            } else {
                return Err(std::io::Error::other("状态锁不可用").into());
            }

            let settings_state = app.state::<SettingsState>();
            if let Ok(mut guard) = settings_state.0.lock() {
                *guard = Some(loaded_settings.clone());
            } else {
                return Err(std::io::Error::other("设置状态锁不可用").into());
            }

            let hotkey_state = app.state::<HotkeyState>();
            if let Ok(mut guard) = hotkey_state.0.lock() {
                *guard = normalize_hotkey(&loaded_settings.hotkey);
            } else {
                return Err(std::io::Error::other("快捷键状态锁不可用").into());
            }

            if let Err(err) = persist_app_settings(&app_handle, &loaded_settings) {
                eprintln!("{err}");
            }

            start_hotkey_listener(app.handle().clone());

            #[cfg(desktop)]
            setup_tray_icon(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            query_entries,
            get_entry,
            upsert_entry,
            get_app_settings,
            save_app_settings,
            open_editor_window,
            take_editor_seed,
            close_editor_window,
            set_editor_seed
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
