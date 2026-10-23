//! Map Options pages of the editor that edit the scenario's players, player pool, managers,
//! engines, ports and robbers (docs/formats/map-gam.md section 10; the dialog is `0x4881d0`,
//! its control handler `0x47ef50`, the page builders `0x480040..0x487380`).
//!
//! Everything works on the raw [`Save`], like the rest of [`crate::editor`]. Offsets are file
//! offsets of the header block (`0x567b38 + off`) or of the state block (`0x5704e8 + off`).

use rt2_formats::map::Save;

use crate::data::DataImage;
use crate::pool;
use crate::rng::GameRng;
use crate::world::World;

// ---- offsets ---------------------------------------------------------------------------------

/// Header `0x567c88` (`+0x150`): six u32 counts of the players (see [`refresh_player_summary`]).
const HDR_SUMMARY: usize = 0x150;
/// Header `0x567ca0` (`+0x168`): 32 slots of 5 bytes (persona, mandatory, computer, human,
/// override) of the players in list order.
const HDR_SLOTS: usize = 0x168;
/// Header `0x567d40` (`+0x208`): the player pool, one byte per persona 1..=40.
const HDR_POOL: usize = 0x208;
/// Header `0x57023b` (`+0x8703`): the Port row of the industry table (4 slots of 0x20 bytes at
/// `+4`, `docs/spec/cargo.md` 7.1.1).
const HDR_PORTS: usize = 0x8703;
/// Header `0x5704a8` (`+0x8970`): i32 "Start offset days" of the Special Restrictions page
/// (`0x83a1`, clamp `0..=0x16c`).
const HDR_START_OFFSET: usize = 0x8970;
/// State `0x573d9e` (`+0x38b6`): i32 overall robbery level (slider `0x83cc`).
const ST_ROBBERY: usize = 0x38b6;
/// Header `0x5702cb` (`+0x8793`): 53 records of 9 bytes by industry type (`+0` disabled by the
/// scenario, `+1` i32 weight in percent, `+5` i32 weighting 0..=4 shown by the page).
const HDR_INDUSTRY: usize = 0x8793;
/// Header bytes of the Regional page (`0x4855e0`): steam, diesel, electric cost level and the
/// passenger traffic level (0 high, 1 normal, 2 low).
const HDR_REGIONAL: usize = 0x235;
const HDR_TRAFFIC: usize = 0x238;
/// Header bytes of the Computer A.I. page (`0x485230`): track expansion and dividend policy.
const HDR_AI_TRACK: usize = 0x8975;
const HDR_AI_DIVIDEND: usize = 0x8976;
/// State `0x570638`: one byte per manager (40).
const ST_MANAGERS: usize = 0x570638 - 0x5704e8;
/// State `0x570667`: one byte per engine (66), `docs/spec/trains.md` 2.4.
const ST_ENGINES: usize = 0x570667 - 0x5704e8;
/// State `0x573da2`: four robbers of 0x18 bytes (name[20] + NUL, `+0x15` unknown, i16 year at
/// `+0x16`).
const ST_ROBBERS: usize = 0x573da2 - 0x5704e8;
/// State `0x570775` and `0x570779`: the player list.
const ST_PL_COUNT: usize = 0x28d;
const ST_PL_LIST: usize = 0x291;

/// Players the list holds (the Add button is disabled at 32, `0x484df0`).
pub const MAX_PLAYERS: usize = 32;
pub const PERSONA_COUNT: usize = 41;
pub const POOL_SIZE: usize = 40;
pub const MANAGER_COUNT: usize = 40;
pub const ENGINE_COUNT: usize = 66;
pub const PORT_COUNT: usize = 4;
pub const ROBBER_COUNT: usize = 4;
/// Largest overall robbery level (slider `0x83cc` of the Robbers page, range `0..=0x191`).
pub const ROBBERY_MAX: u32 = 0x191;
/// Largest "Start offset days" (`0x83a1`: clamp `0x16c`).
pub const START_OFFSET_MAX: u32 = 0x16c;
/// Industry types of the Industry page list (types 1..=48, `0x486d70` adds `0x30` rows).
pub const INDUSTRY_TYPES: usize = 48;
/// Robber names hold 20 characters (`_strncpy(.., 0x14)`).
pub const ROBBER_NAME_LEN: usize = 20;
/// No cargo in a port slot.
pub const NO_CARGO: u8 = 0x7f;
/// Number of cargo names of the port lists (`0x553630`, u16 LNG ids).
pub const PORT_CARGOS: usize = 45;

/// `ai_persona_table` `0x551e88` (0x23 bytes per record): region group at `+2`, name LNG id at
/// `+0x1f`, description LNG id at `+0x21`.
const PERSONA_TABLE: u32 = 0x0055_1e88;
const PERSONA_SIZE: u32 = 0x23;
/// `engine_table` `0x53cec0` (0x3a per record): region columns A, E, W at `+5..+7`.
const ENGINE_TABLE: u32 = 0x0053_cec0;
/// `engine_name_ids` `0x5534f4` and the port cargo name table `0x553630`.
const ENGINE_NAMES: u32 = 0x0055_34f4;
const PORT_CARGO_NAMES: u32 = 0x0055_3630;

// person record (`pool 0x574c68`, 0x576 bytes)
const P_ACTIVE: usize = 4;
const P_NAME: usize = 8;
const P_NAME_LEN: usize = 30;
const P_DESC: usize = 0x27;
const P_DESC_LEN: usize = 250;
const P_HUMAN_SLOT: usize = 0x122;
const P_CASH: usize = 0x12a;
const P_PERSONA_COPY: usize = 0x132;
const P_PERSONA: usize = 0x52f;
/// 0 = mandatory, 1 = optional.
const P_OPTIONAL: usize = 0x530;
/// 1 = a computer player may take the slot.
const P_COMPUTER: usize = 0x531;
/// 1 = a human may take the slot.
const P_HUMAN: usize = 0x532;
/// 1 = "allow user to override" (the player may pick another persona).
const P_OVERRIDE: usize = 0x533;

/// Why an edit was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptError {
    /// "The first player is always mandatory." (LNG 2290)
    FirstMandatory,
    /// "The first player may not be an AI only player." (LNG 2291)
    FirstNotAi,
    /// The first player cannot be deleted (`0x8312`).
    FirstPlayer,
    /// 32 players exist.
    TooManyPlayers,
    NoSuchPlayer,
    NoSuchTerritory,
    /// The first territory cannot be removed (LNG 2392).
    FirstTerritory,
    /// Another territory remaps to this one (LNG 2393).
    Remapped(u8),
}

// ---- players -----------------------------------------------------------------------------

/// Who may take a slot (`0x831a..0x831c`: human only, computer only, either).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Controller {
    Human,
    Computer,
    Either,
}

/// One player slot of the Players page.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    /// Slot in the persons pool.
    pub slot: u16,
    /// Persona 0..=40 (0 = unassigned: drawn at game start).
    pub persona: i8,
    pub mandatory: bool,
    pub controller: Controller,
    pub allow_override: bool,
    pub cash: f64,
    pub name: String,
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    pool::u32_at(b, o)
}

fn list_count(s: &Save) -> usize {
    (u32_at(&s.state, ST_PL_COUNT) as usize).min(MAX_PLAYERS)
}

/// Slots of the active players in list order (`0x570779`).
pub fn player_slots(s: &Save) -> Vec<u16> {
    (0..list_count(s))
        .map(|k| pool::u16_at(&s.state, ST_PL_LIST + 2 * k))
        .collect()
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn put_cstr(b: &mut [u8], text: &str, max: usize) {
    let bytes = text.as_bytes();
    let n = bytes.len().min(max);
    // `_strncpy(dst, src, max)`: copies up to `max` bytes and pads with NULs.
    b[..max].fill(0);
    b[..n].copy_from_slice(&bytes[..n]);
}

pub fn player_info(s: &Save, slot: u16) -> Option<PlayerInfo> {
    let r = pool::record(&s.persons, usize::from(slot))?;
    let d = &r.data;
    if d.len() < 0x551 {
        return None;
    }
    let controller = match (d[P_COMPUTER] != 0, d[P_HUMAN] != 0) {
        (false, true) => Controller::Human,
        (true, false) => Controller::Computer,
        _ => Controller::Either,
    };
    Some(PlayerInfo {
        slot,
        persona: d[P_PERSONA] as i8,
        mandatory: d[P_OPTIONAL] == 0,
        controller,
        allow_override: d[P_OVERRIDE] != 0,
        cash: pool::f64_at(d, P_CASH),
        name: cstr(&d[P_NAME..P_NAME + P_NAME_LEN]),
    })
}

/// The active players in list order.
pub fn players(s: &Save) -> Vec<PlayerInfo> {
    player_slots(s)
        .into_iter()
        .filter_map(|id| player_info(s, id))
        .collect()
}

/// Records of the persons pool that are not players (`0x46a8c0`).
fn free_records(s: &Save) -> usize {
    s.persons.count.saturating_sub(list_count(s))
}

/// `0x46a920`: appends a person to the player list.
fn list_push(s: &mut Save, id: u16) {
    let n = list_count(s);
    if n >= MAX_PLAYERS {
        return;
    }
    pool::put_u16(&mut s.state, ST_PL_LIST + 2 * n, id);
    pool::put_u32(&mut s.state, ST_PL_COUNT, n as u32 + 1);
}

/// `0x46a940`: removes a person from the player list.
fn list_remove(s: &mut Save, id: u16) {
    let ids = player_slots(s);
    let Some(i) = ids.iter().position(|&p| p == id) else {
        return;
    };
    for k in i..ids.len() - 1 {
        pool::put_u16(&mut s.state, ST_PL_LIST + 2 * k, ids[k + 1]);
    }
    pool::put_u16(&mut s.state, ST_PL_LIST + 2 * (ids.len() - 1), 0);
    pool::put_u32(&mut s.state, ST_PL_COUNT, ids.len() as u32 - 1);
}

fn persona_group(data: &DataImage, p: usize) -> u8 {
    data.u8(PERSONA_TABLE + p as u32 * PERSONA_SIZE + 2)
}

fn persona_text_ids(data: &DataImage, p: usize) -> (u32, u32) {
    let base = PERSONA_TABLE + p as u32 * PERSONA_SIZE;
    (
        u32::from(data.u16(base + 0x1f)),
        u32::from(data.u16(base + 0x21)),
    )
}

/// Name and description of persona `p` as the editor shows them (`0x47ef50` common tail): the
/// unassigned persona is "Unassigned <slot>".
pub fn persona_texts(
    data: &DataImage,
    text: &dyn Fn(u32) -> String,
    p: usize,
    slot: u16,
) -> (String, String) {
    let (n, d) = persona_text_ids(data, p);
    let name = if p == 0 {
        format!("{} {}", text(0x14), slot)
    } else {
        text(n)
    };
    (name, text(d))
}

fn apply_persona(
    s: &mut Save,
    data: &DataImage,
    text: &dyn Fn(u32) -> String,
    slot: u16,
    persona: usize,
) {
    let (name, desc) = persona_texts(data, text, persona, slot);
    if let Some(r) = pool::record_mut(&mut s.persons, usize::from(slot)) {
        let d = &mut r.data;
        d[P_PERSONA] = persona as u8;
        d[P_PERSONA_COPY] = persona as u8;
        put_cstr(&mut d[P_NAME..], &name, P_NAME_LEN);
        put_cstr(&mut d[P_DESC..], &desc, P_DESC_LEN);
    }
}

/// `0x4a7990(current, step, allow_unassigned)`: the next persona not used by an active person,
/// walking `step` from `current` (a random start when `current < 0`). Draws once from the game
/// RNG like the original. Returns 0 after 99 failed tries.
pub fn next_persona(s: &Save, rng: &mut GameRng, current: i32, step: i32, allow_zero: bool) -> i32 {
    let mut used = [false; PERSONA_COUNT];
    for r in &s.persons.records {
        let d = &r.data;
        if d.len() > P_PERSONA && (d[P_PERSONA] as i8) >= 0 && u32_at(d, P_ACTIVE) != 0 {
            let p = usize::from(d[P_PERSONA]);
            if p < PERSONA_COUNT {
                used[p] = true;
            }
        }
    }
    used[0] = !allow_zero;
    let n = PERSONA_COUNT as i32;
    let mut i = rng.range(0, 0x28);
    if current >= 0 {
        i = (step + n + current) % n;
    }
    for _ in 0..0x63 {
        if !used[i as usize] {
            return i;
        }
        i = (i + n + step) % n;
    }
    0
}

/// Arrow buttons `0x8315` (previous, `dir` -1) and `0x8316` (next, `dir` 1): another persona for
/// the player in `slot`.
pub fn step_persona(
    s: &mut Save,
    rng: &mut GameRng,
    data: &DataImage,
    text: &dyn Fn(u32) -> String,
    slot: u16,
    dir: i32,
) -> Result<(), OptError> {
    let cur = pool::record(&s.persons, usize::from(slot))
        .map(|r| i32::from(r.data[P_PERSONA] as i8))
        .ok_or(OptError::NoSuchPlayer)?;
    let p = next_persona(s, rng, cur, dir.signum(), true);
    apply_persona(s, data, text, slot, p as usize);
    Ok(())
}

/// The Add button (`0x8311`, `0x4a6a60`, `0x4a6b00`): a new unassigned player with 100000 cash.
/// A person record that is not a player is reused first (and keeps its flags); otherwise a new
/// record is allocated.
pub fn add_player(
    s: &mut Save,
    rng: &mut GameRng,
    data: &DataImage,
    text: &dyn Fn(u32) -> String,
) -> Result<u16, OptError> {
    if list_count(s) >= MAX_PLAYERS {
        return Err(OptError::TooManyPlayers);
    }
    let slot;
    if free_records(s) > 0 {
        let id = s
            .persons
            .ordered()
            .find(|r| u32_at(&r.data, P_ACTIVE) == 0)
            .map(|r| r.slot)
            .ok_or(OptError::TooManyPlayers)?;
        slot = id as u16;
        if let Some(r) = pool::record_mut(&mut s.persons, id) {
            pool::put_u32(&mut r.data, P_ACTIVE, 1);
            pool::put_f64(&mut r.data, P_CASH, 100_000.0);
        }
        list_push(s, slot);
    } else {
        let size = s.persons.record_size.max(0x576);
        let id =
            pool::alloc(&mut s.persons, vec![0u8; size]).map_err(|_| OptError::TooManyPlayers)?;
        slot = id as u16;
        let first = list_count(s) == 0;
        let age = rng.range(0, 0x28); // the persona-used flag search draws once
        let _ = age;
        let born = rng.range(0x3c, 0x1c2);
        let mut d = vec![0u8; size];
        pool::put_u32(&mut d, 0, id as u32);
        pool::put_u32(&mut d, P_ACTIVE, 1);
        let (flag, comp) = if first { (0, 0) } else { (1, 1) };
        d[P_OPTIONAL] = flag;
        d[P_COMPUTER] = comp;
        d[P_HUMAN] = 1;
        d[P_OVERRIDE] = 1;
        pool::put_u32(&mut d, P_HUMAN_SLOT, u32::from(id == 1));
        pool::put_f64(&mut d, P_CASH, 100_000.0);
        pool::put_u32(&mut d, 0x267, (born * 0x5a0) as u32);
        if let Some(r) = pool::record_mut(&mut s.persons, id) {
            r.data = d;
        }
        list_push(s, slot);
    }
    apply_persona(s, data, text, slot, 0);
    Ok(slot)
}

/// The Delete button (`0x8312`, `0x4a6a00`): the player leaves the list and its record is kept
/// for the next Add. The first player cannot be deleted.
pub fn remove_player(s: &mut Save, slot: u16) -> Result<(), OptError> {
    let ids = player_slots(s);
    if ids.first() == Some(&slot) {
        return Err(OptError::FirstPlayer);
    }
    if !ids.contains(&slot) {
        return Err(OptError::NoSuchPlayer);
    }
    if let Some(r) = pool::record_mut(&mut s.persons, usize::from(slot)) {
        r.data[0x1d3..0x1d3 + 0x90].fill(0);
        pool::put_u32(&mut r.data, P_ACTIVE, 0);
    }
    list_remove(s, slot);
    Ok(())
}

fn person_mut(s: &mut Save, slot: u16) -> Result<&mut Vec<u8>, OptError> {
    pool::record_mut(&mut s.persons, usize::from(slot))
        .map(|r| &mut r.data)
        .ok_or(OptError::NoSuchPlayer)
}

fn is_first(s: &Save, slot: u16) -> bool {
    player_slots(s).first() == Some(&slot)
}

/// Mandatory (`0x8317`) or optional (`0x8318`); the first player is always mandatory.
pub fn set_mandatory(s: &mut Save, slot: u16, mandatory: bool) -> Result<(), OptError> {
    if !mandatory && is_first(s, slot) {
        return Err(OptError::FirstMandatory);
    }
    person_mut(s, slot)?[P_OPTIONAL] = u8::from(!mandatory);
    Ok(())
}

/// Human only (`0x831a`), computer only (`0x831b`) or either (`0x831c`); the first player cannot
/// be computer only.
pub fn set_controller(s: &mut Save, slot: u16, c: Controller) -> Result<(), OptError> {
    if c == Controller::Computer && is_first(s, slot) {
        return Err(OptError::FirstNotAi);
    }
    let d = person_mut(s, slot)?;
    let (comp, human) = match c {
        Controller::Human => (0, 1),
        Controller::Computer => (1, 0),
        Controller::Either => (1, 1),
    };
    d[P_COMPUTER] = comp;
    d[P_HUMAN] = human;
    Ok(())
}

/// The "allow user to override" check box (`0x831e`).
pub fn set_override(s: &mut Save, slot: u16, on: bool) -> Result<(), OptError> {
    person_mut(s, slot)?[P_OVERRIDE] = u8::from(on);
    Ok(())
}

/// The cash box (`0x8329`): an integer amount stored as a double.
pub fn set_cash(s: &mut Save, slot: u16, cash: i64) -> Result<(), OptError> {
    pool::put_f64(person_mut(s, slot)?, P_CASH, cash as f64);
    Ok(())
}

/// The three count lines of the Players page: (mandatory, total), (humans-only mandatory,
/// humans allowed), (computer-only mandatory, computers allowed) (`0x46b890`).
pub fn player_counts(s: &Save) -> [u32; 6] {
    let mut c = [0u32; 6];
    for id in player_slots(s) {
        let Some(r) = pool::record(&s.persons, usize::from(id)) else {
            continue;
        };
        let d = &r.data;
        let mandatory = d[P_OPTIONAL] == 0;
        if mandatory {
            c[0] += 1;
        }
        c[1] += 1;
        if mandatory && d[P_COMPUTER] == 0 {
            c[2] += 1;
        }
        if d[P_HUMAN] != 0 {
            c[3] += 1;
        }
        if mandatory && d[P_HUMAN] == 0 {
            c[4] += 1;
        }
        if d[P_COMPUTER] != 0 {
            c[5] += 1;
        }
    }
    c
}

/// `0x46b890`: the summary of the players kept in the header block (the six counts and, per
/// player in list order, persona, optional, computer, human, override). The editor refreshes it
/// whenever the Players page changes; the file saver leaves it alone in editor mode (`0x477540`
/// writes its own counts only when `0x5748f8 == 0`).
pub fn refresh_player_summary(s: &mut Save) {
    let counts = player_counts(s);
    for (i, v) in counts.into_iter().enumerate() {
        if let Some(b) = s
            .header
            .get_mut(HDR_SUMMARY + 4 * i..HDR_SUMMARY + 4 * i + 4)
        {
            b.copy_from_slice(&v.to_le_bytes());
        }
    }
    if s.header.len() < HDR_SLOTS + 5 * MAX_PLAYERS {
        return;
    }
    s.header[HDR_SLOTS..HDR_SLOTS + 5 * MAX_PLAYERS].fill(0);
    for (k, id) in player_slots(s).into_iter().enumerate() {
        let Some(r) = pool::record(&s.persons, usize::from(id)) else {
            continue;
        };
        let d = &r.data;
        let o = HDR_SLOTS + 5 * k;
        let row = [
            d[P_PERSONA],
            d[P_OPTIONAL],
            d[P_COMPUTER],
            d[P_HUMAN],
            d[P_OVERRIDE],
        ];
        s.header[o..o + 5].copy_from_slice(&row);
    }
}

// ---- player pool ---------------------------------------------------------------------------

/// Persona `p` (1..=40) is in the pool unassigned slots draw from (`0x8a48 + p`).
pub fn pool_allowed(s: &Save, p: usize) -> bool {
    s.header.get(HDR_POOL + p).is_some_and(|&b| b != 0)
}

pub fn set_pool_allowed(s: &mut Save, p: usize, on: bool) {
    if (1..=POOL_SIZE).contains(&p)
        && let Some(b) = s.header.get_mut(HDR_POOL + p)
    {
        *b = u8::from(on);
    }
}

/// How many personas the pool holds.
pub fn pool_count(s: &Save) -> usize {
    (1..=POOL_SIZE).filter(|&p| pool_allowed(s, p)).count()
}

/// The North America (0), Europe (1) and World (2) buttons (`0x8339..0x833b`): the pool becomes
/// the personas of that region group.
pub fn pool_preset(s: &mut Save, data: &DataImage, group: u8) {
    for p in 1..=POOL_SIZE {
        let on = persona_group(data, p) == group;
        set_pool_allowed(s, p, on);
    }
}

/// Persona name for the pool page (LNG id from the table).
pub fn persona_name(data: &DataImage, text: &dyn Fn(u32) -> String, p: usize) -> String {
    text(persona_text_ids(data, p).0)
}

// ---- managers -------------------------------------------------------------------------------

pub fn manager_allowed(s: &Save, m: usize) -> bool {
    s.state.get(ST_MANAGERS + m).is_some_and(|&b| b != 0)
}

pub fn set_manager_allowed(s: &mut Save, m: usize, on: bool) {
    if m < MANAGER_COUNT
        && let Some(b) = s.state.get_mut(ST_MANAGERS + m)
    {
        *b = u8::from(on);
    }
}

pub fn manager_count(s: &Save) -> usize {
    (0..MANAGER_COUNT)
        .filter(|&m| manager_allowed(s, m))
        .count()
}

/// LNG id of manager `m`'s name (`1456 + 2m`, `docs/spec/finance.md` 14.2).
pub fn manager_name_id(m: usize) -> u32 {
    1456 + 2 * m as u32
}

// ---- engines --------------------------------------------------------------------------------

pub fn engine_allowed(s: &Save, e: usize) -> bool {
    s.state.get(ST_ENGINES + e).is_some_and(|&b| b != 0)
}

pub fn set_engine_allowed(s: &mut Save, e: usize, on: bool) {
    if e < ENGINE_COUNT
        && let Some(b) = s.state.get_mut(ST_ENGINES + e)
    {
        *b = u8::from(on);
    }
}

pub fn engine_count(s: &Save) -> usize {
    (0..ENGINE_COUNT).filter(|&e| engine_allowed(s, e)).count()
}

/// The North America (0), Europe (1) and World (2) buttons (`0x8330..0x8332`): copy the region
/// column (`engine_table + e*0x3a + 5 + col`).
pub fn engine_preset(s: &mut Save, data: &DataImage, column: usize) {
    for e in 0..ENGINE_COUNT {
        let v = data.u8(ENGINE_TABLE + e as u32 * 0x3a + 5 + column.min(2) as u32);
        set_engine_allowed(s, e, v != 0);
    }
}

/// LNG id of engine `e`'s name (`engine_name_ids`).
pub fn engine_name_id(data: &DataImage, e: usize) -> u32 {
    u32::from(data.u16(ENGINE_NAMES + 2 * e as u32))
}

// ---- ports ------------------------------------------------------------------------------------

/// One of the four port rows: what ports supply and demand (global, not per port).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortInfo {
    /// Annual supply (no demanded cargo needed) or manufactured from the demanded cargo 1:1.
    pub annual: bool,
    /// Supplied cargo (index in the cargo table of the list, [`NO_CARGO`] none).
    pub supplied: u8,
    /// Units per year, 1..=7 (a float in the file).
    pub quantity: f32,
    /// Demanded cargo ([`NO_CARGO`] none).
    pub demanded: u8,
}

fn port_base(k: usize) -> usize {
    HDR_PORTS + 4 + 0x20 * k
}

pub fn port(s: &Save, k: usize) -> Option<PortInfo> {
    let o = port_base(k);
    let h = s.header.get(o..o + 0xb)?;
    Some(PortInfo {
        annual: h[0] != 0,
        supplied: h[1],
        quantity: f32::from_le_bytes([h[2], h[3], h[4], h[5]]),
        demanded: h[6],
    })
}

/// `0x487ce0`'s data effects on one row: the second input slot is cleared, a manufactured row
/// has quantity 1.0 and "needs" its demand, an annual row's quantity is clamped to 1..=7.
fn port_normalize(s: &mut Save, k: usize) {
    let o = port_base(k);
    let h = &mut s.header[o..o + 0x20];
    h[0x11] = NO_CARGO;
    h[0x12] = 0;
    h[0x13..0x17].fill(0);
    h[0x17] = 0;
    h[0x18..0x1c].fill(0);
    if h[0] == 0 {
        h[7] = 1;
        h[2..6].copy_from_slice(&1.0f32.to_le_bytes());
    } else {
        let q = f32::from_le_bytes([h[2], h[3], h[4], h[5]]).clamp(1.0, 7.0);
        h[2..6].copy_from_slice(&q.to_le_bytes());
        h[7] = 0;
    }
}

/// "Annual supply" (`0x8345..`): `annual` true clears the demand; "Manufactured supply"
/// (`0x8351..`) gives an unset demand the default cargo 7.
pub fn set_port_annual(s: &mut Save, k: usize, annual: bool) {
    if k >= PORT_COUNT || s.header.len() < port_base(PORT_COUNT) {
        return;
    }
    let o = port_base(k);
    if annual {
        s.header[o] = 1;
        s.header[o + 6] = NO_CARGO;
    } else {
        s.header[o] = 0;
        if s.header[o + 6] == NO_CARGO {
            s.header[o + 6] = 7;
        }
    }
    port_normalize(s, k);
}

/// The supplied cargo dropdown (`0x8372..`; [`NO_CARGO`] for "none").
pub fn set_port_supplied(s: &mut Save, k: usize, cargo: u8) {
    if k < PORT_COUNT && s.header.len() >= port_base(PORT_COUNT) {
        s.header[port_base(k) + 1] = cargo;
        port_normalize(s, k);
    }
}

/// The demanded cargo dropdown (`0x8386..`).
pub fn set_port_demanded(s: &mut Save, k: usize, cargo: u8) {
    if k < PORT_COUNT && s.header.len() >= port_base(PORT_COUNT) {
        s.header[port_base(k) + 6] = cargo;
        port_normalize(s, k);
    }
}

/// The quantity box (`0x835d..`): an integer, clamped to 1..=7 when the row is annual.
pub fn set_port_quantity(s: &mut Save, k: usize, units: i32) {
    if k < PORT_COUNT && s.header.len() >= port_base(PORT_COUNT) {
        let o = port_base(k) + 2;
        s.header[o..o + 4].copy_from_slice(&(units as f32).to_le_bytes());
        port_normalize(s, k);
    }
}

/// LNG id of the name of port-list cargo `i` (0..45).
pub fn port_cargo_name_id(data: &DataImage, i: usize) -> u32 {
    u32::from(data.u16(PORT_CARGO_NAMES + 2 * i as u32))
}

// ---- robbers ----------------------------------------------------------------------------------

/// Overall robbery level (state `0x573d9e`, i32; the shipped maps hold 0..=400).
pub fn robbery_level(s: &Save) -> u32 {
    s.state
        .get(ST_ROBBERY..ST_ROBBERY + 4)
        .map_or(0, |b| u32_at(b, 0))
}

pub fn set_robbery_level(s: &mut Save, level: u32) {
    if let Some(b) = s.state.get_mut(ST_ROBBERY..ST_ROBBERY + 4) {
        b.copy_from_slice(&level.min(ROBBERY_MAX).to_le_bytes());
    }
}

/// Name and begin year of robber `k` (an empty name: the robber is not used; year 0: active from
/// the start).
pub fn robber(s: &Save, k: usize) -> Option<(String, u16)> {
    let o = ST_ROBBERS + 0x18 * k;
    let d = s.state.get(o..o + 0x18)?;
    Some((
        cstr(&d[..ROBBER_NAME_LEN]),
        u16::from_le_bytes([d[0x16], d[0x17]]),
    ))
}

pub fn set_robber_name(s: &mut Save, k: usize, name: &str) {
    let o = ST_ROBBERS + 0x18 * k;
    if k < ROBBER_COUNT && s.state.len() >= o + 0x18 {
        let n = name.len().min(ROBBER_NAME_LEN);
        s.state[o..o + ROBBER_NAME_LEN].fill(0);
        s.state[o..o + n].copy_from_slice(&name.as_bytes()[..n]);
        s.state[o + ROBBER_NAME_LEN] = 0;
    }
}

/// Begin year: below 1800 means none (0), above 2099 is 2100 (`0x83c2..`).
pub fn set_robber_year(s: &mut Save, k: usize, year: i32) {
    let o = ST_ROBBERS + 0x18 * k + 0x16;
    if k < ROBBER_COUNT && s.state.len() >= o + 2 {
        let y = if year < 1800 {
            0
        } else {
            year.min(2100) as u16
        };
        s.state[o..o + 2].copy_from_slice(&y.to_le_bytes());
    }
}

impl World {
    /// [`add_player`] on the world's map with the world's RNG.
    pub fn editor_player_add(
        &mut self,
        data: &DataImage,
        text: &dyn Fn(u32) -> String,
    ) -> Result<u16, OptError> {
        let sim = self.sim.as_mut().ok_or(OptError::NoSuchPlayer)?;
        add_player(&mut sim.save, &mut self.rng, data, text)
    }

    /// [`step_persona`] on the world's map with the world's RNG.
    pub fn editor_player_step(
        &mut self,
        data: &DataImage,
        text: &dyn Fn(u32) -> String,
        slot: u16,
        dir: i32,
    ) -> Result<(), OptError> {
        let sim = self.sim.as_mut().ok_or(OptError::NoSuchPlayer)?;
        step_persona(&mut sim.save, &mut self.rng, data, text, slot, dir)
    }
}

// ---- territories ------------------------------------------------------------------------------

impl World {
    /// Removes territory `id` (a clone extension: the original has no delete, its list hides the
    /// records whose `+0x23` is 0). The tiles of the territory go back to territory 1, the
    /// record is freed and its number can be given to the next new territory. Refused for the
    /// first territory and while another territory remaps to this one (LNG 2393).
    pub fn editor_territory_remove(&mut self, id: u8) -> Result<(), OptError> {
        let s = self.editor_save_mut().ok_or(OptError::NoSuchTerritory)?;
        territory_remove(s, id)
    }
}

/// See [`World::editor_territory_remove`].
pub fn territory_remove(s: &mut Save, id: u8) -> Result<(), OptError> {
    if pool::record(&s.territories, usize::from(id)).is_none() {
        return Err(OptError::NoSuchTerritory);
    }
    if s.territories.ordered().next().map(|r| r.slot) == Some(usize::from(id)) || id == 1 {
        return Err(OptError::FirstTerritory);
    }
    for r in s.territories.ordered() {
        if r.slot != usize::from(id) && r.data[crate::editor::territory::REGION] == id {
            return Err(OptError::Remapped(r.slot as u8));
        }
    }
    for t in &mut s.map.tiles {
        if t.region == id {
            t.region = 1;
        }
    }
    pool::free(&mut s.territories, usize::from(id));
    Ok(())
}

// ---- special restrictions (`0x480740`) ---------------------------------------------------------

/// The ten check boxes of the Special Restrictions page: header byte, ids `0x833d..` (LNG
/// `2871 + 2k` name, `+1` hint; the last five use `3331 + 2k`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    SlowDate,
    FixedCargo,
    NoMoney,
    NoCustoms,
    LimitedSupply,
    ShowVisited,
    OneShotWaste,
    LimitedTrack,
    TerritoryWars,
    Metra,
}

impl Special {
    pub const ALL: [Special; 10] = [
        Special::SlowDate,
        Special::FixedCargo,
        Special::NoMoney,
        Special::NoCustoms,
        Special::LimitedSupply,
        Special::ShowVisited,
        Special::OneShotWaste,
        Special::LimitedTrack,
        Special::TerritoryWars,
        Special::Metra,
    ];

    /// Header offset of the flag byte (`0x47ef50` toggles `byte == 0`).
    pub fn offset(self) -> usize {
        match self {
            Special::SlowDate => 0x86ff,
            Special::FixedCargo => 0x86fe,
            Special::NoMoney => 0x8700,
            Special::NoCustoms => 0x8701,
            Special::LimitedSupply => 0x86ef,
            Special::ShowVisited => 0x86f0,
            Special::OneShotWaste => 0x86f1,
            Special::LimitedTrack => 0x86f2,
            Special::TerritoryWars => 0x8702,
            Special::Metra => 0x8974,
        }
    }

    /// LNG id of the name; the hint is the next id.
    pub fn text_id(self) -> u32 {
        match self {
            Special::SlowDate => 2871,
            Special::FixedCargo => 2873,
            Special::NoMoney => 2875,
            Special::NoCustoms => 2877,
            Special::LimitedSupply => 3331,
            Special::ShowVisited => 3333,
            Special::OneShotWaste => 3335,
            Special::LimitedTrack => 3337,
            Special::TerritoryWars => 3339,
            Special::Metra => 3341,
        }
    }
}

pub fn special(s: &Save, f: Special) -> bool {
    s.header.get(f.offset()).is_some_and(|&b| b != 0)
}

/// The check box handlers flip the byte (`byte = (byte == 0)`).
pub fn toggle_special(s: &mut Save, f: Special) {
    if let Some(b) = s.header.get_mut(f.offset()) {
        *b = u8::from(*b == 0);
    }
}

/// "Start offset days" (0 means January 1).
pub fn start_offset_days(s: &Save) -> u32 {
    s.header
        .get(HDR_START_OFFSET..HDR_START_OFFSET + 4)
        .map_or(0, |b| u32_at(b, 0))
}

pub fn set_start_offset_days(s: &mut Save, days: u32) {
    if let Some(b) = s.header.get_mut(HDR_START_OFFSET..HDR_START_OFFSET + 4) {
        b.copy_from_slice(&days.min(START_OFFSET_MAX).to_le_bytes());
    }
}

// ---- regional (`0x4855e0`) and computer A.I. (`0x485230`) ---------------------------------------

/// Fuel and maintenance cost level of steam (0), diesel (1) or electric (2) trains: 0 high,
/// 1 normal, 2 low (ids `0x83d8 + 10 * row + level`).
pub fn cost_level(s: &Save, row: usize) -> u8 {
    s.header
        .get(HDR_REGIONAL + row)
        .copied()
        .unwrap_or(1)
        .min(2)
}

pub fn set_cost_level(s: &mut Save, row: usize, level: u8) {
    if row < 3
        && level < 3
        && let Some(b) = s.header.get_mut(HDR_REGIONAL + row)
    {
        *b = level;
    }
}

/// Passenger traffic level: 0 high, 1 normal, 2 low (ids `0x83f6..0x83f8`).
pub fn traffic_level(s: &Save) -> u8 {
    s.header.get(HDR_TRAFFIC).copied().unwrap_or(1).min(2)
}

pub fn set_traffic_level(s: &mut Save, level: u8) {
    if level < 3
        && let Some(b) = s.header.get_mut(HDR_TRAFFIC)
    {
        *b = level;
    }
}

/// Track expansion of A.I. companies: 0 conservative, 1 moderate, 2 aggressive (ids `0x83fc`
/// aggressive .. `0x83fe` conservative, `byte = 0x83fe - id`).
pub fn ai_track(s: &Save) -> u8 {
    s.header.get(HDR_AI_TRACK).copied().unwrap_or(0).min(2)
}

pub fn set_ai_track(s: &mut Save, v: u8) {
    if v < 3
        && let Some(b) = s.header.get_mut(HDR_AI_TRACK)
    {
        *b = v;
    }
}

/// Dividend and stock buyback policy of A.I. companies: 0 aggressive, 1 moderate, 2 none
/// (ids `0x8401..0x8403`, `byte = id - 0x8401`).
pub fn ai_dividend(s: &Save) -> u8 {
    s.header.get(HDR_AI_DIVIDEND).copied().unwrap_or(0).min(2)
}

pub fn set_ai_dividend(s: &mut Save, v: u8) {
    if v < 3
        && let Some(b) = s.header.get_mut(HDR_AI_DIVIDEND)
    {
        *b = v;
    }
}

// ---- industry types (`0x486d70`) ---------------------------------------------------------------

/// LNG ids of the industry names: `0x5536ea`, one per type from 1, and of the weighting names
/// (`0x553890`, 0..=4).
const INDUSTRY_NAMES: u32 = 0x0055_36ea;
const WEIGHT_NAMES: u32 = 0x0055_3890;

/// One row of the Industry page (item `i` is industry type `i + 1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndustryRow {
    pub type_id: usize,
    pub name_id: u32,
    /// "Allow this building type" (the record's disabled byte is 0).
    pub allowed: bool,
    /// LNG id of the weighting name.
    pub weight_name_id: u32,
}

pub fn industry_row(s: &Save, data: &DataImage, i: usize) -> Option<IndustryRow> {
    if i >= INDUSTRY_TYPES {
        return None;
    }
    let o = HDR_INDUSTRY + 9 * (i + 1);
    let rec = s.header.get(o..o + 9)?;
    let w = (u32_at(rec, 5) as usize).min(4);
    Some(IndustryRow {
        type_id: i + 1,
        name_id: u32::from(data.u16(INDUSTRY_NAMES + 2 * i as u32)),
        allowed: rec[0] == 0,
        weight_name_id: u32::from(data.u16(WEIGHT_NAMES + 2 * w as u32)),
    })
}

/// Number of industry types allowed.
pub fn industry_allowed_count(s: &Save) -> usize {
    (1..=INDUSTRY_TYPES)
        .filter(|&t| s.header.get(HDR_INDUSTRY + 9 * t).is_some_and(|&b| b == 0))
        .count()
}

/// The `0x82f9` check box: `byte = 1 - byte` of the selected type.
pub fn toggle_industry(s: &mut Save, i: usize) {
    if i < INDUSTRY_TYPES
        && let Some(b) = s.header.get_mut(HDR_INDUSTRY + 9 * (i + 1))
    {
        *b = 1 - (*b).min(1);
    }
}
