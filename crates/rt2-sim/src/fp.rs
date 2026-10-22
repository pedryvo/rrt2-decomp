//! Floating-point conventions of the original (docs/spec/time.md section 3, cargo.md 4.7).
//!
//! The original computes in the x87 at **53-bit precision** (control word `0x27f`, measured in
//! the running game, `docs/dynamic-analysis.md`) and truncates with the CRT `_ftol`. Rust `f64`
//! arithmetic (SSE2) is bit-identical to that for `+ - * /` and `sqrt` as long as the operation
//! order is kept and every place where the original stores a `float` is rounded with [`f32r`].

/// CRT `_ftol`: truncate toward zero, keep the low 32 bits of the 64-bit result.
#[inline]
pub fn ftol(x: f64) -> i32 {
    // `as i64` saturates and maps NaN to 0, which is enough: the game never feeds it those.
    (x.trunc() as i64) as i32
}

/// Rounds to `f32` and widens again: models a store to a `float` and a reload.
#[inline]
pub fn f32r(x: f64) -> f64 {
    f64::from(x as f32)
}

/// `x ^ y` as the original computes it (CRT `pow`).
///
/// TODO(trains.md "Em aberto"/section 8): this is `f64::powf`, i.e. the platform libm. It matched
/// the original bit for bit in 30 000 oracle cases on this machine, but libm is not guaranteed
/// to be identical across platforms; replace with a software implementation before relying on
/// cross-platform replays.
#[inline]
pub fn pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftol_truncates_toward_zero() {
        assert_eq!(ftol(2.9), 2);
        assert_eq!(ftol(-2.9), -2);
        assert_eq!(ftol(0.0), 0);
        // cargo.md section 3: raw * 1.4 lands one ulp below the integer, so the price is 58799.
        assert_eq!(ftol(42000.0 * 1.4), 58799);
        assert_eq!(ftol(46000.0 * 1.4), 64399);
        assert_eq!(ftol(43000.0 * 1.4), 60199);
        assert_eq!(ftol(56000.0 * 1.4), 78400);
    }

    #[test]
    fn f32r_rounds_to_single() {
        assert_eq!(f32r(0.1), f64::from(0.1f32));
    }
}
