//! UI texts from `default2.lng` where the ids are known; English placeholders otherwise
//! (marked TODO at each use, docs/ui.md "Textos").

use std::path::Path;

use rt2_formats::i18n::Translation;
use rt2_formats::lng::Lng;

/// The English game strings from the install, overlaid by the current language, plus the
/// clone's own texts (`app_strings`). Switching the language rebuilds the overlay from the
/// untouched English table, so it can go back and forth.
pub struct Texts {
    english: Option<Lng>,
    lng: Option<Lng>,
    locales: Vec<Translation>,
    locale: String,
    /// The `[app]` table of the current language (empty for English).
    app: std::collections::BTreeMap<u32, String>,
}

/// Known ids (`docs/formats/lng.md`; checked against the shipped `default2.lng`).
pub mod id {
    pub const CASH: u32 = 31;
    pub const SPEED: u32 = 202;
    pub const PAUSED: u32 = 991;
    pub const OK: u32 = 451;
    pub const CANCEL: u32 = 454;
    pub const QUIT: u32 = 1684;
    pub const NEW_GAME: u32 = 1686;
    pub const LOAD_GAME: u32 = 1688;
    pub const FINANCES: u32 = 111;
    pub const TRAINS: u32 = 978;
    pub const CITIES: u32 = 103;
    pub const NEXT: u32 = 301;
    pub const PREVIOUS: u32 = 303;
    /// January; the 12 months are consecutive.
    pub const MONTH_FIRST: u32 = 1441;
    /// January abbreviated; also consecutive.
    pub const MONTH_SHORT_FIRST: u32 = 1426;
}

impl Texts {
    /// The install's English table with every translation of `locales`; `locale` is chosen with
    /// [`crate::locales::pick`] semantics (unknown means English).
    pub fn load(install: &Path, locales: Vec<Translation>, locale: &str) -> Texts {
        let english = std::fs::read(install.join("default2.lng"))
            .ok()
            .and_then(|raw| Lng::parse(&raw).ok());
        Texts::from_parts(english, locales, locale)
    }

    pub fn from_parts(english: Option<Lng>, locales: Vec<Translation>, locale: &str) -> Texts {
        let mut t = Texts {
            english,
            lng: None,
            locales,
            locale: crate::locales::ENGLISH.to_owned(),
            app: Default::default(),
        };
        t.set_locale(locale);
        t
    }

    #[cfg(test)]
    pub fn none() -> Texts {
        Texts::from_parts(None, Vec::new(), "en")
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Available languages as `(locale, native name)`, English first.
    pub fn languages(&self) -> Vec<(String, String)> {
        let mut out = vec![(
            crate::locales::ENGLISH.to_owned(),
            crate::locales::ENGLISH_NAME.to_owned(),
        )];
        out.extend(
            self.locales
                .iter()
                .filter(|t| t.locale != crate::locales::ENGLISH)
                .map(|t| (t.locale.clone(), t.language.clone())),
        );
        out
    }

    /// Switches the language. Ids a translation lacks stay English; that is logged once per
    /// switch. An unknown locale is English.
    pub fn set_locale(&mut self, locale: &str) {
        let picked = crate::locales::pick(locale, &self.locales);
        let tr = self.locales.iter().find(|t| t.locale == picked);
        let mut lng = self.english.as_ref().map(|e| Lng {
            strings: e.strings.clone(),
            tables: e.tables.clone(),
        });
        self.app.clear();
        if let Some(tr) = tr {
            if let Some(l) = &mut lng {
                let missing = l.overlay(tr);
                if !missing.is_empty() {
                    eprintln!(
                        "i18n: {}: {} of {} ids are not translated and stay English (first: {:?})",
                        tr.locale,
                        missing.len(),
                        l.strings.len(),
                        &missing[..missing.len().min(5)]
                    );
                }
            }
            self.app.clone_from(&tr.app);
            let n = crate::app_strings::APP_EN.len();
            let lacking = (0..n as u32)
                .filter(|i| !tr.app.contains_key(&(crate::app_strings::APP_FIRST + i)))
                .count();
            if lacking > 0 {
                eprintln!(
                    "i18n: {}: {lacking} of {n} [app] strings are not translated and stay English",
                    tr.locale
                );
            }
        }
        self.lng = lng;
        self.locale = picked;
    }

    /// The clone's own text for an English default (`app_strings`), translated if the language
    /// has it. An English text missing from the table is a programming error (caught in debug).
    pub fn a(&self, english: &str) -> String {
        match crate::app_strings::id_of(english) {
            Some(id) => self
                .app
                .get(&id)
                .map_or_else(|| english.to_owned(), Clone::clone),
            None => {
                debug_assert!(false, "app string missing from app_strings.rs: {english:?}");
                english.to_owned()
            }
        }
    }

    /// [`Texts::a`] with `%1`..`%9` replaced by `args`.
    pub fn af(&self, english: &str, args: &[String]) -> String {
        subst(&self.a(english), args)
    }

    /// The LNG string `id`, else `fallback` (an English placeholder).
    pub fn get(&self, id: u32, fallback: &str) -> String {
        self.lng
            .as_ref()
            .and_then(|l| l.get(id))
            .unwrap_or(fallback)
            .to_owned()
    }

    /// Entry `index` of the named LNG table (`;tNAME_0` markers, lng.md), else `fallback`.
    pub fn table(&self, name: &str, index: u32, fallback: &str) -> String {
        self.lng
            .as_ref()
            .and_then(|l| l.table_entry(name, index))
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }

    pub fn date(&self, month: u8, day: u8, year: impl std::fmt::Display) -> String {
        self.af(
            "%1 %2, %3",
            &[
                self.month(month),
                day.to_string(),
                year.to_string(),
                self.month_short(month),
            ],
        )
    }

    /// Abbreviated month name (LNG 1426.., "Jan"; "1月" in Chinese), the date's `%4`.
    pub fn month_short(&self, month: u8) -> String {
        const EN: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let i = usize::from(month.clamp(1, 12)) - 1;
        self.get(id::MONTH_SHORT_FIRST + i as u32, EN[i])
    }

    pub fn month(&self, month: u8) -> String {
        const EN: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let i = usize::from(month.clamp(1, 12)) - 1;
        self.get(id::MONTH_FIRST + i as u32, EN[i])
    }
}

/// Replaces `%1`..`%9` in `template` (a marker may appear several times or not at all).
pub fn subst(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%'
            && let Some(d) = chars.peek().and_then(|d| d.to_digit(10))
            && d >= 1
        {
            chars.next();
            if let Some(a) = args.get(d as usize - 1) {
                out.push_str(a);
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_without_a_language_file() {
        let t = Texts::none();
        assert_eq!(t.get(id::QUIT, "Quit"), "Quit");
        assert_eq!(t.month(3), "March");
        assert_eq!(t.month(0), "January");
        assert_eq!(t.date(3, 7, 1880), "March 7, 1880");
    }

    const FIXTURE: &str = "[meta]\nlocale = \"xx\"\nlanguage = \"Xish\"\n[strings]\n31 = \"Caixa\"\n1441 = \"Janeiro\"\n[app]\n100000 = \"Cardapio\"\n100007 = \"%2 de %1, %3\"\n";

    fn fixture() -> Texts {
        let english = Lng::parse(b"31 \"Cash\"\n202 \"Speed:\"\n1441 \"January\"\n").unwrap();
        Texts::from_parts(
            Some(english),
            vec![Translation::parse(FIXTURE).unwrap()],
            "xx",
        )
    }

    #[test]
    fn a_translation_overlays_by_id_and_missing_ids_stay_english() {
        let t = fixture();
        assert_eq!(t.get(id::CASH, "Cash"), "Caixa");
        assert_eq!(t.get(id::SPEED, "Speed:"), "Speed:");
        assert_eq!(t.month(1), "Janeiro");
        assert_eq!(t.a("Menu"), "Cardapio");
        assert_eq!(t.a("Save"), "Save");
    }

    #[test]
    fn the_language_switches_back_and_forth() {
        let mut t = fixture();
        t.set_locale("en");
        assert_eq!(t.get(id::CASH, "?"), "Cash");
        assert_eq!(t.a("Menu"), "Menu");
        t.set_locale("xx");
        assert_eq!(t.get(id::CASH, "?"), "Caixa");
        t.set_locale("nope");
        assert_eq!(t.locale(), "en");
        let langs = t.languages();
        assert_eq!(langs[0], ("en".to_owned(), "English".to_owned()));
        assert_eq!(langs[1], ("xx".to_owned(), "Xish".to_owned()));
    }

    #[test]
    fn app_strings_substitute_markers_in_any_order() {
        assert_eq!(app_strings_id("%1 %2, %3"), 100_007);
        assert_eq!(fixture().date(1, 5, 1880), "5 de Janeiro, 1880");
        assert_eq!(subst("%1+%1 %9", &["a".into()]), "a+a ");
    }

    #[test]
    fn the_date_can_use_the_abbreviated_month() {
        let mut t = fixture();
        t.set_locale("xx");
        // %4 is the abbreviated month (LNG 1426..), English fallback "Jan" here.
        assert_eq!(
            subst(
                "%3 %4 %2",
                &[t.month(1), "5".into(), "1880".into(), t.month_short(1)]
            ),
            "1880 Jan 5"
        );
    }

    fn app_strings_id(s: &str) -> u32 {
        crate::app_strings::id_of(s).unwrap()
    }
}
