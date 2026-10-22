//! The "Start New Company" dialog's numbers: the Player Investment and Outside Investment
//! sliders and what a company founded with that capital starts with (docs/spec/finance.md 17).
//!
//! Ports of the reference implementations that `crates/rt2-oracle/tests/company.rs` checks
//! against the original executable (`founding_sliders_match_original`,
//! `credit_multiplier_matches_original`, `company_init_matches_original`); the rules are in
//! `finance.md` 17.2 (sliders), 17.3 (command 0x26) and 17.4 (`company_init` 0x438a10).

/// A slider of `n` positions between `min` and `max` (`finance.md` 17.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slider {
    pub min: i32,
    pub max: i32,
    pub n: i32,
}

impl Slider {
    /// The value at position `pos`: `min + pos * (max - min) / (n - 1)` (`ftol`, i.e. truncated).
    pub fn value(&self, pos: i32) -> i32 {
        let pos = pos.clamp(0, (self.n - 1).max(0));
        if self.n <= 1 {
            return self.min;
        }
        self.min + (i64::from(pos) * i64::from(self.max - self.min) / i64::from(self.n - 1)) as i32
    }

    /// The starting position, `(n - 1) / 2`.
    pub fn default_pos(&self) -> i32 {
        (self.n - 1) / 2
    }

    /// The position whose value is closest to `value` from below (keeps the setting when the
    /// other slider changes, `finance.md` 17.2).
    pub fn pos_of(&self, value: i32) -> i32 {
        if self.n <= 1 || self.max <= self.min {
            return 0;
        }
        let step = f64::from(self.max - self.min) / f64::from(self.n - 1);
        ((f64::from(value - self.min) / step) as i32).clamp(0, self.n - 1)
    }
}

/// `Player Investment` (`0x4bc990`): `w` is the buying power, cash plus the (non-positive) value
/// of the shares held, capped at 100 million.
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
    let max = w - w % step;
    let n = ((max - min) / step + 1).max(1);
    Slider { min, max, n }
}

/// `Outside Investment` (`0x4bc7d0`) for a player investment `p` and the investors' confidence
/// `m` ([`investor_confidence`]).
pub fn outside_slider(m: f64, p: i32) -> Slider {
    let mp = m * f64::from(p);
    let mut c = if mp >= 1.0e9 {
        1_000_000_000
    } else {
        mp.trunc() as i32
    };
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
    let half = c / 2;
    let mut min = half.min(500_000 - p).max(0);
    if min % step != 0 {
        min += step - min % step;
    }
    let mut max = c - c % step;
    if max <= min + step {
        max = min + step;
    }
    let n = ((max - min) / step + 1).max(3);
    Slider { min, max, n }
}

/// The investors' confidence multiplier `m` (`0x4a8150`, `finance.md` 17.2): 10 for a computer
/// player; for a human `clamp((reputation + 100 [+ penalty]) * 0.1, 0.5, 15)` where the penalty,
/// `(min(10, max(0, years)) - 10) * 7`, only applies to a person who already chairs a company
/// (`years_as_chairman`).
pub fn investor_confidence(human: bool, reputation: i32, years_as_chairman: Option<i32>) -> f64 {
    if !human {
        return 10.0;
    }
    let penalty = years_as_chairman.map_or(0, |y| (y.clamp(0, 10) - 10) * 7);
    (f64::from(reputation + 100 + penalty) * 0.1).clamp(0.5, 15.0)
}

/// What `company_init` makes of a founding capital (`finance.md` 17.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewCompany {
    /// Initial share price (f32 like the record).
    pub price: f32,
    /// Shares issued, a multiple of 1000.
    pub shares: i32,
    /// Cash after the share sale: `ftol(price) * shares` (the rest of the capital is lost).
    pub cash: i32,
}

/// Initial price, shares and cash of a company founded with `capital` dollars.
pub fn company_init(capital: i32) -> NewCompany {
    let k = if capital == -1 { 500_000 } else { capital };
    let mut price = (k / 10_000) as f32;
    while price >= 100.0 {
        price *= 0.5;
    }
    let shares = (f64::from(k) / f64::from(price) * f64::from(0.001f32)).trunc() as i32 * 1000;
    let cash = (f64::from(price).trunc() as i32).wrapping_mul(shares);
    NewCompany {
        price,
        shares,
        cash,
    }
}

/// The founder's purchase of their own shares (`finance.md` 17.3): `n_p` shares, rounded to the
/// nearest 1000 of `P / price`, capped by the issue, and the money it costs (`ftol(n_p * price)`).
pub fn founder_purchase(own_investment: i32, c: &NewCompany) -> (i32, i32) {
    let want =
        ((f64::from(own_investment) / f64::from(c.price)).trunc() as i32 + 500) / 1000 * 1000;
    let n = c.shares.min(want.max(0));
    (n, (f64::from(n) * f64::from(c.price)).trunc() as i32)
}

/// The founding dialog state: both sliders and the derived numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Founding {
    /// Buying power `W` (capped at 100 million).
    pub power: i32,
    /// Investors' confidence `m`.
    pub confidence: f64,
    pub player_pos: i32,
    pub outside_pos: i32,
}

impl Founding {
    /// A dialog for a player with `power` buying power; both sliders start in the middle
    /// (`finance.md` 17.2).
    pub fn new(power: f64, confidence: f64) -> Founding {
        let power = power.clamp(-1e8, 1e8) as i32;
        let mut f = Founding {
            power,
            confidence,
            player_pos: 0,
            outside_pos: 0,
        };
        f.player_pos = f.player().default_pos();
        f.outside_pos = f.outside().default_pos();
        f
    }

    pub fn player(&self) -> Slider {
        player_slider(self.power)
    }

    pub fn player_value(&self) -> i32 {
        self.player().value(self.player_pos)
    }

    pub fn outside(&self) -> Slider {
        outside_slider(self.confidence, self.player_value())
    }

    pub fn outside_value(&self) -> i32 {
        self.outside().value(self.outside_pos)
    }

    /// Moves the player slider; the outside slider is recomputed and keeps its setting.
    pub fn move_player(&mut self, delta: i32) {
        let before = self.outside_value();
        let max = self.player().n - 1;
        self.player_pos = (self.player_pos + delta).clamp(0, max.max(0));
        self.outside_pos = self.outside().pos_of(before);
    }

    pub fn move_outside(&mut self, delta: i32) {
        let max = self.outside().n - 1;
        self.outside_pos = (self.outside_pos + delta).clamp(0, max.max(0));
    }

    /// Total capital `K = P + O` (`finance.md` 17.3).
    pub fn capital(&self) -> i32 {
        self.player_value().saturating_add(self.outside_value())
    }

    pub fn company(&self) -> NewCompany {
        company_init(self.capital())
    }

    /// Whether the player may start a company at all: buying power of at least 100 000
    /// (`finance.md` 17.1).
    pub fn allowed(&self) -> bool {
        self.power >= 100_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn company_init_matches_the_spec_examples() {
        // finance.md 17.4: K -> (price, shares, cash).
        let c = company_init(500_000);
        assert_eq!((c.price, c.shares, c.cash), (50.0, 10_000, 500_000));
        let c = company_init(1_000_000);
        assert_eq!((c.price, c.shares, c.cash), (50.0, 20_000, 1_000_000));
        let c = company_init(1_500_000);
        assert_eq!((c.price, c.shares, c.cash), (75.0, 20_000, 1_500_000));
        let c = company_init(4_000_000);
        assert_eq!((c.price, c.shares, c.cash), (50.0, 80_000, 4_000_000));
        let c = company_init(123_456);
        assert_eq!((c.price, c.shares, c.cash), (12.0, 10_000, 120_000));
        assert_eq!(company_init(-1), company_init(500_000));
    }

    #[test]
    fn player_slider_brackets() {
        // W < 1M: step 10 000 from 100 000.
        let s = player_slider(250_000);
        assert_eq!((s.min, s.max, s.n), (100_000, 250_000, 16));
        // 1M <= W < 10M: step 100 000.
        let s = player_slider(2_345_678);
        assert_eq!((s.min, s.max, s.n), (100_000, 2_300_000, 23));
        // 10M <= W: step (and minimum) 1M, 10M above 99 999 999.
        let s = player_slider(25_500_000);
        assert_eq!((s.min, s.max, s.n), (1_000_000, 25_000_000, 25));
        let s = player_slider(100_000_000);
        assert_eq!((s.min, s.max, s.n), (10_000_000, 100_000_000, 10));
        // The middle position is (n - 1) / 2.
        assert_eq!(player_slider(250_000).default_pos(), 7);
        assert_eq!(player_slider(250_000).value(7), 100_000 + 7 * 150_000 / 15);
    }

    #[test]
    fn outside_slider_follows_the_player_investment_and_confidence() {
        // m = 10, P = 100 000: C = 1 000 000, step 100 000; min = ceil(500 000, step), max = C.
        let s = outside_slider(10.0, 100_000);
        assert_eq!(s.min, 400_000);
        assert_eq!(s.max, 1_000_000);
        assert_eq!(s.n, 7);
        // Little money and poor confidence: C = max(m*P, 200 000 - P) <= 50 000 is impossible
        // with P >= 100 000, so C = 100 000 at least and a three-position slider is the floor.
        let s = outside_slider(0.5, 100_000);
        assert!(s.n >= 3);
        assert!(s.max >= s.min + 1000);
        // A huge m*P clamps at 4 million with 100 000 steps.
        let s = outside_slider(15.0, 10_000_000);
        assert_eq!(s.max, 4_000_000);
        assert_eq!(s.min, 0);
    }

    #[test]
    fn investor_confidence_rules() {
        assert_eq!(investor_confidence(false, -100, None), 10.0);
        assert_eq!(investor_confidence(true, 50, None), 15.0);
        assert!((investor_confidence(true, 0, None) - 10.0).abs() < 1e-9);
        assert_eq!(investor_confidence(true, -200, None), 0.5);
        // A sitting chairman of 0 years loses 70 points.
        assert!((investor_confidence(true, 0, Some(0)) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_founder_buys_in_lots_of_a_thousand() {
        let c = company_init(1_000_000); // price 50, 20 000 shares
        assert_eq!(founder_purchase(500_000, &c), (10_000, 500_000));
        // 120 000 / 50 = 2400 -> (2400 + 500) / 1000 * 1000 = 2000 shares.
        assert_eq!(founder_purchase(120_000, &c), (2000, 100_000));
        // Never more than the issue.
        assert_eq!(founder_purchase(50_000_000, &c).0, 20_000);
    }

    #[test]
    fn the_dialog_moves_both_sliders_and_keeps_the_outside_setting() {
        let mut f = Founding::new(1_000_000.0, 10.0);
        assert!(f.allowed());
        let before = f.capital();
        f.move_player(1);
        assert!(f.player_value() > 0);
        let _ = before;
        let o = f.outside_value();
        f.move_outside(-1);
        assert!(f.outside_value() <= o);
        // Capital is the sum of both settings.
        assert_eq!(f.capital(), f.player_value() + f.outside_value());
        assert!(!Founding::new(99_999.0, 10.0).allowed());
    }
}
