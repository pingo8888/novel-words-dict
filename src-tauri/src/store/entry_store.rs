use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use tauri::AppHandle;

use crate::core::filter::{
    matches_gender_type_filter, matches_genre_filter, matches_name_type_filter,
};
use crate::core::text::{make_term_key, normalize_text};
use crate::core::types::{DictionaryOption, GenderType, GenreType, NameEntry, NameType};
use crate::infra::files::{
    collect_json_files, is_bundled_dict_order_file, is_custom_entries_file,
    load_bundled_dict_configs, load_entries_from_json_file, load_entries_from_ndjson_file,
    sanitize_dict_id,
};
use crate::infra::paths::{resolve_bundled_db_path, resolve_bundled_dict_dir_candidates};
use crate::{
    ALL_DICT_ID, ALL_DICT_NAME, BUNDLED_DICT_ORDER_FILE_NAME, CUSTOM_DICT_ID, CUSTOM_DICT_NAME,
    LEGACY_DATA_FILE_NAME, PAGE_SIZE,
};

use super::dictionary::DictionaryData;
use super::query::{GroupSuggestionRequest, QueryRequest, QueryResponse};

const CUSTOM_DB_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS custom_entries (
    term_key TEXT PRIMARY KEY,
    term TEXT NOT NULL,
    group_name TEXT NOT NULL DEFAULT '',
    name_type TEXT NOT NULL,
    gender_type TEXT NOT NULL,
    genre TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_custom_entries_term ON custom_entries(term);
"#;

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

fn compare_group_names(a: &str, b: &str) -> std::cmp::Ordering {
    let left = crate::core::sort::build_term_sort_key(a);
    let right = crate::core::sort::build_term_sort_key(b);
    left.bucket
        .cmp(&right.bucket)
        .then_with(|| left.initial.cmp(&right.initial))
        .then_with(|| left.pinyin.cmp(&right.pinyin))
        .then_with(|| a.cmp(b))
}

fn name_type_to_str(value: NameType) -> &'static str {
    match value {
        NameType::Both => "both",
        NameType::Surname => "surname",
        NameType::Given => "given",
        NameType::Place => "place",
        NameType::Myth => "myth",
        NameType::People => "people",
        NameType::Creature => "creature",
        NameType::Monster => "monster",
        NameType::Gear => "gear",
        NameType::Food => "food",
        NameType::Item => "item",
        NameType::Skill => "skill",
        NameType::Faction => "faction",
        NameType::Title => "title",
        NameType::Nickname => "nickname",
        NameType::Book => "book",
        NameType::Others => "others",
    }
}

fn parse_name_type(value: &str) -> NameType {
    match value.trim().to_ascii_lowercase().as_str() {
        "surname" => NameType::Surname,
        "given" => NameType::Given,
        "place" => NameType::Place,
        "myth" => NameType::Myth,
        "people" => NameType::People,
        "creature" => NameType::Creature,
        "monster" => NameType::Monster,
        "gear" => NameType::Gear,
        "food" => NameType::Food,
        "item" | "items" => NameType::Item,
        "skill" => NameType::Skill,
        "faction" => NameType::Faction,
        "title" => NameType::Title,
        "nickname" => NameType::Nickname,
        "book" => NameType::Book,
        "others" | "other" | "incantation" => NameType::Others,
        _ => NameType::Both,
    }
}

fn gender_type_to_str(value: GenderType) -> &'static str {
    match value {
        GenderType::Male => "male",
        GenderType::Female => "female",
        GenderType::Both => "both",
    }
}

fn parse_gender_type(value: &str) -> GenderType {
    match value.trim().to_ascii_lowercase().as_str() {
        "male" => GenderType::Male,
        "female" => GenderType::Female,
        _ => GenderType::Both,
    }
}

fn genre_type_to_str(value: GenreType) -> &'static str {
    match value {
        GenreType::China => "china",
        GenreType::Japan => "japan",
        GenreType::West => "west",
    }
}

fn parse_genre_type(value: &str) -> GenreType {
    match value.trim().to_ascii_lowercase().as_str() {
        "china" | "east" => GenreType::China,
        "japan" => GenreType::Japan,
        _ => GenreType::West,
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DictSourceKeyword {
    Any,
    Custom,
    Bundled,
    Conflict,
}

impl DictSourceKeyword {
    fn with_token(self, token: DictSourceKeyword) -> Self {
        match (self, token) {
            (Self::Any, next) => next,
            (current, next) if current == next => current,
            (Self::Conflict, _) | (_, Self::Conflict) => Self::Conflict,
            _ => Self::Conflict,
        }
    }
}

fn genre_keyword_filter(token: &str) -> Option<&'static str> {
    match token {
        "china" | "east" | "中国" | "东方" => Some("china"),
        "japan" | "日本" => Some("japan"),
        "west" | "西方" => Some("west"),
        _ => None,
    }
}

fn name_type_keyword_filter(token: &str) -> Option<&'static str> {
    match token {
        "surname" | "姓氏" => Some("surname"),
        "given" | "名字" => Some("given"),
        "place" | "地名" => Some("place"),
        "myth" | "神话" => Some("myth"),
        "people" | "人物" => Some("people"),
        "creature" | "生物" => Some("creature"),
        "monster" | "怪物" => Some("monster"),
        "gear" | "装备" => Some("gear"),
        "food" | "食物" => Some("food"),
        "item" | "items" | "物品" => Some("item"),
        "skill" | "技能" => Some("skill"),
        "faction" | "势力" => Some("faction"),
        "title" | "头衔" => Some("title"),
        "nickname" | "绰号" => Some("nickname"),
        "book" | "书籍" => Some("book"),
        "others" | "other" | "incantation" | "其他" => Some("others"),
        "both" => Some("both"),
        _ => None,
    }
}

fn gender_keyword_filter(token: &str) -> Option<&'static str> {
    match token {
        "男性" => Some("male"),
        "女性" => Some("female"),
        "通用" => Some("both"),
        _ => None,
    }
}

#[derive(Default)]
pub(crate) struct EntryStore {
    pub(crate) custom: DictionaryData,
    pub(crate) bundled: Vec<DictionaryData>,
    pub(crate) custom_db_path: Option<PathBuf>,
    pub(crate) custom_conn: Option<Connection>,
    pub(crate) total_all_cache: usize,
    pub(crate) custom_term_keys: HashSet<String>,
}

impl EntryStore {
    pub(crate) fn load<R: tauri::Runtime>(
        &mut self,
        app: &AppHandle<R>,
        custom_db_path: PathBuf,
        legacy_entries_path: PathBuf,
    ) -> Result<(), String> {
        let mut custom_conn = Self::open_custom_db(custom_db_path.as_path())?;
        let custom_entries = self
            .load_custom_entries_with_migration(&mut custom_conn, legacy_entries_path.as_path())?;

        self.custom = DictionaryData::new(
            CUSTOM_DICT_ID.to_string(),
            CUSTOM_DICT_NAME.to_string(),
            true,
            custom_entries,
        );
        self.bundled = self.load_bundled_dictionaries(app);
        self.custom_db_path = Some(custom_db_path);
        self.custom_conn = Some(custom_conn);
        self.refresh_custom_term_keys();
        self.total_all_cache = self.compute_total_entries_merged_all();
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
        let mut genre_keyword_filters: Vec<&'static str> = Vec::new();
        let mut name_type_keyword_filters: Vec<&'static str> = Vec::new();
        let mut gender_keyword_filters: Vec<&'static str> = Vec::new();
        let mut dict_source_keyword = DictSourceKeyword::Any;
        for raw_token in request.keyword.as_deref().unwrap_or("").split_whitespace() {
            let token = raw_token.to_lowercase();
            if token.is_empty() {
                continue;
            }
            match token.as_str() {
                "自定" => {
                    dict_source_keyword = dict_source_keyword.with_token(DictSourceKeyword::Custom);
                    continue;
                }
                "内置" => {
                    dict_source_keyword =
                        dict_source_keyword.with_token(DictSourceKeyword::Bundled);
                    continue;
                }
                _ => {}
            }
            if let Some(group_token) = token.strip_prefix('@') {
                if !group_token.is_empty() {
                    group_keyword_tokens.push(group_token.to_string());
                }
                continue;
            }
            if let Some(filter) = genre_keyword_filter(token.as_str()) {
                genre_keyword_filters.push(filter);
                continue;
            }
            if let Some(filter) = name_type_keyword_filter(token.as_str()) {
                name_type_keyword_filters.push(filter);
                continue;
            }
            if let Some(filter) = gender_keyword_filter(token.as_str()) {
                gender_keyword_filters.push(filter);
                continue;
            }
            normal_keyword_tokens.push(token);
        }

        let matches_item = |entry: &super::query::QueryItem| {
            matches_genre_filter(&genre_type, entry.genre)
                && matches_name_type_filter(&name_type, entry.name_type)
                && matches_gender_type_filter(&gender_type, entry.gender_type)
                && genre_keyword_filters
                    .iter()
                    .all(|filter| matches_genre_filter(filter, entry.genre))
                && name_type_keyword_filters
                    .iter()
                    .all(|filter| matches_name_type_filter(filter, entry.name_type))
                && gender_keyword_filters
                    .iter()
                    .all(|filter| matches_gender_type_filter(filter, entry.gender_type))
                && (normal_keyword_tokens.is_empty()
                    || normal_keyword_tokens
                        .iter()
                        .all(|token| entry.term_norm.contains(token)))
                && (group_keyword_tokens.is_empty()
                    || group_keyword_tokens
                        .iter()
                        .all(|token| entry.group_norm.contains(token)))
        };

        let mut matched: Vec<&super::query::QueryItem> = Vec::new();
        if dict_filter.eq_ignore_ascii_case(ALL_DICT_ID) {
            matched.reserve(self.total_all_cache);
            if matches!(
                dict_source_keyword,
                DictSourceKeyword::Any | DictSourceKeyword::Custom
            ) {
                for entry in &self.custom.query_items {
                    if matches_item(entry) {
                        matched.push(entry);
                    }
                }
            }
            if matches!(
                dict_source_keyword,
                DictSourceKeyword::Any | DictSourceKeyword::Bundled
            ) {
                for dict in &self.bundled {
                    for entry in &dict.query_items {
                        if dict_source_keyword == DictSourceKeyword::Any
                            && self.custom_term_keys.contains(&entry.term_key)
                        {
                            continue;
                        }
                        if matches_item(entry) {
                            matched.push(entry);
                        }
                    }
                }
            }
        } else {
            let selected_dict = self.select_dictionary(dict_filter.as_str());
            matched.reserve(selected_dict.query_items.len());
            let source_matches = if selected_dict.id.eq_ignore_ascii_case(CUSTOM_DICT_ID) {
                matches!(
                    dict_source_keyword,
                    DictSourceKeyword::Any | DictSourceKeyword::Custom
                )
            } else {
                matches!(
                    dict_source_keyword,
                    DictSourceKeyword::Any | DictSourceKeyword::Bundled
                )
            };
            if source_matches {
                for entry in &selected_dict.query_items {
                    if matches_item(entry) {
                        matched.push(entry);
                    }
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

    pub(crate) fn query_group_suggestions(&self, request: &GroupSuggestionRequest) -> Vec<String> {
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
        let keyword = normalize_text(request.keyword.as_deref().unwrap_or("").trim());

        let matches_item = |entry: &super::query::QueryItem| {
            !entry.group.trim().is_empty()
                && matches_genre_filter(&genre_type, entry.genre)
                && matches_name_type_filter(&name_type, entry.name_type)
                && matches_gender_type_filter(&gender_type, entry.gender_type)
                && (keyword.is_empty() || entry.group_norm.contains(&keyword))
        };

        let mut seen = HashSet::new();
        let mut groups = Vec::new();
        let mut collect_group = |entry: &super::query::QueryItem| {
            if !matches_item(entry) {
                return;
            }
            let group = entry.group.trim();
            let key = normalize_text(group);
            if seen.insert(key) {
                groups.push(group.to_string());
            }
        };

        if dict_filter.eq_ignore_ascii_case(ALL_DICT_ID) {
            for entry in &self.custom.query_items {
                collect_group(entry);
            }
            for dict in &self.bundled {
                for entry in &dict.query_items {
                    if self.custom_term_keys.contains(&entry.term_key) {
                        continue;
                    }
                    collect_group(entry);
                }
            }
        } else {
            let selected_dict = self.select_dictionary(dict_filter.as_str());
            for entry in &selected_dict.query_items {
                collect_group(entry);
            }
        }

        groups.sort_by(|a, b| compare_group_names(a, b));
        groups
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
        if key.is_empty() {
            return Err("词条不能为空".to_string());
        }

        let previous_key = original_term.map(make_term_key).unwrap_or_default();
        self.persist_upsert_to_custom_db(&entry, previous_key.as_str(), key.as_str())?;

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
        Ok(())
    }

    pub(crate) fn delete(&mut self, term: &str) -> Result<(), String> {
        let key = make_term_key(term);
        let Some(existing_idx) = self.custom.index.get(&key).copied() else {
            return Err("词条不存在".to_string());
        };

        self.persist_delete_from_custom_db(key.as_str())?;

        self.custom.entries.remove(existing_idx);
        self.custom.rebuild_derived();
        self.refresh_custom_term_keys();
        self.total_all_cache = self.compute_total_entries_merged_all();
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

    fn custom_conn_mut(&mut self) -> Result<&mut Connection, String> {
        self.custom_conn
            .as_mut()
            .ok_or_else(|| "自定义词库数据库连接未初始化".to_string())
    }

    fn open_custom_db(path: &Path) -> Result<Connection, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("创建数据目录失败: {err}"))?;
        }
        let conn = Connection::open(path)
            .map_err(|err| format!("打开自定义词库数据库失败 {}: {err}", path.display()))?;
        conn.execute_batch(CUSTOM_DB_SCHEMA)
            .map_err(|err| format!("初始化自定义词库数据库失败 {}: {err}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
            .map_err(|err| format!("设置自定义词库数据库 PRAGMA 失败 {}: {err}", path.display()))?;
        Ok(conn)
    }

    fn load_custom_entries_with_migration(
        &self,
        conn: &mut Connection,
        legacy_entries_path: &Path,
    ) -> Result<Vec<NameEntry>, String> {
        let count: i64 = conn
            .query_row("SELECT COUNT(1) FROM custom_entries", [], |row| row.get(0))
            .map_err(|err| format!("读取自定义词库数量失败: {err}"))?;

        if count == 0 {
            let migrated = self.migrate_legacy_entries_into_db(conn, legacy_entries_path)?;
            if migrated {
                if let Err(err) = self.backup_legacy_sources(legacy_entries_path) {
                    eprintln!("备份旧词库文件失败: {err}");
                }
            }
        }

        self.read_custom_entries_from_db(conn)
    }

    fn read_custom_entries_from_db(&self, conn: &Connection) -> Result<Vec<NameEntry>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT term, group_name, name_type, gender_type, genre
                 FROM custom_entries
                 ORDER BY term COLLATE NOCASE ASC",
            )
            .map_err(|err| format!("读取自定义词条失败: {err}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(NameEntry {
                    term: row.get::<_, String>(0)?,
                    group: row.get::<_, String>(1)?,
                    name_type: parse_name_type(&row.get::<_, String>(2)?),
                    gender_type: parse_gender_type(&row.get::<_, String>(3)?),
                    genre: parse_genre_type(&row.get::<_, String>(4)?),
                })
            })
            .map_err(|err| format!("读取自定义词条失败: {err}"))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row.map_err(|err| format!("读取自定义词条失败: {err}"))?);
        }
        Ok(entries)
    }

    fn persist_upsert_to_custom_db(
        &mut self,
        entry: &NameEntry,
        previous_key: &str,
        key: &str,
    ) -> Result<(), String> {
        let conn = self.custom_conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启数据库事务失败: {err}"))?;

        if !previous_key.is_empty() && previous_key != key {
            tx.execute(
                "DELETE FROM custom_entries WHERE term_key = ?1",
                params![previous_key],
            )
            .map_err(|err| format!("删除旧词条失败: {err}"))?;
        }

        tx.execute(
            "INSERT INTO custom_entries (term_key, term, group_name, name_type, gender_type, genre)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(term_key) DO UPDATE SET
                term = excluded.term,
                group_name = excluded.group_name,
                name_type = excluded.name_type,
                gender_type = excluded.gender_type,
                genre = excluded.genre",
            params![
                key,
                entry.term,
                entry.group,
                name_type_to_str(entry.name_type),
                gender_type_to_str(entry.gender_type),
                genre_type_to_str(entry.genre)
            ],
        )
        .map_err(|err| format!("保存词条失败: {err}"))?;

        tx.commit()
            .map_err(|err| format!("提交数据库事务失败: {err}"))
    }

    fn persist_delete_from_custom_db(&mut self, key: &str) -> Result<(), String> {
        let conn = self.custom_conn_mut()?;
        let affected = conn
            .execute(
                "DELETE FROM custom_entries WHERE term_key = ?1",
                params![key],
            )
            .map_err(|err| format!("删除词条失败: {err}"))?;
        if affected == 0 {
            return Err("词条不存在".to_string());
        }
        Ok(())
    }

    fn migrate_legacy_entries_into_db(
        &self,
        conn: &mut Connection,
        legacy_entries_path: &Path,
    ) -> Result<bool, String> {
        let entries = self.load_legacy_entries_for_migration(legacy_entries_path)?;
        if entries.is_empty() {
            return Ok(false);
        }

        let tx = conn
            .transaction()
            .map_err(|err| format!("开启迁移事务失败: {err}"))?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO custom_entries (term_key, term, group_name, name_type, gender_type, genre)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(term_key) DO UPDATE SET
                        term = excluded.term,
                        group_name = excluded.group_name,
                        name_type = excluded.name_type,
                        gender_type = excluded.gender_type,
                        genre = excluded.genre",
                )
                .map_err(|err| format!("准备迁移语句失败: {err}"))?;

            for entry in entries {
                let term_key = make_term_key(&entry.term);
                if term_key.is_empty() {
                    continue;
                }
                stmt.execute(params![
                    term_key,
                    entry.term,
                    entry.group,
                    name_type_to_str(entry.name_type),
                    gender_type_to_str(entry.gender_type),
                    genre_type_to_str(entry.genre)
                ])
                .map_err(|err| format!("迁移词条失败: {err}"))?;
            }
        }

        tx.commit()
            .map_err(|err| format!("提交迁移事务失败: {err}"))?;
        Ok(true)
    }

    fn load_legacy_entries_for_migration(
        &self,
        legacy_entries_path: &Path,
    ) -> Result<Vec<NameEntry>, String> {
        let data_dir = legacy_entries_path
            .parent()
            .ok_or_else(|| "旧词库目录路径无效".to_string())?
            .to_path_buf();

        let mut latest: HashMap<String, NameEntry> = HashMap::new();
        let mut loaded_json = false;

        let mut json_files = collect_json_files(&data_dir)?;
        json_files.sort();
        json_files.retain(|candidate| candidate != legacy_entries_path);
        json_files.push(legacy_entries_path.to_path_buf());

        for file_path in json_files {
            if !file_path.exists() {
                continue;
            }
            match load_entries_from_json_file(&file_path) {
                Ok(loaded) => {
                    loaded_json = true;
                    for mut entry in loaded.entries {
                        entry.term = entry.term.trim().to_string();
                        entry.group = entry.group.trim().to_string();
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
                            entry.group = entry.group.trim().to_string();
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

        Ok(latest.into_values().collect())
    }

    fn backup_legacy_sources(&self, legacy_entries_path: &Path) -> Result<(), String> {
        self.backup_single_legacy_file(legacy_entries_path)?;

        let data_dir = match legacy_entries_path.parent() {
            Some(value) => value,
            None => return Ok(()),
        };
        let ndjson_path = data_dir
            .parent()
            .map(|dir| dir.join(LEGACY_DATA_FILE_NAME))
            .unwrap_or_else(|| data_dir.join(LEGACY_DATA_FILE_NAME));
        self.backup_single_legacy_file(&ndjson_path)
    }

    fn backup_single_legacy_file(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("entries");

        for suffix in 0..1000_u32 {
            let backup_name = if suffix == 0 {
                format!("{file_name}.bak-{stamp}")
            } else {
                format!("{file_name}.bak-{stamp}-{suffix}")
            };
            let backup_path = path.with_file_name(backup_name);
            if backup_path.exists() {
                continue;
            }
            match fs::rename(path, &backup_path) {
                Ok(_) => return Ok(()),
                Err(_) => {
                    fs::copy(path, &backup_path).map_err(|err| {
                        format!(
                            "备份旧词库文件失败 {} -> {}: {err}",
                            path.display(),
                            backup_path.display()
                        )
                    })?;
                    return Ok(());
                }
            }
        }

        Err(format!(
            "备份旧词库文件失败 {}: 备份文件名冲突",
            path.display()
        ))
    }

    fn load_bundled_dictionaries<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Vec<DictionaryData> {
        if let Some(db_path) = resolve_bundled_db_path(app) {
            match self.load_bundled_dictionaries_from_db(db_path.as_path()) {
                Ok(loaded) if !loaded.is_empty() => return loaded,
                Ok(_) => {}
                Err(err) => {
                    eprintln!("读取内置词库数据库失败 {}: {err}", db_path.display());
                }
            }
        }

        for dict_dir in resolve_bundled_dict_dir_candidates(app) {
            let loaded = self.load_bundled_dictionaries_from_dir(&dict_dir);
            if loaded.is_empty() {
                continue;
            }
            return loaded;
        }
        Vec::new()
    }

    fn load_bundled_dictionaries_from_db(
        &self,
        db_path: &Path,
    ) -> Result<Vec<DictionaryData>, String> {
        let conn =
            Connection::open(db_path).map_err(|err| format!("打开内置词库数据库失败: {err}"))?;

        let mut dict_stmt = conn
            .prepare(
                "SELECT dict_id, dict_name
                 FROM dictionaries
                 ORDER BY sort_order ASC, file_index ASC, dict_id ASC",
            )
            .map_err(|err| format!("读取内置词库目录失败: {err}"))?;

        let dict_rows = dict_stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| format!("读取内置词库目录失败: {err}"))?;

        let mut dictionaries: Vec<(String, String)> = Vec::new();
        for row in dict_rows {
            dictionaries.push(row.map_err(|err| format!("读取内置词库目录失败: {err}"))?);
        }

        let mut out = Vec::with_capacity(dictionaries.len());
        let mut entry_stmt = conn
            .prepare(
                "SELECT term, group_name, name_type, gender_type, genre
                 FROM entries
                 WHERE dict_id = ?1
                 ORDER BY id ASC",
            )
            .map_err(|err| format!("读取内置词库失败: {err}"))?;

        for (dict_id, dict_name) in dictionaries {
            let rows = entry_stmt
                .query_map([dict_id.as_str()], |row| {
                    Ok(NameEntry {
                        term: row.get::<_, String>(0)?,
                        group: row.get::<_, String>(1)?,
                        name_type: parse_name_type(&row.get::<_, String>(2)?),
                        gender_type: parse_gender_type(&row.get::<_, String>(3)?),
                        genre: parse_genre_type(&row.get::<_, String>(4)?),
                    })
                })
                .map_err(|err| format!("读取内置词库失败: {err}"))?;

            let mut entries = Vec::new();
            for row in rows {
                entries.push(row.map_err(|err| format!("读取内置词库失败: {err}"))?);
            }

            out.push(DictionaryData::new(dict_id, dict_name, false, entries));
        }

        Ok(out)
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
