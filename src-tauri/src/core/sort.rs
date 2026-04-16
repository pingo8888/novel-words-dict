use pinyin::ToPinyin;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TermSortKey {
    pub(crate) bucket: u8,
    pub(crate) initial: Option<char>,
    pub(crate) pinyin: String,
}

fn leading_alpha_initial(term: &str) -> Option<char> {
    for ch in term.trim().chars() {
        if ch.is_ascii_alphabetic() {
            return Some(ch.to_ascii_uppercase());
        }
        if let Some(pinyin) = ch.to_pinyin() {
            let initial = pinyin
                .plain()
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase());
            if initial.is_some() {
                return initial;
            }
        }
    }
    None
}

fn pinyin_sort_key(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii() {
            out.extend(ch.to_lowercase());
            continue;
        }
        if let Some(pinyin) = ch.to_pinyin() {
            out.push_str(pinyin.plain());
            continue;
        }
        out.extend(ch.to_lowercase());
    }
    out
}

pub(crate) fn build_term_sort_key(term: &str) -> TermSortKey {
    let initial = leading_alpha_initial(term);
    let bucket = if initial.is_some() { 0_u8 } else { 1_u8 };
    TermSortKey {
        bucket,
        initial,
        pinyin: pinyin_sort_key(term),
    }
}
