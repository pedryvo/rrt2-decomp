//! Manager, event and territory effects (docs/spec/cargo.md section 5, `effect_multiplier`
//! `0x46b480`, proven bit-exact).

use crate::fp::f32r;

/// `f(n) = max(n, -99) * 0.01 + 1.0`, `n` an `i16` percentage.
pub fn percent_factor(n: i16) -> f64 {
    f64::from(n.max(-99)) * 0.01 + 1.0
}

/// The effect percentages that apply to one effect id, at each level. A `None` means "that
/// level does not take part" (id 0, or record not found), exactly as in the original.
///
/// The caller resolves the records: the person level uses the explicit person if given,
/// otherwise the company's person (`company +0x23`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectLevels {
    /// `0x5707c6 + 2*e`: scenario/event percentage (always applies).
    pub global: i16,
    /// `company +0x1ae + 2*e`.
    pub company: Option<i16>,
    /// `person +0x26b + 2*e`.
    pub person: Option<i16>,
    /// `territory +0x58 + 2*e`.
    pub territory: Option<i16>,
}

/// `effect_multiplier(e, ...)`: product of the level factors, each stored as `f32`, floor 0.01.
pub fn effect_multiplier(l: EffectLevels) -> f32 {
    let mut v = percent_factor(l.global) as f32;
    for n in [l.company, l.person, l.territory].into_iter().flatten() {
        v = (percent_factor(n) * f64::from(v)) as f32;
    }
    if f64::from(v) < 0.01 { 0.01f32 } else { v }
}

/// Global-only multiplier as an `f64` (what the train code reads for company 0, person 0):
/// `f32(max(pct,-99)*0.01 + 1)` with the 0.01 floor.
pub fn global_modifier(pct: i16) -> f64 {
    let m = f32r(percent_factor(pct));
    if m < 0.01 { f64::from(0.01f32) } else { m }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_one() {
        assert_eq!(effect_multiplier(EffectLevels::default()), 1.0);
    }

    #[test]
    fn levels_multiply() {
        let m = effect_multiplier(EffectLevels {
            global: 10,
            company: Some(10),
            person: None,
            territory: Some(-50),
        });
        let want = ((1.1f64 * 1.1f64) as f32 as f64 * 0.5) as f32;
        // 1.0 + 10 * 0.01 is not exactly 1.1; compare with the same operation order
        let g = percent_factor(10) as f32;
        let c = (percent_factor(10) * f64::from(g)) as f32;
        let t = (percent_factor(-50) * f64::from(c)) as f32;
        assert_eq!(m, t);
        assert!((m - want).abs() < 1e-6);
    }

    #[test]
    fn floor_and_clamp() {
        // -99 is the lowest percentage that counts: 0.01 exactly
        assert_eq!(percent_factor(-5000), percent_factor(-99));
        assert_eq!(
            effect_multiplier(EffectLevels {
                global: -99,
                company: Some(-99),
                ..Default::default()
            }),
            0.01f32
        );
        assert_eq!(global_modifier(-99), f64::from(0.01f32));
    }
}
