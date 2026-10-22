//! Where things live: the original install (read only), our config and our saves.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::config::Config;

/// `$RRT2_CONFIG` or `<config dir>/rrt2/config.toml` (`~/.config/rrt2/config.toml` on Linux).
pub fn config_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("RRT2_CONFIG") {
        return Some(PathBuf::from(p));
    }
    dirs::config_dir().map(|d| d.join("rrt2").join("config.toml"))
}

/// Our own saves: `$RRT2_SAVES` or `<data dir>/rrt2/saves` (`~/.local/share/rrt2/saves`). Never
/// inside the install.
pub fn saves_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("RRT2_SAVES") {
        return Some(PathBuf::from(p));
    }
    dirs::data_dir().map(|d| d.join("rrt2").join("saves"))
}

/// A folder is an install if it has the base archive and the executable.
pub fn validate_install(dir: &Path) -> Result<()> {
    for need in ["data/RT2.PK2", "RT2_PLAT.EXE"] {
        if !dir.join(need).is_file() {
            bail!(
                "{} does not look like a Railroad Tycoon II install (missing {need})",
                dir.display()
            );
        }
    }
    Ok(())
}

/// The install to use: `--game-dir` / `RT2_DIR` (clap resolves both into `cli`), then the config.
pub fn resolve_install(cli: Option<PathBuf>, config: &Config) -> Result<PathBuf> {
    let Some(dir) = cli.or_else(|| config.game_dir.clone()) else {
        bail!(
            "no Railroad Tycoon II install configured.\n\
             Run `rrt2 set-path <folder>` once, or pass --game-dir <folder>, or set RT2_DIR."
        );
    };
    validate_install(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_path_gives_the_set_path_hint() {
        let e = resolve_install(None, &Config::default()).unwrap_err();
        assert!(e.to_string().contains("rrt2 set-path"));
    }

    #[test]
    fn cli_wins_over_config_and_is_validated() {
        let cfg = Config {
            game_dir: Some("/from/config".into()),
            ..Config::default()
        };
        let e = resolve_install(Some("/from/cli".into()), &cfg).unwrap_err();
        assert!(e.to_string().contains("/from/cli"), "{e}");
    }

    #[test]
    fn a_folder_with_the_archive_and_exe_is_valid() {
        let dir = std::env::temp_dir().join(format!("rrt2-install-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("data")).unwrap();
        assert!(validate_install(&dir).is_err());
        std::fs::write(dir.join("data/RT2.PK2"), b"x").unwrap();
        std::fs::write(dir.join("RT2_PLAT.EXE"), b"x").unwrap();
        assert!(validate_install(&dir).is_ok());
        assert_eq!(
            resolve_install(
                None,
                &Config {
                    game_dir: Some(dir.clone()),
                    ..Config::default()
                }
            )
            .unwrap(),
            dir
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
