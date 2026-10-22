//! UI window layouts (`*.win`), see `docs/formats/ui.md`.
//!
//! Loader: `0x507600` (`ui_window_load`). The file is a stream of raw structs separated by 4-byte
//! markers, all little-endian:
//!
//! ```text
//! u32 0x7d0                          window start
//! u8[65]                             window struct (size, client rectangle, flags, id base)
//! u32 0xbbe, u32 len, u8[len]        background bank name (empty in all shipped files)
//! u32 0x7d1                          controls start
//! repeat:
//!   u32 0x7d2                        control start (0x7d4 ends the window instead)
//!   u32 type                         1 button, 2 label, 4 image, 8 list, ...
//!   u32 0xbbf, len, bytes            runtime string (empty in files)
//!   u32 0xbb8, len, bytes            runtime string (empty in files)
//!   u32 0xbb9, len, bytes            sprite bank (LAD `.imb`) name, may be empty
//!   u8[N(type)]                      control struct: 53-byte common base + type-specific tail
//!   [u32 tag, len, bytes]            extra string for types 1, 2, 8 and 0x100
//!   u32 0x7d3                        control end
//! u32 0x7d4                          window end
//! ```

use std::fmt::Write as _;

pub const WINDOW_START: u32 = 0x7d0;
pub const CONTROLS_START: u32 = 0x7d1;
pub const CONTROL_START: u32 = 0x7d2;
pub const CONTROL_END: u32 = 0x7d3;
pub const WINDOW_END: u32 = 0x7d4;
pub const WINDOW_STRUCT_LEN: usize = 65;
/// Length of the struct shared by all control types.
pub const BASE_LEN: usize = 53;

const TAG_BACKGROUND: u32 = 0xbbe;
const TAG_RUNTIME_A: u32 = 0xbbf;
const TAG_RUNTIME_B: u32 = 0xbb8;
const TAG_BANK: u32 = 0xbb9;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UiError {
    #[error("window file is truncated at offset {0}")]
    Truncated(usize),
    #[error("expected marker {expected:#x} at offset {at}, found {found:#x}")]
    BadMarker {
        expected: u32,
        found: u32,
        at: usize,
    },
    #[error("unknown control type {0:#x} at offset {1}")]
    UnknownControl(u32, usize),
}

/// Control classes, keyed by the type bit used in the file (constructors listed in `ui.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// 1: push button, two sprite frames (up, down), hotkey, label rectangle (`0x50c850`).
    Button,
    /// 2: text label drawn with the control's font (`0x509960`).
    Label,
    /// 4: sprite image (`0x50afe0`).
    Image,
    /// 8: list box with scroll bar sprites (`0x5047f0`).
    List,
    /// 0x10: invisible hot area (`0x508480`, base struct only).
    HotArea,
    /// 0x20: base-struct-plus-16 control (`0x5088f0`), purpose unknown.
    Type20,
    /// 0x40: movie surface (only in the `smackwi*.win` files, `0x503790`).
    Movie,
    /// 0x80: bar/scroll element with a value range (`0x50a890`).
    Bar,
    /// 0x100: custom/owner-drawn control with an extra string (`0x507cf0`).
    Custom,
    /// 0x200: slider (`0x50b8d0`, bank `horsb_*.imb`).
    Slider,
}

impl ControlKind {
    pub fn from_type(t: u32) -> Option<Self> {
        Some(match t {
            0x1 => Self::Button,
            0x2 => Self::Label,
            0x4 => Self::Image,
            0x8 => Self::List,
            0x10 => Self::HotArea,
            0x20 => Self::Type20,
            0x40 => Self::Movie,
            0x80 => Self::Bar,
            0x100 => Self::Custom,
            0x200 => Self::Slider,
            _ => return None,
        })
    }

    pub fn type_id(self) -> u32 {
        match self {
            Self::Button => 0x1,
            Self::Label => 0x2,
            Self::Image => 0x4,
            Self::List => 0x8,
            Self::HotArea => 0x10,
            Self::Type20 => 0x20,
            Self::Movie => 0x40,
            Self::Bar => 0x80,
            Self::Custom => 0x100,
            Self::Slider => 0x200,
        }
    }

    /// Size of the raw control struct, base included (the `malloc`/read size in the constructor).
    pub fn struct_len(self) -> usize {
        match self {
            Self::Button => 0x52,
            Self::Label => 0x4d,
            Self::Image => 0x3f,
            Self::List => 0x91,
            Self::HotArea => 0x35,
            Self::Type20 => 0x45,
            Self::Movie => 0x45,
            Self::Bar => 0x4f,
            Self::Custom => 0x61,
            Self::Slider => 0x55,
        }
    }

    /// Tag of the extra string that follows the struct, if the type has one.
    pub fn extra_tag(self) -> Option<u32> {
        match self {
            Self::Button => Some(0xbba),
            Self::Label => Some(0xbbb),
            Self::List => Some(0xbc0),
            Self::Custom => Some(0xbc1),
            _ => None,
        }
    }
}

/// Rectangle in original (1024x768 design) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub kind: ControlKind,
    /// Action/command id (`base[0..2]`): dispatched by the screen's message handler and the key
    /// of `ui_find_control` (`0x507000`).
    pub id: u16,
    /// Draw order (`base[4]`, ascending; the loader inserts sorted by it).
    pub order: u8,
    /// Group/link id shared by controls that belong together (`base[5..7]`), 0 if none.
    pub group: u16,
    /// Position relative to the window (`base[7..15]`).
    pub rect: Rect,
    /// Font number 1..=10 (`base[35]`, 0 means 2), see `font.rs`.
    pub font: u8,
    /// Sprite bank (`.imb`) name from field `0xbb9`, if any.
    pub bank: String,
    /// The 53 common bytes, kept raw (undecoded parts: flags, a runtime pointer at `base[45..49]`).
    pub base: [u8; BASE_LEN],
    /// Type-specific bytes after the common base.
    pub tail: Vec<u8>,
    /// Extra string of types 1, 2, 8, 0x100 (the list's fill bank for type 8).
    pub extra: Option<String>,
}

impl Control {
    pub fn tail_u16(&self, at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            self.tail.get(at..at + 2)?.try_into().ok()?,
        ))
    }

    pub fn tail_u32(&self, at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.tail.get(at..at + 4)?.try_into().ok()?,
        ))
    }

    /// Buttons: sprite records (in `bank`) of the up and down frames (`tail[0..2]`, `tail[4..6]`).
    pub fn button_frames(&self) -> Option<(u16, u16)> {
        (self.kind == ControlKind::Button)
            .then(|| Some((self.tail_u16(0)?, self.tail_u16(4)?)))
            .flatten()
    }

    /// Buttons: hotkey character (`tail[6..8]`), 0 if none.
    pub fn button_hotkey(&self) -> Option<u8> {
        (self.kind == ControlKind::Button)
            .then(|| self.tail_u16(6).map(|v| v as u8))
            .flatten()
    }

    /// Buttons: label rectangle inside the button (`tail[14..22]`, four u16).
    pub fn button_label_rect(&self) -> Option<Rect> {
        if self.kind != ControlKind::Button {
            return None;
        }
        Some(Rect {
            x: self.tail_u16(14)?,
            y: self.tail_u16(16)?,
            w: self.tail_u16(18)?,
            h: self.tail_u16(20)?,
        })
    }

    /// Images: sprite record of the bank to show (`tail[0..2]`).
    pub fn image_record(&self) -> Option<u16> {
        (self.kind == ControlKind::Image)
            .then(|| self.tail_u16(0))
            .flatten()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// Window size (`+9`, `+0xd`); 1024x768 for full-screen screens.
    pub width: u32,
    pub height: u32,
    /// Client area inside the frame (`+0x21..+0x2f`); zero for full-screen windows.
    pub client: (u32, u32, u32, u32),
    /// 1 for framed dialogs (centred, with client area), 0 for full-screen layers (`+0x31`).
    pub framed: u32,
    /// Window number / base id (`+0x35`), e.g. 4700 for the campaign screen (0 for dialogs).
    pub id_base: u32,
    /// The 65 raw struct bytes.
    pub raw: [u8; WINDOW_STRUCT_LEN],
    pub background: String,
    pub controls: Vec<Control>,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], UiError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.data.len())
            .ok_or(UiError::Truncated(self.at))?;
        let out = &self.data[self.at..end];
        self.at = end;
        Ok(out)
    }

    fn u32(&mut self) -> Result<u32, UiError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn expect(&mut self, marker: u32) -> Result<(), UiError> {
        let at = self.at;
        let found = self.u32()?;
        if found == marker {
            Ok(())
        } else {
            Err(UiError::BadMarker {
                expected: marker,
                found,
                at,
            })
        }
    }

    /// `[u32 tag][u32 len][bytes]` (`0x4f4220`), decoded as Latin-1 without the trailing NUL.
    fn field(&mut self, tag: u32) -> Result<String, UiError> {
        self.expect(tag)?;
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        Ok(bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect())
    }
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

impl Window {
    pub fn parse(data: &[u8]) -> Result<Self, UiError> {
        let mut r = Reader { data, at: 0 };
        r.expect(WINDOW_START)?;
        let raw: [u8; WINDOW_STRUCT_LEN] = r.take(WINDOW_STRUCT_LEN)?.try_into().unwrap();
        let background = r.field(TAG_BACKGROUND)?;
        r.expect(CONTROLS_START)?;
        let mut controls = Vec::new();
        loop {
            let at = r.at;
            match r.u32()? {
                WINDOW_END => break,
                CONTROL_START => {}
                found => {
                    return Err(UiError::BadMarker {
                        expected: CONTROL_START,
                        found,
                        at,
                    });
                }
            }
            let type_at = r.at;
            let t = r.u32()?;
            let kind = ControlKind::from_type(t).ok_or(UiError::UnknownControl(t, type_at))?;
            r.field(TAG_RUNTIME_A)?;
            r.field(TAG_RUNTIME_B)?;
            let bank = r.field(TAG_BANK)?;
            let st = r.take(kind.struct_len())?;
            let extra = kind.extra_tag().map(|tag| r.field(tag)).transpose()?;
            r.expect(CONTROL_END)?;
            let base: [u8; BASE_LEN] = st[..BASE_LEN].try_into().unwrap();
            controls.push(Control {
                kind,
                id: u16_at(&base, 0),
                order: base[4],
                group: u16_at(&base, 5),
                rect: Rect {
                    x: u16_at(&base, 7),
                    y: u16_at(&base, 9),
                    w: u16_at(&base, 11),
                    h: u16_at(&base, 13),
                },
                font: base[35],
                bank,
                base,
                tail: st[BASE_LEN..].to_vec(),
                extra,
            });
        }
        Ok(Self {
            width: u32_at(&raw, 9),
            height: u32_at(&raw, 13),
            client: (
                u32_at(&raw, 33),
                u32_at(&raw, 37),
                u32_at(&raw, 41),
                u32_at(&raw, 45),
            ),
            framed: u32_at(&raw, 49),
            id_base: u32_at(&raw, 53),
            raw,
            background,
            controls,
        })
    }

    /// One line per control, for dumps.
    pub fn describe(&self) -> String {
        let mut s = format!(
            "window {}x{} client {:?} framed {} id_base {}\n",
            self.width, self.height, self.client, self.framed, self.id_base
        );
        for c in &self.controls {
            let _ = writeln!(
                s,
                "{:?} id {} z {} rect {},{} {}x{} font {} bank {:?}",
                c.kind, c.id, c.order, c.rect.x, c.rect.y, c.rect.w, c.rect.h, c.font, c.bank
            );
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(tag: u32, text: &[u8]) -> Vec<u8> {
        let mut o = tag.to_le_bytes().to_vec();
        o.extend((text.len() as u32).to_le_bytes());
        o.extend(text);
        o
    }

    fn sample() -> Vec<u8> {
        let mut o = WINDOW_START.to_le_bytes().to_vec();
        let mut w = [0u8; WINDOW_STRUCT_LEN];
        w[9..13].copy_from_slice(&1024u32.to_le_bytes());
        w[13..17].copy_from_slice(&768u32.to_le_bytes());
        o.extend(w);
        o.extend(field(TAG_BACKGROUND, b""));
        o.extend(CONTROLS_START.to_le_bytes());
        o.extend(CONTROL_START.to_le_bytes());
        o.extend(1u32.to_le_bytes());
        o.extend(field(TAG_RUNTIME_A, b""));
        o.extend(field(TAG_RUNTIME_B, b""));
        o.extend(field(TAG_BANK, b"bank.imb\0"));
        let mut st = vec![0u8; ControlKind::Button.struct_len()];
        st[0..2].copy_from_slice(&0x1234u16.to_le_bytes());
        st[7..15].copy_from_slice(&[10, 0, 20, 0, 30, 0, 40, 0]);
        st[35] = 3;
        o.extend(st);
        o.extend(field(0xbba, b""));
        o.extend(CONTROL_END.to_le_bytes());
        o.extend(WINDOW_END.to_le_bytes());
        o
    }

    #[test]
    fn parses_a_button_window() {
        let win = Window::parse(&sample()).unwrap();
        assert_eq!((win.width, win.height), (1024, 768));
        let c = &win.controls[0];
        assert_eq!(c.kind, ControlKind::Button);
        assert_eq!(c.id, 0x1234);
        assert_eq!(
            c.rect,
            Rect {
                x: 10,
                y: 20,
                w: 30,
                h: 40
            }
        );
        assert_eq!((c.font, c.bank.as_str()), (3, "bank.imb"));
        assert_eq!(c.extra.as_deref(), Some(""));
    }

    #[test]
    fn reports_bad_markers_and_truncation() {
        let mut bad = sample();
        bad[0] = 0;
        assert!(matches!(
            Window::parse(&bad),
            Err(UiError::BadMarker { at: 0, .. })
        ));
        let cut = &sample()[..100];
        assert!(matches!(Window::parse(cut), Err(UiError::Truncated(_))));
    }

    #[test]
    fn control_types_round_trip() {
        for t in [1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200] {
            assert_eq!(ControlKind::from_type(t).unwrap().type_id(), t);
        }
        assert!(ControlKind::from_type(3).is_none());
    }
}
