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
        "NumpadAdd" => NumpadAdd,
        "NumpadSubtract" => NumpadSubtract,
        "ArrowLeft" | "Left" => ArrowLeft,
        "ArrowRight" | "Right" => ArrowRight,
        "ArrowUp" | "Up" => ArrowUp,
        "ArrowDown" | "Down" => ArrowDown,
        _ => return None,
    })
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
