//! Stable binary encoding: a little-endian writer and reader, and the codec of
//! [`rt2_sim::Command`] (docs/net-protocol.md "Commands").
//!
//! Every enum is a one-byte tag followed by its fields in declaration order. The construction and
//! financial commands use the packet ids of the original as tags (`docs/spec/commands.md`), so a
//! tag never changes meaning; the match over [`Command`] is exhaustive on purpose: a new variant
//! does not compile until it has a tag here.

use rt2_sim::build::cmd::BuildCmd;
use rt2_sim::fincmd::FinCmd;
use rt2_sim::found::FoundCompany;
use rt2_sim::ruleset::Ruleset;
use rt2_sim::{Command, SpeedChange};

use crate::NetError;

/// Appends little-endian values to a buffer.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn new() -> Writer {
        Writer(Vec::new())
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.u8(u8::from(v))
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn raw(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }
    /// A `u32` length and the bytes.
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.u32(b.len() as u32);
        self.raw(b)
    }
    pub fn str(&mut self, s: &str) -> &mut Self {
        self.bytes(s.as_bytes())
    }
}

/// Reads what [`Writer`] wrote; every read is bounds checked.
pub struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], NetError> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&e| e <= self.b.len())
            .ok_or(NetError::Malformed("truncated message"))?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, NetError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, NetError> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16, NetError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, NetError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32, NetError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, NetError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn raw<const N: usize>(&mut self) -> Result<[u8; N], NetError> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    pub fn bytes(&mut self) -> Result<Vec<u8>, NetError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    pub fn str(&mut self) -> Result<String, NetError> {
        String::from_utf8(self.bytes()?).map_err(|_| NetError::Malformed("text is not UTF-8"))
    }
    /// Fails when bytes are left over (a message must be consumed whole).
    pub fn finish(self) -> Result<(), NetError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(NetError::Malformed("trailing bytes"))
        }
    }
}

// Tags of `Command`.
const C_SPEED_ABS: u8 = 1;
const C_SPEED_DELTA: u8 = 2;
const C_TOGGLE_PAUSE: u8 = 3;
const C_SEED_RNG: u8 = 4;
const C_SEED_CRT: u8 = 5;
const C_BUILD: u8 = 6;
const C_START_COMPANY: u8 = 7;
const C_FINANCE: u8 = 8;
const C_ANSWER_CHOICE: u8 = 9;
const C_SET_RULESET: u8 = 10;
const C_PLAYER_LEFT: u8 = 11;
const C_PLAYER_JOINED: u8 = 12;

/// Encodes a command.
pub fn put_command(w: &mut Writer, c: &Command) {
    match *c {
        Command::SetSpeed(SpeedChange::Absolute(v)) => {
            w.u8(C_SPEED_ABS).u8(v);
        }
        Command::SetSpeed(SpeedChange::Delta(d)) => {
            w.u8(C_SPEED_DELTA).u8(d as u8);
        }
        Command::TogglePause => {
            w.u8(C_TOGGLE_PAUSE);
        }
        Command::SeedRng(s) => {
            w.u8(C_SEED_RNG).u32(s);
        }
        Command::SeedCrtRng(s) => {
            w.u8(C_SEED_CRT).u32(s);
        }
        Command::Build(b) => {
            w.u8(C_BUILD);
            put_build(w, &b);
        }
        Command::StartCompany(f) => {
            w.u8(C_START_COMPANY)
                .i32(f.person)
                .raw(&f.name)
                .i32(f.color)
                .i32(f.capital)
                .i32(f.player_investment);
        }
        Command::Finance(f) => {
            w.u8(C_FINANCE);
            put_fin(w, &f);
        }
        Command::AnswerChoice { message_id, button } => {
            w.u8(C_ANSWER_CHOICE).u32(message_id).u8(button);
        }
        Command::SetRuleset(r) => {
            w.u8(C_SET_RULESET).u64(r.bits());
        }
        Command::NetPlayerLeft { net_id } => {
            w.u8(C_PLAYER_LEFT).u32(net_id);
        }
        Command::NetPlayerJoined {
            net_id,
            identity,
            person,
            name,
        } => {
            w.u8(C_PLAYER_JOINED)
                .u32(net_id)
                .u32(identity)
                .i32(person)
                .raw(&name);
        }
    }
}

/// Decodes a command.
pub fn get_command(r: &mut Reader) -> Result<Command, NetError> {
    Ok(match r.u8()? {
        C_SPEED_ABS => Command::SetSpeed(SpeedChange::Absolute(r.u8()?)),
        C_SPEED_DELTA => Command::SetSpeed(SpeedChange::Delta(r.u8()? as i8)),
        C_TOGGLE_PAUSE => Command::TogglePause,
        C_SEED_RNG => Command::SeedRng(r.u32()?),
        C_SEED_CRT => Command::SeedCrtRng(r.u32()?),
        C_BUILD => Command::Build(get_build(r)?),
        C_START_COMPANY => Command::StartCompany(FoundCompany {
            person: r.i32()?,
            name: r.raw::<31>()?,
            color: r.i32()?,
            capital: r.i32()?,
            player_investment: r.i32()?,
        }),
        C_FINANCE => Command::Finance(get_fin(r)?),
        C_ANSWER_CHOICE => Command::AnswerChoice {
            message_id: r.u32()?,
            button: r.u8()?,
        },
        C_SET_RULESET => Command::SetRuleset(
            Ruleset::from_bits(r.u64()?)
                .map_err(|_| NetError::Malformed("unknown ruleset bits"))?,
        ),
        C_PLAYER_LEFT => Command::NetPlayerLeft { net_id: r.u32()? },
        C_PLAYER_JOINED => Command::NetPlayerJoined {
            net_id: r.u32()?,
            identity: r.u32()?,
            person: r.i32()?,
            name: r.raw::<31>()?,
        },
        _ => return Err(NetError::Malformed("unknown command tag")),
    })
}

fn put_build(w: &mut Writer, b: &BuildCmd) {
    match *b {
        BuildCmd::Bulldoze {
            company,
            x,
            y,
            radius,
        } => {
            w.u8(0x0a).i32(company).i32(x).i32(y).i32(radius);
        }
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
            w.u8(0x16)
                .i32(company)
                .i32(x1)
                .i32(y1)
                .i32(x2)
                .i32(y2)
                .i32(d1)
                .i32(d2)
                .bool(single)
                .bool(electric)
                .i32(bridge);
        }
        BuildCmd::Build {
            company,
            x,
            y,
            shape,
            hint,
        } => {
            w.u8(0x17).i32(company).i32(x).i32(y).i32(shape).i32(hint);
        }
        BuildCmd::Improve { station, mask } => {
            w.u8(0x18).i32(station).u32(mask);
        }
        BuildCmd::BuyTrain {
            company,
            station,
            engine,
            copy_from,
        } => {
            w.u8(0x19)
                .i32(company)
                .i32(station)
                .i32(engine)
                .i32(copy_from);
        }
        BuildCmd::TrainGotoStop { train, stop } => {
            w.u8(0x1a).i32(train).i32(stop);
        }
        BuildCmd::StopWait { train, stop, mode } => {
            w.u8(0x1b).i32(train).i32(stop).u8(mode as u8);
        }
        BuildCmd::StopCargo { train, stop, cargo } => {
            w.u8(0x1c).i32(train).i32(stop).raw(&cargo);
        }
        BuildCmd::InsertStop {
            train,
            k,
            station,
            waypoint,
            x,
            y,
        } => {
            w.u8(0x1d)
                .i32(train)
                .i32(k)
                .i32(station)
                .bool(waypoint)
                .i32(x)
                .i32(y);
        }
        BuildCmd::DeleteStop { train, k } => {
            w.u8(0x1e).i32(train).i32(k);
        }
        BuildCmd::DeleteTrain { train } => {
            w.u8(0x1f).i32(train);
        }
        BuildCmd::ReplaceEngine { train, engine } => {
            w.u8(0x20).i32(train).i32(engine);
        }
        BuildCmd::TrainClass { train, class } => {
            w.u8(0x21).i32(train).u8(class as u8);
        }
        BuildCmd::TerritoryRights {
            company,
            territory,
            mask,
            price,
        } => {
            w.u8(0x2a).i32(company).i32(territory).u8(mask).i32(price);
        }
        BuildCmd::TrainMark { train } => {
            w.u8(0x2c).i32(train);
        }
        BuildCmd::TradeIndustry {
            company,
            building,
            buy,
        } => {
            w.u8(0x2d).i32(company).i32(building).bool(buy);
        }
        BuildCmd::StopSell { train, stop, mode } => {
            w.u8(0x2f).i32(train).i32(stop).u8(mode);
        }
        BuildCmd::SetCompanyInfo {
            company,
            name,
            logo,
        } => {
            w.u8(0x27).i32(company).raw(&name).u8(logo);
        }
        BuildCmd::TrainThrottle { train, value } => {
            w.u8(0x29).i32(train).u8(value);
        }
    }
}

fn get_build(r: &mut Reader) -> Result<BuildCmd, NetError> {
    Ok(match r.u8()? {
        0x0a => BuildCmd::Bulldoze {
            company: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            radius: r.i32()?,
        },
        0x16 => BuildCmd::LayTrack {
            company: r.i32()?,
            x1: r.i32()?,
            y1: r.i32()?,
            x2: r.i32()?,
            y2: r.i32()?,
            d1: r.i32()?,
            d2: r.i32()?,
            single: r.bool()?,
            electric: r.bool()?,
            bridge: r.i32()?,
        },
        0x17 => BuildCmd::Build {
            company: r.i32()?,
            x: r.i32()?,
            y: r.i32()?,
            shape: r.i32()?,
            hint: r.i32()?,
        },
        0x18 => BuildCmd::Improve {
            station: r.i32()?,
            mask: r.u32()?,
        },
        0x19 => BuildCmd::BuyTrain {
            company: r.i32()?,
            station: r.i32()?,
            engine: r.i32()?,
            copy_from: r.i32()?,
        },
        0x1a => BuildCmd::TrainGotoStop {
            train: r.i32()?,
            stop: r.i32()?,
        },
        0x1b => BuildCmd::StopWait {
            train: r.i32()?,
            stop: r.i32()?,
            mode: r.u8()? as i8,
        },
        0x1c => BuildCmd::StopCargo {
            train: r.i32()?,
            stop: r.i32()?,
            cargo: r.raw::<6>()?,
        },
        0x1d => BuildCmd::InsertStop {
            train: r.i32()?,
            k: r.i32()?,
            station: r.i32()?,
            waypoint: r.bool()?,
            x: r.i32()?,
            y: r.i32()?,
        },
        0x1e => BuildCmd::DeleteStop {
            train: r.i32()?,
            k: r.i32()?,
        },
        0x1f => BuildCmd::DeleteTrain { train: r.i32()? },
        0x20 => BuildCmd::ReplaceEngine {
            train: r.i32()?,
            engine: r.i32()?,
        },
        0x21 => BuildCmd::TrainClass {
            train: r.i32()?,
            class: r.u8()? as i8,
        },
        0x2a => BuildCmd::TerritoryRights {
            company: r.i32()?,
            territory: r.i32()?,
            mask: r.u8()?,
            price: r.i32()?,
        },
        0x2c => BuildCmd::TrainMark { train: r.i32()? },
        0x2d => BuildCmd::TradeIndustry {
            company: r.i32()?,
            building: r.i32()?,
            buy: r.bool()?,
        },
        0x2f => BuildCmd::StopSell {
            train: r.i32()?,
            stop: r.i32()?,
            mode: r.u8()?,
        },
        0x27 => BuildCmd::SetCompanyInfo {
            company: r.i32()?,
            name: r.raw::<31>()?,
            logo: r.u8()?,
        },
        0x29 => BuildCmd::TrainThrottle {
            train: r.i32()?,
            value: r.u8()?,
        },
        _ => return Err(NetError::Malformed("unknown construction command")),
    })
}

fn put_fin(w: &mut Writer, f: &FinCmd) {
    match *f {
        FinCmd::StockTrade {
            person,
            company,
            buy,
            flag,
        } => {
            w.u8(0x0e).i32(person).i32(company).bool(buy).bool(flag);
        }
        FinCmd::ResignChairman { person, company } => {
            w.u8(0x0f).i32(person).i32(company);
        }
        FinCmd::SetManager { company, manager } => {
            w.u8(0x10).i32(company).i32(manager);
        }
        FinCmd::StockIssue { company, n } => {
            w.u8(0x11).i32(company).i32(n);
        }
        FinCmd::SetDividend {
            company,
            dividend_bits,
        } => {
            w.u8(0x12).i32(company).u32(dividend_bits);
        }
        FinCmd::Bankruptcy { company } => {
            w.u8(0x13).i32(company);
        }
        FinCmd::BondIssue {
            company,
            principal,
            term,
        } => {
            w.u8(0x14).i32(company).i32(principal).i32(term);
        }
        FinCmd::BondRepay { company, bond } => {
            w.u8(0x15).i32(company).i32(bond);
        }
        FinCmd::MergerPropose {
            buyer,
            target,
            offer,
        } => {
            w.u8(0x22).i32(buyer).i32(target).i32(offer);
        }
        FinCmd::MergerVote {
            proposal,
            person,
            yes,
        } => {
            w.u8(0x23).u32(proposal).i32(person).bool(yes);
        }
        FinCmd::ElectionStart {
            company,
            challenger,
        } => {
            w.u8(0x24).i32(company).i32(challenger);
        }
        FinCmd::ElectionVote {
            election,
            person,
            yes,
        } => {
            w.u8(0x25).u32(election).i32(person).bool(yes);
        }
        FinCmd::ChairmanTakeover { person, company } => {
            w.u8(0x2e).i32(person).i32(company);
        }
    }
}

fn get_fin(r: &mut Reader) -> Result<FinCmd, NetError> {
    Ok(match r.u8()? {
        0x0e => FinCmd::StockTrade {
            person: r.i32()?,
            company: r.i32()?,
            buy: r.bool()?,
            flag: r.bool()?,
        },
        0x0f => FinCmd::ResignChairman {
            person: r.i32()?,
            company: r.i32()?,
        },
        0x10 => FinCmd::SetManager {
            company: r.i32()?,
            manager: r.i32()?,
        },
        0x11 => FinCmd::StockIssue {
            company: r.i32()?,
            n: r.i32()?,
        },
        0x12 => FinCmd::SetDividend {
            company: r.i32()?,
            dividend_bits: r.u32()?,
        },
        0x13 => FinCmd::Bankruptcy { company: r.i32()? },
        0x14 => FinCmd::BondIssue {
            company: r.i32()?,
            principal: r.i32()?,
            term: r.i32()?,
        },
        0x15 => FinCmd::BondRepay {
            company: r.i32()?,
            bond: r.i32()?,
        },
        0x22 => FinCmd::MergerPropose {
            buyer: r.i32()?,
            target: r.i32()?,
            offer: r.i32()?,
        },
        0x23 => FinCmd::MergerVote {
            proposal: r.u32()?,
            person: r.i32()?,
            yes: r.bool()?,
        },
        0x24 => FinCmd::ElectionStart {
            company: r.i32()?,
            challenger: r.i32()?,
        },
        0x25 => FinCmd::ElectionVote {
            election: r.u32()?,
            person: r.i32()?,
            yes: r.bool()?,
        },
        0x2e => FinCmd::ChairmanTakeover {
            person: r.i32()?,
            company: r.i32()?,
        },
        _ => return Err(NetError::Malformed("unknown financial command")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(c: Command) {
        let mut w = Writer::new();
        put_command(&mut w, &c);
        let mut r = Reader::new(&w.0);
        assert_eq!(get_command(&mut r).unwrap(), c);
        r.finish().unwrap();
    }

    #[test]
    fn every_command_family_round_trips() {
        round(Command::SetSpeed(SpeedChange::Absolute(7)));
        round(Command::SetSpeed(SpeedChange::Delta(-2)));
        round(Command::TogglePause);
        round(Command::SeedRng(0xdead_beef));
        round(Command::SeedCrtRng(5));
        round(Command::AnswerChoice {
            message_id: 9,
            button: 1,
        });
        round(Command::SetRuleset(Ruleset::all()));
        round(Command::NetPlayerLeft { net_id: 77 });
        round(Command::NetPlayerJoined {
            net_id: 3,
            identity: 99,
            person: -1,
            name: [7; 31],
        });
        round(Command::StartCompany(FoundCompany::new(
            2, "Acme", 4, 1000, 500,
        )));
        round(Command::Build(BuildCmd::LayTrack {
            company: 1,
            x1: 2,
            y1: 3,
            x2: -4,
            y2: 5,
            d1: 6,
            d2: 7,
            single: true,
            electric: false,
            bridge: 2,
        }));
        round(Command::Build(BuildCmd::StopCargo {
            train: 1,
            stop: 2,
            cargo: [1, 2, 3, 4, 5, 255],
        }));
        round(Command::Build(BuildCmd::SetCompanyInfo {
            company: 1,
            name: [65; 31],
            logo: 3,
        }));
        round(Command::Finance(FinCmd::set_dividend(1, 0.25)));
        round(Command::Finance(FinCmd::MergerVote {
            proposal: 4,
            person: 2,
            yes: true,
        }));
        round(Command::Finance(FinCmd::ChairmanTakeover {
            person: 1,
            company: 2,
        }));
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        let mut r = Reader::new(&[C_SEED_RNG, 1, 2]);
        assert!(get_command(&mut r).is_err());
        let mut r = Reader::new(&[0xee]);
        assert!(get_command(&mut r).is_err());
        let mut r = Reader::new(&[C_BUILD, 0xee]);
        assert!(get_command(&mut r).is_err());
        let mut r = Reader::new(&[
            C_SET_RULESET,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
        ]);
        assert!(get_command(&mut r).is_err());
    }
}
