use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri::Manager;

use crate::infra::files::{
    collect_json_files, has_non_custom_dict_meta, is_custom_entries_file, load_entries_from_json_file,
};
use crate::{BUNDLED_DICT_DIR_NAME, DATA_FILE_NAME, SETTINGS_FILE_NAME};

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
        PathBuf::from(trimmed)
    }
}

pub(crate) fn resolve_bundled_dict_dir<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
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
    candidates
        .into_iter()
        .find(|path| is_valid_bundled_dict_dir(path))
}

fn is_valid_bundled_dict_dir(path: &Path) -> bool {
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
        if let Ok(loaded) = load_entries_from_json_file(&file) {
            if !loaded.entries.is_empty() {
                return true;
            }
            if let Some(meta) = loaded.meta {
                if has_non_custom_dict_meta(&meta) {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn sync_bundled_dict_to_install_dir<R: tauri::Runtime>(app: &AppHandle<R>) {
    let Ok(exe_path) = env::current_exe() else {
        return;
    };
    let Some(exe_dir) = exe_path.parent() else {
        return;
    };
    let target_dir = exe_dir.join(BUNDLED_DICT_DIR_NAME);
    if target_dir.is_dir() {
        return;
    }

    let Ok(resource_dir) = app.path().resource_dir() else {
        return;
    };
    let source_dir = resource_dir.join(BUNDLED_DICT_DIR_NAME);
    if !source_dir.is_dir() {
        return;
    }
    if let Err(err) = fs::create_dir_all(&target_dir) {
        eprintln!("创建安装目录词库失败 {}: {err}", target_dir.display());
        return;
    }

    match collect_json_files(&source_dir) {
        Ok(files) => {
            for file in files {
                let Some(name) = file.file_name() else {
                    continue;
                };
                let target_file = target_dir.join(name);
                if let Err(err) = fs::copy(&file, &target_file) {
                    eprintln!(
                        "复制内置词库失败 {} -> {}: {err}",
                        file.display(),
                        target_file.display()
                    );
                }
            }
        }
        Err(err) => {
            eprintln!("读取内置资源词库失败 {}: {err}", source_dir.display());
        }
    }
}
