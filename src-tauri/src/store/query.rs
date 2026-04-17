use serde::{Deserialize, Serialize};

use crate::core::types::{GenderType, GenreType, NameType};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryRequest {
    pub(crate) dict_id: Option<String>,
    pub(crate) genre_type: Option<String>,
    pub(crate) name_type: Option<String>,
    pub(crate) gender_type: Option<String>,
    pub(crate) keyword: Option<String>,
    pub(crate) page: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryItem {
    pub(crate) term: String,
    #[serde(skip_serializing)]
    pub(crate) term_key: String,
    pub(crate) group: String,
    pub(crate) name_type: NameType,
    pub(crate) gender_type: GenderType,
    pub(crate) genre: GenreType,
    pub(crate) dict_id: String,
    pub(crate) dict_name: String,
    pub(crate) editable: bool,
    #[serde(skip_serializing)]
    pub(crate) term_norm: String,
    #[serde(skip_serializing)]
    pub(crate) group_norm: String,
    #[serde(skip_serializing)]
    pub(crate) name_type_norm: String,
    #[serde(skip_serializing)]
    pub(crate) sort_bucket: u8,
    #[serde(skip_serializing)]
    pub(crate) sort_initial: Option<char>,
    #[serde(skip_serializing)]
    pub(crate) sort_pinyin: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryResponse {
    pub(crate) items: Vec<QueryItem>,
    pub(crate) total: usize,
    pub(crate) total_all: usize,
    pub(crate) page: usize,
    pub(crate) page_count: usize,
}
