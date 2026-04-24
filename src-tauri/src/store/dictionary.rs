use std::collections::HashMap;

use crate::core::sort::build_term_sort_key;
use crate::core::text::{make_term_key, normalize_text};
use crate::core::types::{GenderType, GenreType, NameEntry, NameType};
use crate::{CUSTOM_DICT_ID, CUSTOM_DICT_NAME};

use super::query::QueryItem;

fn name_type_search_text(value: NameType) -> &'static str {
    match value {
        NameType::Both => "both 姓氏 名字",
        NameType::Surname => "surname 姓氏",
        NameType::Given => "given 名字",
        NameType::Place => "place 地名",
        NameType::Myth => "myth 神话",
        NameType::People => "people 人物",
        NameType::Creature => "creature 生物",
        NameType::Monster => "monster 怪物",
        NameType::Gear => "gear 装备",
        NameType::Food => "food 食物",
        NameType::Item => "item 物品",
        NameType::Skill => "skill 技能",
        NameType::Faction => "faction 势力",
        NameType::Title => "title 头衔",
        NameType::Nickname => "nickname 绰号",
        NameType::Book => "book 书籍",
        NameType::Others => "others 其他",
    }
}

fn gender_type_search_text(value: GenderType) -> &'static str {
    match value {
        GenderType::Male => "男性",
        GenderType::Female => "女性",
        GenderType::Both => "通用",
    }
}

fn genre_search_text(value: GenreType) -> &'static str {
    match value {
        GenreType::China => "china 中国 东方",
        GenreType::Japan => "japan 日本",
        GenreType::West => "西方",
    }
}

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
            (
                sort_key.bucket,
                sort_key.initial,
                sort_key.pinyin,
                entry.term.clone(),
            )
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
                name_type_norm: normalize_text(name_type_search_text(entry.name_type)),
                gender_type_norm: normalize_text(gender_type_search_text(entry.gender_type)),
                genre_norm: normalize_text(genre_search_text(entry.genre)),
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
