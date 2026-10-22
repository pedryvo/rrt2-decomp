//! Locating the original install's audio files. Read-only: nothing is written next to the game.

use std::path::{Path, PathBuf};

/// Archives in the game's precedence order (`docs/formats/pk2.md`, "Precedência": the patch with
/// the alphabetically larger name wins).
pub const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];

#[derive(Debug, Clone)]
pub struct GameFiles {
    dir: PathBuf,
}

impl GameFiles {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Install directory from the `RT2_DIR` environment variable.
    pub fn from_env() -> Option<Self> {
        std::env::var_os("RT2_DIR").map(Self::new)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Raw bytes of the archives that exist, in precedence order (`RT2.PK2` is required).
    pub fn read_archives(&self) -> std::io::Result<Vec<Vec<u8>>> {
        let mut out = Vec::new();
        for name in ARCHIVES {
            match std::fs::read(self.dir.join("data").join(name)) {
                Ok(bytes) => out.push(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && name != "RT2.PK2" => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// `data2` directory (music).
    pub fn music_dir(&self) -> PathBuf {
        self.dir.join("data2")
    }

    /// File names of the `*.mp3` files in `data2`, unsorted (the playlist sorts them).
    pub fn music_files(&self) -> std::io::Result<Vec<String>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.music_dir())? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if Path::new(&name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
            {
                out.push(name);
            }
        }
        Ok(out)
    }

    /// `anim` directory (Smacker videos).
    pub fn anim_dir(&self) -> PathBuf {
        self.dir.join("anim")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dir_is_an_error() {
        assert!(GameFiles::new("/nonexistent/rt2").read_archives().is_err());
        assert!(GameFiles::new("/nonexistent/rt2").music_files().is_err());
    }
}
