//! Calendar and game speed (docs/spec/time.md sections 2 and 3).
//!
//! 12 months x 28 days x 24 h x 60 min, no leap years. The unit of game time is the minute.

use crate::fp::ftol;

pub const MONTHS_PER_YEAR: i32 = 12;
pub const DAYS_PER_MONTH: i32 = 28;
pub const MINUTES_PER_DAY: i32 = 1440;
pub const MINUTES_PER_MONTH: i32 = 40_320; // 28 * 1440
pub const MINUTES_PER_YEAR: i32 = 483_840; // 12 * 28 * 1440
/// `time_speed_scale` (`0x537578`), game minutes per millisecond at multiplier 1.0.
pub const SPEED_SCALE: f64 = 3.2;
/// Number of entries of the speed table; index 0 is pause.
pub const SPEED_LEVELS: usize = 11;
/// Speed a new game starts at (`0x4732ba`).
pub const DEFAULT_SPEED: u8 = 3;
pub const MAX_SPEED: u8 = 10;

/// Nominal frame interval of the clone, in milliseconds.
///
/// INTENTIONAL DIVERGENCE (time.md section 9): the original advances by the real elapsed time of
/// each frame (10 Hz gate, up to 1000 ms per frame), so its result depends on the frame rate.
/// The clone always advances with this fixed interval, which makes it deterministic but not
/// identical to a particular run of the original.
pub const NOMINAL_FRAME_MS: u32 = 100;

/// A broken-down game date and time (`game_date` + `game_time_of_day`, time.md section 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: u16,
    /// 1..=12
    pub month: u8,
    /// 1..=28
    pub day: u8,
    /// 0..=23
    pub hour: u8,
    /// 0..=59
    pub minute: u8,
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8) -> Self {
        Date {
            year,
            month,
            day,
            hour,
            minute,
        }
    }

    /// Absolute minutes since year 0 (`0x4a4950`).
    pub fn to_minutes(self) -> i32 {
        date_to_minutes(
            i32::from(self.year),
            i32::from(self.month),
            i32::from(self.day),
            i32::from(self.hour),
            i32::from(self.minute),
        )
    }

    /// Inverse of [`Date::to_minutes`] (`0x4a4a30`).
    pub fn from_minutes(t: i32) -> Self {
        minutes_to_date(t)
    }
}

/// `((((y*12 + mo)*28 + d - 29)*24 + h)*60) + mi` (`0x4a4950`, `0x4a4990`), 32-bit wrapping like
/// the original. At 1 Jan 00:00 of year `a` this is `a * 483840`.
pub fn date_to_minutes(y: i32, mo: i32, d: i32, h: i32, mi: i32) -> i32 {
    y.wrapping_mul(12)
        .wrapping_add(mo)
        .wrapping_mul(28)
        .wrapping_add(d)
        .wrapping_sub(29)
        .wrapping_mul(24)
        .wrapping_add(h)
        .wrapping_mul(60)
        .wrapping_add(mi)
}

/// `0x4a4a30`, valid for `t >= 0` (time.md section 2).
pub fn minutes_to_date(t: i32) -> Date {
    let y = t / MINUTES_PER_YEAR;
    let r = t - y * MINUTES_PER_YEAR;
    let mo = r / MINUTES_PER_MONTH + 1;
    let r = r - (mo - 1) * MINUTES_PER_MONTH;
    let d = r / MINUTES_PER_DAY + 1;
    let r = r - (d - 1) * MINUTES_PER_DAY;
    Date {
        year: y as u16,
        month: mo as u8,
        day: d as u8,
        hour: (r / 60) as u8,
        minute: (r % 60) as u8,
    }
}

/// `time_speed_table` (`0x536508`, f32[11]): multiplier per speed index. Game data, loaded from
/// the executable by [`crate::data::GameData`]; nothing is hardcoded here.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedTable(pub [f32; SPEED_LEVELS]);

impl SpeedTable {
    /// Game minutes to advance for `ms` milliseconds at speed index `speed` (`0x46c650`):
    /// `ftol(table[speed] * ms * 3.2)`, x87 at 53 bits (time.md section 3).
    pub fn minutes_for_ms(&self, speed: u8, ms: u32) -> i32 {
        let m = f64::from(self.0[usize::from(speed.min(MAX_SPEED))]);
        ftol(m * f64::from(ms) * SPEED_SCALE)
    }

    /// Minutes per simulation frame of the clone (see [`NOMINAL_FRAME_MS`]).
    pub fn minutes_per_frame(&self, speed: u8) -> i32 {
        self.minutes_for_ms(speed, NOMINAL_FRAME_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values listed in time.md section 3 (used here only as a test fixture).
    fn table() -> SpeedTable {
        SpeedTable([0.0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0])
    }

    #[test]
    fn jan_first_is_year_times_constant() {
        assert_eq!(date_to_minutes(1860, 1, 1, 0, 0), 1860 * MINUTES_PER_YEAR);
        assert_eq!(MINUTES_PER_YEAR, 12 * 28 * 24 * 60);
    }

    #[test]
    fn pack_unpack_round_trip() {
        for (y, mo, d, h, mi) in [
            (1860, 1, 1, 0, 0),
            (1900, 12, 28, 23, 59),
            (2203, 7, 15, 12, 30),
        ] {
            let t = date_to_minutes(y, mo, d, h, mi);
            let dt = minutes_to_date(t);
            assert_eq!(
                (
                    i32::from(dt.year),
                    i32::from(dt.month),
                    i32::from(dt.day),
                    i32::from(dt.hour),
                    i32::from(dt.minute)
                ),
                (y, mo, d, h, mi)
            );
            assert_eq!(dt.to_minutes(), t);
        }
    }

    #[test]
    fn one_minute_after_2359_dec_28_is_new_year() {
        let t = Date::new(1900, 12, 28, 23, 59).to_minutes() + 1;
        assert_eq!(Date::from_minutes(t), Date::new(1901, 1, 1, 0, 0));
    }

    #[test]
    fn minutes_per_frame_follows_the_formula() {
        let t = table();
        // speed 3 (x1.0): 3.2 game minutes per ms, so 100 ms -> 320 and a day takes 450 ms.
        assert_eq!(t.minutes_for_ms(3, 450), 1440);
        assert_eq!(t.minutes_per_frame(3), 320);
        // pause
        assert_eq!(t.minutes_per_frame(0), 0);
        // x18: 100 ms -> ftol(18 * 100 * 3.2) = 5760
        assert_eq!(t.minutes_per_frame(10), 5760);
        // time.md section 3: at 53 bits speed 5, 75 ms gives 503 (24-bit precision gives 504)
        assert_eq!(t.minutes_for_ms(5, 75), 503);
    }
}
