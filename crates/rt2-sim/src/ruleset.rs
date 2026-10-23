//! Optional fixes of original bugs (docs/PLAN.md section 10 item 10, docs/spec/original-bugs.md).
//!
//! The default [`Ruleset`] is **faithful**: every documented bug of `RT2_PLAT.EXE` is kept, and the
//! fidelity scoreboard compares the clone with the original in this mode. Each fix is a named
//! flag ([`Fix`]) that changes only the behaviour it names; the code of a fix never runs unless
//! its flag is on.
//!
//! The ruleset travels with the game: [`Command::SetRuleset`](crate::Command::SetRuleset) puts it
//! in a command log, [`ReplayHeader`] stores the starting one, and a clone save carries it in a
//! trailer after the last byte of the `.GAM` ([`append_save_trailer`], [`split_save_trailer`]).
//! The original's loader (`0x477a60`) reads the file sequentially and stops after the last rule,
//! so it ignores the trailer (checked in Wine, docs/spec/original-bugs.md section 3) and plays
//! the save with the original rules; a faithful ruleset writes no trailer, keeping the clone's
//! saves byte-identical to the original's.
//!
//! Bit positions are part of the save and replay format: a fix keeps its bit forever, new fixes
//! take new bits.

use std::fmt;

/// One optional fix. The discriminant is the bit in [`Ruleset::bits`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Fix {
    /// `company_ai_finance` (`0x45fbf0`) multiplies the AI dividend by 0.66 on every monthly
    /// turn under Dividend Payout 1; the fix applies it once, when the December dividend is set.
    AiDividendPolicyOnce = 0,
    /// The merger window (`0x48b170`) reads permission `0x5706b4` ("Retiring a train"); the fix
    /// reads `0x5706bd` ("Merging companies").
    MergerPermission = 1,
    /// The station fee of a delivery (`0x4d2c10`) is credited to the station owner even when the
    /// owner is the train's company (which pays nothing): the fix credits only another company.
    StationFeeOwnStation = 2,
    /// A station without owner still takes the fee from the train (`0x4d2c10`) and nobody
    /// receives it: the fix charges no fee there.
    StationFeeUnowned = 3,
    /// Station revenue `Srev = ftol(effect(0x3c) + B)` (`0x4bf760`, an `fiadd`): the fix
    /// multiplies, `ftol(effect(0x3c) * B)`.
    StationRevenueEffect = 4,
    /// Scenario conditions: `x / 0 = x` (`0x454270` divides by 1); the fix gives 1, the rule of
    /// the scenario editor's documentation (`editor3.txt`).
    ScenarioDivByZero = 5,
    /// Scenario function `PlayerId` (59) returns the company id (`0x454ae0`, branch shared with
    /// `CompanyId`); the fix returns the person id of the context.
    ScenarioPlayerId = 6,
    /// `CompanyIndustryProfitsThisYear` (97) returns last year's value (branch of 101); the fix
    /// reads the current year's category 3.
    ScenarioIndustryProfitsThisYear = 7,
    /// `CompanyDebt` (24) returns the debt as stored (negative); the fix returns it positive.
    ScenarioCompanyDebt = 8,
    /// Goodwill (`0x43eca0`) divides lifetime loads by `max(1, ftol((founded - year) * 1.5))`,
    /// always 1; the fix uses the company's age, `year - founded`.
    GoodwillAge = 9,
    /// Station placement (`0x4087a0`) and industry purchase (`0x43ea30`) check the track right
    /// (bit 1) of the territory; the fix checks the station right (bit 3) and the building right
    /// (bit 2).
    TerritoryRightsPerKind = 10,
    /// The clock minute from 23:59 to 00:00 (`sim_advance` `0x4a5cb0`, day end) is not given to
    /// the trains; the fix adds it to the next train step.
    TrainsMidnightMinute = 11,
    /// A broken-down or crashed train (`0x4d00c0` writes no `restricted`) decelerates with a
    /// stale stack value read by `0x4d0400` (true in practice); the fix uses false.
    BrokenTrainDeceleration = 12,
    /// The train pathfinder (`0x4cf8c0`) ORs uninitialized heap arrays into bits 4-7 and 14-15
    /// of each route step's flags; the fix clears those bits.
    RouteStepFlags = 13,
    /// Yearly overhead (`company_year_end` `0x43d800`) averages `(0 + R_Y + R_Y-1) / 3`, counting
    /// the zeroed new year; the fix averages the last three completed years.
    OverheadAverage = 14,
}

/// What the interface shows for a fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixInfo {
    pub fix: Fix,
    /// Stable identifier (saves, replays, command line, string table keys).
    pub id: &'static str,
    /// Short English name.
    pub name: &'static str,
    /// One-sentence English description of what changes.
    pub description: &'static str,
    /// Address of the original code and the spec section.
    pub reference: &'static str,
    /// The fix changes the rules a player sees (`false`: determinism or bookkeeping only).
    pub gameplay: bool,
}

/// Every fix, in bit order.
pub const FIXES: &[FixInfo] = &[
    FixInfo {
        fix: Fix::AiDividendPolicyOnce,
        id: "ai_dividend_policy_once",
        name: "AI dividend cut once a year",
        description: "Under the 'reduced' dividend policy the AI cuts its dividend by a third once, when it sets the December dividend, instead of every month.",
        reference: "0x45fbf0; ai.md 10 item 7",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::MergerPermission,
        id: "merger_permission",
        name: "Merger permission",
        description: "The merger button follows the scenario's 'Merging companies' permission instead of 'Retiring a train'.",
        reference: "0x48b170; commands.md 4, finance.md 18",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::StationFeeOwnStation,
        id: "station_fee_own_station",
        name: "No station fee from yourself",
        description: "A train delivering at its own company's station no longer credits that company with a station fee nobody paid.",
        reference: "0x4d2c10; cargo.md 6",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::StationFeeUnowned,
        id: "station_fee_unowned",
        name: "No fee at unowned stations",
        description: "Stations without an owner no longer take a station fee from the train.",
        reference: "0x4d2c10; terrain-track.md 9, cargo.md 6",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::StationRevenueEffect,
        id: "station_revenue_effect",
        name: "Station revenue effect",
        description: "The station revenue multiplier of managers and events multiplies the passenger service bonus instead of adding to it.",
        reference: "0x4bf760; cargo.md 4.8",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::ScenarioDivByZero,
        id: "scenario_div_by_zero",
        name: "Scenario division by zero",
        description: "In scenario conditions a division by zero gives 1, as the editor documentation says, instead of the dividend.",
        reference: "0x454270; scenarios.md 3.2",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::ScenarioPlayerId,
        id: "scenario_player_id",
        name: "Scenario PlayerId",
        description: "The scenario function PlayerId returns the player's id instead of the company's.",
        reference: "0x454ae0 (0x454dbc); scenarios.md 3.5",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::ScenarioIndustryProfitsThisYear,
        id: "scenario_industry_profits_this_year",
        name: "Scenario industry profits this year",
        description: "The scenario function CompanyIndustryProfitsThisYear returns this year's industry profits instead of last year's.",
        reference: "0x454ae0; scenarios.md 3.5",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::ScenarioCompanyDebt,
        id: "scenario_company_debt",
        name: "Scenario company debt sign",
        description: "The scenario function CompanyDebt returns the debt as a positive amount.",
        reference: "0x454ae0; scenarios.md 3.5",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::GoodwillAge,
        id: "goodwill_age",
        name: "Goodwill company age",
        description: "Company goodwill divides lifetime loads by the company's age instead of by 1.",
        reference: "0x43eca0; scenarios.md 3.5 (id 37)",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::TerritoryRightsPerKind,
        id: "territory_rights_per_kind",
        name: "Separate territory rights",
        description: "Placing a station needs the station right and buying an industry the building right of the territory, instead of the track right for both.",
        reference: "0x4087a0, 0x43ea30; commands.md 4",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::TrainsMidnightMinute,
        id: "trains_midnight_minute",
        name: "Trains move at midnight",
        description: "Trains also move during the minute from 23:59 to midnight, which the original skips every day.",
        reference: "0x4a5cb0; world-tick.md 2, time.md 4",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::BrokenTrainDeceleration,
        id: "broken_train_deceleration",
        name: "Broken train braking",
        description: "A broken-down train brakes with a defined rule instead of a value left on the stack by another function.",
        reference: "0x4d00c0, 0x4d0400; trains.md 10.4",
        gameplay: true,
    },
    FixInfo {
        fix: Fix::RouteStepFlags,
        id: "route_step_flags",
        name: "Clean route flags",
        description: "The train pathfinder no longer stores uninitialized memory in the route flags (no visible effect).",
        reference: "0x4cf8c0; trains.md 10.7",
        gameplay: false,
    },
    FixInfo {
        fix: Fix::OverheadAverage,
        id: "overhead_average",
        name: "Overhead average",
        description: "Yearly overhead averages the revenue of the last three completed years instead of counting the new, empty year.",
        reference: "0x43d800; finance.md 16.1",
        gameplay: true,
    },
];

/// The set of enabled fixes. `Default` is the faithful ruleset (no fix).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ruleset {
    bits: u64,
}

/// Version of the fix list; bumped when fixes are added (old saves and replays stay valid,
/// their unknown-free bit sets mean the same thing).
pub const RULESET_VERSION: u16 = 1;

/// Why a stored ruleset could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RulesetError {
    #[error("ruleset uses fixes unknown to this version (bits {0:#x})")]
    UnknownFixes(u64),
    #[error("unknown fix id {0:?}")]
    UnknownId(String),
    #[error("malformed ruleset record")]
    Malformed,
}

impl Ruleset {
    /// No fix: the original's behaviour, bugs included.
    pub const FAITHFUL: Ruleset = Ruleset { bits: 0 };

    /// Mask of the bits with a defined fix.
    pub const KNOWN: u64 = (1u64 << FIXES.len()) - 1;

    /// Every fix on.
    pub fn all() -> Ruleset {
        Ruleset { bits: Self::KNOWN }
    }

    pub fn is_faithful(self) -> bool {
        self.bits == 0
    }

    pub fn has(self, f: Fix) -> bool {
        self.bits & (1u64 << f as u8) != 0
    }

    #[must_use]
    pub fn with(self, f: Fix, on: bool) -> Ruleset {
        let m = 1u64 << f as u8;
        Ruleset {
            bits: if on { self.bits | m } else { self.bits & !m },
        }
    }

    pub fn set(&mut self, f: Fix, on: bool) {
        *self = self.with(f, on);
    }

    pub fn bits(self) -> u64 {
        self.bits
    }

    /// The ruleset of stored bits; bits of fixes this version does not know are an error.
    pub fn from_bits(bits: u64) -> Result<Ruleset, RulesetError> {
        if bits & !Self::KNOWN != 0 {
            return Err(RulesetError::UnknownFixes(bits & !Self::KNOWN));
        }
        Ok(Ruleset { bits })
    }

    /// The enabled fixes, in bit order.
    pub fn fixes(self) -> impl Iterator<Item = &'static FixInfo> {
        FIXES.iter().filter(move |i| self.has(i.fix))
    }

    /// Parses a comma-separated list of fix ids (`all` enables every fix, empty = faithful).
    pub fn parse(list: &str) -> Result<Ruleset, RulesetError> {
        let mut r = Ruleset::FAITHFUL;
        for id in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if id == "all" {
                r = Ruleset::all();
                continue;
            }
            let info = FixInfo::by_id(id).ok_or_else(|| RulesetError::UnknownId(id.into()))?;
            r.set(info.fix, true);
        }
        Ok(r)
    }
}

impl fmt::Display for Ruleset {
    /// The comma-separated ids of the enabled fixes (`faithful` when none).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_faithful() {
            return f.write_str("faithful");
        }
        let ids: Vec<&str> = self.fixes().map(|i| i.id).collect();
        f.write_str(&ids.join(","))
    }
}

impl FixInfo {
    pub fn by_id(id: &str) -> Option<&'static FixInfo> {
        FIXES.iter().find(|i| i.id == id)
    }

    pub fn of(f: Fix) -> &'static FixInfo {
        &FIXES[f as usize]
    }
}

// ---- storage ---------------------------------------------------------------------------------

/// Last 8 bytes of a clone save that carries a ruleset.
pub const SAVE_TRAILER_MAGIC: &[u8; 8] = b"RT2CRULE";
/// Trailer: `version u16, reserved u16, bits u64, payload_len u32 (= 12), magic[8]`.
const TRAILER_LEN: usize = 2 + 2 + 8 + 4 + 8;

/// Appends the ruleset trailer to the bytes of a save (nothing for the faithful ruleset).
pub fn append_save_trailer(file: &mut Vec<u8>, rules: Ruleset) {
    if rules.is_faithful() {
        return;
    }
    file.extend_from_slice(&RULESET_VERSION.to_le_bytes());
    file.extend_from_slice(&0u16.to_le_bytes());
    file.extend_from_slice(&rules.bits.to_le_bytes());
    file.extend_from_slice(&12u32.to_le_bytes());
    file.extend_from_slice(SAVE_TRAILER_MAGIC);
}

/// Splits a save file into the original's bytes and the clone's ruleset (faithful when there is
/// no trailer, as in every file written by the original).
pub fn split_save_trailer(file: &[u8]) -> Result<(&[u8], Ruleset), RulesetError> {
    if file.len() < TRAILER_LEN || &file[file.len() - 8..] != SAVE_TRAILER_MAGIC {
        return Ok((file, Ruleset::FAITHFUL));
    }
    let t = &file[file.len() - TRAILER_LEN..];
    let len = u32::from_le_bytes(t[12..16].try_into().expect("4 bytes"));
    if len != 12 {
        return Err(RulesetError::Malformed);
    }
    let bits = u64::from_le_bytes(t[4..12].try_into().expect("8 bytes"));
    Ok((&file[..file.len() - TRAILER_LEN], Ruleset::from_bits(bits)?))
}

/// The start of a replay: what a command log needs besides the save it starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayHeader {
    pub ruleset: Ruleset,
    pub game_seed: u32,
    pub rng_seed: u32,
    pub crt_seed: u32,
}

/// Magic of [`ReplayHeader::to_bytes`].
pub const REPLAY_MAGIC: &[u8; 8] = b"RT2CRPLY";
const REPLAY_FORMAT: u16 = 1;

impl ReplayHeader {
    /// `magic[8], format u16, ruleset version u16, ruleset bits u64, game_seed, rng_seed,
    /// crt_seed (u32)`, little-endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(32);
        v.extend_from_slice(REPLAY_MAGIC);
        v.extend_from_slice(&REPLAY_FORMAT.to_le_bytes());
        v.extend_from_slice(&RULESET_VERSION.to_le_bytes());
        v.extend_from_slice(&self.ruleset.bits.to_le_bytes());
        for x in [self.game_seed, self.rng_seed, self.crt_seed] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v
    }

    pub fn from_bytes(b: &[u8]) -> Result<ReplayHeader, RulesetError> {
        if b.len() < 32 || &b[..8] != REPLAY_MAGIC {
            return Err(RulesetError::Malformed);
        }
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"));
        if u16::from_le_bytes([b[8], b[9]]) != REPLAY_FORMAT {
            return Err(RulesetError::Malformed);
        }
        let bits = u64::from_le_bytes(b[12..20].try_into().expect("8 bytes"));
        Ok(ReplayHeader {
            ruleset: Ruleset::from_bits(bits)?,
            game_seed: u32_at(20),
            rng_seed: u32_at(24),
            crt_seed: u32_at(28),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_in_bit_order_with_unique_ids() {
        for (i, info) in FIXES.iter().enumerate() {
            assert_eq!(info.fix as usize, i, "{}", info.id);
            assert_eq!(FixInfo::by_id(info.id), Some(info));
        }
        assert_eq!(Ruleset::all().fixes().count(), FIXES.len());
    }

    #[test]
    fn default_is_faithful_and_flags_are_independent() {
        assert!(Ruleset::default().is_faithful());
        for info in FIXES {
            let r = Ruleset::FAITHFUL.with(info.fix, true);
            for other in FIXES {
                assert_eq!(r.has(other.fix), other.fix == info.fix);
            }
            assert!(r.with(info.fix, false).is_faithful());
        }
    }

    #[test]
    fn parse_and_display_round_trip() {
        let r = Ruleset::parse("merger_permission, goodwill_age").unwrap();
        assert!(r.has(Fix::MergerPermission) && r.has(Fix::GoodwillAge));
        assert_eq!(Ruleset::parse(&r.to_string()).unwrap(), r);
        assert_eq!(Ruleset::parse("").unwrap(), Ruleset::FAITHFUL);
        assert_eq!(Ruleset::parse("all").unwrap(), Ruleset::all());
        assert!(Ruleset::parse("nope").is_err());
        assert_eq!(Ruleset::FAITHFUL.to_string(), "faithful");
    }

    #[test]
    fn save_trailer_round_trip_and_faithful_writes_nothing() {
        let body = vec![1u8, 2, 3, 4, 5];
        let mut f = body.clone();
        append_save_trailer(&mut f, Ruleset::FAITHFUL);
        assert_eq!(f, body);
        let r = Ruleset::FAITHFUL.with(Fix::StationFeeOwnStation, true);
        append_save_trailer(&mut f, r);
        assert_eq!(f.len(), body.len() + TRAILER_LEN);
        assert_eq!(split_save_trailer(&f).unwrap(), (&body[..], r));
        assert_eq!(
            split_save_trailer(&body).unwrap(),
            (&body[..], Ruleset::FAITHFUL)
        );
        // A newer clone's fix is reported, not dropped silently.
        let n = f.len();
        f[n - TRAILER_LEN + 4 + 7] = 0x80;
        assert!(matches!(
            split_save_trailer(&f),
            Err(RulesetError::UnknownFixes(_))
        ));
    }

    #[test]
    fn replay_header_round_trip() {
        let h = ReplayHeader {
            ruleset: Ruleset::all(),
            game_seed: 7,
            rng_seed: 1234,
            crt_seed: 99,
        };
        assert_eq!(ReplayHeader::from_bytes(&h.to_bytes()).unwrap(), h);
        assert!(ReplayHeader::from_bytes(b"short").is_err());
    }
}
