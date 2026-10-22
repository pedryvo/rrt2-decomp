//! Bookkeeping of the world loop: how many times each documented step ran and which steps are
//! still missing.
//!
//! The world loop (docs/spec/world-tick.md) calls every step of the original in the documented
//! order. A step that is not implemented yet is an explicit [`todo_hook!`](crate::todo_hook): it
//! changes nothing, but it is counted with the spec item that documents it, so a run reports
//! exactly what it skipped (the fidelity scoreboard, `docs/fidelity-status.md`). Implemented
//! steps are counted too ([`HookLog::ran`]); the counts of a run can be compared with the ones
//! observed in the original (world-tick.md section 6). Counters live in `BTreeMap`s: the
//! iteration order is deterministic.

use std::collections::BTreeMap;

/// A step that was reached but is not implemented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// What the original does there (function and address).
    pub what: &'static str,
    /// The spec item that documents it.
    pub spec: &'static str,
    /// Times the world loop reached it.
    pub hits: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HookLog {
    ran: BTreeMap<&'static str, u64>,
    skipped: BTreeMap<&'static str, Skipped>,
}

impl HookLog {
    /// Counts one execution of an implemented step.
    pub fn ran(&mut self, name: &'static str) {
        *self.ran.entry(name).or_insert(0) += 1;
    }

    /// Counts `n` executions of an implemented step.
    pub fn ran_n(&mut self, name: &'static str, n: u64) {
        *self.ran.entry(name).or_insert(0) += n;
    }

    /// Counts one hit of a step that is not implemented.
    pub fn skip(&mut self, name: &'static str, what: &'static str, spec: &'static str) {
        self.skipped
            .entry(name)
            .or_insert(Skipped {
                what,
                spec,
                hits: 0,
            })
            .hits += 1;
    }

    /// Executions of an implemented step.
    pub fn count(&self, name: &str) -> u64 {
        self.ran.get(name).copied().unwrap_or(0)
    }

    /// All implemented steps and their counts, by name.
    pub fn ran_steps(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        self.ran.iter().map(|(k, v)| (*k, *v))
    }

    /// The steps reached but not implemented, by name.
    pub fn skipped_steps(&self) -> impl Iterator<Item = (&'static str, &Skipped)> + '_ {
        self.skipped.iter().map(|(k, v)| (*k, v))
    }
}

/// Marks a step of the original that is not implemented yet: `todo_hook!(log, "name", "what the
/// original does (address)", "spec file and section")`.
#[macro_export]
macro_rules! todo_hook {
    ($log:expr, $name:literal, $what:literal, $spec:literal) => {
        $log.skip($name, $what, $spec)
    };
}
