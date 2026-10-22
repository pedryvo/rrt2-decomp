//! The New Scenario options (manual chapter 3, "New Scenario"; `setup.win` of the original):
//! difficulty level presets, the financial and industrial models, sandbox mode, the two revenue
//! modifiers and the number of computer players.
//!
//! TODO(engine): the simulation has no setup hooks yet, so these choices are shown and kept but
//! not applied to the world (`docs/spec/finance.md` 13.9 lists where the models act:
//! `0x573d72..0x573d77` flags). The difficulty rating formula is not decoded either; the number
//! shown is a heuristic calibrated on the manual's screenshots (Easy 6 %, Expert models 100 %).

/// The skill level horses of the Game Setup box (LNG 1737..1745).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Easy,
    Medium,
    Hard,
    Expert,
    Custom,
}

impl Level {
    pub const ALL: [Level; 5] = [
        Level::Easy,
        Level::Medium,
        Level::Hard,
        Level::Expert,
        Level::Custom,
    ];
}

/// Financial / industrial model (manual: Basic, Advanced, Expert).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Model {
    Basic,
    Advanced,
    Expert,
}

impl Model {
    pub const ALL: [Model; 3] = [Model::Basic, Model::Advanced, Model::Expert];

    pub fn index(self) -> i32 {
        self as i32
    }
}

/// Range of the revenue sliders in percent. TODO: the original's range is not decoded; the
/// presets use +-20 %.
pub const REVENUE_RANGE: (i32, i32) = (-50, 50);
pub const REVENUE_STEP: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setup {
    pub level: Level,
    pub sandbox: bool,
    pub financial: Model,
    pub industrial: Model,
    /// Revenue modifier of the human player and of the computer players, in percent.
    pub human_revenue: i32,
    pub computer_revenue: i32,
    /// Computer players on top of the ones the scenario already has.
    pub extra_computers: usize,
}

impl Setup {
    /// The preset of a level (manual, "Skill Levels"). `Custom` starts from the Expert models
    /// with neutral modifiers, like the manual's screenshot.
    pub fn preset(level: Level) -> Setup {
        let (fin, ind, human, computer) = match level {
            Level::Easy => (Model::Basic, Model::Basic, 20, 10),
            Level::Medium => (Model::Advanced, Model::Advanced, 0, 0),
            Level::Hard => (Model::Expert, Model::Expert, -10, 10),
            Level::Expert => (Model::Expert, Model::Expert, -20, 20),
            Level::Custom => (Model::Expert, Model::Expert, 0, 0),
        };
        Setup {
            level,
            sandbox: false,
            financial: fin,
            industrial: ind,
            human_revenue: human,
            computer_revenue: computer,
            extra_computers: 0,
        }
    }

    /// Chooses a level: a preset replaces the models and modifiers (not the computer players
    /// or sandbox mode); `Custom` only changes the label.
    pub fn set_level(&mut self, level: Level) {
        if level == Level::Custom {
            self.level = level;
            return;
        }
        let p = Setup::preset(level);
        self.level = level;
        self.financial = p.financial;
        self.industrial = p.industrial;
        self.human_revenue = p.human_revenue;
        self.computer_revenue = p.computer_revenue;
    }

    /// After any option change the level label follows the settings: a preset when they equal
    /// one, else `Custom` (manual: "Changes you make outside these skill settings fall under the
    /// Custom skill level").
    pub fn relabel(&mut self) {
        self.level = Level::ALL[..4]
            .iter()
            .copied()
            .find(|&l| {
                let p = Setup::preset(l);
                p.financial == self.financial
                    && p.industrial == self.industrial
                    && p.human_revenue == self.human_revenue
                    && p.computer_revenue == self.computer_revenue
            })
            .unwrap_or(Level::Custom);
    }

    pub fn set_financial(&mut self, m: Model) {
        self.financial = m;
        self.relabel();
    }

    pub fn set_industrial(&mut self, m: Model) {
        self.industrial = m;
        self.relabel();
    }

    pub fn step_human(&mut self, dir: i32) {
        self.human_revenue =
            (self.human_revenue + dir * REVENUE_STEP).clamp(REVENUE_RANGE.0, REVENUE_RANGE.1);
        self.relabel();
    }

    pub fn step_computer(&mut self, dir: i32) {
        self.computer_revenue =
            (self.computer_revenue + dir * REVENUE_STEP).clamp(REVENUE_RANGE.0, REVENUE_RANGE.1);
        self.relabel();
    }

    pub fn set_sandbox(&mut self, on: bool) {
        self.sandbox = on;
    }

    /// The "Difficulty Rating: N %" (LNG 186). Heuristic, see the module docs: the models carry
    /// the weight, the human revenue modifier and the number of computer players move it. In
    /// sandbox mode "money and competition are non-factors": 0.
    pub fn rating(&self, scenario_computers: usize) -> u32 {
        if self.sandbox {
            return 0;
        }
        let models = f64::from(self.financial.index() + self.industrial.index()) / 4.0;
        let computers = (scenario_computers + self.extra_computers) as f64;
        let r = 15.0 + 85.0 * models - 0.5 * f64::from(self.human_revenue)
            + 0.1 * f64::from(self.computer_revenue)
            + 4.0 * computers;
        r.round().clamp(0.0, 100.0) as u32
    }
}

impl Default for Setup {
    fn default() -> Self {
        Setup::preset(Level::Easy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_follow_the_manual() {
        let e = Setup::preset(Level::Easy);
        assert_eq!(
            (
                e.financial,
                e.industrial,
                e.human_revenue,
                e.computer_revenue
            ),
            (Model::Basic, Model::Basic, 20, 10)
        );
        let m = Setup::preset(Level::Medium);
        assert_eq!(
            (m.financial, m.human_revenue, m.computer_revenue),
            (Model::Advanced, 0, 0)
        );
        let h = Setup::preset(Level::Hard);
        assert_eq!(
            (h.financial, h.human_revenue, h.computer_revenue),
            (Model::Expert, -10, 10)
        );
        let x = Setup::preset(Level::Expert);
        assert_eq!((x.human_revenue, x.computer_revenue), (-20, 20));
    }

    #[test]
    fn changing_an_option_falls_back_to_custom_and_back() {
        let mut s = Setup::default();
        assert_eq!(s.level, Level::Easy);
        s.set_financial(Model::Expert);
        assert_eq!(s.level, Level::Custom);
        s.set_financial(Model::Basic);
        assert_eq!(s.level, Level::Easy);
        s.step_human(1);
        assert_eq!(s.level, Level::Custom);
        s.set_level(Level::Hard);
        assert_eq!(s.level, Level::Hard);
        assert_eq!((s.financial, s.human_revenue), (Model::Expert, -10));
    }

    #[test]
    fn revenue_modifiers_are_clamped() {
        let mut s = Setup::default();
        for _ in 0..40 {
            s.step_human(1);
            s.step_computer(-1);
        }
        assert_eq!(s.human_revenue, REVENUE_RANGE.1);
        assert_eq!(s.computer_revenue, REVENUE_RANGE.0);
    }

    #[test]
    fn rating_matches_the_manual_screenshots_and_grows_with_difficulty() {
        // C303: Easy, one player: 6 %. C305: Custom with the Expert models: 100 %.
        assert_eq!(Setup::preset(Level::Easy).rating(0), 6);
        assert_eq!(Setup::preset(Level::Custom).rating(0), 100);
        let mut last = 0;
        for l in [Level::Easy, Level::Medium, Level::Hard] {
            let r = Setup::preset(l).rating(0);
            assert!(r >= last);
            last = r;
        }
        let mut s = Setup::preset(Level::Medium);
        let base = s.rating(1);
        s.extra_computers = 2;
        assert!(s.rating(1) > base, "more computer players: harder");
        s.sandbox = true;
        assert_eq!(s.rating(1), 0);
    }
}
