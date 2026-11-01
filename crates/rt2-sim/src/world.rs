//! Minimal simulation state (docs/PLAN.md sections 3 and 10).
//!
//! Only what is already specified lives here: the clock, the game RNG and the game speed. The
//! entity lists (trains, stations, industries...) are TODO until their record layouts are fixed
//! by the specs (time.md "Em aberto", third item).

use crate::calendar::{DEFAULT_SPEED, Date, MAX_SPEED, SpeedTable};
use crate::rng::{CrtRng, GameRng};

/// The whole deterministic state. All mutation goes through [`World::apply`] (player/network
/// intent) and [`World::tick`] (time), so a command log plus a seed replays a game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    /// Entity pools and bookkeeping when built from a save ([`World::from_save`]).
    pub(crate) sim: Option<Box<crate::tick::SimState>>,
    /// `game_time_minutes` (`0x5704f4`): always equal to `date.to_minutes()`.
    pub(crate) minutes: i32,
    /// Game RNG (generator A, state `0x55d2c0`).
    pub(crate) rng: GameRng,
    /// The MSVC CRT `rand` (generator C, state `0x5609f0`): sound, UI and the saved smoke puffs.
    pub(crate) crt: CrtRng,
    /// `game_seed` (`0x57486e`): independent of the RNG state (rng.md 2.4).
    pub(crate) game_seed: u32,
    /// `game_speed` (`0x57050e`), 0..=10, 0 = pause.
    pub(crate) speed: u8,
    /// `game_speed_resume` (`0x5707c2`): last non-zero speed.
    pub(crate) speed_resume: u8,
    /// Open merger and chairmanship vote records (`0x574ca0`, `0x574ca4`); not part of a save.
    pub(crate) votes: crate::fincmd::Votes,
    /// Answers to scenario choices (`crate::choice`); not part of a save.
    pub(crate) choices: crate::choice::Choices,
    /// Last build snapshot for single-player undo (PLAN.md section 12).
    pub(crate) last_build_snapshot: Option<UndoBuildSnapshot>,
    /// Network session (`None` = single player, the rules of the fidelity scoreboard); see
    /// [`crate::net`].
    pub(crate) net: Option<crate::net::NetSession>,
}

/// Lightweight snapshot of state before a local player construction command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UndoBuildSnapshot {
    pub(crate) sim: Box<crate::tick::SimState>,
    pub(crate) rng: GameRng,
    pub(crate) crt: CrtRng,
}

impl World {
    /// A new game at `start` (speed 3 and resume 3, `0x472980`). The two seeds are independent
    /// (rng.md 2.4); the caller chooses them, so the core never reads a clock.
    pub fn new(start: Date, game_seed: u32, rng_seed: u32) -> World {
        World {
            sim: None,
            minutes: start.to_minutes(),
            rng: GameRng::new(rng_seed),
            crt: CrtRng::new(rng_seed),
            game_seed,
            speed: DEFAULT_SPEED,
            speed_resume: DEFAULT_SPEED,
            votes: crate::fincmd::Votes::default(),
            choices: crate::choice::Choices::default(),
            last_build_snapshot: None,
            net: None,
        }
    }

    pub fn minutes(&self) -> i32 {
        self.minutes
    }

    pub fn date(&self) -> Date {
        Date::from_minutes(self.minutes)
    }

    /// Estimated breakdown probability at top speed for a train (`0x4d4790` calling `0x4cf030`
    /// for `traindtl.win`). Returns `0.0` if the world has no save pools or if `train` is invalid.
    pub fn train_breakdown_chance(&self, train: i32) -> f32 {
        self.sim
            .as_ref()
            .map(|s| s.train_breakdown_chance(train, Date::from_minutes(self.minutes).year))
            .unwrap_or(0.0)
    }

    pub fn speed(&self) -> u8 {
        self.speed
    }

    pub fn speed_resume(&self) -> u8 {
        self.speed_resume
    }

    pub fn game_seed(&self) -> u32 {
        self.game_seed
    }

    pub fn rng(&self) -> &GameRng {
        &self.rng
    }

    pub fn crt_rng(&self) -> &CrtRng {
        &self.crt
    }

    /// Mutable RNG for simulation systems; the order of draws is part of the state.
    pub fn rng_mut(&mut self) -> &mut GameRng {
        &mut self.rng
    }

    /// Advances the clock by one nominal frame ([`crate::calendar::NOMINAL_FRAME_MS`]) at the
    /// current speed and returns the game minutes elapsed (0 when paused).
    ///
    /// TODO(time.md section 6, "Em aberto"): `sim_advance` also runs the day/week/month/year
    /// end steps (23:59 transitions) and moves entities; those need the entity lists, whose
    /// roles and record layouts are still open. Only the clock moves here.
    pub fn tick(&mut self, speeds: &SpeedTable) -> i32 {
        let dt = speeds.minutes_per_frame(self.speed);
        self.advance_minutes(dt);
        dt
    }

    /// Advances the clock by `dt` game minutes (see the TODO on [`World::tick`]).
    pub fn advance_minutes(&mut self, dt: i32) {
        if dt > 0 {
            self.last_build_snapshot = None;
        }
        self.minutes = self.minutes.wrapping_add(dt);
    }

    /// Undoes the last local construction command (TrackBuild, TrackBulldoze, StationBuild,
    /// StationImprove, StationSizeChange, DoubleTrack, ElectricTrack), restoring the state
    /// before the command.
    ///
    /// Disabled in multiplayer games (`self.net.is_some()`). Returns `true` if an undo was performed,
    /// or `false` if there was no recent build to undo or undo is disabled.
    pub fn undo_last_build(&mut self) -> bool {
        if self.net.is_some() {
            return false;
        }
        if let Some(saved) = self.last_build_snapshot.take() {
            self.sim = Some(saved.sim);
            self.rng = saved.rng;
            self.crt = saved.crt;
            true
        } else {
            false
        }
    }

    /// Whether there is a recent build that can be undone.
    pub fn can_undo_last_build(&self) -> bool {
        self.net.is_none() && self.last_build_snapshot.is_some()
    }

    /// The enabled fixes of original bugs (faithful for a world without save pools).
    pub fn ruleset(&self) -> crate::ruleset::Ruleset {
        self.sim
            .as_ref()
            .map_or(crate::ruleset::Ruleset::FAITHFUL, |s| s.rules)
    }

    /// Sets the fixes of original bugs. The rules only exist for the simulation, so a world
    /// without save pools ignores it.
    pub fn set_ruleset(&mut self, rules: crate::ruleset::Ruleset) {
        if let Some(s) = self.sim.as_mut() {
            s.rules = rules;
        }
    }

    pub(crate) fn change_speed(&mut self, new: u8) {
        let new = new.min(MAX_SPEED);
        if self.speed != 0 {
            self.speed_resume = self.speed;
        }
        self.speed = new;
    }
}
