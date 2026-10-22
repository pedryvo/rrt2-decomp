//! Cargo economy (docs/spec/cargo.md): delivery revenue, industry production rate and input
//! rebuild. All of it is pure: the caller supplies already-resolved plain data.
//!
//! The numeric tables come from the original executable at run time ([`crate::data`]).

use crate::calendar::MINUTES_PER_YEAR;
use crate::fp::ftol;
use crate::rng::hash_range;

// ---- cargo table (cargo.md section 3) ----------------------------------------------------------

/// One record of `cargo_table` (`0x5404b1`, 0x21 bytes), with the runtime fix-ups of `0x471b10`
/// already applied (`price = ftol(raw * 1.4)`, `wait_scale = days * 1440`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CargoType {
    /// `+0x00`, runtime price.
    pub price: i32,
    /// `+0x04`, delivery term in days.
    pub days: i32,
    /// `+0x08`, runtime: `days * 1440` (lot lifetime scale).
    pub wait_scale: i32,
    /// `+0x0c`, distance sensitivity `s`.
    pub ship: f32,
    /// `+0x10`, first year available.
    pub first_year: u16,
    /// `+0x14`, last year towns accept the cargo (0 never, 9999 always).
    pub towns_until: i16,
    /// `+0x16`, same for cities.
    pub cities_until: i16,
    /// `+0x18`, demand per house.
    pub demand_per_house: f32,
    /// `+0x1c`, station upgrade bit that halves the waiting time (0 = none).
    pub storage_bit: i32,
}

/// Size of a cargo record and number of records.
pub const CARGO_RECORD_SIZE: usize = 0x21;
pub const CARGO_COUNT: usize = 46;

impl CargoType {
    /// Parses a raw record and applies the start-up fix-ups (cargo.md section 3).
    pub fn from_raw(r: &[u8]) -> CargoType {
        let i32at = |k: usize| i32::from_le_bytes([r[k], r[k + 1], r[k + 2], r[k + 3]]);
        let f32at = |k: usize| f32::from_bits(i32at(k) as u32);
        let i16at = |k: usize| i16::from_le_bytes([r[k], r[k + 1]]);
        let days = i32at(4);
        CargoType {
            price: ftol(f64::from(i32at(0)) * 1.4),
            days,
            wait_scale: days.wrapping_mul(0x5a0),
            ship: f32at(0xc),
            first_year: u16::from_le_bytes([r[0x10], r[0x11]]),
            towns_until: i16at(0x14),
            cities_until: i16at(0x16),
            demand_per_house: f32at(0x18),
            storage_bit: i32at(0x1c),
        }
    }
}

// ---- revenue tables ---------------------------------------------------------------------------

/// The `.data` tables used by the revenue formula (cargo.md 4.2, 4.4).
#[derive(Clone, Debug, PartialEq)]
pub struct RevenueTables {
    /// `0x550524` f32[10]: price factor by station price level (expert model).
    pub demand_level: [f32; 10],
    /// `0x55050c` f32[6]: undemanded cargo, `[human * 3 + model]`.
    pub undemanded: [f32; 6],
    /// `0x5504bc` f32[5]: revenue by economic state.
    pub econ_revenue: [f32; 5],
    /// `0x550448` f32[11]: human revenue slider.
    pub slider_human: [f32; 11],
    /// `0x55041c` f32[11]: computer revenue slider.
    pub slider_ai: [f32; 11],
}

// ---- delivery revenue (cargo.md section 4, `0x4bf760`, commit = 1) -----------------------------

/// A consumer industry or house in the destination station's catchment, in link order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Consumer {
    pub id: u16,
    /// Industry type (`+7`); 0x12 = house, 0x27 = large house.
    pub kind: u8,
    /// `inputs[8]` of the type (`industry_type_table +0x80`), after [`IndustryType::rebuild_inputs`].
    pub inputs: [u8; 8],
    /// Input stock (`+0x6b`).
    pub stock: [f32; 8],
}

/// Everything `cargo_delivery_revenue` reads, already resolved.
#[derive(Clone, Debug)]
pub struct Delivery<'a> {
    pub year: u16,
    /// `game_time_minutes`.
    pub now: u32,
    /// Economic state 0..=4.
    pub econ_state: usize,
    /// Industrial model 0 basic, 1 advanced, 2 expert.
    pub model: u8,
    /// Revenue sliders (`0x574874`): human, computer.
    pub slider_lo: u8,
    pub slider_hi: u8,
    pub cargo_id: usize,
    pub cargo: CargoType,
    pub lot_id: u32,
    /// `lot +1`: origin industry.
    pub lot_industry: u16,
    /// `lot +3`: origin station.
    pub lot_origin_station: u16,
    /// `lot +7`: creation time.
    pub t_created: u32,
    /// `lot +0xb`: loading time.
    pub t_loaded: u32,
    pub src_station_id: u32,
    pub src_xy: (i32, i32),
    /// Source station upgrade bitmask (`+0x49`).
    pub src_upgrades: u32,
    pub dst_station_id: u32,
    pub dst_xy: (i32, i32),
    /// `dest +0x195 + cargo`.
    pub dst_demands_cargo: bool,
    /// `dest +0x321 + cargo`, 0..=9.
    pub dst_price_level: u8,
    /// Catchment links of the destination (`+0x41` entries), in link order.
    pub consumers: &'a [Consumer],
    /// Train flags (`+0xbb`): bit 0 dining car, bit 2 passenger speed record.
    pub train_flags: u8,
    /// Company owner has a human player.
    pub human: bool,
    /// Company "computer factor" (`+0x4276`).
    pub computer_factor: f32,
    /// Company revenue multiplier (`+0x4248`).
    pub company_multiplier: f32,
    /// `effect_multiplier(0x3d, company, 0, territory)`.
    pub effect_all: f32,
    /// `effect_multiplier(0x3e + c or 0xf1 + c, company, 0, territory)`.
    pub effect_cargo: f32,
}

/// Result of [`delivery_revenue`]: `(*receita, *flag, *indústria)` of the original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revenue {
    pub amount: i32,
    pub flag: u32,
    /// Id of the consumer industry that takes the lot (0 if none).
    pub industry: u32,
}

/// `cargo_delivery_revenue` with `commit = 1` (no side effects), cargo.md section 4, bit-exact
/// at 53-bit x87 precision. Side effects of the paying branch (4.8) are TODO(cargo.md "Em aberto").
pub fn delivery_revenue(d: &Delivery, t: &RevenueTables) -> Revenue {
    let c = d.cargo_id;
    let cgo = &d.cargo;
    let year = i32::from(d.year);
    let human = d.human;

    // 4.1 distance and distance factor X
    let dx = f64::from(d.dst_xy.0 - d.src_xy.0);
    let dy = f64::from(d.dst_xy.1 - d.src_xy.1);
    let dist = ftol((dx * dx + dy * dy).sqrt());
    let yrs = year.min(2000) - 1800;
    let ratio = (f64::from(dist) * 1.4) / (f64::from(yrs) * 0.5 + 15.0);
    let s = f64::from(cgo.ship);
    let x: f32 = (ratio * s + (1.0 - s)) as f32;
    let xf = f64::from(x);

    // 4.2 base price from demand
    let mut r: i32;
    let mut industry = 0u32;
    let level_mul = |lvl: u8| -> f64 {
        if d.model >= 2 {
            f64::from(t.demand_level[usize::from(lvl.min(9))])
        } else {
            1.0
        }
    };
    if u32::from(d.lot_origin_station) == d.dst_station_id && human {
        r = 0;
    } else {
        r = 0;
        let mut accepted = false;
        if d.dst_demands_cargo && !d.consumers.is_empty() {
            let cnt = d.consumers.len() as i32;
            let off = hash_range(d.lot_id, 0, cnt - 1);
            let mut best = 9999.0f32;
            let mut best_id: Option<u32> = None;
            let mut housing = 0;
            for k in 0..cnt {
                let ind = &d.consumers[((off + k) % cnt) as usize];
                if ind.kind == 0x12 {
                    housing += 1;
                }
                if ind.kind == 0x27 {
                    housing += 2;
                }
                if ind.id != d.lot_industry || d.lot_industry == 0 {
                    for (slot, &ic) in ind.inputs.iter().enumerate() {
                        if usize::from(ic) == c && ind.stock[slot] < best {
                            best = ind.stock[slot];
                            best_id = Some(u32::from(ind.id));
                        }
                    }
                }
            }
            if let Some(id) = best_id {
                r = ftol(level_mul(d.dst_price_level) * f64::from(cgo.price));
                industry = id;
                accepted = true;
            } else if (housing > 7 && i32::from(cgo.cities_until) >= year)
                || (housing > 3 && i32::from(cgo.towns_until) >= year)
            {
                r = ftol(level_mul(d.dst_price_level) * f64::from(cgo.price));
                accepted = true;
            }
        }
        if !accepted {
            let idx = usize::from(human) * 3 + usize::from(d.model.min(2));
            r = ftol(f64::from(cgo.price) * f64::from(t.undemanded[idx]));
            if d.model >= 2 && dist < 16 {
                r = r.wrapping_mul(dist) >> 4;
            }
        }
    }
    if r != 0 && dist < 20 {
        r = (dist + 1).wrapping_mul(r) / 21;
    }

    // 4.3 time penalty
    let mut p1 = d.t_loaded.wrapping_sub(d.t_created) / 1440;
    if cgo.storage_bit != 0 && d.src_upgrades & (1u32 << (cgo.storage_bit & 31)) != 0 {
        p1 = ((p1 as i32) >> 1) as u32;
    }
    let p2 = d.now.wrapping_sub(d.t_loaded) / 1440;
    let total = p2.wrapping_add(p1) as i32;
    let allowed = ftol(f64::from(cgo.days) * xf);
    let yv = if year > 2000 {
        ((2000 - 1920) >> 1) + 1920
    } else if year < 1850 {
        year * 3 - 3700
    } else if year <= 1920 {
        year
    } else {
        ((year - 1920) >> 1) + 1920
    };
    let diff = 1960 - yv;
    let mut allowed2 = ftol((f64::from(diff) * 0.005 + 1.0) * f64::from(allowed));
    if allowed2 <= 1 {
        allowed2 = 1;
    }
    let e = total.max(0);
    let e2 = if human {
        e
    } else {
        ftol(f64::from(e) - f64::from(d.computer_factor) * f64::from(e) * 0.25)
    };
    let rem = allowed2.wrapping_sub(e2);
    let mut q = f64::from(rem) / f64::from(allowed2);
    if q < 0.0 {
        q = 0.0;
    }
    let tf = q * 0.7 + 0.3;
    let flag;
    if tf <= 0.0 {
        r = 0;
        flag = 0;
    } else {
        r = ftol(f64::from(r) * tf * xf);
        flag = 1;
    }
    r = ftol(f64::from(r) * xf); // X enters the money twice

    // 4.4 multipliers
    r = ftol(f64::from(d.effect_all) * f64::from(r));
    r = ftol(f64::from(d.effect_cargo) * f64::from(r));
    if c == 0 {
        if d.train_flags & 1 != 0 {
            r = ftol(f64::from(r) * 1.2);
        }
        if d.train_flags & 4 != 0 {
            r = ftol(f64::from(r) * 1.15);
        }
    }
    let lo = usize::from(d.slider_lo.min(10));
    let hi = usize::from(d.slider_hi.min(10));
    if human {
        r = ftol(f64::from(r) * f64::from(t.slider_human[lo]));
    } else {
        let m = f64::from(d.computer_factor);
        let mult = f64::from(t.slider_human[lo]) * (1.0 - m) + f64::from(t.slider_ai[hi]) * m;
        r = ftol(mult * f64::from(r));
    }
    r = ftol(f64::from(r) * f64::from(t.econ_revenue[d.econ_state.min(4)]));
    r = ftol(f64::from(r) * f64::from(d.company_multiplier));

    r = progressive_reduction(r);
    Revenue {
        amount: r,
        flag,
        industry,
    }
}

/// Progressive reduction above $80 000 per lot (cargo.md 4.5): each step is only evaluated if
/// the previous one fired.
pub fn progressive_reduction(mut r: i32) -> i32 {
    for thr in [80_000, 120_000, 180_000, 250_000] {
        if r > thr {
            r -= (r - thr) >> 2;
        } else {
            break;
        }
    }
    r
}

// ---- industry types (cargo.md 7.1) -------------------------------------------------------------

pub const INDUSTRY_TYPES: usize = 53;
pub const INDUSTRY_RECORD_SIZE: usize = 0x90;
/// Marker for an empty cargo field.
pub const NO_CARGO: u8 = 0x7f;

/// One input of an output slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotInput {
    pub cargo: u8,
    /// Units required per conversion (converters).
    pub need: u8,
    /// Bonus threshold (primary producers).
    pub threshold: f32,
    /// Index into the industry's stock (`+0x6b`), computed by [`IndustryType::rebuild_inputs`].
    pub index: u8,
    pub weight: f32,
}

/// One of the 4 output slots (0x20 bytes) of an industry type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub year_from: i16,
    pub year_to: i16,
    /// 1 = primary (rate), 0 = converter.
    pub mode: u8,
    pub output: u8,
    pub rate: f32,
    pub inputs: [SlotInput; 2],
}

/// A record of `industry_type_table` (`0x541ae0`, 0x90 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct IndustryType {
    pub slots: [Slot; 4],
    /// `+0x80`
    pub inputs: [u8; 8],
    /// `+0x88`
    pub weights: [u8; 8],
}

impl IndustryType {
    pub fn from_raw(rec: &[u8]) -> IndustryType {
        let i16at = |o: usize| i16::from_le_bytes([rec[o], rec[o + 1]]);
        let f32at = |o: usize| f32::from_le_bytes([rec[o], rec[o + 1], rec[o + 2], rec[o + 3]]);
        let slot = |s: usize| {
            let o = s * 0x20;
            let input = |c: usize, n: usize, th: usize, ix: usize, w: usize| SlotInput {
                cargo: rec[o + c],
                need: rec[o + n],
                threshold: f32at(o + th),
                index: rec[o + ix],
                weight: f32at(o + w),
            };
            Slot {
                year_from: i16at(o),
                year_to: i16at(o + 2),
                mode: rec[o + 4],
                output: rec[o + 5],
                rate: f32at(o + 6),
                inputs: [
                    input(0xa, 0xb, 0xc, 0x10, 0x11),
                    input(0x15, 0x16, 0x17, 0x1b, 0x1c),
                ],
            }
        };
        IndustryType {
            slots: [slot(0), slot(1), slot(2), slot(3)],
            inputs: rec[0x80..0x88].try_into().expect("8 bytes"),
            weights: rec[0x88..0x90].try_into().expect("8 bytes"),
        }
    }

    fn slot_valid_in(&self, s: usize, year: i32) -> bool {
        let sl = &self.slots[s];
        i32::from(sl.year_from) <= year && year <= i32::from(sl.year_to)
    }

    /// `industry_rebuild_inputs` (`0x471880`): assigns stock indices to the input cargos of the
    /// slots valid in `year` (no repeated cargo) and rebuilds `inputs`/`weights`. The game runs
    /// it for all types at start-up and every year end.
    pub fn rebuild_inputs(&mut self, year: i32) {
        let mut inputs = [NO_CARGO; 8];
        let mut n = 0usize;
        for s in 0..4 {
            if !self.slot_valid_in(s, year) {
                continue;
            }
            for k in 0..2 {
                let c = self.slots[s].inputs[k].cargo;
                if c == NO_CARGO {
                    continue;
                }
                let j = match inputs[..n].iter().position(|&x| x == c) {
                    Some(j) => j,
                    None => {
                        inputs[n] = c;
                        n += 1;
                        n - 1
                    }
                };
                self.slots[s].inputs[k].index = j as u8;
            }
        }
        let mut weights = [0u8; 8];
        for s in 0..4 {
            if !self.slot_valid_in(s, year) {
                continue;
            }
            for k in 0..2 {
                let inp = self.slots[s].inputs[k];
                if inp.cargo == NO_CARGO {
                    continue;
                }
                let j = usize::from(inp.index);
                weights[j] = ftol(f64::from(weights[j]) + f64::from(inp.weight)) as u8;
            }
        }
        self.inputs = inputs;
        self.weights = weights;
    }

    /// `industry_production_rate` (`0x40a8c0`): lots per year of slot `slot` before event and
    /// economy multipliers. `type_id` is the industry type number (0x20 steel mill, 0x0e power
    /// plant get special converter rates); `stock` is the industry's input stock.
    pub fn production_rate(
        &self,
        slot: usize,
        type_id: u8,
        year: i32,
        basic_model: bool,
        sandbox: bool,
        stock: &[f32; 8],
    ) -> f64 {
        let sl = &self.slots[slot];
        if sl.output == NO_CARGO
            || year < i32::from(sl.year_from)
            || year > i32::from(sl.year_to)
            || sl.rate == 0.0
        {
            return 0.0;
        }
        let s0 = &self.slots[0];
        let s1 = &self.slots[1];
        let mut alt = 0;
        if (year < i32::from(s0.year_from) || year > i32::from(s0.year_to))
            && i32::from(s1.year_from) <= year
            && year <= i32::from(s1.year_to)
        {
            alt = 1;
        }
        let base = if sl.mode == 0 {
            if (!basic_model && !sandbox) || slot != alt {
                return 0.0;
            }
            match type_id {
                0x20 => 3.0,
                0x0e => 1.0,
                _ => 2.0,
            }
        } else {
            f64::from(sl.rate)
        };
        let stock_at = |i: u8| stock.get(usize::from(i)).copied().unwrap_or(0.0);
        let [i1, i2] = sl.inputs;
        let mut bonus = 1.0f64;
        if i1.cargo != NO_CARGO && i1.threshold <= stock_at(i1.index) {
            bonus = f64::from(i1.threshold) + 1.0;
        }
        if i2.cargo != NO_CARGO && i2.threshold <= stock_at(i2.index) {
            bonus += f64::from(i2.threshold);
        }
        bonus * base
    }
}

/// Minutes between two lots of a slot (cargo.md 7.3, `0x40aa20`) [S, no differential test]:
/// `ftol(483840 / prod / m)` with the era speed-ups. `prod` is [`IndustryType::production_rate`],
/// `m` the event/economy multiplier (>= 0.01), `overtime` the "Overtime" cheat.
pub fn production_period(prod: f64, m: f64, year: i32, overtime: bool) -> i32 {
    let mut period = ftol(f64::from(MINUTES_PER_YEAR) / prod / m);
    if year >= 1970 {
        period = ftol(f64::from(period) * 0.73);
    } else if year >= 1950 {
        period = ftol(f64::from(period) * 0.82);
    } else if year >= 1930 {
        period = ftol(f64::from(period) * 0.91);
    }
    if overtime {
        period >>= 1;
    }
    period
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_cargo(price: i32, days: i32) -> Vec<u8> {
        let mut r = vec![0u8; CARGO_RECORD_SIZE];
        r[0..4].copy_from_slice(&price.to_le_bytes());
        r[4..8].copy_from_slice(&days.to_le_bytes());
        r
    }

    #[test]
    fn cargo_runtime_fixups() {
        // cargo.md section 3: goods raw 42000 -> 58799; passenger 500 days -> 720000
        assert_eq!(CargoType::from_raw(&raw_cargo(42000, 800)).price, 58799);
        assert_eq!(CargoType::from_raw(&raw_cargo(56000, 500)).price, 78400);
        assert_eq!(
            CargoType::from_raw(&raw_cargo(56000, 500)).wait_scale,
            720_000
        );
    }

    fn tables() -> RevenueTables {
        RevenueTables {
            demand_level: [1.0; 10],
            undemanded: [0.8, 0.5, 0.2, 0.8, 0.55, 0.4],
            econ_revenue: [0.8, 0.9, 1.0, 1.1, 1.2],
            slider_human: [1.0; 11],
            slider_ai: [1.0; 11],
        }
    }

    fn delivery<'a>(consumers: &'a [Consumer]) -> Delivery<'a> {
        Delivery {
            year: 1900,
            now: 1900 * 483_840 + 10_000,
            econ_state: 2,
            model: 0,
            slider_lo: 5,
            slider_hi: 5,
            cargo_id: 7,
            cargo: CargoType::from_raw(&{
                let mut r = raw_cargo(42000, 800);
                r[0xc..0x10].copy_from_slice(&0.33f32.to_le_bytes());
                r
            }),
            lot_id: 1,
            lot_industry: 0,
            lot_origin_station: 1,
            t_created: 1900 * 483_840,
            t_loaded: 1900 * 483_840 + 1000,
            src_station_id: 1,
            src_xy: (0, 0),
            src_upgrades: 0,
            dst_station_id: 2,
            dst_xy: (100, 0),
            dst_demands_cargo: false,
            dst_price_level: 0,
            consumers,
            train_flags: 0,
            human: true,
            computer_factor: 0.0,
            company_multiplier: 1.0,
            effect_all: 1.0,
            effect_cargo: 1.0,
        }
    }

    #[test]
    fn undemanded_cargo_pays_a_fraction_and_is_deterministic() {
        let d = delivery(&[]);
        let a = delivery_revenue(&d, &tables());
        let b = delivery_revenue(&d, &tables());
        assert_eq!(a, b);
        assert_eq!(a.flag, 1);
        assert_eq!(a.industry, 0);
        assert!(a.amount > 0);
        // paying back to the origin station pays nothing for a human company
        let mut same = delivery(&[]);
        same.dst_station_id = 1;
        assert_eq!(delivery_revenue(&same, &tables()).amount, 0);
    }

    #[test]
    fn demanding_industry_with_least_stock_wins() {
        let mk = |id, stock0| Consumer {
            id,
            kind: 1,
            inputs: [
                7, NO_CARGO, NO_CARGO, NO_CARGO, NO_CARGO, NO_CARGO, NO_CARGO, NO_CARGO,
            ],
            stock: [stock0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        let cons = [mk(1, 3.0), mk(2, 1.0), mk(3, 2.0)];
        let mut d = delivery(&cons);
        d.dst_demands_cargo = true;
        let r = delivery_revenue(&d, &tables());
        assert_eq!(r.industry, 2);
    }

    #[test]
    fn progressive_reduction_steps() {
        assert_eq!(progressive_reduction(80_000), 80_000);
        assert_eq!(progressive_reduction(100_000), 95_000);
        // 130000 -> 117500 and the next step (120000) is not reached
        assert_eq!(progressive_reduction(130_000), 117_500);
        // 300000 -> 245000 -> 213750 -> 205313 (250000 not exceeded afterwards)
        assert_eq!(progressive_reduction(300_000), 205_313);
        assert_eq!(progressive_reduction(-5), -5);
    }

    fn slot(from: i16, to: i16, mode: u8, out: u8, rate: f32, i1: (u8, f32, f32)) -> Slot {
        let none = SlotInput {
            cargo: NO_CARGO,
            need: 0,
            threshold: 0.0,
            index: 0,
            weight: 0.0,
        };
        Slot {
            year_from: from,
            year_to: to,
            mode,
            output: out,
            rate,
            inputs: [
                SlotInput {
                    cargo: i1.0,
                    need: 1,
                    threshold: i1.1,
                    index: 0,
                    weight: i1.2,
                },
                none,
            ],
        }
    }

    fn empty_slot() -> Slot {
        let mut s = slot(0, 0, 0, NO_CARGO, 0.0, (NO_CARGO, 0.0, 0.0));
        s.year_from = 1;
        s.year_to = 0;
        s
    }

    #[test]
    fn farm_fertilizer_bonus_and_rebuild() {
        // like a coffee farm (cargo.md 7.1): slot 0 until 1904, slot 1 from 1905 with fertilizer
        let mut t = IndustryType {
            slots: [
                slot(1800, 1904, 1, 5, 2.0, (NO_CARGO, 0.0, 0.0)),
                slot(1905, 9999, 1, 5, 2.0, (16, 0.5, 1.0)),
                empty_slot(),
                empty_slot(),
            ],
            inputs: [0; 8],
            weights: [0; 8],
        };
        t.rebuild_inputs(1910);
        assert_eq!(t.inputs[0], 16);
        assert_eq!(t.inputs[1], NO_CARGO);
        assert_eq!(t.weights[0], 1);
        let mut stock = [0.0f32; 8];
        // year 1910: slot 0 is out of range, slot 1 is the active one
        assert_eq!(t.production_rate(0, 10, 1910, false, false, &stock), 0.0);
        assert_eq!(t.production_rate(1, 10, 1910, false, false, &stock), 2.0);
        stock[0] = 0.5; // threshold reached: x1.5
        assert_eq!(t.production_rate(1, 10, 1910, false, false, &stock), 3.0);
        // before 1905 only slot 0 produces
        assert_eq!(t.production_rate(0, 10, 1900, false, false, &stock), 2.0);
        assert_eq!(t.production_rate(1, 10, 1900, false, false, &stock), 0.0);
    }

    #[test]
    fn converter_only_produces_free_in_basic_model() {
        let t = IndustryType {
            slots: [
                slot(1800, 9999, 0, 18, 1.0, (3, 0.0, 4.0)),
                empty_slot(),
                empty_slot(),
                empty_slot(),
            ],
            inputs: [0; 8],
            weights: [0; 8],
        };
        let stock = [0.0f32; 8];
        assert_eq!(t.production_rate(0, 0x20, 1900, false, false, &stock), 0.0);
        assert_eq!(t.production_rate(0, 0x20, 1900, true, false, &stock), 3.0);
        assert_eq!(t.production_rate(0, 0x0e, 1900, false, true, &stock), 1.0);
        assert_eq!(t.production_rate(0, 0x01, 1900, true, false, &stock), 2.0);
    }

    #[test]
    fn production_period_examples() {
        // 2 lots per year: half a year, then era speed-ups
        assert_eq!(production_period(2.0, 1.0, 1900, false), 241_920);
        assert_eq!(
            production_period(2.0, 1.0, 1975, false),
            ftol(241_920.0 * 0.73)
        );
        assert_eq!(
            production_period(2.0, 1.0, 1955, false),
            ftol(241_920.0 * 0.82)
        );
        assert_eq!(
            production_period(2.0, 1.0, 1935, false),
            ftol(241_920.0 * 0.91)
        );
        assert_eq!(production_period(2.0, 1.0, 1900, true), 120_960);
    }
}
