//! Company hooks of the world loop on the raw records: the monthly step (`0x4399d0`,
//! finance.md 5 and 6), shareholders' payouts and the helpers shared by the yearly steps.

use super::*;
use crate::company_record::{CO_ACTIVE, CO_CHAIRMAN, CO_EFFECTS, CO_MANAGER};
use crate::effects::{EffectLevels, effect_multiplier};
use crate::finance::{Company, Ctx, MonthlyInputs, holder_dividend};

/// `0x54fdad`: the manager table, 40 records of 0x11 bytes, `+0` salary.
pub(super) const MANAGER_TABLE: u32 = 0x0054_fdad;

// State block offsets (VA - 0x5704e8).
pub(super) const ST_LIQUIDITY: usize = 0x1E5; // 0x5706cd f32
pub(super) const ST_SET_COUNT: usize = 0x241; // 0x570729 i32
pub(super) const ST_SET_IDS: usize = 0x245; // 0x57072d u16[36]

pub(super) const PE_ACTIVE: usize = 0x04;
pub(super) const PE_HUMAN: usize = 0x122;
pub(super) const PE_CASH: usize = 0x12a;
pub(super) const PE_HOLDINGS: usize = 0x1d3;
pub(super) const PE_EFFECTS: usize = 0x26b;

impl SimState {
    /// `company_set_at(i)` for all `i` (`0x570729`, `0x57072d`): the ids of the companies of the
    /// player/company set.
    pub(super) fn company_set(&self) -> Vec<usize> {
        let n = (i32_at(&self.save.state, ST_SET_COUNT).max(0) as usize).min(36);
        (0..n)
            .map(|i| usize::from(u16_at(&self.save.state, ST_SET_IDS + 2 * i)))
            .collect()
    }

    /// The player set (`0x570775`, `0x570779`): persons of the indexed set.
    pub(super) fn player_set(&self) -> Vec<usize> {
        let n = (i32_at(&self.save.state, 0x28D).max(0) as usize).min(36);
        (0..n)
            .map(|i| usize::from(u16_at(&self.save.state, 0x291 + 2 * i)))
            .collect()
    }

    /// `effect_multiplier(e, company, 0, 0)` (cargo.md 5): the person level is the company's
    /// chairman.
    pub(super) fn effect_mult(&self, e: usize, company: usize) -> f32 {
        let st = &self.save.state;
        let o = ST_GLOBAL_EFFECTS + 2 * e;
        let global = i16_at(st, o);
        let (mut comp, mut person) = (None, None);
        if let Some(r) = pool::record(&self.save.companies, company) {
            comp = Some(i16_at(&r.data, CO_EFFECTS + 2 * e));
            let p = i32_at(&r.data, CO_CHAIRMAN);
            if p > 0
                && let Some(pr) = pool::record(&self.save.persons, p as usize)
            {
                person = Some(i16_at(&pr.data, PE_EFFECTS + 2 * e));
            }
        }
        effect_multiplier(EffectLevels {
            global,
            company: comp,
            person,
            territory: None,
        })
    }

    /// Additive effect percentage (`0x46b5d0`): global + company + chairman.
    pub(super) fn effect_sum(&self, e: usize, company: usize) -> i32 {
        let st = &self.save.state;
        let mut v = i32::from(i16_at(st, ST_GLOBAL_EFFECTS + 2 * e));
        if let Some(r) = pool::record(&self.save.companies, company) {
            v += i32::from(i16_at(&r.data, CO_EFFECTS + 2 * e));
            let p = i32_at(&r.data, CO_CHAIRMAN);
            if p > 0
                && let Some(pr) = pool::record(&self.save.persons, p as usize)
            {
                v += i32::from(i16_at(&pr.data, PE_EFFECTS + 2 * e));
            }
        }
        v
    }

    /// The date and economy values the finance code reads from globals.
    pub(super) fn fin_ctx(&self, minutes: i32, company: usize) -> Ctx<'_> {
        let d = date_of(minutes);
        let st = &self.save.state;
        Ctx {
            tables: &self.data.finance,
            year: i32::from(d.year),
            month: i32::from(d.month),
            day: i32::from(d.day),
            minutes,
            econ_state: i32_at(st, ST_ECON_STATE).clamp(0, 4) as usize,
            prime_rate: f32_at(st, ST_PRIME_RATE),
            liquidity: f32_at(st, ST_LIQUIDITY),
            price_effect: f64::from(self.effect_mult(0x1e, company)),
            prime_effect_pct: self.effect_sum(0x20, company) as i16,
            rating_effect: self.effect_sum(0x1f, company) as i16,
        }
    }

    /// Salary of the manager hired by the company (`+0x13c`, table `0x54fdad`), 0 when none.
    pub(super) fn manager_salary(&self, rec: &[u8]) -> i32 {
        let m = i32_at(rec, CO_MANAGER);
        if (0..40).contains(&m) {
            self.data.image.i32(MANAGER_TABLE + 0x11 * m as u32)
        } else {
            0
        }
    }

    pub(super) fn person_is_human(&self, person: i32) -> bool {
        person > 0
            && pool::record(&self.save.persons, person as usize)
                .is_some_and(|p| i32_at(&p.data, PE_HUMAN) != 0)
    }

    /// Adds `x` to the personal cash of `person`.
    pub(super) fn person_add_cash(&mut self, person: usize, x: f64) {
        if let Some(p) = pool::record_mut(&mut self.save.persons, person) {
            let c = f64_at(&p.data, PE_CASH);
            put_f64(&mut p.data, PE_CASH, c + x);
        }
    }

    /// `company_month_step` (`0x4399d0`, finance.md 5) on the raw record of `slot`.
    pub(super) fn company_month_step(&mut self, w: &World, slot: usize) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "company_month_step",
                "company_month_step 0x4399d0 on a 0x233C file",
                "finance.md 5"
            );
            return;
        }
        let Some(rec) = pool::record(&self.save.companies, slot) else {
            return;
        };
        if i32_at(&rec.data, CO_ACTIVE) == 0 {
            return;
        }
        let mut co = Company::from_record(&rec.data);
        let chairman = i32_at(&rec.data, CO_CHAIRMAN);
        let set = self.company_set();
        let idx = set.iter().position(|&c| c == slot);
        let human_shares: i32 = idx.map_or(0, |i| {
            pool::list_order(&self.save.persons)
                .into_iter()
                .filter_map(|p| pool::record(&self.save.persons, p))
                .filter(|p| i32_at(&p.data, PE_HUMAN) != 0)
                .map(|p| i32_at(&p.data, PE_HOLDINGS + 4 * i))
                .sum()
        });
        let inp = MonthlyInputs {
            human_chairman: self.person_is_human(chairman),
            human_shares,
            track_effect: f64::from(self.effect_mult(0xc, slot)),
            overhead_effect: f64::from(self.effect_mult(0xf, slot)),
            manager_effect: f64::from(self.effect_mult(0x10, slot)),
            manager_salary: self.manager_salary(&rec.data),
        };
        let rep = {
            let ctx = self.fin_ctx(w.minutes, slot);
            co.monthly_step(&ctx, &inp)
        };
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            co.write_record(&mut r.data);
        }
        // The board's decisions are news for the local player's company (`0x4399d0`, LNG 2274 /
        // 2713, only without `0x570237`); a company with a human chairman stands for it.
        if let Some(d) = rep.dividend
            && inp.human_chairman
            && self.save.header[HDR_NO_ECONOMY] == 0
        {
            match d.outcome {
                crate::finance::DividendOutcome::Suspended => {
                    self.ts_globals
                        .news
                        .push(crate::message::GameMessage::DividendSuspended {
                            company: slot as i32,
                        });
                }
                crate::finance::DividendOutcome::Reduced => {
                    self.ts_globals
                        .news
                        .push(crate::message::GameMessage::DividendCut {
                            company: slot as i32,
                            to_bits: (d.annual as f32).to_bits(),
                        });
                }
                _ => {}
            }
        }
        // Shareholders: every person with shares of the company is paid (finance.md 6).
        if let (Some(d), Some(i)) = (rep.dividend, idx)
            && d.annual > 0.0
        {
            for p in pool::list_order(&self.save.persons) {
                let held = pool::record(&self.save.persons, p)
                    .map_or(0, |r| i32_at(&r.data, PE_HOLDINGS + 4 * i));
                if held != 0 {
                    self.person_add_cash(p, f64::from(holder_dividend(held, d.annual)));
                }
            }
        }
        if chairman > 0 {
            self.person_add_cash(chairman as usize, f64::from(rep.chairman_pay));
        }
        self.log.ran("company_month_step");
    }

    /// `margin_check_all` (`0x46bb10`, finance.md 7.3): the broker's routine for every person of
    /// the player set (calls, forced sales and covers, see `fincmd.rs`).
    pub(crate) fn margin_check_all(&mut self, w: &mut World) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "margin_check_all",
                "margin_check_all 0x46bb10 on a 0x233C file",
                "finance.md 7.3"
            );
            return;
        }
        let mut guard = super::fincmd::MarginGuard::default();
        self.fc_margin_check(w.minutes, w, &mut guard);
        self.log.ran("margin_check_all");
    }
}

// ---- player construction commands (cmd_execute 0x4b0210, crate::build::cmd) -------------------

impl SimState {
    /// Runs a construction command on the train-step view, then fires the scenario triggers
    /// it raised (`station_networks_rebuild` raises 4).
    pub(crate) fn apply_build(
        &mut self,
        w: &mut World,
        cmd: crate::build::cmd::BuildCmd,
    ) -> Result<i32, crate::build::cmd::CmdError> {
        let (r, triggers) = self.with_trainstep(w, |ts| {
            let mut ai = crate::ai::Ai::new(ts);
            let r = ai.check(&cmd).and_then(|()| ai.execute(cmd));
            let ev: Vec<u8> = ai
                .finish()
                .into_iter()
                .filter_map(|c| match c {
                    crate::ai::AiCommand::FireTrigger(k) => Some(k),
                    _ => None,
                })
                .collect();
            (r, ev)
        });
        for k in triggers {
            self.event_trigger(w, k);
        }
        r
    }
}
