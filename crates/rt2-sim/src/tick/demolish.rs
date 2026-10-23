//! Demolition by scenario effects and liquidation (docs/spec/scenarios.md 4.2.1, finance.md
//! 15.5): the station buildings go through `building_destroy(b, 1)` (`0x40adb0`,
//! [`SimState::building_destroy_with`]), which reaches `station_remove` (`0x4bf4e0`) on the
//! train-step view.

use super::*;
use crate::rng::GameRng;

impl SimState {
    /// `0x437f50(company, territory, 1)` (`company_stations_release` with "destroy"): for each
    /// target company (`company_ptr(company)`, or the active set in order when `company < 1`),
    /// the stations of the list from the last to the first owned by it (`+0x39`) and in
    /// `territory` (`+0x4ad`, 0 = any) have their building destroyed.
    pub(super) fn stations_destroy(
        &mut self,
        minutes: i32,
        rng: &mut GameRng,
        crt: &mut crate::rng::CrtRng,
        company: i32,
        territory: i32,
    ) {
        let targets: Vec<i32> = if company < 1 {
            self.company_set().into_iter().map(|c| c as i32).collect()
        } else if pool::record(&self.save.companies, company as usize).is_some() {
            vec![company]
        } else {
            Vec::new()
        };
        for c in targets {
            let cid =
                pool::record(&self.save.companies, c as usize).map_or(c, |r| i32_at(&r.data, 0));
            self.stations_destroy_of(minutes, rng, crt, cid, territory);
        }
    }

    /// The station loop of `0x437f50` and `0x438170` with "destroy" for one company id.
    pub(super) fn stations_destroy_of(
        &mut self,
        minutes: i32,
        rng: &mut GameRng,
        crt: &mut crate::rng::CrtRng,
        cid: i32,
        territory: i32,
    ) {
        for st in pool::list_order(&self.save.stations).into_iter().rev() {
            let Some(r) = pool::record(&self.save.stations, st) else {
                continue;
            };
            if i32_at(&r.data, 0x39) != cid
                || (territory != 0 && i32_at(&r.data, 0x4ad) != territory)
            {
                continue;
            }
            let b = i32_at(&r.data, 4);
            if b > 0 {
                self.building_destroy_with(rng, Some((minutes, &mut *crt)), b as usize);
            }
        }
        self.log.ran("stations_destroy");
    }
}
