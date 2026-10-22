//! The command layer: every state change caused by a player, the network or a script is an
//! [`Command`] applied with [`World::apply`] (docs/PLAN.md section 10, OpenRCT2-style).
//!
//! This is the future seam for replays, undo and multiplayer: a game is `(seed, commands per
//! tick)`. In the original, commands are queued by the UI and run by `cmd_queue_run`/`cmd_execute`
//! (`0x4af3d0`/`0x4b0210`, time.md 5.1).

use crate::calendar::MAX_SPEED;
use crate::world::World;

/// Change to the game speed (`cmd_execute` case `0x4b0d91`, time.md section 3): the byte `+0xc`
/// is `0x7f` for an absolute value, otherwise a signed delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedChange {
    Absolute(u8),
    Delta(i8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Set or step the game speed; the result is clamped to `[0, 10]`. If the old speed was not 0
    /// it is remembered as the resume speed.
    SetSpeed(SpeedChange),
    /// Pause, or resume at the last non-zero speed (the "pause" key goes through this).
    TogglePause,
    /// `rng_seed(arg)` (`cmd_execute`, `0x4b02a4`): the host seeds the game RNG.
    SeedRng(u32),
}

impl World {
    /// Applies a command. Deterministic and free of I/O.
    pub fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::SetSpeed(change) => {
                let new = match change {
                    SpeedChange::Absolute(v) => i32::from(v),
                    SpeedChange::Delta(d) => i32::from(self.speed) + i32::from(d),
                };
                self.change_speed(new.clamp(0, i32::from(MAX_SPEED)) as u8);
            }
            Command::TogglePause => {
                // TODO(time.md "Em aberto", last item): the original key -> command mapping is
                // not read; this follows the documented resume rule.
                if self.speed == 0 {
                    self.change_speed(self.speed_resume);
                } else {
                    self.change_speed(0);
                }
            }
            Command::SeedRng(seed) => self.rng.seed(seed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::{Date, SpeedTable};

    fn world() -> World {
        World::new(Date::new(1860, 1, 1, 0, 0), 7, 1234)
    }

    fn speeds() -> SpeedTable {
        SpeedTable([0.0, 0.3, 0.6, 1.0, 1.7, 2.1, 3.0, 5.0, 8.0, 12.0, 18.0])
    }

    #[test]
    fn new_game_starts_at_speed_3() {
        let w = world();
        assert_eq!((w.speed(), w.speed_resume()), (3, 3));
        assert_eq!(w.date(), Date::new(1860, 1, 1, 0, 0));
    }

    #[test]
    fn speed_absolute_delta_and_clamp() {
        let mut w = world();
        w.apply(Command::SetSpeed(SpeedChange::Absolute(7)));
        assert_eq!(w.speed(), 7);
        w.apply(Command::SetSpeed(SpeedChange::Delta(-2)));
        assert_eq!(w.speed(), 5);
        w.apply(Command::SetSpeed(SpeedChange::Delta(100)));
        assert_eq!(w.speed(), 10);
        w.apply(Command::SetSpeed(SpeedChange::Delta(-100)));
        assert_eq!(w.speed(), 0);
        w.apply(Command::SetSpeed(SpeedChange::Absolute(200)));
        assert_eq!(w.speed(), 10);
    }

    #[test]
    fn pause_remembers_the_old_speed() {
        let mut w = world();
        w.apply(Command::SetSpeed(SpeedChange::Absolute(6)));
        w.apply(Command::SetSpeed(SpeedChange::Absolute(0)));
        assert_eq!((w.speed(), w.speed_resume()), (0, 6));
        // setting 0 again does not overwrite the resume speed
        w.apply(Command::SetSpeed(SpeedChange::Absolute(0)));
        assert_eq!(w.speed_resume(), 6);
        w.apply(Command::TogglePause);
        assert_eq!(w.speed(), 6);
        w.apply(Command::TogglePause);
        assert_eq!(w.speed(), 0);
    }

    #[test]
    fn tick_advances_the_clock_and_pause_stops_it() {
        let mut w = world();
        let s = speeds();
        // speed 3: 320 game minutes per 100 ms frame
        assert_eq!(w.tick(&s), 320);
        assert_eq!(w.minutes(), Date::new(1860, 1, 1, 0, 0).to_minutes() + 320);
        w.apply(Command::SetSpeed(SpeedChange::Absolute(0)));
        let before = w.minutes();
        assert_eq!(w.tick(&s), 0);
        assert_eq!(w.minutes(), before);
    }

    #[test]
    fn replay_is_deterministic() {
        let log = [
            (0, Command::SeedRng(99)),
            (1, Command::SetSpeed(SpeedChange::Absolute(8))),
            (3, Command::SetSpeed(SpeedChange::Delta(1))),
        ];
        let run = || {
            let mut w = world();
            for tick in 0..10 {
                for (t, c) in log {
                    if t == tick {
                        w.apply(c);
                    }
                }
                w.tick(&speeds());
                w.rng_mut().range(0, 1000);
            }
            w
        };
        assert_eq!(run(), run());
    }
}
