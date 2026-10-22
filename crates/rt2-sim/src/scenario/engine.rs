//! Rule evaluation, effect application and trigger firing (docs/spec/scenarios.md sections 2.2,
//! 2.3, 2.4, 4.2, 4.4 and 5): `event_fire_trigger` `0x457fe0` and `rule_evaluate` `0x455f30`.
//!
//! The world supplies state and performs mutations through [`ScenarioWorld`]; the rule pool lives
//! in a [`Scenario`] that the world hands over for the duration of a call (`take_scenario` /
//! `put_scenario`), so rules can be mutated while the world is borrowed mutably.

use super::lang::{Ctx, Eval, ScenarioEnv, effect_money, reduce, undo_date};
use super::rule::{GameOver, MessageKind, N_EFFECTS, Rule, Scenario};
use crate::rng::GameRng;

/// Trigger codes of `Rule::trigger` (spec 2.2).
pub mod trigger {
    /// Start of year (after the rollover) and game start/load.
    pub const YEAR_START: u8 = 0;
    /// End of year (middle of `sim_year_end`, before the yearly files).
    pub const YEAR_END: u8 = 1;
    /// Start of month (after the date advanced) and game start/load.
    pub const MONTH_START: u8 = 2;
    /// End of month (beginning of `sim_month_end`).
    pub const MONTH_END: u8 = 3;
    /// Track or station placed or removed.
    pub const TRACK_CHANGED: u8 = 4;
    /// Company created.
    pub const COMPANY_CREATED: u8 = 5;
    /// Briefing: local evaluation, no effects.
    pub const BRIEFING: u8 = 6;
    /// Briefing status lines: local evaluation, no effects.
    pub const STATUS: u8 = 7;
}

/// Entity a variable or level effect applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Game,
    Company(i32),
    Person(i32),
    Territory(i32),
}

/// Which part of a territory a confiscate/destroy effect hits (spec 4.2, effects 55..59 and
/// 299..303).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confiscate {
    /// All four in the order track, trains, stations, buildings.
    All,
    /// `0x437d40`.
    Track,
    /// `0x437db0`.
    Trains,
    /// `0x437f50`.
    Stations,
    /// `0x4380a0`.
    Buildings,
}

/// Territory right masks of `0x4c8540` (spec 4.2).
pub mod rights {
    pub const TRAINS: u8 = 1;
    pub const TRACK: u8 = 2;
    pub const BUILDINGS: u8 = 4;
    pub const STATIONS: u8 = 8;
}

/// Message a rule shows (spec 2.4), texts with the substitutions applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    pub text: String,
    /// Button label 1 (newspaper: subtitle) and button label 2.
    pub label1: String,
    pub label2: String,
    pub trigger: u8,
    pub person: i32,
    pub company: i32,
    pub territory: i32,
}

/// What the engine needs from the world, beyond the function values of [`ScenarioEnv`].
///
/// Defaults are provided for hooks that are only relevant to the UI or to later milestones.
pub trait ScenarioWorld: ScenarioEnv {
    /// Hands the rule pool to the engine (leaving an empty one behind) and takes it back.
    fn take_scenario(&mut self) -> Scenario;
    fn put_scenario(&mut self, scenario: Scenario);

    /// Current game date as (year, month 1..=12).
    fn date(&self) -> (i32, i32);
    /// `economy_state` `[0x570717]` and its setter `0x469fb0`.
    fn economy_state(&self) -> i32;
    fn set_economy_state(&mut self, state: i32);

    /// The indexed sets the rule loops walk (`0x57072d` companies, `0x570779` players), in order.
    fn indexed_companies(&self) -> Vec<i32>;
    fn indexed_persons(&self) -> Vec<i32>;
    /// President of a company (`empresa+0x23`), 0 when none.
    fn company_person(&self, company: i32) -> i32;
    /// `0x43c030` (company of a human) and `pessoa+0x122 != 0`.
    fn is_human_company(&self, company: i32) -> bool;
    fn is_human_person(&self, person: i32) -> bool;
    /// `[0x574414]` and `[0x566ca4]`: the local company and player, 0 when none.
    fn local_company(&self) -> i32;
    fn local_person(&self) -> i32;
    /// `[0x570234]` and `[0x573f1c]`.
    fn is_campaign(&self) -> bool;
    fn is_multiplayer(&self) -> bool;
    /// `[0x5665f0]`: briefing window opened by the button (local evaluation, no effects).
    fn briefing_open(&self) -> bool {
        false
    }
    /// `[0x5748fc]`: editor active; `event_fire_trigger` does nothing.
    fn editor_active(&self) -> bool {
        false
    }
    /// Player of `0x46b3c0` used as game-over target in a network game with one player.
    fn fallback_player(&self) -> i32 {
        0
    }

    /// Names for the `(PLAYER)`, `(COMPANY)`, `(TERRITORY)` substitutions (the world supplies the
    /// default LNG text for id 0), `(COMPANY_LOADS)` (`empresa+0x19e`) and
    /// `(NUMBER_OF_STATIONS_VISITED)` (`0x4c0240`).
    fn person_name(&self, person: i32) -> String;
    fn company_name(&self, company: i32) -> String;
    fn territory_name(&self, territory: i32) -> String;
    fn company_loads_lifetime(&self, company: i32) -> i32;
    fn stations_visited(&self) -> i32;
    /// `(TRIGGER_MONEY)` formatting.
    fn format_money(&self, value: i32) -> String {
        format!("${value}")
    }

    /// Shows the message; returns the pressed button (0 or 1) for [`MessageKind::Choice`].
    /// Trigger 7 messages are appended to the status buffer `[0x5665e0]` by the world.
    fn show_message(&mut self, message: &Message) -> usize;

    // --- effect actions (spec 4.2) ---
    /// Effect 5: `company_ledger_credit(company, 4, amount)` (`0x439970`).
    fn credit_company(&mut self, company: i32, amount: i32);
    /// Effect 6: `0x4a7570(person, amount)`.
    fn credit_person(&mut self, person: i32, amount: i32);
    /// Effects 50..54: `0x4c8540(territory, mask, allow, company, 0)`.
    fn set_territory_rights(&mut self, territory: i32, company: i32, mask: u8, allow: bool);
    /// Effects 55..59 (`destroy` false) and 299..303 (`destroy` true).
    fn confiscate(&mut self, company: i32, territory: i32, what: Confiscate, destroy: bool);
    /// Effect 189: `0x438170(company, 1)`.
    fn deactivate_company(&mut self, company: i32);
    /// Effect 190 (AI persons only): `0x4a6a00`; also the game-over elimination.
    fn eliminate_player(&mut self, person: i32);
    /// Effect 191: `0x467bd0`.
    fn human_takes_player(&mut self, person: i32);
    /// Effect 192 with `b > 0`: persona table entry (spec 4.2).
    fn set_persona(&mut self, person: i32, index: i32);
    /// Effect 233 with `v > 0`: `0x4c8a30(territory, v)`.
    fn flood_territory(&mut self, territory: i32, amount: i32);
    /// Effect 235 with `b > 0`: retires train `id` when it exists (`0x4cdd50`).
    fn retire_train(&mut self, id: i32);
    /// Effects 236..240: sets the "rights for sale" flags `+0x44 + i` of the territory for each
    /// `i` in `which`.
    fn set_territory_buyable(&mut self, territory: i32, which: &[usize], on: bool);
    /// Effects 241..272: variable `index` (0..4) of the scope: set or add.
    fn set_variable(&mut self, scope: Scope, index: usize, value: i32, add: bool);
    /// Effects 297/298: `empresa+0x4260 += / = value`.
    fn change_track_allowed(&mut self, company: i32, value: i32, add: bool);
    /// Level effects: `i16[...+2e] += delta` of the scope (global when [`Scope::Game`]).
    fn add_level(&mut self, scope: Scope, effect: usize, delta: i32);
    /// A level effect in 127..=188 or 305..=308 ran (`[0x574784]` set).
    fn engines_changed(&mut self) {}
    /// Level effect in 193..=232 with a company: `0x438780(company, manager, 0)`.
    fn manager_effect(&mut self, company: i32, manager: usize);
}

/// Fires every rule of trigger `kind` in pool order (`event_fire_trigger` `0x457fe0`). After each
/// evaluated rule of that trigger the scan stops if the game-over state changed.
///
/// `rng` is the game RNG (`Random1To10000`); local evaluations use the world's CRT generator.
pub fn fire_trigger<W: ScenarioWorld>(kind: u8, world: &mut W, rng: &mut GameRng) {
    if world.editor_active() {
        return;
    }
    let mut sc = world.take_scenario();
    let mut i = 0;
    // The pool grows while it is scanned (undo rules are appended), like the original's list.
    while i < sc.rules.len() {
        if sc.rules[i].trigger == kind {
            let before = sc.game_over.state;
            // An evaluation error ends only this rule (spec 2.3).
            let _ = evaluate_rule(world, &mut sc, i, rng);
            if sc.game_over.state != before {
                break;
            }
        }
        i += 1;
    }
    world.put_scenario(sc);
}

/// Entities of one evaluation combination (0 = none).
#[derive(Clone, Copy, Debug, Default)]
pub struct Combo {
    pub company: i32,
    pub person: i32,
    pub territory: i32,
    /// Value the condition produced (`(TRIGGER)`, effect byte -100).
    pub value: i32,
}

/// `rule_evaluate` (`0x455f30`, `execute = 1`) for the rule at `index`. `Err` carries the error
/// code of a malformed condition; nothing else is evaluated then.
pub fn evaluate_rule<W: ScenarioWorld>(
    world: &mut W,
    sc: &mut Scenario,
    index: usize,
    rng: &mut GameRng,
) -> Result<(), u32> {
    let Scenario {
        rules,
        effects,
        game_over,
    } = sc;
    let mut undo: Vec<Rule> = Vec::new();
    {
        let rule = &mut rules[index];
        let multiplayer = world.is_multiplayer();
        if !world.is_campaign()
            && ((rule.game_mode == 1 && multiplayer) || (rule.game_mode == 2 && !multiplayer))
        {
            return Ok(());
        }
        let local_eval = matches!(rule.trigger, trigger::BRIEFING | trigger::STATUS)
            || rule.local_company
            || rule.local_player;
        let no_effects = local_eval || world.briefing_open();
        let mut t2t = 0;
        let mut shown_once = false;

        let n_persons = if rule.per_player {
            world.indexed_persons().len()
        } else {
            1
        };
        let n_companies = if rule.per_company {
            world.indexed_companies().len()
        } else {
            1
        };
        let territories = if rule.per_territory {
            world.territories()
        } else {
            vec![0]
        };

        for p in 0..n_persons {
            let person = if rule.per_player {
                match world.indexed_persons().get(p) {
                    Some(&x) => x,
                    None => continue,
                }
            } else {
                0
            };
            if rule.per_player {
                let ok = if rule.local_player {
                    person == world.local_person()
                } else if world.is_human_person(person) {
                    rule.human_players
                } else {
                    rule.ai_players
                };
                if !ok {
                    continue;
                }
            }
            for c in 0..n_companies {
                let company = if rule.per_company {
                    match world.indexed_companies().get(c) {
                        Some(&x) => x,
                        None => continue,
                    }
                } else {
                    0
                };
                if rule.per_company {
                    let ok = if rule.local_company {
                        company == world.local_company()
                    } else if world.is_human_company(company) {
                        rule.human_companies
                    } else {
                        rule.ai_companies
                    };
                    if !ok {
                        continue;
                    }
                    // ASSUMPTION: the owner check only applies with a company in the loop
                    // (spec 2.3 states it unconditionally, but a player-only rule has none).
                    if rule.per_player && world.company_person(company) != person {
                        continue;
                    }
                }
                for &territory in &territories {
                    if rule.territory_filter > 0 && i32::from(rule.territory_filter) != territory {
                        continue;
                    }
                    let mut combo = Combo {
                        company,
                        person,
                        territory,
                        value: 0,
                    };
                    let truth = if rule.date_mode {
                        let (y, m) = world.date();
                        combo.company == rule.date_company
                            && combo.person == rule.date_person
                            && (y, m) >= (rule.date_year, rule.date_month)
                    } else {
                        let ctx = Ctx {
                            company,
                            person,
                            territory,
                            crt_rand: local_eval,
                        };
                        let mut ev = Eval::new(&mut *world, rng, ctx);
                        ev.last_t2t = t2t;
                        let r = reduce(&mut ev, &rule.tokens, &rule.values);
                        t2t = ev.last_t2t;
                        combo.value = r?;
                        combo.value != 0
                    };
                    if !(truth || (rule.show_when_false && rule.trigger == trigger::STATUS)) {
                        continue;
                    }
                    // "once" rules keep evaluating (and drawing); only message and effects stop.
                    if rule.once
                        && rule.fired
                        && !matches!(rule.trigger, trigger::BRIEFING | trigger::STATUS)
                    {
                        continue;
                    }
                    let set = show_message(world, rule, game_over, combo, t2t, &mut shown_once);
                    rule.fired = true;
                    if !no_effects {
                        let temporary = apply_effects(world, game_over, rule, set, combo);
                        if temporary {
                            undo.push(undo_rule(world, effects, rule, set, combo));
                        }
                    }
                }
            }
        }
    }
    rules.extend(undo);
    Ok(())
}

/// Spec 2.4: text conditions, substitutions, the game-over buffer and the dialog. Returns the
/// effect set to apply (0, or the button of a choice).
fn show_message<W: ScenarioWorld>(
    world: &mut W,
    rule: &Rule,
    game_over: &mut GameOver,
    combo: Combo,
    t2t: i32,
    shown_once: &mut bool,
) -> usize {
    let has_text = rule.message.len() > 1 && !(*shown_once && rule.first_true_only);
    let visible = combo.person == world.local_person()
        || combo.company == world.local_company()
        || (combo.person == 0 && combo.company == 0)
        || rule.trigger == trigger::STATUS;
    if !(has_text && visible) {
        return 0;
    }
    *shown_once = true;
    let sub = |s: &str| substitute(world, s, combo, t2t);
    let msg = Message {
        kind: rule.message_kind,
        text: sub(&rule.message),
        label1: sub(&rule.label1),
        label2: sub(&rule.label2),
        trigger: rule.trigger,
        person: combo.person,
        company: combo.company,
        territory: combo.territory,
    };
    if rule.has_game_over_effect() {
        if game_over.state == 0 {
            game_over.text = msg.text;
        }
        return 0;
    }
    match rule.message_kind {
        MessageKind::Choice => {
            // In a network game (two or more players) the choice is always 0, no window.
            if world.indexed_persons().len() >= 2 {
                0
            } else {
                world.show_message(&msg).min(1)
            }
        }
        _ => {
            world.show_message(&msg);
            0
        }
    }
}

/// Text substitutions of spec 2.4, in the order of the original.
pub fn substitute<W: ScenarioWorld>(world: &W, text: &str, c: Combo, t2t: i32) -> String {
    let mut s = text.to_string();
    let mut rep = |key: &str, value: String| {
        if s.contains(key) {
            s = s.replace(key, &value);
        }
    };
    rep("(PLAYER)", world.person_name(c.person));
    rep("(COMPANY)", world.company_name(c.company));
    rep("(GESELLSCHAFT)", world.company_name(c.company));
    rep("(TERRITORY)", world.territory_name(c.territory));
    rep(
        "(COMPANY_LOADS)",
        world.company_loads_lifetime(c.company).to_string(),
    );
    rep("(COMPANY_T2T_LOADS)", t2t.to_string());
    rep(
        "(NUMBER_OF_STATIONS_VISITED)",
        world.stations_visited().to_string(),
    );
    rep("(TRIGGER)", c.value.to_string());
    rep("(TRIGGER_MONEY)", world.format_money(c.value));
    rep(
        "(TRIGGER_TF)",
        if c.value != 0 { "true" } else { "false" }.to_string(),
    );
    s
}

/// Effects 1..=4 (spec 5): sets the end-of-game state, only while the game is running.
fn game_over_effect<W: ScenarioWorld>(world: &mut W, game_over: &mut GameOver, e: usize, c: Combo) {
    if game_over.state != 0 {
        return;
    }
    let mut target = c.person;
    if target == 0 && c.company != 0 {
        target = world.company_person(c.company);
    }
    if target == 0 {
        let players = world.indexed_persons();
        target = if players.len() == 1 {
            players[0]
        } else {
            world.fallback_player()
        };
    }
    let local = world.local_person();
    if e == 4 {
        if target == local || target == 0 {
            game_over.state = 4;
        } else {
            world.eliminate_player(target);
        }
    } else if target == local {
        game_over.state = e as u8;
    } else {
        game_over.state = 4;
        game_over.other_won = true;
    }
}

/// Applies effect set `set` of `rule` for one combination (spec 4.2). Returns whether a level
/// effect ran in a temporary set, which requires an undo rule (spec 4.4).
fn apply_effects<W: ScenarioWorld>(
    world: &mut W,
    game_over: &mut GameOver,
    rule: &Rule,
    set: usize,
    c: Combo,
) -> bool {
    let mut temporary = false;
    let bytes = &rule.effects[set];
    // `-100` (0x9c) means "the value the condition produced".
    let var_value = |b: i8| if b == -100 { c.value } else { i32::from(b) };
    for (e, &b) in bytes.iter().enumerate().take(N_EFFECTS) {
        if b == 0 {
            continue;
        }
        let bi = i32::from(b);
        match e {
            1..=4 => game_over_effect(world, game_over, e, c),
            5 if c.company != 0 => world.credit_company(c.company, effect_money(bi)),
            6 if c.person != 0 => world.credit_person(c.person, effect_money(bi)),
            7 => {
                let n = (world.economy_state() + bi).clamp(0, 4);
                world.set_economy_state(n);
            }
            50..=54 if c.territory != 0 => {
                let masks: &[u8] = match e {
                    50 => &[2, 1, 4, 8],
                    51 => &[rights::TRACK],
                    52 => &[rights::TRAINS],
                    53 => &[rights::STATIONS],
                    _ => &[rights::BUILDINGS],
                };
                for &m in masks {
                    world.set_territory_rights(c.territory, c.company, m, b >= 0);
                }
            }
            55..=59 | 299..=303 => {
                let destroy = e >= 299;
                let what = match e - if destroy { 299 } else { 55 } {
                    0 => Confiscate::All,
                    1 => Confiscate::Track,
                    2 => Confiscate::Trains,
                    3 => Confiscate::Stations,
                    _ => Confiscate::Buildings,
                };
                world.confiscate(c.company, c.territory, what, destroy);
            }
            189 if c.company != 0 => world.deactivate_company(c.company),
            190 if c.person != 0 && !world.is_human_person(c.person) => {
                world.eliminate_player(c.person);
            }
            191 if c.person != 0 => world.human_takes_player(c.person),
            192 if b > 0 && c.person != 0 => world.set_persona(c.person, bi),
            233 if c.territory != 0 => {
                let v = var_value(b);
                if v > 0 {
                    world.flood_territory(c.territory, v);
                }
            }
            235 if b > 0 => world.retire_train(bi),
            236..=240 if c.territory != 0 => {
                let which: &[usize] = match e {
                    236 => &[0, 1, 2, 3],
                    237 => &[1],
                    238 => &[0],
                    239 => &[2],
                    _ => &[3],
                };
                world.set_territory_buyable(c.territory, which, b >= 0);
            }
            241..=248 => {
                let i = (e - 241) % 4;
                world.set_variable(Scope::Game, i, var_value(b), e >= 245);
            }
            249..=256 if c.company != 0 => {
                let i = (e - 249) % 4;
                world.set_variable(Scope::Company(c.company), i, var_value(b), e >= 253);
            }
            257..=264 if c.person != 0 => {
                let i = (e - 257) % 4;
                world.set_variable(Scope::Person(c.person), i, var_value(b), e >= 261);
            }
            265..=272 if c.territory != 0 => {
                let i = (e - 265) % 4;
                world.set_variable(Scope::Territory(c.territory), i, var_value(b), e >= 269);
            }
            297 | 298 if c.company != 0 => {
                let v = if b == -100 {
                    c.value
                } else {
                    effect_money(bi) / 1000
                };
                world.change_track_allowed(c.company, v, e == 297);
            }
            // Effects with their own branch whose requirement is unmet do nothing.
            5..=7 | 50..=59 | 189..=192 | 233 | 235..=248 | 249..=272 | 297..=303 => {}
            _ => {
                // Level effect: pessoa > empresa > territorio > global (spec 4.2).
                let scope = if c.person != 0 {
                    Scope::Person(c.person)
                } else if c.company != 0 {
                    Scope::Company(c.company)
                } else if c.territory != 0 {
                    Scope::Territory(c.territory)
                } else {
                    Scope::Game
                };
                world.add_level(scope, e, bi);
                if !rule.permanent[set] {
                    temporary = true;
                }
                if (0x7f..=0xbc).contains(&e) || (0x131..=0x134).contains(&e) {
                    world.engines_changed();
                }
                if (0xc1..=0xe8).contains(&e) && c.company != 0 {
                    world.manager_effect(c.company, e - 0xc1);
                }
            }
        }
    }
    temporary
}

/// The undo rule of a temporary effect set (spec 4.4).
fn undo_rule<W: ScenarioWorld>(
    world: &W,
    table: &super::rule::EffectTable,
    rule: &Rule,
    set: usize,
    c: Combo,
) -> Rule {
    let (year, month) = world.date();
    let (uy, um) = undo_date(year, month, rule.duration[set]);
    let neg: Vec<i8> = rule.effects[set]
        .iter()
        .enumerate()
        .map(|(e, &b)| {
            if table.is_instant(e) {
                0
            } else {
                b.wrapping_neg()
            }
        })
        .collect();
    Rule {
        trigger: trigger::MONTH_START,
        date_mode: true,
        date_year: uy,
        date_month: um,
        date_person: c.person,
        date_company: c.company,
        per_company: c.company != 0,
        per_player: c.person != 0,
        ai_companies: true,
        human_companies: true,
        ai_players: true,
        human_players: true,
        once: true,
        permanent: [true; 2],
        effects: [neg, vec![0; N_EFFECTS]],
        ..Rule::default()
    }
}
