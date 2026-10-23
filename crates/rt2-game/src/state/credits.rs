//! A8 Credits (`credits.win`: the `credits.imb` photograph, a full-screen area 4501 and the
//! footer label 4502; text from `credits2.lng`, docs/reference/screens.md). The text scrolls up
//! by itself; the wheel and the arrow keys move it, Esc or Back return to the main menu.
//!
//! `credits2.lng` is plain lines; a line `{[c11][u1]Text}` is a heading (colour 11, underlined)
//! and `{[c11]Text}` a sub-heading, the first lines before "The Crew:" are the title.

use rt2_ui::{Align, Anchor, Color, Fill, Fit, Kind, Layout, NodeId, Ui};
use winit::keyboard::KeyCode;

use super::menus::Page;
use super::{AppState, Screen};

const CREAM: Color = Color::rgb(0xf2, 0xe9, 0xd0);
const GOLD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
/// Scroll speed in design pixels per second.
const SPEED: f32 = 34.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CLine {
    Blank,
    Title(String),
    Head(String),
    Sub(String),
    Text(String),
}

impl CLine {
    /// Height of the line in design pixels.
    fn height(&self) -> f32 {
        match self {
            CLine::Blank => 14.0,
            CLine::Title(_) => 46.0,
            CLine::Head(_) => 34.0,
            CLine::Sub(_) => 26.0,
            CLine::Text(_) => 21.0,
        }
    }
}

/// Parses `credits2.lng` into display lines.
pub(super) fn parse_credits(text: &str) -> Vec<CLine> {
    let mut out = Vec::new();
    let mut title = true;
    for raw in text.lines() {
        let l = raw.trim_end_matches(['\r', '\t', ' ']);
        if l.is_empty() {
            out.push(CLine::Blank);
            continue;
        }
        if let Some(inner) = l.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            title = false;
            let head = inner.contains("[u1]");
            let mut s = inner;
            while let Some(rest) = s.strip_prefix('[') {
                match rest.split_once(']') {
                    Some((_, r)) => s = r,
                    None => break,
                }
            }
            out.push(if head {
                CLine::Head(s.to_owned())
            } else {
                CLine::Sub(s.to_owned())
            });
        } else if title && !l.ends_with(':') {
            out.push(CLine::Title(l.to_owned()));
        } else {
            title = false;
            out.push(CLine::Text(l.to_owned()));
        }
    }
    out
}

/// Credits scroll state.
#[derive(Debug, Default)]
pub(super) struct CreditsState {
    /// Design pixels the text has moved up; negative = still below the window.
    pub scroll: f32,
    pub lines: std::cell::OnceCell<Vec<CLine>>,
}

impl AppState {
    pub(super) fn credits_lines(&self) -> &[CLine] {
        self.ga.credits.lines.get_or_init(|| {
            std::fs::read(self.install.join("credits2.lng"))
                .map(|b| parse_credits(&String::from_utf8_lossy(&b)))
                .unwrap_or_default()
        })
    }

    pub(super) fn credits_open(&self) -> bool {
        matches!(&self.screen, Screen::Menu(m) if m.page == Page::Credits)
    }

    fn credits_total(&self) -> f32 {
        self.credits_lines().iter().map(CLine::height).sum()
    }

    /// Window height of the text area in design pixels, as `build_credits` lays it out.
    fn credits_view(&self) -> f32 {
        let ch = self.viewport.1 as f32 / self.scale.factor();
        (ch - 70.0).max(120.0)
    }

    /// Per-frame scrolling; rebuilds the page because the lines are positioned by the builder.
    pub(super) fn credits_tick(&mut self, dt_ms: f32) {
        if !self.credits_open() {
            return;
        }
        let total = self.credits_total();
        let view = self.credits_view();
        let s = &mut self.ga.credits;
        s.scroll += SPEED * dt_ms / 1000.0;
        if s.scroll > total + 40.0 {
            s.scroll = -view;
        }
        self.rebuild_ui();
    }

    pub(super) fn credits_scroll(&mut self, delta: f32) {
        let total = self.credits_total();
        let view = self.credits_view();
        let s = &mut self.ga.credits;
        s.scroll = (s.scroll + delta).clamp(-view, total + 40.0);
        self.rebuild_ui();
    }

    pub(super) fn credits_key(&mut self, code: KeyCode) -> bool {
        if !self.credits_open() {
            return false;
        }
        let d = match code {
            KeyCode::ArrowUp => -40.0,
            KeyCode::ArrowDown => 40.0,
            KeyCode::PageUp => -300.0,
            KeyCode::PageDown => 300.0,
            KeyCode::Home => -1.0e6,
            _ => return false,
        };
        self.credits_scroll(d);
        true
    }

    /// Opening the page starts the roll from below.
    pub(super) fn credits_reset(&mut self) {
        let view = self.credits_view();
        self.ga.credits.scroll = -view;
    }

    pub(super) fn build_credits(&self, ui: &mut Ui, layer: NodeId, cw: f32, ch: f32, z: f32) {
        let t = &self.texts;
        if let Some(bg) = self.dialog_art.credits_bg {
            ui.add(
                layer,
                Kind::Image {
                    sprite: bg,
                    fit: Fit::CoverTop,
                },
                Layout::fill(),
            );
        }
        let pw = (cw - 24.0).min(640.0 * z);
        let px = (cw - pw) / 2.0;
        let view = (ch - 70.0).max(120.0);
        ui.add(
            layer,
            Kind::Panel {
                fill: Fill::Solid(Color(0x0a, 0x0c, 0x0a, 0xb8)),
            },
            Layout::at(Anchor::TopLeft, pw, view).offset(px, 0.0),
        );
        let scroll = self.ga.credits.scroll;
        let mut y = -scroll;
        for line in self.credits_lines() {
            let h = line.height();
            let (text, font, color) = match line {
                CLine::Blank => {
                    y += h;
                    continue;
                }
                CLine::Title(s) => (s, super::menus::mfont(4, z * 0.8).with_shadow(), GOLD),
                CLine::Head(s) => (s, super::menus::mfont(3, z * 0.75).with_shadow(), GOLD),
                CLine::Sub(s) => (s, super::menus::mfont(2, z * 0.95), CREAM),
                CLine::Text(s) => (s, super::menus::mfont(1, z * 0.85), CREAM),
            };
            let (top, bottom) = (y, y + h);
            y += h;
            if top < 0.0 || bottom > view {
                continue;
            }
            ui.add(
                layer,
                Kind::Label {
                    text: text.clone(),
                    font,
                    color,
                    align: Align::Center,
                    wrap: false,
                },
                Layout::at(Anchor::TopLeft, pw - 16.0, h).offset(px + 8.0, top),
            );
        }
        ui.add(
            layer,
            Kind::Button {
                text: t.get(crate::text::id::CANCEL, "Cancel"),
                font: super::menus::mfont(1, z),
                action: super::A_BACK,
                enabled: true,
                hotkey: Some('c'),
            },
            Layout::at(Anchor::Bottom, 130.0 * z, 34.0 * z).offset(0.0, 16.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_markup_becomes_heads_and_text() {
        let l = parse_credits(
            "Railroad Tycoon II\nPlatinum Edition\n\nThe Crew:\n\n{[c11][u1]Development}\nPopTop\n{[c11]Design}\nPhil\n",
        );
        assert_eq!(l[0], CLine::Title("Railroad Tycoon II".into()));
        assert_eq!(l[1], CLine::Title("Platinum Edition".into()));
        assert_eq!(l[3], CLine::Text("The Crew:".into()));
        assert_eq!(l[5], CLine::Head("Development".into()));
        assert_eq!(l[6], CLine::Text("PopTop".into()));
        assert_eq!(l[7], CLine::Sub("Design".into()));
    }
}
