//! A9 High Scores (`hiscore.win`: the `hiscore.imb` photograph plus the table the code draws on
//! it, docs/reference/screens.md). The table lives in our own file (`crate::hiscore`); a victory
//! at the end of a scenario adds a row.

use rt2_ui::{Align, Anchor, Color, Fill, Fit, Kind, Layout, NodeId, Ui};

use super::{AppState, Screen};
use crate::hiscore::{self, Entry};

const CREAM: Color = Color::rgb(0xf2, 0xe9, 0xd0);
const GOLD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HILITE: Color = Color::rgb(0x9a, 0xe0, 0x8a);

/// Table state kept by the application.
#[derive(Debug, Default)]
pub(super) struct HiscoreState {
    pub entries: Vec<Entry>,
    /// Row of the score added by the game that just ended.
    pub fresh: Option<usize>,
    /// Where the table is read from and written to (tests); `None` = the user's file.
    pub path: Option<std::path::PathBuf>,
}

impl AppState {
    fn hiscore_read_path(&self) -> Option<std::path::PathBuf> {
        self.ga.hiscores.path.clone().or_else(hiscore::file)
    }

    /// Where a new score may be written: the test path, or the user's file when the window loop
    /// enabled persistence.
    fn hiscore_write_path(&self) -> Option<std::path::PathBuf> {
        self.ga
            .hiscores
            .path
            .clone()
            .or_else(|| self.persist.then(hiscore::file).flatten())
    }

    /// Re-reads the table when the page opens.
    pub(super) fn hiscores_reload(&mut self) {
        let list = self
            .hiscore_read_path()
            .map(|p| hiscore::load(&p))
            .unwrap_or_default();
        self.ga.hiscores.entries = list;
    }

    /// A won scenario goes into the table. `medal`: 1 bronze, 2 silver, 3 gold.
    pub(super) fn record_score(&mut self, medal: u8) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let Some(save) = g.world.save_ref() else {
            return;
        };
        let Some(co) = crate::buildtools::human_company(save) else {
            return;
        };
        let score = crate::buildtools::cash(save, co);
        let d = g.world.date();
        let scenario = g.name.clone();
        let company = self
            .with_info(|i| i.company_name(co as u32))
            .unwrap_or_default();
        let entry = Entry {
            company,
            scenario,
            medal,
            year: d.year,
            month: d.month,
            score: if score.is_finite() { score as i64 } else { 0 },
        };
        let Some(path) = self.hiscore_write_path() else {
            return;
        };
        let mut list = hiscore::load(&path);
        self.ga.hiscores.fresh = hiscore::insert(&mut list, entry);
        let _ = hiscore::save(&path, &list);
        self.ga.hiscores.entries = list;
    }

    pub(super) fn build_hiscores(&self, ui: &mut Ui, layer: NodeId, cw: f32, ch: f32, z: f32) {
        use super::menus::mfont;
        let t = &self.texts;
        if let Some(bg) = self.dialog_art.hiscore_bg {
            ui.add(
                layer,
                Kind::Image {
                    sprite: bg,
                    fit: Fit::CoverTop,
                },
                Layout::fill(),
            );
        }
        let pw = (cw - 24.0).min(820.0 * z);
        let px = (cw - pw) / 2.0;
        let ph = (ch - 70.0).max(160.0);
        let at = |x: f32, y: f32, w: f32, h: f32| Layout::at(Anchor::TopLeft, w, h).offset(x, y);
        ui.add(
            layer,
            Kind::Panel {
                fill: Fill::Solid(Color(0x0a, 0x0c, 0x0a, 0xc4)),
            },
            at(px, 12.0, pw, ph),
        );
        let lab = |text: &str, f, c, a| Kind::Label {
            text: text.to_owned(),
            font: f,
            color: c,
            align: a,
            wrap: false,
        };
        ui.add(
            layer,
            lab(
                &t.get(1680, "High Scores"),
                mfont(3, z * 0.9).with_shadow(),
                GOLD,
                Align::Center,
            ),
            at(px, 20.0, pw, 36.0 * z),
        );
        let narrow = pw < 720.0;
        // Column starts as fractions of the panel width: rank, company, scenario, medal, date,
        // score.
        let cols: &[(f32, f32, Align)] = if narrow {
            &[
                (0.02, 0.08, Align::Left),
                (0.10, 0.42, Align::Left),
                (0.52, 0.18, Align::Left),
                (0.70, 0.28, Align::Right),
            ]
        } else {
            &[
                (0.02, 0.06, Align::Left),
                (0.08, 0.24, Align::Left),
                (0.32, 0.28, Align::Left),
                (0.60, 0.09, Align::Left),
                (0.69, 0.13, Align::Left),
                (0.82, 0.16, Align::Right),
            ]
        };
        let heads = [
            "#".to_owned(),
            t.a("Company"),
            t.a("Scenario"),
            t.a("Medal"),
            t.a("Date"),
            t.a("Score"),
        ];
        let pick = |i: usize| -> usize { if narrow { [0, 1, 3, 5][i] } else { i } };
        let row_h = 24.0 * z;
        let y0 = 64.0 * z;
        for (ci, &(fx, fw, al)) in cols.iter().enumerate() {
            ui.add(
                layer,
                lab(&heads[pick(ci)], mfont(1, z * 0.9), GOLD, al),
                at(px + fx * pw, y0, fw * pw, row_h),
            );
        }
        let list = &self.ga.hiscores.entries;
        if list.is_empty() {
            ui.add(
                layer,
                lab(
                    &t.a("No scores yet. Win a scenario to enter the table."),
                    mfont(1, z),
                    MUTED,
                    Align::Center,
                ),
                at(px, y0 + row_h * 2.0, pw, row_h),
            );
        }
        let fit = (((ph - y0 - 8.0) / row_h) as usize).saturating_sub(1);
        for (r, e) in list.iter().take(fit).enumerate() {
            let y = y0 + row_h * (r as f32 + 1.2);
            let medal = match e.medal {
                3 => t.a("Gold"),
                2 => t.a("Silver"),
                _ => t.a("Bronze"),
            };
            let date = format!("{:02}/{}", e.month, e.year);
            let cells = [
                (r + 1).to_string(),
                e.company.clone(),
                e.scenario.clone(),
                medal,
                date,
                crate::info::money(e.score as f64),
            ];
            let color = if self.ga.hiscores.fresh == Some(r) {
                HILITE
            } else {
                CREAM
            };
            for (ci, &(fx, fw, al)) in cols.iter().enumerate() {
                ui.add(
                    layer,
                    lab(&cells[pick(ci)], mfont(1, z * 0.9), color, al),
                    at(px + fx * pw, y, fw * pw, row_h),
                );
            }
        }
        ui.add(
            layer,
            Kind::Button {
                text: t.get(crate::text::id::CANCEL, "Cancel"),
                font: mfont(1, z),
                action: super::A_BACK,
                enabled: true,
                hotkey: Some('c'),
            },
            Layout::at(Anchor::Bottom, 130.0 * z, 34.0 * z).offset(0.0, 16.0),
        );
    }
}
