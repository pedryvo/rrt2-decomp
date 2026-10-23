//! The stock ticker of the glass dome (docs/reference/screens.md C8; `stockdtl.win`, manual
//! chapter 8): a band of quotes (company, price, change) that scrolls across the top of the
//! Market screen. The scroll position is the only animation state; it is a counter of
//! wall-clock milliseconds fed by `AppState::update`, so the band moves while the game is
//! paused too, and it never touches the simulation.

use std::sync::atomic::{AtomicU32, Ordering};

use rt2_ui::{Align, Anchor, Color, Fill, Kind, Layout, NodeId, Ui, game_font};

use super::info_ui::InfoScreen;
use super::{AppState, Screen};

/// Scroll position in milliseconds of wall time (f32 bits).
static TICKER_MS: AtomicU32 = AtomicU32::new(0);
/// Characters per second the band scrolls.
const SPEED_CPS: f32 = 7.0;
/// The band is redrawn this often while it is visible.
const REDRAW_MS: f32 = 140.0;
const GAP: &str = "   *   ";

fn now_ms() -> f32 {
    f32::from_bits(TICKER_MS.load(Ordering::Relaxed))
}

/// The scrolling text for `items`: `NAME 48 +0.50`, joined, `width` characters from the
/// scroll offset (wrapping).
pub(super) fn ticker_window(items: &[(String, i32, f64)], ms: f32, width: usize) -> String {
    if items.is_empty() || width == 0 {
        return String::new();
    }
    let cycle: Vec<char> = items
        .iter()
        .map(|(n, p, d)| {
            format!(
                "{} ${} {}{:.2}",
                n.to_uppercase(),
                p,
                if *d >= 0.0 { "+" } else { "-" },
                d.abs()
            )
        })
        .collect::<Vec<_>>()
        .join(GAP)
        .chars()
        .chain(GAP.chars())
        .collect();
    let start = ((ms / 1000.0 * SPEED_CPS) as usize) % cycle.len();
    (0..width)
        .map(|i| cycle[(start + i) % cycle.len()])
        .collect()
}

impl AppState {
    /// Advances the ticker; redraws the Market screen when the band moved a character.
    pub(super) fn ticker_advance(&mut self, dt_ms: f32) {
        if !matches!(self.info_screen(), Some(InfoScreen::Stock)) {
            return;
        }
        let before = (now_ms() / REDRAW_MS) as u32;
        let after = now_ms() + dt_ms;
        TICKER_MS.store(after.to_bits(), Ordering::Relaxed);
        if (after / REDRAW_MS) as u32 != before {
            self.rebuild_ui();
        }
    }

    /// Draws the band at `(x, y)` inside `panel`; returns its height.
    pub(super) fn draw_ticker(&self, ui: &mut Ui, panel: NodeId, x: f32, y: f32, w: f32) -> f32 {
        let Some(items) = self.with_info(|i| i.ticker_items()) else {
            return 0.0;
        };
        let h = 28.0;
        let dome = ui.add(
            panel,
            Kind::Panel {
                fill: Fill::Solid(Color(0x10, 0x28, 0x3c, 0xd0)),
            },
            Layout::at(Anchor::TopLeft, w, h).offset(x, y),
        );
        let size = 16.0;
        let width = ((w - 16.0) / (size * 0.62)) as usize;
        let text = ticker_window(&items, now_ms(), width);
        ui.add(
            dome,
            Kind::Label {
                text,
                font: game_font(2).with_size(size),
                color: Color::rgb(0x9a, 0xe0, 0x8a),
                align: Align::Left,
                wrap: false,
            },
            Layout::at(Anchor::TopLeft, w - 12.0, h).offset(8.0, 0.0),
        );
        h + 6.0
    }
}

#[allow(dead_code)]
fn _unused(_: &Screen) {}
