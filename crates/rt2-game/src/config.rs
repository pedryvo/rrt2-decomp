//! `~/.config/rrt2/config.toml`: install path, UI scale and key bindings (docs/ui.md "Teclas").

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Key bindings: each action has a list of alternatives. A key is a winit `KeyCode` name
/// (`KeyQ`, `Space`, `Equal`, `F5`, `ArrowLeft`...; a bare letter works too) with an optional
/// `ctrl+` prefix. Physical keys, so they do not move with the keyboard layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keys {
    pub pause: Vec<String>,
    pub speed_up: Vec<String>,
    pub speed_down: Vec<String>,
    pub save: Vec<String>,
    pub menu: Vec<String>,
    pub rotate_left: Vec<String>,
    pub rotate_right: Vec<String>,
    pub reframe: Vec<String>,
    pub pan_left: Vec<String>,
    pub pan_right: Vec<String>,
    pub pan_up: Vec<String>,
    pub pan_down: Vec<String>,
    pub ui_scale_up: Vec<String>,
    pub ui_scale_down: Vec<String>,
    pub fullscreen: Vec<String>,
    pub mute: Vec<String>,
    pub next_track: Vec<String>,
    /// Information screens (docs/ui.md "Telas de informacao"); pressing the key of the open
    /// screen closes it.
    pub finance: Vec<String>,
    pub stock: Vec<String>,
    pub trains: Vec<String>,
    pub stations: Vec<String>,
    pub cities: Vec<String>,
    pub industries: Vec<String>,
}

fn v(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| (*k).to_owned()).collect()
}

impl Default for Keys {
    /// The original's keys where they are known (Pause toggles pause, `+`/`-` change the speed:
    /// the manual, "Game speed"); the rest follow common conventions and the viewer.
    fn default() -> Keys {
        Keys {
            pause: v(&["Pause", "Space"]),
            speed_up: v(&["Equal", "NumpadAdd"]),
            speed_down: v(&["Minus", "NumpadSubtract"]),
            save: v(&["F5"]),
            menu: v(&["Escape"]),
            rotate_left: v(&["KeyQ"]),
            rotate_right: v(&["KeyE"]),
            reframe: v(&["Home"]),
            pan_left: v(&["KeyA", "ArrowLeft"]),
            pan_right: v(&["KeyD", "ArrowRight"]),
            pan_up: v(&["KeyW", "ArrowUp"]),
            pan_down: v(&["KeyS", "ArrowDown"]),
            ui_scale_up: v(&["ctrl+Equal", "ctrl+NumpadAdd"]),
            ui_scale_down: v(&["ctrl+Minus", "ctrl+NumpadSubtract"]),
            fullscreen: v(&["F11"]),
            // Modern extras (the original has no audio hotkeys). Ctrl avoids menu letter hotkeys.
            mute: v(&["ctrl+KeyM"]),
            next_track: v(&["ctrl+KeyN"]),
            // Modern extras: the original opens these from the toolbar only.
            finance: v(&["KeyF"]),
            stock: v(&["KeyM"]),
            trains: v(&["KeyT"]),
            stations: v(&["KeyG"]),
            cities: v(&["KeyC"]),
            industries: v(&["KeyI"]),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The original game's install folder (read only).
    pub game_dir: Option<PathBuf>,
    /// UI scale in percent (100-300); absent = from the monitor DPI.
    pub ui_scale: Option<u32>,
    pub keys: Keys,
    /// UI language tag (`pt-BR`, `de`, `zh-CN`...; docs/i18n.md); absent = the system locale,
    /// else English.
    pub language: Option<String>,
    /// Volumes and mute (`docs/ui.md`, "Som").
    pub audio: crate::sound::AudioSettings,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config> {
        toml::from_str(text).context("invalid config.toml")
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config serialises")
    }

    pub fn load(path: &Path) -> Result<Config> {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::parse(&text).with_context(|| path.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e).with_context(|| path.display().to_string()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| dir.display().to_string())?;
        }
        std::fs::write(path, self.to_toml()).with_context(|| path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let c = Config::parse("game_dir = \"/games/rt2\"\n[keys]\npause = [\"KeyP\"]\n").unwrap();
        assert_eq!(c.game_dir, Some(PathBuf::from("/games/rt2")));
        assert_eq!(c.keys.pause, vec!["KeyP"]);
        assert_eq!(c.keys.save, Keys::default().save);
        assert_eq!(c.ui_scale, None);
    }

    #[test]
    fn round_trips_through_toml() {
        let c = Config {
            game_dir: Some("/x".into()),
            ui_scale: Some(150),
            ..Config::default()
        };
        assert_eq!(Config::parse(&c.to_toml()).unwrap(), c);
    }

    #[test]
    fn a_missing_file_is_the_default_config() {
        let c = Config::load(Path::new("/nonexistent/rrt2/config.toml")).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn bad_toml_is_an_error() {
        assert!(Config::parse("game_dir = [").is_err());
    }
}
