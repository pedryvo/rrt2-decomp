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

/// `x ^ y` as the original computes it (MSVC CRT `_CIpow`, `0x51bab0`), in pure integer
/// arithmetic: no libm, so the result is identical on every platform.
///
/// The CRT routine (docs/spec/trains.md section 8) is, for finite `x > 0` and finite `y`:
///
/// ```text
/// L = fyl2x(x, y)                     ; y * log2(x), rounded to the 64-bit x87 mantissa
/// r = frndint(L)                      ; round half to even
/// f = L - r                           ; sub at 53-bit precision (control word 0x27f)
/// s = 1 + f2xm1(f)                    ; add at 53-bit precision
/// pow = fscale(s, r)                  ; s * 2^r, exact
/// ```
///
/// `fyl2x` and `f2xm1` are evaluated here to more than 100 bits and rounded once to the 64-bit
/// mantissa (the ideal result of the instruction); the two 53-bit roundings are modelled
/// exactly. Hardware transcendentals are only guaranteed to be within 1 ulp of the 64-bit result,
/// so on a given CPU the result can differ from this model when the exact value lies within about
/// 2^-11 of a 53-bit rounding boundary; the measured effect is in `docs/spec/trains.md`
/// section 8 and the differential tests (`crates/rt2-oracle/tests/pow.rs`). A mismatch in the
/// last bit of the 53-bit result almost never survives the `ftol` the game applies afterwards.
///
/// Domain: finite `x > 0` and finite `y` (the game only uses `0.98^y` and `(20g+1)^2.3`). Outside
/// it (NaN, infinities, `x <= 0`) this returns NaN, except `y == 0` and `x == 1` which give 1.
/// Results below the normal `f64` range are not correctly rounded.
pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 || x == 1.0 {
        return 1.0;
    }
    if !x.is_finite() || !y.is_finite() || x <= 0.0 {
        return f64::NAN;
    }
    let (xm, xe) = split_f64(x);
    // x = xm * 2^xe with xm in [2^52, 2^53).
    let shift = xm.leading_zeros() as i32 - 11;
    let (xm, xe) = (xm << shift, xe - shift);
    let (ym, ye) = split_f64(y.abs());
    let e = xe + 52;
    // log2(x) = e + log2(xm / 2^52), as a signed Q110 fixed-point number.
    let l = (i128::from(e) << 110) + log2_frac_q110(xm) as i128;
    let neg = (l < 0) != (y < 0.0);
    // V = y * L, exact (|l| < 2^121, ym < 2^53), then rounded to the 64-bit mantissa (fyl2x).
    let (hi, lo) = mul128(l.unsigned_abs(), u128::from(ym));
    let (v, s) = round_bits(hi, lo, 64);
    let ve = s as i32 + ye - 110; // V = v * 2^ve
    let v_bits = 128 - v.leading_zeros() as i32;
    if v_bits + ve > 12 {
        return if neg { 0.0 } else { f64::INFINITY };
    }
    // frndint (half to even) and the 53-bit subtraction L - r.
    let k = -ve;
    let (int, dm, d_neg) = if k <= 0 {
        (v << (-k) as u32, 0u128, false)
    } else if k >= 128 {
        (0, v, false)
    } else {
        let (ip, rem) = (v >> k, v & ((1u128 << k) - 1));
        let half = 1u128 << (k - 1);
        if rem > half || (rem == half && ip & 1 == 1) {
            (ip + 1, (1u128 << k) - rem, true)
        } else {
            (ip, rem, false)
        }
    };
    // `as f64` rounds to nearest even: the 53-bit precision of the subtraction.
    let mut f = ldexp(dm as f64, ve);
    if d_neg {
        f = -f;
    }
    let mut r = int as i32;
    if neg {
        f = -f;
        r = -r;
    }
    // f2xm1: 2^f - 1 with |f| <= 0.5, Taylor series in Q120, then rounded to 64 bits.
    let fq = (f * (1u128 << 120) as f64) as i128;
    let z = qmul(fq, LN2_Q120);
    let (mut term, mut sum) = (z, z);
    for n in 2..48 {
        term = qmul(term, z) / n;
        if term == 0 {
            break;
        }
        sum += term;
    }
    let (m, s) = round_bits(0, sum.unsigned_abs(), 64);
    let t = (m << s) as i128 * if sum < 0 { -1 } else { 1 };
    // 1 + t at 53-bit precision, then fscale.
    let (m, s) = round_bits(0, ((1i128 << 120) + t) as u128, 53);
    ldexp(m as f64, s as i32 - 120 + r)
}

/// `ln 2` in Q120.
const LN2_Q120: i128 = 0xb17217f7d1cf79abc9e3b39803f2f6;

/// `x = m * 2^e` for finite `x >= 0` (`m` not normalised).
fn split_f64(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1 << 52) - 1);
    if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1 << 52), exp - 1075)
    }
}

/// `x * 2^n` for a few thousand at most, with a single rounding only in the subnormal range.
fn ldexp(mut x: f64, mut n: i32) -> f64 {
    let p = |k: i32| f64::from_bits(((k + 1023) as u64) << 52);
    n = n.clamp(-2300, 2300);
    while n > 1000 {
        x *= p(1000);
        n -= 1000;
    }
    while n < -1000 {
        x *= p(-1000);
        n += 1000;
    }
    x * p(n)
}

/// Full 128 x 128 -> 256 bit product as `(high, low)`.
fn mul128(a: u128, b: u128) -> (u128, u128) {
    const M: u128 = u64::MAX as u128;
    let (a1, a0, b1, b0) = (a >> 64, a & M, b >> 64, b & M);
    let (p00, p01, p10, p11) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
    let mid = (p00 >> 64) + (p01 & M) + (p10 & M);
    let lo = (p00 & M) | (mid << 64);
    let hi = p11 + (p01 >> 64) + (p10 >> 64) + (mid >> 64);
    (hi, lo)
}

/// Signed Q120 product, truncated toward zero (operands below 2^120 in magnitude).
fn qmul(a: i128, b: i128) -> i128 {
    let (hi, lo) = mul128(a.unsigned_abs(), b.unsigned_abs());
    let m = ((hi << 8) | (lo >> 120)) as i128;
    if (a < 0) != (b < 0) { -m } else { m }
}

/// Fractional part of `log2(m / 2^52)` for `m` in `[2^52, 2^53)`, 110 bits, by repeated
/// squaring (each squaring yields one bit; rounding errors are damped by 2^-j).
fn log2_frac_q110(m: u64) -> u128 {
    let mut v = u128::from(m) << 74; // Q126, value in [1, 2)
    let mut acc = 0u128;
    for _ in 0..110 {
        let (hi, lo) = mul128(v, v);
        let sq = (hi << 2) | (lo >> 126); // v^2 in Q126, value in [1, 4)
        acc <<= 1;
        if sq >> 127 != 0 {
            acc |= 1;
            v = sq >> 1;
        } else {
            v = sq;
        }
    }
    acc
}

/// Rounds the 256-bit magnitude `(hi, lo)` to `bits` significant bits, half to even. Returns
/// `(m, s)` with `value ~ m * 2^s`.
fn round_bits(hi: u128, lo: u128, bits: u32) -> (u128, u32) {
    let n = if hi != 0 {
        256 - hi.leading_zeros()
    } else {
        128 - lo.leading_zeros()
    };
    if n <= bits {
        return (lo, 0);
    }
    let s = n - bits;
    let shr = |k: u32| -> u128 {
        match k {
            0 => lo,
            1..=127 => (lo >> k) | (hi << (128 - k)),
            128 => hi,
            _ => hi >> (k - 128),
        }
    };
    let m = shr(s);
    let bit = |i: u32| -> bool {
        if i >= 128 {
            hi >> (i - 128) & 1 == 1
        } else {
            lo >> i & 1 == 1
        }
    };
    let sticky = if s <= 1 {
        false
    } else if s > 128 {
        lo != 0 || hi & ((1u128 << (s - 1 - 128)) - 1) != 0
    } else {
        lo & ((1u128 << (s - 1)) - 1) != 0
    };
    if bit(s - 1) && (sticky || m & 1 == 1) {
        let m = m + 1;
        if m == 1u128 << bits {
            return (m >> 1, s + 1);
        }
        return (m, s);
    }
    (m, s)
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

    #[test]
    fn pow_exact_cases() {
        assert_eq!(pow(2.0, 10.0), 1024.0);
        assert_eq!(pow(2.0, -3.0), 0.125);
        assert_eq!(pow(4.0, 0.5), 2.0);
        assert_eq!(pow(7.0, 0.0), 1.0);
        assert_eq!(pow(1.0, 123.5), 1.0);
        assert_eq!(pow(10.0, 2.0), 100.0);
        assert!(pow(-2.0, 2.0).is_nan());
    }

    #[test]
    fn pow_close_to_libm() {
        // Not a determinism check (libm differs by platform): a sanity bound of 2 ulp.
        let mut s = 0x1234_5678u64;
        let mut rnd = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..20_000 {
            let x = 0.01 + rnd() * 50.0;
            let y = (rnd() - 0.5) * 40.0;
            let (a, b) = (pow(x, y), x.powf(y));
            let ulp = (a.to_bits() as i64 - b.to_bits() as i64).abs();
            assert!(ulp <= 2, "pow({x}, {y}) = {a} vs {b}");
        }
    }
}
