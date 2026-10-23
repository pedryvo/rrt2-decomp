//! The world loop: `sim_advance` and the day/week/month/year hooks of docs/spec/world-tick.md.
//!
//! [`World::from_save`] builds the state from a parsed save, [`World::advance`] runs the loop and
//! [`World::to_save`] writes the state back. The save is kept whole (`Save` holds every byte of
//! the file, the pools with their `next`/`prev` arrays included), and each implemented step edits
//! the records it owns in place, so everything not simulated yet is written back unchanged. A
//! step that is not implemented is a [`todo_hook!`](crate::todo_hook): it is counted and cites
//! the spec item, see [`HookLog`].
//!
//! Fidelity assumptions that are not in the save (runtime globals of the original, all zero or
//! default in a freshly loaded game): no "Overtime" cheat, no multiplayer, `0x5748f8 == 0`,
//! `0x5794f0 == 0` (no station network rebuild pending), industry set 0 (`0x5704ac`), no queued
//! player commands. The state offsets below equal the offset from `game_running` `0x5704e8`
//! and hold for version 0x2348 files; a 0x233C file shifts them (map-gam.md 5.1), so the steps
//! that read them are skipped (and counted) on those.

use rt2_formats::map::{GameDate, MAGIC_2348, Save};

use crate::calendar::{Date, MINUTES_PER_DAY, minutes_to_date};
use crate::cargo::IndustryType;
use crate::data::GameData;
use crate::finance::{prime_rate_for, prime_rate_for_state};
use crate::fp::ftol;
use crate::hooks::HookLog;
use crate::industry::{self, EffectTable, Env};
use crate::pool::{
    self, f32_at, f64_at, i16_at, i32_at, put_f32, put_f64, put_u16, put_u32, u16_at, u32_at,
};
use crate::todo_hook;
use crate::world::World;

mod company;
mod demolish;
mod editor;
mod events;
mod fincmd;
mod flood;
mod found;
mod history;
mod newgame;
pub use newgame::NewGameOptions;
mod review;
pub(crate) use history::co as history_co;
pub(crate) use review::{PickIn, pick_manager};
mod town;
mod year;

// State block offsets (VA - 0x5704e8), version 0x2348.
const ST_PRIME_RATE: usize = 0x1E9; // 0x5706d1 f32
const ST_ECON_STATE: usize = 0x22F; // 0x570717 i32
const ST_ECON_MOM: usize = 0x233; // 0x57071b i32
const ST_ECON_DATE: usize = 0x237; // 0x57071f: year u16, month, day
const ST_ECON_TIME: usize = 0x23B; // 0x570723: hour, minute
const ST_ECON_DRIFT: usize = 0x23D; // 0x570725 i32
const ST_GLOBAL_EFFECTS: usize = 0x2DE; // 0x5707c6 i16[]
const ST_BASIC_MODEL: usize = 0x388F; // 0x573d77
// Header offsets (VA - 0x567b38).
const HDR_SANDBOX: usize = 0x86EF; // 0x570227
const HDR_NO_ECONOMY: usize = 0x86FF; // 0x570237
const HDR_570238: usize = 0x8700; // 0x570238 (no economy headline as news)
const HDR_57023A: usize = 0x8702; // 0x57023a (idem)
const HDR_FLAG_570229: usize = 0x86F1; // 0x570229
const HDR_FLAG_570236: usize = 0x86FE; // 0x570236

/// Effect id of the prime rate adjustment (`0x46bac0` asks `0x46b5d0` for `0x20`).
const EFFECT_PRIME: usize = 0x20;

/// Simulation state attached to a [`World`] built from a save.
#[derive(Clone, Debug)]
pub struct SimState {
    pub(crate) save: Save,
    data: Box<GameData>,
    /// Industry types of the active set, indices rebuilt for the current year (`0x471880`).
    types: Vec<IndustryType>,
    pub(crate) log: HookLog,
    /// Runtime globals of the train step that outlive a call.
    ts_globals: crate::trainstep::Globals,
    /// The decoded scenario rule pool, built on the first trigger.
    scenario: Option<crate::scenario::Scenario>,
    /// Slot count of the building pool when the save was read: the slots past it live in memory
    /// the pool grew into (`pool_grow`), which Wine's debug heap fills with `0xBAADF00D`; the
    /// bytes `building_construct` never writes (`+0x38..+0x3c`) keep that fill.
    objects_loaded_slots: usize,
    /// The map's redraw rectangle (`0x417770`), painted by `0x417c40` (light pass).
    pub(crate) dirty: crate::tilemark::DirtyRect,
    /// Per tile: its overlay list holds ground blends (runtime state of the loader's terrain
    /// passes and of `0x42a7d0`; [`crate::fields::FieldCtx::ground`]). Built on first use.
    pub(crate) ground_layer: Option<Vec<bool>>,
    /// The drawn track of the whole map while `town_year_step` runs (it builds no track).
    pub(crate) track_cache: Option<crate::fields::TrackMask>,
    /// The status buffer `[0x5665e0]` while `rule_collect_status` (`0x458210`) runs: the
    /// trigger 7 messages are appended to it instead of being shown.
    pub(crate) status_buf: Option<Vec<u8>>,
}

impl PartialEq for SimState {
    fn eq(&self, other: &Self) -> bool {
        self.save == other.save && self.log == other.log
    }
}
impl Eq for SimState {}

fn date_of(minutes: i32) -> Date {
    minutes_to_date(minutes)
}

impl World {
    /// Builds the simulation state from a parsed save: clock, speed, `game_seed`, and every pool
    /// kept raw. The game RNG is not in the file (map-gam.md 5.2): seed it with
    /// [`Command::SeedRng`](crate::Command::SeedRng) before advancing.
    pub fn from_save(save: &Save, data: &GameData) -> World {
        World::from_save_seeded(save, data, 0)
    }

    /// [`World::from_save`] with the game RNG at `rng_seed` when the load starts. A save older
    /// than 0x2348 is converted first as the loader `0x477a60` does ([`crate::convert`]), which
    /// consumes RNG; the world keeps the RNG state the conversion left (the original's load does
    /// not restore it either, `docs/dynamic-analysis.md` 5.1).
    pub fn from_save_seeded(save: &Save, data: &GameData, rng_seed: u32) -> World {
        let mut w = if save.layout.version >= MAGIC_2348 {
            World::from_loaded(save, data, rng_seed, rng_seed)
        } else {
            let mut rng = rt2_formats::terrain::GameRng(rng_seed);
            let converted = crate::convert::upgrade(save, &mut rng);
            let mut w = World::from_loaded(&converted, data, rng.0, rng_seed);
            let mut sim = w.sim.take().expect("from_loaded builds the sim");
            sim.load_passes(&mut w, save.layout.version);
            w.sim = Some(sim);
            w
        };
        // The Load Game wrapper paints the map once the load is done (`0x421550` ->
        // `0x417c40`): the loader's union is lit and cleared, so a later redraw (the save's
        // preview, see `to_save`) only covers what changed since.
        if let Some(sim) = w.sim.as_mut() {
            sim.redraw_map();
        }
        w
    }

    /// A world over `save` (already in the 0x2348 layout) with the given RNG states, before any
    /// load pass.
    pub(crate) fn from_loaded(save: &Save, data: &GameData, rng: u32, crt: u32) -> World {
        let mut w = World::new(
            Date::from_minutes(save.game_time_minutes),
            save.game_seed,
            rng,
        );
        w.crt = crate::rng::CrtRng::new(crt);
        w.minutes = save.game_time_minutes;
        w.speed = save.game_speed;
        if save.game_speed != 0 {
            w.speed_resume = save.game_speed;
        }
        let mut sim = SimState {
            save: save.clone(),
            data: Box::new(data.clone()),
            types: Vec::new(),
            log: HookLog::default(),
            ts_globals: Default::default(),
            scenario: None,
            objects_loaded_slots: save.objects.slots,
            // The loader's ground pass (`0x42a7d0(0, 0, W - 1, H - 1)`) leaves
            // `0x417770(0, 0, W - 2, H - 2)`.
            dirty: crate::tilemark::DirtyRect(Some([
                0,
                0,
                save.map.width as i32 - 2,
                save.map.height as i32 - 2,
            ])),
            ground_layer: None,
            track_cache: None,
            status_buf: None,
        };
        sim.rebuild_types(i32::from(save.date.year));
        w.sim = Some(Box::new(sim));
        w
    }

    /// The state as a save of the version it was loaded from. Panics if the world was not built
    /// with [`World::from_save`].
    pub fn to_save(&self) -> Save {
        let sim = self.sim.as_ref().expect("World::to_save needs from_save");
        let mut s = sim.save.clone();
        let d = date_of(self.minutes);
        s.game_time_minutes = self.minutes;
        s.date = GameDate {
            year: d.year,
            month: d.month,
            day: d.day,
        };
        s.hour = d.hour;
        s.minute = d.minute;
        s.game_speed = self.speed;
        s.game_seed = self.game_seed;
        s.sync_scalars();
        crate::convert::saver_refresh(&mut s);
        // The saver draws the preview from the map view (`0x477540` -> `0x477040`), which runs
        // the pending light pass (`0x417c40`) over the redraw union accumulated since the last
        // paint (`0x417770`; `advance_save.py` paints nothing in between). Seen in
        // NewJapan.MP2 after the 1995 year end (`tile+6 & 0x20` around the levelled lots).
        if let Some([x0, y0, x1, y1]) = sim.dirty.0 {
            crate::tilemark::recompute_light(&mut s, x0, y0, x1, y1);
        }
        s
    }

    /// Drains the output queue of player-visible messages ([`crate::message`]), oldest first.
    /// Deterministic: the queue is filled by the rules and never read by them.
    pub fn take_messages(&mut self) -> Vec<crate::message::GameMessage> {
        self.sim
            .as_mut()
            .map(|s| std::mem::take(&mut s.ts_globals.news))
            .unwrap_or_default()
    }

    /// The end of the game decided by the scenario rules (`GameOver.state` 1..=3 victory
    /// bronze/silver/gold, 4 defeat, scenarios.md 3.3), `None` while playing.
    pub fn game_over(&self) -> Option<crate::message::Outcome> {
        use crate::message::Outcome;
        match self.sim.as_ref()?.scenario.as_ref()?.game_over.state {
            1 => Some(Outcome::Bronze),
            2 => Some(Outcome::Silver),
            3 => Some(Outcome::Gold),
            4 => Some(Outcome::Lose),
            _ => None,
        }
    }

    /// Text the winning/losing rule supplied (empty without one).
    pub fn game_over_text(&self) -> &str {
        self.sim
            .as_ref()
            .and_then(|s| s.scenario.as_ref())
            .map_or("", |sc| sc.game_over.text.as_str())
    }

    /// Defeat because another player won (LNG `0xa83`).
    pub fn game_over_other_won(&self) -> bool {
        self.sim
            .as_ref()
            .and_then(|s| s.scenario.as_ref())
            .is_some_and(|sc| sc.game_over.other_won)
    }

    /// Step counters of the world loop (implemented and missing steps); `None` when the world was
    /// not built from a save.
    pub fn hook_log(&self) -> Option<&HookLog> {
        self.sim.as_ref().map(|s| &s.log)
    }

    /// `sim_advance(dt)` (`0x4a5cb0`, world-tick.md section 2): advances `dt` game minutes,
    /// running the end-of-day/week/month/year hooks at every 23:59 crossed. Without a save only
    /// the clock moves.
    pub fn advance(&mut self, dt: i32) {
        // Interactive scenario choices (`crate::choice`): wait while one is pending, and keep a
        // copy to replay the step when a choice comes up without an answer.
        if self.choices.pending.is_some() {
            return;
        }
        if self.choices.interactive && self.sim.as_ref().is_some_and(|s| s.has_choice_rules()) {
            let before = self.clone();
            self.choices.cursor = 0;
            self.choices.replayable = true;
            self.advance_step(dt);
            self.choices.replayable = false;
            if let Some(message) = self.choices.unanswered.take() {
                let mut c = std::mem::take(&mut self.choices);
                *self = before;
                c.cursor = 0;
                c.pending = Some(crate::choice::PendingChoice {
                    id: c.next_id,
                    message,
                });
                c.next_id = c.next_id.wrapping_add(1);
                self.choices = c;
            } else {
                self.choices.answers.clear();
                self.choices.cursor = 0;
            }
            return;
        }
        self.advance_step(dt);
    }

    fn advance_step(&mut self, dt: i32) {
        match self.sim.take() {
            Some(mut sim) => {
                sim.sim_advance(self, dt);
                self.sim = Some(sim);
            }
            None => self.advance_minutes(dt),
        }
    }
}

impl SimState {
    /// Some rule of the pool is a two-button choice (`+0x60 == 2`).
    fn has_choice_rules(&self) -> bool {
        self.save
            .rules
            .iter()
            .any(|r| r.fixed.get(0x60..0x64) == Some(&[2, 0, 0, 0][..]))
    }

    fn v2348(&self) -> bool {
        self.save.layout.version == MAGIC_2348
    }

    fn hdr(&self, off: usize) -> u8 {
        self.save.header.get(off).copied().unwrap_or(0)
    }

    fn sandbox(&self) -> bool {
        self.hdr(HDR_SANDBOX) != 0
    }

    fn rebuild_types(&mut self, year: i32) {
        // The industry set `0x5704ac` (header +0x8974): 0 standard, 1 expansion. The load
        // fix-ups of `0x4719e0` only touch set 0; set 1 only rebuilds the inputs (cargo.md 7.1.1).
        let set = usize::from(self.hdr(0x8974) != 0);
        let mut types = self.data.industries[set].clone();
        if set == 0 {
            crate::industry::apply_load_fixups(&mut types, &self.save.header, &self.data.cargo);
        }
        for t in &mut types {
            t.rebuild_inputs(year);
        }
        self.types = types;
        self.log.ran("industry_rebuild_inputs");
    }

    /// Writes the clock into the save blocks that mirror it.
    fn set_clock(&mut self, w: &mut World, minutes: i32) {
        w.minutes = minutes;
        let d = date_of(minutes);
        self.save.game_time_minutes = minutes;
        self.save.date = GameDate {
            year: d.year,
            month: d.month,
            day: d.day,
        };
        self.save.hour = d.hour;
        self.save.minute = d.minute;
        // The clock globals live in the state block (`0x5704f4` minutes, `0x5704f8` date,
        // `0x5704fc` time): code reading the raw state (the scenario functions `GameYear`,
        // `GameMonth`...) must see the live date, not the one of the load (found at the 1933
        // year start of #E01: `GameYear>1932` stayed false).
        let st = &mut self.save.state;
        if st.len() >= 0x16 {
            st[0x0c..0x10].copy_from_slice(&minutes.to_le_bytes());
            st[0x10..0x12].copy_from_slice(&d.year.to_le_bytes());
            st[0x12] = d.month;
            st[0x13] = d.day;
            st[0x14] = d.hour;
            st[0x15] = d.minute;
        }
    }

    // ---- sim_advance (world-tick.md 2) ---------------------------------------------------

    fn sim_advance(&mut self, w: &mut World, dt: i32) {
        self.log.ran("sim_advance");
        // 1. Scheduled commands: none in a local game without a player.
        let mut dt = dt;
        if dt == 0 {
            return;
        }
        let cur = date_of(w.minutes);
        let new = date_of(w.minutes.wrapping_add(dt));
        // The original tests each field with `<`, not the date as a whole.
        if cur.year < new.year || cur.month < new.month || cur.day < new.day {
            let to_midnight = 1439 - (i32::from(cur.hour) * 60 + i32::from(cur.minute));
            self.sim_advance(w, to_midnight);
            self.day_end(w);
            dt -= to_midnight + 1;
            loop {
                let d = date_of(w.minutes);
                if (d.year, d.month, d.day) == (new.year, new.month, new.day) {
                    break;
                }
                self.sim_advance(w, 1439);
                self.day_end(w);
                dt -= MINUTES_PER_DAY;
            }
        }
        self.linear_pass(w, dt);
    }

    /// The linear path of `sim_advance` (world-tick.md 2.1), also run with `dt == 0`.
    fn linear_pass(&mut self, w: &mut World, dt: i32) {
        self.log.ran("linear_pass");
        let t = w.minutes.wrapping_add(dt);
        self.set_clock(w, t);
        if !self.v2348() {
            todo_hook!(
                self.log,
                "linear_pass_233c",
                "anim_object_step / train_step on a 0x233C file (offsets not mapped)",
                "map-gam.md 5.1"
            );
            return;
        }
        self.with_trainstep(w, |ts| {
            // Map animations, pool_78 in list order (pattern A).
            let mut id = ts.first_id(crate::trainstep::P::Anim);
            while id != 0 {
                let next = ts.next_id(crate::trainstep::P::Anim, id);
                ts.anim_object_step(id);
                ts.log.ran("anim_object_step");
                id = next;
            }
            // Trains of the single local player, in list order (pattern A).
            let mut id = ts.first_id(crate::trainstep::P::Train);
            while id != 0 {
                let next = ts.next_id(crate::trainstep::P::Train, id);
                ts.train_step(id, dt, true);
                ts.log.ran("train_step");
                id = next;
            }
        });
    }

    /// Runs `f` with the train step's view of the state (`crate::trainstep::Ts`) at the
    /// current clock.
    pub(crate) fn with_trainstep<R>(
        &mut self,
        w: &mut World,
        f: impl FnOnce(&mut crate::trainstep::Ts<'_>) -> R,
    ) -> R {
        self.with_trainstep_parts(w.minutes, &mut w.rng, &mut w.crt, f)
    }

    /// [`SimState::with_trainstep`] from the clock and the two generators (for callers that
    /// hold them apart from the [`World`], like the scenario effects).
    pub(crate) fn with_trainstep_parts<R>(
        &mut self,
        minutes: i32,
        rng: &mut crate::rng::GameRng,
        crt: &mut crate::rng::CrtRng,
        f: impl FnOnce(&mut crate::trainstep::Ts<'_>) -> R,
    ) -> R {
        let date = date_of(minutes);
        let flag_570229 = self.hdr(HDR_FLAG_570229);
        let flag_570236 = self.hdr(HDR_FLAG_570236);
        let mut ts = crate::trainstep::Ts {
            s: &mut self.save,
            img: &self.data.image,
            anim_frames: &self.data.anim_frames,
            types: &self.types,
            cargo: &self.data.cargo,
            revenue: &self.data.revenue,
            now: minutes as u32,
            year: date.year,
            rng,
            crt,
            log: &mut self.log,
            g: std::mem::take(&mut self.ts_globals),
            flag_570229,
            flag_570236,
        };
        let r = f(&mut ts);
        self.ts_globals = ts.g;
        r
    }

    /// Historical notices of `sim_year_end` item 19 (time.md 7.5, world-growth.md 8): 1877 (type
    /// 36), 1879 (type 29 or 10), 1900 (type 12), 1910 (always), 1960 (type 14). The condition is
    /// the "type seen" flag `0x570603[type]` (set by the first building of the type) and the
    /// economy-off header byte `0x570237 == 0`; the polarity of the flag is read as "seen".
    fn year_start_messages(&mut self, year: i32) {
        if self.hdr(HDR_NO_ECONOMY) != 0 {
            return;
        }
        let seen = |s: &Self, t: usize| s.save.state.get(0x11B + t).is_some_and(|&v| v != 0);
        let lng = match year {
            1877 if seen(self, 36) => 2630,
            1879 if seen(self, 29) || seen(self, 10) => 2851,
            1900 if seen(self, 12) => 419,
            1910 => 417,
            1960 if seen(self, 14) => 420,
            _ => return,
        };
        self.log.ran("year_start_messages");
        self.ts_globals
            .news
            .push(crate::message::GameMessage::Historical { year, lng });
    }

    /// The 23:59 step (`fim_de_dia`, world-tick.md 2 and 3.1) of a local game.
    fn day_end(&mut self, w: &mut World) {
        self.log.ran("day_end");
        let day = date_of(w.minutes).day;
        if day.is_multiple_of(7) {
            self.week_end(w);
            if day == 28 {
                self.month_end(w);
                return;
            }
        }
        self.set_clock(w, w.minutes + 1);
    }

    // ---- week (world-tick.md 3.2) --------------------------------------------------------

    fn effect_table(&self) -> EffectTable {
        let mut t = EffectTable::default();
        if self.v2348() {
            let n = 0x140;
            t.global = (0..n)
                .map(|i| {
                    let o = ST_GLOBAL_EFFECTS + 2 * i;
                    self.save
                        .state
                        .get(o..o + 2)
                        .map_or(0, |_| i16_at(&self.save.state, o))
                })
                .collect();
        }
        for r in &self.save.territories.records {
            if r.data.len() >= 0x58 {
                let v = (0..(r.data.len() - 0x58) / 2)
                    .map(|i| i16_at(&r.data, 0x58 + 2 * i))
                    .collect();
                t.territories.insert(r.slot, v);
            }
        }
        t
    }

    pub(super) fn week_end(&mut self, w: &mut World) {
        self.log.ran("sim_week_end");
        if !self.v2348() {
            todo_hook!(
                self.log,
                "state_layout_233c",
                "0x233C files shift the global state block; economy and effect fields not read",
                "map-gam.md 5.1"
            );
        }
        let state = if self.v2348() {
            i32_at(&self.save.state, ST_ECON_STATE).clamp(0, 4) as usize
        } else {
            2
        };
        let effects = self.effect_table();
        let types = std::mem::take(&mut self.types);
        let env = Env {
            types: &types,
            now: w.minutes,
            year: i32::from(date_of(w.minutes).year),
            basic_model: self.v2348() && self.save.state[ST_BASIC_MODEL] != 0,
            sandbox: self.sandbox(),
            overtime: false,
            econ_factor: self.data.production_econ[state],
            effects: &effects,
        };
        // Industries, pattern B: the next record is read after the call, the count is re-read.
        // Lots created by an industry with linked stations update those stations; doing it after
        // the loop gives the same records, since the weekly station pass below recomputes every
        // station from scratch (only the `+0x45` stations keep the `0x468af0` increments, and the
        // `+0x4b5` counters add up the same).
        let mut spawned: Vec<(usize, Vec<u8>)> = Vec::new();
        let mut i = 0usize;
        let mut cur = 0usize;
        while i < self.save.objects.count {
            cur = if i == 0 {
                pool::first(&self.save.objects)
            } else {
                pool::next(&self.save.objects, cur)
            };
            if cur == 0 {
                break;
            }
            let Some(rec) = pool::record(&self.save.objects, cur) else {
                break;
            };
            if rec.data[industry::OFF_TYPE] != 0 {
                let mut data = rec.data.clone();
                industry::recount_piles(&mut data, &self.save.pool_7c, &env);
                self.log.ran("industry_recount_piles");
                let lots = industry::production_tick(
                    &mut data,
                    &mut self.save.pool_7c,
                    &env,
                    &mut self.log,
                );
                self.log.ran("industry_production_tick");
                if !lots.is_empty() {
                    spawned.push((cur, lots));
                }
                if let Some(r) = pool::record_mut(&mut self.save.objects, cur) {
                    r.data = data;
                }
            }
            i += 1;
        }
        self.types = types;
        if self.v2348() {
            for (ind, lots) in spawned {
                self.with_trainstep(w, |ts| {
                    for c in lots {
                        ts.industry_lot_spawned(ind as i32, c);
                    }
                });
            }
        }
        // Stations, pattern B.
        let mut id = pool::first(&self.save.stations);
        while id != 0 {
            let next = pool::next(&self.save.stations, id);
            if self.v2348() {
                self.with_trainstep(w, |ts| ts.station_recompute_supply_demand(id as i32));
                self.log.ran("station_recompute_supply_demand");
            } else {
                todo_hook!(
                    self.log,
                    "station_recompute_supply_demand",
                    "station_recompute_supply_demand 0x4bebf0 on a 0x233C file",
                    "cargo.md 8; world-tick.md 3.2 item 2"
                );
            }
            id = next;
        }
    }

    // ---- month (world-tick.md 3.3) -------------------------------------------------------

    fn month_end(&mut self, w: &mut World) {
        self.log.ran("sim_month_end");
        let date = date_of(w.minutes);
        // 1. station_networks_rebuild (flag 0x5794f0 never set in a loaded game).
        self.event_trigger(w, 4);
        self.event_trigger(w, 3);
        if date.month == 2 || date.month == 8 {
            self.economy_state_step(w, date);
        }
        if date.month < 11 {
            self.town_bonus_roll(w, date);
        }
        if !self.sandbox() {
            self.expire_lots(w);
        }
        self.ai_monthly_turn(w);
        // 8. Companies (pattern C).
        for slot in pool::list_order(&self.save.companies) {
            self.company_month_step(w, slot);
        }
        // 9. Persons (pattern C).
        for slot in pool::list_order(&self.save.persons) {
            self.person_month_step(slot);
        }
        // 10. Trains (pattern C).
        if self.v2348() {
            for slot in pool::list_order(&self.save.trains) {
                self.with_trainstep(w, |ts| ts.train_monthly_costs(slot as i32));
                self.log.ran("train_monthly_costs");
            }
        } else {
            todo_hook!(
                self.log,
                "train_monthly_costs",
                "train_monthly_costs 0x4d4390 on a 0x233C file",
                "trains.md 6.2; world-tick.md 3.3 item 10"
            );
        }
        // 11. Stations: delivery counters decay, then supply/demand.
        for slot in pool::list_order(&self.save.stations) {
            if let Some(r) = pool::record_mut(&mut self.save.stations, slot) {
                for k in 0..44 {
                    let o = 0x271 + 4 * k;
                    if o + 4 <= r.data.len() {
                        let v = f64::from(f32_at(&r.data, o)) * 0.94;
                        put_f32(&mut r.data, o, v as f32);
                    }
                }
            }
            if self.v2348() {
                self.with_trainstep(w, |ts| ts.station_recompute_supply_demand(slot as i32));
                self.log.ran("station_recompute_supply_demand");
            }
        }
        self.margin_check_all(w);
        // 13. `report_due_flag` is a UI flag.
        if date.month == 12 {
            self.year_end(w);
        } else {
            self.set_clock(w, w.minutes + 1);
        }
        self.event_trigger(w, 2);
    }

    /// `economy_state_step` (`0x46a010`, cargo.md 9) [S]; the RNG draws are [D] (world-tick.md 4).
    fn economy_state_step(&mut self, w: &mut World, date: Date) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "economy_state_step",
                "economy_state_step 0x46a010 on a 0x233C state block",
                "cargo.md 9"
            );
            return;
        }
        if self.hdr(HDR_NO_ECONOMY) != 0 {
            return;
        }
        self.log.ran("economy_state_step");
        let st = &mut self.save.state;
        let now_idx = i32::from(date.year) * 12 + i32::from(date.month);
        let last_idx = i32::from(u16_at(st, ST_ECON_DATE)) * 12 + i32::from(st[ST_ECON_DATE + 2]);
        if now_idx == last_idx {
            return;
        }
        let state = i32_at(st, ST_ECON_STATE);
        put_u16(st, ST_ECON_DATE, date.year);
        st[ST_ECON_DATE + 2] = date.month;
        st[ST_ECON_DATE + 3] = date.day;
        st[ST_ECON_TIME] = date.hour;
        st[ST_ECON_TIME + 1] = date.minute;
        let drift = ftol(f64::from(i32_at(st, ST_ECON_DRIFT)) * 0.8)
            .wrapping_add((now_idx - last_idx).wrapping_mul(state - 2));
        put_u32(st, ST_ECON_DRIFT, drift as u32);
        let r = w.rng.range(-5, 5) + 2;
        let mom_old = i32_at(st, ST_ECON_MOM);
        let mom = ftol(f64::from(r) - f64::from(drift) * 0.1 + f64::from(mom_old)).clamp(-5, 5);
        put_u32(st, ST_ECON_MOM, mom as u32);
        let roll = w.rng.range(0, 10);
        let mut new_state = state;
        if mom.abs() > roll {
            new_state += if mom < 1 { -1 } else { 1 };
        }
        let new_state = new_state.clamp(0, 4);
        if new_state != state {
            self.log.ran("economy_headline");
            self.ts_globals
                .news
                .push(crate::message::GameMessage::EconomyChanged {
                    from: state as u8,
                    to: new_state as u8,
                });
            put_f32(st, ST_PRIME_RATE, prime_rate_for_state(new_state as usize));
            put_u32(st, ST_ECON_STATE, new_state as u32);
            // The headline itself (`0x46a010`): outside the editor (`0x573d7a`) and with the
            // header bytes `0x570238`/`0x57023a` clear, a local game posts it as news
            // (`news_post` `0x4a42f0`: one CRT draw, news flag and counter); otherwise a message
            // box (`0x46aa90`, nothing saved but the interface view). Seen in NewJapan.MP2 at
            // 1995-09-01 (day 224 of the long-horizon scoreboard).
            if self.hdr(HDR_570238) == 0 && self.hdr(HDR_57023A) == 0 {
                self.fc_news_post(w);
            }
            // Every company of the set quotes again (`0x43b410(0, 1)`, cargo.md 9, finance.md
            // 3.3).
            for slot in self.company_set() {
                self.company_quote(w.minutes, slot, true);
            }
        }
    }

    /// The monthly `rng_range(0, 10000)` draw and, on success, `town_bonus_offer` (`0x431660`,
    /// world-tick.md 3.3 item 5 and 3.6).
    fn town_bonus_roll(&mut self, w: &mut World, date: Date) {
        let width = self.save.map.width as i32;
        let years = (i32::from(date.year) - 1860).abs();
        let roll = w.rng.range(0, 10000);
        let threshold = (50 - years) * (54300 / width.max(1)) / 70;
        self.log.ran("town_bonus_roll");
        if roll < threshold {
            self.town_bonus_offer(w);
        }
    }

    /// `town_bonus_offer` (`0x431660`, world-tick.md 3.6) [S]: up to 500 tries of one RNG draw.
    fn town_bonus_offer(&mut self, w: &mut World) {
        self.log.ran("town_bonus_offer");
        let places = pool::list_order(&self.save.places);
        if places.is_empty() {
            return;
        }
        let stations: Vec<(i32, i32)> = pool::list_order(&self.save.stations)
            .into_iter()
            .filter_map(|s| pool::record(&self.save.stations, s))
            .map(|r| (i32_at(&r.data, 0xa), i32_at(&r.data, 0xe)))
            .collect();
        for _ in 0..500 {
            let i = w.rng.range(0, places.len() as i32 - 1) as usize;
            let slot = places[i];
            let Some(rec) = pool::record(&self.save.places, slot) else {
                continue;
            };
            let p = &rec.data;
            if p[0x10] != 0 || i32_at(p, 0x36) >= 1 {
                continue;
            }
            let (px, py) = (f64::from(i16_at(p, 4)), f64::from(i16_at(p, 6)));
            let near = stations.iter().any(|&(x, y)| {
                x >= i32::from(i16_at(p, 8))
                    && x <= i32::from(i16_at(p, 12))
                    && y >= i32::from(i16_at(p, 10))
                    && y <= i32::from(i16_at(p, 14))
                    && {
                        let (dx, dy) = (f64::from(x as f32) - px, f64::from(y as f32) - py);
                        (dx * dx + dy * dy).sqrt() < 9.0
                    }
            });
            if near {
                continue;
            }
            let class = usize::from(p[0x2c]);
            let bonus = self.data.town_bonus.get(class).copied().unwrap_or(0);
            if let Some(r) = pool::record_mut(&mut self.save.places, slot) {
                put_u32(&mut r.data, 0x36, bonus as u32);
            }
            // LNG 226 (the map ping `0x475da0` is UI).
            self.log.ran("town_bonus_message");
            self.ts_globals
                .news
                .push(crate::message::GameMessage::TownBonusOffer {
                    place: slot as i32,
                    amount: bonus,
                });
            return;
        }
    }

    /// Lot expiry (`sim_month_end` item 6, cargo.md 7.6): lots without a station that are older
    /// than 150 days and, when their industry exists, than half the cargo's wait scale (a
    /// quarter when the industry has no linked station) are freed (`lot_free` `0x466a60`) and
    /// the pile count of the industry drops. Slots are visited in descending order.
    fn expire_lots(&mut self, w: &mut World) {
        self.log.ran("lot_expiry_scan");
        let mut due: Vec<(i32, Option<(usize, usize)>)> = Vec::new();
        for r in self.save.pool_7c.records.iter().rev() {
            let lot = &r.data;
            if u16_at(lot, industry::LOT_STATION) != 0 {
                continue;
            }
            let age = w.minutes.wrapping_sub(i32_at(lot, industry::LOT_CREATED));
            if age < 216_000 {
                continue;
            }
            let ind = usize::from(u16_at(lot, industry::LOT_INDUSTRY));
            let mut count_slot = None;
            if let Some(ir) = pool::record(&self.save.objects, ind) {
                let wait = self
                    .data
                    .cargo
                    .get(usize::from(lot[industry::LOT_CARGO]))
                    .map_or(0, |c| c.wait_scale);
                let shift = if ir.data[industry::OFF_STATIONS] == 0 {
                    2
                } else {
                    1
                };
                if age < (wait >> shift) {
                    continue;
                }
                let ty = usize::from(ir.data[industry::OFF_TYPE]);
                let slot = self.types.get(ty).and_then(|t| {
                    t.slots
                        .iter()
                        .position(|s| s.output == lot[industry::LOT_CARGO])
                });
                count_slot = slot.map(|k| (ind, k));
            }
            due.push((r.slot as i32, count_slot));
        }
        for (lot, count) in due {
            if let Some((ind, k)) = count
                && let Some(ir) = pool::record_mut(&mut self.save.objects, ind)
            {
                let o = industry::OFF_PILE_COUNT + 2 * k;
                let v = i16_at(&ir.data, o).wrapping_sub(1);
                pool::put_i16(&mut ir.data, o, v);
            }
            self.with_trainstep(w, |ts| ts.lot_free(lot));
            self.log.ran("lot_expiry");
        }
    }

    /// `person_month_step` (`0x4a7590`, world-tick.md 3.3 item 9): the personal cash earns
    /// `(prime(0) - 0.04) * cash * (1/12)`, truncated to whole dollars.
    fn person_month_step(&mut self, slot: usize) {
        if !self.v2348() {
            todo_hook!(
                self.log,
                "person_month_step",
                "person_month_step 0x4a7590 on a 0x233C state block",
                "world-tick.md 3.3 item 9"
            );
            return;
        }
        let prime_rate = f32_at(&self.save.state, ST_PRIME_RATE);
        let effect = i16_at(&self.save.state, ST_GLOBAL_EFFECTS + 2 * EFFECT_PRIME);
        let prime = prime_rate_for(prime_rate, effect);
        if let Some(r) = pool::record_mut(&mut self.save.persons, slot) {
            let cash = f64_at(&r.data, 0x12a);
            let add = ftol((prime - 0.04) * cash * 0.08333333333333333);
            put_f64(&mut r.data, 0x12a, f64::from(add) + cash);
        }
        self.log.ran("person_month_step");
    }

    // ---- year (world-tick.md 3.4) --------------------------------------------------------

    fn year_end(&mut self, w: &mut World) {
        self.log.ran("sim_year_end");
        // Items 1-2: company metric average and ai_pick_engine (RNG [80,120] per candidate).
        self.year_reference_speed(w);
        // 3. Buildings (pattern C).
        for slot in pool::list_order(&self.save.objects) {
            self.industry_year_step(w, slot);
        }
        // 4. Companies (pattern C).
        for slot in pool::list_order(&self.save.companies) {
            self.company_annual_review(w, slot);
        }
        self.event_trigger(w, 1);
        self.history_snapshot(w);
        self.year_growth_factor(w);
        for slot in pool::list_order(&self.save.companies) {
            self.company_year_end(w, slot);
        }
        for slot in pool::list_order(&self.save.persons) {
            self.person_year_step(w, slot);
        }
        // 11. Trains (pattern C): shift the yearly statistics.
        if self.v2348() {
            for slot in pool::list_order(&self.save.trains) {
                self.with_trainstep(w, |ts| ts.train_year_step(slot as i32));
                self.log.ran("train_year_step");
            }
        }
        // 12. Territories: `+0x52 (u16) = 0`.
        for slot in pool::list_order(&self.save.territories) {
            if let Some(r) = pool::record_mut(&mut self.save.territories, slot)
                && r.data.len() >= 0x54
            {
                put_u16(&mut r.data, 0x52, 0);
            }
        }
        self.log.ran("territory_year_step");
        if self.hdr(HDR_NO_ECONOMY) == 0 {
            self.town_year_step(w, false);
        }
        // 14. Second weekly pass in the same minute.
        self.week_end(w);
        // 15. Buildings (pattern C).
        for slot in pool::list_order(&self.save.objects) {
            self.industry_year_reset(w, slot);
        }
        // 16. Track piece wear of the year that ends (slots descending; order-free).
        for r in self.save.track_pieces.records.iter_mut().rev() {
            if r.data.len() >= 0x23 {
                let w = u32_at(&r.data, 0x1b);
                put_u32(&mut r.data, 0x1f, w);
                put_u32(&mut r.data, 0x1b, 0);
            }
        }
        self.log.ran("track_piece_wear_rotation");
        // 17. 1 Jan 00:00.
        self.set_clock(w, w.minutes + 1);
        self.year_counters_reset();
        // 18. Industry type inputs for the new year.
        let year = i32::from(date_of(w.minutes).year);
        self.rebuild_types(year);
        self.year_start_messages(year);
        self.event_trigger(w, 0);
    }
}
