//! The languages of the dashboard and the native window.
//!
//! Each language is one flat JSON file in `locales/`, compiled into the
//! binary. The page gets the whole table; the native window looks up the
//! few strings it needs.

use std::collections::HashMap;
use std::sync::OnceLock;

/// A language the launcher ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    En,
    Es,
}

impl Lang {
    pub(crate) const ALL: [Lang; 2] = [Lang::En, Lang::Es];

    /// The BCP 47 tag, as stored in the config and sent to the page.
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    /// Reads a tag such as `es` or `es-MX`. Unknown tags give `None`.
    pub(crate) fn from_tag(tag: &str) -> Option<Lang> {
        let primary = tag
            .split(['-', '_', '.'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        Lang::ALL.into_iter().find(|lang| lang.tag() == primary)
    }

    /// The config value wins; then the operating system locale; then English.
    pub(crate) fn detect(configured: Option<&str>) -> Lang {
        configured
            .and_then(Lang::from_tag)
            .or_else(|| sys_locale::get_locale().as_deref().and_then(Lang::from_tag))
            .unwrap_or(Lang::En)
    }

    /// The JSON table of this language, as shipped.
    pub(crate) fn json(self) -> &'static str {
        match self {
            Lang::En => include_str!("../locales/en.json"),
            Lang::Es => include_str!("../locales/es.json"),
        }
    }

    fn table(self) -> &'static HashMap<String, String> {
        static TABLES: OnceLock<HashMap<Lang, HashMap<String, String>>> = OnceLock::new();
        let tables = TABLES.get_or_init(|| {
            Lang::ALL
                .into_iter()
                .map(|lang| {
                    let table = serde_json::from_str(lang.json()).unwrap_or_else(|e| {
                        panic!("locales/{}.json is not valid: {e}", lang.tag())
                    });
                    (lang, table)
                })
                .collect()
        });
        &tables[&self]
    }

    /// The string of `key` in this language, or in English, or the key itself.
    pub(crate) fn tr(self, key: &str) -> &'static str {
        self.table()
            .get(key)
            .or_else(|| Lang::En.table().get(key))
            .map(String::as_str)
            .unwrap_or_else(|| {
                // A missing key is a programming error; show it rather than nothing.
                Box::leak(key.to_string().into_boxed_str())
            })
    }
}

impl std::hash::Hash for Lang {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_the_same_keys_as_english() {
        let english: HashMap<String, String> = serde_json::from_str(Lang::En.json()).unwrap();
        for lang in Lang::ALL {
            let table: HashMap<String, String> = serde_json::from_str(lang.json()).unwrap();
            let missing: Vec<_> = english.keys().filter(|k| !table.contains_key(*k)).collect();
            let extra: Vec<_> = table.keys().filter(|k| !english.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{}: missing {missing:?}", lang.tag());
            assert!(extra.is_empty(), "{}: extra {extra:?}", lang.tag());
        }
    }

    #[test]
    fn placeholders_match_english() {
        fn holes(s: &str) -> Vec<&str> {
            let mut out: Vec<_> = s
                .match_indices('{')
                .filter_map(|(i, _)| s[i..].find('}').map(|j| &s[i..=i + j]))
                .collect();
            out.sort();
            out
        }
        for lang in Lang::ALL {
            for (key, english) in Lang::En.table() {
                assert_eq!(holes(english), holes(lang.tr(key)), "{}: {key}", lang.tag());
            }
        }
    }

    #[test]
    fn tags_and_fallbacks() {
        assert_eq!(Lang::from_tag("es-MX"), Some(Lang::Es));
        assert_eq!(Lang::from_tag("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::from_tag("fr"), None);
        assert_eq!(Lang::detect(Some("es")), Lang::Es);
        assert_eq!(Lang::detect(Some("xx")), Lang::detect(None));
        assert_eq!(Lang::Es.tr("nav.play"), "Jugar");
        assert_eq!(Lang::Es.tr("no.such.key"), "no.such.key");
    }
}
