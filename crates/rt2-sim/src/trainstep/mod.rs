//! The per-pass train and animation steps of the linear path of `sim_advance`
//! (docs/spec/world-tick.md 2.1, docs/spec/trains.md section 10): `anim_object_step`
//! (`0x4283e0`) for every map animation object and `train_step` (`0x4d0ce0`) for every train.
//!
//! The port follows the original function by function on the raw records of the save
//! ([`mem::Ts`]); each `fn` names the address it reproduces.

macro_rules! todo_hook {
    ($log:expr, $name:literal, $what:literal, $spec:literal) => {
        $log.skip($name, $what, $spec)
    };
}

pub mod anim;
pub mod arrive;
pub mod geom;
pub mod mem;
pub mod motion;
pub mod path;
pub mod periodic;
pub mod station;
pub mod supply;

pub use mem::{Globals, P, Ts};
