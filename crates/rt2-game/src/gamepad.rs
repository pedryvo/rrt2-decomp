//! Gamepad and Steam Deck controller support (docs/player-guide.md "Gamepad & Steam Deck Controls").
//!
//! Provides smooth camera panning (Left Stick), virtual cursor with adaptive positioning (Right Stick),
//! action buttons (A/B/X/Y for select, cancel, track and station tools), bumpers for map rotation
//! (L1/R1), triggers for simulation speed adjustment (L2/R2), and D-pad navigation.
//!
//! On Linux / SteamOS, connects to the system joystick interface (`/dev/input/js*`) non-blockingly.

use std::collections::VecDeque;

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

/// Standard controller buttons mapped according to the Xbox / Steam Deck layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    /// A / Cross: Select, primary click.
    South,
    /// B / Circle: Cancel, Back, close dialog.
    East,
    /// X / Square: Select Track Tool.
    West,
    /// Y / Triangle: Select Station Tool.
    North,
    /// L1: Rotate camera counter-clockwise.
    LeftBumper,
    /// R1: Rotate camera clockwise.
    RightBumper,
    /// L2: Decrease simulation speed.
    LeftTrigger,
    /// R2: Increase simulation speed.
    RightTrigger,
    /// Select / View / Back: Toggle pause.
    Select,
    /// Start / Options: Open options menu.
    Start,
    /// D-pad directions.
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftThumb,
    RightThumb,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct JsEvent {
    time: u32,
    value: i16,
    event_type: u8,
    number: u8,
}

const JS_EVENT_BUTTON: u8 = 0x01;
const JS_EVENT_AXIS: u8 = 0x02;
const JS_EVENT_INIT: u8 = 0x80;

/// Deadzone threshold for analog sticks (-1.0..1.0).
const DEADZONE: f32 = 0.15;

/// Virtual cursor speed in logical units per second.
pub const CURSOR_SPEED: f32 = 600.0;

/// State of connected controller and virtual cursor.
pub struct GamepadState {
    /// Virtual cursor position in logical screen coordinates `(x, y)`.
    pub cursor: Option<(f32, f32)>,
    /// Left analog stick `(x, y)` in range `-1.0..=1.0`.
    pub left_stick: (f32, f32),
    /// Right analog stick `(x, y)` in range `-1.0..=1.0`.
    pub right_stick: (f32, f32),
    /// Queued button events `(button, pressed)`.
    pub button_events: VecDeque<(GamepadButton, bool)>,
    /// Button states.
    pub buttons: [bool; 16],
    #[cfg(target_os = "linux")]
    device: Option<File>,
}

impl Default for GamepadState {
    fn default() -> Self {
        Self::new()
    }
}

impl GamepadState {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        let device = Self::open_linux_device();
        GamepadState {
            cursor: None,
            left_stick: (0.0, 0.0),
            right_stick: (0.0, 0.0),
            button_events: VecDeque::new(),
            buttons: [false; 16],
            #[cfg(target_os = "linux")]
            device,
        }
    }

    #[cfg(target_os = "linux")]
    fn open_linux_device() -> Option<File> {
        let dev_path =
            std::env::var("RRT2_JOYSTICK").unwrap_or_else(|_| "/dev/input/js0".to_owned());
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x800) // O_NONBLOCK
            .open(&dev_path)
            .ok()
    }

    /// Reads pending OS joystick events.
    pub fn poll_os(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let Some(mut dev) = self.device.take() else {
                return;
            };
            let mut buf = [0u8; 8 * 16];
            let mut keep = true;
            loop {
                match dev.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let count = n / 8;
                        for i in 0..count {
                            let raw = &buf[i * 8..(i + 1) * 8];
                            let ev = JsEvent {
                                time: u32::from_ne_bytes(raw[0..4].try_into().unwrap()),
                                value: i16::from_ne_bytes(raw[4..6].try_into().unwrap()),
                                event_type: raw[6],
                                number: raw[7],
                            };
                            self.process_raw_event(ev);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        keep = false;
                        break;
                    }
                }
            }
            if keep {
                self.device = Some(dev);
            }
        }
    }

    fn process_raw_event(&mut self, ev: JsEvent) {
        let is_init = (ev.event_type & JS_EVENT_INIT) != 0;
        let event_type = ev.event_type & !JS_EVENT_INIT;

        if event_type == JS_EVENT_AXIS {
            let norm = (ev.value as f32 / 32767.0).clamp(-1.0, 1.0);
            let val = if norm.abs() < DEADZONE { 0.0 } else { norm };
            match ev.number {
                0 => self.left_stick.0 = val,
                1 => self.left_stick.1 = val,
                2 => {
                    // Left trigger (L2) axis or button
                    let pressed = norm > 0.5;
                    self.set_button(GamepadButton::LeftTrigger, pressed && !is_init);
                }
                3 => self.right_stick.0 = val,
                4 => self.right_stick.1 = val,
                5 => {
                    // Right trigger (R2) axis or button
                    let pressed = norm > 0.5;
                    self.set_button(GamepadButton::RightTrigger, pressed && !is_init);
                }
                6 => {
                    // D-Pad X
                    self.set_button(GamepadButton::DpadLeft, ev.value < -16000);
                    self.set_button(GamepadButton::DpadRight, ev.value > 16000);
                }
                7 => {
                    // D-Pad Y
                    self.set_button(GamepadButton::DpadUp, ev.value < -16000);
                    self.set_button(GamepadButton::DpadDown, ev.value > 16000);
                }
                _ => {}
            }
        } else if event_type == JS_EVENT_BUTTON {
            let pressed = ev.value != 0;
            let btn = match ev.number {
                0 => Some(GamepadButton::South),
                1 => Some(GamepadButton::East),
                2 => Some(GamepadButton::West),
                3 => Some(GamepadButton::North),
                4 => Some(GamepadButton::LeftBumper),
                5 => Some(GamepadButton::RightBumper),
                6 => Some(GamepadButton::Select),
                7 => Some(GamepadButton::Start),
                9 => Some(GamepadButton::LeftThumb),
                10 => Some(GamepadButton::RightThumb),
                11 => Some(GamepadButton::DpadUp),
                12 => Some(GamepadButton::DpadDown),
                13 => Some(GamepadButton::DpadLeft),
                14 => Some(GamepadButton::DpadRight),
                _ => None,
            };
            if let Some(b) = btn
                && !is_init
            {
                self.set_button(b, pressed);
            }
        }
    }

    /// Sets button state and enqueues change event.
    pub fn set_button(&mut self, btn: GamepadButton, pressed: bool) {
        let idx = btn as usize;
        if let Some(slot) = self.buttons.get_mut(idx)
            && *slot != pressed
        {
            *slot = pressed;
            self.button_events.push_back((btn, pressed));
        }
    }

    pub fn pop_button_event(&mut self) -> Option<(GamepadButton, bool)> {
        self.button_events.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepad_defaults_and_button_queue() {
        let mut gp = GamepadState {
            cursor: None,
            left_stick: (0.0, 0.0),
            right_stick: (0.0, 0.0),
            button_events: VecDeque::new(),
            buttons: [false; 16],
            #[cfg(target_os = "linux")]
            device: None,
        };

        assert!(gp.cursor.is_none());
        assert_eq!(gp.left_stick, (0.0, 0.0));
        assert_eq!(gp.right_stick, (0.0, 0.0));
        assert!(gp.pop_button_event().is_none());

        // Press South
        gp.set_button(GamepadButton::South, true);
        assert!(gp.buttons[GamepadButton::South as usize]);
        assert_eq!(gp.pop_button_event(), Some((GamepadButton::South, true)));
        // Setting to same state does not enqueue duplicate
        gp.set_button(GamepadButton::South, true);
        assert!(gp.pop_button_event().is_none());

        // Release South
        gp.set_button(GamepadButton::South, false);
        assert!(!gp.buttons[GamepadButton::South as usize]);
        assert_eq!(gp.pop_button_event(), Some((GamepadButton::South, false)));
    }

    #[test]
    fn gamepad_deadzone_and_axes() {
        let mut gp = GamepadState {
            cursor: None,
            left_stick: (0.0, 0.0),
            right_stick: (0.0, 0.0),
            button_events: VecDeque::new(),
            buttons: [false; 16],
            #[cfg(target_os = "linux")]
            device: None,
        };

        // Small value inside deadzone (15%) -> 0.0
        let ev_small = JsEvent {
            time: 10,
            value: 3000, // ~0.09
            event_type: JS_EVENT_AXIS,
            number: 0,
        };
        gp.process_raw_event(ev_small);
        assert_eq!(gp.left_stick.0, 0.0);

        // Value outside deadzone (e.g. 50%)
        let ev_large = JsEvent {
            time: 20,
            value: 16384, // ~0.5
            event_type: JS_EVENT_AXIS,
            number: 0,
        };
        gp.process_raw_event(ev_large);
        assert!(gp.left_stick.0 > 0.49 && gp.left_stick.0 < 0.51);

        // Right stick Y axis (axis 4)
        let ev_ry = JsEvent {
            time: 30,
            value: -32767,
            event_type: JS_EVENT_AXIS,
            number: 4,
        };
        gp.process_raw_event(ev_ry);
        assert_eq!(gp.right_stick.1, -1.0);
    }

    #[test]
    fn gamepad_buttons_and_triggers_via_events() {
        let mut gp = GamepadState {
            cursor: None,
            left_stick: (0.0, 0.0),
            right_stick: (0.0, 0.0),
            button_events: VecDeque::new(),
            buttons: [false; 16],
            #[cfg(target_os = "linux")]
            device: None,
        };

        // Button 0 -> South
        let ev_btn = JsEvent {
            time: 50,
            value: 1,
            event_type: JS_EVENT_BUTTON,
            number: 0,
        };
        gp.process_raw_event(ev_btn);
        assert_eq!(gp.pop_button_event(), Some((GamepadButton::South, true)));

        // Axis 2 (L2 trigger) -> LeftTrigger button
        let ev_trig = JsEvent {
            time: 60,
            value: 25000, // > 0.5
            event_type: JS_EVENT_AXIS,
            number: 2,
        };
        gp.process_raw_event(ev_trig);
        assert_eq!(
            gp.pop_button_event(),
            Some((GamepadButton::LeftTrigger, true))
        );
    }
}
