//! Map Options byte maps of the players, pool, managers, engines, ports and robbers pages
//! (docs/formats/map-gam.md section 10) over the shipped maps, and the PCX import. The map tests
//! need the game install (`RT2_DIR`) and are skipped without it.

use std::path::{Path, PathBuf};

use rt2_formats::map::Save;
use rt2_sim::data::GameData;
use rt2_sim::editor_opts as o;
use rt2_sim::editor_pcx::{self as pcx, Pcx};
use rt2_sim::exe::Exe;
use rt2_sim::rng::GameRng;

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

fn data(dir: &Path) -> GameData {
    let exe = Exe::from_bytes(std::fs::read(dir.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    GameData::load(&exe).unwrap()
}

fn mp2s(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir.join("maps"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("mp2"))
        })
        .collect();
    v.sort();
    v
}

fn text(id: u32) -> String {
    match id {
        0x14 => "Unassigned".into(),
        _ => format!("T{id}"),
    }
}

/// The player summary the editor keeps in the header (`0x46b890`) is what the 95 shipped maps
/// carry: the six counts and the five-byte slots.
#[test]
fn player_summary_matches_the_shipped_maps() {
    let Some(dir) = install() else { return };
    let files = mp2s(&dir);
    assert!(files.len() >= 90);
    for p in files {
        let s = Save::parse(&std::fs::read(&p).unwrap()).unwrap();
        let mut t = s.clone();
        o::refresh_player_summary(&mut t);
        assert_eq!(
            t.header[0x150..0x168 + 5 * o::MAX_PLAYERS],
            s.header[0x150..0x168 + 5 * o::MAX_PLAYERS],
            "{}",
            p.display()
        );
    }
}

/// The Ports rows are already normal (second input cleared, manufactured rows 1.0) in the maps:
/// re-applying the dialog's data effects changes nothing.
#[test]
fn port_normalisation_keeps_the_shipped_rows() {
    let Some(dir) = install() else { return };
    for p in mp2s(&dir) {
        let s = Save::parse(&std::fs::read(&p).unwrap()).unwrap();
        let mut t = s.clone();
        for k in 0..o::PORT_COUNT {
            let sup = o::port(&t, k).unwrap().supplied;
            if sup != o::NO_CARGO {
                o::set_port_supplied(&mut t, k, sup);
            }
        }
        let row = |x: &Save| x.header[0x8703..0x8703 + 0x90].to_vec();
        assert_eq!(row(&t), row(&s), "{}", p.display());
    }
}

/// Every option survives a save and a load.
#[test]
fn options_round_trip() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let raw = std::fs::read(dir.join("maps/Alaska.MP2")).unwrap();
    let mut s = Save::parse(&raw).unwrap();
    let mut rng = GameRng::new(5);
    let n0 = o::players(&s).len();
    let id = o::add_player(&mut s, &mut rng, &d.image, &text).unwrap();
    assert_eq!(o::players(&s).len(), n0 + 1);
    o::set_mandatory(&mut s, id, false).unwrap();
    o::set_controller(&mut s, id, o::Controller::Computer).unwrap();
    o::set_override(&mut s, id, false).unwrap();
    o::set_cash(&mut s, id, 250_000).unwrap();
    o::step_persona(&mut s, &mut rng, &d.image, &text, id, 1).unwrap();
    o::pool_preset(&mut s, &d.image, 1);
    o::set_manager_allowed(&mut s, 3, false);
    o::engine_preset(&mut s, &d.image, 0);
    o::set_engine_allowed(&mut s, 7, false);
    o::set_port_annual(&mut s, 1, false);
    o::set_port_supplied(&mut s, 1, 5);
    o::set_port_annual(&mut s, 2, true);
    o::set_port_quantity(&mut s, 2, 4);
    o::set_robbery_level(&mut s, 200);
    o::set_robber_name(&mut s, 0, "Black Bart");
    o::set_robber_year(&mut s, 0, 1880);
    o::refresh_player_summary(&mut s);
    let t = Save::parse(&s.to_bytes()).unwrap();
    assert_eq!(o::players(&t), o::players(&s));
    let me = o::player_info(&t, id).unwrap();
    assert!(!me.mandatory && !me.allow_override);
    assert_eq!(me.controller, o::Controller::Computer);
    assert_eq!(me.cash, 250_000.0);
    assert_ne!(me.persona, 0);
    assert!(me.name.len() > 1);
    assert_eq!(o::pool_count(&t), o::pool_count(&s));
    assert!(!o::manager_allowed(&t, 3) && o::manager_allowed(&t, 4));
    assert!(!o::engine_allowed(&t, 7));
    let p1 = o::port(&t, 1).unwrap();
    assert!(
        !p1.annual && p1.supplied == 5 && p1.quantity == 1.0,
        "{p1:?}"
    );
    assert_ne!(p1.demanded, o::NO_CARGO);
    assert_eq!(o::port(&t, 2).unwrap().quantity, 4.0);
    assert_eq!(o::robbery_level(&t), 200);
    assert_eq!(o::robber(&t, 0), Some(("Black Bart".to_string(), 1880)));
    // Removing the player again gives the same list as before, the record stays for reuse.
    let mut u = t.clone();
    o::remove_player(&mut u, id).unwrap();
    assert_eq!(o::players(&u).len(), n0);
    assert_eq!(
        o::add_player(&mut u, &mut rng, &d.image, &text).unwrap(),
        id
    );
}

/// The editor refuses what the original refuses.
#[test]
fn first_player_rules() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let raw = std::fs::read(dir.join("maps/Alaska.MP2")).unwrap();
    let mut s = Save::parse(&raw).unwrap();
    let first = o::player_slots(&s)[0];
    assert_eq!(
        o::set_mandatory(&mut s, first, false),
        Err(o::OptError::FirstMandatory)
    );
    assert_eq!(
        o::set_controller(&mut s, first, o::Controller::Computer),
        Err(o::OptError::FirstNotAi)
    );
    assert_eq!(
        o::remove_player(&mut s, first),
        Err(o::OptError::FirstPlayer)
    );
    let mut rng = GameRng::new(1);
    while o::players(&s).len() < o::MAX_PLAYERS {
        o::add_player(&mut s, &mut rng, &d.image, &text).unwrap();
    }
    assert_eq!(
        o::add_player(&mut s, &mut rng, &d.image, &text),
        Err(o::OptError::TooManyPlayers)
    );
}

/// The region presets of the pool and of the engines match what the shipped maps carry for the
/// untouched sets (every shipped engine set of a map made from a preset is one of the three).
#[test]
fn presets_have_the_documented_sizes() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let raw = std::fs::read(dir.join("maps/Alaska.MP2")).unwrap();
    let mut s = Save::parse(&raw).unwrap();
    let mut counts = Vec::new();
    for g in 0..3u8 {
        o::pool_preset(&mut s, &d.image, g);
        counts.push(o::pool_count(&s));
    }
    assert_eq!(counts.iter().sum::<usize>(), 40);
    for c in 0..3 {
        o::engine_preset(&mut s, &d.image, c);
        assert!(o::engine_count(&s) > 10);
    }
}

#[test]
fn territory_removal() {
    let Some(dir) = install() else { return };
    let raw = std::fs::read(dir.join("maps/#01B&O~1.MAP")).unwrap_or_default();
    if raw.is_empty() {
        return;
    }
    let mut s = Save::parse(&raw).unwrap();
    let slots: Vec<u8> = s.territories.ordered().map(|r| r.slot as u8).collect();
    assert!(slots.len() > 2);
    assert_eq!(
        o::territory_remove(&mut s, 1),
        Err(o::OptError::FirstTerritory)
    );
    let last = *slots.last().unwrap();
    o::territory_remove(&mut s, last).unwrap();
    assert!(s.map.tiles.iter().all(|t| t.region != last));
    assert_eq!(s.territories.count, slots.len() - 1);
    let t = Save::parse(&s.to_bytes()).unwrap();
    assert_eq!(t.territories.count, slots.len() - 1);
}

// ---- PCX --------------------------------------------------------------------------------------

/// A 8-bit PCX with `w x h` pixels, RLE coded, and a grey palette (index i -> (i, i, i)).
fn gray_pcx(w: usize, h: usize, px: impl Fn(usize, usize) -> u8) -> Vec<u8> {
    let mut f = vec![0u8; 128];
    f[0] = 10;
    f[1] = 5;
    f[2] = 1;
    f[3] = 8;
    f[8..10].copy_from_slice(&((w - 1) as u16).to_le_bytes());
    f[10..12].copy_from_slice(&((h - 1) as u16).to_le_bytes());
    f[0x41] = 1;
    let bpl = (w + 1) & !1;
    f[0x42..0x44].copy_from_slice(&(bpl as u16).to_le_bytes());
    for y in 0..h {
        let mut line: Vec<u8> = (0..w).map(|x| px(x, y)).collect();
        line.resize(bpl, 0);
        let mut i = 0;
        while i < line.len() {
            let v = line[i];
            let mut n = 1;
            while i + n < line.len() && line[i + n] == v && n < 63 {
                n += 1;
            }
            if n > 1 || v >= 0xc0 {
                f.push(0xc0 | n as u8);
            }
            f.push(v);
            i += n;
        }
    }
    f.push(0x0c);
    for i in 0..256 {
        f.extend([i as u8; 3]);
    }
    f
}

#[test]
fn pcx_decoding_and_heights() {
    let raw = gray_pcx(10, 6, |x, y| (x * 20 + y) as u8);
    let img = Pcx::parse(&raw).unwrap();
    assert_eq!((img.stride, img.rows, img.planes.len()), (10, 6, 1));
    assert_eq!(img.planes[0][3 * 10 + 4], (4 * 20 + 3) as u8);
    // 9x5 map: the corners sample the image at 10/10 and 6/6 per corner, i.e. exact pixels.
    let h = pcx::heights(&img, 9, 5, 100);
    assert_eq!(h.len(), 10 * 6);
    assert_eq!(h[0], -1); // black is one below sea level at scale 1
    assert_eq!(h[10 + 2], (2 * 20 + 1 - 1) as i16);
    // a scale of 200 doubles the span: (v - 1) * 2
    let h2 = pcx::heights(&img, 9, 5, 200);
    assert_eq!(h2[10 + 2], 2 * 40);
    // the clamp at 510
    let tall = pcx::heights(&img, 9, 5, 2000);
    assert!(tall.iter().all(|&v| (-255..=510).contains(&v)));
    assert_eq!(*tall.iter().max().unwrap(), 510);
}

#[test]
fn pcx_import_builds_the_map() {
    let Some(dir) = install() else { return };
    let raw = std::fs::read(dir.join("maps/Local Industry.MP2")).unwrap();
    let template = Save::parse(&raw).unwrap();
    let mut s = rt2_sim::editor::new_map(&template, 20, 12, 1, "Image");
    let img = Pcx::parse(&gray_pcx(40, 24, |x, y| (x * 6 + y * 2) as u8)).unwrap();
    let mut rng = GameRng::new(3);
    pcx::import(&mut s, &mut rng, &img, 100);
    assert_eq!(s.map.elevation.len(), 21 * 13);
    assert!(s.map.elevation[0] <= 1);
    assert!(s.map.elevation[20] > 100);
    let again = Save::parse(&s.to_bytes()).unwrap();
    assert_eq!(again.map.elevation, s.map.elevation);
}
