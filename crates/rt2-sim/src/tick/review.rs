//! The annual company review (`company_annual_review` `0x43c760`, docs/spec/finance.md section
//! 13) and the manager candidate pick (`0x438450`, section 14.1), on the raw records.
//!
//! The arithmetic is the Rust reference of `crates/rt2-oracle/tests/company.rs`
//! (`review_ref`, `pick_ref`), proven equal to the original by `annual_review_matches_original`
//! (8000 cases) and `manager_pick_matches_original` (6000 cases); here it reads and writes the
//! save records instead of a test struct.

use super::company::{MANAGER_TABLE, PE_ACTIVE, PE_CASH, PE_HOLDINGS, PE_HUMAN};
use super::*;
use crate::company_record::{CO_ACTIVE, CO_ARCH_ACCT, CO_CHAIRMAN};
use crate::finance::Company;
use crate::fp::ftol as ftol_;
use crate::rng::GameRng;

// State/trailer offsets (VA - 0x5704e8).
const ST_MANAGER_AVAIL: usize = 0x150; // 0x570638 u8[40]
const ST_CHAIRMAN_FIRE: usize = 0x1D2; // 0x5706ba
const ST_FIN_ADVANCED: usize = 0x388D; // 0x573d75
const ST_EDITOR: usize = 0x3892; // 0x573d7a
const ST_REF_SPEED: usize = 0x3920; // 0x573e08
const ST_MANAGER_USE: usize = 0x3921; // 0x573e09 u8[40]
const TRAILER_OPTIONS: usize = 0x32; // 0x574872 (trailer starts at 0x574840)

const CO_REP: usize = 0x46a;
const CO_LAST_BONUS: usize = 0x46e;
const CO_CHAIR_SINCE: usize = 0x470;
const CO_WARN_YEAR: usize = 0x472;
const CO_BONUS: usize = 0x474;
const CO_STANDING: usize = 0x47a;
const CO_STANDING_ALL: usize = 0x4229;
const CO_REV_MULT: usize = 0x4248;
const CO_CAND0: usize = 0x140;
const CO_CAND1: usize = 0x144;
const CO_SPEED_N: usize = 0x196;
const CO_SPEED_SUM: usize = 0x19a;
const TERR_STRIDE: usize = 0x1ed;
const N_TERR: usize = 32;
const PE_REP: usize = 0x52b;

const ST_STATION_GENERATED: usize = 0x4b5;
const ST_STATION_SHIPPED: usize = 0x4b7;
const ST_STATION_RECEIVED: usize = 0x4b9;
const ST_STATION_TERRITORY: usize = 0x4ad;
const ST_STATION_COMPANY: usize = 0x39;

fn sum_cats(c: &[f64; 13], n: usize) -> f64 {
    c.iter().take(n).fold(0.0, |a, b| a + b)
}

fn cdiv(a: i32, b: i32) -> i32 {
    a.wrapping_div(b)
}

/// Inputs of `pick_ref` (`0x438450`).
struct PickIn<'a> {
    year: i32,
    month: i32,
    founded: i32,
    cats: &'a [f64; 13],
    arch: &'a [[f64; 13]; 10],
    c140: i32,
    c144: i32,
    uses: [u8; 40],
    avail: [u8; 40],
    sal: [i32; 40],
}

/// `manager_pick_candidate(-1)` (`0x438450`): the index of a manager, drawing the game RNG.
/// The network-lock branch (`0x5748f8 != 0`) never happens in a local game.
fn pick_manager(p: &PickIn, rng: &mut GameRng) -> i32 {
    let y = p.year - i32::from(p.month != 12);
    let mut acc = 0.0f64;
    let mut cnt = 0i32;
    for i in 0..3 {
        if p.founded <= y {
            let mut rev = 0.0f64;
            for k in 0..5 {
                rev += if y == p.year {
                    p.cats[k]
                } else {
                    p.arch[(p.year - y - 1) as usize][k]
                };
            }
            cnt += 3 - i;
            acc += f64::from(3 - i) * rev;
        }
    }
    let avg = if cnt != 0 { acc / f64::from(cnt) } else { acc };
    let r = rng.range(25, 180);
    let mut v = ((avg * 1.4) * 0.01) * f64::from(r) * 0.01;
    if v < 8000.0 {
        v = f64::from(rng.range(1000, 12000));
    }
    let mut c = rng.range(0, 39);
    let mut attempts = 4000i32;
    let (mut f1, mut f2) = (true, true);
    let (mut lo, mut hi) = (0.8f32, f32::from_bits(0x3fa6_6666));
    loop {
        let cu = c as usize;
        let mut reject = p.uses[cu] != 0 || c == p.c140 || c == p.c144;
        if !reject && f2 {
            let ratio = f64::from(p.sal[cu]) / v;
            if ratio <= f64::from(lo) || ratio >= f64::from(hi) {
                reject = true;
            }
        }
        if !reject && f1 && p.avail[cu] == 0 {
            reject = true;
        }
        if !reject {
            return c;
        }
        if attempts & 0xf == 0xf {
            lo = (f64::from(lo) * 0.96) as f32;
            hi = (f64::from(hi) * 1.04) as f32;
            if attempts <= 1000 {
                f2 = false;
            }
            if attempts <= 500 {
                f1 = false;
            }
        }
        if attempts > 60 {
            c = rng.range(0, 39);
        } else {
            c = (c + 1) % 40;
        }
        attempts -= 1;
    }
}

impl SimState {
    fn manager_salaries(&self) -> [i32; 40] {
        let mut s = [0i32; 40];
        for (i, v) in s.iter_mut().enumerate() {
            *v = self.data.image.i32(MANAGER_TABLE + 0x11 * i as u32);
        }
        s
    }

    /// `company_annual_review` (`0x43c760`, finance.md 13) for the company at `slot`.
    pub(super) fn company_annual_review(&mut self, w: &mut World, slot: usize) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "company_annual_review",
                "company_annual_review 0x43c760 on a 0x233C file",
                "finance.md 13"
            );
            return;
        }
        let Some(rec) = pool::record(&self.save.companies, slot) else {
            return;
        };
        if i32_at(&rec.data, CO_ACTIVE) == 0 {
            return;
        }
        let d = rec.data.clone();
        let date = date_of(w.minutes);
        let year = i32::from(date.year);
        let month = i32::from(date.month);
        let mut co = Company::from_record(&d);
        let age = year - co.founded;
        let chairman = i32_at(&d, CO_CHAIRMAN);
        let (price, eps) = {
            let ctx = self.fin_ctx(w.minutes, slot);
            let p = f64::from(co.trade_price(&ctx, 0, false));
            let e = co.live_metric(&ctx, 3);
            (p, e)
        };
        // Live metrics refresh the price cache of the record (`0x43aea0`).
        let (chair_rep, chair_cash, chair_human) = if chairman > 0 {
            pool::record(&self.save.persons, chairman as usize).map_or((0, 0.0, false), |p| {
                (
                    i32_at(&p.data, PE_REP),
                    f64_at(&p.data, PE_CASH),
                    i32_at(&p.data, PE_HUMAN) != 0,
                )
            })
        } else {
            (0, 0.0, false)
        };
        let rep0 = i32_at(&d, CO_REP);
        let last_bonus = i32::from(i16_at(&d, CO_LAST_BONUS));
        let chair_since = i32::from(i16_at(&d, CO_CHAIR_SINCE));
        let warn_year = i32::from(i16_at(&d, CO_WARN_YEAR));
        let salary0 = i32_at(&d, 0x134);
        let options = self.save.trailer.get(TRAILER_OPTIONS).copied().unwrap_or(0);
        let ref_speed = self.save.state[ST_REF_SPEED];
        let fin_adv = self.save.state[ST_FIN_ADVANCED] != 0;
        let gate = self.save.state[ST_CHAIRMAN_FIRE] != 0;
        let editor = self.save.state[ST_EDITOR] != 0;
        let mut uses = [0u8; 40];
        uses.copy_from_slice(&self.save.state[ST_MANAGER_USE..ST_MANAGER_USE + 40]);
        let mut avail = [0u8; 40];
        avail.copy_from_slice(&self.save.state[ST_MANAGER_AVAIL..ST_MANAGER_AVAIL + 40]);
        let sal_table = self.manager_salaries();
        let set = self.company_set();
        let idx = set.iter().position(|&c| c == slot);
        // Holders: every person in list order.
        let persons = pool::list_order(&self.save.persons);
        let holders: Vec<(usize, bool, bool, i32)> = persons
            .iter()
            .filter_map(|&p| pool::record(&self.save.persons, p).map(|r| (p, r)))
            .map(|(p, r)| {
                (
                    p,
                    i32_at(&r.data, PE_ACTIVE) != 0,
                    i32_at(&r.data, PE_HUMAN) != 0,
                    idx.map_or(0, |i| i32_at(&r.data, PE_HOLDINGS + 4 * i)),
                )
            })
            .collect();
        let held: i32 = holders.iter().filter(|h| h.1 && h.3 > 0).map(|h| h.3).sum();

        // -- performance of the chairman: price growth and profit-per-share growth (13.1)
        let prev = if co.founded == year {
            f64::from(co.init_price)
        } else {
            co.arch_metrics[1][0]
        };
        let prev = if prev <= 0.01 { 0.01 } else { prev };
        let mut g1 = (price / prev - 1.0) as f32;
        if f64::from(g1) > 1.0 {
            g1 = f64::from(g1).sqrt() as f32;
        }
        if f64::from(g1) < -1.0 {
            g1 = (-(-f64::from(g1)).sqrt()) as f32;
        }
        if f64::from(g1) > 2.0 {
            g1 = 2.0;
        }
        if f64::from(g1) < -2.0 {
            g1 = -2.0;
        }
        let mut a = eps;
        let mut b = if co.founded == year {
            1.0
        } else {
            co.arch_metrics[1][3]
        };
        if a == 0.0 {
            a = 0.01;
        }
        if b == 0.0 {
            b = 0.01;
        }
        let squash = |mut x: f64| {
            if x > 1.0 {
                x = x.sqrt();
            }
            if x < -1.0 {
                x = -(-x).sqrt();
            }
            x.clamp(-2.0, 2.0)
        };
        let g2 = if a > 0.0 && b > 0.0 {
            squash(a / (b + 1.0) - 1.0)
        } else if a < 0.0 && b < 0.0 {
            squash(b / (a - 1.0) - 1.0)
        } else if a > 0.0 && b < 0.0 {
            squash(a / (-b + 1.0) - 1.0)
        } else if a < 0.0 && b > 0.0 {
            -1.0
        } else {
            0.0
        };
        let h2 = g2 * 0.5;
        let g1d = f64::from(g1);
        let mix = if h2 <= g1d {
            g1d * 6.0 + h2 * 4.0
        } else {
            g1d * 4.0 + h2 * 6.0
        };
        let v = mix * 0.05 * 100.0;
        let v = if v < 0.0 {
            -((-v).sqrt() * 7.07)
        } else if v > 0.0 {
            v.sqrt() * 7.07
        } else {
            v
        };
        let perf = v * 1.8;
        let wgt = if age < 3 {
            f64::from(age) * 0.15 * 0.45 + 0.225
        } else {
            0.45
        };
        let mut perf32 = perf as f32;
        if (0.5..10.0).contains(&price) && perf32 > 0.0 {
            perf32 = (f64::from(perf32) * price * 0.1) as f32;
        }
        let rep = ftol_((1.0 - wgt) * f64::from(rep0) + f64::from(perf32) * wgt);
        let new_chair_rep = if chairman != 0 {
            ftol_(f64::from(chair_rep) * 0.55 + f64::from(perf32) * 0.45)
        } else {
            chair_rep
        };

        // -- chairman salary (13.2)
        let rev = sum_cats(&co.cats, 5);
        let target = ftol_((rev.sqrt() + 200.0) * f64::from((rep + 40) * 3) * 0.1);
        let mut sal = salary0;
        match options & 0x38 {
            0 => sal = ftol_(f64::from(sal) * 1.5),
            8 => sal = ftol_(f64::from(sal) * 1.25),
            _ => {}
        }
        let sal2 = ftol_(f64::from(sal) * 0.65 + f64::from(target) * 0.35);
        let salary = ((sal2 + 500) / 1000 * 1000).max(1000);

        // -- bonus (13.3)
        let mut bonus = 0;
        let mut new_last_bonus = last_bonus as i16;
        let mut cash_after = chair_cash;
        let mut bonus_paid = 0.0f64;
        if year - last_bonus > 1 && chairman != 0 {
            let roll = w.rng.range(20, 110);
            if roll < rep {
                let mut bb = cdiv(cdiv(rep.wrapping_mul(salary), 100), 1000) + 1000;
                if bb < 5000 {
                    bb = 5000;
                }
                new_last_bonus = year as i16;
                cash_after += f64::from(bb);
                bonus_paid = f64::from(bb);
                bonus = bb;
            }
        }

        // -- standing with the territories (13.4)
        let mut base = 5000;
        if ref_speed != 0 {
            let speed = cdiv(i32_at(&d, CO_SPEED_SUM), i32_at(&d, CO_SPEED_N).max(1));
            let mut ratio = f64::from(speed) / f64::from(ref_speed);
            if ratio > f64::from(2.0f32) {
                ratio = f64::from(2.0f32);
            }
            base = ftol_((ratio - 1.0) * 2000.0 + 5000.0);
        }
        let mut tot_a = [0.0f32; N_TERR];
        let mut tot_b = [0.0f32; N_TERR];
        for s in pool::list_order(&self.save.stations) {
            let Some(r) = pool::record(&self.save.stations, s) else {
                continue;
            };
            if i32_at(&r.data, ST_STATION_COMPANY) == slot as i32 {
                let t = i32_at(&r.data, ST_STATION_TERRITORY) as usize;
                if t < N_TERR {
                    tot_a[t] += (u32::from(u16_at(&r.data, ST_STATION_RECEIVED))
                        + u32::from(u16_at(&r.data, ST_STATION_SHIPPED)))
                        as f32;
                    tot_b[t] += f32::from(u16_at(&r.data, ST_STATION_GENERATED));
                }
            }
        }
        let mut standing = [0i16; N_TERR];
        for (t, s) in standing.iter_mut().enumerate() {
            *s = i16_at(&d, CO_STANDING + TERR_STRIDE * t);
        }
        let (mut ta, mut tb) = (0i32, 0i32);
        for t in 0..N_TERR {
            ta = ftol_(f64::from(ta) + f64::from(tot_a[t]));
            tb = ftol_(f64::from(tb) + f64::from(tot_b[t]));
            if tot_b[t] > 3.0 {
                let score = ftol_(
                    (f64::from(tot_a[t]) / f64::from(tot_b[t]) - 1.0) * 4000.0 + f64::from(base),
                );
                standing[t] =
                    ftol_(f64::from(score) * 0.3 + f64::from(i32::from(standing[t])) * 0.7) as i16;
            }
        }
        let dd = tb.max(1);
        let rnd0 = w.rng.range(-200, 200);
        let score = ftol_((f64::from(ta) / f64::from(dd) - 1.0) * 4000.0 + f64::from(base));
        let vv = rnd0.wrapping_add(score);
        let mut standing_all =
            ftol_(f64::from(i32_at(&d, CO_STANDING_ALL)) * 0.7 + f64::from(vv) * 0.3);
        for s in &mut standing {
            let x = w.rng.range(-200, 200);
            *s = (x as i16).wrapping_add(*s).clamp(-10000, 10000);
        }
        standing_all = standing_all.clamp(0, 10000);

        // -- revenue multiplier (13.5)
        let mut cats_after = co.cats;
        cats_after[11] -= bonus_paid;
        let mut rv = rev;
        let mut pf = sum_cats(&cats_after, 13);
        if age > 0 {
            rv += sum_cats(&co.arch_cats[0], 5) * 0.7;
            pf += sum_cats(&co.arch_cats[0], 13) * 0.7;
        }
        if age > 1 {
            rv += sum_cats(&co.arch_cats[1], 5) * 0.4;
            pf += sum_cats(&co.arch_cats[1], 13) * 0.4;
        }
        if rv < 1.0 {
            rv = 1.0;
        }
        let m = 1.0 - (pf / rv - 0.2) * 0.333_333_333_333_333_3;
        let mut m32 = m as f32;
        if m < 0.8 {
            m32 = f32::from_bits(0x3f4c_cccd);
        } else if m > 1.2 {
            m32 = f32::from_bits(0x3f99_999a);
        }
        if age < 5 {
            m32 = ((f64::from(age) * f64::from(m32) + f64::from(5 - age)) * 0.2) as f32;
        }
        if chairman > 0 && chair_human {
            m32 = ((f64::from(m32) + 1.0) * 0.5) as f32;
        }

        // -- manager candidates, odd years (13.6)
        let mut cand = [i32_at(&d, CO_CAND0), i32_at(&d, CO_CAND1)];
        if year & 1 != 0 {
            let mut u = uses;
            for &c in &cand {
                if c >= 0 {
                    u[c as usize] = u[c as usize].wrapping_add(1);
                }
            }
            let mut inp = PickIn {
                year,
                month,
                founded: co.founded,
                cats: &co.cats,
                arch: &co.arch_cats,
                c140: cand[0],
                c144: cand[1],
                uses: u,
                avail,
                sal: sal_table,
            };
            cand[0] = pick_manager(&inp, &mut w.rng);
            inp.c140 = cand[0];
            cand[1] = pick_manager(&inp, &mut w.rng);
        }

        // -- dismissal of the chairman and liquidation (13.7, 13.8)
        let mut fire = false;
        let mut liquidate = false;
        let mut new_warn = warn_year as i16;
        if !editor && gate {
            if fin_adv && chairman != 0 && co.shares - held > (co.shares >> 1) {
                let mut n = 0i32;
                let mut t = -15i32;
                loop {
                    let y = year - n;
                    if y < co.founded || y < chair_since {
                        break;
                    }
                    let rp = if n == 0 {
                        f64::from(rep)
                    } else {
                        co.arch_metrics[n as usize][12]
                    };
                    if rp >= f64::from(t) {
                        break;
                    }
                    let e = if n == 0 {
                        eps
                    } else {
                        co.arch_metrics[n as usize][3]
                    };
                    if e >= 1.0 {
                        break;
                    }
                    n += 1;
                    t += 5;
                    if t >= 0 {
                        break;
                    }
                }
                if n > 1 {
                    let mut pct = (f64::from(-rep) * 1.75) as f32;
                    if f64::from(-rep) * 1.75 > 100.0 {
                        pct = 100.0;
                    }
                    let mut votes = ftol_(f64::from(pct) * 0.01 * f64::from(co.shares - held));
                    if pct > 50.0 {
                        for h in &holders {
                            if h.3 > 0 && !h.2 && h.0 as i32 != chairman {
                                votes += h.3;
                            }
                        }
                    }
                    if f64::from(votes) > f64::from(co.shares) * 0.5 {
                        if warn_year < year - 1 {
                            new_warn = year as i16;
                        } else {
                            fire = true;
                        }
                    }
                }
            }
            let mut n = 0i32;
            let mut thr = 5i32;
            loop {
                let y = year - n;
                if y < co.founded {
                    break;
                }
                let p = if n == 0 {
                    price
                } else {
                    co.arch_metrics[n as usize][0]
                };
                if p >= f64::from(thr) {
                    break;
                }
                let equity = if n == 0 {
                    -co.acct[7]
                } else {
                    let o = CO_ARCH_ACCT + 0x40 * (n as usize - 1);
                    let mut s = 0.0;
                    for k in 0..6 {
                        s += f64_at(&d, o + 8 * k);
                    }
                    s + f64_at(&d, o + 48)
                };
                if equity >= 0.0 {
                    break;
                }
                let income = if n == 0 {
                    sum_cats(&cats_after, 13)
                } else {
                    sum_cats(&co.arch_cats[n as usize - 1], 13)
                };
                if income >= -10000.0 {
                    break;
                }
                let cash = if n == 0 {
                    co.acct[0]
                } else {
                    f64_at(&d, CO_ARCH_ACCT + 0x40 * (n as usize - 1))
                };
                if cash >= 0.0 {
                    break;
                }
                n += 1;
                thr += 2;
                if thr >= 11 {
                    break;
                }
            }
            if n == 3 {
                liquidate = true;
            }
        }
        if fire {
            todo_hook!(
                self.log,
                "review_fire_chairman",
                "company_set_chairman(0) 0x438900: the board dismisses the chairman",
                "finance.md 13.7, 15.4"
            );
        }
        if liquidate {
            todo_hook!(
                self.log,
                "review_liquidation",
                "company_liquidate 0x438170: the creditors liquidate the company",
                "finance.md 13.8, 15.5"
            );
        }

        // -- write back
        if bonus != 0 {
            co.credit(11, -bonus_paid);
        }
        co.write_record(
            &mut pool::record_mut(&mut self.save.companies, slot)
                .unwrap()
                .data,
        );
        let r = &mut pool::record_mut(&mut self.save.companies, slot)
            .unwrap()
            .data;
        put_u32(r, CO_REP, rep as u32);
        put_u32(r, 0x134, salary as u32);
        put_u32(r, CO_BONUS, bonus as u32);
        pool::put_i16(r, CO_LAST_BONUS, new_last_bonus);
        pool::put_i16(r, CO_WARN_YEAR, new_warn);
        for (t, s) in standing.iter().enumerate() {
            pool::put_i16(r, CO_STANDING + TERR_STRIDE * t, *s);
        }
        put_u32(r, CO_STANDING_ALL, standing_all as u32);
        put_f32(r, CO_REV_MULT, m32);
        put_u32(r, CO_CAND0, cand[0] as u32);
        put_u32(r, CO_CAND1, cand[1] as u32);
        if chairman > 0
            && let Some(p) = pool::record_mut(&mut self.save.persons, chairman as usize)
        {
            put_u32(&mut p.data, PE_REP, new_chair_rep as u32);
            put_f64(&mut p.data, PE_CASH, cash_after);
        }
        self.log.ran("company_annual_review");
    }
}
