//! The raw company record (`list_companies` `0x574c6c`, 0x4e86 bytes) as a [`Company`]
//! (docs/spec/finance.md section 1): [`Company::from_record`] reads the fields the finance rules
//! use and [`Company::write_record`] stores them back, leaving every other byte of the record as
//! it is. Floats round-trip exactly (the fields are the same widths as in the original).

use crate::finance::{Bond, Company};
use crate::pool::{f32_at, f64_at, i32_at, put_f32, put_f64, put_u32};

pub const CO_SIZE: usize = 0x4e86;
pub const CO_CHAIRMAN: usize = 0x23;
pub const CO_SHARES: usize = 0x2c;
pub const CO_BOND_COUNT: usize = 0x40;
pub const CO_BONDS: usize = 0x44;
pub const CO_CHAIRMAN_SALARY: usize = 0x134;
pub const CO_OVERHEAD: usize = 0x138;
pub const CO_MANAGER: usize = 0x13c;
pub const CO_FOUNDED: usize = 0x148;
pub const CO_BANKRUPT_YEAR: usize = 0x158;
pub const CO_ACTIVE: usize = 0x16e;
pub const CO_EFFECTS: usize = 0x1ae;
pub const CO_PRICE_STAMP: usize = 0x4250;
pub const CO_CATS: usize = 0x4286;
pub const CO_YEAR_CATS: usize = 0x46fe;
pub const CO_ARCH_CATS: usize = 0x42ee;
pub const CO_ACCT: usize = 0x4766;
pub const CO_ARCH_ACCT: usize = 0x47a6;
pub const CO_ARCH_METRICS: usize = 0x49b6;

impl Company {
    /// Reads the finance fields of a raw company record.
    pub fn from_record(r: &[u8]) -> Company {
        let mut c = Company {
            shares: i32_at(r, CO_SHARES),
            dividend: f32_at(r, 0x30),
            pressure: f32_at(r, 0x34),
            dividends_paid: f32_at(r, 0x38),
            init_price: f32_at(r, 0x3c),
            overhead: i32_at(r, CO_OVERHEAD),
            chairman_salary: i32_at(r, CO_CHAIRMAN_SALARY),
            founded: i32_at(r, CO_FOUNDED),
            bankrupt_year: i32_at(r, CO_BANKRUPT_YEAR),
            price_stamp: i32_at(r, CO_PRICE_STAMP),
            price_cache: f64_at(r, 0x4254),
            created: i32_at(r, 0x425c),
            investor_mood: f32_at(r, 0x4276),
            no_div_price: f32_at(r, 0x427a),
            last_price: f32_at(r, 0x427e),
            month_end_price: f32_at(r, 0x4282),
            ..Company::default()
        };
        for (i, v) in c.cats.iter_mut().enumerate() {
            *v = f64_at(r, CO_CATS + 8 * i);
        }
        for (i, v) in c.year_cats.iter_mut().enumerate() {
            *v = f64_at(r, CO_YEAR_CATS + 8 * i);
        }
        for (k, row) in c.arch_cats.iter_mut().enumerate() {
            for (i, v) in row.iter_mut().enumerate() {
                *v = f64_at(r, CO_ARCH_CATS + 0x68 * k + 8 * i);
            }
        }
        for (i, v) in c.acct.iter_mut().enumerate() {
            *v = f64_at(r, CO_ACCT + 8 * i);
        }
        for (k, row) in c.arch_metrics.iter_mut().enumerate() {
            for (i, v) in row.iter_mut().enumerate() {
                *v = f64_at(r, CO_ARCH_METRICS + 0x70 * k + 8 * i);
            }
        }
        let n = (i32_at(r, CO_BOND_COUNT).max(0) as usize).min(crate::finance::MAX_BONDS);
        for k in 0..n {
            let o = CO_BONDS + 12 * k;
            c.bonds.push(Bond {
                principal: i32_at(r, o),
                maturity_year: i32_at(r, o + 4),
                rate: f32_at(r, o + 8),
            });
        }
        c
    }

    /// Writes the finance fields back into the raw record.
    pub fn write_record(&self, r: &mut [u8]) {
        put_u32(r, CO_SHARES, self.shares as u32);
        put_f32(r, 0x30, self.dividend);
        put_f32(r, 0x34, self.pressure);
        put_f32(r, 0x38, self.dividends_paid);
        put_f32(r, 0x3c, self.init_price);
        put_u32(r, CO_OVERHEAD, self.overhead as u32);
        put_u32(r, CO_CHAIRMAN_SALARY, self.chairman_salary as u32);
        put_u32(r, CO_BANKRUPT_YEAR, self.bankrupt_year as u32);
        put_u32(r, CO_PRICE_STAMP, self.price_stamp as u32);
        put_f64(r, 0x4254, self.price_cache);
        put_f32(r, 0x4276, self.investor_mood);
        put_f32(r, 0x427a, self.no_div_price);
        put_f32(r, 0x427e, self.last_price);
        put_f32(r, 0x4282, self.month_end_price);
        for (i, v) in self.cats.iter().enumerate() {
            put_f64(r, CO_CATS + 8 * i, *v);
        }
        for (i, v) in self.year_cats.iter().enumerate() {
            put_f64(r, CO_YEAR_CATS + 8 * i, *v);
        }
        for (k, row) in self.arch_cats.iter().enumerate() {
            for (i, v) in row.iter().enumerate() {
                put_f64(r, CO_ARCH_CATS + 0x68 * k + 8 * i, *v);
            }
        }
        for (i, v) in self.acct.iter().enumerate() {
            put_f64(r, CO_ACCT + 8 * i, *v);
        }
        for (k, row) in self.arch_metrics.iter().enumerate().skip(1) {
            for (i, v) in row.iter().enumerate() {
                put_f64(r, CO_ARCH_METRICS + 0x70 * k + 8 * i, *v);
            }
        }
        put_u32(r, CO_BOND_COUNT, self.bonds.len() as u32);
        for (k, b) in self.bonds.iter().enumerate() {
            let o = CO_BONDS + 12 * k;
            put_u32(r, o, b.principal as u32);
            put_u32(r, o + 4, b.maturity_year as u32);
            put_f32(r, o + 8, b.rate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_untouched_bytes() {
        let mut rec = vec![0u8; CO_SIZE];
        for (i, b) in rec.iter_mut().enumerate() {
            *b = (i * 7 + 3) as u8;
        }
        // A plausible bond count so that the array is read.
        put_u32(&mut rec, CO_BOND_COUNT, 3);
        let c = Company::from_record(&rec);
        let mut back = rec.clone();
        c.write_record(&mut back);
        // Row 0 of the metric archive is the live metric: never written back.
        assert_eq!(&rec[..CO_ARCH_METRICS], &back[..CO_ARCH_METRICS]);
        assert_eq!(
            &rec[CO_ARCH_METRICS + 0x70..],
            &back[CO_ARCH_METRICS + 0x70..]
        );
    }
}
