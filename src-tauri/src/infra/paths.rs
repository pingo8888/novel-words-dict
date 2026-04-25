use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use tauri::AppHandle;
use tauri::Manager;

use crate::infra::files::{
    collect_json_files, is_bundled_dict_order_file, is_custom_entries_file,
    load_entries_from_json_file,
};
use crate::{
    BUILTIN_DB_FILE_NAME, BUNDLED_DICT_DIR_NAME, CUSTOM_DB_FILE_NAME, CUSTOM_DICT_ID,
    DATA_FILE_NAME, SETTINGS_FILE_NAME,
};

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

pub(crate) fn resolve_custom_db_path(project_data_dir: &Path) -> PathBuf {
    project_data_dir.join(CUSTOM_DB_FILE_NAME)
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

pub(crate) fn resolve_bundled_db_path_candidates<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join(BUILTIN_DB_FILE_NAME));
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join(BUILTIN_DB_FILE_NAME));
        if let Some(parent) = current_dir.parent() {
            candidates.push(parent.join(BUILTIN_DB_FILE_NAME));
        }
    }
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        let manifest_dir = PathBuf::from(manifest_dir);
        candidates.push(manifest_dir.join(BUILTIN_DB_FILE_NAME));
        if let Some(parent) = manifest_dir.parent() {
            candidates.push(parent.join(BUILTIN_DB_FILE_NAME));
        }
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(BUILTIN_DB_FILE_NAME));
    }

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if candidate.is_file() {
            out.push(candidate);
        }
    }
    out
}

pub(crate) fn resolve_bundled_db_path<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    resolve_bundled_db_path_candidates(app).into_iter().next()
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

pub(crate) fn sync_bundled_db_to_install_dir<R: tauri::Runtime>(app: &AppHandle<R>) {
    let Ok(exe_path) = env::current_exe() else {
        return;
    };
    let Some(exe_dir) = exe_path.parent() else {
        return;
    };
    let target_file = exe_dir.join(BUILTIN_DB_FILE_NAME);
    if target_file.is_file() {
        return;
    }

    let Ok(resource_dir) = app.path().resource_dir() else {
        return;
    };
    let source_file = resource_dir.join(BUILTIN_DB_FILE_NAME);
    if !source_file.is_file() {
        return;
    }

    if normalize_path_for_compare(&source_file) == normalize_path_for_compare(&target_file) {
        return;
    }

    if let Err(err) = fs::copy(&source_file, &target_file) {
        eprintln!(
            "同步内置词库数据库失败 {} -> {}: {err}",
            source_file.display(),
            target_file.display()
        );
    }
}
