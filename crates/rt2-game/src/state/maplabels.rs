//! Names written on the map: the city and region names above the towns and, zoomed in, the
//! names of the stations (the places pool, `world-growth.md` 2.1; names are the records' own
//! text, `Info::place_name`). The original's label font and colours are not decoded, so the
//! style here is the clone's: Copperplate bold with a drop shadow, sized with the zoom.
//!
//! Placement is a pure function of the candidates' screen positions (`place_labels`): labels
//! are placed by priority (bigger towns first, ties by slot, so the result never depends on
//! iteration order), a label that would overlap one already placed or leave the map window is
//! dropped, and nothing is drawn when the view is too far out to read.

use rt2_formats::map::Save;
use rt2_render::NATIVE_ZOOM;
use rt2_ui::{Align, Anchor, Color, Face, FontStyle, Kind, Layout, Ui};

use super::*;
use crate::info::Info;

/// Below this share of the original's full zoom no name is drawn (the map is a postage stamp).
pub const MIN_ZOOM_CITIES: f32 = 0.07;
/// Station names appear once the view is this close.
pub const MIN_ZOOM_STATIONS: f32 = 0.6;

/// A name that wants a place on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct Cand {
    pub text: String,
    /// Physical pixel of the point the name hangs above (the town's centre on the ground).
    pub anchor: (f32, f32),
    /// Higher first.
    pub priority: i64,
    pub station: bool,
    /// A free label of the map (kind 2: seas and the like), written in italics.
    pub label: bool,
}

/// A name with its place: the box's top-left corner and the font size in physical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub size: f32,
    pub station: bool,
    pub label: bool,
}

/// Font size in logical pixels for a city name at `zoom` (a share of the original's full zoom).
pub fn city_size(zoom: f32) -> f32 {
    (10.0 + 9.0 * zoom.sqrt()).clamp(11.0, 26.0)
}

/// Estimated advance of a text in Copperplate bold.
fn text_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.62
}

/// Places the names that fit: `zoom` is the camera's zoom over `NATIVE_ZOOM`, `k` the UI scale
/// factor (the font sizes are logical), `viewport` the physical window; `view` is the part of it
/// that shows the map; `blockers` are the rectangles (`[x, y, w, h]`, physical pixels) the HUD
/// draws over the map: a name that would touch one is dropped instead of being half hidden.
pub fn place_labels(
    mut cands: Vec<Cand>,
    zoom: f32,
    k: f32,
    view: (f32, f32),
    blockers: &[[f32; 4]],
    px_per_tile: f32,
) -> Vec<Placed> {
    if zoom < MIN_ZOOM_CITIES {
        return Vec::new();
    }
    cands.retain(|c| !c.text.is_empty() && (!c.station || zoom >= MIN_ZOOM_STATIONS));
    // Stable: equal priorities keep the caller's (pool) order.
    cands.sort_by_key(|c| std::cmp::Reverse(c.priority));
    let mut placed: Vec<Placed> = Vec::new();
    for c in cands {
        let size = if c.station {
            (city_size(zoom) * 0.72).max(10.0)
        } else {
            city_size(zoom)
        } * k;
        let (w, h) = (text_width(&c.text, size) + 6.0 * k, size * 1.3);
        // Above the town: the buildings stand about a tile and a half tall.
        let lift = if c.station {
            px_per_tile * 0.9
        } else {
            px_per_tile * 1.4
        };
        let (x, y) = (c.anchor.0 - w / 2.0, c.anchor.1 - lift - h);
        if x < 0.0 || y < 0.0 || x + w > view.0 || y + h > view.1 {
            continue;
        }
        let pad = 3.0 * k;
        if blockers
            .iter()
            .any(|b| x < b[0] + b[2] && b[0] < x + w && y < b[1] + b[3] && b[1] < y + h)
        {
            continue;
        }
        let clash = placed.iter().any(|p| {
            x < p.x + p.w + pad && p.x < x + w + pad && y < p.y + p.h + pad && p.y < y + h + pad
        });
        if !clash {
            placed.push(Placed {
                text: c.text,
                x,
                y,
                w,
                h,
                size,
                station: c.station,
                label: c.label,
            });
        }
    }
    placed
}

/// The candidates of a save: cities and regions, then the stations.
pub fn candidates(
    info: &Info,
    save: &Save,
    mut project: impl FnMut(f32, f32) -> (f32, f32),
) -> Vec<Cand> {
    let mut out = Vec::new();
    for p in save.places.ordered() {
        let d = &p.data;
        let (kind, size) = (
            d.get(0x10).copied().unwrap_or(9),
            d.get(0x2c).copied().unwrap_or(0),
        );
        // Kind 1 is a region whose name lists its industries ("Coal,sheep"): an editor note.
        if kind != 0 && kind != 2 {
            continue;
        }
        let x = f32::from(u16::from_le_bytes([d[4], d[5]]));
        let y = f32::from(u16::from_le_bytes([d[6], d[7]]));
        let houses = d.get(0x48).copied().unwrap_or(0);
        out.push(Cand {
            text: info.place_name(p.slot as u32),
            anchor: project(x + 0.5, y + 0.5),
            priority: if kind == 2 {
                0
            } else {
                1000 * i64::from(size) + i64::from(houses) + 500
            },
            station: false,
            label: kind == 2,
        });
    }
    for s in save.stations.ordered() {
        let d = &s.data;
        let x = i32::from_le_bytes([d[0xa], d[0xb], d[0xc], d[0xd]]);
        let y = i32::from_le_bytes([d[0xe], d[0xf], d[0x10], d[0x11]]);
        out.push(Cand {
            text: info.station_name(s.slot as u32),
            anchor: project(x as f32 + 0.5, y as f32 + 0.5),
            priority: -1,
            station: true,
            label: false,
        });
    }
    out.retain(|c| c.text != "-");
    out
}

impl AppState {
    /// True when the view (or the stations) changed since the names were laid out and names are
    /// shown at this zoom: the UI is rebuilt then, once per change.
    pub(super) fn map_labels_stale(&mut self) -> bool {
        let Some(g) = self.game() else {
            return false;
        };
        let c = g.camera;
        let key = (
            c.target.x,
            c.target.y,
            c.zoom,
            c.rotation,
            g.scene_fp,
            self.scale.factor().to_bits(),
        );
        if self.labels_key == Some(key) {
            return false;
        }
        let was_shown = self
            .labels_key
            .is_some_and(|k| k.2 / NATIVE_ZOOM >= MIN_ZOOM_CITIES);
        self.labels_key = Some(key);
        c.zoom / NATIVE_ZOOM >= MIN_ZOOM_CITIES || was_shown
    }

    /// Adds the layer of map names under the other layers of the in-game UI.
    pub(super) fn build_map_labels(&self, ui: &mut Ui) {
        let Some(g) = self.game() else {
            return;
        };
        let (Some(save), Some(data)) = (g.world.save_ref(), self.data.as_ref()) else {
            return;
        };
        let zoom = g.camera.zoom / NATIVE_ZOOM;
        if zoom < MIN_ZOOM_CITIES {
            return;
        }
        let d = g.world.date();
        let info = Info {
            save,
            data,
            texts: &self.texts,
            year: i32::from(d.year),
            month: i32::from(d.month),
            day: i32::from(d.day),
            minutes: g.world.minutes(),
        };
        let cands = candidates(&info, save, |x, y| {
            crate::pick::tile_to_screen(&g.camera, self.viewport, &save.map, x, y)
        });
        let k = self.scale.factor();
        // The HUD (bottom panel, action icons, rotate/zoom and speed buttons) covers parts of it.
        let view = (self.viewport.0 as f32, self.viewport.1 as f32);
        let blockers: Vec<[f32; 4]> = super::mainwin::hud_blockers(view.0 / k, view.1 / k)
            .into_iter()
            .map(|b| b.map(|v| v * k))
            .collect();
        let labels = place_labels(cands, zoom, k, view, &blockers, g.camera.zoom * 0.5);
        if labels.is_empty() {
            return;
        }
        let root = ui.add_root(Kind::Panel { fill: Fill::None }, Layout::fill());
        for l in labels {
            let face = if l.label {
                Face::Script
            } else {
                Face::Copperplate
            };
            let font = FontStyle::new(face, l.size / k)
                .with_weight(700)
                .with_shadow();
            let color = if l.label {
                Color(0xc8, 0xe4, 0xff, 0xff)
            } else if l.station {
                Color(0xf0, 0xe0, 0x9a, 0xff)
            } else {
                Color(0xff, 0xf8, 0xe8, 0xff)
            };
            ui.add(
                root,
                Kind::Label {
                    text: l.text,
                    font,
                    color,
                    align: Align::Center,
                    wrap: false,
                },
                Layout::at(Anchor::TopLeft, l.w / k, l.h / k).offset(l.x / k, l.y / k),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(text: &str, x: f32, y: f32, priority: i64) -> Cand {
        Cand {
            text: text.into(),
            anchor: (x, y),
            priority,
            station: false,
            label: false,
        }
    }

    #[test]
    fn nothing_is_drawn_when_zoomed_far_out() {
        let c = vec![cand("London", 900.0, 500.0, 5)];
        assert!(place_labels(c, 0.05, 1.0, (1920.0, 700.0), &[], 4.0).is_empty());
    }

    #[test]
    fn the_bigger_town_wins_an_overlap_and_the_order_is_stable() {
        let a = cand("Slough", 900.0, 500.0, 1);
        let b = cand("London", 910.0, 505.0, 9);
        let far = cand("Oxford", 300.0, 400.0, 2);
        let r1 = place_labels(
            vec![a.clone(), b.clone(), far.clone()],
            0.5,
            1.0,
            (1920.0, 700.0),
            &[],
            20.0,
        );
        let r2 = place_labels(vec![far, b, a], 0.5, 1.0, (1920.0, 700.0), &[], 20.0);
        let names = |v: &[Placed]| v.iter().map(|p| p.text.clone()).collect::<Vec<_>>();
        assert_eq!(names(&r1), ["London", "Oxford"]);
        assert_eq!(names(&r2), names(&r1));
    }

    #[test]
    fn a_name_sits_above_its_town_and_off_the_panel_is_dropped() {
        let r = place_labels(
            vec![cand("York", 500.0, 400.0, 1), cand("Hull", 500.0, 760.0, 1)],
            1.0,
            1.0,
            (1920.0, 700.0),
            &[],
            32.0,
        );
        assert_eq!(r.len(), 1);
        let p = &r[0];
        assert!(p.y + p.h < 400.0, "above the town");
        assert!((p.x + p.w / 2.0 - 500.0).abs() < 0.01, "centred on it");
    }

    #[test]
    fn a_name_under_a_hud_button_is_dropped_and_one_beside_it_kept() {
        // A rotate/zoom strip at the bottom right (physical pixels).
        let strip = [[1500.0, 600.0, 400.0, 60.0]];
        let r = place_labels(
            vec![
                cand("Cambridge", 1700.0, 700.0, 5),
                cand("Norwich", 1000.0, 700.0, 4),
            ],
            1.0,
            1.0,
            (1920.0, 1080.0),
            &strip,
            20.0,
        );
        let names: Vec<_> = r.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(names, ["Norwich"]);
    }

    #[test]
    fn station_names_need_a_close_view_and_the_font_grows_with_the_zoom() {
        let mut s = cand("Slough Station", 800.0, 500.0, -1);
        s.station = true;
        assert!(place_labels(vec![s.clone()], 0.3, 1.0, (1920.0, 700.0), &[], 10.0).is_empty());
        assert_eq!(
            place_labels(vec![s], 1.0, 1.0, (1920.0, 700.0), &[], 30.0).len(),
            1
        );
        assert!(city_size(1.0) > city_size(0.1));
        assert!(city_size(100.0) <= 26.0);
    }
}
