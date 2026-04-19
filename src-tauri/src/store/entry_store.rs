use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::core::filter::{
    matches_gender_type_filter, matches_genre_filter, matches_name_type_filter,
};
use crate::core::text::make_term_key;
use crate::core::types::{DictionaryMeta, DictionaryOption, NameEntry};
use crate::infra::files::{
    collect_json_files, is_bundled_dict_order_file, is_custom_entries_file,
    load_bundled_dict_configs, load_entries_from_json_file, load_entries_from_ndjson_file,
    replace_file_from_temp, sanitize_dict_id,
};
use crate::infra::paths::resolve_bundled_dict_dir_candidates;
use crate::{
    ALL_DICT_ID, ALL_DICT_NAME, BUNDLED_DICT_ORDER_FILE_NAME, CUSTOM_DICT_ID, CUSTOM_DICT_NAME,
    LEGACY_DATA_FILE_NAME, PAGE_SIZE,
};

use super::dictionary::DictionaryData;
use super::query::{QueryRequest, QueryResponse};

fn compare_query_items(
    a: &super::query::QueryItem,
    b: &super::query::QueryItem,
) -> std::cmp::Ordering {
    a.sort_bucket
        .cmp(&b.sort_bucket)
        .then_with(|| a.sort_initial.cmp(&b.sort_initial))
        .then_with(|| a.sort_pinyin.cmp(&b.sort_pinyin))
        .then_with(|| a.term.cmp(&b.term))
}

fn compare_query_item_refs(
    a: &&super::query::QueryItem,
    b: &&super::query::QueryItem,
) -> std::cmp::Ordering {
    compare_query_items(a, b)
}

#[derive(Default)]
pub(crate) struct EntryStore {
    pub(crate) custom: DictionaryData,
    pub(crate) bundled: Vec<DictionaryData>,
    pub(crate) custom_data_path: Option<PathBuf>,
    pub(crate) total_all_cache: usize,
    pub(crate) custom_term_keys: HashSet<String>,
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
                        // Later file overrides same term from earlier file.
                        // File order is deterministic by sorted path, then custom entries file last.
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
                            // Legacy file is loaded only when no JSON file was parsed.
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
        self.refresh_custom_term_keys();
        self.total_all_cache = self.compute_total_entries_merged_all();

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
        let mut normal_keyword_tokens: Vec<String> = Vec::new();
        let mut group_keyword_tokens: Vec<String> = Vec::new();
        for raw_token in request.keyword.as_deref().unwrap_or("").split_whitespace() {
            let token = raw_token.to_lowercase();
            if token.is_empty() {
                continue;
            }
            if let Some(group_token) = token.strip_prefix('@') {
                if !group_token.is_empty() {
                    group_keyword_tokens.push(group_token.to_string());
                }
                continue;
            }
            normal_keyword_tokens.push(token);
        }

        let matches_item = |entry: &super::query::QueryItem| {
            matches_genre_filter(&genre_type, entry.genre)
                && matches_name_type_filter(&name_type, entry.name_type)
                && matches_gender_type_filter(&gender_type, entry.gender_type)
                && (normal_keyword_tokens.is_empty()
                    || normal_keyword_tokens.iter().all(|token| {
                        entry.term_norm.contains(token)
                            || entry.name_type_norm.contains(token)
                            || entry.gender_type_norm.contains(token)
                            || entry.genre_norm.contains(token)
                    }))
                && (group_keyword_tokens.is_empty()
                    || group_keyword_tokens
                        .iter()
                        .all(|token| entry.group_norm.contains(token)))
        };

        let mut matched: Vec<&super::query::QueryItem> = Vec::new();
        if dict_filter.eq_ignore_ascii_case(ALL_DICT_ID) {
            matched.reserve(self.total_all_cache);
            for entry in &self.custom.query_items {
                if matches_item(entry) {
                    matched.push(entry);
                }
            }
            for dict in &self.bundled {
                for entry in &dict.query_items {
                    // Custom entries still override bundled ones.
                    // Duplicates among bundled dicts are intentionally preserved.
                    if self.custom_term_keys.contains(&entry.term_key) {
                        continue;
                    }
                    if matches_item(entry) {
                        matched.push(entry);
                    }
                }
            }
        } else {
            let selected_dict = self.select_dictionary(dict_filter.as_str());
            matched.reserve(selected_dict.query_items.len());
            for entry in &selected_dict.query_items {
                if matches_item(entry) {
                    matched.push(entry);
                }
            }
        }

        let total = matched.len();
        let page_count = if total == 0 {
            1
        } else {
            total.div_ceil(PAGE_SIZE)
        };
        let page = request.page.unwrap_or(1).max(1).min(page_count);
        let start = (page - 1) * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(total);

        let items = if total == 0 {
            Vec::new()
        } else if start == 0 && end == total {
            matched.sort_by(compare_query_item_refs);
            matched.into_iter().cloned().collect()
        } else if start == 0 {
            let (page_slice, _, _) = matched.select_nth_unstable_by(end, compare_query_item_refs);
            page_slice.sort_by(compare_query_item_refs);
            page_slice.iter().map(|entry| (*entry).clone()).collect()
        } else {
            let (_, pivot, tail) = matched.select_nth_unstable_by(start, compare_query_item_refs);
            let mut candidates: Vec<&super::query::QueryItem> = Vec::with_capacity(total - start);
            candidates.push(*pivot);
            candidates.extend(tail.iter().copied());
            if end < total {
                let page_len = end - start;
                let (page_slice, _, _) =
                    candidates.select_nth_unstable_by(page_len, compare_query_item_refs);
                page_slice.sort_by(compare_query_item_refs);
                page_slice.iter().map(|entry| (*entry).clone()).collect()
            } else {
                candidates.sort_by(compare_query_item_refs);
                candidates.into_iter().cloned().collect()
            }
        };

        QueryResponse {
            items,
            total,
            total_all: self.total_all_cache,
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

    pub(crate) fn get_bundled_entry(&self, term: &str) -> Option<NameEntry> {
        let key = make_term_key(term);
        if key.is_empty() {
            return None;
        }
        self.bundled.iter().find_map(|dict| {
            dict.index
                .get(&key)
                .and_then(|idx| dict.entries.get(*idx))
                .cloned()
        })
    }

    pub(crate) fn upsert(
        &mut self,
        mut entry: NameEntry,
        original_term: Option<&str>,
    ) -> Result<(), String> {
        entry.term = entry.term.trim().to_string();
        entry.group = entry.group.trim().to_string();
        if entry.term.is_empty() {
            return Err("词条不能为空".to_string());
        }

        let key = make_term_key(&entry.term);
        if let Some(previous_term) = original_term {
            let previous_key = make_term_key(previous_term);
            if !previous_key.is_empty() && previous_key != key {
                if let Some(previous_idx) = self
                    .custom
                    .entries
                    .iter()
                    .position(|item| make_term_key(&item.term) == previous_key)
                {
                    self.custom.entries.remove(previous_idx);
                }
            }
        }

        if let Some(existing_idx) = self
            .custom
            .entries
            .iter()
            .position(|item| make_term_key(&item.term) == key)
        {
            self.custom.entries[existing_idx] = entry;
        } else {
            self.custom.entries.push(entry);
        }

        self.custom.rebuild_derived();
        self.refresh_custom_term_keys();
        self.total_all_cache = self.compute_total_entries_merged_all();
        self.persist()
    }

    pub(crate) fn delete(&mut self, term: &str) -> Result<(), String> {
        let key = make_term_key(term);
        let Some(existing_idx) = self.custom.index.get(&key).copied() else {
            return Err("词条不存在".to_string());
        };

        self.custom.entries.remove(existing_idx);
        self.custom.rebuild_derived();
        self.refresh_custom_term_keys();
        self.total_all_cache = self.compute_total_entries_merged_all();
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
        replace_file_from_temp(&temp_path, path)?;
        Ok(())
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

    fn compute_total_entries_merged_all(&self) -> usize {
        // Keep custom entries as-is, and only skip bundled entries that
        // conflict with custom terms. Duplicates inside bundled dicts are kept.
        let mut total = self.custom.entries.len();

        for dict in &self.bundled {
            for entry in &dict.query_items {
                if self.custom_term_keys.contains(&entry.term_key) {
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

    fn refresh_custom_term_keys(&mut self) {
        self.custom_term_keys.clear();
        self.custom_term_keys
            .extend(self.custom.index.keys().cloned());
    }

    fn load_bundled_dictionaries<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Vec<DictionaryData> {
        for dict_dir in resolve_bundled_dict_dir_candidates(app) {
            let loaded = self.load_bundled_dictionaries_from_dir(&dict_dir);
            if loaded.is_empty() {
                continue;
            }
            return loaded;
        }
        Vec::new()
    }

    fn load_bundled_dictionaries_from_dir(&self, dict_dir: &Path) -> Vec<DictionaryData> {
        struct BundledBucket {
            order: i32,
            file_index: usize,
            name: String,
            entries: Vec<NameEntry>,
        }

        let dict_config_map =
            load_bundled_dict_configs(&dict_dir.join(BUNDLED_DICT_ORDER_FILE_NAME));
        let mut grouped: HashMap<String, BundledBucket> = HashMap::new();
        let mut files = match collect_json_files(dict_dir) {
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
            if is_bundled_dict_order_file(&file) {
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
            let mut id = loaded
                .meta
                .as_ref()
                .map(|meta| sanitize_dict_id(&meta.dict_id))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| sanitize_dict_id(&fallback_id));
            if id.is_empty() || id == CUSTOM_DICT_ID {
                id = format!("bundled-{}", sanitize_dict_id(&fallback_id));
            }
            let declared_order = loaded
                .meta
                .as_ref()
                .and_then(|meta| meta.order)
                .unwrap_or(i32::MAX);
            let resolved_order = dict_config_map
                .get(&id)
                .and_then(|config| config.order)
                .unwrap_or(declared_order);

            let fallback_name = loaded
                .meta
                .as_ref()
                .map(|meta| meta.dict_name.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| fallback_id.clone());
            let resolved_name = dict_config_map
                .get(&id)
                .and_then(|config| config.dict_name.as_ref())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or(fallback_name);

            let entries = loaded.entries;
            if let Some(existing) = grouped.get_mut(&id) {
                let same_meta = existing.order == resolved_order && existing.name == resolved_name;
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
                    order: resolved_order,
                    file_index,
                    name: resolved_name,
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
