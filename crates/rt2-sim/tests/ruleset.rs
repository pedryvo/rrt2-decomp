//! The optional fixes of original bugs (`rt2_sim::ruleset`, docs/spec/original-bugs.md): each
//! fix, enabled alone, changes the behaviour it documents; the faithful default changes nothing
//! (the fidelity scoreboard, `scripts/fidelity.py`, runs in the default mode).
//!
//! Needs the game install (`RT2_DIR`) and `base_ex.GAM` in `RT2_FIDELITY_DIR` (the Example save
//! advanced 0 days by the original, as in `worldloop.rs`). Skipped when either is missing.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::build::cmd::{BuildCmd, CmdError};
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::fincmd::{FinCmd, FinError};
use rt2_sim::pool::{self, f32_at, f64_at, i32_at, put_f64};
use rt2_sim::ruleset::{FIXES, Fix, Ruleset};
use rt2_sim::{Command, World};

const HDR_BASE: usize = 0x0056_7b38;
const ST_BASE: usize = 0x0057_04e8;
const CO_CASH: usize = 0x4766;
const CO_CATS: usize = 0x4286;
/// Days of the Example that include train settlements (deliveries with fees and revenue).
const SETTLED: i32 = 110;

fn setup() -> Option<(GameData, Save)> {
    let game = PathBuf::from(std::env::var_os("RT2_DIR")?);
    let fid = PathBuf::from(std::env::var_os("RT2_FIDELITY_DIR")?);
    let raw = std::fs::read(fid.join("base_ex.GAM")).ok()?;
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(&game).unwrap();
    Some((data, Save::parse(&raw).unwrap()))
}

macro_rules! need_game {
    () => {
        match setup() {
            Some(v) => v,
            None => {
                eprintln!("RT2_DIR / RT2_FIDELITY_DIR/base_ex.GAM missing; skipping");
                return;
            }
        }
    };
}

fn world(base: &Save, data: &GameData, rules: Ruleset) -> World {
    let mut w = World::from_save(base, data);
    w.apply(Command::SeedRng(1234));
    w.apply(Command::SeedCrtRng(1234));
    w.apply(Command::SetRuleset(rules));
    w
}

fn advance(w: &mut World, days: i32) {
    let mut left = days * 1440;
    while left > 0 {
        let dt = left.min(320);
        w.advance(dt);
        left -= dt;
    }
}

fn run(base: &Save, data: &GameData, rules: Ruleset, days: i32) -> Save {
    let mut w = world(base, data, rules);
    advance(&mut w, days);
    w.to_save()
}

fn fix(f: Fix) -> Ruleset {
    Ruleset::FAITHFUL.with(f, true)
}

fn companies(s: &Save) -> Vec<usize> {
    pool::list_order(&s.companies)
}

fn co(s: &Save, c: usize) -> &[u8] {
    &pool::record(&s.companies, c).unwrap().data
}

#[test]
fn flag_list_is_exposed_for_the_interface() {
    assert!(FIXES.len() >= 15);
    for f in FIXES {
        assert!(!f.id.is_empty() && !f.name.is_empty() && !f.description.is_empty());
        assert!(f.reference.contains("0x"), "{}", f.id);
    }
}

#[test]
fn faithful_default_and_save_trailer() {
    let (data, base) = need_game!();
    let mut w = World::from_save(&base, &data);
    assert_eq!(w.ruleset(), Ruleset::FAITHFUL);
    // The faithful file is exactly the original's bytes.
    assert_eq!(w.to_file_bytes(), w.to_save().to_bytes());
    let r = fix(Fix::StationFeeOwnStation).with(Fix::GoodwillAge, true);
    w.apply(Command::SetRuleset(r));
    let bytes = w.to_file_bytes();
    let (save, got) = World::parse_file(&bytes).unwrap();
    assert_eq!(got, r);
    assert_eq!(save.to_bytes(), w.to_save().to_bytes());
}

/// Every fix is off in the default world: a run with an explicit faithful ruleset equals the
/// default run field for field (the scoreboard keeps its 131 equal cases, docs/fidelity-status.md).
#[test]
fn explicit_faithful_equals_default() {
    let (data, base) = need_game!();
    let mut a = World::from_save(&base, &data);
    a.apply(Command::SeedRng(1234));
    a.apply(Command::SeedCrtRng(1234));
    advance(&mut a, 20);
    let b = run(&base, &data, Ruleset::FAITHFUL, 20);
    assert_eq!(a.to_save().to_bytes(), b.to_bytes());
}

#[test]
fn scenario_functions() {
    let (data, mut base) = need_game!();
    // Companies founded ten years before, with loads in two territories of different
    // standing (company + 0x479 + t * 0x1ed: +1 standing, +3 track, +5 loads this year, +7
    // lifetime loads), so the lifetime divisor changes the weighting.
    let n_terr = base.territories.count;
    assert!(n_terr >= 2);
    for c in companies(&base) {
        let r = &mut pool::record_mut(&mut base.companies, c).unwrap().data;
        let y = i32::from(base.date.year) - 10;
        r[0x148..0x14c].copy_from_slice(&y.to_le_bytes());
        for t in 1..=n_terr {
            let b = 0x479 + t * 0x1ed;
            let (standing, life, year) = match t {
                1 => (100i16, 3000i16, 0i16),
                2 => (900, 0, 100),
                _ => (0, 0, 0),
            };
            for (o, v) in [(1, standing), (3, 0), (5, year), (7, life)] {
                r[b + o..b + o + 2].copy_from_slice(&v.to_le_bytes());
            }
        }
    }
    let mut f = world(&base, &data, Ruleset::FAITHFUL);
    let all = Ruleset::FAITHFUL
        .with(Fix::ScenarioPlayerId, true)
        .with(Fix::ScenarioCompanyDebt, true)
        .with(Fix::ScenarioIndustryProfitsThisYear, true)
        .with(Fix::GoodwillAge, true);
    let mut x = world(&base, &data, all);
    let person = pool::list_order(&base.persons)[0] as i32;
    let mut goodwill_differs = false;
    for c in companies(&base) {
        let c32 = c as i32;
        // PlayerId (59): the company id, fixed to the person id.
        assert_eq!(f.scenario_function_value(59, 0, c32, person, 0), c32);
        assert_eq!(x.scenario_function_value(59, 0, c32, person, 0), person);
        // CompanyDebt (24): stored negative, fixed positive.
        let d = f.scenario_function_value(24, 0, c32, 0, 0);
        assert!(d <= 0);
        assert_eq!(x.scenario_function_value(24, 0, c32, 0, 0), -d);
        // CompanyIndustryProfitsThisYear (97): last year's (101), fixed to this year's cat 3.
        assert_eq!(
            f.scenario_function_value(97, 0, c32, 0, 0),
            f.scenario_function_value(101, 0, c32, 0, 0)
        );
        let this_year = rt2_sim::fp::ftol(f64_at(co(&base, c), CO_CATS + 24));
        assert_eq!(x.scenario_function_value(97, 0, c32, 0, 0), this_year);
        // Goodwill (37): the fix divides lifetime loads by the age.
        goodwill_differs |= f.scenario_function_value(37, 0, c32, 0, 0)
            != x.scenario_function_value(37, 0, c32, 0, 0);
        // A fix never leaks into another function.
        assert_eq!(
            f.scenario_function_value(23, 0, c32, 0, 0),
            x.scenario_function_value(23, 0, c32, 0, 0)
        );
    }
    assert!(goodwill_differs, "no company's goodwill changed");
}

#[test]
fn merger_permission() {
    let (data, mut base) = need_game!();
    base.state[0x0057_06b4 - ST_BASE] = 1; // "Retiring a train"
    base.state[0x0057_06bd - ST_BASE] = 0; // "Merging companies"
    let cs = companies(&base);
    let cmd = FinCmd::MergerPropose {
        buyer: cs[cs.len() - 1] as i32,
        target: cs[cs.len() - 2] as i32,
        offer: 1,
    };
    let f = world(&base, &data, Ruleset::FAITHFUL);
    let x = world(&base, &data, fix(Fix::MergerPermission));
    assert_ne!(f.check_fin(&cmd), Err(FinError::NotAllowed));
    assert_eq!(x.check_fin(&cmd), Err(FinError::NotAllowed));
    base.state[0x0057_06b4 - ST_BASE] = 0;
    base.state[0x0057_06bd - ST_BASE] = 1;
    let f = world(&base, &data, Ruleset::FAITHFUL);
    let x = world(&base, &data, fix(Fix::MergerPermission));
    assert_eq!(f.check_fin(&cmd), Err(FinError::NotAllowed));
    assert_ne!(x.check_fin(&cmd), Err(FinError::NotAllowed));
}

#[test]
fn territory_rights_per_kind_for_industry_purchase() {
    let (data, mut base) = need_game!();
    base.state[0x0057_3d78 - ST_BASE] = 2; // expert industrial model
    base.state[0x0057_06b9 - ST_BASE] = 1; // buying industries allowed
    let company = companies(&base)[0];
    put_f64(
        &mut pool::record_mut(&mut base.companies, company).unwrap().data,
        CO_CASH,
        1e9,
    );
    let mut checked = 0;
    for b in pool::list_order(&base.objects) {
        let r = &pool::record(&base.objects, b).unwrap().data;
        if i16::from_le_bytes([r[0x97], r[0x98]]) != 0 {
            continue;
        }
        let terr = usize::from(r[0x9d]);
        let cmd = BuildCmd::TradeIndustry {
            company: company as i32,
            building: b as i32,
            buy: true,
        };
        let with_rights = |bits: u8, rules: Ruleset| {
            let mut s = base.clone();
            pool::record_mut(&mut s.companies, company).unwrap().data[0x479 + terr * 0x1ed] = bits;
            world(&s, &data, rules).apply_build(cmd)
        };
        if with_rights(2, Ruleset::FAITHFUL) == Err(CmdError::NotBuyable) {
            continue; // not a buyable type
        }
        let x = fix(Fix::TerritoryRightsPerKind);
        assert_eq!(with_rights(2, x), Err(CmdError::NotBuyable));
        assert_eq!(with_rights(4, Ruleset::FAITHFUL), Err(CmdError::NotBuyable));
        assert_ne!(with_rights(4, x), Err(CmdError::NotBuyable));
        checked += 1;
        if checked == 3 {
            break;
        }
    }
    assert!(checked > 0, "no buyable industry in the Example");
}

fn sum_cat(s: &Save, k: usize) -> f64 {
    companies(s)
        .into_iter()
        .map(|c| f64_at(co(s, c), CO_CATS + 8 * k))
        .sum()
}

#[test]
fn station_fee_own_station() {
    let (data, base) = need_game!();
    // Settlements are rare in the Example (the first one at day 69, after the year end that
    // archives the categories).
    let f = run(&base, &data, Ruleset::FAITHFUL, SETTLED);
    let x = run(&base, &data, fix(Fix::StationFeeOwnStation), SETTLED);
    // Category 2 ("station" revenue) loses the fees paid by nobody.
    assert!(
        sum_cat(&x, 2) < sum_cat(&f, 2),
        "{} {}",
        sum_cat(&x, 2),
        sum_cat(&f, 2)
    );
}

fn total_cash(s: &Save) -> f64 {
    companies(s)
        .into_iter()
        .map(|c| f64_at(co(s, c), CO_CASH))
        .sum()
}

#[test]
fn station_fee_unowned() {
    let (data, mut base) = need_game!();
    for st in pool::list_order(&base.stations) {
        let r = &mut pool::record_mut(&mut base.stations, st).unwrap().data;
        r[0x39..0x3d].copy_from_slice(&0u32.to_le_bytes());
    }
    let f = run(&base, &data, Ruleset::FAITHFUL, SETTLED);
    let x = run(&base, &data, fix(Fix::StationFeeUnowned), SETTLED);
    assert!(total_cash(&x) > total_cash(&f));
}

#[test]
fn station_revenue_effect() {
    let (data, base) = need_game!();
    let srev = |s: &Save| -> i64 {
        pool::list_order(&s.stations)
            .into_iter()
            .map(|st| i64::from(i32_at(&pool::record(&s.stations, st).unwrap().data, 0x5d)))
            .sum()
    };
    let f = run(&base, &data, Ruleset::FAITHFUL, SETTLED);
    let x = run(&base, &data, fix(Fix::StationRevenueEffect), SETTLED);
    // Without a service bonus B = 0: the original books effect + 0 = $1, the fix 1 * 0.
    assert!(srev(&x) < srev(&f), "{} {}", srev(&x), srev(&f));
}

fn trains_bytes(s: &Save) -> Vec<u8> {
    pool::list_order(&s.trains)
        .into_iter()
        .flat_map(|t| pool::record(&s.trains, t).unwrap().data.clone())
        .collect()
}

#[test]
fn trains_midnight_minute() {
    let (data, base) = need_game!();
    let f = run(&base, &data, Ruleset::FAITHFUL, 3);
    let x = run(&base, &data, fix(Fix::TrainsMidnightMinute), 3);
    assert_eq!(f.game_time_minutes, x.game_time_minutes);
    // The trains (records and cars) moved three more minutes.
    let cars = |s: &Save| -> Vec<u8> {
        pool::list_order(&s.pool_70)
            .into_iter()
            .flat_map(|c| pool::record(&s.pool_70, c).unwrap().data.clone())
            .collect()
    };
    assert!(trains_bytes(&f) != trains_bytes(&x) || cars(&f) != cars(&x));
}

#[test]
fn broken_train_deceleration() {
    let (data, mut base) = need_game!();
    let until = (base.game_time_minutes as u32).wrapping_add(10 * 1440);
    for t in pool::list_order(&base.trains) {
        let r = &mut pool::record_mut(&mut base.trains, t).unwrap().data;
        r[0xed] = 1;
        r[0xee..0xf2].copy_from_slice(&until.to_le_bytes());
    }
    let speed = |s: &Save| -> Vec<i32> {
        pool::list_order(&s.trains)
            .into_iter()
            .map(|t| i32_at(&pool::record(&s.trains, t).unwrap().data, 0x69))
            .collect()
    };
    let mut wf = world(&base, &data, Ruleset::FAITHFUL);
    let mut wx = world(&base, &data, fix(Fix::BrokenTrainDeceleration));
    wf.advance(320);
    wx.advance(320);
    let (f, x) = (speed(&wf.to_save()), speed(&wx.to_save()));
    // `tau >> 1` instead of `tau >> 4` for the trains above their target speed (0).
    assert_ne!(f, x);
}

#[test]
fn route_step_flags() {
    let (data, mut base) = need_game!();
    // Heap garbage left by the original in every ring slot.
    for e in &mut base.train_extras {
        let n = e.route_raw.len() / 8;
        for i in 0..n {
            e.route_raw[6 * n + 2 * i] |= 0xf0;
            e.route_raw[6 * n + 2 * i + 1] |= 0xc0;
        }
    }
    let garbage = |s: &Save| -> usize {
        s.train_extras
            .iter()
            .map(|e| {
                let n = e.route_raw.len() / 8;
                (0..n)
                    .filter(|&i| {
                        let o = 6 * n + 2 * i;
                        u16::from_le_bytes([e.route_raw[o], e.route_raw[o + 1]]) & 0xc0f0 != 0
                    })
                    .count()
            })
            .sum()
    };
    let f = run(&base, &data, Ruleset::FAITHFUL, 20);
    let x = run(&base, &data, fix(Fix::RouteStepFlags), 20);
    assert!(garbage(&x) < garbage(&f), "{} {}", garbage(&x), garbage(&f));
}

#[test]
fn ai_dividend_policy_once() {
    let (data, mut base) = need_game!();
    base.header[0x0057_04ae - HDR_BASE] = 1; // Dividend Payout policy 1
    // Two monthly AI turns; the Example starts in December, so one is outside December.
    let days = 60;
    let f = run(&base, &data, Ruleset::FAITHFUL, days);
    let x = run(&base, &data, fix(Fix::AiDividendPolicyOnce), days);
    let div = |s: &Save, c| f32_at(co(s, c), 0x30);
    let mut seen = 0;
    for c in companies(&base) {
        if div(&base, c) > 0.0 && div(&f, c) != div(&x, c) {
            assert!(div(&x, c) > div(&f, c));
            seen += 1;
        }
    }
    assert!(seen > 0, "no AI company with a dividend");
}

#[test]
fn overhead_average() {
    let (data, base) = need_game!();
    // Through the first year end of the Example.
    let days = 28 * (13 - i32::from(base.date.month)) + 2;
    let f = run(&base, &data, Ruleset::FAITHFUL, days);
    let x = run(&base, &data, fix(Fix::OverheadAverage), days);
    let oh = |s: &Save, c| i32_at(co(s, c), 0x138);
    assert!(companies(&base).into_iter().any(|c| oh(&f, c) != oh(&x, c)));
}

/// `x / 0` in a scenario condition (`0x454270`): `x` in the original, 1 with the fix.
#[test]
fn scenario_division_by_zero() {
    use rt2_sim::rng::GameRng;
    use rt2_sim::scenario::lang::{self, Ctx, Eval, ScenarioEnv};
    struct Env(Ruleset);
    impl ScenarioEnv for Env {
        fn value(&mut self, _: usize, _: i32, _: i32, _: i32, _: i32) -> i32 {
            0
        }
        fn crt_range(&mut self, lo: i32, _: i32) -> i32 {
            lo
        }
        fn rules(&self) -> Ruleset {
            self.0
        }
        fn companies(&self) -> Vec<i32> {
            Vec::new()
        }
        fn persons(&self) -> Vec<i32> {
            Vec::new()
        }
        fn territories(&self) -> Vec<i32> {
            Vec::new()
        }
    }
    let eval = |text: &str, rules: Ruleset| {
        let (ids, vals) = lang::compile(text).unwrap();
        let mut env = Env(rules);
        let mut rng = GameRng::new(1);
        lang::reduce(
            &mut Eval::new(&mut env, &mut rng, Ctx::default()),
            &ids,
            &vals,
        )
    };
    let x = fix(Fix::ScenarioDivByZero);
    assert_eq!(eval("7 / 0", Ruleset::FAITHFUL), Ok(7));
    assert_eq!(eval("7 / 0", x), Ok(1));
    assert_eq!(eval("8 / 2", x), Ok(4));
}
