//! Computer persons (`ai_person_turn` `0x4a8800`, ai.md section 9): the decision to found a
//! company and the stock trading of the persons that are chairmen or investors.

use super::{Ai, Trait};
use crate::calendar::date_to_minutes;
use crate::trainstep::P;

impl Ai<'_, '_> {
    /// `ai_person_turn` (`0x4a8800`, ai.md 9.1).
    pub fn person_turn(&mut self, p: i32) {
        self.ts.log.ran("ai_person_turn");
        if self.ts.i32(P::Person, p, 4) == 0 {
            return;
        }
        if self.perm(0x0057_06be)
            && (self.perm(0x0057_06c4) || self.ts.i32(P::Person, p, 0x555) < 1)
        {
            if self.ts.i32(P::Person, p, 0x263) != 0 {
                self.trade_stocks(p);
                return;
            }
            self.founding_test(p);
        }
        if self.ts.i32(P::Person, p, 0x263) == 0 && self.ts.u8(P::Person, p, 0x54c) == 0 {
            return;
        }
        self.trade_stocks(p);
    }

    /// The founding branch of `0x4a8800` (person active, not a chairman).
    fn founding_test(&mut self, p: i32) {
        // Game start date (0x570508: u16 year, u8 month, u8 day; 0x57050c: hour, minute).
        let sy = i32::from(self.st_i16(0x0057_0508) as u16);
        let start = date_to_minutes(
            sy,
            i32::from(self.st8(0x0057_050a)),
            i32::from(self.st8(0x0057_050b)),
            i32::from(self.st8(0x0057_050c)),
            i32::from(self.st8(0x0057_050d)),
        );
        let since = self.now().wrapping_sub(start as u32);
        if since <= self.ts.u32(P::Person, p, 0x267) {
            return;
        }
        let cash = crate::pool::f64_at(&self.ts.record(P::Person, p), 0x12a);
        if cash <= 80_000.0 {
            return;
        }
        let land = self.st_i32(0x0057_3e04);
        let f = (f64::from(land) * 3.333_333_333_333_333_5e-5 + 2.0) as f32;
        let y = (self.year() - sy).min(30);
        let n = self.ids(P::Company).len() as i32;
        let starts = self.persona(p, Trait::StartsCompany);
        let q = (f64::from(n) / (f64::from(60 - y) * f64::from(f) * (1.0 / 60.0))
            * f64::from(200 - starts)
            * 0.01) as f32;
        let seed = self.ts.u32(P::Person, p, 0x548);
        let r = crate::rng::hash_range(seed, 0x50, 0x78);
        let s = f64::from(r) * f64::from(q) * 0.01;
        let forced = !self.perm(0x0057_06aa) || self.st8(0x0057_3d74) == 0;
        if s > 1.0 && !forced {
            self.ts.set8(P::Person, p, 0x54c, 1);
            let v = self.ts.u32(P::Person, p, 0x267).wrapping_add(0x76200);
            self.ts.set32(P::Person, p, 0x267, v);
            return;
        }
        if !self.can_found_company() {
            self.ts.set8(P::Person, p, 0x54c, 1);
            let v = self.ts.u32(P::Person, p, 0x267).wrapping_add(0x24ea00);
            self.ts.set32(P::Person, p, 0x267, v);
            return;
        }
        todo_hook!(
            self.ts.log,
            "ai_company_create",
            "company_create 0x4383f0 / company_init 0x438a10 (RNG), event_fire_trigger(5), the first company turn and the founder's shares",
            "ai.md 9.1; finance.md 14"
        );
    }

    /// `ai_can_found_company` (`0x4361c0`): `ai_plan_first_line(1)` with no current company.
    pub fn can_found_company(&mut self) -> bool {
        let saved = self.company;
        self.company = 0;
        let r = self.plan_first_line(true);
        self.company = saved;
        r
    }

    /// `ai_person_trade_stocks` (`0x4a8af0`, ai.md 9.2): no RNG of the game. Queued in
    /// [`Ai::traders`]; the world loop runs it with the finance commands
    /// (`SimState::ai_trade_stocks`) after the persons' phase.
    pub fn trade_stocks(&mut self, p: i32) {
        self.traders.push(p);
    }
}
