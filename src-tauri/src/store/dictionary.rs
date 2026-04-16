use std::collections::HashMap;

use crate::core::sort::compare_terms;
use crate::core::text::make_term_key;
use crate::core::types::NameEntry;
use crate::{CUSTOM_DICT_ID, CUSTOM_DICT_NAME};

#[derive(Debug, Clone)]
pub(crate) struct DictionaryData {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) editable: bool,
    pub(crate) entries: Vec<NameEntry>,
    pub(crate) index: HashMap<String, usize>,
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
        entries.sort_by(|a, b| compare_terms(&a.term, &b.term));

        let mut index = HashMap::new();
        for (idx, entry) in entries.iter().enumerate() {
            index.insert(make_term_key(&entry.term), idx);
        }

        Self {
            id,
            name,
            editable,
            entries,
            index,
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
        }
    }
}
