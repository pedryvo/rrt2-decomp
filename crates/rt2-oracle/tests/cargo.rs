//! Differential tests for the cargo economy (docs/spec/cargo.md): the manager/event effect
//! multiplier, the industry demand-weight rebuild and the delivery revenue formula. Skipped when
//! the executable is absent. The x87 control word is pinned to 53-bit precision (CW 0x27f); see
//! `revenue_depends_on_x87_precision` for the sensitivity to that assumption.

use rt2_oracle::Oracle;
use rt2_sim::cargo::{
    CargoType, Consumer, Delivery, IndustryType, RevenueTables, delivery_revenue,
};
use rt2_sim::data::GameData;
use rt2_sim::effects::{EffectLevels, effect_multiplier};
use rt2_sim::exe::Exe;
use unicorn_engine::RegisterX86;

/// Game data loaded by rt2-sim from the executable file (the implementation under test).
fn sim_data() -> GameData {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    GameData::load(&Exe::from_bytes(std::fs::read(path).expect("read exe")).expect("parse exe"))
        .expect("load tables")
}

fn oracle() -> Option<Oracle> {
    let path = std::env::var("RT2_EXE").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../re/bin/RT2_PLAT.EXE").to_string()
    });
    if !std::path::Path::new(&path).exists() {
        eprintln!("skipping: {path} not found (set RT2_EXE)");
        return None;
    }
    Some(Oracle::load_file(&path).expect("load exe"))
}

fn set_fpcw(o: &mut Oracle, cw: u64) {
    o.uc().reg_write(RegisterX86::FPCW, cw).unwrap();
}

/// Fixed-seed xorshift32 used only to generate test inputs.
struct Xs(u32);
impl Xs {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u32) as i32
    }
    fn chance(&mut self, pct: u32) -> bool {
        self.next() % 100 < pct
    }
}

fn ftol(x: f64) -> i32 {
    (x.trunc() as i64) as i32
}

// ---- memory helpers --------------------------------------------------------------------------

fn w8(o: &mut Oracle, a: u32, v: u8) {
    o.write_mem(a, &[v]).unwrap();
}
fn w16(o: &mut Oracle, a: u32, v: u16) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn w32(o: &mut Oracle, a: u32, v: u32) {
    o.write_u32(a, v).unwrap();
}
fn wf(o: &mut Oracle, a: u32, v: f32) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}

/// A list object as seen by `0x4f18b0`: record `id` lives at `base + id * stride`
/// (`+8` = stride, `+0x28` = base pointer); id 0 yields a null pointer.
#[derive(Clone, Copy)]
struct List {
    base: u32,
    stride: u32,
    n: u32,
}
impl List {
    fn new(o: &mut Oracle, global: u32, stride: u32, n: u32) -> Self {
        let base = o.alloc(stride * (n + 1)).unwrap();
        let hdr = o.alloc(0x40).unwrap();
        w32(o, hdr + 8, stride);
        w32(o, hdr + 0x28, base);
        w32(o, global, hdr);
        List { base, stride, n }
    }
    fn rec(&self, id: u32) -> u32 {
        self.base + id * self.stride
    }
    fn clear(&self, o: &mut Oracle) {
        o.write_mem(self.base, &vec![0u8; (self.stride * (self.n + 1)) as usize])
            .unwrap();
    }
}

// ---- 1. effect multiplier (0x46b480) ----------------------------------------------------------

const EFFECT: u32 = 0x0046_b480; // fastcall(ECX = effect, EDX = company id) + [person, territory], ret 8; ST0
const EFFECT_GLOBAL: u32 = 0x0057_07c6; // i16[effect]
const G_COMPANIES: u32 = 0x0057_4c6c;
const G_PERSONS: u32 = 0x0057_4c68;
const G_TERRITORIES: u32 = 0x0057_4ca8;
const COMPANY_FX: u32 = 0x1ae; // i16[effect] in the company record
const PERSON_FX: u32 = 0x26b; // i16[effect] in the person record
const TERR_FX: u32 = 0x58; // i16[effect] in the territory record
const FX_N: usize = 0x130;

type Fx = [i16; FX_N];

/// Effect tables visible to `0x46b480`: global, the company (with its own person id), persons and
/// territories by id.
struct FxWorld<'a> {
    global: &'a Fx,
    company: Option<(&'a Fx, u32)>,
    persons: &'a dyn Fn(u32) -> Option<Fx>,
    terrs: &'a dyn Fn(u32) -> Option<Fx>,
}

/// Resolves the records like the original does and calls `rt2_sim::effects::effect_multiplier`
/// (the implementation under test). `person` is the explicit person argument (0 = use the
/// company's).
fn effect_ref(e: usize, company: u32, person: u32, terr: u32, w: &FxWorld) -> f32 {
    let mut levels = EffectLevels {
        global: w.global[e],
        ..Default::default()
    };
    let mut pid = person;
    if company != 0
        && let Some((cfx, cperson)) = w.company
    {
        levels.company = Some(cfx[e]);
        if person == 0 {
            pid = cperson;
        }
    }
    if pid != 0
        && let Some(pfx) = (w.persons)(pid)
    {
        levels.person = Some(pfx[e]);
    }
    if terr != 0
        && let Some(tfx) = (w.terrs)(terr)
    {
        levels.territory = Some(tfx[e]);
    }
    effect_multiplier(levels)
}

fn rand_fx(xs: &mut Xs, density: u32) -> Fx {
    let mut f = [0i16; FX_N];
    for v in f.iter_mut() {
        if xs.chance(density) {
            *v = match xs.below(4) {
                0 => xs.range(-130, -90) as i16,
                1 => xs.range(-30, 30) as i16,
                2 => xs.range(0, 150) as i16,
                _ => xs.range(-32768, 32767) as i16,
            };
        }
    }
    f
}

fn write_fx(o: &mut Oracle, addr: u32, f: &Fx) {
    let b: Vec<u8> = f.iter().flat_map(|v| v.to_le_bytes()).collect();
    o.write_mem(addr, &b).unwrap();
}

#[test]
fn effect_multiplier_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let companies = List::new(&mut o, G_COMPANIES, 0x600, 3);
    let persons = List::new(&mut o, G_PERSONS, 0x600, 3);
    let terrs = List::new(&mut o, G_TERRITORIES, 0x400, 3);
    let mut xs = Xs(0xc0ff_ee11);
    for case in 0..6000 {
        let density = [3, 20, 90][xs.below(3) as usize];
        let g = rand_fx(&mut xs, density);
        let cfx = rand_fx(&mut xs, density);
        let pfx = rand_fx(&mut xs, density);
        let tfx = rand_fx(&mut xs, density);
        write_fx(&mut o, EFFECT_GLOBAL, &g);
        companies.clear(&mut o);
        persons.clear(&mut o);
        terrs.clear(&mut o);
        let cperson = xs.below(3);
        write_fx(&mut o, companies.rec(1) + COMPANY_FX, &cfx);
        w32(&mut o, companies.rec(1) + 0x23, cperson);
        write_fx(&mut o, persons.rec(1) + PERSON_FX, &pfx);
        write_fx(
            &mut o,
            persons.rec(2) + PERSON_FX,
            &rand_fx(&mut xs, density),
        );
        write_fx(&mut o, terrs.rec(1) + TERR_FX, &tfx);
        let e = xs.below(FX_N as u32) as usize;
        let company = xs.below(2);
        let person = xs.below(3);
        let terr = xs.below(2);
        let p2 = {
            let b = o.read_mem(persons.rec(2) + PERSON_FX, FX_N * 2).unwrap();
            let mut f = [0i16; FX_N];
            for (i, c) in b.chunks(2).enumerate() {
                f[i] = i16::from_le_bytes([c[0], c[1]]);
            }
            f
        };
        let want = effect_ref(
            e,
            company,
            person,
            terr,
            &FxWorld {
                global: &g,
                company: Some((&cfx, cperson)),
                persons: &|id| match id {
                    1 => Some(pfx),
                    2 => Some(p2),
                    _ => Some([0; FX_N]),
                },
                terrs: &|id| (id == 1).then_some(tfx),
            },
        );
        o.call_fastcall(EFFECT, e as u32, company, &[person, terr])
            .unwrap();
        let got = o.st0().unwrap();
        assert_eq!(
            got,
            f64::from(want),
            "effect {e:#x} company {company} person {person} terr {terr} case {case}"
        );
    }
}

// ---- 2. industry input list and demand weights (0x471880) ---------------------------------------

const IND_TABLE: u32 = 0x0054_1ae0; // [2 sets][53 types][0x90]
const IND_SET_STRIDE: u32 = 53 * 0x90;
const IND_REBUILD: u32 = 0x0047_1880; // fastcall(ECX = type)
const GAME_DATE: u32 = 0x0057_04f8;
const GAME_TIME: u32 = 0x0057_04f4;

#[test]
fn industry_input_rebuild_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let pristine = o
        .read_mem(IND_TABLE, (2 * IND_SET_STRIDE) as usize)
        .unwrap();
    for year in [
        1800, 1830, 1860, 1877, 1884, 1900, 1904, 1905, 1910, 1920, 1930, 1958, 1960, 1990, 2020,
    ] {
        w32(&mut o, GAME_DATE, year);
        o.write_mem(IND_TABLE, &pristine).unwrap();
        for ty in 0..53u32 {
            o.call_fastcall(IND_REBUILD, ty, 0, &[]).unwrap();
            for set in 0..2u32 {
                let off = (set * IND_SET_STRIDE + ty * 0x90) as usize;
                // implementation under test: rt2_sim::cargo::IndustryType::rebuild_inputs
                let mut want = IndustryType::from_raw(&pristine[off..off + 0x90]);
                want.rebuild_inputs(year as i32);
                let got = IndustryType::from_raw(
                    &o.read_mem(IND_TABLE + set * IND_SET_STRIDE + ty * 0x90, 0x90)
                        .unwrap(),
                );
                assert_eq!(got, want, "year {year} set {set} type {ty}");
            }
        }
    }
}

// ---- 3. delivery revenue (0x4bf760) --------------------------------------------------------------

const REVENUE: u32 = 0x004b_f760; // thiscall(ECX = destination station) (lot, &R, &flag, &industry, commit) ret 0x14
const CARGO_TABLE: u32 = 0x0054_04b1; // 46 records of 0x21 bytes
const CARGO_N: usize = 46;
const G_LOTS: u32 = 0x0057_4c7c;
const G_CARS: u32 = 0x0057_4c70;
const G_TRAINS: u32 = 0x0057_4c64;
const G_STATIONS: u32 = 0x0057_4c60;
const G_INDUSTRIES: u32 = 0x0057_4c74;
const G_LINKS: u32 = 0x0057_4c58;
const ECON_STATE: u32 = 0x0057_0717;
const MODEL_WORD: u32 = 0x0057_3d78; // byte 0 = industrial model 0..2
const DEMAND_LEVELS_ON: u32 = 0x0057_3d76; // 1 when the model is 2 (expert)
const SLIDERS: u32 = 0x0057_4874; // u8 human, u8 computer revenue sliders

/// What `0x471b10` does to the cargo table at start-up: `price = ftol(price * 1.4)`,
/// `+8 = days * 1440`.
fn init_cargo_table(o: &mut Oracle) {
    for i in 0..CARGO_N as u32 {
        let r = CARGO_TABLE + 0x21 * i;
        let price = o.read_u32(r).unwrap() as i32;
        let days = o.read_u32(r + 4).unwrap() as i32;
        w32(o, r, ftol(f64::from(price) * 1.4) as u32);
        w32(o, r + 8, days.wrapping_mul(0x5a0) as u32);
    }
}

const TBL_DEMAND_LEVEL: u32 = 0x0055_0524; // f32[10] 0.5 .. 1.4
const TBL_UNDEMANDED: u32 = 0x0055_050c; // f32[2][3]: computer 0.8 0.55 0.4, human 0.8 0.5 0.2
const TBL_ECON_REVENUE: u32 = 0x0055_04bc; // f32[5] 0.8 .. 1.2
const TBL_SLIDER_HUMAN: u32 = 0x0055_0448; // f32[11] 0.75 .. 1.25
const TBL_SLIDER_AI: u32 = 0x0055_041c; // f32[11] 1.55 .. 2.8

fn read_f32s(o: &Oracle, a: u32, n: usize) -> Vec<f32> {
    o.read_mem(a, n * 4)
        .unwrap()
        .chunks(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[derive(Clone)]
struct Ind {
    id: u16,
    ty: u8,
    stock: [f32; 8],
}

#[derive(Clone)]
struct Sc {
    year: u16,
    now: u32,
    state: usize,
    model: u8,
    lo: u8,
    hi: u8,
    cargo: usize,
    lot_id: u32,
    lot_src: u16,
    lot_origin: u16,
    t_created: u32,
    t_loaded: u32,
    s_id: u32,
    d_id: u32,
    s_xy: (i32, i32),
    d_xy: (i32, i32),
    s_upgrades: u32,
    d_demanded: bool,
    d_level: u8,
    d_terr: u32,
    train_flags: u8,
    c_person: u32,
    human: bool,
    c_m: f32,
    c_mult: f32,
    inds: Vec<Ind>,
    g_fx: Fx,
    c_fx: Fx,
    p_fx: Fx,
    t_fx: Fx,
}

/// Adapter to the implementation under test, `rt2_sim::cargo::delivery_revenue`.
/// Returns (revenue, flag, industry id).
fn revenue_ref(sc: &Sc, env: &Env) -> (i32, u32, u32) {
    let c = sc.cargo;
    let fxw = FxWorld {
        global: &sc.g_fx,
        company: Some((&sc.c_fx, sc.c_person)),
        persons: &|_| Some(sc.p_fx),
        terrs: &|_| Some(sc.t_fx),
    };
    let eff = |e: usize| effect_ref(e, 1, 0, sc.d_terr, &fxw);
    let consumers: Vec<Consumer> = sc
        .inds
        .iter()
        .map(|i| Consumer {
            id: i.id,
            kind: i.ty,
            inputs: env.ind_inputs[i.ty as usize],
            stock: i.stock,
        })
        .collect();
    let d = Delivery {
        year: sc.year,
        now: sc.now,
        econ_state: sc.state,
        model: sc.model,
        slider_lo: sc.lo,
        slider_hi: sc.hi,
        cargo_id: c,
        cargo: env.cg[c],
        lot_id: sc.lot_id,
        lot_industry: sc.lot_src,
        lot_origin_station: sc.lot_origin,
        t_created: sc.t_created,
        t_loaded: sc.t_loaded,
        src_station_id: sc.s_id,
        src_xy: sc.s_xy,
        src_upgrades: sc.s_upgrades,
        dst_station_id: sc.d_id,
        dst_xy: sc.d_xy,
        dst_demands_cargo: sc.d_demanded,
        dst_price_level: sc.d_level,
        consumers: &consumers,
        train_flags: sc.train_flags,
        human: sc.human,
        computer_factor: sc.c_m,
        company_multiplier: sc.c_mult,
        effect_all: eff(0x3d),
        effect_cargo: eff(if c < 32 { 0x3e + c } else { 0xf1 + c }),
    };
    let r = delivery_revenue(&d, &env.tables);
    (r.amount, r.flag, r.industry)
}

struct World {
    lots: List,
    cars: List,
    trains: List,
    stations: List,
    companies: List,
    persons: List,
    inds: List,
    links: List,
    terrs: List,
}

impl World {
    fn new(o: &mut Oracle) -> Self {
        World {
            lots: List::new(o, G_LOTS, 0x40, 4),
            cars: List::new(o, G_CARS, 0x100, 4),
            trains: List::new(o, G_TRAINS, 0x200, 4),
            stations: List::new(o, G_STATIONS, 0x600, 4),
            companies: List::new(o, G_COMPANIES, 0x5000, 4),
            persons: List::new(o, G_PERSONS, 0x600, 4),
            inds: List::new(o, G_INDUSTRIES, 0x100, 8),
            links: List::new(o, G_LINKS, 0x20, 8),
            terrs: List::new(o, G_TERRITORIES, 0x400, 4),
        }
    }

    /// Lays `sc` out in emulated memory; returns the destination station pointer.
    fn setup(&self, o: &mut Oracle, sc: &Sc) -> u32 {
        for l in [
            &self.lots,
            &self.cars,
            &self.trains,
            &self.stations,
            &self.companies,
            &self.persons,
            &self.inds,
            &self.links,
            &self.terrs,
        ] {
            l.clear(o);
        }
        w32(o, GAME_DATE, u32::from(sc.year));
        w32(o, GAME_TIME, sc.now);
        w32(o, ECON_STATE, sc.state as u32);
        w32(o, MODEL_WORD, u32::from(sc.model));
        w8(o, DEMAND_LEVELS_ON, u8::from(sc.model >= 2));
        w8(o, SLIDERS, sc.lo);
        w8(o, SLIDERS + 1, sc.hi);
        write_fx(o, EFFECT_GLOBAL, &sc.g_fx);

        // company 1 (+ optional person 1), train 1, car 1, territory 1
        let c = self.companies.rec(1);
        w32(o, c, 1);
        w32(o, c + 0x23, sc.c_person);
        wf(o, c + 0x4276, sc.c_m);
        wf(o, c + 0x4248, sc.c_mult);
        write_fx(o, c + COMPANY_FX, &sc.c_fx);
        let p = self.persons.rec(1);
        w32(o, p + 0x122, u32::from(sc.human));
        write_fx(o, p + PERSON_FX, &sc.p_fx);
        write_fx(o, self.terrs.rec(1) + TERR_FX, &sc.t_fx);
        let t = self.trains.rec(1);
        w32(o, t + 0x41, 1);
        w8(o, t + 0xbb, sc.train_flags);
        w16(o, self.cars.rec(1) + 4, 1);

        // the lot
        let l = self.lots.rec(sc.lot_id);
        w8(o, l, sc.cargo as u8);
        w16(o, l + 1, sc.lot_src);
        w16(o, l + 3, sc.lot_origin);
        w16(o, l + 5, 1);
        w32(o, l + 7, sc.t_created);
        w32(o, l + 0xb, sc.t_loaded);

        // stations
        let s = self.stations.rec(sc.s_id);
        w32(o, s, sc.s_id);
        w32(o, s + 0xa, sc.s_xy.0 as u32);
        w32(o, s + 0xe, sc.s_xy.1 as u32);
        w32(o, s + 0x49, sc.s_upgrades);
        let d = self.stations.rec(sc.d_id);
        w32(o, d, sc.d_id);
        w32(o, d + 0xa, sc.d_xy.0 as u32);
        w32(o, d + 0xe, sc.d_xy.1 as u32);
        w8(o, d + 0x195 + sc.cargo as u32, u8::from(sc.d_demanded));
        w8(o, d + 0x321 + sc.cargo as u32, sc.d_level);
        w32(o, d + 0x4ad, sc.d_terr);
        w32(o, d + 0x41, sc.inds.len() as u32);
        w32(o, d + 0x3d, 1);
        // catchment: link i -> industry i, chained through +10
        for (i, ind) in sc.inds.iter().enumerate() {
            let link = self.links.rec(i as u32 + 1);
            w16(o, link, ind.id);
            w32(o, link + 10, i as u32 + 2);
            let r = self.inds.rec(u32::from(ind.id));
            w16(o, r, ind.id);
            w8(o, r + 7, ind.ty);
            for (k, v) in ind.stock.iter().enumerate() {
                wf(o, r + 0x6b + 4 * k as u32, *v);
            }
        }
        d
    }
}

fn rand_scenario(xs: &mut Xs, cg: &[CargoType]) -> Sc {
    let year = match xs.below(4) {
        0 => xs.range(1800, 1860),
        1 => xs.range(1850, 1925),
        2 => xs.range(1900, 2000),
        _ => xs.range(1990, 2040),
    } as u16;
    let now = u32::from(year) * 483_840 + xs.below(483_840);
    let cargo = if xs.chance(85) {
        xs.below(33) as usize
    } else {
        xs.below(46) as usize
    };
    let age_max = if xs.chance(50) { 40 } else { 3000 };
    let age_total = xs.below(age_max) * 1440 + xs.below(1440);
    let transit_max = if xs.chance(60) { 30 } else { 2500 };
    let transit = xs.below(transit_max) * 1440 + xs.below(1440);
    let t_loaded = now.wrapping_sub(transit);
    let mut t_created = t_loaded.wrapping_sub(age_total);
    if xs.chance(2) {
        t_created = t_loaded.wrapping_add(xs.below(100_000)); // wait time "negative": u32 wrap
    }
    let near = xs.chance(30);
    let s_xy = (xs.range(0, 450), xs.range(0, 450));
    let d_xy = if near {
        (s_xy.0 + xs.range(-25, 25), s_xy.1 + xs.range(-25, 25))
    } else {
        (xs.range(0, 450), xs.range(0, 450))
    };
    let same = xs.chance(10);
    let d_xy = if same { s_xy } else { d_xy };
    let s_id = 1 + xs.below(2);
    let d_id = if same { s_id } else { 3 - s_id };
    let housing_town = xs.chance(20);
    let n_inds = if housing_town {
        4 + xs.below(5) as usize
    } else {
        xs.below(7) as usize
    };
    let mut inds = Vec::new();
    for i in 0..n_inds {
        let ty = match xs.below(6) {
            0 => 0x12,
            1 => 0x27,
            _ if housing_town => 0x12,
            _ => xs.below(40) as u8,
        };
        let mut stock = [0f32; 8];
        for s in stock.iter_mut() {
            *s = match xs.below(3) {
                0 => 0.0,
                1 => xs.below(5) as f32,
                _ => xs.below(40) as f32 / 4.0,
            };
        }
        inds.push(Ind {
            id: i as u16 + 1,
            ty,
            stock,
        });
    }
    // consumer / cargo pairs (steel mill: coal, iron; textile mill: cotton, wool; ...)
    let mut cargo = cargo;
    if !inds.is_empty() && xs.chance(45) {
        let pairs = [
            (32u8, 18usize),
            (32, 27),
            (34, 11),
            (34, 12),
            (22, 3),
            (25, 14),
            (13, 10),
        ];
        let (ty, c) = pairs[xs.below(pairs.len() as u32) as usize];
        let k = xs.below(inds.len() as u32) as usize;
        inds[k].ty = ty;
        cargo = c;
    }
    let lot_src = if xs.chance(50) || n_inds == 0 {
        0
    } else {
        1 + xs.below(n_inds as u32) as u16
    };
    let density = [2, 10, 50][xs.below(3) as usize];
    let _ = cg;
    let c_person = if xs.chance(80) { 1 } else { 0 };
    let human = c_person == 1 && xs.chance(50);
    Sc {
        year,
        now,
        state: xs.below(5) as usize,
        model: xs.below(3) as u8,
        lo: xs.below(11) as u8,
        hi: xs.below(11) as u8,
        cargo,
        lot_id: 1 + xs.below(3),
        lot_src,
        lot_origin: s_id as u16,
        t_created,
        t_loaded,
        s_id,
        d_id,
        s_xy,
        d_xy,
        s_upgrades: xs.next() & xs.next(),
        d_demanded: xs.chance(80),
        d_level: xs.below(10) as u8,
        d_terr: xs.below(2),
        train_flags: xs.below(8) as u8,
        c_person,
        human,
        c_m: match xs.below(3) {
            0 => 0.0,
            1 => 1.0,
            _ => xs.below(1000) as f32 / 1000.0,
        },
        c_mult: 0.8 + xs.below(5000) as f32 / 10_000.0,
        inds,
        g_fx: rand_fx(xs, density),
        c_fx: rand_fx(xs, density),
        p_fx: rand_fx(xs, density),
        t_fx: rand_fx(xs, density),
    }
}

/// Prepares the table state the game builds at start-up and returns the shared data. The tables
/// come from rt2-sim's loader; they are cross-checked against the emulated `.data` here.
struct Env {
    rebuilt_for: Option<u16>,
    ind_inputs: Vec<[u8; 8]>,
    cg: Vec<CargoType>,
    tables: RevenueTables,
}

fn prepare(o: &mut Oracle) -> Env {
    let sim = sim_data();
    init_cargo_table(o);
    // loader + start-up fix-ups == what the original holds in memory after `0x471b10`
    for (i, c) in sim.cargo.iter().enumerate() {
        let r = CARGO_TABLE + 0x21 * i as u32;
        assert_eq!(o.read_u32(r).unwrap() as i32, c.price, "cargo {i} price");
        assert_eq!(
            o.read_u32(r + 8).unwrap() as i32,
            c.wait_scale,
            "cargo {i} wait scale"
        );
    }
    let tables = RevenueTables {
        demand_level: read_f32s(o, TBL_DEMAND_LEVEL, 10).try_into().unwrap(),
        undemanded: read_f32s(o, TBL_UNDEMANDED, 6).try_into().unwrap(),
        econ_revenue: read_f32s(o, TBL_ECON_REVENUE, 5).try_into().unwrap(),
        slider_human: read_f32s(o, TBL_SLIDER_HUMAN, 11).try_into().unwrap(),
        slider_ai: read_f32s(o, TBL_SLIDER_AI, 11).try_into().unwrap(),
    };
    assert_eq!(tables, sim.revenue);
    Env {
        rebuilt_for: None,
        ind_inputs: Vec::new(),
        cg: sim.cargo,
        tables,
    }
}

fn run_revenue(
    o: &mut Oracle,
    w: &World,
    env: &mut Env,
    sc: &Sc,
    out: u32,
) -> ((i32, u32, u32), (i32, u32, u32)) {
    // rebuild the industry input lists for this year (the game does it at start-up and each Dec)
    if env.rebuilt_for != Some(sc.year) {
        w32(o, GAME_DATE, u32::from(sc.year));
        for ty in 0..53u32 {
            o.call_fastcall(IND_REBUILD, ty, 0, &[]).unwrap();
        }
        env.ind_inputs = (0..53u32)
            .map(|t| {
                o.read_mem(IND_TABLE + t * 0x90 + 0x80, 8)
                    .unwrap()
                    .try_into()
                    .unwrap()
            })
            .collect();
        env.rebuilt_for = Some(sc.year);
    }
    let d = w.setup(o, sc);
    let want = revenue_ref(sc, env);
    w32(o, out, 0x1234_5678);
    w32(o, out + 4, 0xdead_beef);
    w32(o, out + 8, 0xdead_beef);
    o.call_thiscall(REVENUE, d, &[sc.lot_id, out, out + 4, out + 8, 1])
        .unwrap();
    let got = (
        o.read_u32(out).unwrap() as i32,
        o.read_u32(out + 4).unwrap(),
        o.read_u32(out + 8).unwrap(),
    );
    (got, want)
}

#[test]
fn revenue_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let mut env = prepare(&mut o);
    let w = World::new(&mut o);
    let out = o.alloc(16).unwrap();
    let mut xs = Xs(0x5eed_1234);
    let mut nonzero = 0;
    let mut by_path = [0u32; 3];
    for case in 0..8000 {
        let sc = rand_scenario(&mut xs, &env.cg);
        let (got, want) = run_revenue(&mut o, &w, &mut env, &sc, out);
        assert_eq!(
            got,
            want,
            "case {case}: year {} cargo {} model {} human {} d_demanded {} inds {}",
            sc.year,
            sc.cargo,
            sc.model,
            sc.human,
            sc.d_demanded,
            sc.inds.len()
        );
        if got.0 != 0 {
            nonzero += 1;
        }
        by_path[if got.2 != 0 {
            0
        } else if sc.d_demanded && !sc.inds.is_empty() {
            1
        } else {
            2
        }] += 1;
    }
    eprintln!("revenue: nonzero {nonzero}/8000, industry/other-demand/undemanded {by_path:?}");
    assert!(nonzero > 4000);
    assert!(by_path[0] > 500, "industry-demand path under-exercised");
}

/// The formula chains many `ftol` truncations, so the x87 precision setting matters. With 24-bit
/// precision (CW 0x7f) a measurable share of random deliveries differs from the 53-bit
/// reference by a few dollars; the clone must therefore pin the precision (docs/spec/cargo.md).
#[test]
fn revenue_depends_on_x87_precision() {
    let Some(mut o) = oracle() else { return };
    let mut env = prepare(&mut o);
    let w = World::new(&mut o);
    let out = o.alloc(16).unwrap();
    let mut xs = Xs(0x77aa_1001);
    let mut differ = 0;
    let n = 3000;
    for _ in 0..n {
        let sc = rand_scenario(&mut xs, &env.cg);
        set_fpcw(&mut o, 0x27f);
        let (a, _) = run_revenue(&mut o, &w, &mut env, &sc, out);
        set_fpcw(&mut o, 0x7f);
        let (b, _) = run_revenue(&mut o, &w, &mut env, &sc, out);
        if a != b {
            differ += 1;
        }
    }
    eprintln!("revenue: {differ}/{n} scenarios differ between 53-bit and 24-bit x87 precision");
    assert!(differ > 0, "expected the precision setting to matter");
}

// ---- 4. industry production rate (0x40a8c0) -------------------------------------------------------

const PROD_RATE: u32 = 0x0040_a8c0; // thiscall(ECX = industry) (slot) ret 4; result in ST0
const ERA_SET: u32 = 0x0057_04ac; // byte 0 selects the industry table set
const BASIC_MODEL: u32 = 0x0057_3d77; // 1 when the industrial model is Basic
const SANDBOX: u32 = 0x0057_0227;

#[test]
fn production_rate_matches_original() {
    let Some(mut o) = oracle() else { return };
    set_fpcw(&mut o, 0x27f);
    let inds = List::new(&mut o, G_INDUSTRIES, 0x100, 2);
    let mut xs = Xs(0xfeed_0042);
    let mut nonzero = 0;
    for case in 0..4000 {
        let year = xs.range(1800, 2000) as u32;
        let set = xs.below(2);
        let ty = xs.below(53) as u8;
        let basic = xs.chance(40);
        let sandbox = xs.chance(15);
        w32(&mut o, GAME_DATE, year);
        w32(&mut o, ERA_SET, set);
        w8(&mut o, BASIC_MODEL, u8::from(basic));
        w8(&mut o, SANDBOX, u8::from(sandbox));
        // the input list / indices are rebuilt from the date (Dec step), as the game does
        o.call_fastcall(IND_REBUILD, u32::from(ty), 0, &[]).unwrap();
        let mut stock = [0f32; 8];
        for s in stock.iter_mut() {
            *s = [0.0, 0.25, 0.5, 1.0, 2.0, 3.5][xs.below(6) as usize];
        }
        inds.clear(&mut o);
        let r = inds.rec(1);
        w8(&mut o, r + 7, ty);
        for (k, v) in stock.iter().enumerate() {
            wf(&mut o, r + 0x6b + 4 * k as u32, *v);
        }
        let rec = o
            .read_mem(IND_TABLE + (set * 53 + u32::from(ty)) * 0x90, 0x90)
            .unwrap();
        for slot in 0..4usize {
            // implementation under test: rt2_sim::cargo::IndustryType::production_rate
            let want = IndustryType::from_raw(&rec).production_rate(
                slot,
                ty,
                year as i32,
                basic,
                sandbox,
                &stock,
            );
            o.call_thiscall(PROD_RATE, r, &[slot as u32]).unwrap();
            let got = o.st0().unwrap();
            assert_eq!(
                got, want,
                "year {year} set {set} type {ty} slot {slot} basic {basic} sandbox {sandbox} case {case}"
            );
            if want != 0.0 {
                nonzero += 1;
            }
        }
    }
    assert!(nonzero > 2000, "only {nonzero} non-zero rates");
}
