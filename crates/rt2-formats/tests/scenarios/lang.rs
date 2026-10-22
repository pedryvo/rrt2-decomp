//! Scenario condition language of `RT2_PLAT.EXE` (docs/spec/scenarios.md): the 118-entry token
//! table at `0x53b330`, the word splitter (`0x453e70`), the compiler (`0x454060`) and the
//! precedence-pass reducer (`0x454270`) with the class parameter and the per-entity evaluation of
//! `0x454ae0`. The values of the language functions come from an [`Env`] supplied by the caller.
//!
//! Test-support module: included with `#[path]` by `tests/scenarios.rs` (rt2-formats) and by the
//! oracle test, so both sides check the same code. Nothing here reads game files.
#![allow(dead_code)]

/// One entry of the token table (17 bytes in the EXE: name pointer, kind u32, precedence u8, then
/// four flag bytes and the alias pointer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: &'static str,
    /// Second accepted spelling (old names), empty when none.
    pub alias: &'static str,
    /// 0 function or constant, 1 comparator, 2 `(`, 3 `)`, 4 binary operator, 5 class parameter.
    pub kind: u32,
    /// Pass in which the reducer handles the token: 0 class, 1 function, 2 parentheses,
    /// 3 constant, 4 `* /`, 5 `+ -`, 6 comparison, 7 `AND OR`.
    pub prec: u8,
    /// Takes `(n)` with a numeric id.
    pub id_param: bool,
    /// Flags that make the rule loop over companies / players / territories (`0x457a30`).
    pub company: bool,
    pub player: bool,
    pub territory: bool,
}

pub const N_ENTRIES: usize = 118;
/// Token id of a numeric literal (its value is in the value array).
pub const CONSTANT: i32 = 0x5a;
pub const LPAREN: i32 = 0x52;
pub const RPAREN: i32 = 0x53;
pub const HIGHEST: i32 = 0x4a;
pub const LOWEST: i32 = 0x4b;
/// Sentinel the original uses for "no value yet" in the class aggregation.
const UNSET: i32 = 0x3b8d_6c1f;

pub static TABLE: [Entry; N_ENTRIES] = [
    Entry {
        name: "Random1To10000",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 0
    Entry {
        name: "True",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 1
    Entry {
        name: "False",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 2
    Entry {
        name: "GameNumPlayers",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 3
    Entry {
        name: "GameNumCompanies",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 4
    Entry {
        name: "GameExistsCompanyId",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: false,
        player: false,
        territory: false,
    }, // 5
    Entry {
        name: "GameExistsPlayerId",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: false,
        player: false,
        territory: false,
    }, // 6
    Entry {
        name: "GameYearsElapsedSinceStart",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 7
    Entry {
        name: "GameStartYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 8
    Entry {
        name: "GameYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 9
    Entry {
        name: "GameMonth",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 10
    Entry {
        name: "GameYearMonth",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 11
    Entry {
        name: "GameEconomicState",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 12
    Entry {
        name: "GameLoadsHauledThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 13
    Entry {
        name: "GameLoadsHauledLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 14
    Entry {
        name: "GameTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 15
    Entry {
        name: "GameSingleTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 16
    Entry {
        name: "GameDoubleTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 17
    Entry {
        name: "GameElectricTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 18
    Entry {
        name: "GameVariable1",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 19
    Entry {
        name: "GameVariable2",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 20
    Entry {
        name: "GameVariable3",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 21
    Entry {
        name: "GameVariable4",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 22
    Entry {
        name: "CompanyCash",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 23
    Entry {
        name: "CompanyDebt",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 24
    Entry {
        name: "CompanyTotalAssets",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 25
    Entry {
        name: "CompanyBookValue",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 26
    Entry {
        name: "CompanyRevenueThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 27
    Entry {
        name: "CompanyRevenueLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 28
    Entry {
        name: "CompanyNetIncomeThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 29
    Entry {
        name: "CompanyNetIncomeLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 30
    Entry {
        name: "CompanyLoadsHauledThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 31
    Entry {
        name: "CompanyLoadsHauledLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 32
    Entry {
        name: "CompanyTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 33
    Entry {
        name: "CompanySingleTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 34
    Entry {
        name: "CompanyDoubleTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 35
    Entry {
        name: "CompanyElectricTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 36
    Entry {
        name: "CompanyGoodwill",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 37
    Entry {
        name: "CompanyId",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 38
    Entry {
        name: "CompanyVariable1",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 39
    Entry {
        name: "CompanyVariable2",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 40
    Entry {
        name: "CompanyVariable3",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 41
    Entry {
        name: "CompanyVariable4",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 42
    Entry {
        name: "CompanyTerritoryConnectedToTerritory",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 43
    Entry {
        name: "CompanyTerritoryConnectedToCity",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 44
    Entry {
        name: "CompanyTerritoryGoodwill",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 45
    Entry {
        name: "CompanyTerritoryHasTrainAccess",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 46
    Entry {
        name: "CompanyTerritoryHasTrackAccess",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 47
    Entry {
        name: "CompanyTerritoryHasStationAccess",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 48
    Entry {
        name: "CompanyTerritoryHasBuildingBuyAccess",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 49
    Entry {
        name: "CompanyTerritoryHasAnyAccess",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 50
    Entry {
        name: "CompanyTerritoryTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 51
    Entry {
        name: "CompanyTerritoryLoadsHauledThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 52
    Entry {
        name: "CompanyTerritoryLoadsHauledLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: true,
    }, // 53
    Entry {
        name: "PlayerCash",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 54
    Entry {
        name: "PlayerStockValue",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 55
    Entry {
        name: "PlayerNetWorth",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 56
    Entry {
        name: "PlayerControlsAnyCompany",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 57
    Entry {
        name: "PlayerControlledCompanyId",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 58
    Entry {
        name: "PlayerId",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 59
    Entry {
        name: "PlayerVariable1",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 60
    Entry {
        name: "PlayerVariable2",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 61
    Entry {
        name: "PlayerVariable3",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 62
    Entry {
        name: "PlayerVariable4",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: true,
        territory: false,
    }, // 63
    Entry {
        name: "TerritoryTrackCells",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 64
    Entry {
        name: "TerritoryLoadsHauledToThisYear",
        alias: "TerritoryLoadsHauledThisYear",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 65
    Entry {
        name: "TerritoryLoadsHauledToLifetime",
        alias: "TerritoryLoadsHauledLifetime",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 66
    Entry {
        name: "TerritoryConnectedToTerritory",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: false,
        player: false,
        territory: true,
    }, // 67
    Entry {
        name: "TerritoryConnectedToCity",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: false,
        player: false,
        territory: true,
    }, // 68
    Entry {
        name: "TerritoryId",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 69
    Entry {
        name: "TerritoryVariable1",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 70
    Entry {
        name: "TerritoryVariable2",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 71
    Entry {
        name: "TerritoryVariable3",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 72
    Entry {
        name: "TerritoryVariable4",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 73
    Entry {
        name: "IsHighestInClass",
        alias: "",
        kind: 5,
        prec: 0,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 74
    Entry {
        name: "IsLowestInClass",
        alias: "",
        kind: 5,
        prec: 0,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 75
    Entry {
        name: ">=",
        alias: "",
        kind: 1,
        prec: 6,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 76
    Entry {
        name: "<=",
        alias: "",
        kind: 1,
        prec: 6,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 77
    Entry {
        name: ">",
        alias: "",
        kind: 1,
        prec: 6,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 78
    Entry {
        name: "<",
        alias: "",
        kind: 1,
        prec: 6,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 79
    Entry {
        name: "=",
        alias: "",
        kind: 1,
        prec: 6,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 80
    Entry {
        name: "!=",
        alias: "",
        kind: 1,
        prec: 6,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 81
    Entry {
        name: "(",
        alias: "",
        kind: 2,
        prec: 2,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 82
    Entry {
        name: ")",
        alias: "",
        kind: 3,
        prec: 2,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 83
    Entry {
        name: "AND",
        alias: "",
        kind: 4,
        prec: 7,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 84
    Entry {
        name: "OR",
        alias: "",
        kind: 4,
        prec: 7,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 85
    Entry {
        name: "+",
        alias: "",
        kind: 4,
        prec: 5,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 86
    Entry {
        name: "-",
        alias: "",
        kind: 4,
        prec: 5,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 87
    Entry {
        name: "*",
        alias: "",
        kind: 4,
        prec: 4,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 88
    Entry {
        name: "/",
        alias: "",
        kind: 4,
        prec: 4,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 89
    Entry {
        name: "constant",
        alias: "",
        kind: 0,
        prec: 3,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 90
    Entry {
        name: "CompanyTerritoryLoadsHauledToLifetimeFromTerritory",
        alias: "CompanyTerritoryLoadsHauledLifetimeFromTerritory",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 91
    Entry {
        name: "CompanyTerritoryLoadsHauledLifetimeBetweenTerritory",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 92
    Entry {
        name: "CampaignChoice1To3",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 93
    Entry {
        name: "CampaignDifficulty1To3",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 94
    Entry {
        name: "StationVisited",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: false,
        player: false,
        territory: false,
    }, // 95
    Entry {
        name: "NumberOfStationsVisited",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 96
    Entry {
        name: "CompanyIndustryProfitsThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 97
    Entry {
        name: "CompanyIndustryInvestments",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 98
    Entry {
        name: "GameIsMultiPlayer",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 99
    Entry {
        name: "GameIsNotMultiPlayer",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 100
    Entry {
        name: "CompanyIndustryProfitsLastYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 101
    Entry {
        name: "CompanyIndustryProfitsLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 102
    Entry {
        name: "CompanyTerritoryLoadsHauledFromThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 103
    Entry {
        name: "CompanyTerritoryLoadsHauledFromLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 104
    Entry {
        name: "CompanyTerritoryLoadsHauledFromLifetimeToTerritory",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 105
    Entry {
        name: "CompanyTerritoryTypeLoadsHauledToThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 106
    Entry {
        name: "CompanyTerritoryTypeLoadsHauledToLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 107
    Entry {
        name: "CompanyTerritoryTypeLoadsHauledFromThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 108
    Entry {
        name: "CompanyTerritoryTypeLoadsHauledFromLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: true,
    }, // 109
    Entry {
        name: "CompanyTypeLoadsHauledThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: false,
    }, // 110
    Entry {
        name: "CompanyTypeLoadsHauledLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: true,
        player: false,
        territory: false,
    }, // 111
    Entry {
        name: "CompanyAverageSpeedThisYear",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 112
    Entry {
        name: "CompanyAverageSpeedLifetime",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 113
    Entry {
        name: "GameTrackCellsVisited",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: false,
    }, // 114
    Entry {
        name: "TerritoryCompletelyFlooded",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: false,
        player: false,
        territory: true,
    }, // 115
    Entry {
        name: "CompanyTrackCellsRemaining",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: false,
        company: true,
        player: false,
        territory: false,
    }, // 116
    Entry {
        name: "CompanyHasAnyAccessTerritoryParameter",
        alias: "",
        kind: 0,
        prec: 1,
        id_param: true,
        company: false,
        player: false,
        territory: false,
    }, // 117
];

/// Index of a word in the table (case-insensitive, name or alias; first match wins).
pub fn lookup(word: &str) -> Option<usize> {
    TABLE.iter().position(|e| {
        e.name.eq_ignore_ascii_case(word)
            || (!e.alias.is_empty() && e.alias.eq_ignore_ascii_case(word))
    })
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_cmp(c: u8) -> bool {
    matches!(c, b'>' | b'<' | b'=')
}

/// `0x453e70`: whitespace-separated words, additionally split before/after `( ) + - * /` and where
/// the character class changes between comparison characters (`> < =`) and anything else.
pub fn split_words(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        while pos < b.len() && is_ws(b[pos]) {
            pos += 1;
        }
        if pos >= b.len() || b[pos] == 0 {
            break;
        }
        let start = pos;
        let mut p = pos;
        let mut first = true;
        while p < b.len() && b[p] != 0 {
            let c = b[p];
            if is_ws(c) {
                break;
            }
            if !first {
                let prev = b[p - 1];
                if matches!(c, b'(' | b')' | b'+' | b'-' | b'*' | b'/')
                    || matches!(prev, b'(' | b')' | b'+' | b'-' | b'*' | b'/')
                    || is_cmp(prev) != is_cmp(c)
                {
                    break;
                }
            }
            p += 1;
            first = false;
        }
        out.push(String::from_utf8_lossy(&b[start..p]).into_owned());
        pos = p;
    }
    out
}

/// `0x4ef3f0`: decimal, or hexadecimal after a `0x` prefix; any `-` negates; wrapping `i32`.
/// `None` for any other character (the original returns 0 and the word is rejected).
pub fn parse_number(s: &str) -> Option<i32> {
    let mut v: i32 = 0;
    let mut neg = false;
    let mut base: i32 = 10;
    for &c in s.as_bytes() {
        if c == b'-' {
            neg = true;
        } else if c.is_ascii_digit() {
            v = v.wrapping_mul(base).wrapping_add(i32::from(c - b'0'));
        } else {
            let up = if c.is_ascii_lowercase() || c > 0xef {
                c.wrapping_sub(0x20)
            } else {
                c
            };
            if up == b'X' && v == 0 && base == 10 {
                base = 16;
            } else if base != 16 || !(b'A'..=b'F').contains(&up) {
                return None;
            } else {
                v = v.wrapping_mul(16).wrapping_add(i32::from(up) - 0x37);
            }
        }
    }
    Some(if neg { v.wrapping_neg() } else { v })
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompileError {
    /// A word that is neither a table entry nor a number (LNG 0x7a for the first word, 0x7b else).
    UnknownWord(String),
}

/// `0x454060`: words to (token ids, values). Numbers become [`CONSTANT`] with their value.
pub fn compile(text: &str) -> Result<(Vec<i32>, Vec<i32>), CompileError> {
    let mut ids = Vec::new();
    let mut vals = Vec::new();
    for w in split_words(text) {
        if let Some(k) = lookup(&w) {
            ids.push(k as i32);
            vals.push(0);
        } else if let Some(n) = parse_number(&w) {
            ids.push(CONSTANT);
            vals.push(n);
        } else {
            return Err(CompileError::UnknownWord(w));
        }
    }
    Ok((ids, vals))
}

/// The three loop flags `0x457a30` derives from the tokens: (companies, players, territories).
pub fn loop_flags(ids: &[i32]) -> (bool, bool, bool) {
    let mut f = (false, false, false);
    for &i in ids {
        let e = &TABLE[i as usize];
        f.0 |= e.company;
        f.1 |= e.player;
        f.2 |= e.territory;
    }
    f
}

/// Source of function values and entity lists (pool order) for the evaluator.
pub trait Env {
    /// Value of function `id` (index in [`TABLE`]) for the given entities (0 = none).
    fn value(
        &mut self,
        id: usize,
        param: i32,
        company: i32,
        person: i32,
        territory: i32,
        crt_rand: bool,
    ) -> i32;
    fn companies(&self) -> Vec<i32>;
    fn persons(&self) -> Vec<i32>;
    fn territories(&self) -> Vec<i32>;
}

/// Evaluation context of one rule evaluation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    pub company: i32,
    pub person: i32,
    pub territory: i32,
    /// Trigger 6/7 or a local-player-only rule: `Random1To10000` uses the CRT `rand`.
    pub crt_rand: bool,
}

/// LNG string ids of the evaluation errors.
pub mod err {
    /// Parameter list malformed: `name ( number )` expected.
    pub const BAD_PARAM: u32 = 0x898;
    /// Parameter given to a function that takes none.
    pub const NO_PARAM: u32 = 0x899;
    /// Tokens left over after all passes.
    pub const LEFTOVER: u32 = 0x89a;
    /// Class parameter not in `name ( class )` shape.
    pub const BAD_CLASS: u32 = 0x7c;
    pub const STRAY_RPAREN: u32 = 0x7d;
    pub const UNBALANCED: u32 = 0x7e;
    /// Binary operator without operands.
    pub const OPERAND: u32 = 0x7f;
    pub const EMPTY_PARENS: u32 = 0x80;
    /// Class parameter on a function with no company/player/territory scope.
    pub const CLASS_NO_SCOPE: u32 = 0x81;
    /// Class parameter without a matching entity in the context.
    pub const CLASS_NO_ENTITY: u32 = 0xb2b;
}

/// `0x454ae0`: value of one function token, optionally with a class parameter.
pub fn function_value(
    env: &mut dyn Env,
    id: usize,
    param: i32,
    ctx: &Ctx,
    class: Option<i32>,
) -> Result<i32, u32> {
    let e = &TABLE[id];
    if id == CONSTANT as usize {
        return Ok(param);
    }
    let Some(class) = class else {
        return Ok(env.value(
            id,
            param,
            ctx.company,
            ctx.person,
            ctx.territory,
            ctx.crt_rand,
        ));
    };
    if class != HIGHEST && class != LOWEST {
        return if e.id_param {
            Ok(env.value(
                id,
                param,
                ctx.company,
                ctx.person,
                ctx.territory,
                ctx.crt_rand,
            ))
        } else {
            Err(err::NO_PARAM)
        };
    }
    if !(e.company || e.player || e.territory) {
        return Err(err::CLASS_NO_SCOPE);
    }
    // Priority company > player > territory, each only if the context has that entity.
    let (dim, own, list) = if ctx.company != 0 && e.company {
        (0, ctx.company, env.companies())
    } else if ctx.person != 0 && e.player {
        (1, ctx.person, env.persons())
    } else if ctx.territory != 0 && e.territory {
        (2, ctx.territory, env.territories())
    } else {
        return Err(err::CLASS_NO_ENTITY);
    };
    let mut acc = UNSET;
    let mut own_v = UNSET;
    for x in list {
        let (c, p, t) = match dim {
            0 => (x, ctx.person, ctx.territory),
            1 => (ctx.company, x, ctx.territory),
            _ => (ctx.company, ctx.person, x),
        };
        let v = env.value(id, param, c, p, t, ctx.crt_rand);
        if x == own {
            own_v = v;
        } else if acc == UNSET || (class == HIGHEST && acc <= v) || (class == LOWEST && v <= acc) {
            acc = v;
        }
    }
    if acc == UNSET {
        return Ok(1);
    }
    Ok(i32::from(if class == HIGHEST {
        acc < own_v
    } else {
        own_v < acc
    }))
}

/// `0x454270`: reduces the token list by precedence passes 0..7 and returns the single value
/// left. Functions are all evaluated (in text order) before any operator, so `AND`/`OR` do not
/// short-circuit and every `Random1To10000` draws.
pub fn reduce(env: &mut dyn Env, ids_in: &[i32], vals_in: &[i32], ctx: &Ctx) -> Result<i32, u32> {
    let mut ids = ids_in.to_vec();
    let mut vals = vals_in.to_vec();
    let mut pass: u8 = 0;
    let mut i: usize = 0;
    loop {
        if i >= ids.len() {
            pass += 1;
            if pass > 7 {
                return if ids.len() == 1 {
                    Ok(vals[0])
                } else {
                    Err(err::LEFTOVER)
                };
            }
            i = 0;
            continue;
        }
        let id = ids[i] as usize;
        let e = &TABLE[id];
        if e.prec != pass {
            i += 1;
            continue;
        }
        let n = ids.len();
        match e.kind {
            0 => {
                let mut param = vals[i];
                if e.id_param {
                    if i + 3 >= n
                        || ids[i + 2] != CONSTANT
                        || ids[i + 1] != LPAREN
                        || ids[i + 3] != RPAREN
                    {
                        return Err(err::BAD_PARAM);
                    }
                    param = vals[i + 2];
                    vals[i] = param;
                    ids.drain(i + 1..i + 4);
                    vals.drain(i + 1..i + 4);
                }
                vals[i] = function_value(env, id, param, ctx, None)?;
                ids[i] = CONSTANT;
            }
            1 | 4 => {
                let prev_const = i > 0 && ids[i - 1] == CONSTANT;
                let next_const = i + 1 < n && ids[i + 1] == CONSTANT;
                if ((!prev_const) && id != 0x56 && id != 0x57) || !next_const {
                    return Err(err::OPERAND);
                }
                let (l, r) = if prev_const {
                    (vals[i - 1], vals[i + 1])
                } else {
                    (0, vals[i + 1])
                };
                let b = |x: bool| i32::from(x);
                let res = match id {
                    0x4c => b(r <= l),
                    0x4d => b(l <= r),
                    0x4e => b(r < l),
                    0x4f => b(l < r),
                    0x50 => b(l == r),
                    0x51 => b(l != r),
                    0x54 => b(l != 0 && r != 0),
                    0x55 => b(l != 0 || r != 0),
                    0x56 | 0x57 => {
                        let s = if id == 0x56 { 1 } else { -1 };
                        if !prev_const {
                            // unary: [op, const] -> const
                            vals[i] = r.wrapping_mul(s);
                            ids[i] = CONSTANT;
                            ids.remove(i + 1);
                            vals.remove(i + 1);
                            if ids.len() == 1 {
                                return Ok(vals[0]);
                            }
                            continue;
                        }
                        r.wrapping_mul(s).wrapping_add(l)
                    }
                    0x58 => l.wrapping_mul(r),
                    0x59 => l.wrapping_div(if r != 0 { r } else { 1 }),
                    _ => 0,
                };
                // binary: [l, op, r] -> res at i-1
                ids[i - 1] = CONSTANT;
                vals[i - 1] = res;
                ids.drain(i..i + 2);
                vals.drain(i..i + 2);
                if ids.len() == 1 {
                    return Ok(vals[0]);
                }
                // the original resumes at i-1 and then increments, i.e. at i
                continue;
            }
            2 => {
                let mut depth = 1;
                let mut j = i + 1;
                loop {
                    if j >= n {
                        return Err(err::UNBALANCED);
                    }
                    match TABLE[ids[j] as usize].kind {
                        2 => depth += 1,
                        3 => depth -= 1,
                        _ => {}
                    }
                    if depth == 0 {
                        break;
                    }
                    j += 1;
                }
                let len = j - i - 1;
                if len == 0 {
                    return Err(err::EMPTY_PARENS);
                }
                let v = reduce(env, &ids[i + 1..j], &vals[i + 1..j], ctx)?;
                ids[i] = CONSTANT;
                vals[i] = v;
                ids.drain(i + 1..j + 1);
                vals.drain(i + 1..j + 1);
                if ids.len() == 1 {
                    return Ok(vals[0]);
                }
            }
            3 => return Err(err::STRAY_RPAREN),
            5 => {
                if i < 2
                    || i + 1 >= n
                    || TABLE[ids[i - 2] as usize].kind != 0
                    || ids[i - 1] != LPAREN
                    || ids[i + 1] != RPAREN
                {
                    return Err(err::BAD_CLASS);
                }
                let f = ids[i - 2] as usize;
                let v = function_value(env, f, vals[i - 2], ctx, Some(id as i32))?;
                vals[i - 2] = v;
                ids[i - 2] = CONSTANT;
                ids.drain(i - 1..i + 2);
                vals.drain(i - 1..i + 2);
                if ids.len() == 1 {
                    return Ok(vals[0]);
                }
                i -= 2;
            }
            _ => {}
        }
        i += 1;
    }
}

/// `0x45c150`: slider byte of the money effects (Company Cash, Player Cash, ...) to dollars.
/// 1..=10 -> 1000 steps, 11..=59 -> 10000 steps from 20 000, 60..=104 -> 100 000 steps from
/// 600 000, above -> 1 000 000 steps from 6 000 000; sign preserved (0 and negatives negative).
pub fn effect_money(v: i32) -> i32 {
    let a = v.wrapping_abs();
    let sign = if v < 1 { -1 } else { 1 };
    if a < 0xb {
        sign * a * 1000
    } else if a < 0x3c {
        (a - 9) * sign * 10_000
    } else if a < 0x69 {
        (a - 0x36) * sign * 100_000
    } else {
        (a - 99) * sign * 1_000_000
    }
}

/// Date at which the undo rule of a temporary effect fires (`0x455f30`, end): `months` after the
/// current (year, month 1..=12), as (year, month).
pub fn undo_date(year: i32, month: i32, months: i32) -> (i32, i32) {
    let t = months + year * 12 - 1 + month;
    (t / 12, t % 12 + 1)
}
