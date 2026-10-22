//! Command `0x26` and the message queue on a real save (the Tutorial of the fidelity set).
//! Skipped unless `RT2_DIR` and `RT2_FIDELITY_DIR` are set (see `worldloop.rs`).

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::World;
use rt2_sim::build::cmd::CmdError;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::found::{self, FoundCompany};

const PE_ACTIVE: usize = 0x04;
const PE_HUMAN: usize = 0x122;
const PE_CASH: usize = 0x12a;
const PE_HOLDINGS: usize = 0x1d3;
const PE_CHAIR_OF: usize = 0x263;
const PE_LAST_FOUNDED: usize = 0x551;

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// A world on the Example save whose first person is a free human with `cash` dollars; returns
/// the world and that person's slot.
fn world(cash: f64) -> Option<(World, usize)> {
    let game = PathBuf::from(std::env::var_os("RT2_DIR")?);
    let fid = PathBuf::from(std::env::var_os("RT2_FIDELITY_DIR")?);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(&game).unwrap();
    let mut base =
        Save::parse(&std::fs::read(fid.join("Example__e4b2f915__s1234__d0.GAM")).unwrap()).unwrap();
    let slot = rt2_sim::pool::list_order(&base.persons)[0];
    let r = rt2_sim::pool::record_mut(&mut base.persons, slot).unwrap();
    r.data[PE_ACTIVE..PE_ACTIVE + 4].copy_from_slice(&1i32.to_le_bytes());
    r.data[PE_HUMAN..PE_HUMAN + 4].copy_from_slice(&1i32.to_le_bytes());
    r.data[PE_CASH..PE_CASH + 8].copy_from_slice(&cash.to_le_bytes());
    r.data[PE_CHAIR_OF..PE_CHAIR_OF + 4].copy_from_slice(&0i32.to_le_bytes());
    r.data[PE_LAST_FOUNDED..PE_LAST_FOUNDED + 4].copy_from_slice(&0i32.to_le_bytes());
    for i in 0..36 {
        r.data[PE_HOLDINGS + 4 * i..PE_HOLDINGS + 4 * i + 4].copy_from_slice(&0i32.to_le_bytes());
    }
    let mut w = World::from_save(&base, &data);
    w.apply(rt2_sim::Command::SeedRng(1234));
    Some((w, slot))
}

fn set_len(w: &World) -> i32 {
    i32_at(&w.save_ref().unwrap().state, 0x241)
}

#[test]
fn start_company_creates_the_record_and_buys_the_shares() {
    let Some((mut w, person)) = world(1_000_000.0) else {
        eprintln!("RT2_DIR / RT2_FIDELITY_DIR not set; skipping");
        return;
    };
    let person = person as i32;
    let before = set_len(&w);
    let ps = found::player_slider(1_000_000);
    let p = ps.value(ps.initial_position());
    let m = found::credit_multiplier(true, 0, false, 0, i32::from(w.date().year));
    let os = found::outside_slider(m, p);
    let o = os.value(os.initial_position());
    let pv = found::founding_preview(p, o);
    let cmd = FoundCompany::new(person, "Test & Co", -1, pv.capital, p);
    let id = w.apply_found(cmd).expect("founded") as usize;
    assert_eq!(set_len(&w), before + 1);
    let sv = w.save_ref().unwrap();
    let co = &rt2_sim::pool::record(&sv.companies, id).unwrap().data;
    assert_eq!(i32_at(co, 0x2c), pv.company_shares);
    assert_eq!(
        f32::from_le_bytes(co[0x3c..0x40].try_into().unwrap()),
        pv.price
    );
    assert_eq!(&co[4..13], b"Test & Co");
    let pe = &rt2_sim::pool::record(&sv.persons, person as usize)
        .unwrap()
        .data;
    let idx = before as usize;
    assert_eq!(i32_at(pe, PE_HOLDINGS + 4 * idx), pv.founder_shares);
    let cash = f64::from_le_bytes(pe[PE_CASH..PE_CASH + 8].try_into().unwrap());
    assert_eq!(cash, 1_000_000.0 - f64::from(pv.founder_cost));
    assert_eq!(i32_at(pe, PE_CHAIR_OF), id as i32);
    // The person now chairs a company.
    assert_eq!(w.apply_found(cmd), Err(CmdError::AlreadyChairman));
}

#[test]
fn start_company_rejects_bad_input() {
    let Some((mut w, person)) = world(1_000_000.0) else {
        return;
    };
    let person = person as i32;
    let bad = FoundCompany::new(person, "X", -1, 1_234_567, 123_456);
    assert_eq!(w.apply_found(bad), Err(CmdError::InvalidInput));
    let ghost = FoundCompany::new(9999, "X", -1, 500_000, 100_000);
    assert_eq!(w.apply_found(ghost), Err(CmdError::NoSuchObject));
    let (mut poor, person) = world(50_000.0).unwrap();
    let cmd = FoundCompany::new(person as i32, "X", -1, 500_000, 100_000);
    assert_eq!(poor.apply_found(cmd), Err(CmdError::InsufficientCash));
}

#[test]
fn messages_queue_drains() {
    let Some((mut w, _)) = world(1_000_000.0) else {
        return;
    };
    assert_eq!(w.game_over(), None);
    w.advance(60 * 1440);
    let first = w.take_messages();
    assert!(w.take_messages().is_empty(), "the queue is drained");
    let _ = first;
}
