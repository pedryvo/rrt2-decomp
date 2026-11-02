//! Change Company Name/Logo (docs/reference/screens.md G12; manual chapter 7, "Change Company
//! Name/Logo" on the Overview tab; command `0x27` `cmd_set_company_info`, commands.md 2.1:
//! `+0xc` company, `+0xd` name char[31], `+0x2c` logo/colour byte).
//!
//! The dialog edits the name by typing (30 bytes, like the founding dialog) and the logo with
//! the arrows (`brlogo00..40`). **Gap**: `rt2-sim` has no `0x27` command yet, so OK cannot change
//! the company; it reports that instead of faking the change.

use rt2_ui::{Color, Fill, Fit, Kind, Ui, game_font};
use winit::keyboard::KeyCode;

use super::super::{AppState, Screen};
use super::{CREAM, Dialog, E_TEXT, GOLD, INK, at, label};
use crate::artwork::brand::BRAND_COUNT;

pub(in crate::state) const A_BRAND_PREV: u32 = 11100;
pub(in crate::state) const A_BRAND_NEXT: u32 = 11101;
pub(in crate::state) const A_BRAND_OK: u32 = 11102;

pub(in crate::state) fn owns(a: u32) -> bool {
    (A_BRAND_PREV..=A_BRAND_OK).contains(&a)
}

impl AppState {
    /// Opens the dialog for `company` with its current name and logo.
    pub(in crate::state) fn open_brand(&mut self, company: i32) {
        let Some((name, logo)) = self.with_info(|i| {
            (
                i.company_name(company as u32),
                i.company_logo(company as u32) as u8,
            )
        }) else {
            return;
        };
        self.push_dialog(Dialog::Brand {
            company,
            name,
            logo,
            error: None,
        });
    }

    fn brand_state(&mut self) -> Option<(&mut String, &mut u8, &mut Option<String>)> {
        let Screen::Game(g) = &mut self.screen else {
            return None;
        };
        match g.dialogs.last_mut() {
            Some(Dialog::Brand {
                name, logo, error, ..
            }) => Some((name, logo, error)),
            _ => None,
        }
    }

    pub(in crate::state) fn brand_activated(&mut self, action: u32) {
        match action {
            A_BRAND_PREV | A_BRAND_NEXT => {
                if let Some((_, logo, error)) = self.brand_state() {
                    let n = BRAND_COUNT as i32;
                    let d = if action == A_BRAND_NEXT { 1 } else { -1 };
                    *logo = (i32::from(*logo) + d).rem_euclid(n) as u8;
                    *error = None;
                }
                self.rebuild_ui();
            }
            A_BRAND_OK => {
                let Some((company, name, logo)) = (match self.screen {
                    Screen::Game(ref g) => match g.dialogs.last() {
                        Some(Dialog::Brand {
                            company,
                            name,
                            logo,
                            ..
                        }) => Some((*company, name.clone(), *logo)),
                        _ => None,
                    },
                    _ => None,
                }) else {
                    return;
                };
                if let Screen::Game(g) = &mut self.screen {
                    let mut name_buf = [0u8; 31];
                    let bytes = name.as_bytes();
                    let len = bytes.len().min(30);
                    name_buf[..len].copy_from_slice(&bytes[..len]);
                    match g.world.set_company_info(company as u8, name_buf, logo) {
                        Ok(_) => {
                            g.dialogs.pop();
                        }
                        Err(e) => {
                            let msg = self.cmd_error_text(e, 396);
                            if let Some((_, _, error)) = self.brand_state() {
                                *error = Some(msg);
                            }
                        }
                    }
                }
                self.rebuild_ui();
            }
            _ => {}
        }
    }

    /// Typing into the open dialog; true when the key was consumed.
    pub(in crate::state) fn brand_key(&mut self, code: KeyCode, text: Option<&str>) -> bool {
        if self.brand_state().is_none() {
            return false;
        }
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => self.brand_activated(A_BRAND_OK),
            KeyCode::Escape => self.close_dialog(),
            KeyCode::ArrowUp | KeyCode::ArrowRight => self.brand_activated(A_BRAND_NEXT),
            KeyCode::ArrowDown | KeyCode::ArrowLeft => self.brand_activated(A_BRAND_PREV),
            KeyCode::Backspace => {
                if let Some((name, _, error)) = self.brand_state() {
                    name.pop();
                    *error = None;
                }
                self.rebuild_ui();
            }
            _ => {
                let Some(c) = text
                    .and_then(|t| t.chars().next())
                    .filter(|c| c.is_ascii() && !c.is_ascii_control())
                else {
                    return false;
                };
                if let Some((name, _, error)) = self.brand_state()
                    && name.len() < 30
                {
                    name.push(c);
                    *error = None;
                }
                self.rebuild_ui();
            }
        }
        true
    }

    pub(in crate::state) fn build_brand(
        &self,
        ui: &mut Ui,
        name: &str,
        logo: u8,
        error: Option<&str>,
    ) {
        let t = &self.texts;
        let fr = self.eagle(ui, 6);
        let sx = fr.sx;
        let (x, y, w, _) = fr.text;
        let size = (15.0 * sx).round();
        let f = game_font(8).with_size(size.max(10.0));
        let lh = (size * 1.3).round();
        ui.add(
            fr.root,
            label(
                &t.a("Change Company Name/Logo").to_uppercase(),
                f,
                GOLD,
                rt2_ui::Align::Left,
            ),
            at(x, y, w, lh),
        );
        let mut yy = y + lh * 1.4;
        ui.add(
            fr.root,
            label(&t.get(543, "Company Name"), f, CREAM, rt2_ui::Align::Left),
            at(x, yy, w * 0.5, lh),
        );
        yy += lh * 1.1;
        let box_h = (lh * 1.4).round();
        let name_box = ui.add(
            fr.root,
            Kind::Panel {
                fill: Fill::Solid(INK),
            },
            at(x, yy, w, box_h),
        );
        ui.add(
            name_box,
            Kind::Label {
                text: format!("{name}_"),
                font: f,
                color: GOLD,
                align: rt2_ui::Align::Left,
                wrap: false,
            },
            at(8.0 * sx, (box_h - lh) * 0.5, w - 16.0 * sx, lh),
        );
        yy += box_h + lh * 0.4;
        ui.add(
            fr.root,
            label(&t.get(544, "Company Logo:"), f, CREAM, rt2_ui::Align::Left),
            at(x, yy, w * 0.5, lh),
        );
        yy += lh * 1.1;
        let bh = (30.0 * sx).round();
        let ph = (lh * 4.2).min(fr.text.3 + 120.0 * sx);
        let pw = ph * 1.1;
        let cx = x + w / 2.0;
        let arrow = game_font(3).with_size((game_font(3).size * sx * 0.7).round().max(10.0));

        // Navigation buttons
        ui.add(
            fr.root,
            Kind::Button {
                text: "<".into(),
                font: arrow,
                action: A_BRAND_PREV,
                enabled: true,
                hotkey: None,
            },
            at(
                cx - pw / 2.0 - bh * 1.5,
                yy + ph / 2.0 - bh / 2.0,
                bh * 1.2,
                bh,
            ),
        );
        ui.add(
            fr.root,
            Kind::Button {
                text: ">".into(),
                font: arrow,
                action: A_BRAND_NEXT,
                enabled: true,
                hotkey: None,
            },
            at(
                cx + pw / 2.0 + bh * 0.3,
                yy + ph / 2.0 - bh / 2.0,
                bh * 1.2,
                bh,
            ),
        );

        // Previous and Next logo previews (side thumbnails)
        let side_w = pw * 0.55;
        let side_h = ph * 0.55;
        let prev_logo = (i32::from(logo) - 1).rem_euclid(BRAND_COUNT as i32) as usize;
        let next_logo = (i32::from(logo) + 1).rem_euclid(BRAND_COUNT as i32) as usize;
        if let Some(Some(sprite)) = self.icons.brand.logos.get(prev_logo) {
            let side_box = ui.add(
                fr.root,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x18, 0x12, 0x0a, 0x90)),
                },
                at(
                    cx - pw / 2.0 - bh * 1.5 - side_w - 6.0 * sx,
                    yy + (ph - side_h) * 0.5,
                    side_w,
                    side_h,
                ),
            );
            ui.add(
                side_box,
                Kind::Image {
                    sprite: *sprite,
                    fit: Fit::Contain,
                },
                rt2_ui::Layout::fill(),
            );
        }
        if let Some(Some(sprite)) = self.icons.brand.logos.get(next_logo) {
            let side_box = ui.add(
                fr.root,
                Kind::Panel {
                    fill: Fill::Solid(Color(0x18, 0x12, 0x0a, 0x90)),
                },
                at(
                    cx + pw / 2.0 + bh * 1.6 + 6.0 * sx,
                    yy + (ph - side_h) * 0.5,
                    side_w,
                    side_h,
                ),
            );
            ui.add(
                side_box,
                Kind::Image {
                    sprite: *sprite,
                    fit: Fit::Contain,
                },
                rt2_ui::Layout::fill(),
            );
        }

        // Active center logo frame
        let logo_frame = ui.add(
            fr.root,
            Kind::Panel {
                fill: Fill::Solid(INK),
            },
            at(cx - pw / 2.0, yy, pw, ph),
        );
        if let Some(sprite) = self
            .icons
            .brand
            .logos
            .get(usize::from(logo))
            .copied()
            .flatten()
        {
            ui.add(
                logo_frame,
                Kind::Image {
                    sprite,
                    fit: Fit::Contain,
                },
                rt2_ui::Layout::fill(),
            );
        }
        ui.add(
            fr.root,
            label(
                &format!("{} / {}", logo + 1, BRAND_COUNT),
                f,
                CREAM,
                rt2_ui::Align::Center,
            ),
            at(cx - pw / 2.0, yy + ph + 4.0 * sx, pw, lh),
        );
        yy += ph + lh * 1.5;
        if let Some(e) = error {
            ui.add(
                fr.root,
                Kind::Label {
                    text: e.to_owned(),
                    font: game_font(8).with_size((size * 0.85).round().max(9.0)),
                    color: rt2_ui::Color::rgb(0xf0, 0x8c, 0x7a),
                    align: rt2_ui::Align::Left,
                    wrap: true,
                },
                at(x, yy, w, lh * 2.4),
            );
            yy += lh * 2.4;
        }
        let bw = 130.0 * sx;
        for (i, (text, action)) in [
            (t.get(crate::text::id::OK, "OK"), A_BRAND_OK),
            (t.get(crate::text::id::CANCEL, "Cancel"), super::A_DLG_CLOSE),
        ]
        .into_iter()
        .enumerate()
        {
            ui.add(
                fr.root,
                Kind::Button {
                    text,
                    font: f,
                    action,
                    enabled: true,
                    hotkey: None,
                },
                at(
                    cx - bw - 4.0 * sx + i as f32 * (bw + 8.0 * sx),
                    yy,
                    bw,
                    bh + 6.0 * sx,
                ),
            );
        }
        let _ = E_TEXT;
    }
}
