//! The editor's saving and tools over the shipped maps (docs/PLAN.md phase 5). Needs the game
//! install (`RT2_DIR`); skipped without it.

use std::path::{Path, PathBuf};

use rt2_formats::map::Save;
use rt2_sim::World;
use rt2_sim::data::GameData;
use rt2_sim::editor::{self, Elevation, FillMode, PlaceKind};
use rt2_sim::exe::Exe;

fn install() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

fn data(dir: &Path) -> GameData {
    let exe = Exe::from_bytes(std::fs::read(dir.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut d = GameData::load(&exe).unwrap();
    d.load_sprites(dir).unwrap();
    d
}

fn maps(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir.join("maps"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
        .collect();
    v.sort();
    v
}

fn small_world(dir: &Path, d: &GameData) -> World {
    let raw = std::fs::read(dir.join("maps/Local Industry.MP2")).unwrap();
    World::from_save_seeded(&Save::parse(&raw).unwrap(), d, 1)
}

/// Offsets the original's saver rewrites on every save: the player counts of the header
/// (`0x567c88..0x567ca0`) and the checksum slot.
fn volatile(i: usize) -> bool {
    (0x150..0x168).contains(&i) || (0x86f7..0x86fb).contains(&i)
}

#[test]
fn every_mp2_loaded_and_saved_unchanged_is_byte_identical_modulo_volatile_fields() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let files = maps(&dir, "mp2");
    assert!(files.len() > 90);
    for p in &files {
        let raw = std::fs::read(p).unwrap();
        let w = World::from_save_seeded(&Save::parse(&raw).unwrap(), &d, 1);
        let out = w.to_save().to_bytes();
        assert_eq!(raw.len(), out.len(), "{}", p.display());
        let bad: Vec<usize> = (0..raw.len())
            .filter(|&i| raw[i] != out[i] && !volatile(i))
            .take(5)
            .collect();
        assert!(bad.is_empty(), "{}: differs at {bad:x?}", p.display());
    }
}

#[test]
fn every_old_map_converts_saves_and_reloads_to_the_same_bytes() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    for p in maps(&dir, "map") {
        let raw = std::fs::read(&p).unwrap();
        let w = World::from_save_seeded(&Save::parse(&raw).unwrap(), &d, 1);
        let saved = w.to_save().to_bytes();
        let again = Save::parse(&saved).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(again.layout.version, 0x2348);
        let w2 = World::from_save_seeded(&again, &d, 1);
        let resaved = w2.to_save().to_bytes();
        let bad: Vec<usize> = (0..saved.len())
            .filter(|&i| saved[i] != resaved[i] && !volatile(i))
            .take(5)
            .collect();
        assert!(bad.is_empty(), "{}: differs at {bad:x?}", p.display());
    }
}

#[test]
fn the_overview_matches_the_one_the_original_saved_in_every_mp2() {
    let Some(dir) = install() else { return };
    let mut checked = 0;
    for p in maps(&dir, "mp2") {
        let save = Save::parse(&std::fs::read(&p).unwrap()).unwrap();
        let mine = editor::preview(&save);
        let o = editor::header::PREVIEW;
        let theirs = &save.header[o..o + mine.len()];
        // Files saved from a game (not the map editor) hold the 117x117 colour screenshot of
        // `0x477040`'s other branch (bytes above 3); only the map branch is compared.
        if theirs.iter().any(|&b| b > 3) {
            continue;
        }
        checked += 1;
        assert_eq!(theirs, &mine[..], "{}", p.display());
    }
    assert!(checked >= 5, "{checked} editor-saved overviews compared");
}

fn corner(s: &Save, x: usize, y: usize) -> i32 {
    i32::from(s.map.elevation[y * (s.map.width + 1) + x])
}

#[test]
fn raise_adds_the_ring_weighted_amount_and_lower_pulls_to_the_level() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    let before = w.editor_save().unwrap().clone();
    // 0x426480 mode 0: +((r - d) * amount + r / 2) / r with radius 5, amount 10 at the centre.
    w.editor_elevation(Elevation::Raise, 40, 40, 5, 10);
    let s = w.editor_save().unwrap();
    assert_eq!(
        corner(s, 40, 40) - corner(&before, 40, 40),
        (5 * 10 + 2) / 5
    );
    assert_eq!(
        corner(s, 43, 40) - corner(&before, 43, 40),
        (2 * 10 + 2) / 5
    );
    assert_eq!(
        corner(s, 45, 40),
        corner(&before, 45, 40),
        "outside the radius"
    );
    // Lower to level 0 (tool 3): every corner in the radius moves towards 0, never past it.
    w.editor_elevation(Elevation::Lower, 40, 40, 5, 20);
    let s = w.editor_save().unwrap();
    for dx in -4i32..=4 {
        let c = corner(s, (40 + dx) as usize, 40);
        assert!(c >= 0 || corner(&before, (40 + dx) as usize, 40) < 0);
    }
}

#[test]
fn flatten_levels_towards_the_height_under_the_click() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    let centre = corner(w.editor_save().unwrap(), 30, 30);
    for _ in 0..30 {
        w.editor_elevation(Elevation::Flatten, 30, 30, 3, 15);
    }
    let s = w.editor_save().unwrap();
    for (x, y) in [(29, 30), (31, 30), (30, 29), (30, 31)] {
        assert_eq!(corner(s, x, y), centre);
    }
}

#[test]
fn smoothing_does_not_increase_the_roughness() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    w.editor_elevation(Elevation::Raise, 50, 40, 2, 200);
    let rough = |s: &Save| -> i32 {
        let mut t = 0;
        for y in 35..45usize {
            for x in 45..55usize {
                t += (corner(s, x, y) - corner(s, x + 1, y)).abs();
            }
        }
        t
    };
    let before = rough(w.editor_save().unwrap());
    w.editor_elevation(Elevation::Smooth, 50, 40, 8, editor::SMOOTH_AMOUNT);
    assert!(rough(w.editor_save().unwrap()) < before);
}

#[test]
fn painting_sets_the_class_and_the_beach_follows_water() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    w.editor_paint(60, 40, 2, 7);
    let s = w.editor_save().unwrap();
    for (x, y) in [(60, 40), (59, 41), (62, 40), (58, 38)] {
        assert_eq!(editor::class_at(s, x, y), 7);
    }
    // Water next to land turns the land into beach (0x426d30).
    w.editor_paint(60, 40, 0, 18);
    let s = w.editor_save().unwrap();
    assert_eq!(editor::class_at(s, 60, 40), 18);
    assert_eq!(
        editor::class_at(s, 61, 40),
        0,
        "a neighbour of the water is beach"
    );
    assert_eq!(editor::class_at(s, 62, 40), 7);
}

#[test]
fn fill_paints_the_connected_region_and_water_levels_it() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    w.editor_paint(30, 30, 7, 4);
    assert!(w.editor_fill(30, 30, 5, FillMode::Match, true));
    let s = w.editor_save().unwrap();
    assert_eq!(editor::class_at(s, 30, 30), 5);
    assert_eq!(editor::class_at(s, 36, 34), 5);
    // Fill all with water from a start tile: its four corners all take the same height.
    assert!(w.editor_fill(30, 30, 18, FillMode::Match, true));
    let s = w.editor_save().unwrap();
    let h = corner(s, 30, 30);
    assert_eq!(corner(s, 31, 31), h);
}

#[test]
fn trees_come_and_go() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    w.editor_paint(70, 50, 3, 5);
    let n = |w: &World| -> u32 {
        w.editor_save()
            .unwrap()
            .map
            .tiles
            .iter()
            .map(|t| u32::from(t.trees & 0x1f))
            .sum()
    };
    let before = n(&w);
    for _ in 0..4 {
        w.editor_trees(70, 50, 5, 1, 4);
    }
    assert!(n(&w) > before, "trees added");
    for _ in 0..12 {
        w.editor_trees(70, 50, 5, -1, 4);
    }
    assert!(n(&w) <= before + 1, "trees removed");
}

#[test]
fn rivers_roads_territories_and_reserved_cells() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    for y in 20..26 {
        w.editor_river(10, y, true);
    }
    let mw = w.editor_save().unwrap().map.width;
    assert!(w.editor_save().unwrap().map.tiles[22 * mw + 10].var & 1 != 0);
    w.editor_river(10, 22, false);
    assert!(w.editor_save().unwrap().map.tiles[22 * mw + 10].var & 1 == 0);
    w.editor_road(12, 30, true);
    assert!(w.editor_save().unwrap().map.tiles[30 * mw + 12].trees & 0x20 != 0);
    let before = w.editor_save().unwrap().map.tiles[40 * mw + 5].var;
    w.editor_reserve(5, 40, 2);
    w.editor_reserve(5, 40, 2);
    assert_eq!(w.editor_save().unwrap().map.tiles[40 * mw + 5].var, before);
    let t = w.editor_territory_add("North").unwrap();
    w.editor_territory_paint(20, 20, 3, t);
    let s = w.editor_save().unwrap();
    assert_eq!(s.map.tiles[20 * s.map.width + 20].region, t);
    assert_eq!(w.editor_territories().last().unwrap().name, "North");
}

#[test]
fn places_buildings_and_the_bulldozer() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let mut w = small_world(&dir, &d);
    let n0 = w.editor_places().len();
    let id = w
        .editor_place_add(80, 50, PlaceKind::City, "Newtown")
        .unwrap();
    assert_eq!(w.editor_places().len(), n0 + 1);
    assert_eq!(
        w.editor_place_add(80, 50, PlaceKind::Region, "Again"),
        Err(editor::EditorError::PlaceHere)
    );
    let p = w.editor_place(id).unwrap();
    assert_eq!((p.name.as_str(), p.size, p.kind), ("Newtown", 2, 0));
    assert!(p.weights.iter().all(|&x| x == 100));
    // A building from the editor list (shapes 24..121), then bulldoze it.
    let objects = |w: &World| w.editor_save().unwrap().objects.count;
    let n_obj = objects(&w);
    let mut placed = None;
    'search: for shape in 24..122 {
        for y in (45..55).step_by(2) {
            for x in (78..88).step_by(2) {
                if w.editor_can_build(x, y, shape) && w.editor_build(x, y, shape, -1).is_ok() {
                    placed = Some((x, y));
                    break 'search;
                }
            }
        }
    }
    let (x, y) = placed.expect("some shape fits near the new city");
    assert_eq!(objects(&w), n_obj + 1);
    w.editor_bulldoze(x, y, 1).unwrap();
    // `building_destroy` keeps the record, marked inactive (`+0x4f`), and frees the tiles.
    let s = w.editor_save().unwrap();
    assert!(s.objects.records.iter().any(|r| r.data[0x4f] == 1));
    let t = &s.map.tiles[y as usize * s.map.width + x as usize];
    assert_eq!(t.flags2 & 0x10, 0, "no building structure left on the tile");
    w.editor_place_remove(id).unwrap();
    assert_eq!(w.editor_places().len(), n0);
    // Saving gives a file the parser reads to the last byte.
    w.editor_recompute();
    let mut s = w.to_save();
    editor::refresh_preview(&mut s);
    Save::parse(&s.to_bytes()).unwrap();
}

#[test]
fn the_same_tools_with_the_same_seed_give_the_same_file() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let run = || {
        let mut w = small_world(&dir, &d);
        w.editor_paint(40, 40, 3, 6);
        w.editor_trees(40, 40, 7, 1, 5);
        w.editor_fill(40, 40, 3, FillMode::Match, true);
        w.editor_paint_mixed(60, 30, 7, 9);
        w.to_save().to_bytes()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_new_map_is_blank_saves_and_loads_again() {
    let Some(dir) = install() else { return };
    let d = data(&dir);
    let raw = std::fs::read(dir.join("maps/Local Industry.MP2")).unwrap();
    let template = Save::parse(&raw).unwrap();
    let s = editor::new_map(&template, 64, 48, 5, "Blank");
    assert_eq!((s.map.width, s.map.height), (64, 48));
    assert_eq!(s.objects.count + s.places.count + s.stations.count, 0);
    assert!(s.rules.is_empty());
    assert_eq!(editor::map_name(&s), "Blank");
    let bytes = s.to_bytes();
    let back = Save::parse(&bytes).unwrap();
    assert_eq!((back.map.width, back.map.height), (64, 48));
    let mut w = World::from_save_seeded(&back, &d, 1);
    w.editor_paint(10, 10, 3, 18);
    w.editor_place_add(30, 30, PlaceKind::City, "First")
        .unwrap();
    w.editor_recompute();
    Save::parse(&w.to_save().to_bytes()).unwrap();
}

#[test]
fn rebuilding_an_untouched_rule_keeps_its_record() {
    let Some(dir) = install() else { return };
    let (mut total, mut same) = (0, 0);
    for p in maps(&dir, "mp2") {
        let save = Save::parse(&std::fs::read(&p).unwrap()).unwrap();
        for r in &save.rules {
            let rule = rt2_sim::scenario::Rule::from_file(r);
            let b = rt2_sim::scenario::edit::build(Some(r), &rule);
            total += 1;
            same += usize::from(b.fixed == r.fixed && b.raw_blobs == r.raw_blobs);
        }
    }
    eprintln!("{same} of {total} rules rebuilt identically");
    assert!(same * 100 >= total * 99, "{same} of {total}");
}
