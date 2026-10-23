//! Computer opponents (docs/spec/ai.md): the monthly AI turn of `sim_month_end` (step 7,
//! `ai_monthly_turn` `0x436d60`) on the raw records of the save.
//!
//! The port follows the original function by function on the train step's view of the state
//! ([`Ts`]): each `fn` names the address it reproduces, and the order of the game RNG draws is
//! the one of ai.md section 11. The two runtime globals of the original that hold the current
//! company (`ai_cur_company_ptr` `0x565140`, `ai_cur_company_id` `0x565144`) are the
//! [`Ai::company`] field; the stop-selection ratio `0x57951c` is [`Ai::ratio`].
//!
//! Entry point for the world loop: [`monthly_turn`], called from `sim_month_end` inside the
//! train-step view (`with_trainstep`), between the lot expiry and the companies' monthly step
//! (world-tick.md 3.3 item 7).
//!
//! What is not ported is a counted [`todo_hook!`](crate::todo_hook) that cites the spec item;
//! the RNG consequence of each gap is documented there (ai.md "Em aberto").

macro_rules! todo_hook {
    ($log:expr, $name:literal, $what:literal, $spec:literal) => {
        $log.skip($name, $what, $spec)
    };
}

pub mod build;
pub mod engine;
pub mod finance;
pub mod industry;
pub mod network;
pub mod path;
pub mod person;
pub mod site;
pub mod trains;

use crate::calendar::minutes_to_date;
use crate::trainstep::{P, Ts};

/// `ai_persona_table` (`0x551e88`): 41 records of 0x23 bytes, indexed by `person+0x52f` (i8).
pub const PERSONA_TABLE: u32 = 0x0055_1e88;
pub const PERSONA_SIZE: u32 = 0x23;

/// Persona tendencies (ai.md section 3), offsets in the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trait {
    StartsCompany = 0x03,
    MarginBuying = 0x07,
    ShortSelling = 0x0b,
    InvestingInOthers = 0x0f,
    IssueBonds = 0x13,
    ExpandRail = 0x17,
    IndustryInvestment = 0x1b,
}

// Company record offsets used by the AI (ai.md section 1, finance.md section 1).
pub const CO_CHAIRMAN: usize = 0x23;
pub const CO_TRAINS: usize = 0x27;
pub const CO_SHARES: usize = 0x2c;
pub const CO_TRAIN_STAMP: usize = 0x154;
pub const CO_SATISFIED: usize = 0x166;
pub const CO_ACTIVE: usize = 0x16e;
pub const CO_STATIONS: usize = 0x172;
pub const CO_MAIN_STATION: usize = 0x176;
pub const CO_NETWORK: usize = 0x17a;
pub const CO_DEADLINE: usize = 0x17e;
pub const CO_NET_STATIONS: usize = 0x1aa;
pub const CO_MANAGER_YEAR: usize = 0x422f;
pub const CO_BOX: usize = 0x4233;
pub const CO_ENGINE_YEAR: usize = 0x4243;
pub const CO_REPLAN_YEAR: usize = 0x4245;
pub const CO_INDUSTRY_STAMP: usize = 0x424c;
pub const CO_ELECTRIC: usize = 0x4264;
pub const CO_CASH: usize = 0x4766;

/// Header byte of a global at `va` (header block = `0x567b38..`).
pub(crate) const HDR_BASE: u32 = 0x0056_7b38;
/// State byte of a global at `va` (state block = `0x5704e8..`).
const ST_BASE: u32 = 0x0057_04e8;

/// The AI's view of the world during a turn.
pub struct Ai<'t, 'a> {
    pub ts: &'t mut Ts<'a>,
    /// `ai_cur_company_id` (`0x565144`); 0 outside a company turn.
    pub company: i32,
    /// `0x57951c`: the "served / offered" ratio of the stop choice (ai.md 7).
    pub ratio: f32,
    /// `0x566a40` / `0x566a44`: foreign / total tiles of the last route found with
    /// `0x566a5c` set (ai.md 7.1).
    pub route_foreign: i32,
    pub route_total: i32,
    /// Track globals read by `track_tile_cost`: `track_single` (`0x5534b0`),
    /// `track_bridge_type` (`0x53ce18`), `track_electric` (`0x566fcc`). During the AI turn
    /// `build_preview_suspend` (`0x46b150`) sets them to double track, wooden bridges, not
    /// electric; `track_lay_route` (`0x4cc060`) overwrites them with its arguments.
    pub track_single: bool,
    pub track_bridge: i32,
    pub track_electric: bool,
    /// `track_edge_base` (`0x566a64`) and `track_elec_base` (`0x566a60`).
    pub edge_base: i32,
    pub elec_base: i32,
    pub tables: Tables,
    /// Commands queued by the turn (`cmd_queue` `0x4af6d0`), executed by the command step of
    /// the next frame.
    pub commands: Vec<AiCommand>,
    /// The earthwork log of `0x426250` (`0x5650f8` vertex indices, `0x5650fc` old heights,
    /// `0x565100` count; recording while `0x565104` is set), see [`crate::build::level`].
    pub elev_log: Option<Vec<(i32, i32)>>,
    /// `0x5794f0`: the station networks must be rebuilt at the next month end (set by
    /// `track_lay_route` when there are 20 stations or more).
    pub networks_dirty: bool,
    /// The pending redraw rectangle of `0x417770` (flushed by
    /// [`flush_redraw`](crate::ai::Ai::flush_redraw)).
    pub dirty_rect: Option<(i32, i32, i32, i32)>,
    /// Scenario triggers raised by the primitives (`event_fire_trigger(k)`, e.g. 4 at the end
    /// of `station_networks_rebuild`), for the caller to fire in order.
    pub events: Vec<u8>,
    /// Track-overlay presence of a rectangle before a change (x0, y0, x1, y1, cells), see
    /// [`crate::build::tiles`].
    pub overlay_snapshot: Option<(i32, i32, i32, i32, Vec<bool>)>,
    /// Per tile (`y * W + x`): the overlay list holds a ground blend (layer 0), when the caller
    /// knows it (`SimState::ground_layer`); read by [`crate::build::tiles`] to tell whether a
    /// tile keeps overlays once its track overlays are gone.
    pub ground_layer: Option<Vec<bool>>,
    /// `0x574b80` / `0x574b78` raised (the water fill of a flood, `0x490490` mode 3): the track
    /// removal of a rectangle (`0x4caaa0`) skips the network rebuild and the train refresh.
    pub suppress_networks: bool,
    /// `0x5746ec`: the AI turn is running (station track is laid double, the build A* adds the
    /// slope penalty).
    pub ai_turn: bool,
    /// The language file, for the names the primitives write (stations).
    pub lng: Option<std::rc::Rc<rt2_formats::lng::Lng>>,
    /// Persons whose turn reached `ai_person_trade_stocks` (`0x4a8af0`), in order: the trading
    /// needs the finance commands of the world loop, which runs it after the persons' phase
    /// (the trades draw no RNG and touch nothing the later founding tests read).
    pub traders: Vec<i32>,
}

/// A command the AI queues instead of acting at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiCommand {
    /// Command `0x2d` with flag 1 (`building_buy_request` `0x43ea30`): buy an industry,
    /// executed by `company_industry_trade` (`0x43eb60`), see [`apply_command`].
    BuyIndustry { company: i32, industry: i32 },
    /// `event_fire_trigger(k)` raised inside the turn (e.g. 4 by `station_networks_rebuild`
    /// after a construction); fired by the caller after the turn (the original fires it at
    /// once, which only matters for scenario rules on that trigger).
    FireTrigger(u8),
}

/// Executes a queued AI command (the command step of the original).
pub fn apply_command(ts: &mut Ts<'_>, cmd: AiCommand) {
    match cmd {
        AiCommand::BuyIndustry { company, industry } => {
            industry::buy_industry(ts, company, industry);
        }
        AiCommand::FireTrigger(_) => {}
    }
}

/// Tables of the executable the AI reads through the shared rules.
#[derive(Clone, Debug)]
pub struct Tables {
    pub track: crate::track::TrackTables,
    pub finance: crate::finance::FinanceTables,
}

impl Tables {
    /// Builds the tables from the data image (the same values `TrackTables::load` and
    /// `FinanceTables::load` read from the executable).
    pub fn from_image(img: &crate::data::DataImage) -> Tables {
        use crate::track::*;
        let i32s = |va: u32, n: usize| {
            (0..n)
                .map(|i| img.i32(va + 4 * i as u32))
                .collect::<Vec<_>>()
        };
        let shapes = |va: u32| {
            let mut v = [false; 256];
            for (i, ok) in v.iter_mut().enumerate() {
                *ok = img.u8(va + 2 * i as u32) != b'c';
            }
            v
        };
        let mut terrain_mult = [0f32; TERRAIN_CLASSES];
        for (c, m) in terrain_mult.iter_mut().enumerate() {
            *m = img.f32(ADDR_TERRAIN_CLASS + c as u32 * TERRAIN_CLASS_STRIDE);
        }
        let footprints = (0..BUILDING_TYPES as u32)
            .map(|t| {
                let base = ADDR_BUILDING_TYPES + t * BUILDING_RECORD_SIZE + FOOTPRINT_OFFSET;
                let mut g = [0u8; 100];
                for row in 0..10u32 {
                    for col in 0..10u32 {
                        g[(row * 10 + col) as usize] = img.u8(base - row * 20 - col * 2);
                    }
                }
                Footprint(g)
            })
            .collect();
        let track = TrackTables {
            bridge_cost: i32s(ADDR_BRIDGE_COST, 12).try_into().expect("12"),
            terrain_mult,
            econ_track: std::array::from_fn(|i| img.f32(ADDR_ECON_TRACK + 4 * i as u32)),
            station_radius: i32s(ADDR_STATION_RADIUS, 3).try_into().expect("3"),
            shape_valid: shapes(ADDR_EDGE_SHAPES),
            shape_valid_bridge: shapes(ADDR_EDGE_SHAPES_BRIDGE),
            footprints,
        };
        let mut age_weights = [[0i32; 4]; 5];
        for (a, row) in age_weights.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = img.i32(crate::finance::ADDR_AGE_WEIGHTS + 4 * (a * 4 + j) as u32);
            }
        }
        let finance = crate::finance::FinanceTables {
            econ_stock: std::array::from_fn(|i| {
                img.f32(crate::finance::ADDR_ECON_STOCK + 4 * i as u32)
            }),
            age_weights,
        };
        Tables { track, finance }
    }
}

impl Drop for Ai<'_, '_> {
    /// The ratio `0x57951c` is a global of the original: it survives the session.
    fn drop(&mut self) {
        self.ts.g.ai_ratio = self.ratio.to_bits();
    }
}

impl<'t, 'a> Ai<'t, 'a> {
    /// End of a session on the world: the queued commands plus the scenario triggers raised
    /// by the primitives, and the deferred network rebuild flag (`0x5794f0`) kept in the train
    /// step globals.
    pub fn finish(&mut self) -> Vec<AiCommand> {
        if self.networks_dirty {
            self.ts.g.networks_dirty = true;
        }
        let mut out = std::mem::take(&mut self.commands);
        out.extend(self.events.drain(..).map(AiCommand::FireTrigger));
        out
    }

    pub fn new(ts: &'t mut Ts<'a>) -> Self {
        let tables = Tables::from_image(ts.img);
        let elec = ts.img.i32(0x0055_34c0);
        let ratio = f32::from_bits(ts.g.ai_ratio);
        Ai {
            ts,
            company: 0,
            ratio,
            route_foreign: 0,
            route_total: 0,
            track_single: false,
            track_bridge: 1,
            track_electric: false,
            edge_base: 2000,
            elec_base: elec,
            tables,
            commands: Vec::new(),
            elev_log: None,
            networks_dirty: false,
            dirty_rect: None,
            events: Vec::new(),
            overlay_snapshot: None,
            ground_layer: None,
            suppress_networks: false,
            ai_turn: false,
            lng: None,
            traders: Vec::new(),
        }
    }

    // ---- globals ---------------------------------------------------------------------------

    /// A byte of the header block (globals `0x567b38..0x5704c2`).
    pub fn hdr8(&self, va: u32) -> u8 {
        self.ts
            .s
            .header
            .get((va - HDR_BASE) as usize)
            .copied()
            .unwrap_or(0)
    }

    /// A byte of the global state block (`0x5704e8..`).
    pub fn st8(&self, va: u32) -> u8 {
        self.ts
            .s
            .state
            .get((va - ST_BASE) as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn st_i32(&self, va: u32) -> i32 {
        self.ts.state_i32((va - ST_BASE) as usize)
    }

    pub fn st_i16(&self, va: u32) -> i16 {
        let o = (va - ST_BASE) as usize;
        i16::from_le_bytes([self.ts.s.state[o], self.ts.s.state[o + 1]])
    }

    pub fn st_f32(&self, va: u32) -> f32 {
        f32::from_bits(self.st_i32(va) as u32)
    }

    /// `game_time_minutes` (`0x5704f4`).
    pub fn now(&self) -> u32 {
        self.ts.now
    }

    /// `game_date` year (`0x5704f8`).
    pub fn year(&self) -> i32 {
        i32::from(self.ts.year)
    }

    /// `game_date` month (`0x5704fa`), 1..=12.
    pub fn month(&self) -> i32 {
        i32::from(minutes_to_date(self.ts.now as i32).month)
    }

    /// Scenario permission byte (`0x5706aa..0x5706c4`, ai.md section 1).
    pub fn perm(&self, va: u32) -> bool {
        self.st8(va) != 0
    }

    /// "Track Expansion" AI option (`0x5704ad`).
    pub fn track_expansion(&self) -> u8 {
        self.hdr8(0x0057_04ad)
    }

    /// "Dividend Payout / Stock Buyback" AI option (`0x5704ae`).
    pub fn dividend_policy(&self) -> u8 {
        self.hdr8(0x0057_04ae)
    }

    // ---- records -----------------------------------------------------------------------------

    pub fn co_i32(&self, o: usize) -> i32 {
        self.ts.i32(P::Company, self.company, o)
    }
    pub fn co_u32(&self, o: usize) -> u32 {
        self.ts.u32(P::Company, self.company, o)
    }
    pub fn co_set_i32(&mut self, o: usize, v: i32) {
        let c = self.company;
        self.ts.seti32(P::Company, c, o, v);
    }
    pub fn co_f64(&self, o: usize) -> f64 {
        crate::pool::f64_at(&self.ts.record(P::Company, self.company), o)
    }

    /// `company_account(c, 0)` (`0x43a060`): the cash.
    pub fn cash(&self) -> f64 {
        self.co_f64(CO_CASH)
    }

    /// Persona tendency of a person (`ai_persona_table[person+0x52f]`).
    pub fn persona(&self, person: i32, t: Trait) -> i32 {
        let idx = i32::from(self.ts.i8(P::Person, person, 0x52f));
        self.ts.img.i32(
            PERSONA_TABLE
                .wrapping_add((idx * PERSONA_SIZE as i32) as u32)
                .wrapping_add(t as u32),
        )
    }

    /// The chairman of the current company (0 = none).
    pub fn chairman(&self) -> i32 {
        self.co_i32(CO_CHAIRMAN)
    }

    /// `effect_sum(e, company, 0, 0)` (`0x46b5d0`): global + company + its chairman.
    pub fn effect_sum(&self, e: usize, company: i32) -> i32 {
        let mut v = i32::from(self.st_i16(0x0057_07c6 + 2 * e as u32));
        if company != 0 && self.ts.exists(P::Company, company) {
            v += i32::from(self.ts.i16(P::Company, company, 0x1ae + 2 * e));
            let p = self.ts.i32(P::Company, company, CO_CHAIRMAN);
            if p != 0 && self.ts.exists(P::Person, p) {
                v += i32::from(self.ts.i16(P::Person, p, 0x26b + 2 * e));
            }
        }
        v
    }

    /// Ids of a pool in list order (pattern C: `list_nth(i)` for `i < count`).
    pub fn ids(&self, p: P) -> Vec<i32> {
        crate::pool::list_order(crate::trainstep::mem::pool_of(self.ts.s, p))
            .into_iter()
            .map(|s| s as i32)
            .collect()
    }

    /// `int_dist` (`0x4f0cd0`): `ftol(sqrt(dx*dx + dy*dy))` on the integer sum of squares.
    pub fn dist(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        let (dx, dy) = (bx.wrapping_sub(ax), by.wrapping_sub(ay));
        let s = dy.wrapping_mul(dy).wrapping_add(dx.wrapping_mul(dx));
        crate::fp::ftol(f64::from(s).sqrt())
    }

    /// `station_usable_by(st, c)` (`0x4c01b0`): own station, or in the company's main network.
    pub fn station_usable(&self, st: i32, company: i32) -> bool {
        if self.ts.i32(P::Station, st, 0x39) == company {
            return true;
        }
        let net = self.ts.u32(P::Company, company, CO_NETWORK);
        u32::from(self.ts.u16(P::Station, st, 0x57)) == net
    }
}

/// `ai_monthly_turn` (`0x436d60`, ai.md section 1): every AI person (`+0x122 == 0`) in list
/// order, then every company without a chairman or with an AI chairman. Skipped with the
/// "no AI" flag `0x570229` (and the runtime cheat `0x5748fc`, 0 in a loaded game). Returns the
/// commands the turn queued (apply them with [`apply_command`] at the next command step).
pub fn monthly_turn(ts: &mut Ts<'_>) -> Vec<AiCommand> {
    let (_, mut cmds) = monthly_turn_persons(ts);
    cmds.extend(monthly_turn_companies(ts));
    cmds
}

/// The persons' phase of [`monthly_turn`]: returns the persons that trade stocks
/// ([`Ai::traders`], in order) and the queued commands.
pub fn monthly_turn_persons(ts: &mut Ts<'_>) -> (Vec<i32>, Vec<AiCommand>) {
    if ts.flag_570229 != 0 {
        return (Vec::new(), Vec::new());
    }
    ts.log.ran("ai_monthly_turn");
    // build_preview_suspend / resume (0x46b150, 0x46b290): UI preview layer, no state.
    let mut ai = Ai::new(ts);
    ai.ai_turn = true;
    // Pattern C over the persons: the count is re-read every iteration.
    let mut i = 0;
    loop {
        let ids = ai.ids(P::Person);
        if i >= ids.len() {
            break;
        }
        let p = ids[i];
        if ai.ts.i32(P::Person, p, 0x122) == 0 {
            ai.person_turn(p);
        }
        i += 1;
    }
    let traders = std::mem::take(&mut ai.traders);
    (traders, ai.finish())
}

/// The companies' phase of [`monthly_turn`]: every company without a chairman or with an AI
/// chairman.
pub fn monthly_turn_companies(ts: &mut Ts<'_>) -> Vec<AiCommand> {
    if ts.flag_570229 != 0 {
        return Vec::new();
    }
    let mut ai = Ai::new(ts);
    ai.ai_turn = true;
    let mut i = 0;
    loop {
        let ids = ai.ids(P::Company);
        if i >= ids.len() {
            break;
        }
        let c = ids[i];
        let chair = ai.ts.i32(P::Company, c, CO_CHAIRMAN);
        if chair == 0 || ai.ts.i32(P::Person, chair, 0x122) == 0 {
            ai.company_turn(c);
        }
        i += 1;
    }
    ai.finish()
}

/// `ai_company_turn` (`0x436b30`) on its own, for tests and for the founding path. Returns the
/// queued commands.
pub fn company_turn(ts: &mut Ts<'_>, company: i32) -> Vec<AiCommand> {
    let mut ai = Ai::new(ts);
    ai.company_turn(company);
    ai.finish()
}

impl Ai<'_, '_> {
    /// `ai_company_turn` (`0x436b30`, ai.md 1 and 2).
    pub fn company_turn(&mut self, id: i32) {
        self.company = id;
        self.ai_turn = true;
        self.ts.log.ran("ai_company_turn");
        if self.co_i32(CO_ACTIVE) != 0 {
            self.company_turn_active();
        }
        self.company = 0;
    }

    fn company_turn_active(&mut self) {
        self.co_set_i32(CO_SATISFIED, 0);
        let year = self.year();
        if self.co_u32(CO_MANAGER_YEAR) != year as u32 && year & 1 != 0 {
            self.review_manager();
        }
        let c = self.company;
        self.ts.set8(P::Company, c, CO_ELECTRIC, 0);
        let e = self.pick_engine(1);
        if e >= 0 && self.ts.en8(e, 0x28) == 2 {
            self.ts.set8(P::Company, c, CO_ELECTRIC, 1);
        }
        if self.perm(0x0057_06b5)
            && self.perm(0x0057_06b6)
            && self.perm(0x0057_06b7)
            && self.co_u32(CO_DEADLINE) < self.now()
        {
            let r = self.ts.rng.range(180, 360);
            let r1440 = r.wrapping_mul(1440);
            let chair = self.chairman();
            let delay = if chair == 0 {
                r1440.wrapping_add(r1440)
            } else {
                let x = 300 - self.persona(chair, Trait::ExpandRail);
                crate::fp::ftol(f64::from(x) * f64::from(r1440) * 0.005)
            };
            let dl = self.now().wrapping_add(delay as u32);
            self.co_set_i32(CO_DEADLINE, dl as i32);
            if self.co_i32(CO_NET_STATIONS) < 1 {
                self.plan_first_line(false);
                if self.co_i32(CO_NET_STATIONS) < 1 {
                    return;
                }
                self.co_set_i32(CO_TRAIN_STAMP, 0);
            } else if !self.extend_network() {
                let dl = self.co_i32(CO_DEADLINE).wrapping_add(0x15180);
                self.co_set_i32(CO_DEADLINE, dl);
            } else {
                self.co_set_i32(CO_TRAIN_STAMP, 0);
            }
        }
        let stamp = self.co_u32(CO_TRAIN_STAMP);
        if stamp == 0 || stamp.wrapping_add(0x1d880) < self.now() {
            let now = self.now() as i32;
            self.co_set_i32(CO_TRAIN_STAMP, now);
            self.manage_trains();
        }
        if self.month() >= 8 {
            let s = self.co_u32(CO_INDUSTRY_STAMP);
            if s == 0 || s.wrapping_add(0x3b100) < self.now() {
                self.industry_investment();
                let now = self.now() as i32;
                self.co_set_i32(CO_INDUSTRY_STAMP, now);
            }
        }
        let c = self.company;
        self.company_ai_finance(c);
    }
}
