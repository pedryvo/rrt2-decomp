//! Translation files (`assets/lang/<locale>.toml`, `docs/i18n.md`): numbered strings that are
//! overlaid by id on the English `default2.lng`, plus an `[app]` table for the clone's own texts.
//!
//! ```toml
//! [meta]
//! locale = "pt-BR"
//! language = "Português (Brasil)"
//!
//! [strings]      # ids of default2.lng
//! 812 = "passageiros"
//!
//! [app]          # ids >= 100000, the clone's own strings (docs/i18n.md)
//! 100000 = "Menu"
//! ```

use std::collections::BTreeMap;

use crate::lng::Lng;

#[derive(Debug, thiserror::Error)]
pub enum I18nError {
    #[error("invalid translation file: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("[meta] has no `locale`")]
    NoLocale,
    #[error("key {0:?} in [{1}] is not a numeric id")]
    BadId(String, &'static str),
}

/// A parsed translation file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translation {
    /// BCP 47 tag, e.g. `pt-BR`.
    pub locale: String,
    /// The language's own name, shown in the language selector.
    pub language: String,
    /// Overrides of `default2.lng` ids.
    pub strings: BTreeMap<u32, String>,
    /// The clone's own strings (ids 100000+).
    pub app: BTreeMap<u32, String>,
}

impl Translation {
    pub fn parse(text: &str) -> Result<Translation, I18nError> {
        let doc: toml::Table = text.parse()?;
        let meta = doc.get("meta").and_then(toml::Value::as_table);
        let field = |k: &str| {
            meta.and_then(|m| m.get(k))
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        };
        let locale = field("locale").ok_or(I18nError::NoLocale)?;
        let language = field("language").unwrap_or_else(|| locale.clone());
        let table = |name: &'static str| -> Result<BTreeMap<u32, String>, I18nError> {
            let mut out = BTreeMap::new();
            if let Some(t) = doc.get(name).and_then(toml::Value::as_table) {
                for (k, v) in t {
                    let id: u32 = k.parse().map_err(|_| I18nError::BadId(k.clone(), name))?;
                    if let Some(s) = v.as_str() {
                        out.insert(id, s.to_owned());
                    }
                }
            }
            Ok(out)
        };
        Ok(Translation {
            locale,
            language,
            strings: table("strings")?,
            app: table("app")?,
        })
    }
}

impl Lng {
    /// Replaces the strings of `tr` that exist in this table (ids unknown to the English file
    /// are ignored). Returns the ids that `tr` does not translate, in order: those stay English.
    pub fn overlay(&mut self, tr: &Translation) -> Vec<u32> {
        let mut missing = Vec::new();
        for (id, text) in &mut self.strings {
            match tr.strings.get(id) {
                Some(t) => text.clone_from(t),
                None => missing.push(*id),
            }
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "[meta]\nlocale = \"xx\"\nlanguage = \"Xish\"\n[strings]\n7 = \"sete\"\n99 = \"unknown id\"\n[app]\n100000 = \"Menu-x\"\n[glossary]\n\"a\" = \"b\"\n";

    #[test]
    fn parses_meta_strings_and_app() {
        let t = Translation::parse(FILE).unwrap();
        assert_eq!((t.locale.as_str(), t.language.as_str()), ("xx", "Xish"));
        assert_eq!(t.strings[&7], "sete");
        assert_eq!(t.app[&100_000], "Menu-x");
    }

    #[test]
    fn overlay_replaces_by_id_and_reports_missing() {
        let mut lng = Lng::parse(b"7 \"seven\"\n8 \"eight\"\n").unwrap();
        let missing = lng.overlay(&Translation::parse(FILE).unwrap());
        assert_eq!(lng.get(7), Some("sete"));
        assert_eq!(lng.get(8), Some("eight"));
        assert_eq!(lng.get(99), None);
        assert_eq!(missing, vec![8]);
    }

    #[test]
    fn rejects_missing_locale_and_bad_ids() {
        assert!(matches!(
            Translation::parse("[strings]\n"),
            Err(I18nError::NoLocale)
        ));
        assert!(matches!(
            Translation::parse("[meta]\nlocale=\"a\"\n[strings]\nfoo = \"x\"\n"),
            Err(I18nError::BadId(..))
        ));
    }
}
