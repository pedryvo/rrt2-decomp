//! Steps of the world loop that only exist in a network game (docs/spec/multiplayer.md 1.6 and
//! 1.8): the checksum point and the daily RNG re-seed of `sim_advance` (`0x4a5cb0`), and
//! `trains_advance` (`0x4a58f0`).

use super::company::PE_CASH;
use super::*;
use crate::net::{NetEvent, checkpoint};

impl SimState {
    /// The persons of the player set (`0x570779`): the humans and computers that play.
    pub(crate) fn net_player_set(&self) -> Vec<usize> {
        self.player_set()
    }

    /// The start of `fim_de_dia` in a network game, before the week and month steps:
    /// `net_time_sync` (`0x49e0b0`: on day 1 the checksum record goes to the host) and
    /// `rng_seed(ftol((f64)(T + game_seed) + person0.cash))` (the f64 at `+0x12a` of the first
    /// person of the player set; the sum `T + game_seed` is the 64-bit integer of the original
    /// with a zero high word).
    pub(super) fn net_day_start(&mut self, w: &mut World) {
        self.log.ran("net_time_sync");
        if date_of(w.minutes).day == 1 {
            let cp = checkpoint(Some(self), w, false);
            if let Some(n) = w.net.as_mut() {
                n.events.push(NetEvent::Sync(Box::new(cp)));
            }
        }
        let cash = self
            .player_set()
            .first()
            .and_then(|&p| pool::record(&self.save.persons, p))
            .map_or(0.0, |r| f64_at(&r.data, PE_CASH));
        let t = w.minutes.wrapping_add(w.game_seed as i32) as u32;
        w.rng.seed(ftol(f64::from(t) + cash) as u32);
        self.log.ran("net_daily_reseed");
    }

    /// `trains_advance(dt)` (`0x4a58f0`): `train_step(train, dt, 1)` for every train in list
    /// order.
    pub(super) fn trains_advance(&mut self, w: &mut World, dt: i32) {
        self.log.ran("trains_advance");
        if !self.v2348() {
            return;
        }
        self.with_trainstep(w, |ts| {
            let mut id = ts.first_id(crate::trainstep::P::Train);
            while id != 0 {
                let next = ts.next_id(crate::trainstep::P::Train, id);
                ts.train_step(id, dt, true);
                ts.log.ran("train_step");
                id = next;
            }
        });
    }
}
