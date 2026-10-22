//! The 118-entry token table of the condition language (`0x53b330`, docs/spec/scenarios.md 3.5).
//! Verified entry by entry against the executable by `crates/rt2-oracle/tests/scenarios.rs`.

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

const fn ent(
    name: &'static str,
    alias: &'static str,
    kind: u32,
    prec: u8,
    id_param: bool,
    scope: [bool; 3],
) -> Entry {
    Entry {
        name,
        alias,
        kind,
        prec,
        id_param,
        company: scope[0],
        player: scope[1],
        territory: scope[2],
    }
}

pub static TABLE: [Entry; N_ENTRIES] = [
    ent("Random1To10000", "", 0, 1, false, [false, false, false]), // 0
    ent("True", "", 0, 1, false, [false, false, false]),           // 1
    ent("False", "", 0, 1, false, [false, false, false]),          // 2
    ent("GameNumPlayers", "", 0, 1, false, [false, false, false]), // 3
    ent("GameNumCompanies", "", 0, 1, false, [false, false, false]), // 4
    ent("GameExistsCompanyId", "", 0, 1, true, [false, false, false]), // 5
    ent("GameExistsPlayerId", "", 0, 1, true, [false, false, false]), // 6
    ent(
        "GameYearsElapsedSinceStart",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 7
    ent("GameStartYear", "", 0, 1, false, [false, false, false]),  // 8
    ent("GameYear", "", 0, 1, false, [false, false, false]),       // 9
    ent("GameMonth", "", 0, 1, false, [false, false, false]),      // 10
    ent("GameYearMonth", "", 0, 1, false, [false, false, false]),  // 11
    ent("GameEconomicState", "", 0, 1, false, [false, false, false]), // 12
    ent(
        "GameLoadsHauledThisYear",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 13
    ent(
        "GameLoadsHauledLifetime",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 14
    ent("GameTrackCells", "", 0, 1, false, [false, false, false]), // 15
    ent(
        "GameSingleTrackCells",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 16
    ent(
        "GameDoubleTrackCells",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 17
    ent(
        "GameElectricTrackCells",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 18
    ent("GameVariable1", "", 0, 1, false, [false, false, false]),  // 19
    ent("GameVariable2", "", 0, 1, false, [false, false, false]),  // 20
    ent("GameVariable3", "", 0, 1, false, [false, false, false]),  // 21
    ent("GameVariable4", "", 0, 1, false, [false, false, false]),  // 22
    ent("CompanyCash", "", 0, 1, false, [true, false, false]),     // 23
    ent("CompanyDebt", "", 0, 1, false, [true, false, false]),     // 24
    ent("CompanyTotalAssets", "", 0, 1, false, [true, false, false]), // 25
    ent("CompanyBookValue", "", 0, 1, false, [true, false, false]), // 26
    ent(
        "CompanyRevenueThisYear",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 27
    ent(
        "CompanyRevenueLifetime",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 28
    ent(
        "CompanyNetIncomeThisYear",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 29
    ent(
        "CompanyNetIncomeLifetime",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 30
    ent(
        "CompanyLoadsHauledThisYear",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 31
    ent(
        "CompanyLoadsHauledLifetime",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 32
    ent("CompanyTrackCells", "", 0, 1, false, [true, false, false]), // 33
    ent(
        "CompanySingleTrackCells",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 34
    ent(
        "CompanyDoubleTrackCells",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 35
    ent(
        "CompanyElectricTrackCells",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 36
    ent("CompanyGoodwill", "", 0, 1, false, [true, false, false]), // 37
    ent("CompanyId", "", 0, 1, false, [true, false, false]),       // 38
    ent("CompanyVariable1", "", 0, 1, false, [true, false, false]), // 39
    ent("CompanyVariable2", "", 0, 1, false, [true, false, false]), // 40
    ent("CompanyVariable3", "", 0, 1, false, [true, false, false]), // 41
    ent("CompanyVariable4", "", 0, 1, false, [true, false, false]), // 42
    ent(
        "CompanyTerritoryConnectedToTerritory",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 43
    ent(
        "CompanyTerritoryConnectedToCity",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 44
    ent(
        "CompanyTerritoryGoodwill",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 45
    ent(
        "CompanyTerritoryHasTrainAccess",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 46
    ent(
        "CompanyTerritoryHasTrackAccess",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 47
    ent(
        "CompanyTerritoryHasStationAccess",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 48
    ent(
        "CompanyTerritoryHasBuildingBuyAccess",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 49
    ent(
        "CompanyTerritoryHasAnyAccess",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 50
    ent(
        "CompanyTerritoryTrackCells",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 51
    ent(
        "CompanyTerritoryLoadsHauledThisYear",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 52
    ent(
        "CompanyTerritoryLoadsHauledLifetime",
        "",
        0,
        1,
        false,
        [true, false, true],
    ), // 53
    ent("PlayerCash", "", 0, 1, false, [false, true, false]),      // 54
    ent("PlayerStockValue", "", 0, 1, false, [false, true, false]), // 55
    ent("PlayerNetWorth", "", 0, 1, false, [false, true, false]),  // 56
    ent(
        "PlayerControlsAnyCompany",
        "",
        0,
        1,
        false,
        [false, true, false],
    ), // 57
    ent(
        "PlayerControlledCompanyId",
        "",
        0,
        1,
        false,
        [false, true, false],
    ), // 58
    ent("PlayerId", "", 0, 1, false, [false, true, false]),        // 59
    ent("PlayerVariable1", "", 0, 1, false, [false, true, false]), // 60
    ent("PlayerVariable2", "", 0, 1, false, [false, true, false]), // 61
    ent("PlayerVariable3", "", 0, 1, false, [false, true, false]), // 62
    ent("PlayerVariable4", "", 0, 1, false, [false, true, false]), // 63
    ent("TerritoryTrackCells", "", 0, 1, false, [false, false, true]), // 64
    ent(
        "TerritoryLoadsHauledToThisYear",
        "TerritoryLoadsHauledThisYear",
        0,
        1,
        false,
        [false, false, true],
    ), // 65
    ent(
        "TerritoryLoadsHauledToLifetime",
        "TerritoryLoadsHauledLifetime",
        0,
        1,
        false,
        [false, false, true],
    ), // 66
    ent(
        "TerritoryConnectedToTerritory",
        "",
        0,
        1,
        true,
        [false, false, true],
    ), // 67
    ent(
        "TerritoryConnectedToCity",
        "",
        0,
        1,
        true,
        [false, false, true],
    ), // 68
    ent("TerritoryId", "", 0, 1, false, [false, false, true]),     // 69
    ent("TerritoryVariable1", "", 0, 1, false, [false, false, true]), // 70
    ent("TerritoryVariable2", "", 0, 1, false, [false, false, true]), // 71
    ent("TerritoryVariable3", "", 0, 1, false, [false, false, true]), // 72
    ent("TerritoryVariable4", "", 0, 1, false, [false, false, true]), // 73
    ent("IsHighestInClass", "", 5, 0, false, [false, false, false]), // 74
    ent("IsLowestInClass", "", 5, 0, false, [false, false, false]), // 75
    ent(">=", "", 1, 6, false, [false, false, false]),             // 76
    ent("<=", "", 1, 6, false, [false, false, false]),             // 77
    ent(">", "", 1, 6, false, [false, false, false]),              // 78
    ent("<", "", 1, 6, false, [false, false, false]),              // 79
    ent("=", "", 1, 6, false, [false, false, false]),              // 80
    ent("!=", "", 1, 6, false, [false, false, false]),             // 81
    ent("(", "", 2, 2, false, [false, false, false]),              // 82
    ent(")", "", 3, 2, false, [false, false, false]),              // 83
    ent("AND", "", 4, 7, false, [false, false, false]),            // 84
    ent("OR", "", 4, 7, false, [false, false, false]),             // 85
    ent("+", "", 4, 5, false, [false, false, false]),              // 86
    ent("-", "", 4, 5, false, [false, false, false]),              // 87
    ent("*", "", 4, 4, false, [false, false, false]),              // 88
    ent("/", "", 4, 4, false, [false, false, false]),              // 89
    ent("constant", "", 0, 3, false, [false, false, false]),       // 90
    ent(
        "CompanyTerritoryLoadsHauledToLifetimeFromTerritory",
        "CompanyTerritoryLoadsHauledLifetimeFromTerritory",
        0,
        1,
        true,
        [true, false, true],
    ), // 91
    ent(
        "CompanyTerritoryLoadsHauledLifetimeBetweenTerritory",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 92
    ent("CampaignChoice1To3", "", 0, 1, false, [false, false, false]), // 93
    ent(
        "CampaignDifficulty1To3",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 94
    ent("StationVisited", "", 0, 1, true, [false, false, false]),  // 95
    ent(
        "NumberOfStationsVisited",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 96
    ent(
        "CompanyIndustryProfitsThisYear",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 97
    ent(
        "CompanyIndustryInvestments",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 98
    ent("GameIsMultiPlayer", "", 0, 1, false, [false, false, false]), // 99
    ent(
        "GameIsNotMultiPlayer",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 100
    ent(
        "CompanyIndustryProfitsLastYear",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 101
    ent(
        "CompanyIndustryProfitsLifetime",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 102
    ent(
        "CompanyTerritoryLoadsHauledFromThisYear",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 103
    ent(
        "CompanyTerritoryLoadsHauledFromLifetime",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 104
    ent(
        "CompanyTerritoryLoadsHauledFromLifetimeToTerritory",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 105
    ent(
        "CompanyTerritoryTypeLoadsHauledToThisYear",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 106
    ent(
        "CompanyTerritoryTypeLoadsHauledToLifetime",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 107
    ent(
        "CompanyTerritoryTypeLoadsHauledFromThisYear",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 108
    ent(
        "CompanyTerritoryTypeLoadsHauledFromLifetime",
        "",
        0,
        1,
        true,
        [true, false, true],
    ), // 109
    ent(
        "CompanyTypeLoadsHauledThisYear",
        "",
        0,
        1,
        true,
        [true, false, false],
    ), // 110
    ent(
        "CompanyTypeLoadsHauledLifetime",
        "",
        0,
        1,
        true,
        [true, false, false],
    ), // 111
    ent(
        "CompanyAverageSpeedThisYear",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 112
    ent(
        "CompanyAverageSpeedLifetime",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 113
    ent(
        "GameTrackCellsVisited",
        "",
        0,
        1,
        false,
        [false, false, false],
    ), // 114
    ent(
        "TerritoryCompletelyFlooded",
        "",
        0,
        1,
        false,
        [false, false, true],
    ), // 115
    ent(
        "CompanyTrackCellsRemaining",
        "",
        0,
        1,
        false,
        [true, false, false],
    ), // 116
    ent(
        "CompanyHasAnyAccessTerritoryParameter",
        "",
        0,
        1,
        true,
        [false, false, false],
    ), // 117
];
