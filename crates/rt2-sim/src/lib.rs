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
//! | [`command`] | `Command` enum and `World::apply` (replay/undo/multiplayer seam) | `docs/spec/time.md` 3 and 5.1 |
//! | [`world`] | minimal state: clock, RNG, speed | `docs/spec/time.md` |
//! | [`exe`], [`data`] | run-time loader for the copyrighted data tables (cargo, industries, engines, vehicles, track and station tables) | `docs/spec/cargo.md` 3, 7.1; `trains.md` 2; `terrain-track.md` 2.2, 3.3 |
//! | [`fp`] | `ftol`, f32 stores, `pow` (integer x87 emulation of the CRT `_CIpow`, no libm) | `docs/spec/time.md` 3; `trains.md` 8 |
//!
//! Intentional divergence: the simulation advances with a fixed nominal frame
//! ([`calendar::NOMINAL_FRAME_MS`]) instead of the original's frame-rate dependent step.

pub mod calendar;
pub mod cargo;
pub mod command;
pub mod data;
pub mod effects;
pub mod exe;
pub mod fp;
pub mod rng;
pub mod track;
pub mod trains;
pub mod world;

pub use command::{Command, SpeedChange};
pub use world::World;
