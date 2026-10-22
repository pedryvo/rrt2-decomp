//! The train steps of the month and year ends (world-tick.md 3.3 item 10, 3.4 item 11):
//! `train_monthly_costs` (`0x4d4390`, trains.md 6.2) and `train_year_step` (`0x4d4620`).

use super::mem::{P, Ts};
use crate::trains::{MonthlyInput, monthly_costs};

impl Ts<'_> {
    /// `train_monthly_costs` (`0x4d4390`): an AI train is refilled (sand, oil, water 100,
    /// throttle 85); fuel, maintenance and car costs ([`monthly_costs`], verified) are booked
    /// with `train_ledger_add` (3, 1, 2) and the month counters `+0xa2`/`+0xe1` restart.
    pub fn train_monthly_costs(&mut self, t: i32) {
        let head = self.i32(P::Train, t, 0);
        let eng_idx = self.vt_engine(self.u8(P::Car, head, 8));
        let company = self.i32(P::Train, t, 0x41);
        if !self.company_is_human(company) {
            self.set32(P::Train, t, 0xcf, 0x42c8_0000);
            self.set32(P::Train, t, 0xd3, 0x42c8_0000);
            self.set32(P::Train, t, 0xd7, 0x42c8_0000);
            self.set32(P::Train, t, 0xdb, 0x42aa_0000);
        }
        let kind = u32::from(self.en8(eng_idx, 0x28));
        let ton_cells = self.i32(P::Train, t, 0xa2);
        let cells = self.i32(P::Train, t, 0xe1);
        self.seti32(
            P::Train,
            t,
            0xa2,
            ton_cells.wrapping_add(cells.wrapping_mul(200)),
        );
        let terr = i32::from(self.u8(P::Train, t, 0xf3));
        // 0x567d6d[kind] (header block): price level of fuel and maintenance, 1 = neutral.
        let level = u32::from(
            self.s
                .header
                .get(0x235 + kind as usize)
                .copied()
                .unwrap_or(1),
        );
        let eng = self.engine_struct_pub(eng_idx);
        let costs = monthly_costs(
            &eng,
            &MonthlyInput {
                age: i32::from(self.year) - i32::from(self.u16(P::Train, t, 0xbd)),
                ton_cells,
                cells,
                oil: self.f32(P::Train, t, 0xd3),
                cargo_vehicles: self
                    .cars_with(t, |ts, c| ts.vt8(ts.u8(P::Car, c, 8), 0x30) != 0x7f)
                    .len() as i32,
                fuel_level: self.img.f32(0x0055_2ae8 + 4 * level),
                maint_level: self.img.f32(0x0055_2af4 + 4 * level),
                fuel_kind: f64::from(self.effect(0x12 + kind as usize, company, 0, terr)),
                fuel_all: f64::from(self.effect(0x11, company, 0, terr)),
                maint_kind: f64::from(self.effect(0x1b + kind as usize, company, 0, terr)),
                maint_all: f64::from(self.effect(0x15, company, 0, terr)),
                car_cost: f64::from(self.effect(0x19, company, 0, terr)),
            },
        );
        self.train_ledger_add(t, 3, costs.fuel);
        self.train_ledger_add(t, 1, costs.maintenance);
        self.train_ledger_add(t, 2, costs.cars);
        self.seti32(P::Train, t, 0xa2, 0);
        self.seti32(P::Train, t, 0xe1, 0);
    }

    /// `train_year_step` (`0x4d4620`): the yearly statistics move one year back
    /// (`+0x11d..+0x13d` <- `+0x10d..`, `+0x10d..+0x11d` <- `+0xfd..`) and the current year
    /// (`+0xfd..+0x10d`) restarts.
    pub fn train_year_step(&mut self, t: i32) {
        let rec = self.record(P::Train, t);
        let mut new = rec.clone();
        new.copy_within(0x10d..0x12d, 0x11d);
        let moved: Vec<u8> = new[0xfd..0x10d].to_vec();
        new[0x10d..0x11d].copy_from_slice(&moved);
        new[0xfd..0x10d].fill(0);
        self.set_record(P::Train, t, &new);
    }
}
