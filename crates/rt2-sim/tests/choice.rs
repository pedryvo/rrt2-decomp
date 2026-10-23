//! Interactive scenario choices (`rt2_sim::choice`, docs/spec/scenarios.md 2.4.1): a choice waits
//! for `Command::AnswerChoice`, the step is replayed with the answer, and answering button 0
//! gives the same world as a headless run. Needs `RT2_DIR` (skipped otherwise).

use std::path::PathBuf;

use rt2_formats::map::Save;
use rt2_sim::World;
use rt2_sim::command::Command;
use rt2_sim::data::GameData;
use rt2_sim::exe::Exe;
use rt2_sim::scenario::{MessageKind, Rule, trigger};

/// `0x573d8e`: game variable 0 (effects 241..244 set it).
const ST_GAME_VAR0: usize = 0x38A6;

fn setup() -> Option<(GameData, Save)> {
    let game = PathBuf::from(std::env::var_os("RT2_DIR")?);
    let exe = Exe::from_bytes(std::fs::read(game.join("RT2_PLAT.EXE")).unwrap()).unwrap();
    let mut data = GameData::load(&exe).unwrap();
    data.load_sprites(&game).unwrap();
    let mut save = Save::parse(&std::fs::read(game.join("maps/Alaska.MP2")).unwrap()).unwrap();
    // A one-shot month-start choice: button 1 sets variable 0 to 1, button 2 to 2.
    let mut r = Rule::from_condition("True", trigger::MONTH_START).unwrap();
    r.message_kind = MessageKind::Choice;
    r.message = "Pick one".into();
    r.label1 = "One".into();
    r.label2 = "Two".into();
    r.once = true;
    r.effects[0][241] = 1;
    r.effects[1][241] = 2;
    save.rules.push(r.to_file());
    Some((data, save))
}

/// Advances 40 days in 320-minute steps (or until a choice is pending); returns the steps left.
fn run(w: &mut World, mut left: i32) -> i32 {
    while left > 0 {
        let before = w.minutes();
        w.advance(320);
        if w.pending_choice().is_some() {
            assert_eq!(w.minutes(), before, "a pending choice throws the step away");
            return left;
        }
        left -= 320;
    }
    0
}

fn var0(w: &World) -> i32 {
    let s = w.to_save();
    i32::from_le_bytes(s.state[ST_GAME_VAR0..ST_GAME_VAR0 + 4].try_into().unwrap())
}

#[test]
fn choice_waits_for_the_answer_and_replays_the_step() {
    let Some((data, save)) = setup() else {
        return;
    };
    let steps = 40 * 1440;
    let mut headless = World::from_save_seeded(&save, &data, 1234);
    assert_eq!(run(&mut headless, steps), 0);
    assert_eq!(var0(&headless), 1, "headless answers button 0");

    for (button, want) in [(0u8, 1), (1, 2)] {
        let mut w = World::from_save_seeded(&save, &data, 1234);
        w.set_interactive_choices(true);
        let left = run(&mut w, steps);
        assert!(left > 0, "the month start raises the choice");
        let id = w.pending_choice().unwrap().id;
        assert_eq!(w.pending_choice().unwrap().message.label2, "Two");
        w.advance(320);
        assert!(w.pending_choice().is_some(), "paused until answered");
        w.apply(Command::AnswerChoice {
            message_id: id.wrapping_add(7),
            button,
        });
        assert!(w.pending_choice().is_some(), "a stale id is ignored");
        w.apply(Command::AnswerChoice {
            message_id: id,
            button,
        });
        assert_eq!(run(&mut w, left), 0);
        assert_eq!(var0(&w), want);
        if button == 0 {
            assert_eq!(w.to_save(), headless.to_save(), "button 0 = headless run");
        }
    }
}
