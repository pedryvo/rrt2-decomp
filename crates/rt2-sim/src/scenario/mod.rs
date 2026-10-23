//! Scenario rule engine (docs/spec/scenarios.md): condition language (tokenizer, compiler,
//! 8-pass reducer), the 350-row effect table, rule evaluation with message and effects, temporary
//! effects through undo rules, and the victory/loss state.
//!
//! - [`table`]: the 118-entry token table (`0x53b330`).
//! - [`lang`]: words, compile, reduce, function evaluator and the [`ScenarioEnv`] bindings.
//! - [`rule`]: [`Rule`] (decoded from the map/save record), [`EffectTable`] (loaded from the
//!   executable at `0x550550`), [`Scenario`] pool and [`GameOver`] state.
//! - [`engine`]: [`fire_trigger`] (`event_fire_trigger` `0x457fe0`), [`evaluate_rule`]
//!   (`rule_evaluate` `0x455f30`) and the [`ScenarioWorld`] trait the world implements.
//!
//! Determinism: the game RNG is passed explicitly and drawn only by `Random1To10000`, in text
//! order, once per evaluated combination, without short-circuit (spec 3.4).

pub mod edit;
pub mod engine;
pub mod lang;
pub mod rule;
pub mod table;

pub use engine::{
    Combo, Confiscate, Message, ScenarioWorld, Scope, evaluate_rule, fire_trigger, rights,
    substitute, trigger,
};
pub use lang::{
    CompileError, Ctx, Eval, ScenarioEnv, compile, effect_money, err, function_value, lookup,
    loop_flags, parse_number, reduce, split_words, undo_date,
};
pub use rule::{
    ADDR_EFFECT_TABLE, EffectRow, EffectTable, GameOver, MessageKind, N_EFFECTS, Rule, Scenario,
};
pub use table::{CONSTANT, Entry, HIGHEST, LOWEST, LPAREN, N_ENTRIES, RPAREN, TABLE};
