//! Command `0x26` (`cmd_found_company` `0x4bdcd0`, docs/spec/commands.md and finance.md 17) on
//! the raw records: the window checks of `0x4bd950`, `company_init` (`0x438a10`) and the
//! founder's share purchase.
//!
//! Not modelled (reported as errors or left out, finance.md 17 "Em aberto"): the resignation of a
//! previous chairmanship (`0x438900`, the command is refused instead), the temporary company name
//! (the LNG table `TEMP_COMPANY_NAMES` is not in the core: only the counter `0x5707b9` advances;
//! front ends send a name), and the territory standing of `0x43ddb0` (read, never tested: applied
//! as written in 17.4).

use super::company::{PE_ACTIVE, PE_CASH, PE_HOLDINGS, PE_HUMAN, ST_SET_COUNT, ST_SET_IDS};
use super::review::{PickIn, ST_MANAGER_AVAIL, ST_MANAGER_USE, pick_manager};
use super::*;
use crate::build::cmd::CmdError;
use crate::company_record::{
    CO_ACCT, CO_ACTIVE, CO_CHAIRMAN, CO_FOUNDED, CO_MANAGER, CO_SHARES, CO_SIZE,
};
use crate::found::{self, FoundCompany};

const PE_CHAIR_OF: usize = 0x263;
const PE_REP: usize = 0x52b;
const PE_LAST_FOUNDED: usize = 0x551;
const PE_FOUNDED_COUNT: usize = 0x555;
const CO_COLOR: usize = 0x2b;
const CO_NAME: usize = 4;
const CO_CHAIR_SINCE: usize = 0x470;
const CO_SCENERY: usize = 0x478;
const CO_STANDING: usize = 0x47a;
const CO_STANDING_ALL: usize = 0x4229;
const CO_RIGHTS: usize = 0x479;
const CO_TERR_STRIDE: usize = 0x1ed;
const ST_TEMP_NAME: usize = 0x2d1; // 0x5707b9
const COLORS: usize = 41;

impl SimState {
    /// Value of the person's share positions (`0x4a6d80`): `0.5x` of each position, `1.5x` of
    /// each short one (finance.md 17.2).
    fn founding_stock_value(&self, rec: &[u8], set: &[usize]) -> f64 {
        let mut s = 0.0;
        for (i, &c) in set.iter().enumerate().take(36) {
            let h = i32_at(rec, PE_HOLDINGS + 4 * i);
            if h == 0 {
                continue;
            }
            let price =
                pool::record(&self.save.companies, c).map_or(0.0, |r| f32_at(&r.data, 0x427e));
            let v = f64::from(price) * f64::from(h);
            s += if v < 0.0 { v * 0.5 + v } else { v * 0.5 };
        }
        s
    }

    /// Runs command `0x26`; returns the id of the new company.
    pub(crate) fn found_company(
        &mut self,
        w: &mut World,
        c: FoundCompany,
    ) -> Result<i32, CmdError> {
        if !self.v2348() {
            return Err(CmdError::Unknown);
        }
        let person = c.person.max(0) as usize;
        let Some(prec) = pool::record(&self.save.persons, person) else {
            return Err(CmdError::NoSuchObject);
        };
        let pd = prec.data.clone();
        if i32_at(&pd, PE_ACTIVE) == 0 {
            return Err(CmdError::NoSuchObject);
        }
        // Window checks `0x4bd950`, in the order of the original (finance.md 17.1).
        if i32_at(&pd, PE_CHAIR_OF) != 0 {
            return Err(CmdError::AlreadyChairman);
        }
        let set = self.company_set();
        if set.len() >= found::MAX_COMPANIES {
            return Err(CmdError::TooManyCompanies);
        }
        let now = w.minutes;
        if now.wrapping_sub(i32_at(&pd, PE_LAST_FOUNDED)) < found::FOUNDING_COOLDOWN_MINUTES {
            return Err(CmdError::FoundingCooldown);
        }
        let cash = f64_at(&pd, PE_CASH);
        let power = found::purchasing_power(cash, self.founding_stock_value(&pd, &set));
        if power < found::MIN_POWER {
            return Err(CmdError::InsufficientCash);
        }
        // The packet must carry values the two sliders can produce.
        let year = i32::from(date_of(now).year);
        let p = c.player_investment;
        if !found::player_slider(power).contains(p) {
            return Err(CmdError::InvalidInput);
        }
        let human = i32_at(&pd, PE_HUMAN) != 0;
        let m = found::credit_multiplier(human, i32_at(&pd, PE_REP), false, 0, year);
        let outside = found::outside_slider(m, p);
        if !outside.contains(c.capital.wrapping_sub(p)) {
            return Err(CmdError::InvalidInput);
        }
        if !(-1..COLORS as i32).contains(&c.color) {
            return Err(CmdError::InvalidInput);
        }

        let slot = self.company_init(w, c, human, year)?;
        let (price, shares, _) = found::initial_stock(c.capital);
        let n_p = found::founder_shares(p, price, shares);
        let cost = ftol(f64::from(n_p) * f64::from(price));
        let idx = set.len();
        if let Some(r) = pool::record_mut(&mut self.save.persons, person) {
            let d = &mut r.data;
            let cash = f64_at(d, PE_CASH);
            put_f64(d, PE_CASH, cash - f64::from(cost));
            put_u32(d, PE_HOLDINGS + 4 * idx, n_p as u32);
            put_u32(d, PE_CHAIR_OF, slot as u32);
            let count = i32_at(d, PE_FOUNDED_COUNT);
            put_u32(d, PE_FOUNDED_COUNT, count.wrapping_add(1) as u32);
            // Inferred: the stamp read by the cooldown check of 17.1.
            put_u32(d, PE_LAST_FOUNDED, now as u32);
        }
        self.log.ran("company_found");
        self.event_trigger(w, 5);
        Ok(slot as i32)
    }

    /// `company_init` (`0x438a10`, finance.md 17.4): colour, manager and candidates draw the game
    /// RNG in this order (the order proven by `company_init_matches_original`).
    fn company_init(
        &mut self,
        w: &mut World,
        c: FoundCompany,
        human: bool,
        year: i32,
    ) -> Result<usize, CmdError> {
        let now = w.minutes;
        let month = i32::from(date_of(now).month);
        let (price, shares, cash) = found::initial_stock(c.capital);
        let mut d = vec![0u8; CO_SIZE];
        // Colour: `-1` = the (r+1)-th free colour, the new company counting as colour 0.
        let color = if c.color == -1 {
            let mut used = [false; COLORS];
            used[0] = true;
            for slot in self.company_set() {
                if let Some(r) = pool::record(&self.save.companies, slot)
                    && i32_at(&r.data, CO_ACTIVE) != 0
                    && r.data[CO_SCENERY] == 0
                {
                    used[usize::from(r.data[CO_COLOR]).min(COLORS - 1)] = true;
                }
            }
            let free = used.iter().filter(|&&u| !u).count() as i32;
            if free == 0 {
                0
            } else {
                let mut left = w.rng.range(0, free - 1) + 1;
                let mut pick = 0;
                for (i, &u) in used.iter().enumerate() {
                    if !u {
                        left -= 1;
                    }
                    if left == 0 {
                        pick = i;
                        break;
                    }
                }
                pick as u8
            }
        } else {
            c.color as u8
        };
        // Manager and two candidates (finance.md 14.1, 17.4).
        let mut uses = [0u8; 40];
        uses.copy_from_slice(&self.save.state[ST_MANAGER_USE..ST_MANAGER_USE + 40]);
        let mut avail = [0u8; 40];
        avail.copy_from_slice(&self.save.state[ST_MANAGER_AVAIL..ST_MANAGER_AVAIL + 40]);
        let sal = self.manager_salaries();
        let zero = [0.0f64; 13];
        let arch = [[0.0f64; 13]; 10];
        let pick_in = |uses: [u8; 40], c140: i32, c144: i32| PickIn {
            year,
            month,
            founded: year,
            cats: &zero,
            arch: &arch,
            c140,
            c144,
            uses,
            avail,
            sal,
        };
        let g0 = pick_manager(&pick_in(uses, -1, -1), &mut w.rng);
        let mut uses2 = uses;
        uses2[g0 as usize] = 0xff;
        let g1 = pick_manager(&pick_in(uses2, -1, -1), &mut w.rng);
        let g2 = pick_manager(&pick_in(uses2, g1, -1), &mut w.rng);

        put_u32(&mut d, CO_CHAIRMAN, c.person as u32);
        d[CO_COLOR] = color;
        put_u32(&mut d, CO_SHARES, shares as u32);
        put_f32(&mut d, 0x30, 0.5);
        put_f32(&mut d, 0x3c, price);
        put_u32(&mut d, 0x134, 10_000);
        put_u32(&mut d, 0x138, 6000);
        put_u32(&mut d, CO_MANAGER, g0 as u32);
        put_u32(&mut d, 0x140, g1 as u32);
        put_u32(&mut d, 0x144, g2 as u32);
        put_u32(&mut d, 0x15c, 15);
        // Inferred layout of the date of the year 1800 (no waiting to issue shares).
        put_u16(&mut d, 0x160, 1800);
        d[0x162] = 1;
        d[0x163] = 1;
        put_u32(&mut d, CO_FOUNDED, year as u32);
        put_u16(&mut d, CO_CHAIR_SINCE, year as u16);
        put_u32(&mut d, CO_ACTIVE, 1);
        put_u32(&mut d, 0x17e, now.wrapping_sub(1) as u32);
        put_u32(&mut d, 0x425c, now as u32);
        put_f32(&mut d, 0x4248, 1.0);
        put_f32(&mut d, 0x4276, if human { 0.0 } else { 1.0 });
        put_f32(&mut d, 0x427e, price);
        put_f32(&mut d, 0x4282, price);
        put_f64(&mut d, CO_ACCT, f64::from(cash));
        put_f64(&mut d, CO_ACCT + 56, -f64::from(cash));
        put_u32(&mut d, CO_STANDING_ALL, 5000);
        // Name sent by the front end; the temporary-name counter advances without one.
        let n = c.name.iter().position(|&b| b == 0).unwrap_or(31).min(31);
        if n > 0 {
            d[CO_NAME..CO_NAME + n].copy_from_slice(&c.name[..n]);
        } else {
            let t = u32_at(&self.save.state, ST_TEMP_NAME);
            put_u32(&mut self.save.state, ST_TEMP_NAME, t.wrapping_add(1));
        }
        // Territories (`0x43ddb0`): standing and rights.
        let terr: Vec<(usize, i16)> = pool::list_order(&self.save.territories)
            .into_iter()
            .filter_map(|t| pool::record(&self.save.territories, t).map(|r| (t, r)))
            .map(|(t, r)| {
                let v = i32::from(r.data.get(0x4e).copied().unwrap_or(0));
                (t, if v == 0 { 5000 } else { 2 * v - 5000 } as i16)
            })
            .collect();
        for (t, standing) in terr {
            let base = CO_RIGHTS + t * CO_TERR_STRIDE;
            if base + (CO_STANDING - CO_RIGHTS) + 2 <= d.len() {
                d[base] |= 0xf;
                put_u16(&mut d, base + (CO_STANDING - CO_RIGHTS), standing as u16);
            }
        }
        let slot =
            pool::alloc(&mut self.save.companies, d).map_err(|_| CmdError::TooManyCompanies)?;
        if let Some(r) = pool::record_mut(&mut self.save.companies, slot) {
            put_u32(&mut r.data, 0, slot as u32);
        }
        // `company_set_manager(g0)`: the manager is employed by the new company.
        self.save.state[ST_MANAGER_USE + g0 as usize] = slot as u8;
        // Company set (`0x46a810`).
        let n = i32_at(&self.save.state, ST_SET_COUNT).max(0) as usize;
        if n < 36 {
            put_u16(&mut self.save.state, ST_SET_IDS + 2 * n, slot as u16);
            put_u32(&mut self.save.state, ST_SET_COUNT, n as u32 + 1);
        }
        self.log.ran("company_init");
        Ok(slot)
    }
}
