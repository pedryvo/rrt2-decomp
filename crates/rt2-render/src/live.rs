//! The live layer: what moves between simulation steps (cars of the trains and smoke puffs), read
//! from the save pools the simulation writes. Read-only; nothing here feeds back into the
//! simulation, and interpolation between steps is purely a drawing matter.
//!
//! Field offsets: `docs/spec/trains.md` 10.1 (car, 0x4f bytes) and 10.6 (smoke, `pool_78`).

use rt2_formats::map::{Pool, Save};

/// One car at a simulation step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarSample {
    /// Slot in the car pool (stable identity across steps).
    pub slot: u32,
    /// Vehicle type (`vehicle_type_table` index, car `+8`).
    pub vehicle: u8,
    /// Ground position in tile units (x east, y south): tile + sub-tile offset / 64.
    pub x: f32,
    pub y: f32,
    /// Drawing direction 0..8 (car `+0x26`, `track_point` output).
    pub dir: u8,
}

/// One smoke puff (`pool_78`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokeSample {
    pub slot: u32,
    /// Sequence = record of `SMOKE.imb`.
    pub seq: u8,
    /// Animation frame = image of group 0 of the record.
    pub frame: u8,
    pub x: f32,
    pub y: f32,
    /// Height above the ground in sprite pixels at native scale.
    pub height: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveFrame {
    /// Sorted by slot.
    pub cars: Vec<CarSample>,
    pub smoke: Vec<SmokeSample>,
}

fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn i16_at(d: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([d[o], d[o + 1]])
}
fn i32_at(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// Smoke x/y are bytes holding a possibly negative drift (`anim_object_step` stores `ftol` as u8).
fn drift_byte(v: u8) -> f32 {
    if v >= 192 { v as i8 as f32 } else { v as f32 }
}

impl LiveFrame {
    pub fn from_save(save: &Save) -> LiveFrame {
        let mut frame = LiveFrame::default();
        read_cars(&save.pool_70, &mut frame.cars);
        for r in &save.pool_78.records {
            let d = &r.data;
            if d.len() < 0x23 || d[0] == 0 || d[0] > 2 || d[1] == 0xff {
                continue; // unused or just born (frame 0xff until its first step)
            }
            let (tx, ty) = (u16_at(d, 3), u16_at(d, 5));
            frame.smoke.push(SmokeSample {
                slot: r.slot as u32,
                seq: d[2],
                frame: d[1],
                x: tx as f32 + drift_byte(d[7]) / 64.0,
                y: ty as f32 + drift_byte(d[8]) / 64.0,
                height: u16_at(d, 9) as f32,
            });
        }
        frame
    }
}

fn read_cars(pool: &Pool, out: &mut Vec<CarSample>) {
    for r in &pool.records {
        let d = &r.data;
        if d.len() < 0x2b || d[0x2a] == 0 {
            continue; // not on the rails
        }
        let (tx, ty) = (i16_at(d, 9), i16_at(d, 0xb));
        if tx < 0 || ty < 0 {
            continue;
        }
        let (sx, sy) = (i32_at(d, 0x1b), i32_at(d, 0x1f));
        out.push(CarSample {
            slot: r.slot as u32,
            vehicle: d[8],
            x: tx as f32 + sx as f32 / 64.0,
            y: ty as f32 + sy as f32 / 64.0,
            dir: d[0x26] & 7,
        });
    }
}

impl LiveFrame {
    /// Cars of `self` moved a fraction `alpha` (0..=1) of the way towards `next`, matched by slot.
    /// Cars that did not exist before, changed type, or jumped more than two tiles are taken as
    /// they are in `next`.
    pub fn lerp(&self, next: &LiveFrame, alpha: f32) -> Vec<CarSample> {
        let a = alpha.clamp(0.0, 1.0);
        next.cars
            .iter()
            .map(|c| {
                let Ok(i) = self.cars.binary_search_by_key(&c.slot, |p| p.slot) else {
                    return *c;
                };
                let p = &self.cars[i];
                let (dx, dy) = (c.x - p.x, c.y - p.y);
                if p.vehicle != c.vehicle || dx * dx + dy * dy > 4.0 {
                    return *c;
                }
                CarSample {
                    x: p.x + dx * a,
                    y: p.y + dy * a,
                    ..*c
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(slot: u32, x: f32) -> CarSample {
        CarSample {
            slot,
            vehicle: 1,
            x,
            y: 0.0,
            dir: 2,
        }
    }

    #[test]
    fn lerp_matches_by_slot_and_snaps_jumps() {
        let a = LiveFrame {
            cars: vec![car(1, 1.0), car(2, 5.0)],
            smoke: vec![],
        };
        let b = LiveFrame {
            cars: vec![car(1, 2.0), car(2, 20.0), car(3, 7.0)],
            smoke: vec![],
        };
        let m = a.lerp(&b, 0.5);
        assert_eq!(m[0].x, 1.5);
        assert_eq!(m[1].x, 20.0);
        assert_eq!(m[2].x, 7.0);
    }

    #[test]
    fn drift_bytes_are_signed_when_high() {
        assert_eq!(drift_byte(10), 10.0);
        assert_eq!(drift_byte(250), -6.0);
    }

    #[test]
    #[ignore = "needs RT2_DIR"]
    fn dump_example() {
        let dir = std::env::var("RT2_DIR").unwrap();
        let raw = std::fs::read(format!("{dir}/games/Example save game.GAM")).unwrap();
        let save = Save::parse(&raw).unwrap();
        let f = LiveFrame::from_save(&save);
        for c in f.cars.iter().take(12) {
            let d = &save.pool_70.get(c.slot as usize).unwrap().data;
            println!(
                "{c:?} tile=({},{}) raw={:02x?}",
                i16_at(d, 9),
                i16_at(d, 0xb),
                &d[0x13..0x2b]
            );
        }
        println!("cars {} smoke {}", f.cars.len(), f.smoke.len());
        for s in f.smoke.iter().take(5) {
            println!("{s:?}");
        }
    }
}
