//! Finances of a computer company (`company_ai_finance` `0x45fbf0`, ai.md section 10,
//! finance.md 9.1) on the finance rules of [`crate::finance`].

use super::{Ai, CO_SATISFIED, Trait};
use crate::finance::{ACCT_CASH, Company, Ctx};
use crate::fp::ftol;
use crate::trainstep::P;

/// `0x43bd90`: index of the bond with the highest rate (the first of equal rates), -1 if none.
fn highest_rate_bond(c: &Company) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, b) in c.bonds.iter().enumerate() {
        if best.is_none_or(|k| c.bonds[k].rate < b.rate) {
            best = Some(i);
        }
    }
    best
}

/// `company_year_value(c, year, 13)` (`0x43a7d0`): profit of the current year or of an archived
/// one, summed from 0.0 in category order.
pub fn profit_of_year(c: &Company, cur_year: i32, year: i32) -> f64 {
    let row: &[f64; 13] = if year == cur_year {
        &c.cats
    } else if year == 9001 {
        &c.year_cats
    } else {
        let k = cur_year - year - 1;
        match usize::try_from(k).ok().and_then(|k| c.arch_cats.get(k)) {
            Some(r) => r,
            None => return 0.0,
        }
    };
    row.iter().fold(0.0, |a, v| a + v)
}

impl Ai<'_, '_> {
    /// Index of a company in the player/company set (`0x43a9e0`, state `0x570729`).
    pub fn company_index(&self, company: i32) -> Option<usize> {
        let n = (self.st_i32(0x0057_0729).max(0) as usize).min(36);
        (0..n).find(|&i| i32::from(self.st_i16(0x0057_072d + 2 * i as u32)) == company)
    }

    /// `company_free_float` (`0x43e580`): shares not held long by active persons.
    pub fn free_float(&self, company: i32, shares: i32) -> i32 {
        let idx = self.company_index(company);
        let mut held = 0i32;
        for p in self.ids(P::Person) {
            if self.ts.i32(P::Person, p, 4) == 0 {
                continue;
            }
            // Index -1 reads the field before the holdings (`+0x1cf`).
            let o = match idx {
                Some(i) => 0x1d3 + 4 * i,
                None => 0x1cf,
            };
            let v = self.ts.i32(P::Person, p, o);
            if v > 0 {
                held += v;
            }
        }
        shares - held
    }

    /// `effect_multiplier(e, company, 0, 0)`.
    pub fn effect_mult(&self, e: usize, company: i32) -> f32 {
        self.ts.effect(e, company, 0, 0)
    }

    /// The calendar/economy inputs of the finance rules for `company`.
    pub fn fin_ctx(&self, company: i32) -> Ctx<'_> {
        let d = crate::calendar::minutes_to_date(self.now() as i32);
        Ctx {
            tables: &self.tables.finance,
            year: i32::from(d.year),
            month: i32::from(d.month),
            day: i32::from(d.day),
            minutes: self.now() as i32,
            econ_state: self.st_i32(0x0057_0717).clamp(0, 4) as usize,
            prime_rate: self.st_f32(0x0057_06d1),
            liquidity: self.st_f32(0x0057_06cd),
            price_effect: f64::from(self.effect_mult(0x1e, company)),
            prime_effect_pct: self.effect_sum(0x20, company) as i16,
            rating_effect: self.effect_sum(0x1f, company) as i16,
        }
    }

    /// `company_ai_finance` (`0x45fbf0`).
    pub fn company_ai_finance(&mut self, company: i32) {
        self.ts.log.ran("company_ai_finance");
        let mut rec = self.ts.record(P::Company, company);
        let mut c = Company::from_record(&rec);
        let satisfied = crate::pool::i32_at(&rec, CO_SATISFIED) != 0;
        let chairman = crate::pool::i32_at(&rec, 0x23);
        let year = self.year();
        let (b_ac, b_ae, b_af, b_b0, b_b1) = (
            self.perm(0x0057_06ac),
            self.perm(0x0057_06ae),
            self.perm(0x0057_06af),
            self.perm(0x0057_06b0),
            self.perm(0x0057_06b1),
        );
        let policy = self.dividend_policy();
        let ctx_vals = self.fin_ctx(company);
        let tables = self.tables.finance.clone();
        let ctx = Ctx {
            tables: &tables,
            ..ctx_vals
        };
        // 1. Simulated refinancing on a copy.
        let saved = c.clone();
        let n_bonds = saved.bonds.len() as i32;
        let mut best_gain = 0f32;
        let mut best_k = 0i32;
        let mut k = 1i32;
        while k <= n_bonds {
            let mut repaid = 0f32;
            let mut issued = 0f32;
            for _ in 0..k {
                let i = highest_rate_bond(&c).expect("bond");
                let b = c.bonds[i];
                repaid = (f64::from(b.principal) * f64::from(b.rate) + f64::from(repaid)) as f32;
                c.repay_bond(i, true);
            }
            let mut ok = true;
            for _ in 0..k {
                if c.issue_bond(&ctx, 500_000, 30, true, false).is_err() {
                    ok = false;
                    break;
                }
                let b = *c.bonds.last().expect("bond");
                issued = (f64::from(b.principal) * f64::from(b.rate) + f64::from(issued)) as f32;
            }
            if ok {
                let g = (f64::from(repaid) - f64::from(issued)) - f64::from(k) * 8000.0;
                if g > f64::from(best_gain) {
                    best_gain = g as f32;
                    best_k = k;
                }
            }
            c = saved.clone();
            k += 1;
        }
        if !(b_b0 && b_af) {
            best_k = 0;
        }
        let mut repays = 0i32;
        let mut issues = 0i32;
        for _ in 0..best_k.max(0) {
            if let Some(i) = highest_rate_bond(&c) {
                c.repay_bond(i, true);
            }
            repays = best_k;
        }
        // 2. Issue while the cash is below -250 000.
        if b_af {
            while c.acct[ACCT_CASH] < -250_000.0
                && c.issue_bond(&ctx, 500_000, 30, true, false).is_ok()
            {
                issues += 1;
            }
        }
        // 3. Repay the dearest bonds (rate >= 6%) while the cash is above 600 000.
        if b_b0 && satisfied {
            while c.acct[ACCT_CASH] > 600_000.0 && !c.bonds.is_empty() {
                let i = highest_rate_bond(&c).expect("bond");
                if f64::from(c.bonds[i].rate) < 0.06 {
                    break;
                }
                c.repay_bond(i, true);
                repays += 1;
            }
        }
        // 4. Share buy-back.
        let mut bought = 0i32;
        if satisfied {
            loop {
                let mut w = 1.0f32;
                if chairman != 0 {
                    let x = self.persona(chairman, Trait::InvestingInOthers);
                    w = (f64::from(x + 300) * 0.0025) as f32;
                }
                if policy == 1 {
                    w = (f64::from(w) * 1.6) as f32;
                } else if policy == 2 {
                    break;
                }
                if !b_ae {
                    break;
                }
                let cash = c.acct[ACCT_CASH];
                if f64::from(w) * f64::from(800_000.0f32) >= cash {
                    break;
                }
                let price = f64::from(c.trade_price(&ctx, 0, false));
                if price * f64::from(w) * f64::from(1000.0f32) * 1.2 >= cash {
                    break;
                }
                if self.free_float(company, c.shares) < 1000 {
                    break;
                }
                c.repurchase_shares(&ctx, 1000);
                bought += 1000;
            }
        }
        // 5. Bankruptcy.
        if issues == 0
            && repays == 0
            && bought == 0
            && b_ac
            && c.acct[ACCT_CASH] < -300_000.0
            && year - c.founded >= 3
        {
            let mut any = false;
            for i in 0..5 {
                if profit_of_year(&c, year, year - i - 1) > -20_000.0 && i < 3 {
                    any = true;
                }
            }
            if !any && year - c.bankrupt_year >= 5 {
                c.declare_bankruptcy(&ctx);
                // LNG 182; the positions and stations of finance.md 9 are not ported.
                self.ts.log.ran("ai_bankruptcy_news");
                self.ts
                    .g
                    .news
                    .push(crate::message::GameMessage::CompanyBankrupt {
                        company: self.company,
                    });
            }
        }
        // 6. Dividend review in December.
        let last_review = crate::pool::i32_at(&rec, 0x16a);
        if b_b1 && self.month() == 12 && year - last_review >= 1 && year - c.founded >= 2 {
            crate::pool::put_u32(&mut rec, 0x16a, year as u32);
            let mut acc = 0f32;
            for i in 0..3 {
                let w = 3 - i;
                acc = (profit_of_year(&c, year, year - i) * f64::from(w) + f64::from(acc)) as f32;
            }
            let p = (f64::from(acc) * (1.0 / 6.0)) as f32;
            let ff = self.free_float(company, c.shares);
            let d = if ff <= 1000 {
                let cash = c.acct[ACCT_CASH];
                let cash0 = if cash < 0.0 { 0.0 } else { cash };
                (f64::from(p) * (1.0 / 3.0) + cash0 * (1.0 / 3.0)) / f64::from(c.shares)
            } else {
                f64::from(p) * 0.1 / f64::from(c.shares)
            };
            if d < 0.1 {
                c.dividend = 0.0;
            } else {
                let v = ftol((f64::from(c.dividend) + d + 0.1) * 0.5 * f64::from(10.0f32));
                c.dividend = (f64::from(v) * 0.1) as f32;
            }
            let cur = c.dividend;
            let max = c.max_dividend(&ctx);
            if max <= f64::from(cur) {
                c.dividend = max as f32;
            }
        }
        // 7. Dividend policy, every turn.
        if policy == 1 {
            c.dividend = (f64::from(c.dividend) * 0.66) as f32;
        } else if policy == 2 {
            c.dividend = 0.0;
        }
        c.write_record(&mut rec);
        self.ts.set_record(P::Company, company, &rec);
    }
}
