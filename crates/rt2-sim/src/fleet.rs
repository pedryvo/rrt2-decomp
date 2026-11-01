//! Fleet management: consist templates and bulk engine replacement (docs/PLAN.md section 12).

use serde::{Deserialize, Serialize};

use crate::build::cmd::{BuildCmd, CmdError};
use crate::pool;
use crate::world::World;

/// A consist template of wagons/cargo types for train stops (up to 8 cars).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsistTemplate {
    pub name: String,
    pub cars: Vec<u8>,
}

impl World {
    /// Returns the current cash balance of `company` (account 0, offset `+0x4766`).
    pub fn company_cash(&self, company: u8) -> f64 {
        let Some(s) = self.sim.as_ref() else {
            return 0.0;
        };
        let Some(rec) = pool::record(&s.save.companies, usize::from(company)) else {
            return 0.0;
        };
        pool::f64_at(&rec.data, 0x4766)
    }

    /// Applies a consist template of cars (up to 8 cargos) to a stop (or 0xff for all stops)
    /// of a train.
    pub fn apply_consist_template(
        &mut self,
        train: u16,
        stop: u8,
        cars: &[u8],
    ) -> Result<i32, CmdError> {
        let mut car_arr = [0xffu8; 8];
        let n = cars.len().min(8);
        car_arr[..n].copy_from_slice(&cars[..n]);
        self.apply_build(BuildCmd::ConsistCars {
            train: i32::from(train),
            stop: i32::from(stop),
            cars: car_arr,
        })
    }

    /// Bulk replaces engines for a company: finds trains belonging to `company` (matching
    /// `from_engine` if specified, or all trains whose current engine is not `to_engine`).
    /// Replaces trains while the company has sufficient cash, and returns the number of trains updated.
    pub fn bulk_replace_engines(
        &mut self,
        company: u8,
        from_engine: Option<u8>,
        to_engine: u8,
    ) -> Result<u32, CmdError> {
        let Some(sim) = self.sim.as_ref() else {
            return Err(CmdError::NoSuchObject);
        };
        let co_slot = usize::from(company);
        if pool::record(&sim.save.companies, co_slot).is_none() {
            return Err(CmdError::NoSuchObject);
        }

        let s = &sim.save;
        let img = &sim.data.image;
        let mut eligible = Vec::new();

        for t in pool::list_order(&s.trains) {
            let Some(tr) = pool::record(&s.trains, t) else {
                continue;
            };
            let tr_co = i32::from_le_bytes(tr.data[0x41..0x45].try_into().unwrap());
            if tr_co != i32::from(company) {
                continue;
            }
            let first_car = i32::from_le_bytes(tr.data[0..4].try_into().unwrap());
            if first_car <= 0 {
                continue;
            }
            let Some(cr) = pool::record(&s.pool_70, first_car as usize) else {
                continue;
            };
            let vt = cr.data.get(8).copied().unwrap_or(0);
            let cur_engine = img.i8(0x0053_ddb8 + u32::from(vt) * 0x39 + 0x2f) as i32;
            if cur_engine < 0 || cur_engine == i32::from(to_engine) {
                continue;
            }
            if let Some(fe) = from_engine
                && cur_engine != i32::from(fe)
            {
                continue;
            }
            eligible.push(t as i32);
        }

        let mut replaced = 0u32;
        for train_id in eligible {
            let Some(sim) = self.sim.as_ref() else {
                break;
            };
            let s = &sim.save;
            let img = &sim.data.image;
            let Some(tr) = pool::record(&s.trains, train_id as usize) else {
                continue;
            };
            let kind = usize::from(img.u8(0x0053_cec0 + u32::from(to_engine) * 0x3a + 0x28)) + 0x1b;
            let terr = i32::from(tr.data.get(0xf3).copied().unwrap_or(0));
            let m_kind =
                crate::trainstep::mem::effect_from_save(s, kind, i32::from(company), 0, terr);
            let m_all =
                crate::trainstep::mem::effect_from_save(s, 0x1a, i32::from(company), 0, terr);
            let base_price = img.i32(0x0053_cec0 + u32::from(to_engine) * 0x3a + 0xc);
            let price =
                crate::fp::ftol(f64::from(m_all) * f64::from(m_kind) * f64::from(base_price));

            let cash = self.company_cash(company);
            if cash < f64::from(price) {
                break;
            }

            let res = self.apply_build(BuildCmd::ReplaceEngine {
                train: train_id,
                engine: i32::from(to_engine),
            });
            if res.is_ok() {
                replaced += 1;
            }
        }
        Ok(replaced)
    }
}
