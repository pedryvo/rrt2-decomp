//! The founding dialog math ("Start Company", docs/spec/finance.md section 17) and the packet of
//! command `0x26`: the sliders, the initial price and share count of `company_init`, and the
//! "You buy" line. The front end must call these functions and never re-derive the formulas.
//!
//! The slider limits and `company_init` arithmetic are the Rust references of
//! `crates/rt2-oracle/tests/company.rs` (`founding_sliders_match_original`,
//! `company_init_matches_original`, `credit_multiplier_matches_original`); the share rounding of
//! the founder (`cmd_found_company` `0x4bdcd0`, finance.md 17.3) is read from the disassembly.
//!
//! ## What the original shows for "You buy"
//!
//! The founder buys `min(shares, max(0, (ftol(P / price0) + 500) / 1000 * 1000))` shares: the
//! count is **rounded to the nearest 1000** (half up), not down, and the cost is `ftol(shares *
//! price0)`, written back over the investment. So the cost can exceed the investment `P`: with
//! `P = 150 000` and a price of 52.50 the founder buys 3000 shares for 157 500 (2857 shares
//! rounds up to 3000). It is not a bug of the clone: [`founding_preview`] reproduces it.

use crate::fp::ftol;

/// Minutes between two foundings by the same person: 672 days (`0xec400`, finance.md 17.1).
pub const FOUNDING_COOLDOWN_MINUTES: i32 = 0xec400;
/// Smallest purchasing power that allows founding (LNG 145, finance.md 17.1).
pub const MIN_POWER: i32 = 100_000;
/// Companies in the set at which founding is refused (LNG 144).
pub const MAX_COMPANIES: usize = 36;

fn crem(a: i32, b: i32) -> i32 {
    a.wrapping_rem(b)
}

fn cdiv(a: i32, b: i32) -> i32 {
    a.wrapping_div(b)
}

/// A founding slider (`0x4bc990`, `0x4bc7d0`): `n` positions between `min` and `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slider {
    pub min: i32,
    pub max: i32,
    pub n: i32,
}

impl Slider {
    /// Value of position `pos`: `ftol(min + pos * (max - min) / (n - 1))` (finance.md 17.2).
    pub fn value(&self, pos: i32) -> i32 {
        if self.n <= 1 {
            return self.min;
        }
        let pos = pos.clamp(0, self.n - 1);
        ftol(
            f64::from(self.min)
                + f64::from(pos) * f64::from(self.max - self.min) / f64::from(self.n - 1),
        )
    }

    /// Initial position `(n - 1) / 2`.
    pub fn initial_position(&self) -> i32 {
        (self.n - 1) / 2
    }

    /// Position that keeps the old value of the outside slider after the limits changed
    /// (finance.md 17.2): `clamp((old - min) / ((max - min) / (n - 1)), 0, n - 1)`.
    pub fn position_keeping(&self, old: i32) -> i32 {
        let k = self.n - 1;
        let step = if k > 0 {
            cdiv(self.max - self.min, k)
        } else {
            0
        };
        if step == 0 {
            return self.initial_position();
        }
        cdiv(old - self.min, step).clamp(0, k)
    }

    /// Whether `v` is one of the values the slider can produce.
    pub fn contains(&self, v: i32) -> bool {
        (0..self.n).any(|p| self.value(p) == v)
    }
}

/// Purchasing power `W` of a person (finance.md 17.2): `cash + s` when the stock value `s` is
/// not positive, else `cash`; capped at 1e8. `stock_value` is `0x4a6d80(person)`.
pub fn purchasing_power(cash: f64, stock_value: f64) -> i32 {
    let w = if stock_value <= 0.0 {
        cash + stock_value
    } else {
        cash
    };
    ftol(w.min(1.0e8))
}

/// "Player Investment" slider for the purchasing power `w` (`0x4bc990`).
pub fn player_slider(w: i32) -> Slider {
    let (step, min) = if w < 1_000_000 {
        (10_000, 100_000)
    } else if w < 10_000_000 {
        (100_000, 100_000)
    } else {
        let s = if w > 99_999_999 {
            10_000_000
        } else {
            1_000_000
        };
        (s, s)
    };
    let max = w - crem(w, step);
    let n = (cdiv(max - min, step) + 1).max(1);
    Slider { min, max, n }
}

/// Investor confidence `m` of a person (`0x4a8150`, finance.md 17.2). `chairman_since` is the
/// chairman year of the person's current company when `is_chairman`.
pub fn credit_multiplier(
    human: bool,
    reputation: i32,
    is_chairman: bool,
    chairman_since: i32,
    year: i32,
) -> f64 {
    if !human {
        return 10.0;
    }
    let pen = if is_chairman {
        (year - chairman_since).clamp(0, 10).saturating_sub(10) * 7
    } else {
        0
    };
    // Both clamps use `<=`: the original replaces a value equal to the bound by the bound.
    (f64::from(reputation + 100 + pen) * 0.1).clamp(0.5, 15.0)
}

/// "Outside Investment" slider for the confidence `m` and the player investment `p`
/// (`0x4bc7d0`).
pub fn outside_slider(m: f64, p: i32) -> Slider {
    let mp = m * f64::from(p);
    let mut c = if mp >= 1.0e9 { 1_000_000_000 } else { ftol(mp) };
    if c <= 200_000 - p {
        c = 200_000 - p;
    }
    let step;
    if c <= 50_000 {
        c = 50_000;
        step = 1000;
    } else if c >= 4_000_000 {
        c = 4_000_000;
        step = 100_000;
    } else if c < 100_000 {
        step = 1000;
    } else if c < 1_000_000 {
        step = 10_000;
    } else {
        step = 100_000;
    }
    let half = cdiv(c, 2);
    let mut min = half.min(500_000 - p).max(0);
    if crem(min, step) != 0 {
        min += step - crem(min, step);
    }
    let mut max = c - crem(c, step);
    if max <= min + step {
        max = min + step;
    }
    let n = (cdiv(max - min, step) + 1).max(3);
    Slider { min, max, n }
}

/// Initial price, share count and cash of a company founded with `capital` (`company_init`
/// `0x438a10`, finance.md 17.4; `-1` means 500 000).
pub fn initial_stock(capital: i32) -> (f32, i32, i32) {
    let k = if capital == -1 { 500_000 } else { capital };
    let mut price = cdiv(k, 10_000) as f32;
    while price >= 100.0 {
        price *= 0.5;
    }
    let shares = ftol(f64::from(k) / f64::from(price) * f64::from(0.001f32)).wrapping_mul(1000);
    let cash = ftol(f64::from(price)).wrapping_mul(shares);
    (price, shares, cash)
}

/// What the founding dialog shows for a pair of slider values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoundingPreview {
    /// Total capital `K = P + O`.
    pub capital: i32,
    /// Initial share price (shown with two decimals).
    pub price: f32,
    /// Shares the company issues.
    pub company_shares: i32,
    /// Cash the company starts with: `ftol(price) * shares` (`K` rounded down).
    pub company_cash: i32,
    /// "You buy" shares: the founder's investment rounded to the nearest 1000 shares.
    pub founder_shares: i32,
    /// "You buy" cost, `ftol(founder_shares * price)`; may exceed the investment (see the module
    /// documentation).
    pub founder_cost: i32,
}

/// The founding dialog numbers for the player investment `p` and the outside investment `o`
/// (finance.md 17.3 and 17.4). The UI must display these and send `capital` and `founder_cost`.
pub fn founding_preview(p: i32, o: i32) -> FoundingPreview {
    let capital = p.wrapping_add(o);
    let (price, shares, cash) = initial_stock(capital);
    let founder_shares = founder_shares(p, price, shares);
    FoundingPreview {
        capital,
        price,
        company_shares: shares,
        company_cash: cash,
        founder_shares,
        founder_cost: ftol(f64::from(founder_shares) * f64::from(price)),
    }
}

/// `min(shares, max(0, (ftol(P / price) + 500) / 1000 * 1000))` (`0x4bdcd0`, finance.md 17.3).
pub fn founder_shares(p: i32, price: f32, shares: i32) -> i32 {
    let n = cdiv(
        ftol(f64::from(p) / f64::from(price)).wrapping_add(500),
        1000,
    )
    .wrapping_mul(1000);
    shares.min(n.max(0))
}

/// Packet of command `0x26` (docs/spec/commands.md: `+0xc` person, `+0xd` name char[31], `+0x2c`
/// logo/colour, `+0x2d` capital, `+0x31` investment). The executor recomputes the founder's cost
/// from `player_investment`, as the original overwrites `+0x31`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundCompany {
    pub person: i32,
    /// NUL-terminated name; empty = a temporary name (`TEMP_COMPANY_NAMES`, finance.md 17.4).
    pub name: [u8; 31],
    /// Logo/colour byte; `-1` lets the game pick a free colour.
    pub color: i32,
    /// Total capital `K = P + O` (`+0x2d`).
    pub capital: i32,
    /// Founder's investment `P` (`+0x31`).
    pub player_investment: i32,
}

impl FoundCompany {
    /// Packet with a name given as text (cut at 30 bytes).
    pub fn new(person: i32, name: &str, color: i32, capital: i32, player_investment: i32) -> Self {
        let mut n = [0u8; 31];
        for (d, s) in n.iter_mut().take(30).zip(name.bytes()) {
            *d = s;
        }
        FoundCompany {
            person,
            name: n,
            color,
            capital,
            player_investment,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn you_buy_rounds_shares_to_the_nearest_thousand() {
        // Price 52.50 needs K in 1 050 000..=1 059 999; 150 000 / 52.5 = 2857.14 -> 3000 shares.
        let p = founding_preview(150_000, 900_000);
        assert_eq!(p.capital, 1_050_000);
        assert_eq!(p.price, 52.5);
        assert_eq!(p.founder_shares, 3000);
        assert_eq!(p.founder_cost, 157_500);
        assert!(p.founder_cost > 150_000);
    }

    #[test]
    fn rounds_down_below_half_a_lot() {
        // price 50: 120 000 / 50 = 2400 -> 2000 shares, 100 000 paid.
        let p = founding_preview(120_000, 380_000);
        assert_eq!(
            (p.price, p.founder_shares, p.founder_cost),
            (50.0, 2000, 100_000)
        );
    }

    #[test]
    fn founder_cannot_buy_more_than_the_issue() {
        assert_eq!(founder_shares(10_000_000, 50.0, 10_000), 10_000);
    }

    #[test]
    fn initial_stock_examples_of_the_spec() {
        assert_eq!(initial_stock(500_000), (50.0, 10_000, 500_000));
        assert_eq!(initial_stock(-1), (50.0, 10_000, 500_000));
        assert_eq!(initial_stock(1_000_000), (50.0, 20_000, 1_000_000));
        assert_eq!(initial_stock(1_500_000), (75.0, 20_000, 1_500_000));
        assert_eq!(initial_stock(4_000_000), (50.0, 80_000, 4_000_000));
        assert_eq!(initial_stock(123_456), (12.0, 10_000, 120_000));
    }

    #[test]
    fn sliders_of_a_typical_player() {
        let s = player_slider(1_000_000);
        assert_eq!((s.min, s.max, s.n), (100_000, 1_000_000, 10));
        assert_eq!(s.value(0), 100_000);
        assert_eq!(s.value(s.n - 1), 1_000_000);
        let m = credit_multiplier(true, 0, false, 0, 1860);
        assert_eq!(m, 10.0);
        let o = outside_slider(m, 150_000);
        assert!(o.min <= o.max && o.n >= 3);
        assert!(o.contains(o.value(o.initial_position())));
    }

    #[test]
    fn purchasing_power_ignores_long_positions() {
        assert_eq!(purchasing_power(500_000.0, 200_000.0), 500_000);
        assert_eq!(purchasing_power(500_000.0, -100_000.0), 400_000);
        assert_eq!(purchasing_power(5.0e8, 0.0), 100_000_000);
    }
}
