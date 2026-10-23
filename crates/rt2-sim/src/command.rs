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
    /// `crt_srand(arg)`: seeds the MSVC CRT `rand` (generator C). The original seeds it once at
    /// start-up; the save oracle (`advance_save.py`) seeds it together with the game RNG.
    SeedCrtRng(u32),
    /// A construction command of `cmd_execute` (`0x4b0210`), with the fields of the original
    /// packet: build track (`0x16`), build a station or a building (`0x17`), buy station
    /// improvements (`0x18`), buy a train (`0x19`), insert / delete a stop (`0x1d`, `0x1e`),
    /// sell a train (`0x1f`), replace an engine (`0x20`), go to a stop (`0x1a`), stop wait mode
    /// (`0x1b`), cargo per stop (`0x1c`), train class (`0x21`), mark a train ready to leave
    /// (`0x2c`, needed before a bought train is put on the track), stop sell mode (`0x2f`),
    /// buy or sell an industry (`0x2d`).
    /// See [`World::apply_build`] for the result.
    Build(crate::build::cmd::BuildCmd),
    /// `0x26` start a company (`cmd_found_company` `0x4bdcd0`, docs/spec/finance.md 17); see
    /// [`World::apply_found`] for the result and [`crate::found::founding_preview`] for the
    /// numbers of the dialog.
    StartCompany(crate::found::FoundCompany),
    /// A financial command of `cmd_execute` (stock trades, bonds, stock issue, dividend,
    /// manager, mergers, chairmanship; docs/spec/commands.md 2.1). `World::apply` runs the
    /// checks of the original's windows first, see [`World::apply_fin`].
    Finance(crate::fincmd::FinCmd),
    /// The player's button for the pending scenario choice (`World::pending_choice`): 0 the
    /// first, 1 the second (effect set 1). Ignored when `message_id` is not the pending one.
    /// See [`crate::choice`].
    AnswerChoice { message_id: u32, button: u8 },
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
            Command::SeedCrtRng(seed) => self.crt.seed(seed),
            Command::Build(c) => {
                let _ = self.apply_build(c);
            }
            Command::StartCompany(c) => {
                let _ = self.apply_found(c);
            }
            Command::Finance(c) => {
                let _ = self.apply_fin(c);
            }
            Command::AnswerChoice { message_id, button } => {
                let _ = self.answer_choice(message_id, button);
            }
        }
    }

    /// Applies a construction command and reports what the original reports: the created id
    /// (or the track cost), or why nothing was done (invalid placement, no path, insufficient
    /// cash as checked by the original's windows, unknown object). Without save pools (a bare
    /// [`World::new`]) every construction command fails with `NoSuchObject`.
    pub fn apply_build(
        &mut self,
        cmd: crate::build::cmd::BuildCmd,
    ) -> Result<i32, crate::build::cmd::CmdError> {
        let Some(mut sim) = self.sim.take() else {
            return Err(crate::build::cmd::CmdError::NoSuchObject);
        };
        let r = sim.apply_build(self, cmd);
        self.sim = Some(sim);
        r
    }
}

impl World {
    /// Starts a company (command `0x26`): the window checks of `0x4bd950`, `company_init` and the
    /// founder's purchase. Returns the new company id or why nothing was done. Without save pools
    /// every founding fails with `NoSuchObject`.
    pub fn apply_found(
        &mut self,
        cmd: crate::found::FoundCompany,
    ) -> Result<i32, crate::build::cmd::CmdError> {
        let Some(mut sim) = self.sim.take() else {
            return Err(crate::build::cmd::CmdError::NoSuchObject);
        };
        let r = sim.found_company(self, cmd);
        self.sim = Some(sim);
        r
    }
}

impl World {
    /// Runs a financial command as the player's window does: the checks of the window
    /// ([`World::check_fin`]) and then the executor of `cmd_execute`. Without save pools every
    /// command fails with `NoSuchObject`.
    pub fn apply_fin(
        &mut self,
        cmd: crate::fincmd::FinCmd,
    ) -> Result<crate::fincmd::FinOutcome, crate::fincmd::FinError> {
        self.check_fin(&cmd)?;
        self.apply_fin_unchecked(cmd)
    }

    /// The executor alone: what a packet from a peer or a replay does (`cmd_execute` validates
    /// only that the ids are live).
    pub fn apply_fin_unchecked(
        &mut self,
        cmd: crate::fincmd::FinCmd,
    ) -> Result<crate::fincmd::FinOutcome, crate::fincmd::FinError> {
        let Some(mut sim) = self.sim.take() else {
            return Err(crate::fincmd::FinError::NoSuchObject);
        };
        let r = sim.fin_execute(self, cmd);
        self.sim = Some(sim);
        r
    }

    /// The refusal the window of the original would show for `cmd`, `Ok` when it would send
    /// the packet.
    pub fn check_fin(&self, cmd: &crate::fincmd::FinCmd) -> Result<(), crate::fincmd::FinError> {
        match self.sim.as_ref() {
            Some(sim) => sim.fin_check(self.minutes, cmd),
            None => Err(crate::fincmd::FinError::NoSuchObject),
        }
    }

    /// `margin_check_all` (`0x46bb10`): the broker's routine for every person of the player set.
    /// The original runs it after each stock trade and every month; this is the entry point for a
    /// check at any other time (a replay, a test).
    pub fn margin_check(&mut self) {
        let Some(mut sim) = self.sim.take() else {
            return;
        };
        sim.margin_check_all(self);
        self.sim = Some(sim);
    }

    /// The merger and chairmanship votes that wait for a human shareholder.
    pub fn pending_votes(&self) -> Vec<crate::fincmd::PendingVote> {
        self.sim
            .as_ref()
            .map_or_else(Vec::new, |s| s.fc_pending_votes(self))
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
