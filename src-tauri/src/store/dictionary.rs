use std::collections::HashMap;

use crate::core::sort::build_term_sort_key;
use crate::core::text::{make_term_key, normalize_text};
use crate::core::types::NameEntry;
use crate::{CUSTOM_DICT_ID, CUSTOM_DICT_NAME};

use super::query::QueryItem;

#[derive(Debug, Clone)]
pub(crate) struct DictionaryData {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) editable: bool,
    pub(crate) entries: Vec<NameEntry>,
    pub(crate) index: HashMap<String, usize>,
    pub(crate) query_items: Vec<QueryItem>,
}

impl DictionaryData {
    pub(crate) fn new(
        id: String,
        name: String,
        editable: bool,
        mut entries: Vec<NameEntry>,
    ) -> Self {
        for entry in &mut entries {
            entry.term = entry.term.trim().to_string();
            entry.group = entry.group.trim().to_string();
        }
        entries.retain(|entry| !entry.term.is_empty());

        let mut data = Self {
            id,
            name,
            editable,
            entries,
            index: HashMap::new(),
            query_items: Vec::new(),
        };
        data.rebuild_derived();
        data
    }

    pub(crate) fn rebuild_derived(&mut self) {
        self.entries.sort_by_cached_key(|entry| {
            let sort_key = build_term_sort_key(&entry.term);
            (sort_key.bucket, sort_key.initial, sort_key.pinyin, entry.term.clone())
        });

        self.index.clear();
        self.index.reserve(self.entries.len());

        self.query_items.clear();
        self.query_items.reserve(self.entries.len());

        for (idx, entry) in self.entries.iter().enumerate() {
            let term_key = make_term_key(&entry.term);
            self.index.insert(term_key.clone(), idx);
            let sort_key = build_term_sort_key(&entry.term);
            self.query_items.push(QueryItem {
                term: entry.term.clone(),
                term_key,
                group: entry.group.clone(),
                name_type: entry.name_type,
                gender_type: entry.gender_type,
                genre: entry.genre,
                dict_id: self.id.clone(),
                dict_name: self.name.clone(),
                editable: self.editable,
                term_norm: normalize_text(&entry.term),
                group_norm: normalize_text(&entry.group),
                sort_bucket: sort_key.bucket,
                sort_initial: sort_key.initial,
                sort_pinyin: sort_key.pinyin,
            });
        }
    }
}

impl Default for DictionaryData {
    fn default() -> Self {
        Self {
            id: CUSTOM_DICT_ID.to_string(),
            name: CUSTOM_DICT_NAME.to_string(),
            editable: true,
            entries: Vec::new(),
            index: HashMap::new(),
            query_items: Vec::new(),
        }
    }
}
