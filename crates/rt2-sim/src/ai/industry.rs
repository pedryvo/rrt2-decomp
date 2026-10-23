//! Industry purchase of the AI (`ai_industry_investment` `0x436950`, ai.md section 8) and the
//! purchase command it queues (`building_buy_request` `0x43ea30` -> command `0x2d` ->
//! `company_industry_trade` `0x43eb60`, terrain-track.md 2.4).

use super::{Ai, AiCommand, CO_STATIONS, Trait};
use crate::fp::ftol;
use crate::trainstep::{P, Ts};

/// `building_shape_table` cost (`0x5457ef + type*0x154`).
pub fn building_type_cost(ts: &Ts<'_>, t: u8) -> i32 {
    ts.img.i32(0x0054_57ef + 0x154 * u32::from(t))
}

impl Ai<'_, '_> {
    /// `ai_industry_investment` (`0x436950`) [S]: no RNG.
    pub fn industry_investment(&mut self) {
        self.ts.log.ran("ai_industry_investment");
        if self.co_i32(CO_STATIONS) < 2
            || self.st8(0x0057_3d78) != 2
            || !self.perm(0x0057_06b9)
            || self.cash() < 200_000.0
        {
            return;
        }
        let chair = self.chairman();
        let mut limit: f32 = 0.16;
        if chair != 0 {
            let x = 300 - self.persona(chair, Trait::IndustryInvestment);
            limit = (f64::from(x) * 0.000_799_999_982_118_606_6) as f32;
        }
        let mut pick = 0i32;
        for b in self.ids(P::Industry) {
            let kind = self.ts.u8(P::Industry, b, 7);
            if self.ts.img.u8(0x0053_5c39 + 0x27 * u32::from(kind)) == 0 {
                continue;
            }
            if self.ts.u16(P::Industry, b, 0x4b) <= 0x6b {
                continue;
            }
            let t = self.ts.u8(P::Industry, b, 6);
            let price = building_type_cost(self.ts, t);
            if self.cash() <= f64::from(price) {
                continue;
            }
            if self.ts.u16(P::Industry, b, 0x97) != 0 {
                continue;
            }
            let prod = f64::from(self.ts.i16(P::Industry, b, 0x36));
            let mut r = prod * f64::from(1000.0f32) / f64::from(price);
            if self.ts.u8(P::Industry, b, 0x33) != 0 {
                let st = self.industry_first_station(b);
                let rr = f64::from(r as f32);
                r = if self.ts.i32(P::Station, st, 0x39) == self.company {
                    rr * 1.1
                } else {
                    rr * 0.7
                };
            }
            if r > f64::from(limit) {
                limit = r as f32;
                pick = i32::from(self.ts.u16(P::Industry, b, 0));
            }
        }
        if pick != 0 {
            self.building_buy_request(pick);
        }
    }

    /// `0x40a5c0(industry, 0, NULL)`: the station of the first catchment link (`+0x2f`).
    fn industry_first_station(&self, b: i32) -> i32 {
        let link = self.ts.i32(P::Industry, b, 0x2f);
        i32::from(self.ts.u16(P::Link, link, 2))
    }

    /// `building_buy_request` (`0x43ea30`): queues command `0x2d` when the territory allows
    /// buying (`company+0x479+terr*0x1ed & 2`) and the price is covered.
    fn building_buy_request(&mut self, b: i32) {
        let terr = i32::from(self.ts.i8(P::Industry, b, 0x9d));
        let price = building_price(self.ts, b, self.company);
        let rights = self
            .ts
            .u8(P::Company, self.company, 0x479 + (terr * 0x1ed) as usize);
        // Original: the track right (bit 1); fix: the building right (bit 2).
        let need = if self
            .ts
            .g
            .rules
            .has(crate::ruleset::Fix::TerritoryRightsPerKind)
        {
            4
        } else {
            2
        };
        if rights & need != 0 && f64::from(price) <= self.cash() {
            self.commands.push(AiCommand::BuyIndustry {
                company: self.company,
                industry: b,
            });
        }
    }
}

/// `building_price` (`0x40bae0`): an owned building is worth what was paid; otherwise
/// `ftol(effect(0xd, company, 0, territory) * type cost)`.
pub fn building_price(ts: &Ts<'_>, b: i32, company: i32) -> i32 {
    if ts.u16(P::Industry, b, 0x97) != 0 {
        return ts.i32(P::Industry, b, 0x99);
    }
    let terr = i32::from(ts.i8(P::Industry, b, 0x9d));
    let m = ts.effect(0xd, company, 0, terr);
    let t = ts.u8(P::Industry, b, 6);
    ftol(f64::from(m) * f64::from(building_type_cost(ts, t)))
}

/// `company_industry_trade(company, industry, 1)` (`0x43eb60`, purchase branch) [V in
/// `tests/stations.rs`]: account 5 up, cash down, the building records the price and owner.
pub fn buy_industry(ts: &mut Ts<'_>, company: i32, b: i32) {
    let price = building_price(ts, b, company);
    let mut r = ts.record(P::Company, company);
    let get = |r: &[u8], o: usize| crate::pool::f64_at(r, o);
    crate::pool::put_u32(&mut r, 0x4250, 0);
    let a5 = f64::from(price) + get(&r, 0x478e);
    crate::pool::put_f64(&mut r, 0x478e, a5);
    let cash = get(&r, 0x4766) - f64::from(price);
    crate::pool::put_f64(&mut r, 0x4766, cash);
    ts.set_record(P::Company, company, &r);
    ts.seti32(P::Industry, b, 0x99, price);
    ts.set16(P::Industry, b, 0x97, company as u16);
}
