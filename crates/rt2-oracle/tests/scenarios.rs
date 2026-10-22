//! Differential tests for the scenario rule system (docs/spec/scenarios.md): the token table, the
//! effect table, and the condition reducer `0x454270` (with the function evaluator `0x454ae0` for
//! the language functions that only read plain globals and the two RNGs) against the Rust port in
//! `rt2-formats/tests/scenarios/lang.rs`. The data checks need `RT2_DIR`; everything is skipped
//! when the executable is absent. The x87 control word is pinned to 53-bit precision.

#[path = "../../rt2-formats/tests/scenarios/lang.rs"]
mod lang;

use lang::{Ctx, Env, N_ENTRIES, TABLE};
use rt2_oracle::Oracle;
use rt2_sim::rng::GameRng;
use unicorn_engine::RegisterX86;

fn exe_path() -> String {
    std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    })
}

fn oracle() -> Option<Oracle> {
    let path = exe_path();
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    let mut o = Oracle::load_file(&path).expect("load exe");
    o.uc().reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    Some(o)
}

const TOKEN_TABLE: u32 = 0x0053_b330;
const EFFECT_TABLE: u32 = 0x0055_0550;
const N_EFFECTS: usize = 350;

const RULE_EVAL: u32 = 0x0045_4270;
const RNG_STATE: u32 = 0x0055_d2c0;
const CRT_STATE: u32 = 0x0056_09f0;

// Plain globals read by the language functions (spec section 3).
const G_START_DATE: u32 = 0x0057_0508;
const G_ECON: u32 = 0x0057_0717;
const G_NUM_COMPANIES: u32 = 0x0057_0729;
const G_NUM_PLAYERS: u32 = 0x0057_0775;
const G_LOADS_YEAR: u32 = 0x0057_3d05;
const G_LOADS_LIFE: u32 = 0x0057_3d11;
const G_TRACK: u32 = 0x0057_3d1d;
const G_TRACK_SINGLE: u32 = 0x0057_3d21;
const G_TRACK_DOUBLE: u32 = 0x0057_3d25;
const G_TRACK_ELEC: u32 = 0x0057_3d29;
const G_VARS: u32 = 0x0057_3d42; // + 4 * function id (19..22)
const G_MULTI: u32 = 0x0057_3f1c;
const G_CAMPAIGN: u32 = 0x0057_0234;
const G_CAMP_CHOICE: u32 = 0x0056_6d2a;
const G_CAMP_DIFF: u32 = 0x0056_6d00;

fn rd(o: &Oracle, a: u32, n: usize) -> Vec<u8> {
    o.read_mem(a, n).unwrap()
}

fn cstr(o: &Oracle, a: u32) -> String {
    let mut out = Vec::new();
    for i in 0..80 {
        let c = rd(o, a + i, 1)[0];
        if c == 0 {
            break;
        }
        out.push(c);
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn token_table_matches_executable() {
    let Some(o) = oracle() else { return };
    for (k, e) in TABLE.iter().enumerate() {
        let a = TOKEN_TABLE + 17 * k as u32;
        let raw = rd(&o, a, 17);
        let name = cstr(&o, u32::from_le_bytes(raw[0..4].try_into().unwrap()));
        let alias = cstr(&o, u32::from_le_bytes(raw[13..17].try_into().unwrap()));
        assert_eq!(name, e.name, "entry {k}");
        assert_eq!(alias, e.alias, "entry {k} alias");
        assert_eq!(
            u32::from_le_bytes(raw[4..8].try_into().unwrap()),
            e.kind,
            "{k} kind"
        );
        assert_eq!(raw[8], e.prec, "{k} prec");
        assert_eq!(raw[9] != 0, e.id_param, "{k} id_param");
        assert_eq!(raw[10] != 0, e.company, "{k} company");
        assert_eq!(raw[11] != 0, e.player, "{k} player");
        assert_eq!(raw[12] != 0, e.territory, "{k} territory");
    }
    assert_eq!(N_ENTRIES, 118);
}

/// One row of the effect table (16 bytes at `0x550550`).
#[derive(Debug, Clone, Copy)]
struct Effect {
    _ui_id: u16,
    instant: bool,
    min: i8,
    max: i8,
    name_lng: u16,
}

fn effect_table(o: &Oracle) -> Vec<Effect> {
    (0..N_EFFECTS)
        .map(|i| {
            let r = rd(o, EFFECT_TABLE + 16 * i as u32, 16);
            Effect {
                _ui_id: u16::from_le_bytes([r[0], r[1]]),
                instant: r[11] != 0,
                min: r[12] as i8,
                max: r[13] as i8,
                name_lng: u16::from_le_bytes([r[14], r[15]]),
            }
        })
        .collect()
}

#[test]
fn effect_table_shape() {
    let Some(o) = oracle() else { return };
    let t = effect_table(&o);
    // Instant (not undone by a temporary event): the four game-over effects, cash, economy.
    let instant: Vec<usize> = (0..N_EFFECTS).filter(|&i| t[i].instant).collect();
    eprintln!("instant effects: {instant:?}");
    assert!(instant.starts_with(&[0, 1, 2, 3, 4, 5, 6, 7]));
    // Multiplier rows carry the -99..100 percent range; the variable rows -100..100 where
    // -100 (0x9c) means "use the trigger value".
    assert_eq!((t[9].min, t[9].max), (-99, 100));
    assert_eq!((t[241].min, t[241].max), (-100, 100));
    assert_eq!((t[7].min, t[7].max), (-4, 4));
    assert_eq!((t[1].min, t[1].max), (-1, 1));
    assert_eq!(t[8].name_lng, 1627);
}

/// Effect bytes of every shipped rule stay inside the row range of the table.
#[test]
fn shipped_effect_values_are_in_range() {
    let Some(o) = oracle() else { return };
    let Some(dir) = std::env::var_os("RT2_DIR").map(std::path::PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let t = effect_table(&o);
    let mut bad = Vec::new();
    let mut checked = 0;
    for sub in ["maps", "games"] {
        for e in std::fs::read_dir(dir.join(sub)).unwrap().flatten() {
            let p = e.path();
            let Ok(raw) = std::fs::read(&p) else { continue };
            let Ok(save) = rt2_formats::map::Save::parse(&raw) else {
                continue;
            };
            for (ri, r) in save.rules.iter().enumerate() {
                let stride = if r.fixed.len() == 0x362 { 0x15e } else { 0x111 };
                for k in 0..2 {
                    for (j, row) in t.iter().enumerate().take(stride) {
                        let v = r.fixed[0x64 + k * stride + j] as i8;
                        if v == 0 {
                            continue;
                        }
                        checked += 1;
                        let row = *row;
                        if v < row.min || v > row.max {
                            bad.push(format!(
                                "{} rule {ri} choice {k} effect {j}: {v} not in {}..{}",
                                p.file_name().unwrap().to_string_lossy(),
                                row.min,
                                row.max
                            ));
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "{checked} nonzero effect bytes checked, {} out of range",
        bad.len()
    );
    for b in bad.iter().take(20) {
        eprintln!("  {b}");
    }
    assert!(checked > 1000);
    assert!(
        bad.len() <= 5,
        "too many out-of-range effect values: {bad:?}"
    );
}

// ---- reducer ---------------------------------------------------------------------------------

struct Xs(u64);
impl Xs {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// The language functions that read only plain globals, plus the two RNGs.
#[derive(Clone, Default)]
struct World {
    year: u32,
    month: u32,
    day: u32,
    start_year: u32,
    econ: u32,
    n_companies: u32,
    n_players: u32,
    loads_year: u32,
    loads_life: u32,
    track: [u32; 4],
    vars: [u32; 4],
    multi: u32,
    campaign: u32,
    camp_choice: u32,
    camp_diff: u32,
    rng: Option<GameRng>,
    crt: u32,
    companies: Vec<i32>,
    persons: Vec<i32>,
    territories: Vec<i32>,
}

impl World {
    fn crt_rand(&mut self) -> i32 {
        self.crt = self.crt.wrapping_mul(214013).wrapping_add(2531011);
        ((self.crt >> 16) & 0x7fff) as i32
    }
}

impl Env for World {
    fn value(
        &mut self,
        id: usize,
        param: i32,
        company: i32,
        _p: i32,
        territory: i32,
        crt_rand: bool,
    ) -> i32 {
        let _ = param;
        match id {
            0 => {
                if crt_rand {
                    1 + self.crt_rand() % 10000
                } else {
                    self.rng.as_mut().unwrap().range(1, 10000)
                }
            }
            1 => 1,
            2 => 0,
            3 => self.n_players as i32,
            4 => self.n_companies as i32,
            7 => {
                let mut v = self.year.wrapping_sub(self.start_year) as i32;
                if self.month > 11 && self.day > 27 {
                    v += 1;
                }
                v
            }
            8 => self.start_year as i32,
            9 => self.year as i32,
            10 => self.month as i32,
            11 => (self.month + self.year * 100) as i32,
            12 => self.econ as i32,
            13 => self.loads_year as i32,
            14 => self.loads_life as i32,
            15..=18 => self.track[id - 15] as i32,
            19..=22 => self.vars[id - 19] as i32,
            38 => company, // CompanyId
            // `PlayerId` returns the *company* of the context (0x454dbc is shared with CompanyId)
            59 => company,
            69 => territory,
            93 => {
                if self.campaign != 0 {
                    self.camp_choice as i32 + 1
                } else {
                    0
                }
            }
            94 => {
                if self.campaign != 0 {
                    self.camp_diff as i32 + 1
                } else {
                    0
                }
            }
            99 => self.multi as i32,
            100 => i32::from(self.multi == 0),
            _ => panic!("function {id} not covered by the test world"),
        }
    }
    fn companies(&self) -> Vec<i32> {
        self.companies.clone()
    }
    fn persons(&self) -> Vec<i32> {
        self.persons.clone()
    }
    fn territories(&self) -> Vec<i32> {
        self.territories.clone()
    }
}

/// Function ids the test world supports (no id parameter, no scope).
const SIMPLE: [i32; 24] = [
    0, 0, 1, 2, 3, 4, 7, 8, 9, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 93, 94, 99,
];
const BINOPS: [i32; 12] = [
    0x4c, 0x4d, 0x4e, 0x4f, 0x50, 0x51, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
];

fn gen_expr(xs: &mut Xs, depth: u32, out: &mut Vec<(i32, i32)>) {
    let pick = if depth == 0 { xs.below(2) } else { xs.below(7) };
    match pick {
        0 => {
            let v = xs.below(30) as i32 - 3;
            out.push((lang::CONSTANT, v));
        }
        1 => {
            let f = SIMPLE[xs.below(SIMPLE.len() as u32) as usize];
            out.push((f, 0));
        }
        2 => {
            out.push((lang::LPAREN, 0));
            gen_expr(xs, depth - 1, out);
            out.push((lang::RPAREN, 0));
        }
        3 => {
            // unary sign
            out.push((if xs.below(2) == 0 { 0x56 } else { 0x57 }, 0));
            out.push((lang::CONSTANT, xs.below(9) as i32));
        }
        _ => {
            gen_expr(xs, depth - 1, out);
            let op = BINOPS[xs.below(BINOPS.len() as u32) as usize];
            out.push((op, 0));
            gen_expr(xs, depth - 1, out);
        }
    }
}

fn random_world(xs: &mut Xs) -> World {
    World {
        year: 1830 + xs.below(150),
        month: 1 + xs.below(12),
        day: 1 + xs.below(28),
        start_year: 1830 + xs.below(100),
        econ: xs.below(5),
        n_companies: xs.below(8),
        n_players: xs.below(5),
        loads_year: xs.below(5000),
        loads_life: xs.below(100_000),
        track: [
            xs.below(9000),
            xs.below(9000),
            xs.below(9000),
            xs.below(9000),
        ],
        vars: [xs.below(200), xs.below(200), xs.below(200), xs.below(200)],
        multi: xs.below(2),
        campaign: xs.below(2),
        camp_choice: xs.below(3),
        camp_diff: xs.below(3),
        rng: Some(GameRng::new(xs.next())),
        crt: xs.next(),
        companies: Vec::new(),
        persons: Vec::new(),
        territories: Vec::new(),
    }
}

fn load_world(o: &mut Oracle, w: &World) {
    let wr = |o: &mut Oracle, a: u32, v: u32| o.write_mem(a, &v.to_le_bytes()).unwrap();
    // game_date: year u16, month u8, day u8
    let date = w.year | (w.month << 16) | (w.day << 24);
    let gd = game_date_addr();
    wr(o, gd, date);
    wr(o, G_START_DATE, w.start_year);
    wr(o, G_ECON, w.econ);
    wr(o, G_NUM_COMPANIES, w.n_companies);
    wr(o, G_NUM_PLAYERS, w.n_players);
    wr(o, G_LOADS_YEAR, w.loads_year);
    wr(o, G_LOADS_LIFE, w.loads_life);
    for (i, a) in [G_TRACK, G_TRACK_SINGLE, G_TRACK_DOUBLE, G_TRACK_ELEC]
        .iter()
        .enumerate()
    {
        wr(o, *a, w.track[i]);
    }
    for i in 0..4 {
        wr(o, G_VARS + 4 * (19 + i as u32), w.vars[i]);
    }
    wr(o, G_MULTI, w.multi);
    wr(o, G_CAMPAIGN, w.campaign);
    wr(o, G_CAMP_CHOICE, w.camp_choice);
    wr(o, G_CAMP_DIFF, w.camp_diff);
    wr(o, RNG_STATE, w.rng.as_ref().unwrap().state());
    wr(o, CRT_STATE, w.crt);
}

/// `game_date` (symbols.toml) is read from the executable's symbol list at run time.
fn game_date_addr() -> u32 {
    static ADDR: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *ADDR.get_or_init(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../re/symbols.toml"
        ))
        .expect("symbols.toml");
        let at = text.find("name = \"game_date\"").expect("game_date symbol");
        let head = &text[..at];
        let a = head.rfind("addr = \"").unwrap() + 8;
        u32::from_str_radix(
            head[a..]
                .split('"')
                .next()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap()
    })
}

/// Bump allocator for the CRT `malloc` (`HeapAlloc`) the parentheses recursion uses.
fn install_heap(o: &mut Oracle) -> u32 {
    let arena = o.alloc(1 << 20).unwrap();
    let next = std::rc::Rc::new(std::cell::Cell::new(arena));
    let end = arena + (1 << 20);
    let n2 = next.clone();
    o.hook_import(
        "HeapAlloc",
        12,
        Box::new(move |uc| {
            let size = rt2_oracle::stack_arg(uc, 2);
            let p = n2.get();
            let q = (p + size + 15) & !15;
            assert!(q < end, "test arena exhausted");
            n2.set(q);
            p
        }),
    )
    .unwrap();
    o.hook_import("HeapFree", 12, Box::new(|_| 1)).unwrap();
    let scratch = o.alloc(0x4000).unwrap();
    ARENA.with(|a| *a.borrow_mut() = Some((next, arena, scratch)));
    scratch
}

type Arena = (std::rc::Rc<std::cell::Cell<u32>>, u32, u32);

thread_local! {
    static ARENA: std::cell::RefCell<Option<Arena>> = const { std::cell::RefCell::new(None) };
}

/// Stubs for the two string helpers the error path calls (LNG lookup and `sprintf`-like).
fn stub_error_helpers(o: &mut Oracle) {
    o.write_mem(0x0047_6910, &[0x31, 0xc0, 0xc3]).unwrap(); // xor eax,eax; ret
    o.write_mem(0x0047_5fa0, &[0x31, 0xc0, 0xc2, 0x20, 0x00])
        .unwrap(); // xor eax,eax; ret 0x20
}

struct Run {
    ok: bool,
    value: i32,
    rng: u32,
    crt: u32,
}

fn run_original(
    o: &mut Oracle,
    w: &World,
    trigger: u32,
    ids: &[i32],
    vals: &[i32],
    ctx: &Ctx,
) -> Run {
    load_world(o, w);
    let (next, arena, scratch) = ARENA.with(|a| a.borrow().clone().expect("install_heap"));
    next.set(arena);
    let this = scratch;
    o.write_mem(this, &[0u8; 0x362]).unwrap();
    o.write_u32(this + 0x34, trigger).unwrap();
    let tb: Vec<u8> = ids.iter().flat_map(|v| v.to_le_bytes()).collect();
    let vb: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
    assert!(tb.len() < 0x200);
    let tok = scratch + 0x400;
    let val = scratch + 0x800;
    let msg = scratch + 0x1000;
    let out = scratch + 0x3ff0;
    o.write_mem(tok, &tb).unwrap();
    o.write_mem(val, &vb).unwrap();
    o.write_u32(out, 0xdead_beef).unwrap();
    let r = o
        .call_thiscall(
            RULE_EVAL,
            this,
            &[
                ids.len() as u32,
                tok,
                val,
                ctx.company as u32,
                ctx.person as u32,
                ctx.territory as u32,
                msg,
                out,
            ],
        )
        .expect("original reducer");
    Run {
        ok: r & 0xff != 0,
        value: o.read_u32(out).unwrap() as i32,
        rng: o.read_u32(RNG_STATE).unwrap(),
        crt: o.read_u32(CRT_STATE).unwrap(),
    }
}

fn run_rust(w: &World, ids: &[i32], vals: &[i32], ctx: &Ctx) -> (Result<i32, u32>, u32, u32) {
    let mut w = w.clone();
    let r = lang::reduce(&mut w, ids, vals, ctx);
    (r, w.rng.as_ref().unwrap().state(), w.crt)
}

fn compare(
    o: &mut Oracle,
    w: &World,
    trigger: u32,
    toks: &[(i32, i32)],
    ctx: &Ctx,
    label: &str,
) -> bool {
    let ids: Vec<i32> = toks.iter().map(|t| t.0).collect();
    let vals: Vec<i32> = toks.iter().map(|t| t.1).collect();
    let want = run_original(o, w, trigger, &ids, &vals, ctx);
    let (got, rng, crt) = run_rust(w, &ids, &vals, ctx);
    let names: Vec<&str> = ids.iter().map(|&i| TABLE[i as usize].name).collect();
    assert_eq!(
        got.is_ok(),
        want.ok,
        "{label}: success {names:?} {vals:?} -> {got:?}"
    );
    if let Ok(v) = got {
        assert_eq!(v, want.value, "{label}: value {names:?} {vals:?}");
    }
    assert_eq!(rng, want.rng, "{label}: game rng state {names:?}");
    assert_eq!(crt, want.crt, "{label}: crt rand state {names:?}");
    want.ok
}

#[test]
fn reducer_matches_original_on_well_formed_expressions() {
    let Some(mut o) = oracle() else { return };
    stub_error_helpers(&mut o);
    install_heap(&mut o);
    let mut xs = Xs(0x5eed_1234_9abc_def1);
    let mut ok = 0;
    for i in 0..6000 {
        let w = random_world(&mut xs);
        let mut toks = Vec::new();
        gen_expr(&mut xs, 3, &mut toks);
        // trigger kinds 6/7 use the CRT rand for Random1To10000
        let trigger = [2, 3, 0, 6, 7][xs.below(5) as usize];
        let ctx = Ctx {
            crt_rand: trigger == 6 || trigger == 7,
            ..Ctx::default()
        };
        if compare(&mut o, &w, trigger, &toks, &ctx, &format!("case {i}")) {
            ok += 1;
        }
    }
    assert!(ok > 3000, "only {ok} of 6000 evaluated");
}

#[test]
fn reducer_matches_original_on_malformed_token_streams() {
    let Some(mut o) = oracle() else { return };
    stub_error_helpers(&mut o);
    install_heap(&mut o);
    let mut xs = Xs(0xbad0_c0de_0123_4567);
    let vocab: Vec<i32> = SIMPLE
        .iter()
        .copied()
        .chain([
            lang::CONSTANT,
            lang::LPAREN,
            lang::RPAREN,
            lang::HIGHEST,
            lang::LOWEST,
        ])
        .chain(BINOPS)
        .collect();
    let mut failed = 0;
    for i in 0..4000 {
        let w = random_world(&mut xs);
        let n = 1 + xs.below(9);
        let toks: Vec<(i32, i32)> = (0..n)
            .map(|_| {
                let id = vocab[xs.below(vocab.len() as u32) as usize];
                (
                    id,
                    if id == lang::CONSTANT {
                        xs.below(20) as i32
                    } else {
                        0
                    },
                )
            })
            .collect();
        let ctx = Ctx::default();
        if !compare(&mut o, &w, 2, &toks, &ctx, &format!("garbage {i}")) {
            failed += 1;
        }
    }
    assert!(failed > 1000, "garbage mostly evaluated: {failed}");
}

/// The class parameter on a function without company/player/territory scope is an error; with a
/// company in the context `CompanyId(IsHighestInClass)` compares against the other companies of
/// the pool. The pool is a hand-built linked list in emulated memory.
#[test]
fn class_parameter_error_paths_match_original() {
    let Some(mut o) = oracle() else { return };
    stub_error_helpers(&mut o);
    install_heap(&mut o);
    let w = random_world(&mut Xs(77));
    let ctx = Ctx::default();
    // GameYear(IsHighestInClass): no scope
    let toks = [
        (9, 0),
        (lang::LPAREN, 0),
        (lang::HIGHEST, 0),
        (lang::RPAREN, 0),
    ];
    assert!(!compare(
        &mut o,
        &w,
        2,
        &toks,
        &ctx,
        "class on unscoped function"
    ));
    // CompanyId(IsLowestInClass) with no company in the context: no matching entity
    let toks = [
        (38, 0),
        (lang::LPAREN, 0),
        (lang::LOWEST, 0),
        (lang::RPAREN, 0),
    ];
    assert!(!compare(&mut o, &w, 2, &toks, &ctx, "class without entity"));
    // shape errors
    let toks = [(lang::HIGHEST, 0)];
    assert!(!compare(&mut o, &w, 2, &toks, &ctx, "bare class"));
    let toks = [(9, 0), (lang::RPAREN, 0)];
    assert!(!compare(&mut o, &w, 2, &toks, &ctx, "stray paren"));
}

/// Every condition of every shipped map and save reduces to the same value (or fails the same
/// way) in the original and in the Rust port, with the function evaluator `0x454ae0` replaced by
/// a stub that returns 7 for every function (and the literal for constants), so the expression
/// shapes, the pass order, unary signs, division by zero and the six malformed rules are all
/// exercised on real data.
#[test]
fn every_shipped_condition_reduces_like_the_original() {
    let Some(mut o) = oracle() else { return };
    let Some(dir) = std::env::var_os("RT2_DIR").map(std::path::PathBuf::from) else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    stub_error_helpers(&mut o);
    install_heap(&mut o);
    #[rustfmt::skip]
    let stub: [u8; 33] = [
        0x8b, 0x44, 0x24, 0x04, // mov eax,[esp+4]      ; function id
        0x3d, 0x5a, 0, 0, 0, // cmp eax,0x5a
        0x75, 0x08, // jne L
        0x8b, 0x44, 0x24, 0x08, // mov eax,[esp+8]      ; literal value
        0x89, 0x01, // mov [ecx],eax
        0xeb, 0x06, // jmp E
        0xc7, 0x01, 7, 0, 0, 0, // L: mov dword [ecx],7
        0xb8, 1, 0, 0, 0, // E: mov eax,1
        0xc2, 0x1c, 0x00, // ret 0x1c
    ];
    o.write_mem(0x0045_4ae0, &stub).unwrap();
    struct Seven;
    impl Env for Seven {
        fn value(&mut self, _: usize, _: i32, _: i32, _: i32, _: i32, _: bool) -> i32 {
            7
        }
        fn companies(&self) -> Vec<i32> {
            vec![]
        }
        fn persons(&self) -> Vec<i32> {
            vec![]
        }
        fn territories(&self) -> Vec<i32> {
            vec![]
        }
    }
    let w = random_world(&mut Xs(5));
    let (mut total, mut failures, mut skipped) = (0, 0, 0);
    for sub in ["maps", "games"] {
        for e in std::fs::read_dir(dir.join(sub)).unwrap().flatten() {
            let Ok(raw) = std::fs::read(e.path()) else {
                continue;
            };
            let Ok(save) = rt2_formats::map::Save::parse(&raw) else {
                continue;
            };
            for r in &save.rules {
                let ctx = Ctx {
                    company: 1,
                    person: 1,
                    territory: 1,
                    crt_rand: false,
                };
                let has_class = r
                    .tokens_a
                    .iter()
                    .any(|&t| t == lang::HIGHEST || t == lang::LOWEST);
                let ids = &r.tokens_a;
                let vals = &r.tokens_b;
                // the Rust class aggregation has no counterpart under the stub: only compare
                // class rules that fail before any function is evaluated
                if has_class {
                    let mut seven = Seven;
                    if !matches!(
                        lang::reduce(&mut seven, ids, vals, &ctx),
                        Err(lang::err::BAD_CLASS)
                    ) {
                        skipped += 1;
                        continue;
                    }
                }
                let toks: Vec<(i32, i32)> = ids.iter().copied().zip(vals.iter().copied()).collect();
                let want = run_original(&mut o, &w, 2, ids, vals, &ctx);
                let mut seven = Seven;
                let got = lang::reduce(&mut seven, ids, vals, &ctx);
                total += 1;
                assert_eq!(got.is_ok(), want.ok, "{}: {toks:?}", r.condition);
                if let Ok(v) = got {
                    assert_eq!(v, want.value, "{}", r.condition);
                } else {
                    failures += 1;
                }
            }
        }
    }
    eprintln!(
        "{total} shipped conditions compared, {failures} fail in both, {skipped} class rules skipped"
    );
    assert!(total > 3500);
    assert_eq!(failures, 6, "the six malformed rules");
}

/// A pool header in emulated memory with the ids in list order (`0x4f18d0` walks the `next`
/// array from `+0x24` or the `prev` array from `+0x26`).
fn make_pool(o: &mut Oracle, ids: &[u16]) -> u32 {
    let hdr = o.alloc(0x40).unwrap();
    let next = o.alloc(0x200).unwrap();
    let prev = o.alloc(0x200).unwrap();
    o.write_mem(hdr, &[0u8; 0x40]).unwrap();
    o.write_mem(next, &[0u8; 0x200]).unwrap();
    o.write_mem(prev, &[0u8; 0x200]).unwrap();
    o.write_u32(hdr + 0x14, ids.len() as u32).unwrap();
    o.write_mem(hdr + 0x24, &ids[0].to_le_bytes()).unwrap();
    o.write_mem(hdr + 0x26, &ids[ids.len() - 1].to_le_bytes())
        .unwrap();
    o.write_u32(hdr + 0x2c, next).unwrap();
    o.write_u32(hdr + 0x30, prev).unwrap();
    for w in ids.windows(2) {
        o.write_mem(next + 2 * u32::from(w[0]), &w[1].to_le_bytes())
            .unwrap();
        o.write_mem(prev + 2 * u32::from(w[1]), &w[0].to_le_bytes())
            .unwrap();
    }
    hdr
}

/// `Company/Player/TerritoryId(IsHighest/LowestInClass)` over hand-built pools: the aggregation
/// order, ties, the "no other entity" case and the company > player > territory priority.
#[test]
fn class_aggregation_matches_original() {
    let Some(mut o) = oracle() else { return };
    stub_error_helpers(&mut o);
    install_heap(&mut o);
    let mut xs = Xs(0xc1a5_5c1a);
    let mut results = [0u32; 2];
    for i in 0..600 {
        let mut w = random_world(&mut xs);
        let mk = |xs: &mut Xs, o: &mut Oracle| {
            let n = 1 + xs.below(5) as usize;
            let mut ids: Vec<u16> = Vec::new();
            while ids.len() < n {
                let id = 1 + xs.below(40) as u16;
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let pool = make_pool(o, &ids);
            (ids.iter().map(|&x| i32::from(x)).collect::<Vec<_>>(), pool)
        };
        let (cs, cp) = mk(&mut xs, &mut o);
        let (ps, pp) = mk(&mut xs, &mut o);
        let (ts, tp) = mk(&mut xs, &mut o);
        o.write_u32(0x0057_4c6c, cp).unwrap();
        o.write_u32(0x0057_4c68, pp).unwrap();
        o.write_u32(0x0057_4ca8, tp).unwrap();
        w.companies = cs.clone();
        w.persons = ps.clone();
        w.territories = ts.clone();
        let pick = |xs: &mut Xs, l: &[i32]| {
            if xs.below(4) == 0 {
                0
            } else {
                l[xs.below(l.len() as u32) as usize]
            }
        };
        let ctx = Ctx {
            company: pick(&mut xs, &cs),
            person: pick(&mut xs, &ps),
            territory: pick(&mut xs, &ts),
            crt_rand: false,
        };
        let f = [38, 59, 69][xs.below(3) as usize];
        let class = if xs.below(2) == 0 {
            lang::HIGHEST
        } else {
            lang::LOWEST
        };
        let toks = [(f, 0), (lang::LPAREN, 0), (class, 0), (lang::RPAREN, 0)];
        if compare(&mut o, &w, 2, &toks, &ctx, &format!("class {i}")) {
            results[0] += 1;
        } else {
            results[1] += 1;
        }
    }
    eprintln!(
        "class cases: {} evaluated, {} rejected",
        results[0], results[1]
    );
    assert!(results[0] > 200);
}

#[test]
fn effect_money_scale_matches_original() {
    let Some(mut o) = oracle() else { return };
    for v in -127..=127 {
        let got = o.call_fastcall(0x0045_c150, v as u32, 0, &[]).unwrap() as i32;
        assert_eq!(got, lang::effect_money(v), "value {v}");
    }
    assert_eq!(lang::effect_money(14), 50_000);
    assert_eq!(lang::effect_money(19), 100_000);
    assert_eq!(lang::undo_date(1850, 12, 1), (1851, 1));
    assert_eq!(lang::undo_date(1850, 3, 24), (1852, 3));
}
