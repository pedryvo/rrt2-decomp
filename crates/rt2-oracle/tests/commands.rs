//! Oracle tests for the command layer (`docs/spec/commands.md`): the packet constructor
//! `cmd_packet_init` (`0x4af890`), the `(id, size)` of every packet the windows send, the
//! building/industry tables behind `0x17` and `0x2d`, the sender-side validation of the industry
//! purchase (`building_buy_request` `0x43ea30`, `building_sell_request` `0x43eb20`) and of the
//! territory rights purchase (`0x43e760`, price `0x440fb0`), the executor `cmd_execute`
//! (`0x4b0210`) for the commands that touch only plain records (`0x12`, `0x1b`, `0x28`, `0x29`,
//! `0x2b`, `0x2c`, `0x2f`) and the track-palette gates `max_bridge_type` (`0x469ef0`) and
//! `electric_track_available` (`0x469f30`).
//!
//! Each test lays its inputs out in the emulated memory, runs the original and compares it with
//! a reference written from the spec in this file; it depends only on `rt2-oracle`. The command
//! enqueue function `cmd_send` (`0x4af6d0`) is replaced by a recorder that copies the packet, so
//! the tests see the exact bytes a window would send. Window refreshes (`0x502200`), the message
//! box (`0x4510a0`) and the text lookup (`0x476910`) are patched to bare returns. Skipped when
//! the executable is absent. The x87 control word is pinned to `0x27f` (53 bits).

use std::collections::{BTreeMap, BTreeSet};

use rt2_oracle::Oracle;
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
    Some(Oracle::load_file(&path).expect("load exe"))
}

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
}

fn fpu_reset(o: &mut Oracle) {
    let uc = o.uc();
    uc.reg_write(RegisterX86::FPCW, 0x27f).unwrap();
    uc.reg_write(RegisterX86::FPSW, 0).unwrap();
    uc.reg_write(RegisterX86::FPTAG, 0xffff).unwrap();
}

fn w32(o: &mut Oracle, a: u32, v: u32) {
    o.write_u32(a, v).unwrap();
}
fn w16(o: &mut Oracle, a: u32, v: u16) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn w8(o: &mut Oracle, a: u32, v: u8) {
    o.write_mem(a, &[v]).unwrap();
}
fn wf64(o: &mut Oracle, a: u32, v: f64) {
    o.write_mem(a, &v.to_le_bytes()).unwrap();
}
fn r32(o: &Oracle, a: u32) -> u32 {
    o.read_u32(a).unwrap()
}
fn r16(o: &Oracle, a: u32) -> u16 {
    u16::from_le_bytes(o.read_mem(a, 2).unwrap().try_into().unwrap())
}
fn r8(o: &Oracle, a: u32) -> u8 {
    o.read_mem(a, 1).unwrap()[0]
}
fn rf32(o: &Oracle, a: u32) -> f32 {
    f32::from_le_bytes(o.read_mem(a, 4).unwrap().try_into().unwrap())
}

// ---- globals and layouts -------------------------------------------------------------------------

const GAME_TIME: u32 = 0x0057_04f4; // game_time_minutes
const GAME_DATE: u32 = 0x0057_04f8; // u16 year, u8 month, u8 day
const SCENARIO: u32 = 0x0057_4bc8; // pointer to the scenario options
const LOCAL_COMPANY: u32 = 0x0057_4414;
const EDITOR_FLAG: u32 = 0x0057_3d7a; // scenario-editor flag (`0x573d7a`)
const SANDBOX: u32 = 0x0057_0229;
const RNG_STATE: u32 = 0x0055_d2c0;
const UI_BUSY: u32 = 0x0057_fee0;
const LIST_STATIONS: u32 = 0x0057_4c60;
const LIST_TRAINS: u32 = 0x0057_4c64;
const LIST_COMPANIES: u32 = 0x0057_4c6c;
const LIST_PERSONS: u32 = 0x0057_4c68;
const LIST_OBJECTS: u32 = 0x0057_4c74;
const LIST_TERRITORIES: u32 = 0x0057_4ca8;

const CMD_SEND: u32 = 0x004a_f6d0;
const CMD_INIT: u32 = 0x004a_f890;
const CMD_EXECUTE: u32 = 0x004b_0210;

const SHAPES: u32 = 0x0054_57ef; // 122 records of 0x154 bytes, +9 type
const SHAPE_STRIDE: u32 = 0x154;
const SHAPE_COUNT: u32 = 122;
const TYPES: u32 = 0x0053_5c38; // 53 records of 0x27 bytes, +1 buyable
const TYPE_STRIDE: u32 = 0x27;
const TYPE_NAME_IDS: u32 = 0x0055_36e8; // u16 LNG id per type

const CO_SIZE: u32 = 0x4c00;
const CO_RIGHTS: u32 = 0x479; // + territory * 0x1ed
const CO_BLOCK: u32 = 0x1ed;
const CO_PERSON: u32 = 0x23;
const CO_CASH: u32 = 0x4766; // f64
const OBJ_SIZE: u32 = 0xa4;
const TERR_SIZE: u32 = 0x300;
const TR_SIZE: u32 = 0x200;
const ST_SIZE: u32 = 0x500;
const PERSON_SIZE: u32 = 0x600;

/// A pool object (`docs/spec/world-tick.md` 1): record `id` at `base + id * stride`; ids must be
/// below `+0x10` and have their bit clear in the free bitmap at `+0x34`. Returns the records.
fn make_pool(o: &mut Oracle, global: u32, stride: u32, n: u32) -> u32 {
    let base = o.alloc(stride * (n + 1)).unwrap();
    let hdr = o.alloc(0x40).unwrap();
    let bitmap = o.alloc(n / 8 + 16).unwrap();
    w32(o, hdr + 8, stride);
    w32(o, hdr + 0x10, n + 1);
    w32(o, hdr + 0x14, n);
    w32(o, hdr + 0x28, base);
    w32(o, hdr + 0x34, bitmap);
    w32(o, global, hdr);
    base
}

/// Marks pool slot `id` as free (the executor must then ignore commands naming it).
fn free_slot(o: &mut Oracle, global: u32, id: u32) {
    let hdr = r32(o, global);
    let bitmap = r32(o, hdr + 0x34);
    let b = r8(o, bitmap + id / 8);
    w8(o, bitmap + id / 8, b | (1 << (id & 7)));
}

/// Replaces `cmd_send` by a recorder: copies `size` bytes (`[ecx + 4]`) of the packet to a
/// buffer and counts the calls. Returns `(buffer, counter address)`.
fn install_recorder(o: &mut Oracle) -> (u32, u32) {
    let buf = o.alloc(0x400).unwrap();
    let cnt = o.alloc(0x10).unwrap();
    let mut c = vec![0x60, 0x89, 0xCE, 0xBF]; // pushad; mov esi,ecx; mov edi,imm32
    c.extend(buf.to_le_bytes());
    c.extend([0x8B, 0x4E, 0x04, 0xF3, 0xA4]); // mov ecx,[esi+4]; rep movsb
    c.extend([0xFF, 0x05]); // inc dword [cnt]
    c.extend(cnt.to_le_bytes());
    c.extend([0x61, 0xC3]); // popad; ret
    o.write_mem(CMD_SEND, &c).unwrap();
    (buf, cnt)
}

/// Patches the UI helpers the commands touch to bare returns (matching their stack cleanup).
fn patch_ui(o: &mut Oracle) {
    o.write_mem(0x0050_2200, &[0xC3]).unwrap(); // window refresh (fastcall, no stack args)
    o.write_mem(0x0047_6910, &[0x31, 0xC0, 0xC3]).unwrap(); // LNG text: xor eax,eax; ret
    o.write_mem(0x0045_10a0, &[0x31, 0xC0, 0xC2, 0x24, 0x00])
        .unwrap(); // message box (9 stack args): xor eax,eax; ret 0x24
}

// ---- 1. packet constructor and the (id, size) of every window packet ---------------------------

/// `(id, size)` of every packet a window or the network code builds (spec 2). Ids `0xc`/`0xd`
/// have no sender.
const PACKETS: [(u32, u32); 45] = [
    (0x01, 0x10),
    (0x02, 0x0d),
    (0x03, 0x0d),
    (0x04, 0x38),
    (0x05, 0xc2),
    (0x06, 0x0d),
    (0x07, 0x10),
    (0x08, 0x10),
    (0x09, 0x15),
    (0x0a, 0x13),
    (0x0b, 0x0e),
    (0x0e, 0x10),
    (0x0f, 0x0e),
    (0x10, 0x0f),
    (0x11, 0x11),
    (0x12, 0x11),
    (0x13, 0x0d),
    (0x14, 0x12),
    (0x15, 0x0f),
    (0x16, 0x1a),
    (0x17, 0x13),
    (0x18, 0x12),
    (0x19, 0x12),
    (0x1a, 0x0f),
    (0x1b, 0x10),
    (0x1c, 0x15),
    (0x1d, 0x15),
    (0x1e, 0x0f),
    (0x1f, 0x0e),
    (0x20, 0x0f),
    (0x21, 0x0f),
    (0x22, 0x10),
    (0x23, 0x12),
    (0x24, 0x0e),
    (0x25, 0x12),
    (0x26, 0x35),
    (0x27, 0x2d),
    (0x28, 0x2d),
    (0x29, 0x0f),
    (0x2a, 0x13),
    (0x2b, 0x10),
    (0x2c, 0x0e),
    (0x2d, 0x12),
    (0x2e, 0x0e),
    (0x2f, 0x10),
];

#[test]
fn packet_ids_and_sizes_match_spec() {
    let Some(o) = oracle() else {
        return;
    };
    // Every call to `cmd_packet_init` through its jump-table thunk (`0x401b8b`) is
    // `push size; mov edx, id; lea ecx, [buf]; call thunk`.
    let text = o.read_mem(0x0040_1000, 0x0013_4000).unwrap();
    let mut seen: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for i in 24..text.len() - 5 {
        if text[i] != 0xe8 {
            continue;
        }
        let rel = i32::from_le_bytes(text[i + 1..i + 5].try_into().unwrap());
        let va = 0x0040_1000u32 + i as u32;
        if va.wrapping_add(5).wrapping_add(rel as u32) != 0x0040_1b8b {
            continue;
        }
        let w = &text[i - 24..i];
        let mut at = None;
        for j in 0..w.len() - 4 {
            if w[j] == 0xba && w[j + 2] == 0 && w[j + 3] == 0 && w[j + 4] == 0 {
                at = Some(j);
            }
        }
        let j = at.expect("mov edx, id before the call");
        let id = u32::from(w[j + 1]);
        let size = if w[j - 2] == 0x6a {
            u32::from(w[j - 1])
        } else {
            assert_eq!(w[j - 5], 0x68, "push size before mov edx at {va:#x}");
            u32::from_le_bytes(w[j - 4..j].try_into().unwrap())
        };
        seen.entry(id).or_default().insert(size);
    }
    let want: BTreeMap<u32, BTreeSet<u32>> = PACKETS
        .iter()
        .map(|&(id, size)| (id, BTreeSet::from([size])))
        .collect();
    assert_eq!(seen, want);
    assert!(!seen.contains_key(&0xc) && !seen.contains_key(&0xd));
}

#[test]
fn packet_init_zeroes_and_stamps_the_header() {
    let Some(mut o) = oracle() else {
        return;
    };
    let buf = o.alloc(0x100).unwrap();
    for (id, size) in PACKETS {
        o.write_mem(buf, &[0xaa; 0x100]).unwrap();
        o.call_fastcall(CMD_INIT, buf, id, &[size]).unwrap();
        let b = o.read_mem(buf, 0x100).unwrap();
        assert_eq!(u32::from_le_bytes(b[0..4].try_into().unwrap()), id);
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), size);
        assert!(
            b[8..size as usize].iter().all(|&x| x == 0),
            "body of {id:#x}"
        );
        assert!(
            b[size as usize..].iter().all(|&x| x == 0xaa),
            "tail of {id:#x}"
        );
    }
}

// ---- 2. the tables behind 0x17 and 0x2d ----------------------------------------------------------

/// `(first shape, last shape, type, base price)` of `building_shape_table` (spec 3.3).
const SHAPE_GROUPS: &[(u32, u32, u8, i32)] = &[
    (0, 7, 0, 50_000),
    (8, 15, 0, 100_000),
    (16, 23, 0, 200_000),
    (24, 24, 22, 200_000),
    (25, 32, 18, 100_000),
    (33, 36, 16, 250_000),
    (37, 37, 2, 600_000),
    (38, 38, 34, 400_000),
    (39, 39, 31, 200_000),
    (40, 40, 27, 300_000),
    (41, 41, 25, 500_000),
    (42, 42, 24, 1_000_000),
    (43, 43, 15, 300_000),
    (44, 44, 13, 300_000),
    (45, 45, 12, 200_000),
    (46, 46, 23, 400_000),
    (47, 47, 17, 200_000),
    (48, 51, 11, 200_000),
    (52, 52, 9, 300_000),
    (53, 53, 8, 300_000),
    (54, 54, 7, 400_000),
    (55, 55, 5, 250_000),
    (56, 56, 3, 300_000),
    (57, 57, 32, 800_000),
    (58, 58, 35, 300_000),
    (59, 59, 36, 200_000),
    (60, 60, 1, 500_000),
    (61, 61, 6, 150_000),
    (62, 65, 10, 150_000),
    (66, 66, 14, 400_000),
    (67, 67, 20, 300_000),
    (68, 68, 21, 150_000),
    (69, 69, 26, 200_000),
    (70, 73, 29, 200_000),
    (74, 77, 30, 150_000),
    (78, 78, 37, 600_000),
    (79, 82, 33, 200_000),
    (83, 83, 4, 150_000),
    (84, 87, 28, 300_000),
    (88, 88, 19, 200_000),
    (89, 89, 38, 2_000_000),
    (90, 97, 39, 120_000),
    (98, 105, 40, 300_000),
    (106, 113, 41, 150_000),
    (114, 114, 42, 400_000),
    (115, 115, 43, 250_000),
    (116, 116, 44, 200_000),
    (117, 117, 45, 700_000),
    (118, 118, 46, 500_000),
    (119, 119, 47, 10_000_000),
    (120, 121, 48, 10_000_000),
];

#[test]
fn shape_table_matches_spec() {
    let Some(o) = oracle() else {
        return;
    };
    let mut covered = 0u32;
    for &(first, last, ty, price) in SHAPE_GROUPS.iter() {
        assert_eq!(first, covered, "groups are contiguous");
        for s in first..=last {
            let a = SHAPES + s * SHAPE_STRIDE;
            assert_eq!(r32(&o, a) as i32, price, "price of shape {s}");
            assert_eq!(r8(&o, a + 9), ty, "type of shape {s}");
        }
        covered = last + 1;
    }
    assert_eq!(covered, SHAPE_COUNT);
    // stations: shapes 0..24 = size * 8 + orientation; +0xa size, +0xb orientation
    for s in 0..24u32 {
        let a = SHAPES + s * SHAPE_STRIDE;
        assert_eq!(r8(&o, a + 0xa), (s / 8) as u8, "size of station shape {s}");
        assert_eq!(
            r8(&o, a + 0xb),
            (s % 8) as u8,
            "orientation of station shape {s}"
        );
    }
    // the name of a type is LNG 920 + type, and 3010 + (type - 38) for the expansion types
    for t in 0..53u32 {
        let want = if t < 38 { 920 + t } else { 3010 + t - 38 };
        assert_eq!(
            u32::from(r16(&o, TYPE_NAME_IDS + 2 * t)),
            want,
            "LNG id of type {t}"
        );
    }
}

#[test]
fn buyable_industry_types_match_spec() {
    let Some(o) = oracle() else {
        return;
    };
    let flagged: Vec<u32> = (0..53u32)
        .filter(|t| r8(&o, TYPES + 1 + t * TYPE_STRIDE) == 1)
        .collect();
    let not: Vec<u32> = (0..53u32)
        .filter(|t| r8(&o, TYPES + 1 + t * TYPE_STRIDE) == 0)
        .collect();
    assert_eq!(not, [0, 18, 38, 39, 43, 44, 47, 48]);
    // types 49..=52 are flagged but have no shape: 41 reachable buyable types (spec 3.6)
    let with_shape: BTreeSet<u32> = (0..SHAPE_COUNT)
        .map(|s| u32::from(r8(&o, SHAPES + s * SHAPE_STRIDE + 9)))
        .collect();
    let buyable: Vec<u32> = flagged
        .into_iter()
        .filter(|t| with_shape.contains(t))
        .collect();
    assert_eq!(buyable.len(), 41);
    let want: Vec<u32> = (1..=17)
        .chain(19..=37)
        .chain([40, 41, 42, 45, 46])
        .collect();
    assert_eq!(buyable, want);
}

// ---- 3. building_buy_request / building_sell_request (0x2d) --------------------------------------

struct World {
    o: Oracle,
    rec: u32,
    cnt: u32,
    company: u32,
    objects: u32,
}

const BUILDING: u32 = 3;
const COMPANY_ID: u32 = 2;
const TERRITORY: u32 = 3;

fn industry_world() -> Option<World> {
    let mut o = oracle()?;
    patch_ui(&mut o);
    let (rec, cnt) = install_recorder(&mut o);
    let objects = make_pool(&mut o, LIST_OBJECTS, OBJ_SIZE, 8);
    let company = o.alloc(CO_SIZE).unwrap();
    w32(&mut o, company, COMPANY_ID);
    w32(&mut o, LOCAL_COMPANY, 0);
    // an owned building: the price is then the paid price at +0x99 (no effect lookup)
    let b = objects + BUILDING * OBJ_SIZE;
    w16(&mut o, b + 0x97, 5);
    w8(&mut o, b + 0x9d, TERRITORY as u8);
    Some(World {
        o,
        rec,
        cnt,
        company,
        objects,
    })
}

impl World {
    fn set(&mut self, rights: u8, cash: f64, price: u32) {
        let (c, b) = (self.company, self.objects + BUILDING * OBJ_SIZE);
        w8(&mut self.o, c + CO_RIGHTS + TERRITORY * CO_BLOCK, rights);
        wf64(&mut self.o, c + CO_CASH, cash);
        w32(&mut self.o, b + 0x99, price);
        w32(&mut self.o, self.cnt, 0);
        self.o.write_mem(self.rec, &[0u8; 0x20]).unwrap();
    }
    fn sent(&self) -> u32 {
        r32(&self.o, self.cnt)
    }
}

#[test]
fn building_buy_request_validation_and_packet() {
    let Some(mut w) = industry_world() else {
        return;
    };
    let mut xs = Xs(0x9e37_79b9);
    for case in 0..400 {
        let rights = xs.below(16) as u8;
        let price = 1000 * (1 + xs.below(900));
        let cash = match xs.below(3) {
            0 => f64::from(price),                           // exactly enough
            1 => f64::from(price) - 0.5,                     // just short
            _ => f64::from(xs.below(2_000_000)) - 100_000.0, // anywhere
        };
        w.set(rights, cash, price);
        fpu_reset(&mut w.o);
        let ok =
            w.o.call_thiscall(0x0043_ea30, w.company, &[BUILDING])
                .unwrap();
        // spec 3.6: the right is bit 1 of the rights byte (not bit 2), and `price <= cash`
        let want = rights & 2 != 0 && f64::from(price) <= cash;
        assert_eq!(
            ok != 0,
            want,
            "case {case}: rights {rights:#x} cash {cash} price {price}"
        );
        assert_eq!(w.sent(), u32::from(want), "case {case}: packets");
        if want {
            let p = w.o.read_mem(w.rec, 0x12).unwrap();
            assert_eq!(u32::from_le_bytes(p[0..4].try_into().unwrap()), 0x2d);
            assert_eq!(u32::from_le_bytes(p[4..8].try_into().unwrap()), 0x12);
            assert_eq!(p[0xc], COMPANY_ID as u8);
            assert_eq!(
                u32::from_le_bytes(p[0xd..0x11].try_into().unwrap()),
                BUILDING
            );
            assert_eq!(p[0x11], 1, "buy flag");
        }
    }
}

#[test]
fn building_request_by_the_local_company_shows_the_message_and_sends_nothing() {
    let Some(mut w) = industry_world() else {
        return;
    };
    w32(&mut w.o, LOCAL_COMPANY, COMPANY_ID);
    for (rights, cash) in [(0u8, 1e9), (2, 0.0), (0xd, 1e9)] {
        w.set(rights, cash, 500_000);
        fpu_reset(&mut w.o);
        let ok =
            w.o.call_thiscall(0x0043_ea30, w.company, &[BUILDING])
                .unwrap();
        assert_eq!(ok, 0, "rights {rights:#x} cash {cash}");
        assert_eq!(w.sent(), 0);
    }
}

#[test]
fn building_sell_request_packet() {
    let Some(mut w) = industry_world() else {
        return;
    };
    w.set(0, 0.0, 400_000);
    fpu_reset(&mut w.o);
    w.o.call_thiscall(0x0043_eb20, w.company, &[BUILDING])
        .unwrap();
    assert_eq!(w.sent(), 1);
    let p = w.o.read_mem(w.rec, 0x12).unwrap();
    assert_eq!(u32::from_le_bytes(p[0..4].try_into().unwrap()), 0x2d);
    assert_eq!(u32::from_le_bytes(p[4..8].try_into().unwrap()), 0x12);
    assert_eq!(p[0xc], COMPANY_ID as u8);
    assert_eq!(
        u32::from_le_bytes(p[0xd..0x11].try_into().unwrap()),
        BUILDING
    );
    assert_eq!(p[0x11], 0, "sell flag");
}

// ---- 4. territory rights (0x2a) -----------------------------------------------------------------

/// Spec 3.7: `y = f*f` below 1, `1 + 3 (f*f - 1)` above, `f = (15000 - g) * 0.0001`.
fn rights_price_ref(base_thousands: i16, goodwill: u32) -> i32 {
    let f = f64::from(15_000 - goodwill as i32) * 0.0001;
    let y = if f <= 1.0 {
        f * f
    } else {
        1.0 + 3.0 * (f * f - 1.0)
    };
    let v = (f64::from(i32::from(base_thousands) * 1000) * y) as i32; // ftol truncates
    (v / 1000) * 1000
}

struct Rights {
    o: Oracle,
    rec: u32,
    cnt: u32,
    company: u32,
    terr: u32,
    person: u32,
}

fn rights_world() -> Option<Rights> {
    let mut o = oracle()?;
    patch_ui(&mut o);
    let (rec, cnt) = install_recorder(&mut o);
    let companies = make_pool(&mut o, LIST_COMPANIES, CO_SIZE, 4);
    let terr = make_pool(&mut o, LIST_TERRITORIES, TERR_SIZE, 6);
    let persons = make_pool(&mut o, LIST_PERSONS, PERSON_SIZE, 4);
    let company = companies + COMPANY_ID * CO_SIZE;
    w32(&mut o, company, COMPANY_ID);
    w32(&mut o, LOCAL_COMPANY, 0);
    w32(&mut o, 0x0057_04f8, 1870); // game date: year 1870
    Some(Rights {
        o,
        rec,
        cnt,
        company,
        terr,
        person: persons + PERSON_SIZE,
    })
}

#[test]
fn territory_rights_price_matches_formula() {
    let Some(mut w) = rights_world() else {
        return;
    };
    let mut xs = Xs(0x1357_9bdf);
    let mut goodwills = BTreeSet::new();
    for case in 0..300 {
        let t = 1 + xs.below(5);
        let bit = xs.below(4);
        // the goodwill inputs (spec 3.7; `company_goodwill` `0x43eca0`): per-territory blocks
        for k in 1..=5u32 {
            let blk = w.company + CO_RIGHTS + k * CO_BLOCK;
            w16(&mut w.o, blk + 1, xs.below(2000) as u16); // position in the territory
            w16(&mut w.o, blk + 3, xs.below(3000) as u16); // track cells
            w16(&mut w.o, blk + 5, xs.below(800) as u16); // loads this year
            w16(&mut w.o, blk + 7, xs.below(900) as u16); // loads lifetime
        }
        w32(&mut w.o, w.company + 0x148, 1850 + xs.below(30)); // founding year
        let base = xs.below(3000) as i16 + 1;
        let tr = w.terr + t * TERR_SIZE;
        w16(&mut w.o, tr + 0x3b + 2 * bit, base as u16);
        fpu_reset(&mut w.o);
        let g = w.o.call_thiscall(0x0043_eca0, w.company, &[t]).unwrap();
        assert!(g <= 10_000, "goodwill is clamped to 0..=10000");
        goodwills.insert(g);
        fpu_reset(&mut w.o);
        let price =
            w.o.call_fastcall(0x0044_0fb0, COMPANY_ID, t, &[bit])
                .unwrap() as i32;
        assert_eq!(
            price,
            rights_price_ref(base, g),
            "case {case}: g {g} base {base}"
        );
        assert_eq!(price % 1000, 0);
    }
    assert!(goodwills.len() > 20, "the goodwill inputs vary");
}

#[test]
fn territory_rights_request_validation_and_packet() {
    let Some(mut w) = rights_world() else {
        return;
    };
    let mut xs = Xs(0x2468_ace1);
    let (t, bit) = (2u32, 3u32);
    let tr = w.terr + t * TERR_SIZE;
    for case in 0..200 {
        w16(&mut w.o, tr + 0x3b + 2 * bit, (1 + xs.below(900)) as u16);
        let separate = xs.below(2) as u8;
        w8(&mut w.o, tr + 0x43, separate);
        let human = xs.below(2);
        // `company+0x23` = person (0 = none); the check only applies to a human chairman
        w32(
            &mut w.o,
            w.company + CO_PERSON,
            if xs.below(4) == 0 { 0 } else { 1 },
        );
        w32(&mut w.o, w.person + 0x122, human);
        fpu_reset(&mut w.o);
        let price =
            w.o.call_fastcall(0x0044_0fb0, COMPANY_ID, t, &[bit])
                .unwrap();
        let cash = f64::from(xs.below(3_000_000));
        wf64(&mut w.o, w.company + CO_CASH, cash);
        w32(&mut w.o, w.cnt, 0);
        w.o.write_mem(w.rec, &[0u8; 0x20]).unwrap();
        fpu_reset(&mut w.o);
        let ok =
            w.o.call_thiscall(0x0043_e760, w.company, &[t, bit])
                .unwrap();
        let chairman = r32(&w.o, w.company + CO_PERSON) > 0;
        let refused = chairman && human != 0 && cash < f64::from(price);
        assert_eq!(ok != 0, !refused, "case {case}: price {price} cash {cash}");
        assert_eq!(r32(&w.o, w.cnt), u32::from(!refused));
        if !refused {
            let p = w.o.read_mem(w.rec, 0x13).unwrap();
            assert_eq!(u32::from_le_bytes(p[0..4].try_into().unwrap()), 0x2a);
            assert_eq!(u32::from_le_bytes(p[4..8].try_into().unwrap()), 0x13);
            assert_eq!(p[0xc], COMPANY_ID as u8);
            assert_eq!(p[0xd], t as u8);
            // separate purchases off: all four rights; on: just the clicked one
            assert_eq!(p[0xe], if separate == 0 { 0xf } else { 1 << bit });
            assert_eq!(u32::from_le_bytes(p[0xf..0x13].try_into().unwrap()), price);
        }
    }
}

// ---- 5. cmd_execute on plain records -------------------------------------------------------------

struct Exec {
    o: Oracle,
    trains: u32,
    stations: u32,
    companies: u32,
    stops: u32,
}

const TRAIN: u32 = 4;
const STATION: u32 = 3;
const STOP_STRIDE: u32 = 0xe;
const STOPS: u32 = 5;

fn exec_world() -> Option<Exec> {
    let mut o = oracle()?;
    patch_ui(&mut o);
    let trains = make_pool(&mut o, LIST_TRAINS, TR_SIZE, 8);
    let stations = make_pool(&mut o, LIST_STATIONS, ST_SIZE, 8);
    let companies = make_pool(&mut o, LIST_COMPANIES, CO_SIZE, 4);
    // stops array of train 4 and the fields the executors read
    let stops = o.alloc(STOP_STRIDE * 8).unwrap();
    let t = trains + TRAIN * TR_SIZE;
    w32(&mut o, t + 0x37, stops);
    w8(&mut o, t + 0x3b, STOPS as u8);
    w32(&mut o, t + 0x41, 1); // owner: company 1, local company stays 0 (no UI branches)
    w32(&mut o, LOCAL_COMPANY, 0);
    w32(&mut o, GAME_TIME, 123_456);
    Some(Exec {
        o,
        trains,
        stations,
        companies,
        stops,
    })
}

impl Exec {
    fn train(&self) -> u32 {
        self.trains + TRAIN * TR_SIZE
    }
    fn run(&mut self, packet: &[u8]) {
        let p = self.o.alloc_bytes(packet).unwrap();
        fpu_reset(&mut self.o);
        w32(&mut self.o, UI_BUSY, 0);
        self.o.call_fastcall(CMD_EXECUTE, p, 0, &[]).unwrap();
        assert_eq!(r32(&self.o, UI_BUSY), 0, "the busy counter is balanced");
    }
}

fn packet(id: u32, size: usize, body: &[(usize, &[u8])]) -> Vec<u8> {
    let mut p = vec![0u8; size];
    p[0..4].copy_from_slice(&id.to_le_bytes());
    p[4..8].copy_from_slice(&(size as u32).to_le_bytes());
    for (off, bytes) in body {
        p[*off..*off + bytes.len()].copy_from_slice(bytes);
    }
    p
}

#[test]
fn cmd_seed_rng_0x2b() {
    let Some(mut e) = exec_world() else {
        return;
    };
    for seed in [1u32, 0x1234_5678, 99_999, 0xffff_fff0] {
        w32(&mut e.o, 0x0057_8af8, 77);
        e.run(&packet(0x2b, 0x10, &[(0xc, &seed.to_le_bytes())]));
        assert_eq!(r32(&e.o, RNG_STATE), seed);
        assert_eq!(r32(&e.o, 0x0057_8af8), 0);
    }
}

#[test]
fn cmd_rename_station_0x28() {
    let Some(mut e) = exec_world() else {
        return;
    };
    let st = e.stations + STATION * ST_SIZE;
    let mut name = [0u8; 31];
    name[..11].copy_from_slice(b"New Aachen\0");
    // bytes past the NUL are not copied
    name[20] = b'X';
    let p = packet(
        0x28,
        0x2d,
        &[(0xc, &(STATION as u16).to_le_bytes()), (0xe, &name)],
    );
    e.o.write_mem(st + 0x1a, &[b'#'; 31]).unwrap();
    e.run(&p);
    let got = e.o.read_mem(st + 0x1a, 31).unwrap();
    assert_eq!(&got[..11], b"New Aachen\0");
    assert!(
        got[11..].iter().all(|&b| b == b'#'),
        "only strlen + 1 bytes are written"
    );
    // a free slot, an id past the pool and station 0 do nothing
    for id in [STATION, 99, 0] {
        if id == STATION {
            free_slot(&mut e.o, LIST_STATIONS, STATION);
        }
        e.o.write_mem(st + 0x1a, &[b'#'; 31]).unwrap();
        e.run(&packet(
            0x28,
            0x2d,
            &[(0xc, &(id as u16).to_le_bytes()), (0xe, &name)],
        ));
        assert!(
            e.o.read_mem(st + 0x1a, 31)
                .unwrap()
                .iter()
                .all(|&b| b == b'#'),
            "id {id}"
        );
    }
}

#[test]
fn cmd_train_mark_0x2c_and_dead_trains_are_ignored() {
    let Some(mut e) = exec_world() else {
        return;
    };
    let t = e.train();
    e.run(&packet(0x2c, 0xe, &[(0xc, &(TRAIN as u16).to_le_bytes())]));
    assert_eq!(r32(&e.o, t + 0x6d), 123_456);
    assert_eq!(r8(&e.o, t + 0x9d), 1);
    w32(&mut e.o, t + 0x6d, 0);
    w8(&mut e.o, t + 0x9d, 0);
    free_slot(&mut e.o, LIST_TRAINS, TRAIN);
    e.run(&packet(0x2c, 0xe, &[(0xc, &(TRAIN as u16).to_le_bytes())]));
    assert_eq!((r32(&e.o, t + 0x6d), r8(&e.o, t + 0x9d)), (0, 0));
    // an id past the pool (the spec's "live ids" rule)
    e.run(&packet(0x2c, 0xe, &[(0xc, &200u16.to_le_bytes())]));
}

#[test]
fn cmd_stop_modes_0x1b_and_0x2f() {
    let Some(mut e) = exec_world() else {
        return;
    };
    let mut xs = Xs(0x0bad_cafe);
    for case in 0..200 {
        let stop = xs.below(STOPS + 2) as u8; // 5 and 6 are out of range
        let old = xs.below(256) as u8;
        let mode = xs.below(256) as u8;
        let sandbox = xs.below(2) as u8;
        w8(&mut e.o, SANDBOX, sandbox);
        for k in 0..STOPS {
            w8(&mut e.o, e.stops + k * STOP_STRIDE + 3, old);
        }
        // 0x1b: wait mode in bits 6..7 (`mode` is a signed char shifted left by 6)
        e.run(&packet(
            0x1b,
            0x10,
            &[
                (0xc, &(TRAIN as u16).to_le_bytes()),
                (0xe, &[stop]),
                (0xf, &[mode]),
            ],
        ));
        for k in 0..STOPS {
            let got = r8(&e.o, e.stops + k * STOP_STRIDE + 3);
            let want = if u32::from(stop) == k {
                (old & 0x3f) | (mode << 6)
            } else {
                old
            };
            assert_eq!(got, want, "0x1b case {case} stop {k}");
        }
        for k in 0..STOPS {
            w8(&mut e.o, e.stops + k * STOP_STRIDE + 3, old);
        }
        // 0x2f: sell mode in bits 4..5; the sandbox flag forces both bits on
        e.run(&packet(
            0x2f,
            0x10,
            &[
                (0xc, &(TRAIN as u16).to_le_bytes()),
                (0xe, &[stop]),
                (0xf, &[mode]),
            ],
        ));
        for k in 0..STOPS {
            let got = r8(&e.o, e.stops + k * STOP_STRIDE + 3);
            let mut want = old;
            if u32::from(stop) == k {
                want = ((mode & 3) << 4) | (old & 0xcf);
                if sandbox != 0 {
                    want |= 0x30;
                }
            }
            assert_eq!(got, want, "0x2f case {case} stop {k}");
        }
    }
    w8(&mut e.o, SANDBOX, 0);
}

#[test]
fn cmd_train_throttle_0x29() {
    let Some(mut e) = exec_world() else {
        return;
    };
    let t = e.train();
    w16(&mut e.o, t + 0xcb, 0xffff); // tile x = -1: no "own track" bonus
    for (value, class) in [
        (0u8, 3u8),
        (1, 2),
        (65, 3),
        (66, 3),
        (66, 1),
        (100, 2),
        (255, 0),
    ] {
        w8(&mut e.o, t + 0xa7, class);
        e.run(&packet(
            0x29,
            0xf,
            &[(0xc, &(TRAIN as u16).to_le_bytes()), (0xe, &[value])],
        ));
        assert_eq!(rf32(&e.o, t + 0xdb), f32::from(value));
        // throttle <= 65: stopped (priority 0); above: priority = class
        let want = if value <= 65 { 0 } else { class };
        assert_eq!(r8(&e.o, t + 0xa8), want, "value {value} class {class}");
    }
}

#[test]
fn cmd_set_dividend_0x12() {
    let Some(mut e) = exec_world() else {
        return;
    };
    let co = e.companies + COMPANY_ID * CO_SIZE;
    w32(&mut e.o, co, COMPANY_ID);
    for d in [0.0f32, 0.25, 1.5, 10.0, -1.0] {
        e.run(&packet(
            0x12,
            0x11,
            &[(0xc, &[COMPANY_ID as u8]), (0xd, &d.to_le_bytes())],
        ));
        assert_eq!(rf32(&e.o, co + 0x30), d);
    }
}

// ---- 6. track palette gates ----------------------------------------------------------------------

/// Spec 3.2: scenario option `+0x1bf` if set, else the year decides (1840, 1880); the editor
/// flag skips the year limits.
fn max_bridge_ref(option: i32, year: u16, editor: bool) -> i32 {
    if option != 0 {
        option
    } else if year < 0x758 && !editor {
        if year < 0x730 { 1 } else { 2 }
    } else {
        3
    }
}

#[test]
fn max_bridge_type_matches_spec() {
    let Some(mut o) = oracle() else {
        return;
    };
    let opts = o.alloc(0x400).unwrap();
    w32(&mut o, SCENARIO, opts);
    let mut xs = Xs(0xfeed_0001);
    for case in 0..500 {
        let option = if xs.below(3) == 0 {
            xs.below(6) as i32
        } else {
            0
        };
        let year = match xs.below(4) {
            0 => 1839 + xs.below(3) as u16,
            1 => 1879 + xs.below(3) as u16,
            _ => 1800 + xs.below(200) as u16,
        };
        let editor = xs.below(4) == 0;
        w32(&mut o, opts + 0x1bf, option as u32);
        w16(&mut o, GAME_DATE, year);
        w32(&mut o, EDITOR_FLAG, u32::from(editor));
        fpu_reset(&mut o);
        let got = o.call_fastcall(0x0046_9ef0, 0, 0, &[]).unwrap() as i32;
        assert_eq!(
            got,
            max_bridge_ref(option, year, editor),
            "case {case}: {year} {option}"
        );
    }
}

const ENGINES: u32 = 0x0053_cec0; // 66 records of 0x3a bytes: +0 enabled, +0x28 kind, +0x2c vehicle
const ENGINE_STRIDE: u32 = 0x3a;
const VEHICLES: u32 = 0x0053_ddb8; // 175 records of 0x39 bytes: +0x1c first year, +0x1e last year
const VEHICLE_STRIDE: u32 = 0x39;
const MODIFIERS: u32 = 0x0057_07c6; // i16 percent per effect id
const ENGINE_ALLOWED: u32 = 0x0057_0667;

/// Spec (`trains.md` 2.4): a positive modifier forces the engine, a negative one forbids it;
/// otherwise it needs the engine enabled, allowed on the map and inside the vehicle's years
/// (it appears in June of the first year).
fn engine_available_ref(
    modifier: i32,
    enabled: bool,
    allowed: bool,
    first: u16,
    last: u16,
    year: u16,
    month: u8,
) -> bool {
    if modifier > 0 {
        true
    } else if modifier < 0 {
        false
    } else {
        enabled && allowed && (first < year || (year == first && month > 5)) && year <= last
    }
}

/// A world with the pools `engine_available` reads (companies, persons) and random engine data.
fn engine_world(o: &mut Oracle, xs: &mut Xs) {
    make_pool(o, LIST_COMPANIES, CO_SIZE, 4);
    make_pool(o, LIST_PERSONS, PERSON_SIZE, 4);
    let opts = o.alloc(0x400).unwrap();
    w32(o, SCENARIO, opts);
    w32(o, LOCAL_COMPANY, 1);
    w32(o, EDITOR_FLAG, 0);
    for e in 0..66u32 {
        w8(o, ENGINES + e * ENGINE_STRIDE + 0x2c, xs.below(175) as u8);
    }
}

#[test]
fn engine_available_matches_spec() {
    let Some(mut o) = oracle() else {
        return;
    };
    let mut xs = Xs(0x5eed_0007);
    engine_world(&mut o, &mut xs);
    let mut seen = BTreeSet::new();
    for case in 0..2000 {
        let e = xs.below(66);
        let year = 1800 + xs.below(200) as u16;
        let month = 1 + xs.below(12) as u8;
        w16(&mut o, GAME_DATE, year);
        w8(&mut o, GAME_DATE + 2, month);
        // pin the vehicle years around the date so that every branch is reached
        let veh = u32::from(r8(&o, ENGINES + e * ENGINE_STRIDE + 0x2c));
        let first = year - 3 + xs.below(7) as u16;
        let last = first + xs.below(40) as u16;
        w16(&mut o, VEHICLES + veh * VEHICLE_STRIDE + 0x1c, first);
        w16(&mut o, VEHICLES + veh * VEHICLE_STRIDE + 0x1e, last);
        let enabled = xs.below(5) != 0;
        w8(&mut o, ENGINES + e * ENGINE_STRIDE, u8::from(enabled));
        let allowed = xs.below(4) != 0;
        w8(&mut o, ENGINE_ALLOWED + e, u8::from(allowed));
        let id = if e < 0x3e { 0x7f + e } else { 0xf3 + e };
        let modifier = match xs.below(6) {
            0 => 1 + xs.below(50) as i32,
            1 => -(1 + xs.below(50) as i32),
            _ => 0,
        };
        w16(&mut o, MODIFIERS + 2 * id, modifier as i16 as u16);
        fpu_reset(&mut o);
        let got = o.call_fastcall(0x0046_83a0, e, 1, &[]).unwrap() != 0;
        let want = engine_available_ref(modifier, enabled, allowed, first, last, year, month);
        assert_eq!(
            got, want,
            "case {case}: engine {e} year {year}/{month} {first}..{last}"
        );
        w16(&mut o, MODIFIERS + 2 * id, 0);
        seen.insert(got);
    }
    assert_eq!(seen.len(), 2);
}

#[test]
fn electric_track_is_offered_only_with_an_available_electric_engine() {
    let Some(mut o) = oracle() else {
        return;
    };
    let mut xs = Xs(0xabcd_1234);
    engine_world(&mut o, &mut xs);
    let opts = r32(&o, SCENARIO);
    let mut seen = BTreeSet::new();
    for case in 0..300 {
        let year = 1800 + xs.below(200) as u16;
        w16(&mut o, GAME_DATE, year);
        w8(&mut o, GAME_DATE + 2, 1 + xs.below(12) as u8);
        for e in 0..66u32 {
            w8(&mut o, ENGINE_ALLOWED + e, u8::from(xs.below(40) == 0));
        }
        let option = u32::from(xs.below(6) == 0);
        let editor = u32::from(xs.below(8) == 0);
        w32(&mut o, opts + 100, option);
        w32(&mut o, EDITOR_FLAG, editor);
        let mut any = false;
        for e in 0..66u32 {
            if r8(&o, ENGINES + e * ENGINE_STRIDE + 0x28) == 2 {
                fpu_reset(&mut o);
                any |= o.call_fastcall(0x0046_83a0, e, 1, &[]).unwrap() != 0;
            }
        }
        fpu_reset(&mut o);
        let got = o.call_fastcall(0x0046_9f30, 0, 0, &[]).unwrap() != 0;
        // spec 3.2: the option (`+0x64`) or the editor offer it unconditionally
        assert_eq!(
            got,
            option != 0 || editor != 0 || any,
            "case {case}: year {year}"
        );
        seen.insert(got);
    }
    assert_eq!(seen.len(), 2, "both outcomes were exercised");
}
