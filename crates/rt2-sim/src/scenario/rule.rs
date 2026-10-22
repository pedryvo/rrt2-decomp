//! Scenario rules, the effect table and the victory/loss state (docs/spec/scenarios.md sections
//! 2.1, 4.1 and 5).

use rt2_formats::map;

use super::lang::{CompileError, compile, loop_flags};
use super::table::N_ENTRIES;
use crate::exe::{Exe, ExeError};

/// Address of the effect table (spec 4.1).
pub const ADDR_EFFECT_TABLE: u32 = 0x0055_0550;
/// Rows of the effect table.
pub const N_EFFECTS: usize = 350;
const EFFECT_ROW_SIZE: usize = 16;

/// One row of the effect table (16 bytes at `0x550550`, spec 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectRow {
    /// Editor control id (`+0`).
    pub ui_id: u16,
    /// `+2`: visible in the editor.
    pub visible: bool,
    /// `+3..+5`: the effect needs the rule to test against each company / player / territory.
    pub needs_company: bool,
    pub needs_player: bool,
    pub needs_territory: bool,
    /// `+0xb`: not undone by a temporary rule (spec 4.4).
    pub instant: bool,
    /// Control range (`+0xc`, `+0xd`).
    pub min: i8,
    pub max: i8,
    /// LNG string id of the name (`+0xe`), 0 when unnamed.
    pub name_lng: u16,
}

/// The 350-row effect table, read from the executable.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EffectTable {
    pub rows: Vec<EffectRow>,
}

impl EffectTable {
    /// Reads the table at [`ADDR_EFFECT_TABLE`].
    pub fn load(exe: &Exe) -> Result<EffectTable, ExeError> {
        let raw = exe.read(ADDR_EFFECT_TABLE, N_EFFECTS * EFFECT_ROW_SIZE)?;
        Ok(Self::from_bytes(raw))
    }

    /// Decodes `N_EFFECTS` rows of 16 bytes.
    pub fn from_bytes(raw: &[u8]) -> EffectTable {
        let rows = raw
            .as_chunks::<EFFECT_ROW_SIZE>()
            .0
            .iter()
            .take(N_EFFECTS)
            .map(|r| EffectRow {
                ui_id: u16::from_le_bytes([r[0], r[1]]),
                visible: r[2] != 0,
                needs_company: r[3] != 0,
                needs_player: r[4] != 0,
                needs_territory: r[5] != 0,
                instant: r[11] != 0,
                min: r[12] as i8,
                max: r[13] as i8,
                name_lng: u16::from_le_bytes([r[14], r[15]]),
            })
            .collect();
        EffectTable { rows }
    }

    /// Whether effect `e` is instant (not negated by an undo rule); unknown rows count as instant.
    pub fn is_instant(&self, e: usize) -> bool {
        self.rows.get(e).is_none_or(|r| r.instant)
    }
}

/// Message type of a rule (`+0x60`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MessageKind {
    /// Plain dialog (`0x469320`).
    #[default]
    Dialog,
    /// Newspaper (`0x4a42f0`): title is the main text, subtitle is label 1.
    Newspaper,
    /// Two-button choice (`0x4510a0`): the button picks the effect set.
    Choice,
}

/// A scenario rule (the 0x362-byte record of spec 2.1 decoded into named fields).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Rule {
    pub condition: String,
    /// Compiled token ids and values (`+0x08`, `+0x0c`).
    pub tokens: Vec<i32>,
    pub values: Vec<i32>,
    /// `+0x10`: date mode, only runtime-generated undo rules (spec 4.4).
    pub date_mode: bool,
    /// `+0x14` / `+0x18`: year and month of the date mode.
    pub date_year: i32,
    pub date_month: i32,
    /// `+0x1c` / `+0x20`: person and company of the date mode.
    pub date_person: i32,
    pub date_company: i32,
    /// `+0x24` / `+0x28` / `+0x324`: test against each company / player / territory.
    pub per_company: bool,
    pub per_player: bool,
    pub per_territory: bool,
    /// `+0x34`: trigger 0..=7 (spec 2.2).
    pub trigger: u8,
    /// `+0x38` / `+0x3c`: company loop includes AI / human companies.
    pub ai_companies: bool,
    pub human_companies: bool,
    /// `+0x40` / `+0x44`: player loop includes AI / human players.
    pub ai_players: bool,
    pub human_players: bool,
    /// `+0x48` / `+0x4c`: fire once only, and whether it already fired.
    pub once: bool,
    pub fired: bool,
    pub message_kind: MessageKind,
    /// `+0x50`, `+0x54`, `+0x58`: main text, label/subtitle 1, label 2.
    pub message: String,
    pub label1: String,
    pub label2: String,
    /// `+0x5c`: comment/title.
    pub title: String,
    /// `+0x328`: restricts the territory loop to one territory (<= 0: all).
    pub territory_filter: i16,
    /// `+0x32a`: trigger 7 shows the text even with a false condition.
    pub show_when_false: bool,
    /// `+0x32e`: show the message only for the first true combination of an evaluation.
    pub first_true_only: bool,
    /// `+0x34f` / `+0x350`: local evaluation (local company / local player only, CRT `rand`,
    /// no effects).
    pub local_company: bool,
    pub local_player: bool,
    /// `+0x351`: 0 any game, 1 local game only, 2 network game only (ignored in a campaign).
    pub game_mode: u8,
    /// `+0x352 + 4k`: effect set `k` is permanent (otherwise temporary).
    pub permanent: [bool; 2],
    /// `+0x35a + 4k`: duration in months of a temporary effect set.
    pub duration: [i32; 2],
    /// `+0x64 + k * stride + e`: effect byte `e` of set `k` (0 = none, -100 = trigger value).
    pub effects: [Vec<i8>; 2],
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

impl Rule {
    /// Decodes a rule stored in a map or save file. The 0x2C8 layout of `.MAP` files has the same
    /// offsets below `0x64` and `0x9A` less from `0x320`; its effect blocks are 0x111 bytes.
    pub fn from_file(r: &map::Rule) -> Rule {
        let f = &r.fixed;
        let long = f.len() >= 0x362;
        let at = |off: usize| if long || off < 0x320 { off } else { off - 0x9a };
        let u32f = |off: usize| u32_at(f, at(off));
        let byte = |off: usize| f.get(at(off)).copied().unwrap_or(0);
        let stride = if long { 0x15e } else { 0x111 };
        let effect_set = |k: usize| {
            let mut v = vec![0i8; super::rule::N_EFFECTS];
            for (e, slot) in v.iter_mut().enumerate().take(stride) {
                *slot = f.get(0x64 + k * stride + e).copied().unwrap_or(0) as i8;
            }
            v
        };
        Rule {
            condition: r.condition.clone(),
            tokens: r.tokens_a.clone(),
            values: r.tokens_b.clone(),
            date_mode: u32f(0x10) != 0,
            date_year: u32f(0x14) as i32,
            date_month: u32f(0x18) as i32,
            date_person: u32f(0x1c) as i32,
            date_company: u32f(0x20) as i32,
            per_company: u32f(0x24) != 0,
            per_player: u32f(0x28) != 0,
            per_territory: u32f(0x324) != 0,
            trigger: u32f(0x34) as u8,
            ai_companies: u32f(0x38) != 0,
            human_companies: u32f(0x3c) != 0,
            ai_players: u32f(0x40) != 0,
            human_players: u32f(0x44) != 0,
            once: u32f(0x48) != 0,
            fired: u32f(0x4c) != 0,
            message_kind: match u32f(0x60) {
                1 => MessageKind::Newspaper,
                2 => MessageKind::Choice,
                _ => MessageKind::Dialog,
            },
            message: r.message.clone(),
            label1: r.text2.clone(),
            label2: r.text3.clone(),
            title: r.title.clone(),
            territory_filter: u32f(0x328) as u16 as i16,
            show_when_false: u32f(0x32a) != 0,
            first_true_only: u32f(0x32e) != 0,
            local_company: byte(0x34f) != 0,
            local_player: byte(0x350) != 0,
            game_mode: byte(0x351),
            permanent: [u32f(0x352) != 0, u32f(0x356) != 0],
            duration: [u32f(0x35a) as i32, u32f(0x35e) as i32],
            effects: [effect_set(0), effect_set(1)],
        }
    }

    /// A rule from condition source text: compiles it and derives the loop flags like the editor
    /// (`rule_set_condition` `0x457a30`). Everything else takes the defaults of a new editor rule
    /// (`+0x38..+0x44 = 1`, one-shot off).
    pub fn from_condition(condition: &str, trigger: u8) -> Result<Rule, CompileError> {
        let (tokens, values) = compile(condition)?;
        debug_assert!(tokens.iter().all(|&t| (t as usize) < N_ENTRIES));
        let (c, p, t) = loop_flags(&tokens);
        Ok(Rule {
            condition: condition.to_string(),
            tokens,
            values,
            per_company: c,
            per_player: p,
            per_territory: t,
            trigger,
            ai_companies: true,
            human_companies: true,
            ai_players: true,
            human_players: true,
            permanent: [true; 2],
            effects: [vec![0; N_EFFECTS], vec![0; N_EFFECTS]],
            ..Rule::default()
        })
    }

    /// Whether any of the game-over effects 1..=4 is set in effect set 0 (`+0x65..+0x68`).
    pub fn has_game_over_effect(&self) -> bool {
        self.effects[0].iter().skip(1).take(4).any(|&b| b != 0)
    }
}

/// End-of-game state (`[0x573d70]`, spec 5) with the text of the victory/defeat frame
/// (`0x574b90`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GameOver {
    /// 0 playing, 1/2/3 victory bronze/silver/gold, 4 defeat.
    pub state: u8,
    /// Text the rule supplied for the frame (empty without message).
    pub text: String,
    /// Defeat because another player won (LNG `0xa83`).
    pub other_won: bool,
}

/// The rule pool (`0x574c9c`), the effect table and the end-of-game state.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Scenario {
    /// Pool order; undo rules are appended at the end.
    pub rules: Vec<Rule>,
    pub effects: EffectTable,
    pub game_over: GameOver,
}

impl Scenario {
    pub fn new(rules: Vec<Rule>, effects: EffectTable) -> Scenario {
        Scenario {
            rules,
            effects,
            game_over: GameOver::default(),
        }
    }

    /// The rules of a parsed map or save, in file order.
    pub fn from_save(save: &map::Save, effects: EffectTable) -> Scenario {
        Scenario::new(save.rules.iter().map(Rule::from_file).collect(), effects)
    }
}
