use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NameType {
    #[default]
    Both,
    Surname,
    Given,
    Place,
    Creature,
    Monster,
    Gear,
    Food,
    Item,
    Skill,
    Faction,
    Title,
    Nickname,
    #[serde(alias = "incantation")]
    Others,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GenderType {
    #[default]
    Both,
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GenreType {
    East,
    #[default]
    West,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NameEntry {
    pub(crate) term: String,
    pub(crate) group: String,
    #[serde(default)]
    pub(crate) name_type: NameType,
    #[serde(default)]
    pub(crate) gender_type: GenderType,
    #[serde(default)]
    pub(crate) genre: GenreType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DictionaryMeta {
    pub(crate) dict_id: String,
    pub(crate) dict_name: String,
    #[serde(default)]
    pub(crate) order: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DictionaryOption {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) editable: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedJsonData {
    pub(crate) meta: Option<DictionaryMeta>,
    pub(crate) entries: Vec<NameEntry>,
}
