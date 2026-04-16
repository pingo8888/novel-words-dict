pub(crate) fn normalize_text(value: &str) -> String {
    value.chars().flat_map(|ch| ch.to_lowercase()).collect()
}

pub(crate) fn make_term_key(term: &str) -> String {
    normalize_text(term.trim())
}
