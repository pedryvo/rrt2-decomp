//! Scenario condition language of `RT2_PLAT.EXE` (docs/spec/scenarios.md section 3): the word
//! splitter (`0x453e70`), the compiler (`0x454060`), the precedence-pass reducer (`0x454270`) and
//! the function evaluator with the class parameter (`0x454ae0`). The values of the language
//! functions come from a [`ScenarioEnv`] supplied by the world.
//!
//! The two random sources are passed explicitly: `Random1To10000` draws the game RNG
//! (`rng_range(1, 10000)`, spec 3.4) or, in a local evaluation, the CRT `rand` through
//! [`ScenarioEnv::crt_range`]. Functions are all evaluated left to right before any operator, so
//! `AND`/`OR` never short-circuit and every `Random1To10000` of the text draws, in order.

use super::table::{CONSTANT, HIGHEST, LOWEST, LPAREN, RPAREN, TABLE};
use crate::rng::GameRng;

/// Sentinel the original uses for "no value yet" in the class aggregation.
const UNSET: i32 = 0x3b8d_6c1f;

/// Index of a word in the table (case-insensitive, name or alias; first match wins).
pub fn lookup(word: &str) -> Option<usize> {
    TABLE.iter().position(|e| {
        e.name.eq_ignore_ascii_case(word)
            || (!e.alias.is_empty() && e.alias.eq_ignore_ascii_case(word))
    })
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_cmp(c: u8) -> bool {
    matches!(c, b'>' | b'<' | b'=')
}

/// `0x453e70`: whitespace-separated words, additionally split before/after `( ) + - * /` and where
/// the character class changes between comparison characters (`> < =`) and anything else.
pub fn split_words(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        while pos < b.len() && is_ws(b[pos]) {
            pos += 1;
        }
        if pos >= b.len() || b[pos] == 0 {
            break;
        }
        let start = pos;
        let mut p = pos;
        let mut first = true;
        while p < b.len() && b[p] != 0 {
            let c = b[p];
            if is_ws(c) {
                break;
            }
            if !first {
                let prev = b[p - 1];
                if matches!(c, b'(' | b')' | b'+' | b'-' | b'*' | b'/')
                    || matches!(prev, b'(' | b')' | b'+' | b'-' | b'*' | b'/')
                    || is_cmp(prev) != is_cmp(c)
                {
                    break;
                }
            }
            p += 1;
            first = false;
        }
        out.push(String::from_utf8_lossy(&b[start..p]).into_owned());
        pos = p;
    }
    out
}

/// `0x4ef3f0`: decimal, or hexadecimal after a `0x` prefix; any `-` negates; wrapping `i32`.
/// `None` for any other character (the original returns 0 and the word is rejected).
pub fn parse_number(s: &str) -> Option<i32> {
    let mut v: i32 = 0;
    let mut neg = false;
    let mut base: i32 = 10;
    for &c in s.as_bytes() {
        if c == b'-' {
            neg = true;
        } else if c.is_ascii_digit() {
            v = v.wrapping_mul(base).wrapping_add(i32::from(c - b'0'));
        } else {
            let up = if c.is_ascii_lowercase() || c > 0xef {
                c.wrapping_sub(0x20)
            } else {
                c
            };
            if up == b'X' && v == 0 && base == 10 {
                base = 16;
            } else if base != 16 || !(b'A'..=b'F').contains(&up) {
                return None;
            } else {
                v = v.wrapping_mul(16).wrapping_add(i32::from(up) - 0x37);
            }
        }
    }
    Some(if neg { v.wrapping_neg() } else { v })
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompileError {
    /// A word that is neither a table entry nor a number (LNG 0x7a for the first word, 0x7b else).
    UnknownWord(String),
}

/// `0x454060`: words to (token ids, values). Numbers become [`CONSTANT`] with their value.
pub fn compile(text: &str) -> Result<(Vec<i32>, Vec<i32>), CompileError> {
    let mut ids = Vec::new();
    let mut vals = Vec::new();
    for w in split_words(text) {
        if let Some(k) = lookup(&w) {
            ids.push(k as i32);
            vals.push(0);
        } else if let Some(n) = parse_number(&w) {
            ids.push(CONSTANT);
            vals.push(n);
        } else {
            return Err(CompileError::UnknownWord(w));
        }
    }
    Ok((ids, vals))
}

/// The three loop flags `0x457a30` derives from the tokens: (companies, players, territories).
pub fn loop_flags(ids: &[i32]) -> (bool, bool, bool) {
    let mut f = (false, false, false);
    for &i in ids {
        let e = &TABLE[i as usize];
        f.0 |= e.company;
        f.1 |= e.player;
        f.2 |= e.territory;
    }
    f
}

/// Source of function values and entity lists (pool order) for the evaluator.
///
/// Implemented by the world (see `ScenarioWorld`); the implementation reads the state listed in
/// docs/spec/scenarios.md 3.5 for each function id. Entities are ids with 0 meaning "none".
pub trait ScenarioEnv {
    /// Value of function `id` (index in [`TABLE`], never 0 `Random1To10000` and never the
    /// constant) for the given entities (0 = none). Functions whose data needs an absent entity
    /// return 0.
    fn value(&mut self, id: usize, param: i32, company: i32, person: i32, territory: i32) -> i32;
    /// `crt_rand_range(lo, hi)` (`0x4f0790`) on the CRT generator, used by local evaluations.
    fn crt_range(&mut self, lo: i32, hi: i32) -> i32;
    /// `Random1To10000`: `rng_range(1, 10000)` on the game RNG, or the CRT generator when the
    /// evaluation is local (spec 3.3). Override only to stub the draw in tests.
    fn random(&mut self, rng: &mut GameRng, crt: bool) -> i32 {
        if crt {
            self.crt_range(1, 10000)
        } else {
            rng.range(1, 10000)
        }
    }
    /// Every company / person / territory of the pool, in list order (class parameter scan).
    fn companies(&self) -> Vec<i32>;
    fn persons(&self) -> Vec<i32>;
    fn territories(&self) -> Vec<i32>;
}

/// Evaluation context of one rule evaluation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    pub company: i32,
    pub person: i32,
    pub territory: i32,
    /// Trigger 6/7 or a local-player-only rule: `Random1To10000` uses the CRT `rand`.
    pub crt_rand: bool,
}

/// Evaluator state: the environment, the game RNG and the "last company-territory-to-territory
/// loads value" that `(COMPANY_T2T_LOADS)` prints (`_DAT_005665dc`, spec 2.4).
pub struct Eval<'a> {
    pub env: &'a mut dyn ScenarioEnv,
    pub rng: &'a mut GameRng,
    pub ctx: Ctx,
    pub last_t2t: i32,
}

impl<'a> Eval<'a> {
    pub fn new(env: &'a mut dyn ScenarioEnv, rng: &'a mut GameRng, ctx: Ctx) -> Self {
        Eval {
            env,
            rng,
            ctx,
            last_t2t: 0,
        }
    }

    /// One function value; ids 91, 92 and 105..=109 (loads matrix functions) record the value
    /// for `(COMPANY_T2T_LOADS)`.
    fn call(&mut self, id: usize, param: i32, company: i32, person: i32, territory: i32) -> i32 {
        if id == 0 {
            return self.env.random(self.rng, self.ctx.crt_rand);
        }
        let v = self.env.value(id, param, company, person, territory);
        if matches!(id, 91 | 92 | 105..=109) {
            self.last_t2t = v;
        }
        v
    }
}

/// LNG string ids of the evaluation errors.
pub mod err {
    /// Parameter list malformed: `name ( number )` expected.
    pub const BAD_PARAM: u32 = 0x898;
    /// Parameter given to a function that takes none.
    pub const NO_PARAM: u32 = 0x899;
    /// Tokens left over after all passes.
    pub const LEFTOVER: u32 = 0x89a;
    /// Class parameter not in `name ( class )` shape.
    pub const BAD_CLASS: u32 = 0x7c;
    pub const STRAY_RPAREN: u32 = 0x7d;
    pub const UNBALANCED: u32 = 0x7e;
    /// Binary operator without operands.
    pub const OPERAND: u32 = 0x7f;
    pub const EMPTY_PARENS: u32 = 0x80;
    /// Class parameter on a function with no company/player/territory scope.
    pub const CLASS_NO_SCOPE: u32 = 0x81;
    /// Class parameter without a matching entity in the context.
    pub const CLASS_NO_ENTITY: u32 = 0xb2b;
}

/// `0x454ae0`: value of one function token, optionally with a class parameter.
pub fn function_value(
    ev: &mut Eval,
    id: usize,
    param: i32,
    class: Option<i32>,
) -> Result<i32, u32> {
    let e = &TABLE[id];
    let ctx = ev.ctx;
    if id == CONSTANT as usize {
        return Ok(param);
    }
    let Some(class) = class else {
        return Ok(ev.call(id, param, ctx.company, ctx.person, ctx.territory));
    };
    if class != HIGHEST && class != LOWEST {
        return if e.id_param {
            Ok(ev.call(id, param, ctx.company, ctx.person, ctx.territory))
        } else {
            Err(err::NO_PARAM)
        };
    }
    if !(e.company || e.player || e.territory) {
        return Err(err::CLASS_NO_SCOPE);
    }
    // Priority company > player > territory, each only if the context has that entity.
    let (dim, own, list) = if ctx.company != 0 && e.company {
        (0, ctx.company, ev.env.companies())
    } else if ctx.person != 0 && e.player {
        (1, ctx.person, ev.env.persons())
    } else if ctx.territory != 0 && e.territory {
        (2, ctx.territory, ev.env.territories())
    } else {
        return Err(err::CLASS_NO_ENTITY);
    };
    let mut acc = UNSET;
    let mut own_v = UNSET;
    for x in list {
        let (c, p, t) = match dim {
            0 => (x, ctx.person, ctx.territory),
            1 => (ctx.company, x, ctx.territory),
            _ => (ctx.company, ctx.person, x),
        };
        let v = ev.call(id, param, c, p, t);
        if x == own {
            own_v = v;
        } else if acc == UNSET || (class == HIGHEST && acc <= v) || (class == LOWEST && v <= acc) {
            acc = v;
        }
    }
    if acc == UNSET {
        return Ok(1);
    }
    Ok(i32::from(if class == HIGHEST {
        acc < own_v
    } else {
        own_v < acc
    }))
}

/// `0x454270`: reduces the token list by precedence passes 0..7 and returns the single value
/// left. Functions are all evaluated (in text order) before any operator, so `AND`/`OR` do not
/// short-circuit and every `Random1To10000` draws.
pub fn reduce(ev: &mut Eval, ids_in: &[i32], vals_in: &[i32]) -> Result<i32, u32> {
    let mut ids = ids_in.to_vec();
    let mut vals = vals_in.to_vec();
    let mut pass: u8 = 0;
    let mut i: usize = 0;
    loop {
        if i >= ids.len() {
            pass += 1;
            if pass > 7 {
                return if ids.len() == 1 {
                    Ok(vals[0])
                } else {
                    Err(err::LEFTOVER)
                };
            }
            i = 0;
            continue;
        }
        let id = ids[i] as usize;
        let e = &TABLE[id];
        if e.prec != pass {
            i += 1;
            continue;
        }
        let n = ids.len();
        match e.kind {
            0 => {
                let mut param = vals[i];
                if e.id_param {
                    if i + 3 >= n
                        || ids[i + 2] != CONSTANT
                        || ids[i + 1] != LPAREN
                        || ids[i + 3] != RPAREN
                    {
                        return Err(err::BAD_PARAM);
                    }
                    param = vals[i + 2];
                    vals[i] = param;
                    ids.drain(i + 1..i + 4);
                    vals.drain(i + 1..i + 4);
                }
                vals[i] = function_value(ev, id, param, None)?;
                ids[i] = CONSTANT;
            }
            1 | 4 => {
                let prev_const = i > 0 && ids[i - 1] == CONSTANT;
                let next_const = i + 1 < n && ids[i + 1] == CONSTANT;
                if ((!prev_const) && id != 0x56 && id != 0x57) || !next_const {
                    return Err(err::OPERAND);
                }
                let (l, r) = if prev_const {
                    (vals[i - 1], vals[i + 1])
                } else {
                    (0, vals[i + 1])
                };
                let b = |x: bool| i32::from(x);
                let res = match id {
                    0x4c => b(r <= l),
                    0x4d => b(l <= r),
                    0x4e => b(r < l),
                    0x4f => b(l < r),
                    0x50 => b(l == r),
                    0x51 => b(l != r),
                    0x54 => b(l != 0 && r != 0),
                    0x55 => b(l != 0 || r != 0),
                    0x56 | 0x57 => {
                        let s = if id == 0x56 { 1 } else { -1 };
                        if !prev_const {
                            // unary: [op, const] -> const
                            vals[i] = r.wrapping_mul(s);
                            ids[i] = CONSTANT;
                            ids.remove(i + 1);
                            vals.remove(i + 1);
                            if ids.len() == 1 {
                                return Ok(vals[0]);
                            }
                            continue;
                        }
                        r.wrapping_mul(s).wrapping_add(l)
                    }
                    0x58 => l.wrapping_mul(r),
                    0x59 => l.wrapping_div(if r != 0 { r } else { 1 }),
                    _ => 0,
                };
                // binary: [l, op, r] -> res at i-1
                ids[i - 1] = CONSTANT;
                vals[i - 1] = res;
                ids.drain(i..i + 2);
                vals.drain(i..i + 2);
                if ids.len() == 1 {
                    return Ok(vals[0]);
                }
                // the original resumes at i-1 and then increments, i.e. at i
                continue;
            }
            2 => {
                let mut depth = 1;
                let mut j = i + 1;
                loop {
                    if j >= n {
                        return Err(err::UNBALANCED);
                    }
                    match TABLE[ids[j] as usize].kind {
                        2 => depth += 1,
                        3 => depth -= 1,
                        _ => {}
                    }
                    if depth == 0 {
                        break;
                    }
                    j += 1;
                }
                let len = j - i - 1;
                if len == 0 {
                    return Err(err::EMPTY_PARENS);
                }
                let v = reduce(ev, &ids[i + 1..j], &vals[i + 1..j])?;
                ids[i] = CONSTANT;
                vals[i] = v;
                ids.drain(i + 1..j + 1);
                vals.drain(i + 1..j + 1);
                if ids.len() == 1 {
                    return Ok(vals[0]);
                }
            }
            3 => return Err(err::STRAY_RPAREN),
            5 => {
                if i < 2
                    || i + 1 >= n
                    || TABLE[ids[i - 2] as usize].kind != 0
                    || ids[i - 1] != LPAREN
                    || ids[i + 1] != RPAREN
                {
                    return Err(err::BAD_CLASS);
                }
                let f = ids[i - 2] as usize;
                let v = function_value(ev, f, vals[i - 2], Some(id as i32))?;
                vals[i - 2] = v;
                ids[i - 2] = CONSTANT;
                ids.drain(i - 1..i + 2);
                vals.drain(i - 1..i + 2);
                if ids.len() == 1 {
                    return Ok(vals[0]);
                }
                i -= 2;
            }
            _ => {}
        }
        i += 1;
    }
}

/// `0x45c150`: slider byte of the money effects (Company Cash, Player Cash, ...) to dollars.
/// 1..=10 -> 1000 steps, 11..=59 -> 10000 steps from 20 000, 60..=104 -> 100 000 steps from
/// 600 000, above -> 1 000 000 steps from 6 000 000; sign preserved (0 and negatives negative).
pub fn effect_money(v: i32) -> i32 {
    let a = v.wrapping_abs();
    let sign = if v < 1 { -1 } else { 1 };
    if a < 0xb {
        sign * a * 1000
    } else if a < 0x3c {
        (a - 9) * sign * 10_000
    } else if a < 0x69 {
        (a - 0x36) * sign * 100_000
    } else {
        (a - 99) * sign * 1_000_000
    }
}

/// Date at which the undo rule of a temporary effect fires (`0x455f30`, end): `months` after the
/// current (year, month 1..=12), as (year, month).
pub fn undo_date(year: i32, month: i32, months: i32) -> (i32, i32) {
    let t = months + year * 12 - 1 + month;
    (t / 12, t % 12 + 1)
}
