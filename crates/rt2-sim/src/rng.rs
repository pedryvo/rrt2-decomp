//! Random number generators of the simulation (docs/spec/rng.md).
//!
//! - Generator A, [`GameRng`]: the game LCG (`rng_range` `0x4f0660`, state `0x55d2c0`).
//! - Generator B, [`hash_range`]: the stateless seeded hash (`rng_hash_range` `0x4f06f0`).
//!
//! - Generator C, [`CrtRng`]: the MSVC CRT `rand` (`crt_rand` `0x51c137`, state `0x5609f0`) and
//!   `crt_rand_range` (`0x4f0790`). It drives sound and UI, but also the smoke puffs that the
//!   steam engines leave in `pool_78` (`0x412a90`, trains.md 10.6), which are saved: the clone
//!   must consume it in the same order as the original to reproduce a save.

/// Generator A: ANSI C LCG, `s = s * 0x41c64e6d + 0x3039`, 15-bit output (rng.md 2.1).
///
/// The order of calls is part of the game state; see rng.md 2.4 for who re-seeds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRng {
    state: u32,
}

impl GameRng {
    /// `rng_seed` (`0x4f06d0`).
    pub fn new(seed: u32) -> Self {
        GameRng { state: seed }
    }

    /// `rng_seed` (`0x4f06d0`).
    pub fn seed(&mut self, seed: u32) {
        self.state = seed;
    }

    /// `rng_get_state` (`0x4f06e0`). Not stored in `.GAM` files (rng.md 2.4).
    pub fn state(&self) -> u32 {
        self.state
    }

    /// One LCG step; returns the 15-bit output (rng.md 2.1).
    pub fn next15(&mut self) -> i32 {
        self.state = self.state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        ((self.state as i32) >> 16) & 0x7fff
    }

    /// `rng_range(lo, hi)`, inclusive on both ends (rng.md 2.2).
    ///
    /// Reproduces the original's modulo bias, the two-draw path for spans above `0x7fff` and the
    /// fact that `lo == hi` does not advance the state.
    ///
    /// # Panics
    /// When `hi == lo - 1` (so `span + 1 == 0`): the original divides by zero (crash).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        let span = hi.wrapping_sub(lo);
        if span > 0x7fff {
            let a = self.range(0, 0x7fff) as u32;
            let b = self.range(0, 0x7fff) as u32;
            let v = b | (a << 15);
            return (v % (span as u32 + 1)).wrapping_add(lo as u32) as i32;
        }
        if hi == lo {
            return hi;
        }
        let v = self.next15();
        let d = span.wrapping_add(1);
        assert!(
            d != 0,
            "rng_range({lo}, {hi}): division by zero in the original"
        );
        v.wrapping_rem(d).wrapping_add(lo)
    }
}

/// Generator C: the MSVC CRT `rand`, `s = s * 214013 + 2531011`, 15-bit output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrtRng {
    state: u32,
}

impl CrtRng {
    /// `crt_srand` (`0x51c12d`).
    pub fn new(seed: u32) -> Self {
        CrtRng { state: seed }
    }

    pub fn seed(&mut self, seed: u32) {
        self.state = seed;
    }

    pub fn state(&self) -> u32 {
        self.state
    }

    /// `crt_rand` (`0x51c137`).
    pub fn rand(&mut self) -> i32 {
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        ((self.state >> 16) & 0x7fff) as i32
    }

    /// `crt_rand_range(lo, hi)` (`0x4f0790`): inclusive; `hi == lo` and `hi < lo` return without
    /// a draw (`hi` and `lo` respectively); spans above `0x7fff` take two draws.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi == lo {
            return hi;
        }
        if hi < lo {
            return lo;
        }
        let span = hi.wrapping_sub(lo);
        if span > 0x7fff {
            let a = self.rand() as u32;
            let b = self.rand() as u32;
            let v = (b & 0x7fff) | ((a & 0x7fff) << 15);
            return (v % (span as u32).wrapping_add(1)).wrapping_add(lo as u32) as i32;
        }
        self.rand() % (span + 1) + lo
    }
}

/// `rng_hash_range(seed, lo, hi)` (`0x4f06f0`, rng.md section 3): pure, no state.
///
/// For `hi < lo` the spec (3) says the original returns `lo` in a local game (the version
/// check `0x49d920 > 0x232c` is always true there). TODO(rng.md "Em aberto", last item): that
/// branch is not covered by the oracle test.
pub fn hash_range(seed: u32, lo: i32, hi: i32) -> i32 {
    if hi < lo {
        return lo;
    }
    let span = hi.wrapping_sub(lo);
    if span > 0x7fff {
        let a = hash_range(seed, 0, 0x7fff) as u32;
        let b = hash_range(seed.wrapping_add(7), 0, 0x7fff) as u32;
        return ((b | (a << 15)) % (span as u32 + 1)).wrapping_add(lo as u32) as i32;
    }
    if hi == lo {
        return hi;
    }
    let t = 0x3039u32.wrapping_sub(seed.wrapping_mul(0x3e39_b193));
    let v = ((t as i32) >> 16) & 0x7fff;
    (v % (span + 1)).wrapping_add(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_step_matches_ansi_c() {
        // glibc/ANSI constants: state 0 -> 12345 -> (12345 >> 16) & 0x7fff = 0
        let mut r = GameRng::new(0);
        assert_eq!(r.next15(), 0);
        assert_eq!(r.state(), 0x3039);
        // second step: 0x3039 * 0x41c64e6d + 0x3039 (mod 2^32)
        let want = 0x3039u32.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        assert_eq!(r.next15(), ((want as i32) >> 16) & 0x7fff);
        assert_eq!(r.state(), want);
    }

    #[test]
    fn range_with_equal_bounds_keeps_state() {
        let mut r = GameRng::new(777);
        assert_eq!(r.range(5, 5), 5);
        assert_eq!(r.state(), 777);
    }

    #[test]
    fn large_span_consumes_two_draws() {
        let mut a = GameRng::new(1);
        let mut b = GameRng::new(1);
        a.range(0, 100_000);
        b.next15();
        b.next15();
        assert_eq!(a.state(), b.state());
        let mut c = GameRng::new(1);
        c.range(0, 99);
        let mut d = GameRng::new(1);
        d.next15();
        assert_eq!(c.state(), d.state());
    }

    #[test]
    fn range_is_inclusive_and_deterministic() {
        let mut r = GameRng::new(42);
        let mut seen = [false; 11];
        for _ in 0..2000 {
            let v = r.range(-5, 5);
            assert!((-5..=5).contains(&v));
            seen[(v + 5) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        let mut x = GameRng::new(42);
        let mut y = GameRng::new(42);
        for _ in 0..100 {
            assert_eq!(x.range(0, 10_000), y.range(0, 10_000));
        }
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn range_hi_below_lo_by_one_panics_like_the_original() {
        GameRng::new(1).range(0, -1);
    }

    #[test]
    fn hash_is_pure() {
        // t = 0x3039 - seed * 0x3e39b193; seed 0 -> t = 0x3039 -> (t >> 16) & 0x7fff = 0
        assert_eq!(hash_range(0, 3, 9), 3);
        assert_eq!(hash_range(1, 4, 4), 4);
        assert_eq!(hash_range(12345, 0, 99), hash_range(12345, 0, 99));
        // hi < lo returns lo (local game)
        assert_eq!(hash_range(9, 10, 3), 10);
        // wide span uses seed and seed + 7
        let wide = hash_range(5, 0, 100_000);
        assert!((0..=100_000).contains(&wide));
    }
}
