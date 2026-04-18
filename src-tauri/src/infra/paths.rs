use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;
use tauri::Manager;

use crate::infra::files::{
    collect_json_files, is_bundled_dict_order_file, is_custom_entries_file,
    load_entries_from_json_file,
};
use crate::{BUNDLED_DICT_DIR_NAME, CUSTOM_DICT_ID, DATA_FILE_NAME, SETTINGS_FILE_NAME};
const BUNDLED_SYNC_MANIFEST_NAME: &str = ".bundled-sync-manifest.json";

pub(crate) fn resolve_project_data_dir<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let project_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("获取项目数据目录失败: {err}"))?;
    fs::create_dir_all(&project_data_dir).map_err(|err| format!("创建项目数据目录失败: {err}"))?;
    Ok(project_data_dir)
}

pub(crate) fn resolve_settings_file_path<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    Ok(resolve_project_data_dir(app)?.join(SETTINGS_FILE_NAME))
}

pub(crate) fn resolve_entries_file_path(dict_dir: &Path) -> PathBuf {
    dict_dir.join(DATA_FILE_NAME)
}

pub(crate) fn normalize_dict_dir(input: &str, project_data_dir: &Path) -> PathBuf {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        project_data_dir.to_path_buf()
    } else {
        let candidate = PathBuf::from(trimmed);
        if candidate.is_absolute() {
            candidate
        } else {
            project_data_dir.join(candidate)
        }
    }
}

pub(crate) fn sanitize_windows_verbatim_prefix(raw: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = raw.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    raw.to_string()
}

pub(crate) fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir()
                .map(|current| current.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        normalize_lexically(absolute.as_path())
    })
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub(crate) fn same_dir_path(left: &Path, right: &Path) -> bool {
    let normalized_left = normalize_path_for_compare(left);
    let normalized_right = normalize_path_for_compare(right);

    #[cfg(target_os = "windows")]
    {
        normalized_left
            .to_string_lossy()
            .eq_ignore_ascii_case(normalized_right.to_string_lossy().as_ref())
    }

    #[cfg(not(target_os = "windows"))]
    {
        normalized_left == normalized_right
    }
}

fn is_path_within(base: &Path, target: &Path) -> bool {
    let normalized_base = normalize_path_for_compare(base);
    let normalized_target = normalize_path_for_compare(target);

    #[cfg(target_os = "windows")]
    {
        let base = normalized_base
            .to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase();
        let target = normalized_target
            .to_string_lossy()
            .replace('/', "\\")
            .to_ascii_lowercase();
        target == base || target.starts_with(format!("{base}\\").as_str())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let base = normalized_base
            .to_string_lossy()
            .trim_end_matches('/')
            .to_string();
        let target = normalized_target.to_string_lossy().to_string();
        target == base || target.starts_with(format!("{base}/").as_str())
    }
}

pub(crate) fn validate_dict_dir_path(
    path: &Path,
    project_data_dir: &Path,
) -> Result<PathBuf, String> {
    let canonical = normalize_path_for_compare(path);
    if canonical.parent().is_none() {
        return Err("词库目录不能为文件系统根目录".to_string());
    }

    let allowed_base = normalize_path_for_compare(project_data_dir);
    if !is_path_within(&allowed_base, &canonical) {
        return Err(format!(
            "词库目录必须位于项目数据目录内（{}）",
            sanitize_windows_verbatim_prefix(allowed_base.to_string_lossy().as_ref())
        ));
    }

    #[cfg(target_os = "windows")]
    {
        for key in ["WINDIR", "ProgramFiles", "ProgramFiles(x86)"] {
            let Ok(raw_base) = env::var(key) else {
                continue;
            };
            let base = normalize_path_for_compare(Path::new(raw_base.as_str()));
            if is_path_within(&base, &canonical) {
                return Err(format!(
                    "词库目录不能位于系统目录下（{}）",
                    sanitize_windows_verbatim_prefix(base.to_string_lossy().as_ref())
                ));
            }
        }
    }

    Ok(canonical)
}

pub(crate) fn resolve_bundled_dict_dir_candidates<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join(BUNDLED_DICT_DIR_NAME));
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join(BUNDLED_DICT_DIR_NAME));
        if let Some(parent) = current_dir.parent() {
            candidates.push(parent.join(BUNDLED_DICT_DIR_NAME));
        }
    }
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest_dir);
        candidates.push(manifest_dir.join(BUNDLED_DICT_DIR_NAME));
        if let Some(parent) = manifest_dir.parent() {
            candidates.push(parent.join(BUNDLED_DICT_DIR_NAME));
        }
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(BUNDLED_DICT_DIR_NAME));
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if has_non_custom_json_file(&candidate) {
            out.push(candidate);
        }
    }
    out
}

#[allow(dead_code)]
pub(crate) fn resolve_bundled_dict_dir<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    resolve_bundled_dict_dir_candidates(app).into_iter().next()
}

fn has_non_custom_json_file(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let Ok(files) = collect_json_files(path) else {
        return false;
    };
    for file in files {
        if is_custom_entries_file(&file) {
            continue;
        }
        if is_bundled_dict_order_file(&file) {
            continue;
        }
        let Ok(loaded) = load_entries_from_json_file(&file) else {
            continue;
        };
        if !loaded.entries.is_empty() {
            return true;
        }
        if loaded
            .meta
            .as_ref()
            .is_some_and(|meta| !meta.dict_id.eq_ignore_ascii_case(CUSTOM_DICT_ID))
            && has_explicit_entries_field(&file)
        {
            return true;
        }
    }
    false
}

fn has_explicit_entries_field(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text.as_str()) else {
        return false;
    };
    value
        .as_object()
        .and_then(|map| map.get("entries"))
        .is_some_and(|entries| entries.is_array())
}

pub(crate) fn sync_bundled_dict_to_install_dir<R: tauri::Runtime>(app: &AppHandle<R>) {
    let Ok(exe_path) = env::current_exe() else {
        return;
    };
    let Some(exe_dir) = exe_path.parent() else {
        return;
    };
    let target_dir = exe_dir.join(BUNDLED_DICT_DIR_NAME);

    let Ok(resource_dir) = app.path().resource_dir() else {
        return;
    };
    let source_dir = resource_dir.join(BUNDLED_DICT_DIR_NAME);
    if !source_dir.is_dir() {
        return;
    }
    if same_dir_path(&source_dir, &target_dir) {
        return;
    }
    if let Err(err) = fs::create_dir_all(&target_dir) {
        eprintln!("创建安装目录词库失败 {}: {err}", target_dir.display());
        return;
    }
    let manifest_path = target_dir.join(BUNDLED_SYNC_MANIFEST_NAME);

    let source_files = match collect_json_files(&source_dir) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("读取内置资源词库失败 {}: {err}", source_dir.display());
            return;
        }
    };

    let mut source_names = HashSet::new();
    for file in &source_files {
        if is_custom_entries_file(file) {
            continue;
        }
        if is_bundled_sync_manifest_file(file) {
            continue;
        }
        if let Some(name) = file.file_name().and_then(|value| value.to_str()) {
            source_names.insert(name.to_ascii_lowercase());
        }
    }

    let previously_synced = read_synced_manifest(&manifest_path);

    match collect_json_files(&target_dir) {
        Ok(files) => {
            let mut target_index: std::collections::HashMap<String, PathBuf> =
                std::collections::HashMap::new();
            for file in files {
                if is_custom_entries_file(&file) {
                    continue;
                }
                if is_bundled_sync_manifest_file(&file) {
                    continue;
                }
                let Some(name) = file.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                target_index.insert(name.to_ascii_lowercase(), file);
            }

            for stale_name in previously_synced {
                if source_names.contains(&stale_name) {
                    continue;
                }
                if let Some(path) = target_index.get(&stale_name) {
                    if let Err(err) = move_stale_bundled_file(path, &target_dir) {
                        eprintln!("隔离旧内置词库失败 {}: {err}", path.display());
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("读取安装目录词库失败 {}: {err}", target_dir.display());
        }
    }

    for file in source_files {
        if is_custom_entries_file(&file) {
            continue;
        }
        if is_bundled_sync_manifest_file(&file) {
            continue;
        }
        let target_file = if is_bundled_dict_order_file(&file) {
            target_dir.join(crate::BUNDLED_DICT_ORDER_FILE_NAME)
        } else {
            let Some(name) = file.file_name() else {
                continue;
            };
            target_dir.join(name)
        };
        if let Err(err) = fs::copy(&file, &target_file) {
            eprintln!(
                "复制内置词库失败 {} -> {}: {err}",
                file.display(),
                target_file.display()
            );
        }
    }

    if let Err(err) = write_synced_manifest(&manifest_path, &source_names) {
        eprintln!(
            "写入内置词库同步清单失败 {}: {err}",
            manifest_path.display()
        );
    }
}

fn is_bundled_sync_manifest_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(BUNDLED_SYNC_MANIFEST_NAME))
}

fn read_synced_manifest(path: &Path) -> HashSet<String> {
    if !path.is_file() {
        return HashSet::new();
    }
    let Ok(text) = fs::read_to_string(path) else {
        return HashSet::new();
    };
    let Ok(items) = serde_json::from_str::<Vec<String>>(&text) else {
        return HashSet::new();
    };
    items
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn write_synced_manifest(path: &Path, names: &HashSet<String>) -> Result<(), String> {
    let mut list: Vec<String> = names.iter().cloned().collect();
    list.sort();
    let payload =
        serde_json::to_string_pretty(&list).map_err(|err| format!("序列化清单失败: {err}"))?;
    fs::write(path, payload).map_err(|err| format!("写入清单失败: {err}"))
}

fn move_stale_bundled_file(path: &Path, target_dir: &Path) -> Result<(), String> {
    let stale_dir = target_dir.join(".bundled-stale");
    fs::create_dir_all(&stale_dir).map_err(|err| format!("创建隔离目录失败: {err}"))?;

    let fallback_name = format!(
        "stale-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or(fallback_name);

    let mut target_path = stale_dir.join(file_name);
    if target_path.exists() {
        let stem = target_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("stale");
        let ext = target_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("json");
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        target_path = stale_dir.join(format!("{stem}-{suffix}.{ext}"));
    }

    fs::rename(path, &target_path).map_err(|err| format!("移动文件失败: {err}"))
}
