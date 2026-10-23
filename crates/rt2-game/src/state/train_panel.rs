//! The train detail's instrument panel and stop controls (docs/reference/screens.md E2) and the
//! car list of a stop (E4), on top of the editable train detail of `build_dialogs.rs`.
//!
//! Panel (manual chapter 6, "Instrument Panel"): oil, sand and (steam only) water gauges and the
//! throttle, read from the train record (trains.md 1.1: `+0xd3`, `+0xcf`, `+0xd7`, `+0xdb`). Stop
//! controls and the commands they send (`rt2_sim::build::cmd::BuildCmd`, commands.md 3.1):
//! next station `0x1a` (`TrainGotoStop`), wait stoplight `0x1b` (`StopWait`), sell mode `0x2f`
//! (`StopSell`), train class `0x21` (`TrainClass`), cars of the stop `0x1c` (`StopCargo`).
//! **Gap**: the throttle lever sends `0x29` (`cmd_train_throttle`), which `rt2-sim` does not have
//! yet, so the lever is a read-only gauge.

use rt2_sim::build::cmd::BuildCmd;
use rt2_ui::{Align, Anchor, Color, Fill, FontStyle, Kind, Layout, NodeId, Ui, game_font};

use super::dialogs::Dialog;
use super::{AppState, Screen};
use crate::buildtools as bt;

pub(super) const A_TP_GOTO: u32 = 11200;
pub(super) const A_TP_WAIT: u32 = 11201;
pub(super) const A_TP_SELL: u32 = 11202;
pub(super) const A_TP_CLASS: u32 = 11203;
pub(super) const A_TP_CARS: u32 = 11204;
/// Car list: remove the car in slot `a - A_CL_REMOVE` (6 slots).
pub(super) const A_CL_REMOVE: u32 = 11220;
/// Car list: add the cargo `a - A_CL_ADD` (cargo ids below 0x2c).
pub(super) const A_CL_ADD: u32 = 11230;
pub(super) const A_CL_CLEAR: u32 = 11285;
pub(super) const A_CL_OK: u32 = 11286;
pub(super) const A_CL_CANCEL: u32 = 11287;

const TEXT: Color = Color::rgb(0xee, 0xe4, 0xc8);
const MUTED: Color = Color::rgb(0xb4, 0xaa, 0x8e);
const HEAD: Color = Color::rgb(0xe6, 0xc2, 0x6a);
const INSET: Color = Color(0x0c, 0x0f, 0x0c, 0xd8);

pub(super) fn owns(a: u32) -> bool {
    (A_TP_GOTO..=A_CL_CANCEL).contains(&a)
}

fn fnt(n: u8, dz: f32) -> FontStyle {
    let f = game_font(n);
    f.with_size((f.size * dz).round())
}

fn at(x: f32, y: f32, w: f32, h: f32) -> Layout {
    Layout::at(Anchor::TopLeft, w, h).offset(x, y)
}

fn lbl(text: &str, f: FontStyle, color: Color, align: Align) -> Kind {
    Kind::Label {
        text: text.to_owned(),
        font: f,
        color,
        align,
        wrap: false,
    }
}

fn button(text: String, f: FontStyle, action: u32, enabled: bool) -> Kind {
    Kind::Button {
        text,
        font: f,
        action,
        enabled,
        hotkey: None,
    }
}

/// One gauge: caption, a bar filled to `value` percent (with an optional red zone from
/// `red_from`) and the number.
#[allow(clippy::too_many_arguments)]
fn gauge(
    ui: &mut Ui,
    root: NodeId,
    caption: &str,
    value: f32,
    color: Color,
    red_from: Option<f32>,
    x: f32,
    y: f32,
    w: f32,
    dz: f32,
) {
    let f = fnt(1, dz * 0.8);
    ui.add(
        root,
        lbl(caption, f, HEAD, Align::Left),
        at(x, y, w * 0.6, 16.0 * dz),
    );
    ui.add(
        root,
        lbl(&format!("{:.0}%", value.max(0.0)), f, TEXT, Align::Right),
        at(x + w * 0.5, y, w * 0.5, 16.0 * dz),
    );
    let by = y + 18.0 * dz;
    let bh = 12.0 * dz;
    ui.add(
        root,
        Kind::Panel {
            fill: Fill::Solid(INSET),
        },
        at(x, by, w, bh),
    );
    let frac = (value / 100.0).clamp(0.0, 1.0);
    let split = red_from.map_or(1.0, |r| r / 100.0).min(frac);
    if split > 0.0 {
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(color),
            },
            at(x + 1.0, by + 1.0, (w - 2.0) * split, bh - 2.0),
        );
    }
    if frac > split {
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(Color::rgb(0xd0, 0x40, 0x30)),
            },
            at(
                x + 1.0 + (w - 2.0) * split,
                by + 1.0,
                (w - 2.0) * (frac - split),
                bh - 2.0,
            ),
        );
    }
    if let Some(r) = red_from {
        // The 85 percent mark: the throttle's normal setting.
        ui.add(
            root,
            Kind::Panel {
                fill: Fill::Solid(TEXT),
            },
            at(x + (w - 2.0) * r / 100.0, by, 2.0, bh),
        );
    }
}

impl AppState {
    fn train_class_name(&self, class: u8) -> String {
        let t = &self.texts;
        match class {
            0 => t.a("Halted"),
            1 => t.a("Slow"),
            3 => t.a("Express"),
            _ => t.a("Normal"),
        }
    }

    /// The panel inside `(x, y, w, h)` of the train detail for the selected stop `sel`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_train_panel(
        &self,
        ui: &mut Ui,
        root: NodeId,
        x: f32,
        y: f32,
        w: f32,
        dz: f32,
        train: i32,
        sel: usize,
    ) {
        let t = &self.texts;
        let panel = self.with_info(|i| i.train_panel(train as u32)).flatten();
        let stops = self
            .game()
            .and_then(|g| g.world.save_ref())
            .map(|s| bt::train_stops(s, train))
            .unwrap_or_default();
        let sell = self
            .with_info(|i| i.stop_sell_mode(train as u32, sel))
            .unwrap_or(0);
        let stop = stops.get(sel);
        let bh = 24.0 * dz;
        let f = fnt(1, dz * 0.72);
        let bw = (w - 2.0 * 4.0 * dz) / 3.0;
        let wait_names = [t.a("no wait"), t.a("half full"), t.a("full")];
        let sell_names = [
            t.a("Sell demanded"),
            t.a("Store the rest"),
            t.a("Sell all"),
            t.a("Keep the rest"),
        ];
        let has = stop.is_some();
        let cells = [
            (t.a("Go to this stop"), A_TP_GOTO),
            (
                format!(
                    "{}: {}",
                    t.a("Wait"),
                    wait_names[usize::from(stop.map_or(0, |s| s.wait.min(2)))]
                ),
                A_TP_WAIT,
            ),
            (
                format!("{}: {}", t.a("Sell"), sell_names[usize::from(sell & 3)]),
                A_TP_SELL,
            ),
        ];
        for (i, (text, action)) in cells.into_iter().enumerate() {
            ui.add(
                root,
                button(text, f, action, has),
                at(x + i as f32 * (bw + 4.0 * dz), y, bw, bh),
            );
        }
        let by2 = y + bh + 4.0 * dz;
        let class = panel.map_or(2, |p| p.class.min(3));
        ui.add(
            root,
            button(
                format!("{}: {}", t.a("Class"), self.train_class_name(class)),
                f,
                A_TP_CLASS,
                panel.is_some(),
            ),
            at(x, by2, bw, bh),
        );
        ui.add(
            root,
            button(t.a("Car list"), f, A_TP_CARS, has),
            at(x + bw + 4.0 * dz, by2, bw, bh),
        );
        // The wait / sell explanation of the selected stop, in the original's words.
        if let Some(s) = stop {
            ui.add(
                root,
                Kind::Label {
                    text: t.get(1114 + u32::from(s.wait.min(2)), ""),
                    font: fnt(8, dz * 0.5),
                    color: MUTED,
                    align: Align::Left,
                    wrap: true,
                },
                at(x, by2 + bh + 4.0 * dz, w, 34.0 * dz),
            );
        }
        // Gauges.
        let gy = by2 + bh + 44.0 * dz;
        let Some(p) = panel else { return };
        let steam = p.kind.is_none_or(|k| k == 0);
        let n = if steam { 4.0 } else { 3.0 };
        let gw = (w - (n - 1.0) * 8.0 * dz) / n;
        let mut gx = x;
        let mut put = |ui: &mut Ui, cap: String, v: f32, c: Color, red: Option<f32>| {
            gauge(ui, root, &cap, v, c, red, gx, gy, gw, dz);
            gx += gw + 8.0 * dz;
        };
        put(ui, t.a("Oil"), p.oil, Color::rgb(0xd8, 0xa0, 0x30), None);
        put(ui, t.a("Sand"), p.sand, Color::rgb(0xc8, 0xb0, 0x78), None);
        if steam {
            put(
                ui,
                t.a("Water"),
                p.water,
                Color::rgb(0x50, 0x98, 0xd8),
                None,
            );
        }
        put(
            ui,
            t.a("Throttle"),
            p.throttle,
            Color::rgb(0x60, 0xc0, 0x60),
            Some(85.0),
        );
        let info = format!(
            "{}   {}   {}",
            t.af("%1 years", &[p.age.to_string()]),
            t.af("%1 t", &[p.tons.to_string()]),
            if p.broken {
                t.a("broken down")
            } else {
                String::new()
            }
        );
        ui.add(
            root,
            lbl(&info, fnt(1, dz * 0.75), MUTED, Align::Left),
            at(x, gy + 36.0 * dz, w, 18.0 * dz),
        );
    }

    fn tp_top(&self) -> Option<Dialog> {
        match &self.screen {
            Screen::Game(g) => g.dialogs.last().cloned(),
            Screen::Menu(_) => None,
        }
    }

    fn tp_error(&mut self, e: rt2_sim::build::cmd::CmdError) {
        let m = self.cmd_error_text(e, 394);
        self.toast(m);
    }

    /// Buttons of the instrument panel and the car list; true when `action` was one of them.
    pub(super) fn train_panel_activated(&mut self, action: u32) -> bool {
        match self.tp_top() {
            Some(Dialog::TrainEdit { train, sel, .. }) => self.tp_edit(action, train, sel),
            Some(Dialog::CarList { train, stop, cargo }) => {
                self.tp_cars(action, train, stop, cargo)
            }
            _ => return false,
        }
        true
    }

    fn tp_edit(&mut self, action: u32, train: i32, sel: usize) {
        let stop = sel as i32;
        let stops = self
            .game()
            .and_then(|g| g.world.save_ref())
            .map(|s| bt::train_stops(s, train))
            .unwrap_or_default();
        let cmd = match action {
            A_TP_GOTO => BuildCmd::TrainGotoStop { train, stop },
            A_TP_WAIT => BuildCmd::StopWait {
                train,
                stop,
                mode: ((stops.get(sel).map_or(0, |s| s.wait) + 1) % 3) as i8,
            },
            A_TP_SELL => {
                let m = self
                    .with_info(|i| i.stop_sell_mode(train as u32, sel))
                    .unwrap_or(0);
                BuildCmd::StopSell {
                    train,
                    stop,
                    mode: (m + 1) & 3,
                }
            }
            A_TP_CLASS => {
                let c = self
                    .with_info(|i| i.train_panel(train as u32))
                    .flatten()
                    .map_or(2, |p| p.class);
                BuildCmd::TrainClass {
                    train,
                    class: ((c + 1) % 4) as i8,
                }
            }
            A_TP_CARS => {
                let cargo = stops.get(sel).map(|s| s.cargo.clone()).unwrap_or_default();
                return self.push_dialog(Dialog::CarList {
                    train,
                    stop: sel,
                    cargo,
                });
            }
            _ => return,
        };
        match self.issue(cmd) {
            Ok(_) => self.rebuild_ui(),
            Err(e) => self.tp_error(e),
        }
    }

    fn tp_cars(&mut self, action: u32, train: i32, stop: usize, mut cargo: Vec<u8>) {
        match action {
            a if (A_CL_REMOVE..A_CL_REMOVE + 6).contains(&a) => {
                let i = (a - A_CL_REMOVE) as usize;
                if i < cargo.len() {
                    cargo.remove(i);
                }
            }
            a if (A_CL_ADD..A_CL_ADD + 0x2c).contains(&a) => {
                if cargo.len() < 6 {
                    cargo.push((a - A_CL_ADD) as u8);
                }
            }
            A_CL_CLEAR => cargo.clear(),
            A_CL_CANCEL => return self.close_dialog(),
            A_CL_OK => {
                let mut c = [0xffu8; 6];
                for (slot, v) in c.iter_mut().zip(&cargo) {
                    *slot = *v;
                }
                let r = self.issue(BuildCmd::StopCargo {
                    train,
                    stop: stop as i32,
                    cargo: c,
                });
                match r {
                    Ok(_) => self.close_dialog(),
                    Err(e) => self.tp_error(e),
                }
                return;
            }
            _ => return,
        }
        if let Screen::Game(g) = &mut self.screen
            && let Some(top) = g.dialogs.last_mut()
        {
            *top = Dialog::CarList { train, stop, cargo };
        }
        self.rebuild_ui();
    }

    /// The car list of a stop (`traindtl.win` car list mode, manual "Map Router/Car List"):
    /// the cars the train takes from this stop on (click one to remove it, Clear empties the
    /// list) and the cars that can be added.
    pub(super) fn build_car_list(&self, ui: &mut Ui, train: i32, stop: usize, cargo: &[u8]) {
        let t = &self.texts;
        let (cw, ch) = ui.logical_size();
        let dz = (ch / 620.0).clamp(1.0, 2.0);
        let w = (700.0 * dz).min(cw - 16.0);
        let h = (460.0 * dz).min(ch - 16.0);
        let root = ui.add_root(
            Kind::Panel { fill: Fill::Panel },
            Layout::at(Anchor::Center, w, h),
        );
        let pad = 14.0 * dz;
        let name = self
            .with_info(|i| {
                let k = bt::train_stops(i.save, train);
                k.get(stop).map_or(String::new(), |s| {
                    if s.waypoint {
                        self.texts.a("waypoint")
                    } else {
                        i.station_name(s.station as u32)
                    }
                })
            })
            .unwrap_or_default();
        ui.add(
            root,
            lbl(
                &format!("{} {}: {}", t.a("Car list, stop"), stop + 1, name).to_uppercase(),
                fnt(3, dz * 0.7),
                HEAD,
                Align::Left,
            ),
            at(pad, pad * 0.6, w - 2.0 * pad, 28.0 * dz),
        );
        let f = fnt(1, dz * 0.8);
        let lw = (w - 3.0 * pad) * 0.38;
        ui.add(
            root,
            lbl(&t.a("Cars on the train"), f, HEAD, Align::Left),
            at(pad, 40.0 * dz, lw, 18.0 * dz),
        );
        let rh = 30.0 * dz;
        for i in 0..6 {
            let text = match cargo.get(i) {
                Some(&c) => format!(
                    "{}. {}",
                    i + 1,
                    self.with_info(|inf| inf.cargo_label(c)).unwrap_or_default()
                ),
                None => format!("{}. -", i + 1),
            };
            ui.add(
                root,
                button(text, f, A_CL_REMOVE + i as u32, i < cargo.len()),
                at(pad, 62.0 * dz + i as f32 * (rh + 4.0 * dz), lw, rh),
            );
        }
        ui.add(
            root,
            lbl(
                &t.a("Click a car to remove it."),
                fnt(8, dz * 0.5),
                MUTED,
                Align::Left,
            ),
            at(pad, 62.0 * dz + 6.0 * (rh + 4.0 * dz), lw, 16.0 * dz),
        );
        let ax = pad * 2.0 + lw;
        let aw = w - ax - pad;
        ui.add(
            root,
            lbl(&t.a("Cars that can be added"), f, HEAD, Align::Left),
            at(ax, 40.0 * dz, aw, 18.0 * dz),
        );
        let choices = self
            .with_info(|i| i.car_cargo_choices())
            .unwrap_or_default();
        let cols = 4usize;
        let bw = (aw - (cols as f32 - 1.0) * 4.0 * dz) / cols as f32;
        let bh = 26.0 * dz;
        for (k, (c, n)) in choices.iter().enumerate() {
            let (cx, cy) = (k % cols, k / cols);
            let fit = (bw * 0.92 / (0.66 * f.size * n.chars().count().max(1) as f32)).min(1.0);
            ui.add(
                root,
                button(
                    n.clone(),
                    f.with_size((f.size * fit).round().max(9.0)),
                    A_CL_ADD + u32::from(*c),
                    cargo.len() < 6,
                ),
                at(
                    ax + cx as f32 * (bw + 4.0 * dz),
                    62.0 * dz + cy as f32 * (bh + 4.0 * dz),
                    bw,
                    bh,
                ),
            );
        }
        let by = h - pad - 30.0 * dz;
        for (i, (text, action)) in [
            (t.a("Clear"), A_CL_CLEAR),
            (t.get(crate::text::id::OK, "OK"), A_CL_OK),
            (t.get(crate::text::id::CANCEL, "Cancel"), A_CL_CANCEL),
        ]
        .into_iter()
        .enumerate()
        {
            ui.add(
                root,
                button(text, f, action, true),
                at(
                    w - pad - (3 - i) as f32 * (124.0 * dz),
                    by,
                    120.0 * dz,
                    30.0 * dz,
                ),
            );
        }
        let _ = TEXT;
    }
}
