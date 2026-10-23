//! The construction commands of `cmd_execute` (`0x4b0210`, docs/spec/commands.md): every
//! player (and network) action of the original is a packet whose first dword is the command id
//! and whose fields start at `+0xc` (company byte). [`BuildCmd`] mirrors the ids and fields of
//! the packets so that a replay or a network peer keeps the original's semantics, and
//! [`BuildCmd::from_packet`] reads an original packet.
//!
//! [`Ai::execute`] runs a command on the same primitives as the AI. What the original only does
//! for the local player's UI (sounds, opening windows) is left out. Validation is the one the
//! original performs inside the command (placement for `0x17`, live train ids for the train
//! commands); the cash checks are made by the original's windows before the command is sent,
//! and are reported here by [`Ai::check`] with the same rule.

use crate::ai::Ai;
use crate::trainstep::P;

use super::route::LayRoute;
use super::station::NewBuilding;

/// A construction command with the fields of its packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildCmd {
    /// `0x0a`: `bulldoze_area(company, x, y, radius, 0, ...)` (`0x428930`).
    Bulldoze {
        company: i32,
        x: i32,
        y: i32,
        radius: i32,
    },
    /// `0x16`: `track_lay_route(company, x1, y1, x2, y2, d1, d2, preview, single, electric,
    /// bridge)`; flags byte `+0x19`: bit 0 preview, 1 single, 2 electric, 3..5 bridge type.
    LayTrack {
        company: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        d1: i32,
        d2: i32,
        single: bool,
        electric: bool,
        bridge: i32,
    },
    /// `0x17`: `building_can_place` then `building_create(x, y, shape, 0, company, 0, hint)`.
    /// A shape of type 0 is a station, any other type a building or industry.
    Build {
        company: i32,
        x: i32,
        y: i32,
        shape: i32,
        hint: i32,
    },
    /// `0x18`: `station_buy_improvements(station, mask)`.
    Improve { station: i32, mask: u32 },
    /// `0x19`: `train_buy(engine, station, company)` then `train_default_stops(train, from)`.
    BuyTrain {
        company: i32,
        station: i32,
        engine: i32,
        copy_from: i32,
    },
    /// `0x1d`: insert a stop at `k` (station or waypoint `(x, y)`).
    InsertStop {
        train: i32,
        k: i32,
        station: i32,
        waypoint: bool,
        x: i32,
        y: i32,
    },
    /// `0x1e`: delete stop `k`.
    DeleteStop { train: i32, k: i32 },
    /// `0x1f`: sell (delete) a train.
    DeleteTrain { train: i32 },
    /// `0x20`: replace the engine (no cash movement besides the accounts, `pay = 0`).
    ReplaceEngine { train: i32, engine: i32 },
    /// `0x1a`: `train_set_stop(train, stop, 1)` (`0x4d1630`): the train heads for `stop`.
    TrainGotoStop { train: i32, stop: i32 },
    /// `0x1b`: `train_set_stop_wait` (`0x4d5830`): `mode` 0 no wait, 1 half load, 2 full load
    /// (stop flags `+3` bits 6..7).
    StopWait { train: i32, stop: i32, mode: i8 },
    /// `0x1c`: cargo per car of a stop (`stop == 0xff`: every stop), `0xff` = empty slot.
    StopCargo {
        train: i32,
        stop: i32,
        cargo: [u8; 6],
    },
    /// `0x21`: `train_set_class` (`0x4d59b0`): 0 Halted, 1 Slow, 2 Normal, 3 Express.
    TrainClass { train: i32, class: i8 },
    /// `0x2c`: `train+0x6d = now`, `train+0x9d = 1`: the train may leave (the purchase window
    /// sends it after the stops are edited; `vehicle_place` needs it to put the train on the
    /// track).
    TrainMark { train: i32 },
    /// `0x2f`: `train_set_stop_sell` (`0x4d58e0`): mode 0..3 (stop flags `+3` bits 4..5).
    StopSell { train: i32, stop: i32, mode: u8 },
    /// `0x2a`: territory rights `mask` (bit 0 trains, 1 track, 2 buildings, 3 stations) for
    /// `price` (computed by the rights window `0x43e760`/`0x440fb0`).
    TerritoryRights {
        company: i32,
        territory: i32,
        mask: u8,
        price: i32,
    },
    /// `0x2d`: buy (`buy = true`) or sell an industry (`company_industry_trade` `0x43eb60`).
    TradeIndustry {
        company: i32,
        building: i32,
        buy: bool,
    },
    /// `0x27`: `cmd_set_company_info` (`0x4bddb0`): the company's logo/colour byte `+0x2b` and
    /// its name (`+4`, copied up to and including the NUL). Sent by the founding window
    /// `0x4bd950` when it edits an existing company.
    SetCompanyInfo {
        company: i32,
        name: [u8; 31],
        logo: u8,
    },
    /// `0x29`: `train_set_throttle(train, (f32) value)` (`0x4d6080`): the throttle `+0xdb` and
    /// the effective class `+0xa8` (the slider of `traindtl.win`).
    TrainThrottle { train: i32, value: u8 },
}

/// Why a command did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmdError {
    /// The packet id is not a construction command (or not modelled).
    Unknown,
    /// No build path between the two tiles (`track_lay_route` returns 0).
    NoPath,
    /// `building_can_place` refused the site.
    InvalidPlacement,
    /// The train, station or building id is not live.
    NoSuchObject,
    /// Fewer than two stations in the world (`train_buy`, LNG 352).
    TooFewStations,
    /// The cash does not cover the price (the original's windows refuse to send the command).
    InsufficientCash,
    /// A non-station shape: building houses and industries (`0x17` with shapes `24..121`) is
    /// only offered by the map editor's tool 7 (docs/spec/commands.md 3.1).
    EditorOnly,
    /// The issuing window refuses: industrial model not Expert (`0x573d78 != 2`), permission
    /// `0x5706b9` off, type not buyable (`0x535c39 + type*0x27`), already owned, or no track
    /// rights in the territory (`+0x479 & 2`, LNG 2288).
    NotBuyable,
    /// A command input the original's windows cannot produce (slider value, colour, person).
    InvalidInput,
    /// The person already chairs a company (`0x4bd950`, LNG 2863).
    AlreadyChairman,
    /// 36 companies exist (`0x4bd950`, LNG 144).
    TooManyCompanies,
    /// Less than 672 days since the person's last founding (`0x4bd950`, LNG 2708).
    FoundingCooldown,
    /// The bulldozer refused (reason of `bulldoze_area`, LNG 1121..1128).
    Bulldoze(super::bulldoze::BulldozeError),
}

fn i16_at(p: &[u8], o: usize) -> i32 {
    i32::from(i16::from_le_bytes([p[o], p[o + 1]]))
}

impl BuildCmd {
    /// The command of an original `cmd_execute` packet.
    pub fn from_packet(raw: &[u8]) -> Option<BuildCmd> {
        // The packet buffer is at least 0x1a bytes; shorter input reads as zero-padded.
        let mut buf = [0u8; 0x40];
        let n = raw.len().min(0x40);
        buf[..n].copy_from_slice(&raw[..n]);
        let p = &buf[..];
        let id = u32::from_le_bytes(p.get(0..4)?.try_into().ok()?);
        let co = i32::from(*p.get(0xc)? as i8);
        Some(match id {
            0x0a => BuildCmd::Bulldoze {
                company: co,
                x: i16_at(p, 0xd),
                y: i16_at(p, 0xf),
                radius: i16_at(p, 0x11),
            },
            0x16 => {
                let f = p[0x19];
                BuildCmd::LayTrack {
                    company: co,
                    x1: i16_at(p, 0xd),
                    y1: i16_at(p, 0xf),
                    x2: i16_at(p, 0x11),
                    y2: i16_at(p, 0x13),
                    d1: i16_at(p, 0x15),
                    d2: i16_at(p, 0x17),
                    single: f & 2 != 0,
                    electric: (f >> 2) & 1 != 0,
                    bridge: i32::from((f >> 3) & 7),
                }
            }
            0x17 => BuildCmd::Build {
                company: co,
                x: i16_at(p, 0xd),
                y: i16_at(p, 0xf),
                shape: i32::from(p[0x11]),
                hint: i32::from(p[0x12] as i8),
            },
            0x18 => BuildCmd::Improve {
                station: i16_at(p, 0xc),
                mask: u32::from_le_bytes(p[0xe..0x12].try_into().ok()?),
            },
            0x19 => BuildCmd::BuyTrain {
                company: co,
                station: i16_at(p, 0xd),
                engine: i32::from(p[0xf]),
                copy_from: i32::from(u16::from_le_bytes([p[0x10], p[0x11]])),
            },
            0x1d => {
                let w = u16::from_le_bytes([p[0x11], p[0x12]]);
                BuildCmd::InsertStop {
                    train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                    k: i32::from(p[0xe]),
                    station: i32::from(u16::from_le_bytes([p[0xf], p[0x10]])),
                    waypoint: w & 1 != 0,
                    x: i32::from(w >> 1),
                    y: i32::from(u16::from_le_bytes([p[0x13], p[0x14]])),
                }
            }
            0x1e => BuildCmd::DeleteStop {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                k: i32::from(p[0xe]),
            },
            0x1f => BuildCmd::DeleteTrain {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
            },
            0x20 => BuildCmd::ReplaceEngine {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                engine: i32::from(p[0xe]),
            },
            0x1a => BuildCmd::TrainGotoStop {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                stop: i32::from(p[0xe]),
            },
            0x1b => BuildCmd::StopWait {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                stop: i32::from(p[0xe]),
                mode: p[0xf] as i8,
            },
            0x1c => BuildCmd::StopCargo {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                stop: i32::from(p[0xe]),
                cargo: p[0xf..0x15].try_into().ok()?,
            },
            0x21 => BuildCmd::TrainClass {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                class: p[0xe] as i8,
            },
            0x2c => BuildCmd::TrainMark {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
            },
            0x2f => BuildCmd::StopSell {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                stop: i32::from(p[0xe]),
                mode: p[0xf],
            },
            0x2a => BuildCmd::TerritoryRights {
                company: co,
                territory: i32::from(p[0xd] as i8),
                mask: p[0xe],
                price: i32::from_le_bytes(p[0xf..0x13].try_into().ok()?),
            },
            0x2d => BuildCmd::TradeIndustry {
                company: co,
                building: i32::from_le_bytes(p[0xd..0x11].try_into().ok()?),
                buy: p[0x11] != 0,
            },
            0x27 => BuildCmd::SetCompanyInfo {
                company: co,
                name: p[0xd..0x2c].try_into().ok()?,
                logo: p[0x2c],
            },
            0x29 => BuildCmd::TrainThrottle {
                train: i32::from(u16::from_le_bytes([p[0xc], p[0xd]])),
                value: p[0xe],
            },
            _ => return None,
        })
    }
}

impl Ai<'_, '_> {
    /// The cash check the original's windows make before sending a command (`0x43a060`;
    /// skipped with the cheat `0x5748fc`, never set in a loaded game): the price of a station
    /// building, an improvement set or a train must not exceed the cash.
    pub fn check(&self, cmd: &BuildCmd) -> Result<(), CmdError> {
        let cash = |c: i32| crate::pool::f64_at(&self.ts.record(P::Company, c), 0x4766);
        match *cmd {
            BuildCmd::Build {
                company,
                shape,
                x,
                y,
                ..
            } => {
                if self.ts.img.u8(0x0054_57f8 + 0x154 * shape as u32) != 0 {
                    return Err(CmdError::EditorOnly);
                }
                let terr = i32::from(self.ts.t8(x.max(0), y.max(0), 9));
                let m = self.ts.effect(8, company, 0, terr);
                let price = self.ts.img.i32(0x0054_57ef + 0x154 * shape as u32);
                if f64::from(m) * f64::from(price) > cash(company) {
                    return Err(CmdError::InsufficientCash);
                }
            }
            BuildCmd::BuyTrain {
                company, engine, ..
            } => {
                let kind = usize::from(self.ts.en8(engine, 0x28)) + 0x1b;
                let m = f64::from(self.ts.effect(kind, company, 0, 1))
                    * f64::from(self.ts.effect(0x1a, company, 0, 1));
                if m * f64::from(self.ts.en32(engine, 0xc)) > cash(company) {
                    return Err(CmdError::InsufficientCash);
                }
            }
            BuildCmd::TradeIndustry {
                company,
                building,
                buy: true,
            } => {
                let kind = u32::from(self.ts.u8(P::Industry, building, 7));
                let terr = usize::from(self.ts.u8(P::Industry, building, 0x9d));
                if self.st_i32(0x0057_3d78) & 0xff != 2
                    || !self.perm(0x0057_06b9)
                    || self.ts.img.u8(0x0053_5c39 + kind * 0x27) != 1
                    || self.ts.i16(P::Industry, building, 0x97) != 0
                    || self.ts.u8(P::Company, company, 0x479 + terr * 0x1ed)
                        & if self
                            .ts
                            .g
                            .rules
                            .has(crate::ruleset::Fix::TerritoryRightsPerKind)
                        {
                            4
                        } else {
                            2
                        }
                        == 0
                {
                    return Err(CmdError::NotBuyable);
                }
                let price = crate::ai::industry::building_price(self.ts, building, company);
                if f64::from(price) > cash(company) {
                    return Err(CmdError::InsufficientCash);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Runs a command like `cmd_execute`. Returns the id of what was created (track: the
    /// cost; building: the building id; train: the train id), 0 otherwise.
    pub fn execute(&mut self, cmd: BuildCmd) -> Result<i32, CmdError> {
        match cmd {
            BuildCmd::Bulldoze {
                company,
                x,
                y,
                radius,
            } => self
                .bulldoze_area(company, x, y, radius, false)
                .map_err(CmdError::Bulldoze),
            BuildCmd::LayTrack {
                company,
                x1,
                y1,
                x2,
                y2,
                d1,
                d2,
                single,
                electric,
                bridge,
            } => {
                self.company = company;
                let r = self.track_lay_route(LayRoute {
                    company,
                    x1,
                    y1,
                    x2,
                    y2,
                    d1,
                    d2,
                    single,
                    electric,
                    bridge,
                });
                self.flush_redraw();
                r.map(|l| l.cost).ok_or(CmdError::NoPath)
            }
            BuildCmd::Build {
                company,
                x,
                y,
                shape,
                hint,
            } => {
                if self
                    .building_can_place(x, y, shape, company, true, 0, true, false)
                    .is_none()
                {
                    return Err(CmdError::InvalidPlacement);
                }
                self.company = company;
                Ok(self.building_create(NewBuilding {
                    x,
                    y,
                    shape,
                    ghost: false,
                    company,
                    place: 0,
                    hint,
                }))
            }
            BuildCmd::Improve { station, mask } => {
                if !self.ts.live(P::Station, station) {
                    return Err(CmdError::NoSuchObject);
                }
                Ok(self.station_buy_improvements(station, mask))
            }
            BuildCmd::BuyTrain {
                company,
                engine,
                copy_from,
                ..
            } => {
                let t = self
                    .train_buy(engine, company)
                    .ok_or(CmdError::TooFewStations)?;
                self.train_default_stops(t, copy_from);
                Ok(t)
            }
            BuildCmd::InsertStop {
                train,
                k,
                station,
                waypoint,
                x,
                y,
            } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                self.stop_insert(train, k, None, station as u16, waypoint, x as u16, y as u16);
                Ok(0)
            }
            BuildCmd::DeleteStop { train, k } => {
                if !self.ts.live(P::Train, train)
                    || k >= i32::from(self.ts.u8(P::Train, train, 0x3b))
                {
                    return Err(CmdError::NoSuchObject);
                }
                self.stop_delete(train, k);
                Ok(0)
            }
            BuildCmd::DeleteTrain { train } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                self.train_delete(train);
                Ok(0)
            }
            BuildCmd::ReplaceEngine { train, engine } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                self.replace_engine(train, engine, false);
                Ok(0)
            }
            BuildCmd::TrainGotoStop { train, stop } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                self.ts.set_stop(train, stop, true);
                Ok(0)
            }
            BuildCmd::StopWait { train, stop, mode } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                if stop < i32::from(self.ts.u8(P::Train, train, 0x3b)) {
                    self.train_set_stop_wait(train, stop, mode);
                }
                Ok(0)
            }
            BuildCmd::StopCargo { train, stop, cargo } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                if stop != 0xff && stop >= i32::from(self.ts.u8(P::Train, train, 0x3b)) {
                    return Ok(0);
                }
                self.train_stop_cargo(train, stop, cargo);
                Ok(0)
            }
            BuildCmd::TrainClass { train, class } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                self.train_set_class(train, class);
                Ok(0)
            }
            BuildCmd::TrainMark { train } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                let now = self.ts.now;
                self.ts.set32(P::Train, train, 0x6d, now);
                self.ts.set8(P::Train, train, 0x9d, 1);
                Ok(0)
            }
            BuildCmd::StopSell { train, stop, mode } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                if stop < i32::from(self.ts.u8(P::Train, train, 0x3b)) {
                    self.train_set_stop_sell(train, stop, mode);
                }
                Ok(0)
            }
            BuildCmd::TerritoryRights {
                company,
                territory,
                mask,
                price,
            } => {
                if !self.ts.live(P::Territory, territory) {
                    return Err(CmdError::NoSuchObject);
                }
                self.buy_territory_rights(company, territory, mask, price);
                Ok(0)
            }
            BuildCmd::TradeIndustry {
                company,
                building,
                buy,
            } => {
                if !self.ts.live(P::Industry, building) {
                    return Err(CmdError::NoSuchObject);
                }
                if buy {
                    crate::ai::industry::buy_industry(self.ts, company, building);
                } else {
                    self.sell_industry(company, building);
                }
                Ok(0)
            }
            BuildCmd::SetCompanyInfo {
                company,
                name,
                logo,
            } => {
                // The executor takes the company record without a liveness test (`0x4668a0`);
                // only the window of a live company sends it. The name is a C string: the window
                // always terminates it inside the 31 bytes (`_strncpy(.., 0x1e)`).
                let len = name
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or(CmdError::InvalidInput)?;
                if !self.ts.live(P::Company, company) {
                    return Err(CmdError::NoSuchObject);
                }
                self.ts.set8(P::Company, company, 0x2b, logo);
                for (k, &b) in name[..=len].iter().enumerate() {
                    self.ts.set8(P::Company, company, 4 + k, b);
                }
                Ok(0)
            }
            BuildCmd::TrainThrottle { train, value } => {
                if !self.ts.live(P::Train, train) {
                    return Err(CmdError::NoSuchObject);
                }
                self.train_set_throttle(train, f32::from(value));
                Ok(0)
            }
        }
    }
}
