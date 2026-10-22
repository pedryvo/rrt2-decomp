//! Which sound plays for which game event, as found in the EXE (`docs/formats/sound.md`).
//!
//! The original calls `0x4eeeb0(sound, looping, volume_percent, frequency, delay)` from the UI,
//! command and train code. This module records those call sites as data so the game layer can
//! trigger them by event name. Confidence is `Verified` when the call site and arguments were read
//! in the decompilation and `Inferred` when only the sound name suggests the meaning.

use crate::ids::SoundId;

/// How sure the event to sound link is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Verified,
    Inferred,
}

/// Game events that have a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    ButtonUp,
    ButtonDown,
    ListClick,
    ListHover,
    Tab,
    PageTop,
    PageTurn,
    PaperFlip,
    PaperNew,
    Book,
    Radio,
    CargoSelect,
    CargoRemove,
    LockClosed,
    LockOpen,
    Campaign,
    Resource,
    BuildBuilding,
    Bulldoze,
    Chain,
    AddTrack,
    LayTrack,
    LayRiver,
    Raise,
    Brush,
    Bucket,
    Buy,
    StockTrade,
    StockSell,
    GameMode,
    BellA,
    BellB,
    TrainBreakdown,
    TrainCrash,
    TrainRobbery,
    TrainBrokenSteam,
    TrainBrokenOther,
    TrainFire,
    TrainNextStation,
    TrainHorse,
    TrainAddStation1,
    TrainAddStation2,
    TrainAddStation3,
    DesignSteam,
    DesignDiesel,
    DesignElectric,
    Clock,
    CombatBackground,
    CombatBackground2,
    AmbientHawk,
    AmbientGeese,
    AmbientFlyby,
    AmbientRoost,
    AmbientAfrica,
    AmbientPigeons,
    AmbientTrainIn,
    Announce,
}

/// One row of the event table.
#[derive(Debug, Clone, Copy)]
pub struct EventRow {
    pub event: Event,
    pub sound: SoundId,
    /// Volume the original passes, in percent (`0x4eeeb0` third argument).
    pub volume: u8,
    /// Played with the DirectSound looping flag (second argument non-zero).
    pub looping: bool,
    /// Address of the (first) call site in `RT2_PLAT.EXE`.
    pub addr: u32,
    pub confidence: Confidence,
}

const fn row(
    event: Event,
    sound: SoundId,
    volume: u8,
    looping: bool,
    addr: u32,
    confidence: Confidence,
) -> EventRow {
    EventRow {
        event,
        sound,
        volume,
        looping,
        addr,
        confidence,
    }
}

use Confidence::{Inferred as I, Verified as V};
use Event as E;

/// All events with a fixed sound. Families chosen by index (terrain ambience, engine chug and
/// whistle, brake variants, building loops, chimes) have their own functions below.
pub static EVENTS: [EventRow; 58] = [
    row(E::ButtonUp, SoundId::BUTTON_UP, 100, false, 0x50cbd0, V),
    row(E::ButtonDown, SoundId::BUTTON_DOWN, 100, false, 0x50cc60, V),
    row(E::ListClick, SoundId::LIST_SCROLL, 100, false, 0x506220, V),
    row(
        E::ListHover,
        SoundId::LIST_SCROLL_SOFT,
        90,
        false,
        0x5062f0,
        V,
    ),
    row(E::Tab, SoundId::TAB, 100, false, 0x401884, V),
    row(E::PageTop, SoundId::PAGE_TOP, 100, false, 0x4022c5, V),
    row(E::PageTurn, SoundId::PAGE_TURN, 100, false, 0x4027ac, V),
    row(E::PaperFlip, SoundId::PAPER_FLIP, 100, false, 0x4a4050, I),
    row(E::PaperNew, SoundId::PAPER_NEW, 100, false, 0x4a4050, V),
    row(E::Book, SoundId::BOOK, 100, false, 0x4ebbd0, V),
    row(E::Radio, SoundId::RADIO, 100, false, 0x4012da, V),
    row(E::CargoSelect, SoundId::CARGO, 100, false, 0x4012da, V),
    row(
        E::CargoRemove,
        SoundId::CARGO_REMOVE,
        100,
        false,
        0x4012da,
        V,
    ),
    row(E::LockClosed, SoundId::LOCK_CLOSED, 100, false, 0x401884, V),
    row(E::LockOpen, SoundId::LOCK_OPEN, 100, false, 0x401884, V),
    row(E::Campaign, SoundId::CAMPAIGN, 100, false, 0x411150, V),
    row(E::Resource, SoundId::RESOURCE, 100, false, 0x493970, V),
    row(
        E::BuildBuilding,
        SoundId::ADD_BUILDING,
        100,
        false,
        0x4b0210,
        V,
    ),
    row(E::Bulldoze, SoundId::BULLDOZE, 100, false, 0x4b0210, V),
    row(E::Chain, SoundId::CHAIN, 100, false, 0x48eb30, V),
    row(E::AddTrack, SoundId::ADD_TRACK, 100, false, 0x48eb30, V),
    row(E::LayTrack, SoundId::LAY_TRACK, 100, false, 0x495d50, V),
    row(E::LayRiver, SoundId::LAY_RIVER, 100, false, 0x48eb30, V),
    row(E::Raise, SoundId::RAISE, 100, false, 0x48eb30, V),
    row(E::Brush, SoundId::BRUSH, 100, false, 0x48eb30, V),
    row(E::Bucket, SoundId::BUCKET, 100, false, 0x48eb30, V),
    row(E::Buy, SoundId::BUY, 100, false, 0x4c1f80, V),
    row(E::StockTrade, SoundId::STOCK, 100, false, 0x4c57d0, V),
    row(E::StockSell, SoundId::STOCK_SELL, 82, false, 0x4c56e0, V),
    row(E::GameMode, SoundId::GAMES, 85, false, 0x4b0210, V),
    row(E::BellA, SoundId::BELL_A, 70, false, 0x404e30, V),
    row(E::BellB, SoundId::BELL_B, 70, false, 0x404e30, V),
    row(
        E::TrainBreakdown,
        SoundId::BREAKDOWN,
        100,
        false,
        0x4ce4c0,
        V,
    ),
    row(E::TrainCrash, SoundId::CRASH, 100, false, 0x4ce730, V),
    row(E::TrainRobbery, SoundId::ROBBERY, 100, false, 0x4ceaa0, V),
    row(
        E::TrainBrokenSteam,
        SoundId::BROKEN_STEAM,
        100,
        true,
        0x4d0870,
        V,
    ),
    row(
        E::TrainBrokenOther,
        SoundId::BROKEN_OTHER,
        100,
        true,
        0x4d0870,
        V,
    ),
    row(E::TrainFire, SoundId::FIRE, 100, true, 0x4d0870, V),
    row(
        E::TrainNextStation,
        SoundId::NEXT_STATION,
        100,
        false,
        0x4b0210,
        V,
    ),
    row(E::TrainHorse, SoundId::HORSE, 100, false, 0x4b74f0, V),
    row(
        E::TrainAddStation1,
        SoundId::ADD_STATION_1,
        100,
        false,
        0x4d11f0,
        V,
    ),
    row(
        E::TrainAddStation2,
        SoundId::ADD_STATION_2,
        100,
        false,
        0x4dd6c0,
        I,
    ),
    row(
        E::TrainAddStation3,
        SoundId::ADD_STATION_3,
        100,
        false,
        0x4dd6c0,
        V,
    ),
    row(E::DesignSteam, SoundId::DESIGN_STEAM, 50, true, 0x4de210, V),
    row(
        E::DesignDiesel,
        SoundId::DESIGN_DIESEL,
        50,
        true,
        0x4de210,
        V,
    ),
    row(
        E::DesignElectric,
        SoundId::DESIGN_ELECTRIC,
        50,
        true,
        0x4de210,
        V,
    ),
    row(E::Clock, SoundId::CLOCK, 70, true, 0x40de20, V),
    row(
        E::CombatBackground,
        SoundId::COMBAT_BG,
        75,
        true,
        0x40de20,
        V,
    ),
    row(
        E::CombatBackground2,
        SoundId::COMBAT_BG_2,
        75,
        true,
        0x40de20,
        V,
    ),
    row(E::AmbientHawk, SoundId::HAWK, 100, false, 0x406470, V),
    row(E::AmbientGeese, SoundId::GEESE, 100, false, 0x406470, V),
    row(E::AmbientFlyby, SoundId::FLYBY, 100, false, 0x406470, V),
    row(E::AmbientRoost, SoundId::ROOST, 100, false, 0x406470, V),
    row(E::AmbientAfrica, SoundId::AFRICA, 100, false, 0x406470, V),
    row(E::AmbientPigeons, SoundId::PIGEONS, 100, false, 0x406470, V),
    row(E::AmbientTrainIn, SoundId::TRAIN_IN, 90, false, 0x406470, V),
    row(E::Announce, SoundId::ANNOUNCE, 100, false, 0x4eeeb0, I),
    row(E::Clock, SoundId::CLOCK, 50, true, 0x40de20, V),
];

impl Event {
    /// First row for this event (the default sound and volume).
    pub fn row(self) -> &'static EventRow {
        EVENTS
            .iter()
            .find(|r| r.event == self)
            .expect("every event has a row")
    }

    pub fn sound(self) -> SoundId {
        self.row().sound
    }
}

/// Terrain ambience (`0x49bf70`, `0x49b700`): `birds, waves, swamp, river, blizzard, jungle` are
/// consecutive ids chosen by an ambience index 0..=5, always looping.
pub fn terrain_ambience(index: u8) -> Option<SoundId> {
    (index < 6).then(|| SoundId(SoundId::AMBIENT_BIRDS.0 + index))
}

/// Engine record fields `+0x28` (kind), `+0x29` (running loop) and `+0x2a` (departure whistle) for
/// each of the 66 engines of `engine_table` (`0x53cec0`, `trains.md` 2.2): `(kind, chug, whistle)`.
/// Kind 0 steam, 1 diesel, 2 electric. Chug ids 19 and 20 alternate by train id parity (`0x4d0870`).
pub static ENGINE_SOUNDS: [(u8, u8, u8); 66] = [
    (0, 19, 26),
    (0, 20, 30),
    (0, 19, 31),
    (2, 23, 29),
    (0, 19, 30),
    (0, 20, 31),
    (0, 19, 26),
    (0, 20, 30),
    (0, 19, 31),
    (0, 20, 26),
    (0, 19, 30),
    (0, 20, 31),
    (0, 19, 26),
    (0, 20, 30),
    (0, 19, 31),
    (0, 20, 26),
    (0, 19, 30),
    (0, 20, 31),
    (0, 19, 26),
    (0, 20, 30),
    (0, 19, 31),
    (0, 20, 26),
    (0, 19, 30),
    (0, 20, 31),
    (0, 19, 26),
    (0, 20, 30),
    (0, 19, 31),
    (0, 20, 26),
    (0, 19, 30),
    (0, 20, 31),
    (1, 21, 27),
    (1, 21, 28),
    (1, 21, 32),
    (1, 21, 33),
    (1, 21, 27),
    (1, 21, 28),
    (1, 21, 32),
    (1, 21, 33),
    (1, 21, 27),
    (1, 21, 28),
    (1, 21, 32),
    (1, 21, 33),
    (2, 22, 25),
    (2, 23, 29),
    (2, 22, 25),
    (2, 23, 29),
    (2, 22, 25),
    (2, 23, 29),
    (2, 22, 25),
    (2, 23, 29),
    (2, 22, 25),
    (2, 23, 29),
    (2, 22, 25),
    (2, 23, 29),
    (2, 24, 25),
    (2, 24, 29),
    (0, 20, 30),
    (0, 20, 30),
    (2, 23, 29),
    (1, 21, 32),
    (2, 23, 29),
    (0, 20, 31),
    (2, 24, 29),
    (0, 20, 31),
    (2, 23, 29),
    (2, 23, 29),
];

/// Running loop for a train (`0x4d0870`): chug ids 19/20 and 22/23 alternate with the parity of
/// the train id; a breakdown replaces it with the broken loop and an accident with the fire loop.
pub fn train_loop(engine: usize, train_id: u32, broken: bool, accident: bool) -> Option<SoundId> {
    let (kind, chug, _) = *ENGINE_SOUNDS.get(engine)?;
    if accident {
        return Some(SoundId::FIRE);
    }
    if broken {
        return Some(if kind == 0 {
            SoundId::BROKEN_STEAM
        } else {
            SoundId::BROKEN_OTHER
        });
    }
    let chug = match chug {
        19 | 20 => 19 + (train_id & 1) as u8,
        22 => 22 + (train_id & 1) as u8,
        c => c,
    };
    Some(SoundId(chug))
}

/// Whistle played when a train leaves a station (`0x4d0400`), only if the listener volume is above 5.
pub fn departure_whistle(engine: usize) -> Option<SoundId> {
    ENGINE_SOUNDS.get(engine).map(|e| SoundId(e.2))
}

/// Brake squeal on arrival (`0x4d29e0`): `brake`, `brak2`, `brak3` or `brak4`, picked with
/// `rand_range(0, 3)` (here from the audio RNG).
pub fn arrival_brake(variant: u32) -> SoundId {
    SoundId(SoundId::BRAKE_FIRST.0 + (variant & 3) as u8)
}

/// Station chime (`0x47d100`): `chin1..3`, `rand_range(0, 2)`; the game plays one per car, 150 ms apart.
pub fn chime(variant: u32) -> SoundId {
    SoundId(SoundId::CHIME_FIRST.0 + (variant % 3) as u8)
}

/// Ambient loop of a building type (`building_type_table` `0x535c38`, field `+0`; `0` = none),
/// used by `0x40b8b0` and `0x49b700`. 53 building types.
pub static BUILDING_LOOPS: [u8; 53] = [
    60, 65, 51, 64, 57, 52, 48, 50, 68, 46, 67, 67, 48, 70, 56, 71, 53, 45, 0, 57, 59, 49, 43, 58,
    54, 63, 68, 44, 69, 67, 67, 61, 47, 67, 55, 62, 62, 66, 120, 0, 0, 0, 119, 118, 0, 121, 122,
    123, 0, 0, 0, 0, 0,
];

pub fn building_loop(building_type: usize) -> Option<SoundId> {
    BUILDING_LOOPS
        .get(building_type)
        .copied()
        .filter(|&s| s != 0)
        .map(SoundId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_points_at_a_real_sound() {
        for r in &EVENTS {
            assert!(r.sound.entry().is_some(), "{:?}", r.event);
            assert!(r.volume <= 100);
        }
        for (_, chug, whistle) in ENGINE_SOUNDS {
            assert!(SoundId(chug).entry().is_some_and(|e| e.variable_pitch));
            assert!(SoundId(whistle).name().starts_with("whis"));
        }
        for b in 0..BUILDING_LOOPS.len() {
            if let Some(id) = building_loop(b) {
                assert!(id.entry().is_some());
            }
        }
    }

    #[test]
    fn families() {
        assert_eq!(terrain_ambience(0), Some(SoundId::AMBIENT_BIRDS));
        assert_eq!(terrain_ambience(5), Some(SoundId::AMBIENT_JUNGLE));
        assert_eq!(terrain_ambience(6), None);
        assert_eq!(arrival_brake(3).name(), "brak461m.raw");
        assert_eq!(chime(2).name(), "chin361m.raw");
        assert_eq!(train_loop(0, 4, false, false), Some(SoundId(19)));
        assert_eq!(train_loop(1, 4, false, false), Some(SoundId(19)));
        assert_eq!(train_loop(1, 5, false, false), Some(SoundId(20)));
        assert_eq!(train_loop(0, 5, false, false), Some(SoundId(20)));
        assert_eq!(train_loop(44, 3, false, false), Some(SoundId(23)));
        assert_eq!(train_loop(3, 3, true, false), Some(SoundId::BROKEN_OTHER));
        assert_eq!(train_loop(0, 3, true, false), Some(SoundId::BROKEN_STEAM));
        assert_eq!(train_loop(0, 3, true, true), Some(SoundId::FIRE));
        assert_eq!(departure_whistle(0).unwrap().name(), "whis261m.raw");
        assert_eq!(building_loop(0), Some(SoundId(60)));
        assert_eq!(building_loop(18), None);
        assert_eq!(Event::ButtonUp.sound(), SoundId::BUTTON_UP);
    }
}
