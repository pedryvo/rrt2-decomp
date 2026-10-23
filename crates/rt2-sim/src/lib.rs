//! Deterministic simulation core of the Railroad Tycoon II clone.
//!
//! No rendering, UI, I/O, wall clock or hash-map iteration: the same seed and the same command
//! sequence always produce the same state (docs/PLAN.md sections 3, 10 and 11). Numeric rules
//! are bit-exact with the original at 53-bit x87 precision ([`fp`]); each public rule is checked
//! against the original executable by the differential tests in `crates/rt2-oracle/tests/`
//! (`pow`: against the host x87, see `tests/pow.rs`).
//!
//! | Module | Implements | Spec |
//! |---|---|---|
//! | [`rng`] | game LCG (generator A) and seeded hash (generator B) | `docs/spec/rng.md` |
//! | [`calendar`] | date pack/unpack, speed table, game minutes per frame | `docs/spec/time.md` sections 2-3 |
//! | [`effects`] | manager/event/territory effect multiplier | `docs/spec/cargo.md` section 5 |
//! | [`cargo`] | cargo table, delivery revenue, industry input rebuild, production rate and period | `docs/spec/cargo.md` sections 3, 4, 7 |
//! | [`trains`] | segment grade, speed curve, target speed, acceleration, breakdown chance and dice (crash, theft), purchase price, monthly costs | `docs/spec/trains.md` sections 4-6 |
//! | [`track`] | direction/edge-mask types, valid shapes, track tile cost, station catchment | `docs/spec/terrain-track.md` sections 1, 2.2, 3.3 |
//! | [`finance`] | fundamental and trade share price, credit rating, prime and bond rate, cash interest, dividends and the board cap, split, bankruptcy, monthly step | `docs/spec/finance.md` sections 2-6, 8-9 |
//! | [`command`] | `Command` enum and `World::apply` (replay/undo/multiplayer seam) | `docs/spec/time.md` 3 and 5.1 |
//! | [`world`] | state: clock, RNG, speed, plus the save pools when built with `World::from_save` | `docs/spec/time.md` |
//! | [`tick`], [`industry`], [`pool`], [`hooks`] | the world loop (`sim_advance`, day/week/month/year hooks), weekly industry production, slot pools, counters of implemented and missing steps | `docs/spec/world-tick.md`, `cargo.md` 7 |
//! | [`scenario`] | scenario rule engine: condition language (compile, 8-pass reducer), 350-row effect table, rule evaluation, effects, undo rules, victory/loss state; [`scenario::fire_trigger`] is NOT yet called by the world: wire `tick.rs` `event_trigger(k)` (year/month hooks 0..3, track change 4, company created 5, start/load 6, 0, 2 in that order) to it, `docs/spec/world-tick.md` and `scenarios.md` 2.2 | `docs/spec/scenarios.md` |
//! | [`found`] | founding dialog math (sliders, price, shares, "You buy") and command `0x26` | `docs/spec/finance.md` 17 |
//! | [`message`] | output queue of dialogs, headlines and the game outcome | `docs/spec/scenarios.md` 2.4, `time.md` 7.5 |
//! | [`exe`], [`data`] | run-time loader for the copyrighted data tables (cargo, industries, engines, vehicles, track and station tables, stock economy multiplier and company-age weights) | `docs/spec/cargo.md` 3, 7.1; `trains.md` 2; `terrain-track.md` 2.2, 3.3; `finance.md` 3.1 |
//! | [`growth`] | yearly town/region growth, building type pick and site search, removal pick, industry rating | `docs/spec/world-growth.md` |
//! | [`fp`] | `ftol`, f32 stores, `pow` (integer x87 emulation of the CRT `_CIpow`, no libm) | `docs/spec/time.md` 3; `trains.md` 8 |
//!
//! Intentional divergence: the simulation advances with a fixed nominal frame
//! ([`calendar::NOMINAL_FRAME_MS`]) instead of the original's frame-rate dependent step.

pub mod ai;
pub mod build;
pub mod calendar;
pub mod cargo;
pub mod choice;
pub mod command;
pub mod company_record;
pub mod convert;
pub mod data;
pub mod editor;
pub mod effects;
pub mod exe;
pub mod fields;
pub mod finance;
pub mod fincmd;
pub mod found;
pub mod fp;
pub mod growth;
pub mod hooks;
pub mod industry;
pub mod message;
pub mod pool;
pub mod rng;
pub mod scenario;
pub mod tick;
pub mod tilemark;
pub mod track;
pub mod trains;
pub mod trainstep;
pub mod view;
pub mod world;

pub use command::{Command, SpeedChange};
pub use message::{GameMessage, Outcome};
pub use tick::NewGameOptions;
pub use world::World;
