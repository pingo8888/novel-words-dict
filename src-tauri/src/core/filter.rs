use super::types::{GenderType, GenreType, NameType};

pub(crate) fn matches_genre_filter(filter: &str, value: GenreType) -> bool {
    match filter {
        "all" => true,
        "east" => value == GenreType::East,
        "west" => value == GenreType::West,
        _ => true,
    }
}

pub(crate) fn matches_name_type_filter(filter: &str, value: NameType) -> bool {
    match filter {
        "all" => true,
        "surname" => value == NameType::Surname || value == NameType::Both,
        "given" => value == NameType::Given || value == NameType::Both,
        "place" => value == NameType::Place || value == NameType::Both,
        "gear" => value == NameType::Gear || value == NameType::Both,
        "item" => value == NameType::Item || value == NameType::Both,
        "skill" => value == NameType::Skill || value == NameType::Both,
        "faction" => value == NameType::Faction || value == NameType::Both,
        "nickname" => value == NameType::Nickname || value == NameType::Both,
        "creature" => value == NameType::Creature || value == NameType::Both,
        "others" => value == NameType::Others || value == NameType::Both,
        "both" => value == NameType::Both,
        _ => true,
    }
}

pub(crate) fn matches_gender_type_filter(filter: &str, value: GenderType) -> bool {
    match filter {
        "all" => true,
        "male" => value == GenderType::Male || value == GenderType::Both,
        "female" => value == GenderType::Female || value == GenderType::Both,
        "both" => value == GenderType::Both,
        _ => true,
    }
}
