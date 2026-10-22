//! UI scale: logical units x factor = physical pixels (docs/ui.md "Escala").
//!
//! The factor is a multiple of 25 % between 100 % and 300 %. The default comes from the monitor's
//! DPI scale factor reported by the windowing system; the player can override it (config, keys).

pub const MIN_PERCENT: u32 = 100;
pub const MAX_PERCENT: u32 = 300;
pub const STEP_PERCENT: u32 = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiScale(u32);

impl UiScale {
    /// From a percentage, clamped and snapped to the step.
    pub fn from_percent(percent: u32) -> UiScale {
        let snapped = (percent + STEP_PERCENT / 2) / STEP_PERCENT * STEP_PERCENT;
        UiScale(snapped.clamp(MIN_PERCENT, MAX_PERCENT))
    }

    /// The default for a monitor: its DPI scale factor (1.0 = 96 dpi), snapped.
    pub fn from_dpi(scale_factor: f64) -> UiScale {
        UiScale::from_percent((scale_factor * 100.0).round().max(0.0) as u32)
    }

    pub fn percent(self) -> u32 {
        self.0
    }

    pub fn factor(self) -> f32 {
        self.0 as f32 / 100.0
    }

    pub fn step(self, steps: i32) -> UiScale {
        let p = self.0 as i32 + steps * STEP_PERCENT as i32;
        UiScale::from_percent(p.max(0) as u32)
    }

    /// The size of the logical canvas for a physical viewport.
    pub fn logical_size(self, physical: (u32, u32)) -> (f32, f32) {
        let k = self.factor();
        (physical.0 as f32 / k, physical.1 as f32 / k)
    }
}

impl Default for UiScale {
    fn default() -> Self {
        UiScale(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_is_snapped_and_clamped() {
        assert_eq!(UiScale::from_dpi(1.0).percent(), 100);
        assert_eq!(UiScale::from_dpi(1.33).percent(), 125);
        assert_eq!(UiScale::from_dpi(2.0).percent(), 200);
        assert_eq!(UiScale::from_dpi(0.5).percent(), 100);
        assert_eq!(UiScale::from_dpi(5.0).percent(), 300);
        assert_eq!(UiScale::from_percent(180).percent(), 175);
    }

    #[test]
    fn step_and_logical_size() {
        assert_eq!(UiScale::from_percent(200).step(1).percent(), 225);
        assert_eq!(UiScale::from_percent(100).step(-3).percent(), 100);
        assert_eq!(
            UiScale::from_percent(200).logical_size((3840, 2160)),
            (1920.0, 1080.0)
        );
    }
}
