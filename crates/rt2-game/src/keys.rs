//! Key binding parsing and lookup.

use winit::keyboard::KeyCode;

use crate::config::Keys;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeySpec {
    pub ctrl: bool,
    pub code: KeyCode,
}

pub fn parse_code(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    let n = name.trim();
    let letters = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    let fkeys = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    let digits = [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ];
    if n.len() == 1 {
        let c = n.as_bytes()[0].to_ascii_uppercase();
        if c.is_ascii_uppercase() {
            return Some(letters[usize::from(c - b'A')]);
        }
        if c.is_ascii_digit() {
            return Some(digits[usize::from(c - b'0')]);
        }
    }
    if let Some(rest) = n.strip_prefix("Key")
        && rest.len() == 1
    {
        return parse_code(rest);
    }
    if let Some(rest) = n.strip_prefix("Digit")
        && rest.len() == 1
    {
        return parse_code(rest);
    }
    if let Some(num) = n.strip_prefix('F').and_then(|r| r.parse::<usize>().ok())
        && (1..=12).contains(&num)
    {
        return Some(fkeys[num - 1]);
    }
    Some(match n {
        "Space" => Space,
        "Escape" | "Esc" => Escape,
        "Enter" | "Return" => Enter,
        "Tab" => Tab,
        "Backspace" => Backspace,
        "Delete" => Delete,
        "Insert" => Insert,
        "Home" => Home,
        "End" => End,
        "PageUp" => PageUp,
        "PageDown" => PageDown,
        "Pause" => Pause,
        "Equal" => Equal,
        "Minus" => Minus,
        "Comma" => Comma,
        "Period" => Period,
        "Slash" => Slash,
        "Backslash" => Backslash,
        "Semicolon" => Semicolon,
        "Quote" => Quote,
        "Backquote" => Backquote,
        "BracketLeft" => BracketLeft,
        "BracketRight" => BracketRight,
        "NumpadAdd" => NumpadAdd,
        "NumpadSubtract" => NumpadSubtract,
        "NumpadMultiply" => NumpadMultiply,
        "NumpadDivide" => NumpadDivide,
        "Numpad0" => Numpad0,
        "Numpad1" => Numpad1,
        "Numpad2" => Numpad2,
        "Numpad3" => Numpad3,
        "Numpad4" => Numpad4,
        "Numpad5" => Numpad5,
        "Numpad6" => Numpad6,
        "Numpad7" => Numpad7,
        "Numpad8" => Numpad8,
        "Numpad9" => Numpad9,
        "ArrowLeft" | "Left" => ArrowLeft,
        "ArrowRight" | "Right" => ArrowRight,
        "ArrowUp" | "Up" => ArrowUp,
        "ArrowDown" | "Down" => ArrowDown,
        _ => return None,
    })
}

/// Names `parse_code` accepts, in canonical form: the inverse table of [`code_name`].
const NAMED: &[&str] = &[
    "Space",
    "Escape",
    "Enter",
    "Tab",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Pause",
    "Equal",
    "Minus",
    "Comma",
    "Period",
    "Slash",
    "Backslash",
    "Semicolon",
    "Quote",
    "Backquote",
    "BracketLeft",
    "BracketRight",
    "NumpadAdd",
    "NumpadSubtract",
    "NumpadMultiply",
    "NumpadDivide",
    "Numpad0",
    "Numpad1",
    "Numpad2",
    "Numpad3",
    "Numpad4",
    "Numpad5",
    "Numpad6",
    "Numpad7",
    "Numpad8",
    "Numpad9",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "ArrowDown",
];

/// The `config.toml` name of a key, if it can be bound (modifiers alone cannot).
pub fn code_name(code: KeyCode) -> Option<String> {
    let letters = ('A'..='Z').map(|c| format!("Key{c}"));
    let digits = (0..=9).map(|d| format!("Digit{d}"));
    let fkeys = (1..=12).map(|n| format!("F{n}"));
    let named = NAMED.iter().map(|n| (*n).to_owned());
    letters
        .chain(digits)
        .chain(fkeys)
        .chain(named)
        .find(|n| parse_code(n) == Some(code))
}

/// The spec string of a pressed key (`ctrl+KeyS`), or `None` for a key that cannot be bound.
pub fn spec_string(code: KeyCode, ctrl: bool) -> Option<String> {
    let n = code_name(code)?;
    Some(if ctrl { format!("ctrl+{n}") } else { n })
}

/// A key spec as a player reads it: `ctrl+KeyS` -> `Ctrl+S`, `ArrowLeft` -> `Left`.
pub fn display_spec(spec: &str) -> String {
    let lower = spec.to_ascii_lowercase();
    let (ctrl, name) = if lower.starts_with("ctrl+") {
        (true, &spec[5..])
    } else {
        (false, spec)
    };
    let nice = if let Some(l) = name.strip_prefix("Key").filter(|l| l.len() == 1) {
        l.to_owned()
    } else if let Some(d) = name.strip_prefix("Digit").filter(|d| d.len() == 1) {
        d.to_owned()
    } else if let Some(d) = name.strip_prefix("Numpad") {
        match d {
            "Add" => "Num +".to_owned(),
            "Subtract" => "Num -".to_owned(),
            "Multiply" => "Num *".to_owned(),
            "Divide" => "Num /".to_owned(),
            d => format!("Num {d}"),
        }
    } else {
        match name {
            "Equal" => "=",
            "Minus" => "-",
            "Comma" => ",",
            "Period" => ".",
            "Slash" => "/",
            "Backslash" => "\\",
            "Semicolon" => ";",
            "Quote" => "'",
            "Backquote" => "`",
            "BracketLeft" => "[",
            "BracketRight" => "]",
            "ArrowLeft" => "Left",
            "ArrowRight" => "Right",
            "ArrowUp" => "Up",
            "ArrowDown" => "Down",
            n => n,
        }
        .to_owned()
    };
    if ctrl { format!("Ctrl+{nice}") } else { nice }
}

/// Everything a key can be bound to, in the order the options screen lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Pause,
    SpeedUp,
    SpeedDown,
    QuickSave,
    QuickLoad,
    SaveDialog,
    LoadDialog,
    Options,
    Menu,
    RotateLeft,
    RotateRight,
    Reframe,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    UiScaleUp,
    UiScaleDown,
    Fullscreen,
    Mute,
    NextTrack,
    Finance,
    Stock,
    Trains,
    Stations,
    Cities,
    Industries,
}

impl KeyAction {
    pub const ALL: [KeyAction; 27] = [
        KeyAction::Pause,
        KeyAction::SpeedUp,
        KeyAction::SpeedDown,
        KeyAction::QuickSave,
        KeyAction::QuickLoad,
        KeyAction::SaveDialog,
        KeyAction::LoadDialog,
        KeyAction::Options,
        KeyAction::Menu,
        KeyAction::RotateLeft,
        KeyAction::RotateRight,
        KeyAction::Reframe,
        KeyAction::PanLeft,
        KeyAction::PanRight,
        KeyAction::PanUp,
        KeyAction::PanDown,
        KeyAction::UiScaleUp,
        KeyAction::UiScaleDown,
        KeyAction::Fullscreen,
        KeyAction::Mute,
        KeyAction::NextTrack,
        KeyAction::Finance,
        KeyAction::Stock,
        KeyAction::Trains,
        KeyAction::Stations,
        KeyAction::Cities,
        KeyAction::Industries,
    ];

    /// The English name (an `app_strings` entry; see the test).
    pub fn english(self) -> &'static str {
        match self {
            KeyAction::Pause => "Pause / resume",
            KeyAction::SpeedUp => "Faster",
            KeyAction::SpeedDown => "Slower",
            KeyAction::QuickSave => "Quick save",
            KeyAction::QuickLoad => "Quick load",
            KeyAction::SaveDialog => "Save game dialog",
            KeyAction::LoadDialog => "Load game dialog",
            KeyAction::Options => "Options",
            KeyAction::Menu => "File options",
            KeyAction::RotateLeft => "Rotate view left",
            KeyAction::RotateRight => "Rotate view right",
            KeyAction::Reframe => "Fit the whole map",
            KeyAction::PanLeft => "Pan left",
            KeyAction::PanRight => "Pan right",
            KeyAction::PanUp => "Pan up",
            KeyAction::PanDown => "Pan down",
            KeyAction::UiScaleUp => "Larger interface",
            KeyAction::UiScaleDown => "Smaller interface",
            KeyAction::Fullscreen => "Toggle fullscreen",
            KeyAction::Mute => "Mute sound",
            KeyAction::NextTrack => "Next music track",
            KeyAction::Finance => "Finance screen",
            KeyAction::Stock => "Stock market screen",
            KeyAction::Trains => "Trains screen",
            KeyAction::Stations => "Stations screen",
            KeyAction::Cities => "Cities screen",
            KeyAction::Industries => "Industries screen",
        }
    }
}

/// `$a` is the action, `$k` the `Keys`, `$r` the reference form (`&` or `&mut`).
macro_rules! slot {
    ($a:expr, $k:expr, $($r:tt)+) => {
        match $a {
            KeyAction::Pause => $($r)+ $k.pause,
            KeyAction::SpeedUp => $($r)+ $k.speed_up,
            KeyAction::SpeedDown => $($r)+ $k.speed_down,
            KeyAction::QuickSave => $($r)+ $k.save,
            KeyAction::QuickLoad => $($r)+ $k.quick_load,
            KeyAction::SaveDialog => $($r)+ $k.save_dialog,
            KeyAction::LoadDialog => $($r)+ $k.load_dialog,
            KeyAction::Options => $($r)+ $k.options,
            KeyAction::Menu => $($r)+ $k.menu,
            KeyAction::RotateLeft => $($r)+ $k.rotate_left,
            KeyAction::RotateRight => $($r)+ $k.rotate_right,
            KeyAction::Reframe => $($r)+ $k.reframe,
            KeyAction::PanLeft => $($r)+ $k.pan_left,
            KeyAction::PanRight => $($r)+ $k.pan_right,
            KeyAction::PanUp => $($r)+ $k.pan_up,
            KeyAction::PanDown => $($r)+ $k.pan_down,
            KeyAction::UiScaleUp => $($r)+ $k.ui_scale_up,
            KeyAction::UiScaleDown => $($r)+ $k.ui_scale_down,
            KeyAction::Fullscreen => $($r)+ $k.fullscreen,
            KeyAction::Mute => $($r)+ $k.mute,
            KeyAction::NextTrack => $($r)+ $k.next_track,
            KeyAction::Finance => $($r)+ $k.finance,
            KeyAction::Stock => $($r)+ $k.stock,
            KeyAction::Trains => $($r)+ $k.trains,
            KeyAction::Stations => $($r)+ $k.stations,
            KeyAction::Cities => $($r)+ $k.cities,
            KeyAction::Industries => $($r)+ $k.industries,
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindMode {
    Replace,
    Add,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rebind {
    Done,
    /// The key is used by these other actions.
    Conflict(Vec<KeyAction>),
    Invalid,
}

impl Keys {
    /// The key specs bound to an action, as written in `config.toml`.
    pub fn get(&self, a: KeyAction) -> &[String] {
        slot!(a, self, &)
    }

    fn get_mut(&mut self, a: KeyAction) -> &mut Vec<String> {
        slot!(a, self, &mut)
    }

    /// The other actions that already use `spec` (compared as parsed keys, so `q` and `KeyQ` are
    /// the same key).
    pub fn conflicts(&self, spec: &str, except: KeyAction) -> Vec<KeyAction> {
        let Some(want) = parse_spec(spec) else {
            return Vec::new();
        };
        KeyAction::ALL
            .into_iter()
            .filter(|&a| a != except)
            .filter(|&a| self.get(a).iter().any(|s| parse_spec(s) == Some(want)))
            .collect()
    }

    /// Binds `spec` to `action`: replaces the action's keys (`BindMode::Replace`) or adds one
    /// (`BindMode::Add`). A key another action uses is refused with the list of those actions
    /// unless `steal`, which takes it from them.
    pub fn rebind(&mut self, action: KeyAction, spec: &str, mode: BindMode, steal: bool) -> Rebind {
        let Some(want) = parse_spec(spec) else {
            return Rebind::Invalid;
        };
        let clash = self.conflicts(spec, action);
        if !clash.is_empty() && !steal {
            return Rebind::Conflict(clash);
        }
        for a in &clash {
            self.get_mut(*a).retain(|s| parse_spec(s) != Some(want));
        }
        let slot = self.get_mut(action);
        if mode == BindMode::Replace {
            slot.clear();
        }
        if !slot.iter().any(|s| parse_spec(s) == Some(want)) {
            slot.push(spec.to_owned());
        }
        Rebind::Done
    }

    /// Back to the shipped keys for one action. A default another action now uses is taken back
    /// from it, so a reset always restores the shipped binding.
    pub fn reset(&mut self, action: KeyAction) {
        let keys = Keys::default().get(action).to_vec();
        for k in &keys {
            let Some(want) = parse_spec(k) else { continue };
            for a in self.conflicts(k, action) {
                self.get_mut(a).retain(|s| parse_spec(s) != Some(want));
            }
        }
        *self.get_mut(action) = keys;
    }
}

pub fn parse_spec(s: &str) -> Option<KeySpec> {
    let lower = s.to_ascii_lowercase();
    match lower.strip_prefix("ctrl+") {
        Some(_) => Some(KeySpec {
            ctrl: true,
            code: parse_code(&s[5..])?,
        }),
        None => Some(KeySpec {
            ctrl: false,
            code: parse_code(s)?,
        }),
    }
}

/// An action's parsed alternatives; unknown names are dropped (and reported once at start-up).
#[derive(Clone, Debug, Default)]
pub struct Binding(Vec<KeySpec>);

impl Binding {
    fn from(names: &[String], bad: &mut Vec<String>) -> Binding {
        let mut out = Vec::new();
        for n in names {
            match parse_spec(n) {
                Some(k) => out.push(k),
                None => bad.push(n.clone()),
            }
        }
        Binding(out)
    }

    pub fn matches(&self, code: KeyCode, ctrl: bool) -> bool {
        self.0.iter().any(|k| k.code == code && k.ctrl == ctrl)
    }

    /// Held-key query (panning): ctrl is ignored.
    pub fn held(&self, down: &std::collections::BTreeSet<KeyCode>) -> bool {
        self.0.iter().any(|k| down.contains(&k.code))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Bindings {
    pub pause: Binding,
    pub speed_up: Binding,
    pub speed_down: Binding,
    pub save: Binding,
    pub menu: Binding,
    pub rotate_left: Binding,
    pub rotate_right: Binding,
    pub reframe: Binding,
    pub pan_left: Binding,
    pub pan_right: Binding,
    pub pan_up: Binding,
    pub pan_down: Binding,
    pub ui_scale_up: Binding,
    pub ui_scale_down: Binding,
    pub fullscreen: Binding,
    pub mute: Binding,
    pub next_track: Binding,
    pub quick_load: Binding,
    pub save_dialog: Binding,
    pub load_dialog: Binding,
    pub options: Binding,
    pub finance: Binding,
    pub stock: Binding,
    pub trains: Binding,
    pub stations: Binding,
    pub cities: Binding,
    pub industries: Binding,
    /// Names in the config that were not recognised.
    pub invalid: Vec<String>,
}

impl Bindings {
    pub fn from_config(k: &Keys) -> Bindings {
        let mut bad = Vec::new();
        let mut b = |n: &[String]| Binding::from(n, &mut bad);
        let out = Bindings {
            pause: b(&k.pause),
            speed_up: b(&k.speed_up),
            speed_down: b(&k.speed_down),
            save: b(&k.save),
            menu: b(&k.menu),
            rotate_left: b(&k.rotate_left),
            rotate_right: b(&k.rotate_right),
            reframe: b(&k.reframe),
            pan_left: b(&k.pan_left),
            pan_right: b(&k.pan_right),
            pan_up: b(&k.pan_up),
            pan_down: b(&k.pan_down),
            ui_scale_up: b(&k.ui_scale_up),
            ui_scale_down: b(&k.ui_scale_down),
            fullscreen: b(&k.fullscreen),
            mute: b(&k.mute),
            next_track: b(&k.next_track),
            quick_load: b(&k.quick_load),
            save_dialog: b(&k.save_dialog),
            load_dialog: b(&k.load_dialog),
            options: b(&k.options),
            finance: b(&k.finance),
            stock: b(&k.stock),
            trains: b(&k.trains),
            stations: b(&k.stations),
            cities: b(&k.cities),
            industries: b(&k.industries),
            invalid: Vec::new(),
        };
        Bindings {
            invalid: bad,
            ..out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse() {
        assert_eq!(parse_code("KeyQ"), Some(KeyCode::KeyQ));
        assert_eq!(parse_code("q"), Some(KeyCode::KeyQ));
        assert_eq!(parse_code("F5"), Some(KeyCode::F5));
        assert_eq!(parse_code("Pause"), Some(KeyCode::Pause));
        assert_eq!(parse_code("7"), Some(KeyCode::Digit7));
        assert_eq!(parse_code("Nonsense"), None);
        assert_eq!(
            parse_spec("ctrl+Equal"),
            Some(KeySpec {
                ctrl: true,
                code: KeyCode::Equal
            })
        );
    }

    #[test]
    fn defaults_are_all_valid_and_distinct_from_ctrl_variants() {
        let b = Bindings::from_config(&Keys::default());
        assert!(b.invalid.is_empty(), "{:?}", b.invalid);
        assert!(b.speed_up.matches(KeyCode::Equal, false));
        assert!(!b.speed_up.matches(KeyCode::Equal, true));
        assert!(b.ui_scale_up.matches(KeyCode::Equal, true));
    }

    #[test]
    fn every_bindable_key_has_a_name_that_parses_back() {
        for code in [
            KeyCode::KeyQ,
            KeyCode::Digit3,
            KeyCode::F9,
            KeyCode::Space,
            KeyCode::ArrowUp,
            KeyCode::NumpadAdd,
            KeyCode::Slash,
        ] {
            let n = code_name(code).unwrap();
            assert_eq!(parse_code(&n), Some(code), "{n}");
        }
        assert_eq!(code_name(KeyCode::ShiftLeft), None);
        assert_eq!(
            spec_string(KeyCode::KeyS, true).as_deref(),
            Some("ctrl+KeyS")
        );
        assert_eq!(display_spec("ctrl+KeyS"), "Ctrl+S");
        assert_eq!(display_spec("ArrowLeft"), "Left");
    }

    #[test]
    fn rebinding_to_a_used_key_reports_the_conflict_and_changes_nothing() {
        let mut k = Keys::default();
        let before = k.clone();
        // Q rotates left by default.
        let r = k.rebind(KeyAction::Pause, "KeyQ", BindMode::Replace, false);
        assert_eq!(r, Rebind::Conflict(vec![KeyAction::RotateLeft]));
        assert_eq!(k, before);
        // The same key written as a bare letter is the same key.
        let r = k.rebind(KeyAction::Pause, "q", BindMode::Add, false);
        assert_eq!(r, Rebind::Conflict(vec![KeyAction::RotateLeft]));
    }

    #[test]
    fn ctrl_makes_a_different_key() {
        let mut k = Keys::default();
        assert!(k.conflicts("ctrl+KeyQ", KeyAction::Pause).is_empty());
        assert_eq!(
            k.rebind(KeyAction::Pause, "ctrl+KeyQ", BindMode::Replace, false),
            Rebind::Done
        );
        assert_eq!(k.get(KeyAction::Pause), ["ctrl+KeyQ"]);
    }

    #[test]
    fn stealing_moves_the_key_and_add_keeps_the_old_ones() {
        let mut k = Keys::default();
        assert_eq!(
            k.rebind(KeyAction::Pause, "KeyQ", BindMode::Add, true),
            Rebind::Done
        );
        assert_eq!(k.get(KeyAction::Pause), ["Pause", "Space", "KeyQ"]);
        assert!(k.get(KeyAction::RotateLeft).is_empty());
        // A key the action already has is not a conflict and is not added twice.
        assert_eq!(
            k.rebind(KeyAction::Pause, "Space", BindMode::Add, false),
            Rebind::Done
        );
        assert_eq!(k.get(KeyAction::Pause).len(), 3);
        assert_eq!(
            k.rebind(KeyAction::Pause, "Bogus", BindMode::Replace, false),
            Rebind::Invalid
        );
    }

    #[test]
    fn reset_restores_the_default_even_if_another_action_took_the_key() {
        let mut k = Keys::default();
        k.rebind(KeyAction::Pause, "KeyQ", BindMode::Replace, true);
        k.reset(KeyAction::RotateLeft);
        assert_eq!(k.get(KeyAction::RotateLeft), ["KeyQ"]);
        assert!(k.get(KeyAction::Pause).is_empty());
        k.reset(KeyAction::Pause);
        assert_eq!(k, Keys::default());
    }

    #[test]
    fn the_defaults_do_not_clash_with_each_other() {
        let k = Keys::default();
        for a in KeyAction::ALL {
            for s in k.get(a) {
                assert!(k.conflicts(s, a).is_empty(), "{s} of {a:?}");
            }
        }
    }

    #[test]
    fn invalid_names_are_reported() {
        let k = Keys {
            pause: vec!["Bogus".into(), "KeyP".into()],
            ..Keys::default()
        };
        let b = Bindings::from_config(&k);
        assert_eq!(b.invalid, vec!["Bogus"]);
        assert!(b.pause.matches(KeyCode::KeyP, false));
    }
}
