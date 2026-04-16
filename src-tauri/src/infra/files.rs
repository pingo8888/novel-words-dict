use std::fs;
use std::path::{Path, PathBuf};

use crate::core::types::{DictionaryMeta, LoadedJsonData, NameEntry};
use crate::DATA_FILE_NAME;

pub(crate) fn collect_json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
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

pub(crate) fn load_entries_from_json_file(path: &Path) -> Result<LoadedJsonData, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取文件失败: {err}"))?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(LoadedJsonData {
            meta: None,
            entries: Vec::new(),
        });
    }

    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|err| format!("JSON 解析失败: {err}"))?;
    parse_entries_from_json_value(value)
}

fn parse_entries_from_json_value(value: serde_json::Value) -> Result<LoadedJsonData, String> {
    match value {
        serde_json::Value::Array(items) => {
            let mut meta = None;
            let mut out = Vec::new();
            let mut non_meta_count = 0;
            let mut invalid_count = 0;
            for (idx, item) in items.into_iter().enumerate() {
                if idx == 0 {
                    if let Some(found_meta) = parse_dict_meta(&item) {
                        meta = Some(found_meta);
                        continue;
                    }
                }
                non_meta_count += 1;
                if let Ok(entry) = serde_json::from_value::<NameEntry>(item) {
                    out.push(entry);
                } else {
                    invalid_count += 1;
                }
            }
            if non_meta_count > 0 && out.is_empty() {
                return Err("JSON 数组中未找到有效词条对象".to_string());
            }
            if invalid_count > 0 {
                eprintln!("JSON 数组中发现 {invalid_count} 条无效词条，已跳过");
            }
            Ok(LoadedJsonData { meta, entries: out })
        }
        serde_json::Value::Object(mut map) => {
            let meta = parse_dict_meta_from_object(&map);
            if let Some(entries_value) = map.remove("entries") {
                if let serde_json::Value::Array(items) = entries_value {
                    let mut out = Vec::new();
                    let item_count = items.len();
                    let mut invalid_count = 0;
                    for item in items {
                        if let Ok(entry) = serde_json::from_value::<NameEntry>(item) {
                            out.push(entry);
                        } else {
                            invalid_count += 1;
                        }
                    }
                    if item_count > 0 && out.is_empty() {
                        return Err("entries 数组中未找到有效词条对象".to_string());
                    }
                    if invalid_count > 0 {
                        eprintln!("entries 数组中发现 {invalid_count} 条无效词条，已跳过");
                    }
                    return Ok(LoadedJsonData { meta, entries: out });
                }
                return Ok(LoadedJsonData {
                    meta,
                    entries: Vec::new(),
                });
            }
            if let Ok(entry) = serde_json::from_value::<NameEntry>(serde_json::Value::Object(map)) {
                return Ok(LoadedJsonData {
                    meta: None,
                    entries: vec![entry],
                });
            }
            Ok(LoadedJsonData {
                meta,
                entries: Vec::new(),
            })
        }
        _ => Ok(LoadedJsonData {
            meta: None,
            entries: Vec::new(),
        }),
    }
}

fn parse_dict_meta(value: &serde_json::Value) -> Option<DictionaryMeta> {
    let serde_json::Value::Object(map) = value else {
        return None;
    };
    parse_dict_meta_from_object(map)
}

fn parse_dict_meta_from_object(map: &serde_json::Map<String, serde_json::Value>) -> Option<DictionaryMeta> {
    let dict_id = map
        .get("dictId")
        .or_else(|| map.get("dict_id"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let dict_name = map
        .get("dictName")
        .or_else(|| map.get("dict_name"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let order = map
        .get("order")
        .and_then(|value| value.as_i64())
        .and_then(|value| i32::try_from(value).ok());

    let id = sanitize_dict_id(dict_id);
    let name = dict_name.trim().to_string();
    if id.is_empty() || name.is_empty() {
        return None;
    }
    Some(DictionaryMeta {
        dict_id: id,
        dict_name: name,
        order,
    })
}

pub(crate) fn sanitize_dict_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        }
    }
    out
}

pub(crate) fn is_custom_entries_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(DATA_FILE_NAME))
}

pub(crate) fn load_entries_from_ndjson_file(path: &Path) -> Result<Vec<NameEntry>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取旧数据文件失败: {err}"))?;
    let mut out = Vec::new();
    let mut invalid_count = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<NameEntry>(trimmed) {
            out.push(entry);
        } else {
            invalid_count += 1;
        }
    }
    if invalid_count > 0 {
        eprintln!(
            "旧数据文件 {} 中发现 {invalid_count} 条无效词条，已跳过",
            path.display()
        );
    }
    Ok(out)
}

pub(crate) fn replace_file_from_temp(temp_path: &Path, target_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };

        let mut from_wide: Vec<u16> = temp_path.as_os_str().encode_wide().collect();
        from_wide.push(0);
        let mut to_wide: Vec<u16> = target_path.as_os_str().encode_wide().collect();
        to_wide.push(0);

        let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
        let moved = unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), flags) };
        if moved == 0 {
            return Err(format!(
                "替换目标文件失败 {} -> {}: {}",
                temp_path.display(),
                target_path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(temp_path, target_path).map_err(|err| {
            format!(
                "替换目标文件失败 {} -> {}: {err}",
                temp_path.display(),
                target_path.display()
            )
        })?;
        Ok(())
    }
}
