//! The New Scenario options (manual chapter 3, "New Scenario"; `setup.win` of the original):
//! difficulty level presets, the financial and industrial models, sandbox mode, the two revenue
//! modifiers and the number of computer players, and how they reach `World::new_game`
//! ([`Setup::new_game_options`]).
//!
//! Applied to the world: the financial and industrial models (option byte `+0x32` of the
//! `0x574840` block, `docs/spec/finance.md` 13.9 and `world-tick.md` 10: bits 3..5 financial,
//! 0 / 8 / 0x10, bits 0..2 industrial, 0 / 1 / 2) and the game seed (`+0x2e`); a campaign start
//! sets the campaign block (`+0x16` index, `+0x1a` running, `+0x3d` set) and the fixed option
//! bits `(opt & 0xd2) | 0x12` of `0x410de0`.
//!
//! TODO(sim): kept in [`Setup`] but not applied yet, because `rt2-sim` has no hook for them:
//! sandbox mode, the two revenue modifiers (`cargo.md` 8: bytes `0x574874`, lo human / hi
//! computer, 0..10, default 5 and 5; the UI range here is a percentage), the extra computer
//! players (slot assembly of `0x4b45b0` is not ported; the map's own slots are used) and the
//! campaign bonus choice. The difficulty rating formula is not decoded either; the number shown
//! is a heuristic calibrated on the manual's screenshots (Easy 6 %, Expert models 100 %).

use rt2_formats::map::Save;
use rt2_sim::NewGameOptions;

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

/// Which scenario of which campaign a campaign start plays (`scenarios.md` 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignStart {
    /// 0 the classic campaign, 1 "The Second Century".
    pub set: usize,
    /// Scenario index 0..18 within the set.
    pub index: usize,
    /// Difficulty 0..=2 (`CampaignDifficulty1To3 = difficulty + 1`).
    pub difficulty: usize,
    /// Bonus choice 0..=2 (`CampaignChoice1To3 = choice + 1`).
    pub bonus: usize,
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    if let Some(d) = b.get_mut(off..off + 4) {
        d.copy_from_slice(&v.to_le_bytes());
    }
}

impl Setup {
    /// The options `World::new_game` takes (`0x574840` block, campaign block `0x566d00`): the
    /// map's own blocks with the setup screen's choices written over them. `seed` becomes the
    /// game seed and the CRT state of the start (the original uses the clock, `world-tick.md`
    /// 10), so the caller picks it: fixed for tests and screenshots, the wall clock when playing.
    pub fn new_game_options(
        &self,
        map: &Save,
        name: &str,
        campaign: Option<&CampaignStart>,
        seed: u32,
    ) -> NewGameOptions {
        let mut o = NewGameOptions::from_save(map, name, campaign.is_some(), seed);
        if o.trailer.len() >= 0x36 {
            o.trailer[0x2e..0x32].copy_from_slice(&seed.to_le_bytes());
            // `+0x22`: the start year. The map file's own block holds a stale value (B&O: 1880);
            // the setup (`0x4b3ac0`, `world-tick.md` 10) fills in the scenario's start year, the
            // `Date` of its preview. TODO: the year choice of maps that allow one.
            o.trailer[0x22..0x24].copy_from_slice(&map.date.year.to_le_bytes());
            let opt = o.trailer[0x32];
            o.trailer[0x32] = if campaign.is_some() {
                // `0x410de0`: `options & 0xd2 | 0x12` whatever the difficulty.
                (opt & 0xd2) | 0x12
            } else {
                (opt & 0xc0) | ((self.financial.index() as u8) << 3) | self.industrial.index() as u8
            };
        }
        if let Some(c) = campaign {
            // `world-tick.md` 10: `0x566d00 + 0` difficulty, `+0x16` index, `+0x1a` running,
            // `+0x2a` choice, `+0x3d` set. The widths are inferred from the spacing of the
            // addresses.
            put_u32(&mut o.block86, 0, c.difficulty as u32);
            put_u32(&mut o.block86, 0x16, c.index as u32);
            put_u32(&mut o.block86, 0x1a, 1);
            put_u32(&mut o.block86, 0x2a, c.bonus as u32);
            if let Some(b) = o.block86.get_mut(0x3d) {
                *b = c.set as u8;
            }
        }
        o
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
