//! UI texts from `default2.lng` where the ids are known; English placeholders otherwise
//! (marked TODO at each use, docs/ui.md "Textos").

use std::path::Path;

use rt2_formats::lng::Lng;

pub struct Texts {
    lng: Option<Lng>,
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
    /// January; the 12 months are consecutive.
    pub const MONTH_FIRST: u32 = 1441;
}

impl Texts {
    pub fn load(install: &Path) -> Texts {
        let lng = std::fs::read(install.join("default2.lng"))
            .ok()
            .and_then(|raw| Lng::parse(&raw).ok());
        Texts { lng }
    }

    #[cfg(test)]
    pub fn none() -> Texts {
        Texts { lng: None }
    }

    /// The LNG string `id`, else `fallback` (an English placeholder).
    pub fn get(&self, id: u32, fallback: &str) -> String {
        self.lng
            .as_ref()
            .and_then(|l| l.get(id))
            .unwrap_or(fallback)
            .to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_without_a_language_file() {
        let t = Texts::none();
        assert_eq!(t.get(id::QUIT, "Quit"), "Quit");
        assert_eq!(t.month(3), "March");
        assert_eq!(t.month(0), "January");
    }
}
