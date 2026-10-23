//! Territory rights and industry trades (docs/spec/commands.md 3.6 and 3.7):
//! `territory_set_rights` (`0x4c8540`), the executor of command `0x2a` (`0x43e8e0`),
//! `company_territory_access` (`0x43df20`) and `company_industry_trade` (`0x43eb60`).

use crate::ai::Ai;
use crate::trainstep::P;

/// State offsets of the active company list (`0x570729` count, `0x57072d` u16 ids).
const ST_CO_COUNT: usize = 0x241;
const ST_CO_LIST: usize = 0x245;

impl Ai<'_, '_> {
    pub(crate) fn active_companies(&self) -> Vec<i32> {
        let n = self.ts.state_i32(ST_CO_COUNT).max(0) as usize;
        (0..n)
            .map(|k| {
                let o = ST_CO_LIST + 2 * k;
                i32::from(u16::from_le_bytes([
                    self.ts.s.state[o],
                    self.ts.s.state[o + 1],
                ]))
            })
            .collect()
    }

    /// `territory_set_rights(territory, bit, value, company, nested)` (`0x4c8540`): the
    /// territories that remap to this one (`+0x2a == t`, `+0x24 == 0`) first, last to first;
    /// with `company == 0` the territory's own flags (`+0x26..+0x29`); then the bit of
    /// `company+0x479+t*0x1ed` of the company (or of every active company); a change of the
    /// trains bit refreshes every train's route.
    pub fn territory_set_rights(
        &mut self,
        terr: i32,
        bit: u8,
        value: bool,
        company: i32,
        nested: bool,
    ) {
        let tid = self.ts.i32(P::Territory, terr, 0);
        if !nested {
            let subs: Vec<i32> = self
                .ids(P::Territory)
                .into_iter()
                .filter(|&s| {
                    self.ts.i32(P::Territory, s, 0) != tid
                        && self.ts.u8(P::Territory, s, 0x24) == 0
                        && i32::from(self.ts.u8(P::Territory, s, 0x2a)) == tid
                })
                .collect();
            for s in subs.into_iter().rev() {
                self.territory_set_rights(s, bit, value, company, true);
            }
        }
        if company == 0 {
            let o = match bit {
                1 => Some(0x26),
                2 => Some(0x27),
                4 => Some(0x28),
                8 => Some(0x29),
                _ => None,
            };
            if let Some(o) = o {
                self.ts.set8(P::Territory, terr, o, u8::from(value));
            }
        }
        for c in self.active_companies() {
            let id = self.ts.i32(P::Company, c, 0);
            if company != 0 && id != company {
                continue;
            }
            let o = 0x479 + tid as usize * 0x1ed;
            let old = self.ts.u8(P::Company, c, o);
            let new = if value { old | bit } else { old & !bit };
            self.ts.set8(P::Company, c, o, new);
            if bit & 1 != 0 && (new & 1) != (old & 1) {
                self.trains_refresh_all();
                self.trains_flag_route_check();
            }
            self.company_territory_access(c);
        }
    }

    /// `0x43df20`: `+0x422d` (track rights in every territory) and `+0x422e` (train rights in
    /// every territory).
    fn company_territory_access(&mut self, c: i32) {
        let (mut all_track, mut all_trains) = (1u8, 1u8);
        for t in self.ids(P::Territory) {
            let tid = self.ts.i32(P::Territory, t, 0);
            let r = self.ts.u8(P::Company, c, 0x479 + tid as usize * 0x1ed);
            if r & 2 == 0 {
                all_track = 0;
            }
            if r & 1 == 0 {
                all_trains = 0;
            }
        }
        self.ts.set8(P::Company, c, 0x422d, all_track);
        self.ts.set8(P::Company, c, 0x422e, all_trains);
    }

    /// Command `0x2a` (`0x43e8e0`): the rights of `mask` (bit 0 trains, 1 track, 2 buildings,
    /// 3 stations) in a territory for `price` (account 2 against cash).
    pub fn buy_territory_rights(&mut self, company: i32, terr: i32, mask: u8, price: i32) {
        for k in 0..4 {
            let b = 1u8 << k;
            if mask & b != 0 {
                self.territory_set_rights(terr, b, true, company, false);
            }
        }
        let mut r = self.ts.record(P::Company, company);
        crate::pool::put_u32(&mut r, 0x4250, 0);
        let v = f64::from(price) + crate::pool::f64_at(&r, 0x4776);
        crate::pool::put_f64(&mut r, 0x4776, v);
        let v = crate::pool::f64_at(&r, 0x4766) - f64::from(price);
        crate::pool::put_f64(&mut r, 0x4766, v);
        self.ts.set_record(P::Company, company, &r);
    }

    /// The sale of `company_industry_trade` (`0x43eb60`, flag 0): only the owner sells; account
    /// 5 and cash give back the price `+0x99`, half of it is lost (category 3).
    pub fn sell_industry(&mut self, company: i32, b: i32) {
        if i32::from(self.ts.i16(P::Industry, b, 0x97)) != company {
            return;
        }
        let v = self.ts.i32(P::Industry, b, 0x99);
        let mut r = self.ts.record(P::Company, company);
        let g = |r: &[u8], o: usize| crate::pool::f64_at(r, o);
        let d = -f64::from(v);
        crate::pool::put_u32(&mut r, 0x4250, 0);
        let x = d + g(&r, 0x478e);
        crate::pool::put_f64(&mut r, 0x478e, x);
        let x = g(&r, 0x4766) - d;
        crate::pool::put_f64(&mut r, 0x4766, x);
        let h = -f64::from(v >> 1);
        let x = h + g(&r, 0x429e);
        crate::pool::put_f64(&mut r, 0x429e, x);
        let x = h + g(&r, 0x4716);
        crate::pool::put_f64(&mut r, 0x4716, x);
        let x = h + g(&r, 0x4766);
        crate::pool::put_f64(&mut r, 0x4766, x);
        let x = g(&r, 0x479e) - h;
        crate::pool::put_f64(&mut r, 0x479e, x);
        self.ts.set_record(P::Company, company, &r);
        self.ts.set16(P::Industry, b, 0x97, 0);
    }
}
