//! Small RNG for audio choices. It is deliberately not the simulation RNG: the original draws
//! sound variants from the CRT `rand`, which never affects the simulation (`docs/spec/trains.md`
//! section 5.4), so the clone keeps the audio stream separate and the sim never sees it.

/// xorshift64* generator; seedable for tests, seeded from the wall clock in the app.
#[derive(Debug, Clone)]
pub struct AudioRng(u64);

impl AudioRng {
    pub fn new(seed: u64) -> Self {
        // Zero is the one fixed point of xorshift.
        Self(seed ^ 0x9e37_79b9_7f4a_7c15 | 1)
    }

    /// Seed from the system clock (audio only; never use this in the simulation).
    pub fn from_clock() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x1234_5678, |d| d.as_nanos() as u64);
        Self::new(nanos)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform integer in `lo..=hi` (the shape of the original `crt_rand_range`).
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        debug_assert!(lo <= hi);
        lo + (self.next_u64() >> 33) as u32 % (hi - lo + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let mut a = AudioRng::new(7);
        let mut b = AudioRng::new(7);
        for _ in 0..1000 {
            let x = a.range(2, 5);
            assert_eq!(x, b.range(2, 5));
            assert!((2..=5).contains(&x));
        }
        assert_eq!(AudioRng::new(0).range(3, 3), 3);
    }

    #[test]
    fn covers_the_range() {
        let mut r = AudioRng::new(1);
        let mut seen = [false; 4];
        for _ in 0..200 {
            seen[r.range(0, 3) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s));
    }
}
