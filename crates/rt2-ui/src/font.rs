//! The game's 10 bitmap fonts (`docs/formats/font.md`) mapped to vector faces (docs/ui.md
//! "Fontes"). The faces are free (SIL OFL 1.1) and vendored in `assets/fonts/`.

use glyphon::fontdb;

/// A vendored face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// Cinzel: small-caps style serif, stands in for `copp*` (Copperplate Gothic).
    Copperplate,
    /// Libre Franklin: stands in for `fran13` (Franklin Gothic).
    Gothic,
    /// Anton: condensed display face, stands in for `haet46`/`htra46` (Haettenschweiler).
    Display,
    /// Dancing Script: stands in for `scri11`.
    Script,
}

/// The writing system the UI is shown in. It picks the family of each [`Face`] so that the text
/// has glyphs: Cinzel, Anton and Dancing Script have no Cyrillic, and nothing here has CJK
/// except the Noto subset (docs/ui.md "Idiomas e fontes").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Script {
    #[default]
    Latin,
    Cyrillic,
    Cjk,
}

impl Script {
    /// The script of a BCP 47 locale (`ru` Cyrillic, `zh-*`/`ja`/`ko` CJK, the rest Latin).
    pub fn of_locale(locale: &str) -> Script {
        let lang = locale.split(['-', '_']).next().unwrap_or("");
        match lang {
            "ru" | "uk" | "bg" | "sr" | "be" | "mk" => Script::Cyrillic,
            "zh" | "ja" | "ko" => Script::Cjk,
            _ => Script::Latin,
        }
    }
}

impl Face {
    /// Em-size factor of the family used for `script`, so the substitute has the optical size of
    /// the Latin face: Cormorant SC's small capitals are much smaller than Cinzel's at the same
    /// em, and Noto Sans SC's ideographs read small next to Latin letters.
    pub fn size_factor(self, script: Script) -> f32 {
        match (script, self) {
            (Script::Cyrillic, Face::Copperplate) => 1.3,
            (Script::Cyrillic, Face::Display) => 0.95,
            (Script::Cjk, _) => 1.12,
            _ => 1.0,
        }
    }

    /// Latin family name as registered in the font database.
    pub fn family(self) -> &'static str {
        self.family_for(Script::Latin)
    }

    /// Family name for `script`. Every Latin text keeps its own look; Cyrillic and CJK use faces
    /// of a similar style that cover the script (the glyph fallback of cosmic-text fills the
    /// rest, for example Latin letters inside Chinese text).
    pub fn family_for(self, script: Script) -> &'static str {
        match (script, self) {
            (Script::Cjk, _) => "Noto Sans SC",
            (Script::Cyrillic, Face::Copperplate) => "Cormorant SC",
            (Script::Cyrillic, Face::Display) => "Oswald",
            (Script::Cyrillic, Face::Script) => "Marck Script",
            (_, Face::Copperplate) => "Cinzel",
            (_, Face::Gothic) => "Libre Franklin",
            (_, Face::Display) => "Anton",
            (_, Face::Script) => "Dancing Script",
        }
    }
}

static CINZEL: &[u8] = include_bytes!("../../../assets/fonts/Cinzel.ttf");
static FRANKLIN: &[u8] = include_bytes!("../../../assets/fonts/LibreFranklin.ttf");
static ANTON: &[u8] = include_bytes!("../../../assets/fonts/Anton-Regular.ttf");
static SCRIPT: &[u8] = include_bytes!("../../../assets/fonts/DancingScript.ttf");
static CORMORANT: &[u8] = include_bytes!("../../../assets/fonts/CormorantSC-subset.ttf");
static OSWALD: &[u8] = include_bytes!("../../../assets/fonts/Oswald.ttf");
static MARCK: &[u8] = include_bytes!("../../../assets/fonts/MarckScript-Regular.ttf");
static NOTO_SC: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-subset.ttf");

/// The vendored font files, to load into a font database (no system fonts are used, so the look
/// does not depend on the machine).
pub fn font_sources() -> Vec<fontdb::Source> {
    [
        CINZEL, FRANKLIN, ANTON, SCRIPT, CORMORANT, OSWALD, MARCK, NOTO_SC,
    ]
    .into_iter()
    .map(|b| fontdb::Source::Binary(std::sync::Arc::new(b)))
    .collect()
}

/// Face + size + text transform of a piece of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontStyle {
    pub face: Face,
    /// Size in logical pixels (em size).
    pub size: f32,
    /// Fold to capitals before drawing (display fonts 9 and 10 do this in the original).
    pub caps: bool,
    /// Draw a 1 px drop shadow (the original's second glyph group, `font.md`).
    pub shadow: bool,
    /// CSS-style weight (400 regular, 700 bold) for faces that have one.
    pub weight: u16,
}

impl FontStyle {
    pub const fn new(face: Face, size: f32) -> FontStyle {
        FontStyle {
            face,
            size,
            caps: false,
            shadow: false,
            weight: 400,
        }
    }

    pub const fn with_shadow(mut self) -> FontStyle {
        self.shadow = true;
        self
    }

    pub const fn with_weight(mut self, weight: u16) -> FontStyle {
        self.weight = weight;
        self
    }

    pub const fn with_size(mut self, size: f32) -> FontStyle {
        self.size = size;
        self
    }

    /// Line height in logical pixels.
    pub fn line_height(&self) -> f32 {
        (self.size * 1.25).round()
    }
}

/// Original line-box height (`FontSpec::height`) of fonts 1..=10.
pub const ORIGINAL_HEIGHT: [f32; 10] = [6.0, 8.0, 14.0, 24.0, 10.0, 12.0, 11.0, 13.0, 46.0, 46.0];

/// Vector style for the original font `n` (1..=10; 0 means 2, the default of the controls, as in
/// `ui.md`). Sizes are `original height x factor` with a per-face factor that gives the same
/// optical x-height / cap height as the bitmap at 1024x768 scaled to a 1.5x logical canvas, and
/// never below 11 logical pixels so the smallest fonts stay legible.
pub fn game_font(n: u8) -> FontStyle {
    let n = if n == 0 { 2 } else { n.min(10) };
    let h = ORIGINAL_HEIGHT[usize::from(n) - 1];
    match n {
        1..=6 => FontStyle::new(Face::Copperplate, (h * 1.65).max(11.0).round()).with_weight(700),
        7 => FontStyle::new(Face::Script, (h * 1.7).round()),
        8 => FontStyle::new(Face::Gothic, (h * 1.3).round()),
        _ => FontStyle {
            caps: true,
            ..FontStyle::new(Face::Display, (h * 1.15).round())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ten_fonts_map_and_grow_with_the_original() {
        assert_eq!(game_font(0), game_font(2));
        let copp: Vec<f32> = [1u8, 2, 5, 6, 3, 4]
            .iter()
            .map(|&n| game_font(n).size)
            .collect();
        assert!(copp.windows(2).all(|w| w[0] <= w[1]), "{copp:?}");
        assert_eq!(game_font(7).face, Face::Script);
        assert_eq!(game_font(8).face, Face::Gothic);
        assert!(game_font(9).caps && game_font(10).caps);
        assert!(game_font(1).size >= 11.0);
    }

    #[test]
    fn vendored_fonts_register_their_family_names() {
        let mut db = fontdb::Database::new();
        for s in font_sources() {
            db.load_font_source(s);
        }
        for face in [Face::Copperplate, Face::Gothic, Face::Display, Face::Script] {
            for script in [Script::Latin, Script::Cyrillic, Script::Cjk] {
                let want = face.family_for(script);
                let found = db
                    .faces()
                    .any(|f| f.families.iter().any(|(n, _)| n == want));
                assert!(found, "{face:?} {script:?} {want}");
            }
        }
    }

    #[test]
    fn locales_map_to_scripts() {
        assert_eq!(Script::of_locale("ru"), Script::Cyrillic);
        assert_eq!(Script::of_locale("zh-CN"), Script::Cjk);
        assert_eq!(Script::of_locale("pt-BR"), Script::Latin);
    }
}
