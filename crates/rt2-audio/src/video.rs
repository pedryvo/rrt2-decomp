//! Smacker (`anim\*.SMK`) support: the clone converts videos with the `ffmpeg` command line tool
//! into a cache directory the first time they are needed, then plays the cached MP4 with whatever
//! video player the front end has. Nothing is written next to the original install.
//!
//! Format notes: 640x480 `smackvideo` at 12 fps plus 22050 Hz mono `smackaudio` (`INTRO_01.SMK`
//! is 87 s); the 104 files in `anim` total about 218 MB, which the cache turns into
//! H.264/AAC. Which screen plays which clip is documented in `docs/formats/sound.md`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::AudioError;

/// Names of the videos in the order of the game's table (`0x55fed8`, 9-byte records, index 1..=104;
/// index 0 is unused). `play_video(n)` is `FUN_005043c0` and opens `anim\<name>.SMK`, first in the
/// install folder, then on the CD. Groups: `br_*` bridge/opening scenes, `scenwin`/`scenlose`,
/// `intro_01`, `cv01..18`/`cvwin` and `cve01..18`/`cvewin` (full-screen clips, American and
/// European campaigns), `ca*`/`cae*` and the `cabrok`, `calook`, `caloss`, `casnor`, `cawait`
/// families (advisor/character animations shown in a window).
pub fn animation_name(index: usize) -> Option<String> {
    const GROUPS: [(&str, usize, usize); 4] =
        [("br_", 1, 9), ("cv", 1, 18), ("ca", 1, 18), ("cae", 1, 18)];
    let numbered = |prefix: &str, n: usize| format!("{prefix}{n:02}");
    let mut names: Vec<String> = Vec::with_capacity(105);
    names.push(String::new());
    names.extend((1..=GROUPS[0].2).map(|n| numbered(GROUPS[0].0, n)));
    names.extend(["scenwin", "scenlose", "intro_01"].map(String::from));
    names.extend((1..=GROUPS[1].2).map(|n| numbered(GROUPS[1].0, n)));
    names.push("cvwin".into());
    names.extend((1..=GROUPS[2].2).map(|n| numbered(GROUPS[2].0, n)));
    names.extend((1..=6).map(|n| numbered("cabrok", n)));
    names.extend(
        [
            "cagetst2", "calookle", "calookri", "caloss_1", "caloss_2", "casnorea", "casnores",
            "caepopto",
        ]
        .map(String::from),
    );
    names.extend((1..=18).map(|n| numbered("cve", n)));
    names.push("cvewin".into());
    names.extend((1..=GROUPS[3].2).map(|n| numbered(GROUPS[3].0, n)));
    names.extend((1..=4).map(|n| format!("cawait{n}")));
    names.into_iter().nth(index).filter(|n| !n.is_empty())
}

/// Default cache directory: `$XDG_CACHE_HOME/rrt2/video`, else `~/.cache/rrt2/video`
/// (`%LOCALAPPDATA%\rrt2\video` on Windows).
pub fn default_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("rrt2").join("video")
}

/// Where the converted copy of `smk` lives inside `cache` (lower-case stem, `.mp4`).
pub fn cache_path(cache: &Path, smk: &Path) -> PathBuf {
    let stem = smk
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    cache.join(format!("{stem}.mp4"))
}

/// `true` if `ffmpeg` runs.
pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// All `*.smk` files in `anim_dir`, sorted by lower-case name.
pub fn list_animations(anim_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(anim_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("smk")))
        .collect();
    out.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    Ok(out)
}

fn run_ffmpeg(input: &Path, output: &Path, video_codec: &[&str]) -> Result<(), String> {
    let result = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2"])
        .args(video_codec)
        .args(["-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "128k"])
        .args(["-movflags", "+faststart", "-f", "mp4"])
        .arg(output)
        .output()
        .map_err(|e| format!("cannot run ffmpeg: {e}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&result.stderr).trim().to_string())
    }
}

/// Returns the cached MP4 for `smk`, converting it first if it is missing or older than the source.
pub fn ensure_converted(smk: &Path, cache: &Path) -> Result<PathBuf, AudioError> {
    let out = cache_path(cache, smk);
    let fresh = || -> Option<bool> {
        let src = std::fs::metadata(smk).ok()?.modified().ok()?;
        let dst = std::fs::metadata(&out).ok()?.modified().ok()?;
        Some(dst >= src)
    };
    if fresh() == Some(true) {
        return Ok(out);
    }
    std::fs::create_dir_all(cache)?;
    // Convert to a temporary name and rename, so an interrupted run never leaves a bad file.
    let tmp = out.with_extension("mp4.part");
    let first = ["-c:v", "libx264", "-preset", "veryfast", "-crf", "20"];
    let fallback = ["-c:v", "mpeg4", "-q:v", "3"];
    run_ffmpeg(smk, &tmp, &first)
        .or_else(|_| run_ffmpeg(smk, &tmp, &fallback))
        .map_err(AudioError::Video)?;
    std::fs::rename(&tmp, &out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_names() {
        let p = cache_path(Path::new("/c"), Path::new("/g/anim/INTRO_01.SMK"));
        assert_eq!(p, Path::new("/c/intro_01.mp4"));
    }

    #[test]
    fn animation_table_matches_the_exe() {
        assert_eq!(animation_name(0), None);
        assert_eq!(animation_name(1).as_deref(), Some("br_01"));
        assert_eq!(animation_name(12).as_deref(), Some("intro_01"));
        assert_eq!(animation_name(31).as_deref(), Some("cvwin"));
        assert_eq!(animation_name(32).as_deref(), Some("ca01"));
        assert_eq!(animation_name(62).as_deref(), Some("casnores"));
        assert_eq!(animation_name(64).as_deref(), Some("cve01"));
        assert_eq!(animation_name(82).as_deref(), Some("cvewin"));
        assert_eq!(animation_name(104).as_deref(), Some("cawait4"));
        assert_eq!(animation_name(105), None);
    }

    #[test]
    fn lists_only_smk() {
        let dir = std::env::temp_dir().join(format!("rt2-video-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["b.SMK", "A.smk", "c.txt"] {
            std::fs::write(dir.join(n), b"").unwrap();
        }
        let names: Vec<_> = list_animations(&dir)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["A.smk", "b.SMK"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_source_is_an_error() {
        let dir = std::env::temp_dir().join(format!("rt2-video-miss-{}", std::process::id()));
        let r = ensure_converted(Path::new("/nonexistent/x.smk"), &dir);
        assert!(r.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
