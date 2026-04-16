use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use tauri::AppHandle;

use crate::core::filter::{
    matches_gender_type_filter, matches_genre_filter, matches_name_type_filter, matches_query_text,
};
use crate::core::sort::compare_terms;
use crate::core::text::make_term_key;
use crate::core::types::{DictionaryMeta, DictionaryOption, NameEntry};
use crate::infra::files::{
    collect_json_files, is_custom_entries_file, load_entries_from_json_file, load_entries_from_ndjson_file,
    sanitize_dict_id,
};
use crate::infra::paths::resolve_bundled_dict_dir;
use crate::{
    ALL_DICT_ID, ALL_DICT_NAME, CUSTOM_DICT_ID, CUSTOM_DICT_NAME, LEGACY_DATA_FILE_NAME, PAGE_SIZE,
};

use super::dictionary::DictionaryData;
use super::query::{QueryItem, QueryRequest, QueryResponse};

#[derive(Default)]
pub(crate) struct EntryStore {
    pub(crate) custom: DictionaryData,
    pub(crate) bundled: Vec<DictionaryData>,
    pub(crate) custom_data_path: Option<PathBuf>,
}

impl EntryStore {
    pub(crate) fn load<R: tauri::Runtime>(
        &mut self,
        app: &AppHandle<R>,
        path: PathBuf,
    ) -> Result<(), String> {
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
                Ok(loaded) => {
                    loaded_json = true;
                    for mut entry in loaded.entries {
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

        self.custom = DictionaryData::new(
            CUSTOM_DICT_ID.to_string(),
            CUSTOM_DICT_NAME.to_string(),
            true,
            latest.into_values().collect(),
        );
        self.bundled = self.load_bundled_dictionaries(app);
        self.custom_data_path = Some(path.clone());

        if !path.exists() {
            self.persist()?;
        }
        Ok(())
    }

    pub(crate) fn query(&self, request: &QueryRequest) -> QueryResponse {
        let dict_filter = request
            .dict_id
            .as_deref()
            .unwrap_or(ALL_DICT_ID)
            .trim()
            .to_ascii_lowercase();
        let genre_type = request
            .genre_type
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_lowercase();
        let name_type = request
            .name_type
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_lowercase();
        let mut gender_type = request
            .gender_type
            .as_deref()
            .unwrap_or("all")
            .trim()
            .to_ascii_lowercase();
        if name_type != "surname" && name_type != "given" {
            gender_type = "all".to_string();
        }
        let keyword = request
            .keyword
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();

        let mut matched = self
            .collect_query_items(dict_filter.as_str())
            .into_iter()
            .filter(|entry| matches_genre_filter(&genre_type, entry.genre))
            .filter(|entry| matches_name_type_filter(&name_type, entry.name_type))
            .filter(|entry| matches_gender_type_filter(&gender_type, entry.gender_type))
            .filter(|entry| matches_query_text(&keyword, &entry.term, &entry.group))
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
            total_all: self.total_entries_merged_all(),
            page,
            page_count,
        }
    }

    pub(crate) fn get_entry(&self, term: &str) -> Option<NameEntry> {
        let key = make_term_key(term);
        self.custom
            .index
            .get(&key)
            .and_then(|idx| self.custom.entries.get(*idx))
            .cloned()
    }

    pub(crate) fn get_bundled_entry_dict_name(&self, term: &str) -> Option<String> {
        let key = make_term_key(term);
        if key.is_empty() {
            return None;
        }
        self.bundled
            .iter()
            .find(|dict| dict.index.contains_key(&key))
            .map(|dict| dict.name.clone())
    }

    pub(crate) fn upsert(&mut self, mut entry: NameEntry) -> Result<(), String> {
        entry.term = entry.term.trim().to_string();
        entry.group = entry.group.trim().to_string();
        if entry.term.is_empty() {
            return Err("词条不能为空".to_string());
        }

        let key = make_term_key(&entry.term);
        if let Some(existing_idx) = self.custom.index.get(&key).copied() {
            self.custom.entries[existing_idx] = entry;
        } else {
            self.custom.entries.push(entry);
        }

        self.sort_custom_entries();
        self.rebuild_custom_index();
        self.persist()
    }

    pub(crate) fn delete(&mut self, term: &str) -> Result<(), String> {
        let key = make_term_key(term);
        let Some(existing_idx) = self.custom.index.get(&key).copied() else {
            return Err("词条不存在".to_string());
        };

        self.custom.entries.remove(existing_idx);
        self.sort_custom_entries();
        self.rebuild_custom_index();
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let path = self
            .custom_data_path
            .as_ref()
            .ok_or_else(|| "数据文件路径未初始化".to_string())?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("创建数据目录失败: {err}"))?;
        }
        let mut lines = Vec::with_capacity(self.custom.entries.len() + 1);
        let header = DictionaryMeta {
            dict_id: CUSTOM_DICT_ID.to_string(),
            dict_name: CUSTOM_DICT_NAME.to_string(),
            order: None,
        };
        lines.push(
            serde_json::to_string(&header).map_err(|err| format!("序列化词库元数据失败: {err}"))?,
        );
        for entry in &self.custom.entries {
            let line =
                serde_json::to_string(entry).map_err(|err| format!("序列化词条失败: {err}"))?;
            lines.push(line);
        }
        let out = format!("[\n{}\n]", lines.join(",\n"));

        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, out).map_err(|err| format!("写入临时文件失败: {err}"))?;

        if path.exists() {
            fs::remove_file(path).map_err(|err| format!("替换旧数据文件失败: {err}"))?;
        }
        fs::rename(&temp_path, path).map_err(|err| format!("落盘数据文件失败: {err}"))?;
        Ok(())
    }

    fn sort_custom_entries(&mut self) {
        self.custom
            .entries
            .sort_by(|a, b| compare_terms(&a.term, &b.term));
    }

    fn rebuild_custom_index(&mut self) {
        self.custom.index.clear();
        for (idx, entry) in self.custom.entries.iter().enumerate() {
            self.custom.index.insert(make_term_key(&entry.term), idx);
        }
    }

    pub(crate) fn list_dictionaries(&self) -> Vec<DictionaryOption> {
        let mut items = Vec::with_capacity(self.bundled.len() + 2);
        items.push(DictionaryOption {
            id: ALL_DICT_ID.to_string(),
            name: ALL_DICT_NAME.to_string(),
            editable: false,
        });
        items.push(DictionaryOption {
            id: self.custom.id.clone(),
            name: self.custom.name.clone(),
            editable: self.custom.editable,
        });
        for dict in &self.bundled {
            items.push(DictionaryOption {
                id: dict.id.clone(),
                name: dict.name.clone(),
                editable: dict.editable,
            });
        }
        items
    }

    fn total_entries_merged_all(&self) -> usize {
        // Keep custom entries as-is, and only skip bundled entries that
        // conflict with custom terms. Duplicates inside bundled dicts are kept.
        let mut custom_seen = HashSet::new();
        let mut total = 0_usize;

        for entry in &self.custom.entries {
            total += 1;
            custom_seen.insert(make_term_key(&entry.term));
        }

        for dict in &self.bundled {
            for entry in &dict.entries {
                let key = make_term_key(&entry.term);
                if custom_seen.contains(&key) {
                    continue;
                }
                total += 1;
            }
        }

        total
    }

    fn select_dictionary(&self, dict_id: &str) -> &DictionaryData {
        if dict_id.trim().eq_ignore_ascii_case(CUSTOM_DICT_ID) {
            return &self.custom;
        }
        self.bundled
            .iter()
            .find(|dict| dict.id.eq_ignore_ascii_case(dict_id.trim()))
            .unwrap_or(&self.custom)
    }

    fn collect_query_items(&self, dict_filter: &str) -> Vec<QueryItem> {
        if dict_filter.eq_ignore_ascii_case(ALL_DICT_ID) {
            return self.collect_query_items_all();
        }
        let selected_dict = self.select_dictionary(dict_filter);
        self.collect_query_items_from_dict(selected_dict)
    }

    fn collect_query_items_all(&self) -> Vec<QueryItem> {
        let mut out = Vec::new();
        let mut custom_seen = HashSet::new();

        for item in self.collect_query_items_from_dict(&self.custom) {
            custom_seen.insert(make_term_key(&item.term));
            out.push(item);
        }
        for dict in &self.bundled {
            for item in self.collect_query_items_from_dict(dict) {
                let key = make_term_key(&item.term);
                // Custom entries still override bundled ones.
                // Duplicates among bundled dicts are intentionally preserved.
                if custom_seen.contains(&key) {
                    continue;
                }
                out.push(item);
            }
        }
        out
    }

    fn collect_query_items_from_dict(&self, dict: &DictionaryData) -> Vec<QueryItem> {
        dict.entries
            .iter()
            .map(|entry| QueryItem {
                term: entry.term.clone(),
                group: entry.group.clone(),
                name_type: entry.name_type,
                gender_type: entry.gender_type,
                genre: entry.genre,
                dict_id: dict.id.clone(),
                dict_name: dict.name.clone(),
                editable: dict.editable,
            })
            .collect()
    }

    fn load_bundled_dictionaries<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Vec<DictionaryData> {
        struct BundledBucket {
            order: i32,
            file_index: usize,
            name: String,
            entries: Vec<NameEntry>,
        }

        let mut grouped: HashMap<String, BundledBucket> = HashMap::new();
        let Some(dict_dir) = resolve_bundled_dict_dir(app) else {
            return Vec::new();
        };
        let mut files = match collect_json_files(&dict_dir) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("读取内置词库目录失败 {}: {err}", dict_dir.display());
                return Vec::new();
            }
        };
        files.sort();
        for (file_index, file) in files.into_iter().enumerate() {
            if is_custom_entries_file(&file) {
                continue;
            }
            let loaded = match load_entries_from_json_file(&file) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("忽略无效内置词库文件 {}: {err}", file.display());
                    continue;
                }
            };
            let fallback_id = file
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("bundled")
                .trim()
                .to_string();
            let declared_order = loaded
                .meta
                .as_ref()
                .and_then(|meta| meta.order)
                .unwrap_or(i32::MAX);
            let mut id = loaded
                .meta
                .as_ref()
                .map(|meta| sanitize_dict_id(&meta.dict_id))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| sanitize_dict_id(&fallback_id));
            if id.is_empty() || id == CUSTOM_DICT_ID {
                id = format!("bundled-{}", sanitize_dict_id(&fallback_id));
            }

            let name = loaded
                .meta
                .as_ref()
                .map(|meta| meta.dict_name.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback_id.clone());

            let entries = loaded.entries;
            if let Some(existing) = grouped.get_mut(&id) {
                let same_meta = existing.order == declared_order && existing.name == name;
                if same_meta {
                    existing.entries.extend(entries);
                    continue;
                }
            }

            let mut resolved_id = id.clone();
            if grouped.contains_key(&resolved_id) {
                while grouped.contains_key(&resolved_id) {
                    resolved_id.push('1');
                }
                eprintln!(
                    "内置词库 dictId 冲突且元信息不一致，已重命名 {} -> {}（文件：{}）",
                    id,
                    resolved_id,
                    file.display()
                );
            }

            grouped.insert(
                resolved_id,
                BundledBucket {
                    order: declared_order,
                    file_index,
                    name,
                    entries,
                },
            );
        }
        let mut result: Vec<(i32, usize, DictionaryData)> = grouped
            .into_iter()
            .map(|(id, bucket)| {
                (
                    bucket.order,
                    bucket.file_index,
                    DictionaryData::new(id, bucket.name, false, bucket.entries),
                )
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        result.into_iter().map(|(_, _, dict)| dict).collect()
    }
}
