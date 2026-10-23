//! C3 and C4: the information box of a building (left click: the Buy button in the advanced
//! industrial model; right click: the same without it) and of unimproved terrain (right click:
//! type of terrain, territory, your access rights, X,Y). Manual chapter 4, "Viewing Buildings and
//! Terrain"; the box is the original's generic eagle dialog (`eagled.win`, LNG 2330..2338).
//!
//! Gaps: the mapping from the profitability rating (`+0x4b`, 100 = neutral) to the six labels of
//! LNG 2466..2471 was not located in the executable (`world-growth.md`, gaps), so the bands below
//! are the clone's, with "Gushing Cash" at the documented `rating >= 180`.

use rt2_formats::map::Save;

use super::dialogs::{Act, Dialog, Message};
use super::{AppState, Screen};
use crate::buildtools::{self as bt, Rec, Trade};
use crate::overview::building_cargos;

/// The profitability label of LNG 2466..2471 for a rating (`+0x4b`, world-growth.md 2.x).
pub(super) fn profit_label_index(rating: u16) -> u32 {
    match rating {
        ..75 => 0,
        75..100 => 1,
        100..125 => 2,
        125..150 => 3,
        150..180 => 4,
        _ => 5,
    }
}

/// True when a building (any object box) stands on tile `(x, y)`.
pub(super) fn object_at(save: &Save, x: i32, y: i32) -> bool {
    save.objects.ordered().any(|o| {
        let b = Rec(&o.data);
        x >= i32::from(b.i16(0x26))
            && y >= i32::from(b.i16(0x28))
            && x <= i32::from(b.i16(0x2a))
            && y <= i32::from(b.i16(0x2c))
    })
}

fn text_of(r: &Rec, from: usize, to: usize) -> String {
    let bytes: Vec<u8> = (from..to)
        .map(|o| r.u8(o))
        .take_while(|&b| b != 0)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The territory record of a map region.
fn territory_of(save: &Save, region: u8) -> Option<(i32, String)> {
    save.territories.ordered().find_map(|t| {
        let r = Rec(&t.data);
        (r.u8(0x2a) == region).then(|| (t.slot as i32, text_of(&r, 4, 0x24)))
    })
}

impl AppState {
    fn cargo_list(&self, ids: &[u8]) -> String {
        let names: Vec<String> = ids
            .iter()
            .map(|&c| {
                self.texts.table(
                    "CARGO_TYPES",
                    u32::from(c),
                    &self.texts.af("cargo %1", &[c.to_string()]),
                )
            })
            .collect();
        let mut names = names.join(", ");
        // The tables are lower case ("grain"): the box starts each list with a capital.
        if let Some(c) = names.get(..1) {
            names = c.to_uppercase() + &names[1..];
        }
        names
    }

    /// The box of an industry (or house) building; `allow_buy` is false for a right click.
    pub(super) fn bldinfo_industry(&mut self, building: i32, allow_buy: bool) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let (Some(save), Some(data)) = (g.world.save_ref(), self.data.as_ref()) else {
            return;
        };
        let Some(b) = bt::rec(&save.objects, building) else {
            return;
        };
        let t = &self.texts;
        let ty = b.u8(7);
        let co = self.player();
        let year = i32::from(g.world.date().year);
        let owner = b.u16(0x97);
        let owner_text = if owner == 0 {
            t.get(2332, "Owned by no one")
        } else {
            self.with_info(|i| i.company_name(u32::from(owner)))
                .unwrap_or_default()
        };
        let profit = t.get(
            2466 + profit_label_index(b.u16(0x4b)),
            [
                "Poor",
                "Mediocre",
                "Good",
                "Lucrative",
                "Very Lucrative",
                "Gushing Cash",
            ][profit_label_index(b.u16(0x4b)) as usize],
        );
        let price = bt::building_price(data, save, co.unwrap_or(0), building);
        let (supply, demand) = building_cargos(data, ty, year);
        let mut lines = vec![
            format!("{}: {}", t.a("Ownership"), owner_text),
            format!("{}: {}", t.get(2333, "Profitability"), profit),
            format!(
                "{}: {}",
                t.get(2334, "Purchase Price"),
                crate::info::money(f64::from(price))
            ),
        ];
        if !supply.is_empty() {
            lines.push(format!(
                "{}: {}",
                t.get(151, "Supplies"),
                self.cargo_list(&supply)
            ));
        }
        if !demand.is_empty() {
            lines.push(format!(
                "{}: {}",
                t.get(152, "Demands"),
                self.cargo_list(&demand)
            ));
        }
        if supply.is_empty() && demand.is_empty() {
            lines.push(t.a("Nothing is supplied or demanded here."));
        }
        let trade = match (allow_buy, co) {
            (true, Some(co)) => bt::industry_trade(data, save, co, building),
            _ => Trade::None,
        };
        let mut buttons = Vec::new();
        match trade {
            Trade::Buy => buttons.push((
                t.get(2335, "Buy"),
                Act::Trade {
                    building,
                    buy: true,
                },
            )),
            Trade::Sell => buttons.push((
                t.get(2337, "Sell"),
                Act::Trade {
                    building,
                    buy: false,
                },
            )),
            Trade::None => {}
        }
        buttons.push((t.get(crate::text::id::OK, "OK"), Act::Close));
        let title = t.get(920 + u32::from(ty), &t.af("Building %1", &[ty.to_string()]));
        self.push_dialog(Dialog::Message(Message {
            title,
            body: lines.join("\n"),
            buttons,
        }));
    }

    /// Right click: what is under the map tile `(x, y)`.
    pub(super) fn bldinfo_right_click(&mut self, x: i32, y: i32) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let Some(save) = g.world.save_ref() else {
            return;
        };
        let d = g.world.date();
        let pick = self.with_info(|i| i.pick(x, y)).flatten();
        match pick {
            Some(crate::info::Pick::Industry(s)) => return self.bldinfo_industry(s as i32, false),
            Some(crate::info::Pick::Station(s)) => {
                let name = self
                    .with_info(|i| i.station_page(s).title)
                    .unwrap_or_default();
                let owner = bt::rec(&save.stations, s as i32).map_or(0, |r| r.i32(0x39));
                let owner = if owner > 0 {
                    self.with_info(|i| i.company_name(owner as u32))
                        .unwrap_or_default()
                } else {
                    self.texts.get(2332, "Owned by no one")
                };
                let body = format!(
                    "{}: {}\n{}: {x}, {y}",
                    self.texts.a("Ownership"),
                    owner,
                    "X,Y"
                );
                return self.push_dialog(Dialog::Message(Message {
                    title: name,
                    body,
                    buttons: vec![(self.texts.get(crate::text::id::OK, "OK"), Act::Close)],
                }));
            }
            // The box of a named city or region is not a building: its open ground is terrain.
            Some(crate::info::Pick::Place(s)) if object_at(save, x, y) => {
                let name = self
                    .with_info(|i| i.place_page(s).title)
                    .unwrap_or_default();
                let body = format!("{}: {x}, {y}", "X,Y");
                return self.push_dialog(Dialog::Message(Message {
                    title: name,
                    body,
                    buttons: vec![(self.texts.get(crate::text::id::OK, "OK"), Act::Close)],
                }));
            }
            _ => {}
        }
        let _ = d;
        self.bldinfo_terrain(x, y);
    }

    /// Unimproved terrain: type, territory, your access rights and the coordinates.
    pub(super) fn bldinfo_terrain(&mut self, x: i32, y: i32) {
        let Screen::Game(g) = &self.screen else {
            return;
        };
        let Some(save) = g.world.save_ref() else {
            return;
        };
        if x < 0 || y < 0 || x as usize >= save.map.width || y as usize >= save.map.height {
            return;
        }
        let t = &self.texts;
        let class = rt2_sim::editor::class_at(save, x, y);
        // TERRAIN_TYPES holds the 14 named classes (beach .. ocean); water is class 18.
        let named = if class == 18 { 13 } else { u32::from(class) };
        let terrain = if named <= 13 {
            t.table(
                "TERRAIN_TYPES",
                named,
                &t.af("Terrain %1", &[class.to_string()]),
            )
        } else {
            t.af("Terrain %1", &[class.to_string()])
        };
        let region = save.map.tile(x as usize, y as usize).region;
        let territory = territory_of(save, region);
        let mut lines = vec![format!("{}: {}", t.a("Type of Terrain"), terrain)];
        let tname = territory
            .as_ref()
            .map_or_else(|| t.get(2326, "None"), |(_, n)| n.clone());
        lines.push(format!("{}: {}", t.get(2325, "Territory"), tname));
        if let (Some(co), Some((slot, _))) = (self.player(), territory.as_ref()) {
            let mine = bt::rights(save, co, *slot) & 3;
            let text = t.get(
                2326 + u32::from(mine),
                ["None", "Train running only", "Track laying only", "Full"][usize::from(mine)],
            );
            lines.push(format!("{}: {}", t.a("Your Access Rights"), text));
        }
        lines.push(format!("X,Y: {x}, {y}"));
        let title = t.a("Terrain");
        self.push_dialog(Dialog::Message(Message {
            title,
            body: lines.join("\n"),
            buttons: vec![(t.get(crate::text::id::OK, "OK"), Act::Close)],
        }));
    }

    /// A right click on the map that did not move: the box of what is under it.
    pub(super) fn map_right_click(&mut self, pos: (f32, f32)) {
        if self.info_screen().is_some() || self.has_dialog() || self.overlay.is_some() {
            return;
        }
        let Screen::Game(g) = &self.screen else {
            return;
        };
        if g.build.tool.is_some() || g.editor.is_some() {
            return;
        }
        let Some(save) = g.world.save_ref() else {
            return;
        };
        if let Some((cx, cy)) = self.cursor
            && ((cx - pos.0).powi(2) + (cy - pos.1).powi(2)).sqrt() > 5.0
        {
            return;
        }
        let Some((tx, ty)) = crate::pick::screen_to_tile(&g.camera, self.viewport, pos, &save.map)
        else {
            return;
        };
        self.bldinfo_right_click(tx, ty);
    }
}
