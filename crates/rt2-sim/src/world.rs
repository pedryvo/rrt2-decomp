//! Minimal simulation state (docs/PLAN.md sections 3 and 10).
//!
//! Only what is already specified lives here: the clock, the game RNG and the game speed. The
//! entity lists (trains, stations, industries...) are TODO until their record layouts are fixed
//! by the specs (time.md "Em aberto", third item).

use crate::calendar::{DEFAULT_SPEED, Date, MAX_SPEED, SpeedTable};
use crate::rng::GameRng;

/// The whole deterministic state. All mutation goes through [`World::apply`] (player/network
/// intent) and [`World::tick`] (time), so a command log plus a seed replays a game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    /// `game_time_minutes` (`0x5704f4`): always equal to `date.to_minutes()`.
    pub(crate) minutes: i32,
    /// Game RNG (generator A, state `0x55d2c0`).
    pub(crate) rng: GameRng,
    /// `game_seed` (`0x57486e`): independent of the RNG state (rng.md 2.4).
    pub(crate) game_seed: u32,
    /// `game_speed` (`0x57050e`), 0..=10, 0 = pause.
    pub(crate) speed: u8,
    /// `game_speed_resume` (`0x5707c2`): last non-zero speed.
    pub(crate) speed_resume: u8,
}

impl World {
    /// A new game at `start` (speed 3 and resume 3, `0x472980`). The two seeds are independent
    /// (rng.md 2.4); the caller chooses them, so the core never reads a clock.
    pub fn new(start: Date, game_seed: u32, rng_seed: u32) -> World {
        World {
            minutes: start.to_minutes(),
            rng: GameRng::new(rng_seed),
            game_seed,
            speed: DEFAULT_SPEED,
            speed_resume: DEFAULT_SPEED,
        }
    }

    pub fn minutes(&self) -> i32 {
        self.minutes
    }

    pub fn date(&self) -> Date {
        Date::from_minutes(self.minutes)
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
        self.minutes = self.minutes.wrapping_add(dt);
    }

    pub(crate) fn change_speed(&mut self, new: u8) {
        let new = new.min(MAX_SPEED);
        if self.speed != 0 {
            self.speed_resume = self.speed;
        }
        self.speed = new;
    }
}
