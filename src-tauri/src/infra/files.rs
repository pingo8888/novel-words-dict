use std::fs;
use std::path::{Path, PathBuf};

use crate::core::types::{DictionaryMeta, LoadedJsonData, NameEntry};
use crate::{CUSTOM_DICT_ID, DATA_FILE_NAME};

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
                }
            }
            if non_meta_count > 0 && out.is_empty() {
                return Err("JSON 数组中未找到有效词条对象".to_string());
            }
            Ok(LoadedJsonData { meta, entries: out })
        }
        serde_json::Value::Object(map) => {
            let meta = parse_dict_meta(&serde_json::Value::Object(map.clone()));
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
    let Ok(meta) = serde_json::from_value::<DictionaryMeta>(value.clone()) else {
        return None;
    };
    let id = sanitize_dict_id(meta.dict_id.as_str());
    let name = meta.dict_name.trim().to_string();
    if id.is_empty() || name.is_empty() {
        return None;
    }
    Some(DictionaryMeta {
        dict_id: id,
        dict_name: name,
        order: meta.order,
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

pub(crate) fn has_non_custom_dict_meta(meta: &DictionaryMeta) -> bool {
    !meta.dict_id.eq_ignore_ascii_case(CUSTOM_DICT_ID)
}
