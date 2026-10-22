//! Engine choice (`ai_pick_engine` `0x4d86f0`, ai.md section 4) and the manager review
//! (`ai_review_manager` `0x436880`, ai.md section 6).

use super::{Ai, CO_CHAIRMAN};
use crate::fp::ftol;
use crate::trainstep::P;

/// Engines in the table (`0x53cec0`, 66 records of 0x3a bytes).
pub const N_ENGINES: i32 = 66;
/// `engine_allowed` (`0x570667`, u8 per engine; state block).
const ENGINE_ALLOWED: u32 = 0x0057_0667;

impl Ai<'_, '_> {
    /// First/last year of the loco vehicle of engine `e` (vehicle `engine+0x2c`, `+0x1c/+0x1e`).
    fn engine_years(&self, e: i32) -> (u16, u16) {
        let v = self.ts.en8(e, 0x2c);
        (self.ts.vt16(v, 0x1c), self.ts.vt16(v, 0x1e))
    }

    /// `engine_available(e, company)` (`0x4683a0`, trains.md 2.4).
    pub fn engine_available(&self, e: i32, company: i32) -> bool {
        let id = if e < 0x3e { e + 0x7f } else { e + 0xf3 } as usize;
        let m = self.effect_sum(id, company);
        if m < 0 {
            return false;
        }
        if m > 0 {
            return true;
        }
        if self.ts.en8(e, 0) == 0 || self.st8(ENGINE_ALLOWED + e as u32) == 0 {
            return false;
        }
        let (first, last) = self.engine_years(e);
        let (year, month) = (self.ts.year, self.month());
        (first < year || (year == first && month > 5)) && year <= last
    }

    /// `ai_pick_engine(mode)` (`0x4d86f0`) [V]: one `rng_range(80, 120)` per candidate; the
    /// best score wins (strict `>`, ties keep the first). Engine `-1` (none) becomes 0.
    pub fn pick_engine(&mut self, mode: i32) -> i32 {
        self.ts.log.ran("ai_pick_engine");
        let mut best = -1i32;
        let mut best_score = -1i32;
        let year = self.ts.year;
        for pass in 0..3 {
            if best != -1 {
                continue;
            }
            for e in 0..N_ENGINES {
                if self.ts.en8(e, 0x28) == 2 && pass < 1 {
                    continue;
                }
                let ok = self.engine_available(e, self.company) || {
                    let (first, last) = self.engine_years(e);
                    pass >= 2 && first < year && year <= last
                };
                if !ok {
                    continue;
                }
                let speed = self.ts.en32(e, 0x14);
                let attr = if mode != 0 {
                    speed
                } else {
                    speed.wrapping_mul(self.ts.en32(e, 0x18))
                };
                let r = self.ts.rng.range(80, 120);
                let score = ftol(f64::from(r) * 0.01 * f64::from(attr));
                if score > best_score {
                    best_score = score;
                    best = e;
                }
            }
        }
        if best < 1 { 0 } else { best }
    }

    /// `ai_review_manager` (`0x436880`, ai.md 6): once in odd years. The value of each manager
    /// (`ai_manager_value` `0x436210`) draws `ai_pick_engine` twice per effect `0x1a..0x1d`.
    pub fn review_manager(&mut self) {
        let year = self.year() as u32;
        let c = self.company;
        self.ts.set32(P::Company, c, super::CO_MANAGER_YEAR, year);
        let _ = CO_CHAIRMAN;
        todo_hook!(
            self.ts.log,
            "ai_review_manager",
            "ai_review_manager 0x436880 / ai_manager_value 0x436210 (RNG via ai_pick_engine) and company_set_manager 0x438780",
            "ai.md 6"
        );
    }
}
