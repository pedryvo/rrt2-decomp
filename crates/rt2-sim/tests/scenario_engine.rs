//! Rule engine behaviour (docs/spec/scenarios.md 2.2-2.4, 4.2, 4.4, 5) on a small mock world:
//! RNG draw order, once-only rules, effect application, undo rules, game-over state.

use rt2_sim::rng::GameRng;
use rt2_sim::scenario::{
    Confiscate, EffectRow, EffectTable, Message, N_EFFECTS, Rule, Scenario, ScenarioEnv,
    ScenarioWorld, Scope, fire_trigger, trigger,
};

#[derive(Default)]
struct Mock {
    sc: Scenario,
    year: i32,
    month: i32,
    economy: i32,
    companies: Vec<i32>,
    persons: Vec<i32>,
    local_person: i32,
    cash: Vec<(i32, i32)>,
    levels: Vec<(Scope, usize, i32)>,
    vars: Vec<(Scope, usize, i32, bool)>,
    messages: Vec<Message>,
    eliminated: Vec<i32>,
    crt: u32,
    net: bool,
    outcomes: Vec<(i32, u8, bool)>,
}

impl ScenarioEnv for Mock {
    fn value(&mut self, id: usize, _p: i32, company: i32, _person: i32, _t: i32) -> i32 {
        match id {
            1 => 1,
            9 => self.year,
            10 => self.month,
            38 => company,
            // CompanyCash: 1000 * company id
            23 => company * 1000,
            _ => 0,
        }
    }
    fn crt_range(&mut self, lo: i32, hi: i32) -> i32 {
        self.crt = self.crt.wrapping_mul(214013).wrapping_add(2531011);
        lo + (((self.crt >> 16) & 0x7fff) as i32) % (hi - lo + 1)
    }
    fn companies(&self) -> Vec<i32> {
        self.companies.clone()
    }
    fn persons(&self) -> Vec<i32> {
        self.persons.clone()
    }
    fn territories(&self) -> Vec<i32> {
        vec![1, 2]
    }
}

impl ScenarioWorld for Mock {
    fn take_scenario(&mut self) -> Scenario {
        std::mem::take(&mut self.sc)
    }
    fn put_scenario(&mut self, s: Scenario) {
        self.sc = s;
    }
    fn date(&self) -> (i32, i32) {
        (self.year, self.month)
    }
    fn economy_state(&self) -> i32 {
        self.economy
    }
    fn set_economy_state(&mut self, s: i32) {
        self.economy = s;
    }
    fn indexed_companies(&self) -> Vec<i32> {
        self.companies.clone()
    }
    fn indexed_persons(&self) -> Vec<i32> {
        self.persons.clone()
    }
    fn company_person(&self, company: i32) -> i32 {
        company
    }
    fn is_human_company(&self, company: i32) -> bool {
        company == 1
    }
    fn is_human_person(&self, person: i32) -> bool {
        person == 1
    }
    fn local_company(&self) -> i32 {
        1
    }
    fn local_person(&self) -> i32 {
        self.local_person
    }
    fn is_campaign(&self) -> bool {
        false
    }
    fn is_multiplayer(&self) -> bool {
        self.net
    }
    fn network_active(&self) -> bool {
        self.net
    }
    fn record_net_outcome(&mut self, person: i32, state: u8, other_won: bool) {
        self.outcomes.push((person, state, other_won));
    }
    fn person_name(&self, p: i32) -> String {
        format!("P{p}")
    }
    fn company_name(&self, c: i32) -> String {
        format!("C{c}")
    }
    fn territory_name(&self, t: i32) -> String {
        format!("T{t}")
    }
    fn company_loads_lifetime(&self, _c: i32) -> i32 {
        0
    }
    fn stations_visited(&self) -> i32 {
        0
    }
    fn show_message(&mut self, m: &Message) -> usize {
        self.messages.push(m.clone());
        0
    }
    fn credit_company(&mut self, company: i32, amount: i32) {
        self.cash.push((company, amount));
    }
    fn credit_person(&mut self, _p: i32, _a: i32) {}
    fn set_territory_rights(&mut self, _t: i32, _c: i32, _m: u8, _a: bool, _r: &mut GameRng) {}
    fn confiscate(&mut self, _c: i32, _t: i32, _w: Confiscate, _d: bool, _r: &mut GameRng) {}
    fn deactivate_company(&mut self, _c: i32, _rng: &mut GameRng) {}
    fn eliminate_player(&mut self, p: i32) {
        self.eliminated.push(p);
    }
    fn human_takes_player(&mut self, _p: i32) {}
    fn set_persona(&mut self, _p: i32, _i: i32) {}
    fn flood_territory(&mut self, _t: i32, _a: i32, _rng: &mut GameRng) {}
    fn retire_train(&mut self, _id: i32, _rng: &mut GameRng) {}
    fn set_territory_buyable(&mut self, _t: i32, _w: &[usize], _o: bool) {}
    fn set_variable(&mut self, scope: Scope, index: usize, value: i32, add: bool) {
        self.vars.push((scope, index, value, add));
    }
    fn change_track_allowed(&mut self, _c: i32, _v: i32, _a: bool) {}
    fn add_level(&mut self, scope: Scope, effect: usize, delta: i32) {
        self.levels.push((scope, effect, delta));
    }
    fn manager_effect(&mut self, _c: i32, _m: usize, _rng: &mut GameRng) {}
}

/// All rows non-instant except 1..=7 and the variable rows.
fn table() -> EffectTable {
    EffectTable {
        rows: (0..N_EFFECTS)
            .map(|e| EffectRow {
                ui_id: e as u16,
                visible: true,
                needs_company: false,
                needs_player: false,
                needs_territory: false,
                instant: (1..=7).contains(&e) || (241..=272).contains(&e),
                min: -99,
                max: 100,
                name_lng: 0,
            })
            .collect(),
    }
}

fn world(rules: Vec<Rule>) -> Mock {
    Mock {
        sc: Scenario::new(rules, table()),
        year: 1850,
        month: 3,
        companies: vec![1, 2, 3],
        persons: vec![1, 2],
        local_person: 1,
        ..Mock::default()
    }
}

fn rule(cond: &str, trig: u8) -> Rule {
    Rule::from_condition(cond, trig).unwrap()
}

#[test]
fn random_draws_in_text_order_without_short_circuit() {
    // `False and Random...` still draws: both draws happen even though the left side is false.
    let mut w = world(vec![rule(
        "False and Random1To10000>5 and Random1To10000>5",
        trigger::MONTH_START,
    )]);
    let mut rng = GameRng::new(42);
    fire_trigger(trigger::MONTH_START, &mut w, &mut rng);
    let mut want = GameRng::new(42);
    want.range(1, 10000);
    want.range(1, 10000);
    assert_eq!(rng.state(), want.state());
    // other triggers do not touch the RNG
    fire_trigger(trigger::YEAR_END, &mut w, &mut rng);
    assert_eq!(rng.state(), want.state());
}

#[test]
fn once_rule_keeps_drawing_but_applies_effects_once() {
    let mut r = rule("Random1To10000>=1", trigger::MONTH_START);
    r.once = true;
    r.effects[0][7] = 1; // Economy Status +1
    let mut w = world(vec![r]);
    let mut rng = GameRng::new(7);
    fire_trigger(trigger::MONTH_START, &mut w, &mut rng);
    fire_trigger(trigger::MONTH_START, &mut w, &mut rng);
    assert_eq!(w.economy, 1);
    assert!(w.sc.rules[0].fired);
    let mut want = GameRng::new(7);
    want.range(1, 10000);
    want.range(1, 10000);
    assert_eq!(rng.state(), want.state());
}

#[test]
fn company_loop_cash_and_message_visibility() {
    let mut r = rule("CompanyCash>=2000", trigger::MONTH_END);
    r.effects[0][5] = 14; // Company Cash: 14 -> $50,000
    r.message = "(COMPANY) earned (TRIGGER_TF)".into();
    r.human_companies = true;
    r.ai_companies = true;
    let mut w = world(vec![r]);
    fire_trigger(trigger::MONTH_END, &mut w, &mut GameRng::new(1));
    assert_eq!(w.cash, [(2, 50_000), (3, 50_000)]);
    // message only for the local company's combination or global ones: company 1 is false
    assert!(w.messages.is_empty());
    let mut r = rule("CompanyCash>=1000", trigger::MONTH_END);
    r.message = "(COMPANY) earned (TRIGGER_TF)".into();
    let mut w = world(vec![r]);
    fire_trigger(trigger::MONTH_END, &mut w, &mut GameRng::new(1));
    assert_eq!(w.messages.len(), 1);
    assert_eq!(w.messages[0].text, "C1 earned True");
}

#[test]
fn game_over_state_stops_the_scan_and_follows_the_target() {
    let mut win = rule("GameYear>=1850", trigger::YEAR_END);
    win.effects[0][3] = 1; // Game Over - Win 3
    win.message = "You won".into();
    let mut later = rule("True", trigger::YEAR_END);
    later.effects[0][7] = 1;
    let mut w = world(vec![win.clone(), later.clone()]);
    fire_trigger(trigger::YEAR_END, &mut w, &mut GameRng::new(1));
    // no person/company in the context and two players: target 0 -> not the local player
    // (local is person 1), so the local player loses to "someone else"
    assert_eq!(w.sc.game_over.state, 4);
    assert!(w.sc.game_over.other_won);
    assert_eq!(w.economy, 0, "scan stopped after the state changed");
    // a single player that is the local one wins
    let mut w = world(vec![win, later]);
    w.persons = vec![1];
    fire_trigger(trigger::YEAR_END, &mut w, &mut GameRng::new(1));
    assert_eq!(w.sc.game_over.state, 3);
    assert_eq!(w.sc.game_over.text, "You won");
}

#[test]
fn temporary_level_effect_creates_and_runs_an_undo_rule() {
    let mut r = rule("CompanyId=2", trigger::MONTH_START);
    r.effects[0][9] = 20; // Track Building +20%
    r.permanent[0] = false;
    r.duration[0] = 24;
    let mut w = world(vec![r]);
    let mut rng = GameRng::new(1);
    fire_trigger(trigger::MONTH_START, &mut w, &mut rng);
    assert_eq!(w.levels, [(Scope::Company(2), 9, 20)]);
    assert_eq!(w.sc.rules.len(), 2);
    let undo = &w.sc.rules[1];
    assert!(undo.date_mode);
    assert_eq!((undo.date_year, undo.date_month), (1852, 3));
    assert_eq!(undo.date_company, 2);
    assert_eq!(undo.effects[0][9], -20);
    // not yet due
    fire_trigger(trigger::MONTH_START, &mut w, &mut rng);
    assert_eq!(
        w.levels.len(),
        2,
        "the original rule adds again, the undo is not due"
    );
    w.levels.clear();
    w.year = 1852;
    w.month = 3;
    // the original rule fires again (permanent-true condition) and the undo rule runs
    fire_trigger(trigger::MONTH_START, &mut w, &mut rng);
    assert!(w.levels.contains(&(Scope::Company(2), 9, -20)));
    assert!(w.sc.rules[1].fired);
}

#[test]
fn variables_and_level_scope_priority() {
    let mut r = rule("True", trigger::YEAR_START);
    r.effects[0][241] = 5; // Set GameVariable1 = 5
    r.effects[0][246] = -100; // Add To GameVariable2 = trigger value (1)
    r.effects[0][12] = 3; // level effect: no entities -> global
    let mut w = world(vec![r]);
    fire_trigger(trigger::YEAR_START, &mut w, &mut GameRng::new(1));
    assert_eq!(
        w.vars,
        [(Scope::Game, 0, 5, false), (Scope::Game, 1, 1, true)]
    );
    assert_eq!(w.levels, [(Scope::Game, 12, 3)]);
}

#[test]
fn local_evaluation_uses_the_crt_generator_and_applies_no_effects() {
    let mut r = rule("Random1To10000>=1", trigger::BRIEFING);
    r.effects[0][7] = 1;
    let mut w = world(vec![r]);
    let mut rng = GameRng::new(9);
    fire_trigger(trigger::BRIEFING, &mut w, &mut rng);
    assert_eq!(rng.state(), GameRng::new(9).state(), "game RNG untouched");
    assert_ne!(w.crt, 0, "CRT generator drawn");
    assert_eq!(w.economy, 0);
}

/// Network game (docs/spec/multiplayer.md 3.3): the first to reach the goal wins, every other
/// human loses with "another player achieved the goals" (LNG 2691), and the outcome is kept per
/// person so that every machine computes the same; a defeat eliminates only its target.
#[test]
fn network_victory_is_per_person_and_defeat_eliminates_only_the_target() {
    let mut win = rule("True", trigger::YEAR_END);
    win.effects[0][2] = 1; // Game Over - Win 2
    let mut w = world(vec![win]);
    w.net = true;
    w.persons = vec![1, 2, 3];
    // Person 1 is the only human of the mock; the rule has no person in its context, so the
    // fallback target (person 0) wins and the human loses to it.
    w.local_person = 1;
    fire_trigger(trigger::YEAR_END, &mut w, &mut GameRng::new(1));
    assert!(w.outcomes.contains(&(0, 2, false)), "{:?}", w.outcomes);
    assert!(w.outcomes.contains(&(1, 4, true)), "{:?}", w.outcomes);
    assert!(
        !w.outcomes.iter().any(|&(p, _, _)| p == 3),
        "computers have no outcome"
    );
    assert_ne!(w.sc.game_over.state, 0, "the game was decided once");

    let mut lose = rule("True", trigger::YEAR_END);
    lose.effects[0][4] = 1; // Game Over - Lose
    let mut w = world(vec![lose]);
    w.net = true;
    w.persons = vec![1, 2];
    fire_trigger(trigger::YEAR_END, &mut w, &mut GameRng::new(1));
    assert_eq!(w.sc.game_over.state, 0, "the game goes on after a defeat");
}
