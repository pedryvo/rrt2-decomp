//! Map animation objects (`pool_78`, 0x23 bytes: the smoke and steam puffs of the trains) and
//! the tile structure chains they hang on (`0x574c54`, 7 bytes), trains.md 10.6.
//!
//! Animation record: `+0` state (1 drifting puff, 2 still puff), `+1` frame (0xff = not drawn
//! yet), `+2` sequence (record of `SMOKE.imb`), `+3`/`+5` u16 tile x/y, `+7`/`+8` sub-tile x/y,
//! `+9` u16 height, `+0x0b`/`+0x0c` i8 start x/y, `+0x0d` u16 start height, `+0x0f` u32 birth
//! minute, `+0x13`/`+0x17`/`+0x1b` f32 drift per unit of age (x, y, height), `+0x1f` f32 frames
//! per unit of age (one unit = 2160 minutes = 1.5 days).

use super::mem::{P, Ts};
use crate::fp::ftol;

/// Structure types in a tile chain (`+0` of a `0x574c54` record).
pub const STRUCT_ANIM: u8 = 1;
pub const STRUCT_CAR: u8 = 2;
pub const STRUCT_BUILDING: u8 = 3;
pub const STRUCT_STATION: u8 = 4;

impl Ts<'_> {
    // ---- tile structure chain -----------------------------------------------------------

    fn st_type(&self, id: i32) -> u8 {
        self.u8(P::Struct, id, 0)
    }
    fn st_next(&self, id: i32) -> i32 {
        i32::from(self.u16(P::Struct, id, 5))
    }

    /// `tile_struct_add` (`0x42e850`): inserts `(kind, id)` in the chain of tile `(x, y)`,
    /// which is kept in decreasing order of `kind` (the new record goes after the last one with
    /// a kind `>= kind`); kind 3 sets `tile+2 |= 0x10`. Returns the new structure slot.
    pub fn tile_struct_add(&mut self, x: i32, y: i32, kind: u8, id: u32) -> i32 {
        let mut cur = i32::from(self.t16(x, y, 4));
        let mut prev = 0;
        while cur != 0 {
            if self.st_type(cur) < kind {
                break;
            }
            prev = cur;
            cur = self.st_next(cur);
        }
        let mut rec = vec![kind];
        rec.extend_from_slice(&id.to_le_bytes());
        rec.extend_from_slice(&(cur as u16).to_le_bytes());
        let new = self.alloc(P::Struct, rec);
        if prev == 0 {
            self.set_t16(x, y, 4, new as u16);
        } else {
            self.set16(P::Struct, prev, 5, new as u16);
        }
        if kind == STRUCT_BUILDING {
            let f = self.t8(x, y, 2);
            self.set_t8(x, y, 2, f | 0x10);
        }
        new
    }

    /// `tile_struct_remove` (`0x42e320`): unlinks and frees the first `(kind, id)` of the chain
    /// of tile `(x, y)`, then recomputes `tile+2 & 0x10` (a kind-3 structure is left).
    pub fn tile_struct_remove(&mut self, x: i32, y: i32, kind: u8, id: u32) {
        let head = i32::from(self.t16(x, y, 4));
        if head == 0 {
            return;
        }
        // `link` = where the pointer to `cur` lives: None = the tile, Some(s) = struct s +5.
        let mut link: Option<i32> = None;
        let mut cur = head;
        loop {
            if self.u32(P::Struct, cur, 1) == id && self.st_type(cur) == kind {
                break;
            }
            let nx = self.st_next(cur);
            if nx == 0 {
                return;
            }
            link = Some(cur);
            cur = nx;
        }
        let nx = self.st_next(cur);
        self.free(P::Struct, cur);
        match link {
            None => self.set_t16(x, y, 4, nx as u16),
            Some(s) => self.set16(P::Struct, s, 5, nx as u16),
        }
        let mut c = i32::from(self.t16(x, y, 4));
        let f = self.t8(x, y, 2);
        while c != 0 {
            if self.st_type(c) == STRUCT_BUILDING {
                self.set_t8(x, y, 2, f | 0x10);
                return;
            }
            c = self.st_next(c);
        }
        self.set_t8(x, y, 2, f & 0xef);
    }

    /// `tile_struct_find` (`0x42e8f0`): the first structure of `kind` on the tile (0 = none).
    pub fn tile_struct_find(&self, x: i32, y: i32, kind: u8) -> i32 {
        let mut c = i32::from(self.t16(x, y, 4));
        while c != 0 {
            if self.st_type(c) == kind {
                return c;
            }
            c = self.st_next(c);
        }
        0
    }

    /// `station_at` (`0x4bf1d0`): the station whose structure (kind 4) is on the tile, 0 if none.
    pub fn station_at(&self, x: i32, y: i32) -> i32 {
        let s = self.tile_struct_find(x, y, STRUCT_STATION);
        if s == 0 { 0 } else { self.i32(P::Struct, s, 1) }
    }

    // ---- anim_object_step (0x4283e0) -----------------------------------------------------

    /// `anim_object_step` (`0x4283e0`, world-tick.md 2.1): advances the frame and the drift of
    /// a puff from its age; deletes it when the frame passes the sequence length.
    pub fn anim_object_step(&mut self, id: i32) {
        let state = self.u8(P::Anim, id, 0);
        if state == 0 || state > 2 {
            return;
        }
        let age = self.now.wrapping_sub(self.u32(P::Anim, id, 0x0f));
        let mut t = (f64::from(age) * f64::from(self.img.f32(0x0053_52f8))) as f32;
        if f64::from(t) < f64::from(self.img.f32(0x0053_500c)) {
            t = 0.0;
        }
        let t = f64::from(t);
        let frame = ftol(t * f64::from(self.f32(P::Anim, id, 0x1f)));
        let seq = usize::from(self.u8(P::Anim, id, 2));
        let frames = self.anim_frames.get(seq).copied().unwrap_or(0);
        if frame >= frames {
            self.anim_object_delete(id);
            return;
        }
        self.set8(P::Anim, id, 1, frame as u8);
        let x = ftol(
            t * f64::from(self.f32(P::Anim, id, 0x13)) + f64::from(self.i8(P::Anim, id, 0x0b)),
        );
        self.set8(P::Anim, id, 7, x as u8);
        let y = ftol(
            t * f64::from(self.f32(P::Anim, id, 0x17)) + f64::from(self.i8(P::Anim, id, 0x0c)),
        );
        self.set8(P::Anim, id, 8, y as u8);
        let z = ftol(
            t * f64::from(self.f32(P::Anim, id, 0x1b)) + f64::from(self.u16(P::Anim, id, 0x0d)),
        );
        self.set16(P::Anim, id, 9, z as u16);
    }

    /// `anim_object_delete` (`0x466d20`): removes the puff from its tile chain and frees it.
    pub fn anim_object_delete(&mut self, id: i32) {
        let x = i32::from(self.u16(P::Anim, id, 3));
        let y = i32::from(self.u16(P::Anim, id, 5));
        self.tile_struct_remove(x, y, STRUCT_ANIM, id as u32);
        self.free(P::Anim, id);
    }

    // ---- smoke (0x412900, 0x412a90) ------------------------------------------------------

    /// `car_smoke_step` (`0x412900`): emits the puffs a car owes since its last one. A steam
    /// engine (vehicle `+0x2b != 0`) of a train that is neither broken down nor crashed puffs
    /// every `engine.+0x36` minutes; the head car of a broken or crashed train puffs every
    /// `max(30, ftol(50 / length * 100))` minutes (x0.7 when crashed). The backlog is capped to
    /// 3000 minutes.
    pub fn car_smoke_step(&mut self, car: i32) {
        if self.u8(P::Car, car, 0x2a) == 0 {
            return;
        }
        let train = i32::from(self.i16(P::Car, car, 4));
        let broken = self.u8(P::Train, train, 0xed) != 0;
        let crashed = self.u8(P::Train, train, 0xf2) != 0;
        let vt = self.u8(P::Car, car, 8);
        let interval = if !broken && !crashed {
            if self.vt8(vt, 0x2b) == 0 {
                return;
            }
            self.cap_smoke_backlog(car);
            self.en32(self.vt_engine(vt), 0x36)
        } else {
            if self.i32(P::Train, train, 0) != car {
                return;
            }
            self.cap_smoke_backlog(car);
            let len = f64::from(self.vt16(vt, 6));
            let mut i = ftol(self.img.f64(0x0053_51b8) / len * self.img.f64(0x0053_51a8));
            if i < 0x1e {
                i = 0x1e;
            }
            if crashed {
                i = ftol(f64::from(i) * self.img.f64(0x0053_50f8));
            }
            i
        };
        let mut at = self.u32(P::Car, car, 0x3d).wrapping_add(interval as u32);
        while at < self.now {
            self.car_smoke_puff(car, at);
            at = self.u32(P::Car, car, 0x3d).wrapping_add(interval as u32);
        }
    }

    fn cap_smoke_backlog(&mut self, car: i32) {
        let floor = self.now.wrapping_sub(3000);
        if self.u32(P::Car, car, 0x3d) < floor {
            self.set32(P::Car, car, 0x3d, floor);
        }
    }

    /// `car_smoke_puff` (`0x412a90`): creates one puff born at minute `at` over the car (CRT
    /// rand for the drift, the lifetime and, for broken or crashed trains, the position and the
    /// kind), hangs it on its tile and steps it once.
    pub fn car_smoke_puff(&mut self, car: i32, at: u32) {
        self.set32(P::Car, car, 0x41, 0x60000);
        self.set32(P::Car, car, 0x3d, at);
        let id = self.alloc(P::Anim, vec![0u8; 0x23]);
        let train = i32::from(self.i16(P::Car, car, 4));
        let broken = self.u8(P::Train, train, 0xed) != 0;
        let crashed = self.u8(P::Train, train, 0xf2) != 0;
        let vt = self.u8(P::Car, car, 8);
        self.set8(P::Anim, id, 0, 1);
        self.set8(P::Anim, id, 1, 0xff);
        let z0 = if broken {
            0
        } else {
            u16::from(self.vt8(vt, 0x2d))
        };
        self.set16(P::Anim, id, 0x0d, z0);
        let seq = self.en8(self.vt_engine(vt), 0x32);
        self.set8(P::Anim, id, 2, seq);
        self.set16(P::Anim, id, 3, self.u16(P::Car, car, 9));
        self.set16(P::Anim, id, 5, self.u16(P::Car, car, 0xb));
        let along = if !broken && !crashed {
            i32::from(self.vt8(vt, 0x2c) as i8)
        } else {
            let len = i32::from(self.vt16(vt, 6));
            let r = self.crt.range(0, len) - (len >> 1);
            ftol(f64::from(r) * self.img.f64(0x0053_50f8))
        };
        let pos = (self.i32(P::Car, car, 0x17) >> 16) + along + i32::from(self.vt8(vt, 0x26) as i8);
        let (mut x, mut y, _) = self.track_point(
            u32::from(self.u8(P::Car, car, 0x23)),
            u32::from(self.u8(P::Car, car, 0x24)),
            pos,
            u32::from(self.u8(P::Car, car, 0x25)),
        );
        let tx = self.u16(P::Anim, id, 3);
        if x < 0 && tx != 0 {
            x += 0x40;
            self.set16(P::Anim, id, 3, tx - 1);
        }
        let tx = self.u16(P::Anim, id, 3);
        if x >= 0x40 && i32::from(tx) < self.map_w() - 1 {
            x -= 0x40;
            self.set16(P::Anim, id, 3, tx + 1);
        }
        let ty = self.u16(P::Anim, id, 5);
        if y < 0 && ty != 0 {
            y += 0x40;
            self.set16(P::Anim, id, 5, ty - 1);
        }
        let ty = self.u16(P::Anim, id, 5);
        if y >= 0x40 && i32::from(ty) < self.map_h() - 1 {
            y -= 0x40;
            self.set16(P::Anim, id, 5, ty + 1);
        }
        self.set8(P::Anim, id, 0x0b, x as u8);
        self.set8(P::Anim, id, 0x0c, y as u8);
        self.set32(P::Anim, id, 0x0f, at);
        let k = self.img.f64(0x0053_5010);
        let r = self.crt.range(-2000, 2000);
        self.setf32(P::Anim, id, 0x13, (f64::from(r) * k) as f32);
        let r = self.crt.range(-2000, 2000);
        self.setf32(P::Anim, id, 0x17, (f64::from(r) * k) as f32);
        let r = self.crt.range(0x19, 0x23);
        self.setf32(P::Anim, id, 0x1b, (r << 1) as f32);
        let r = self.crt.range(0xdac, 5000);
        let rate = f64::from(r) * k;
        self.setf32(P::Anim, id, 0x1f, rate as f32);
        let c8 = self.img.f64(0x0053_51d8);
        if broken {
            self.set8(P::Anim, id, 2, 6);
            let z = f64::from(self.f32(P::Anim, id, 0x1b)) * c8;
            self.setf32(P::Anim, id, 0x1b, z as f32);
            self.setf32(P::Anim, id, 0x1f, (rate * c8) as f32);
        }
        if crashed {
            let r = self.crt.range(0, 100);
            let (lo, hi) = if r < 0x1e {
                self.set8(P::Anim, id, 0, 2);
                self.set32(P::Anim, id, 0x13, 0);
                self.set32(P::Anim, id, 0x17, 0);
                self.set8(P::Anim, id, 2, 8);
                let z = f64::from(self.f32(P::Anim, id, 0x1b)) * self.img.f64(0x0053_51c8);
                self.setf32(P::Anim, id, 0x1b, z as f32);
                (0, 8)
            } else {
                self.set8(P::Anim, id, 2, 6);
                let z = f64::from(self.f32(P::Anim, id, 0x1b)) * c8;
                self.setf32(P::Anim, id, 0x1b, z as f32);
                let f = f64::from(self.f32(P::Anim, id, 0x1f)) * c8;
                self.setf32(P::Anim, id, 0x1f, f as f32);
                (8, 0x10)
            };
            let r = self.crt.range(lo, hi);
            self.set16(P::Anim, id, 0x0d, r as u16);
        }
        let tx = i32::from(self.u16(P::Anim, id, 3));
        let ty = i32::from(self.u16(P::Anim, id, 5));
        self.tile_struct_add(tx, ty, STRUCT_ANIM, id as u32);
        self.anim_object_step(id);
    }
}
