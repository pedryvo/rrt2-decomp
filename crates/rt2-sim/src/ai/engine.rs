//! Engine choice (`ai_pick_engine` `0x4d86f0`, ai.md section 4) and the manager review
//! (`ai_review_manager` `0x436880`, ai.md section 6).

use super::{Ai, CO_CHAIRMAN};
use crate::company_record::{CO_ACCT, CO_ARCH_ACCT, CO_EFFECTS};
use crate::fp::ftol;
use crate::tick::history_co;
use crate::trainstep::P;

/// Manager table (`0x54fdad`, 40 records of 0x11 bytes: salary, four `(effect, pct)` pairs).
const MANAGER_TABLE: u32 = 0x0054_fdad;
const N_MANAGERS: usize = 40;
/// `0x570638` (state `+0x150`): manager available; `0x573e09` (`+0x3921`): his company.
const ST_MANAGER_AVAIL: usize = 0x150;
const ST_MANAGER_USE: usize = 0x3921;
/// The company set `0x570729` (count) / `0x57072d` (u16 ids), state offsets.
const ST_SET_COUNT: usize = 0x241;
const ST_SET_IDS: usize = 0x245;
/// `company+0x13c`: the manager (-1 = none).
const CO_MANAGER: usize = 0x13c;

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

    /// `ai_review_manager` (`0x436880`, ai.md 6): once in odd years. Among the managers free or
    /// already its own (`0x573e09[g]`) and available (`0x570638[g]`), the best
    /// `ai_manager_value(g) - salary` (strict, from -9999999); the current one stays when
    /// `(best - value(current)) * 0.8 < salary(best) * 0.4`; otherwise
    /// `company_set_manager(best, 0)` unless he already works here.
    pub fn review_manager(&mut self) {
        let year = self.year() as u32;
        let c = self.company;
        self.ts.set32(P::Company, c, super::CO_MANAGER_YEAR, year);
        let _ = CO_CHAIRMAN;
        let mut best = 0usize;
        let mut best_v = -9_999_999i32;
        for g in 0..N_MANAGERS {
            let used = self.ts.s.state[ST_MANAGER_USE + g];
            if (used == 0 || used == c as u8) && self.ts.s.state[ST_MANAGER_AVAIL + g] != 0 {
                let v = self.manager_value(g).wrapping_sub(self.manager_salary(g));
                if best_v < v {
                    best = g;
                    best_v = v;
                }
            }
        }
        let cur = self.co_i32(CO_MANAGER);
        if (0..N_MANAGERS as i32).contains(&cur) {
            let vc = self.manager_value(cur as usize);
            if f64::from(best_v.wrapping_sub(vc)) * self.ts.img.f64(0x0053_51d8)
                < f64::from(self.manager_salary(best)) * self.ts.img.f64(0x0053_54c0)
            {
                self.ts.log.ran("ai_review_manager");
                return;
            }
        }
        if self.ts.s.state[ST_MANAGER_USE + best] != c as u8 {
            self.set_manager(best);
        }
        self.ts.log.ran("ai_review_manager");
    }

    /// Salary of manager `g` (table `0x54fdad`, `+0`).
    fn manager_salary(&self, g: usize) -> i32 {
        self.ts.img.i32(MANAGER_TABLE + 0x11 * g as u32)
    }

    /// `0x437980(company, kind)`: the company's trains whose engine (`0x4cfc10`: the head
    /// car's vehicle type `+0x2f`) is of kind `engine+0x28 == kind`, in list order.
    fn trains_of_kind(&self, kind: u8) -> i32 {
        let mut n = 0;
        let mut t = self.ts.first_id(P::Train);
        while t != 0 {
            if self.ts.i32(P::Train, t, 0x41) == self.company {
                let head = self.ts.i32(P::Train, t, 0);
                let e = self.ts.vt_engine(self.ts.u8(P::Car, head, 8));
                if self.ts.en8(e, 0x28) == kind {
                    n += 1;
                }
            }
            t = self.ts.next_id(P::Train, t);
        }
        n
    }

    /// `ai_manager_value(g)` (`0x436210`, ai.md 6): 0 for a company less than a year old;
    /// otherwise the sum over the manager's four `(effect, pct)` pairs of a term from last
    /// year's categories (`0x43a7d0`), accounts (`0x43a0a0`/`0x43a060`) or engine prices, each
    /// `ftol`'d and weighted 0.75 when positive (`(v >> 2) + (v >> 1)`) or 1.25 when negative.
    /// The pairs `0x1a..0x1d` draw `ai_pick_engine(1)` twice.
    fn manager_value(&mut self, g: usize) -> i32 {
        let year = self.year();
        let founded = self.co_i32(0x148);
        if year - founded < 1 {
            return 0;
        }
        let img = self.ts.img;
        let k = |a: u32| img.f64(a);
        let mut total = 0i32;
        for pair in 0..4u32 {
            let base = MANAGER_TABLE + 4 + 0x11 * g as u32 + 2 * pair;
            let effect = img.u8(base);
            let pct = img.i8(base + 1);
            let p = f64::from((f64::from(pct) * k(0x0053_50b8)) as f32);
            let rec = self.ts.record(P::Company, self.company);
            let cat = |i: usize| history_co::cat_archive(&rec, year, year - 1, i);
            let trains = f64::from(self.co_i32(0x27).max(1));
            let v = match effect {
                0x3d => ftol((cat(0) + cat(1)) * p),
                0x50 | 0x59 => ftol(cat(1) * k(0x0053_5450) * p),
                0x27 => ftol(cat(14) * k(0x0053_5450) * p),
                0x3f => ftol(cat(1) * k(0x0053_54c0) * p),
                0x3e => ftol(cat(0) * p),
                0x3c => ftol(cat(2) * p),
                0x0c => ftol(cat(5) * p),
                0x19 => ftol(cat(7) * p),
                0x15 => ftol(cat(6) * p),
                0x0f => ftol(cat(14) * k(0x0053_50b8) * p),
                0x11 => ftol(cat(8) * p),
                0x12 => ftol(f64::from(self.trains_of_kind(0)) * cat(8) / trains * p),
                0x14 => ftol(f64::from(self.trains_of_kind(2)) * cat(8) / trains * p),
                0x29 => ftol(cat(14) * k(0x0053_5480) * p),
                0x2e => ftol(cat(14) * k(0x0053_5470) * p),
                0x23 => ftol(cat(14) * k(0x0053_50e8) * p),
                0x31 => {
                    ftol(f64::from(self.trains_of_kind(2)) * cat(14) * k(0x0053_5470) / trains * p)
                }
                0x24 => {
                    ftol(f64::from(self.trains_of_kind(0)) * cat(14) * k(0x0053_50e8) / trains * p)
                }
                0x2b => {
                    ftol(f64::from(self.trains_of_kind(0)) * cat(14) * k(0x0053_5460) / trains * p)
                }
                0x1f => {
                    let acct6 = crate::pool::f64_at(&rec, CO_ACCT + 6 * 8);
                    ftol(acct6 * f64::from(-i32::from(pct)) * k(0x0053_5490))
                }
                0x08 | 0x09 | 0x0a | 0x0e => {
                    let x = if year - founded < 3 {
                        300_000
                    } else {
                        // `0x43a0a0(co, y, 3)` for a past year: archive row `year - y - 1`.
                        let acct3 = |y: i32| {
                            crate::pool::f64_at(
                                &rec,
                                CO_ARCH_ACCT + (3 + (year - y - 1) as usize * 8) * 8,
                            )
                        };
                        ftol((acct3(year - 1) - acct3(year - 3)) * k(0x0053_54a0))
                    };
                    let c = match effect {
                        0x0e => k(0x0053_5178),
                        0x08 => k(0x0053_50e8),
                        0x09 => k(0x0053_50f8),
                        _ => k(0x0053_51c8),
                    };
                    ftol(f64::from(x) * p * c)
                }
                0x1a..=0x1d => {
                    let f = if year - founded < 5 {
                        2.0f32
                    } else {
                        let v = f64::from(self.co_i32(0x27)) * k(0x0053_54b0);
                        (if v > k(0x0053_50c8) {
                            v
                        } else {
                            k(0x0053_50c8)
                        }) as f32
                    };
                    let e1 = self.pick_engine(1);
                    let kind = self.ts.en8(e1, 0x28);
                    let (frac, home) = match effect {
                        0x1a => (1.0f32, 1.0f32),
                        _ => {
                            let t = effect - 0x1b;
                            let tr = self.co_i32(0x27).max(1);
                            let n = self.trains_of_kind(t);
                            (
                                (f64::from(n) / f64::from(tr)) as f32,
                                f32::from(u8::from(kind == t)),
                            )
                        }
                    };
                    let e2 = self.pick_engine(1);
                    let price = f64::from(self.ts.en32(e2, 0xc));
                    let mix = f64::from(home) * k(0x0053_50f8) + f64::from(frac) * k(0x0053_50e8);
                    ftol(price * mix * f64::from(f) * p)
                }
                _ => 0,
            };
            if v != 0 {
                total = total.wrapping_add(if v > 0 {
                    (v >> 2) + (v >> 1)
                } else {
                    ftol(f64::from(v) * k(0x0053_53e0))
                });
            }
        }
        total
    }

    /// `company_set_manager(company, g, 0)` (`0x438780`, finance.md 14.3) without the signing
    /// fee: the current manager's effects come off and his slot is freed, `+0x13c = g`, `g`'s
    /// effects go on and `0x573e09[g] = company`; every company of the set whose candidate
    /// `+0x140`/`+0x144` was `g` draws a new one (`manager_pick_candidate`, game RNG).
    fn set_manager(&mut self, g: usize) {
        let c = self.company;
        if g >= N_MANAGERS || self.ts.s.state[ST_MANAGER_USE + g] != 0 {
            return;
        }
        let old = self.co_i32(CO_MANAGER);
        if (0..N_MANAGERS as i32).contains(&old) {
            self.manager_effects(c, old as usize, true);
            self.ts.s.state[ST_MANAGER_USE + old as usize] = 0;
        }
        self.co_set_i32(CO_MANAGER, g as i32);
        self.manager_effects(c, g, false);
        self.ts.s.state[ST_MANAGER_USE + g] = c as u8;
        let st = &self.ts.s.state;
        let n = (crate::pool::i32_at(st, ST_SET_COUNT).max(0) as usize).min(36);
        let set: Vec<i32> = (0..n)
            .map(|i| i32::from(crate::pool::u16_at(st, ST_SET_IDS + 2 * i)))
            .collect();
        for cc in set {
            for off in [0x140usize, 0x144] {
                if self.ts.exists(P::Company, cc) && self.ts.i32(P::Company, cc, off) == g as i32 {
                    let pick = self.manager_pick(cc);
                    self.ts.seti32(P::Company, cc, off, pick);
                }
            }
        }
        self.ts.log.ran("company_set_manager");
    }

    /// `0x438710(g, remove)`: the manager's four `(effect, pct)` pairs added to (or taken from)
    /// the company's effect levels, each clamped to +-127.
    fn manager_effects(&mut self, c: i32, g: usize, remove: bool) {
        let base = MANAGER_TABLE + 4 + 0x11 * g as u32;
        for k in 0..4u32 {
            let e = self.ts.img.u8(base + 2 * k);
            if e == 0 {
                continue;
            }
            let mut pct = i32::from(self.ts.img.i8(base + 2 * k + 1));
            if remove {
                pct = -pct;
            }
            let o = CO_EFFECTS + 2 * usize::from(e);
            let v = (i32::from(self.ts.i16(P::Company, c, o)) + pct).clamp(-0x7f, 0x7f);
            self.ts.set16(P::Company, c, o, v as i16 as u16);
        }
    }

    /// `manager_pick_candidate(-1)` (`0x438450`) for company `cc` (game RNG).
    fn manager_pick(&mut self, cc: i32) -> i32 {
        let rec = self.ts.record(P::Company, cc);
        let co = crate::finance::Company::from_record(&rec);
        let st = &self.ts.s.state;
        let mut uses = [0u8; 40];
        uses.copy_from_slice(&st[ST_MANAGER_USE..ST_MANAGER_USE + 40]);
        let mut avail = [0u8; 40];
        avail.copy_from_slice(&st[ST_MANAGER_AVAIL..ST_MANAGER_AVAIL + 40]);
        let mut sal = [0i32; 40];
        for (g, v) in sal.iter_mut().enumerate() {
            *v = self.manager_salary(g);
        }
        let inp = crate::tick::PickIn {
            year: self.year(),
            month: self.month(),
            founded: co.founded,
            cats: &co.cats,
            arch: &co.arch_cats,
            c140: crate::pool::i32_at(&rec, 0x140),
            c144: crate::pool::i32_at(&rec, 0x144),
            uses,
            avail,
            sal,
        };
        crate::tick::pick_manager(&inp, self.ts.rng)
    }
}
