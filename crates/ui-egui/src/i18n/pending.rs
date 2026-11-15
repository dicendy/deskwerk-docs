//! Interface text the craft-i18n catalogs don't have yet (`pending.tsv`), looked up only when a
//! language's catalog has no row for it. The rows move to craft-i18n, which wins once it has
//! them.

use std::collections::HashMap;
use std::sync::OnceLock;

const ROWS: &str = include_str!("pending.tsv");

fn table() -> &'static HashMap<(&'static str, &'static str), &'static str> {
    static TABLE: OnceLock<HashMap<(&'static str, &'static str), &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        ROWS.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| {
                let mut cols = l.split('\t');
                Some(((cols.next()?, cols.next()?), cols.next()?))
            })
            .collect()
    })
}

/// `s` in the language `code`, when this file has it.
pub(super) fn get(code: &str, s: &str) -> Option<&'static str> {
    table().get(&(code, s)).copied()
}

#[cfg(test)]
mod tests {
    /// Every row is a known language with a translation, and every language has the same rows.
    #[test]
    fn rows_are_complete() {
        let mut per_lang: std::collections::BTreeMap<&str, std::collections::BTreeSet<&str>> = Default::default();
        for l in super::ROWS.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let cols: Vec<&str> = l.split('\t').collect();
            assert_eq!(cols.len(), 3, "{l}");
            assert!(crate::i18n::LANGUAGES.iter().any(|lang| lang.code == cols[0]), "{l}");
            assert!(!cols[2].trim().is_empty(), "{l}");
            per_lang.entry(cols[0]).or_default().insert(cols[1]);
        }
        let first = per_lang.values().next().cloned().unwrap_or_default();
        assert!(!first.is_empty());
        assert!(per_lang.values().all(|keys| *keys == first));
        assert_eq!(super::get("de", "This PC"), Some("Dieser PC"));
        assert_eq!(super::get("ru", "A file name can't contain \\ or /."), Some("Имя файла не может содержать \\ или /."));
    }
}
