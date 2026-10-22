//! Sound ids: the index into the EXE's descriptor table at `0x55e7b0` (`docs/formats/sound.md`).

use rt2_formats::sound::{self, SoundEntry};

/// Index of a sound effect in the original table (`rt2_formats::sound::SOUNDS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundId(pub u8);

macro_rules! ids {
    ($($name:ident = $n:expr),* $(,)?) => {
        impl SoundId { $(pub const $name: SoundId = SoundId($n);)* }
    };
}

ids! {
    BUTTON_UP = 1, BUTTON_DOWN = 2, LIST_SCROLL = 3, LIST_SCROLL_SOFT = 4, CLOCK = 5,
    AMBIENT_BIRDS = 6, AMBIENT_WAVES = 7, AMBIENT_SWAMP = 8, AMBIENT_RIVER = 9,
    AMBIENT_BLIZZARD = 10, AMBIENT_JUNGLE = 11,
    PAGE_TOP = 12, PAGE_TURN = 13, ADD_BUILDING = 14, CHAIN = 15, BULLDOZE = 16, SWITCH = 17,
    TAB = 18, CHUG_FIRST = 19, WHISTLE_FIRST = 25, HORSE = 34, DESIGN_STEAM = 35, CARGO = 36,
    RADIO = 37, NEXT_STATION = 38, BRAKE_FIRST = 39, INDUSTRY_FIRST = 43, CHIME_FIRST = 72,
    LAY_TRACK = 75, TRAIN_IN = 76, LAY_RIVER = 77, ADD_TRACK = 78, RAISE = 79, BUCKET = 80,
    BELL_A = 81, BELL_B = 82, ADD_STATION_1 = 83, ADD_STATION_2 = 84, ADD_STATION_3 = 85,
    HAWK = 87, CARGO_REMOVE = 88, BOOK = 89, BUY = 91, BRUSH = 92, RESOURCE = 93,
    DESIGN_DIESEL = 94, PAPER_FLIP = 95, PAPER_NEW = 96, STOCK = 97, ROOST = 98,
    DESIGN_ELECTRIC = 99, LOCK_CLOSED = 100, LOCK_OPEN = 101, GEESE = 102, AFRICA = 103,
    GAMES = 104, PIGEONS = 105, BREAKDOWN = 106, BROKEN_STEAM = 107, CRASH = 108, FIRE = 109,
    ROBBERY = 110, ANNOUNCE = 111, BROKEN_OTHER = 112, COMBAT_BG = 113, STOCK_SELL = 115,
    CAMPAIGN = 116, COMBAT_BG_2 = 117, FLYBY = 124,
}

impl SoundId {
    /// Table row, or `None` for the two empty slots and out-of-range ids.
    pub fn entry(self) -> Option<&'static SoundEntry> {
        sound::by_id(self.0)
    }

    /// File name in the PK2 (`"buttu61m.raw"`), or `""` for an unused id.
    pub fn name(self) -> &'static str {
        self.entry().map_or("", |e| e.name)
    }

    /// Finds an id by file name, case-insensitive, extension optional.
    pub fn from_name(name: &str) -> Option<SoundId> {
        sound::find_by_name(name).map(|e| SoundId(e.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_table() {
        assert_eq!(SoundId::BUTTON_UP.name(), "buttu61m.raw");
        assert_eq!(SoundId::CHUG_FIRST.name(), "chug161m.raw");
        assert_eq!(SoundId::WHISTLE_FIRST.name(), "whis161m.raw");
        assert_eq!(SoundId::BRAKE_FIRST.name(), "brake61m.raw");
        assert_eq!(SoundId::INDUSTRY_FIRST.name(), "LUMBR61M.RAW");
        assert_eq!(SoundId::CHIME_FIRST.name(), "chin161m.raw");
        assert_eq!(SoundId::BROKEN_OTHER.name(), "ENBR361m.raw");
        assert_eq!(SoundId::FLYBY.name(), "flyby61m.raw");
        assert_eq!(SoundId::from_name("crash"), None);
        assert_eq!(SoundId::from_name("encra61m"), Some(SoundId::CRASH));
    }
}
