use pinyin::ToPinyin;
use std::cmp::Ordering;

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

pub(crate) fn compare_terms(left: &str, right: &str) -> Ordering {
    let left_initial = leading_alpha_initial(left);
    let right_initial = leading_alpha_initial(right);
    let left_bucket = if left_initial.is_some() { 0_u8 } else { 1_u8 };
    let right_bucket = if right_initial.is_some() { 0_u8 } else { 1_u8 };

    left_bucket
        .cmp(&right_bucket)
        .then_with(|| left_initial.cmp(&right_initial))
        .then_with(|| pinyin_sort_key(left).cmp(&pinyin_sort_key(right)))
        .then_with(|| left.cmp(right))
}
