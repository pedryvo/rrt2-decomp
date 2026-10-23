//! What the scenario editor needs of the rule engine (docs/spec/scenarios.md 2.1, 3 and 4):
//! live validation of a condition text (the compile and the test evaluation of
//! `rule_set_condition` `0x457a30`), building a rule record from the edited fields while keeping
//! the bytes the editor does not model, and the effect rows with their ranges.

use rt2_formats::map;

use super::lang::{CompileError, Ctx, Eval, ScenarioEnv, compile, err, loop_flags, reduce};
use super::rule::{EffectTable, N_EFFECTS, Rule};
use crate::rng::GameRng;

/// Why a condition is refused, with the LNG string id of the original's message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// A word that is neither a name of the language nor a number (LNG 0x7a first word, 0x7b).
    UnknownWord { word: String, first: bool },
    /// The reducer's error (`lang::err`).
    Shape(u32),
}

impl Invalid {
    /// LNG id of the message the original shows.
    pub fn lng(&self) -> u32 {
        match self {
            Invalid::UnknownWord { first: true, .. } => 0x7a,
            Invalid::UnknownWord { .. } => 0x7b,
            Invalid::Shape(id) => *id,
        }
    }
}

/// An environment where every function is 0 and one company, player and territory exist, so a
/// class parameter finds an entity.
struct Probe;

impl ScenarioEnv for Probe {
    fn value(&mut self, _: usize, _: i32, _: i32, _: i32, _: i32) -> i32 {
        0
    }
    fn crt_range(&mut self, lo: i32, _: i32) -> i32 {
        lo
    }
    fn random(&mut self, _: &mut GameRng, _: bool) -> i32 {
        1
    }
    fn companies(&self) -> Vec<i32> {
        vec![1, 2]
    }
    fn persons(&self) -> Vec<i32> {
        vec![1, 2]
    }
    fn territories(&self) -> Vec<i32> {
        vec![1, 2]
    }
}

/// Compiles and test-evaluates a condition text. An empty text is valid (it reduces to nothing
/// and the editor leaves the rule without a condition).
pub fn validate(text: &str) -> Result<(), Invalid> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let (ids, vals) = compile(text).map_err(|CompileError::UnknownWord(w)| {
        let first = text.split_whitespace().next().is_some_and(|f| f == w);
        Invalid::UnknownWord { word: w, first }
    })?;
    let mut env = Probe;
    let mut rng = GameRng::new(1);
    let mut ev = Eval::new(
        &mut env,
        &mut rng,
        Ctx {
            company: 1,
            person: 1,
            territory: 1,
            crt_rand: false,
        },
    );
    reduce(&mut ev, &ids, &vals)
        .map(|_| ())
        .map_err(Invalid::Shape)
}

/// Text of the editor's error line (English fallback when the LNG lacks the id).
pub fn describe(e: &Invalid) -> String {
    match e {
        Invalid::UnknownWord { word, .. } => format!("Unknown word: {word}"),
        Invalid::Shape(id) => match *id {
            err::BAD_PARAM => "A function needs a number in parentheses: Name(n)".into(),
            err::NO_PARAM => "This function takes no parameter".into(),
            err::LEFTOVER => "Leftover words: join conditions with AND / OR".into(),
            err::BAD_CLASS => "Class parameter must read Function(IsHighestInClass)".into(),
            err::STRAY_RPAREN => "Unmatched )".into(),
            err::UNBALANCED => "Unmatched (".into(),
            err::OPERAND => "An operator is missing an operand".into(),
            err::EMPTY_PARENS => "Empty parentheses".into(),
            err::CLASS_NO_SCOPE => {
                "This function has no company, player or territory to rank".into()
            }
            err::CLASS_NO_ENTITY => "Nothing to rank against here".into(),
            other => format!("Error {other:#x}"),
        },
    }
}

/// A rule the editor adds: no condition yet, trigger "end of month", every player and company
/// tested, permanent effects (spec 2.1, "regra nova do editor").
pub fn new_rule() -> Rule {
    let mut r = Rule::from_condition("", 3).unwrap_or_default();
    r.once = true;
    r
}

/// Offsets of the fixed part the editor models (the others are kept from the base record).
const MODELED: [(usize, usize); 22] = [
    (0x04, 4),
    (0x24, 4),
    (0x28, 4),
    (0x2c, 8),
    (0x34, 4),
    (0x38, 4),
    (0x3c, 4),
    (0x40, 4),
    (0x44, 4),
    (0x48, 4),
    (0x60, 4),
    (0x64, 2 * 0x15e),
    (0x320, 4),
    (0x324, 4),
    (0x328, 4),
    (0x32a, 4),
    (0x32e, 4),
    (0x34f, 3),
    (0x352, 16),
    (0x4c, 4),
    (0x10, 0x10),
    (0x35e, 4),
];

/// The file record of `rule`: the bytes of `base` (the rule as the file had it) with the modelled
/// fields replaced, the derived scope flags (`+0x2c`, `+0x30`, `+0x320`) refreshed from the
/// condition like `0x457a30`, and the texts and token arrays rewritten. Without a base (a new
/// rule) the record is the fresh one of [`Rule::to_file`].
pub fn build(base: Option<&map::Rule>, rule: &Rule) -> map::Rule {
    let mut fresh = rule.to_file();
    let (c, p, t) = loop_flags(&rule.tokens);
    for (o, v) in [(0x2c, c), (0x30, p), (0x320, t)] {
        fresh.fixed[o..o + 4].copy_from_slice(&u32::from(v).to_le_bytes());
    }
    let Some(base) = base.filter(|b| b.fixed.len() == fresh.fixed.len()) else {
        return fresh;
    };
    let mut out = base.clone();
    for (o, n) in MODELED {
        out.fixed[o..o + n].copy_from_slice(&fresh.fixed[o..o + n]);
    }
    out.condition = fresh.condition;
    out.tokens_a = fresh.tokens_a;
    out.tokens_b = fresh.tokens_b;
    out.message = fresh.message;
    out.text2 = fresh.text2;
    out.text3 = fresh.text3;
    out.title = fresh.title;
    out.raw_blobs = fresh.raw_blobs;
    out
}

/// Sets the condition text of a rule: compiles it, refreshes the tokens and the three loop flags
/// (`+0x24`, `+0x28`, `+0x324` follow the functions used, as `0x457a30` does).
pub fn set_condition(rule: &mut Rule, text: &str) -> Result<(), Invalid> {
    validate(text)?;
    let (ids, vals) = compile(text).unwrap_or_default();
    let (c, p, t) = loop_flags(&ids);
    rule.condition = text.to_owned();
    rule.tokens = ids;
    rule.values = vals;
    rule.per_company = c;
    rule.per_player = p;
    rule.per_territory = t;
    Ok(())
}

/// Effect rows shown by the editor: visible rows with a name, as (row, min, max, instant).
pub fn effect_rows(table: &EffectTable) -> Vec<(usize, i8, i8, bool)> {
    table
        .rows
        .iter()
        .enumerate()
        .take(N_EFFECTS)
        .filter(|(_, r)| r.visible && r.name_lng != 0)
        .map(|(i, r)| (i, r.min, r.max, r.instant))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_and_bad_conditions() {
        assert_eq!(
            validate("GameYear>=1850 and CompanyCash(IsHighestInClass)"),
            Ok(())
        );
        assert_eq!(validate("1 + 2 * 3 > 5"), Ok(()));
        assert!(matches!(
            validate("Bogus > 1"),
            Err(Invalid::UnknownWord { first: true, .. })
        ));
        assert_eq!(
            validate("GameYear > 1 )"),
            Err(Invalid::Shape(err::STRAY_RPAREN))
        );
        assert_eq!(
            validate("(GameYear > 1"),
            Err(Invalid::Shape(err::UNBALANCED))
        );
        assert_eq!(
            validate("GameYear > 1 GameYear < 3"),
            Err(Invalid::Shape(err::LEFTOVER))
        );
    }

    #[test]
    fn set_condition_derives_the_loop_flags() {
        let mut r = new_rule();
        set_condition(&mut r, "CompanyCash > 5").unwrap();
        assert!(r.per_company && !r.per_player);
        assert_eq!(r.tokens.len(), 3);
    }
}
