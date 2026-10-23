//! The player's financial actions end to end on a real save (the Example of the fidelity set,
//! 10 Dec 1875: person 1 is a human chairman of company 4 with 8000 of its 80000 shares, company 5
//! belongs to a computer chairman). Skipped unless `RT2_DIR` and `RT2_FIDELITY_DIR` are set (see
//! `worldloop.rs`).
//!
//! The flow: issue a bond (the rate the window shows is the one written), buy shares of the other
//! company, set a dividend within the board's cap and let the December quarter pay it; the cash and
//! the ledger move by the amounts of docs/spec/finance.md 4.4, 6 and 7.2.

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::World;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::fincmd::{FinCmd, FinError, FinOutcome};
use rt2_sim::pool::{self, f32_at, f64_at, i32_at};

const PLAYER: i32 = 1;
const OWN: i32 = 4;
const OTHER: i32 = 5;
const PE_CASH: usize = 0x12a;
const PE_HOLDINGS: usize = 0x1d3;
const CO_CASH: usize = 0x4766;
const CO_DEBT: usize = 0x4766 + 8 * 6;
const CO_EQUITY: usize = 0x4766 + 8 * 7;
const CO_FEES: usize = 0x4286 + 8 * 12;

fn world() -> Option<World> {
    world_with_cash(None)
}

/// The Example world, optionally giving the human US$ `cash`.
fn world_with_cash(cash: Option<f64>) -> Option<World> {
    let game = PathBuf::from(std::env::var_os("RT2_DIR")?);
    let fid = PathBuf::from(std::env::var_os("RT2_FIDELITY_DIR")?);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).ok()?).ok()?;
    let data = GameData::load(&exe).ok()?;
    let mut save =
        Save::parse(&std::fs::read(fid.join("Example__e4b2f915__s1234__d0.GAM")).ok()?).ok()?;
    if let Some(c) = cash {
        let r = pool::record_mut(&mut save.persons, PLAYER as usize)?;
        r.data[PE_CASH..PE_CASH + 8].copy_from_slice(&c.to_le_bytes());
    }
    Some(World::from_save_seeded(&save, &data, 1234))
}

fn company(w: &World, c: i32) -> Vec<u8> {
    pool::record(&w.save_ref().unwrap().companies, c as usize)
        .unwrap()
        .data
        .clone()
}

fn person(w: &World, p: i32) -> Vec<u8> {
    pool::record(&w.save_ref().unwrap().persons, p as usize)
        .unwrap()
        .data
        .clone()
}

#[test]
fn bond_issue_and_repay_move_the_ledger_by_the_window_numbers() {
    let Some(mut w) = world() else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let quote = w.fin_bond_quote(OWN).unwrap();
    assert_eq!((quote.principal, quote.term), (500_000, 30));
    assert!(quote.refused.is_none(), "{quote:?}");
    let before = company(&w, OWN);
    let cmd = FinCmd::BondIssue {
        company: OWN,
        principal: quote.principal,
        term: quote.term,
    };
    let out = w.apply_fin(cmd).unwrap();
    assert_eq!(out, FinOutcome::Bond { rate: quote.rate });
    let after = company(&w, OWN);
    // 500 000 in cash less the 2% underwriting fee; debt up by the principal; the fee is an
    // expense of the fees category and lowers the equity.
    assert_eq!(
        f64_at(&after, CO_CASH) - f64_at(&before, CO_CASH),
        500_000.0 - 10_000.0
    );
    assert_eq!(
        f64_at(&after, CO_DEBT) - f64_at(&before, CO_DEBT),
        -500_000.0
    );
    assert_eq!(
        f64_at(&after, CO_FEES) - f64_at(&before, CO_FEES),
        -10_000.0
    );
    assert_eq!(i32_at(&after, 0x40), 1);
    assert_eq!(i32_at(&after, 0x44), 500_000);
    assert_eq!(i32_at(&after, 0x48), 1875 + 30);
    assert_eq!(f32_at(&after, 0x4c), quote.rate);
    // The bond list now shows it with the repayment fee.
    let lines = w.fin_bond_lines(OWN);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].fee, 10_000);
    // Repaying costs the principal and the 2% fee.
    let cash_before = f64_at(&after, CO_CASH);
    w.apply_fin(FinCmd::BondRepay {
        company: OWN,
        bond: 0,
    })
    .unwrap();
    let back = company(&w, OWN);
    assert_eq!(i32_at(&back, 0x40), 0);
    assert_eq!(f64_at(&back, CO_CASH), cash_before - 500_000.0 - 10_000.0);
    assert_eq!(f64_at(&back, CO_DEBT), f64_at(&before, CO_DEBT));
}

#[test]
fn buying_shares_pays_the_brokerage_and_moves_the_price() {
    let Some(mut w) = world_with_cash(Some(50_000.0)) else {
        return;
    };
    let r = w.apply_fin(FinCmd::StockTrade {
        person: PLAYER,
        company: OTHER,
        buy: true,
        flag: false,
    });
    // With US$ 50 000 a lot of shares costs more: the window refuses without margin.
    assert_eq!(r, Err(FinError::CannotBuy));

    let mut w = world_with_cash(Some(5_000_000.0)).unwrap();
    let q = w.fin_trade_quote(PLAYER, OTHER).unwrap();
    assert!(q.can_buy && q.buy_lot >= 1000, "{q:?}");
    let before = person(&w, PLAYER);
    let out = w
        .apply_fin(FinCmd::StockTrade {
            person: PLAYER,
            company: OTHER,
            buy: true,
            flag: false,
        })
        .unwrap();
    let FinOutcome::Trade {
        shares,
        price,
        cash,
    } = out
    else {
        panic!("{out:?}");
    };
    assert_eq!(shares, q.buy_lot);
    // Buying pushes the price up (finance.md 3.2) and the last price is the committed quote.
    assert!(price >= q.price, "{price} < {}", q.price);
    let after = person(&w, PLAYER);
    assert_eq!(i32_at(&after, PE_HOLDINGS + 4), shares);
    // The debit is the cost with a 3% brokerage on top, both truncated.
    let cost = (f64::from(price) * f64::from(shares)) as i32;
    let debit = ((f64::from(0.03f32) + 1.0) * f64::from(cost)) as i32;
    assert_eq!(cash, -f64::from(debit));
    assert_eq!(
        f64_at(&after, PE_CASH) - f64_at(&before, PE_CASH),
        -f64::from(debit)
    );
    assert_eq!(f32_at(&company(&w, OTHER), 0x427e), price as f32);
    // Selling the lot back credits 97% of the proceeds: the round trip costs the brokerage.
    let q2 = w.fin_trade_quote(PLAYER, OTHER).unwrap();
    assert!(q2.can_sell && q2.held == shares);
    let out = w
        .apply_fin(FinCmd::StockTrade {
            person: PLAYER,
            company: OTHER,
            buy: false,
            flag: false,
        })
        .unwrap();
    let FinOutcome::Trade {
        shares: sold,
        cash: credit,
        ..
    } = out
    else {
        panic!("{out:?}");
    };
    assert_eq!(sold, -q2.sell_lot);
    assert!(credit > 0.0);
    let end = person(&w, PLAYER);
    assert!(f64_at(&end, PE_CASH) < f64_at(&before, PE_CASH));
    assert_eq!(i32_at(&end, PE_HOLDINGS + 4), shares - q2.sell_lot);
}

#[test]
fn dividend_cap_is_enforced_by_the_window_and_the_quarter_pays_it() {
    let Some(mut w) = world() else {
        return;
    };
    let info = w.fin_dividend(OWN).unwrap();
    assert!(info.permitted);
    // Over the board's cap the window refuses to raise the dividend.
    let too_much = FinCmd::set_dividend(OWN, info.cap + 1.0);
    assert_eq!(w.apply_fin(too_much), Err(FinError::DividendRefused));
    // Within the cap it is stored.
    let d = (info.cap * 0.5 * 100.0).floor() / 100.0;
    assert!(d > 0.0, "cap {info:?}");
    w.apply_fin(FinCmd::set_dividend(OWN, d)).unwrap();
    assert_eq!(f32_at(&company(&w, OWN), 0x30), d);

    // A baseline world without the dividend: the difference after the December step is the
    // dividend of the quarter.
    let mut base = world().unwrap();
    let steps = (28 - 10) * 24 * 60 + 24 * 60; // to the 29th: past the 23:59 of the 28th
    for _ in 0..steps / 60 {
        w.advance(60);
        base.advance(60);
    }
    let (a, b) = (company(&w, OWN), company(&base, OWN));
    let shares = i32_at(&a, 0x2c);
    let paid = f64::from(shares) * f64::from(d) / 4.0;
    // The quarter paid `shares * d / 4` from the cash into the equity (outside the income
    // statement); interest and salaries are the same in both worlds.
    let dcash = f64_at(&a, CO_CASH) - f64_at(&b, CO_CASH);
    assert!((dcash + paid).abs() < 1e-6, "cash {dcash} paid {paid}");
    let dequity = f64_at(&a, CO_EQUITY) - f64_at(&b, CO_EQUITY);
    assert!(
        (dequity - paid).abs() < 1e-6,
        "equity {dequity} paid {paid}"
    );
    // The human holds 8000 shares: he is credited `ftol(8000 * d / 4)`.
    let dp = f64_at(&person(&w, PLAYER), PE_CASH) - f64_at(&person(&base, PLAYER), PE_CASH);
    // `person_month_step` runs right after the companies' step and pays interest on the larger
    // balance (`(prime - 0.04) / 12` of it, whole dollars): a few dollars on top.
    let div = f64::from((8000.0 * f64::from(d) / 4.0) as i32);
    assert!((0.0..=12.0).contains(&(dp - div)), "dp {dp} dividend {div}");
    assert_eq!(i32_at(&person(&w, PLAYER), PE_HOLDINGS), 8000);
}
