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

impl Face {
    /// Family name as registered in the font database.
    pub fn family(self) -> &'static str {
        match self {
            Face::Copperplate => "Cinzel",
            Face::Gothic => "Libre Franklin",
            Face::Display => "Anton",
            Face::Script => "Dancing Script",
        }
    }
}

static CINZEL: &[u8] = include_bytes!("../../../assets/fonts/Cinzel.ttf");
static FRANKLIN: &[u8] = include_bytes!("../../../assets/fonts/LibreFranklin.ttf");
static ANTON: &[u8] = include_bytes!("../../../assets/fonts/Anton-Regular.ttf");
static SCRIPT: &[u8] = include_bytes!("../../../assets/fonts/DancingScript.ttf");

/// The vendored font files, to load into a font database (no system fonts are used, so the look
/// does not depend on the machine).
pub fn font_sources() -> Vec<fontdb::Source> {
    [CINZEL, FRANKLIN, ANTON, SCRIPT]
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
            let found = db
                .faces()
                .any(|f| f.families.iter().any(|(n, _)| n == face.family()));
            assert!(found, "{face:?}");
        }
    }
}
