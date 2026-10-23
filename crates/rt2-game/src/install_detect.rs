//! Finds installs of the original game (docs/ui.md "Tela de pasta do jogo"): Steam libraries
//! (Linux, Windows and Proton `compatdata`), GOG (Windows registry, default folders), Heroic and
//! Lutris, Wine prefixes and the usual Windows folders.
//!
//! Everything is read only. The search takes a [`Roots`] value instead of looking at the machine,
//! so tests build a fake tree in a temporary folder; [`Roots::system`] describes the real one.

use std::path::{Path, PathBuf};

use crate::paths;

/// Where a candidate came from; the setup screen shows it next to the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The folder already in `config.toml` (or `--game-dir` / `RT2_DIR`).
    Configured,
    Steam,
    Gog,
    Heroic,
    Lutris,
    Wine,
    Windows,
    /// Picked with the folder chooser.
    Picked,
}

impl Source {
    /// English label (a key of the `[app]` table).
    pub fn english(self) -> &'static str {
        match self {
            Source::Configured => "Current setting",
            Source::Steam => "Steam library",
            Source::Gog => "GOG",
            Source::Heroic => "Heroic (GOG)",
            Source::Lutris => "Lutris",
            Source::Wine => "Wine prefix",
            Source::Windows => "Windows folder",
            Source::Picked => "Chosen by you",
        }
    }
}

/// A candidate folder with the files it lacks (empty when it is a usable install).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    pub source: Source,
    pub missing: Vec<&'static str>,
}

impl Found {
    pub fn new(path: PathBuf, source: Source) -> Found {
        let missing = paths::missing_files(&path);
        Found {
            path,
            source,
            missing,
        }
    }

    pub fn valid(&self) -> bool {
        self.missing.is_empty()
    }
}

/// What to search. Every field is optional; an empty value searches nothing.
#[derive(Clone, Debug, Default)]
pub struct Roots {
    /// The folder in the configuration, listed first even when it is broken.
    pub configured: Option<PathBuf>,
    /// Steam installs (the folder that holds `steamapps/`).
    pub steam: Vec<PathBuf>,
    /// Windows drive roots (`C:\`): Program Files, GOG Games and friends are looked up under them.
    pub windows_drives: Vec<PathBuf>,
    /// `drive_c` folders of Wine prefixes (plain Wine, Bottles, PlayOnLinux, Heroic, Lutris).
    pub wine_drives: Vec<PathBuf>,
    /// Folders whose children are games (`~/Games`, `~/Games/Heroic`), for Lutris and Heroic.
    pub game_folders: Vec<(PathBuf, Source)>,
    /// Lutris configuration folders (`~/.config/lutris`): `games/*.yml` name the executable.
    pub lutris: Vec<PathBuf>,
    /// Heroic configuration folders (`~/.config/heroic`): `gog_store/installed.json`.
    pub heroic: Vec<PathBuf>,
    /// Install folders GOG recorded in the Windows registry.
    pub gog_registry: Vec<PathBuf>,
}

impl Roots {
    /// The usual places under a Linux home folder.
    pub fn for_home(home: &Path) -> Roots {
        let mut r = Roots::default();
        let h = |p: &str| home.join(p);
        r.steam = vec![
            h(".steam/steam"),
            h(".steam/root"),
            h(".local/share/Steam"),
            h(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            h(".var/app/com.valvesoftware.Steam/data/Steam"),
        ];
        // Prefixes: each child of these folders, plus `~/.wine` itself.
        let mut prefixes = vec![h(".wine")];
        for parent in [
            ".local/share/wineprefixes",
            ".PlayOnLinux/wineprefix",
            ".local/share/bottles/bottles",
            ".var/app/com.usebottles.bottles/data/bottles/bottles",
            ".local/share/lutris/prefixes",
            "Games",
            "Games/Heroic/Prefixes",
            "Games/Heroic/Prefixes/default",
            ".wine-prefixes",
        ] {
            prefixes.extend(subdirs(&h(parent)));
        }
        r.wine_drives = prefixes
            .into_iter()
            .map(|p| p.join("drive_c"))
            .filter(|p| p.is_dir())
            .collect();
        r.game_folders = vec![
            // Heroic first: its folder is also a child of `~/Games`.
            (h("Games/Heroic"), Source::Heroic),
            (h("Games"), Source::Lutris),
            (h(".local/share/lutris/games"), Source::Lutris),
            (h("GOG Games"), Source::Gog),
        ];
        r.lutris = vec![
            h(".config/lutris"),
            h(".var/app/net.lutris.Lutris/config/lutris"),
        ];
        r.heroic = vec![
            h(".config/heroic"),
            h(".var/app/com.heroicgameslauncher.hgl/config/heroic"),
        ];
        r
    }

    /// The real machine: the home folder, `$WINEPREFIX`, the Windows drives and registry, and the
    /// configured folder.
    pub fn system(configured: Option<PathBuf>) -> Roots {
        let mut r = dirs::home_dir().map_or_else(Roots::default, |h| Roots::for_home(&h));
        r.configured = configured;
        if let Some(p) = std::env::var_os("WINEPREFIX") {
            let drive = PathBuf::from(p).join("drive_c");
            if drive.is_dir() && !r.wine_drives.contains(&drive) {
                r.wine_drives.push(drive);
            }
        }
        if cfg!(windows) {
            for letter in b'A'..=b'Z' {
                let drive = PathBuf::from(format!("{}:\\", letter as char));
                if drive.is_dir() {
                    r.windows_drives.push(drive);
                }
            }
            for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
                if let Some(p) = std::env::var_os(var) {
                    r.steam.push(PathBuf::from(p).join("Steam"));
                }
            }
            r.gog_registry = gog_registry_paths();
        }
        r
    }
}

/// Child folders of `dir`, sorted (so the result does not depend on the file system order).
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// A folder name that is probably the game: "Railroad Tycoon II Platinum", "railroad-tycoon-2",
/// "RT2".
pub fn looks_like_rt2(name: &str) -> bool {
    let n = name.to_lowercase().replace(['-', '_'], " ");
    n.contains("railroad tycoon")
        || n.contains("railroadtycoon")
        || n == "rt2"
        || n.starts_with("rt2 ")
}

/// Collects the accumulating candidates, once per folder.
struct Collector {
    out: Vec<Found>,
    seen: Vec<PathBuf>,
}

impl Collector {
    fn key(p: &Path) -> PathBuf {
        p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
    }

    /// Adds a folder. `strict` folders (found through an executable path or a registry value, not
    /// through their name) only count when they are a usable install.
    fn add(&mut self, path: &Path, source: Source, strict: bool) {
        if !path.is_dir() {
            return;
        }
        let key = Collector::key(path);
        if self.seen.contains(&key) {
            return;
        }
        let found = Found::new(path.to_path_buf(), source);
        if strict && !found.valid() {
            return;
        }
        self.seen.push(key);
        self.out.push(found);
    }

    /// Folders under `parent` whose name looks like the game, looking `depth` levels further
    /// into the other children (publisher folders such as `Hasbro Interactive`).
    fn scan(&mut self, parent: &Path, depth: u32, source: Source) {
        for child in subdirs(parent) {
            let name = child
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if looks_like_rt2(&name) {
                self.add(&child, source, false);
            } else if depth > 0 {
                self.scan(&child, depth - 1, source);
            }
        }
    }

    fn windows_drive(&mut self, drive: &Path, wine: bool) {
        let tag = |gog: bool| {
            if gog {
                Source::Gog
            } else if wine {
                Source::Wine
            } else {
                Source::Windows
            }
        };
        for (sub, depth, gog) in [
            ("Program Files", 2, false),
            ("Program Files (x86)", 2, false),
            ("GOG Games", 1, true),
            ("GOG Galaxy/Games", 1, true),
            ("Games", 1, false),
            ("Hasbro Interactive", 1, false),
            ("Sierra", 1, false),
            ("Gathering of Developers", 1, false),
            ("", 0, false),
        ] {
            let parent = if sub.is_empty() {
                drive.to_path_buf()
            } else {
                drive.join(sub)
            };
            self.scan(&parent, depth, tag(gog));
        }
    }

    fn steam_library(&mut self, lib: &Path) {
        let apps = lib.join("steamapps");
        // The manifests say which folder of `common/` belongs to which game.
        let mut manifests: Vec<PathBuf> = std::fs::read_dir(&apps)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    n.starts_with("appmanifest_") && n.ends_with(".acf")
                })
            })
            .collect();
        manifests.sort();
        for m in manifests {
            let Ok(text) = std::fs::read_to_string(&m) else {
                continue;
            };
            let name = vdf_value(&text, "name").unwrap_or_default();
            if let Some(dir) = vdf_value(&text, "installdir")
                && (looks_like_rt2(&name) || looks_like_rt2(&dir))
            {
                self.add(&apps.join("common").join(dir), Source::Steam, false);
            }
        }
        self.scan(&apps.join("common"), 0, Source::Steam);
        // Proton: the game's own Windows prefix (a copy installed by another launcher inside
        // the prefix of a Steam game, or a non-Steam game added to Steam).
        for prefix in subdirs(&apps.join("compatdata")) {
            let drive = prefix.join("pfx").join("drive_c");
            if drive.is_dir() {
                for (sub, depth) in [
                    ("Program Files", 2),
                    ("Program Files (x86)", 2),
                    ("GOG Games", 1),
                    ("", 0),
                ] {
                    let parent = if sub.is_empty() {
                        drive.clone()
                    } else {
                        drive.join(sub)
                    };
                    self.scan(&parent, depth, Source::Steam);
                }
            }
        }
    }
}

/// Finds the installs under `roots`, the configured folder first, then the usable ones, then the
/// folders that look right but lack files. Deterministic: folders are visited in sorted order.
pub fn detect(roots: &Roots) -> Vec<Found> {
    let mut c = Collector {
        out: Vec::new(),
        seen: Vec::new(),
    };
    // The configured folder is always shown, even when it is not a usable install.
    if let Some(p) = &roots.configured {
        let found = Found::new(p.clone(), Source::Configured);
        c.seen.push(Collector::key(p));
        c.out.push(found);
    }
    let mut steam_roots = roots.steam.clone();
    for d in &roots.windows_drives {
        for sub in ["Program Files (x86)/Steam", "Program Files/Steam", "Steam"] {
            steam_roots.push(d.join(sub));
        }
    }
    for d in &roots.wine_drives {
        for sub in ["Program Files (x86)/Steam", "Program Files/Steam"] {
            steam_roots.push(d.join(sub));
        }
    }
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in &steam_roots {
        if !root.join("steamapps").is_dir() {
            continue;
        }
        let mut libs = vec![root.clone()];
        for vdf in [
            root.join("steamapps/libraryfolders.vdf"),
            root.join("config/libraryfolders.vdf"),
        ] {
            if let Ok(text) = std::fs::read_to_string(&vdf) {
                libs.extend(parse_library_folders(&text));
            }
        }
        for lib in libs {
            let key = Collector::key(&lib);
            if !libraries.iter().any(|l| Collector::key(l) == key) {
                libraries.push(lib);
            }
        }
    }
    for lib in &libraries {
        c.steam_library(lib);
    }
    for p in &roots.gog_registry {
        c.add(p, Source::Gog, true);
    }
    for d in &roots.windows_drives {
        c.windows_drive(d, false);
    }
    for d in &roots.wine_drives {
        c.windows_drive(d, true);
    }
    for dir in &roots.heroic {
        for rel in ["gog_store/installed.json", "store_cache/gog_library.json"] {
            if let Ok(text) = std::fs::read_to_string(dir.join(rel)) {
                for p in json_strings(&text, "install_path") {
                    c.add(Path::new(&p), Source::Heroic, true);
                }
            }
        }
    }
    for dir in &roots.lutris {
        let mut ymls: Vec<PathBuf> = std::fs::read_dir(dir.join("games"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "yml"))
            .collect();
        ymls.sort();
        for y in ymls {
            let Ok(text) = std::fs::read_to_string(&y) else {
                continue;
            };
            for line in text.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("exe:") {
                    let exe = PathBuf::from(unquote(v));
                    if let Some(parent) = exe.parent() {
                        c.add(parent, Source::Lutris, true);
                    }
                } else if let Some(v) = line.strip_prefix("prefix:") {
                    let drive = PathBuf::from(unquote(v)).join("drive_c");
                    if drive.is_dir() {
                        c.windows_drive(&drive, true);
                    }
                }
            }
        }
    }
    for (dir, source) in &roots.game_folders {
        c.scan(dir, 1, *source);
    }
    // Usable first (stable), the configured folder staying on top.
    let (head, rest): (Vec<Found>, Vec<Found>) = c
        .out
        .into_iter()
        .partition(|f| f.source == Source::Configured);
    let (ok, bad): (Vec<Found>, Vec<Found>) = rest.into_iter().partition(Found::valid);
    head.into_iter().chain(ok).chain(bad).collect()
}

fn unquote(v: &str) -> String {
    v.trim().trim_matches(['"', '\'']).to_owned()
}

/// Reads the value of `"key"  "value"` in Valve's text format (first match).
pub fn vdf_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let t = vdf_pair(l)?;
        t.0.eq_ignore_ascii_case(key).then_some(t.1)
    })
}

/// `"key"    "value"` -> (key, value), with `\\` unescaped.
fn vdf_pair(line: &str) -> Option<(String, String)> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (c, in_str) {
            ('"', false) => in_str = true,
            ('"', true) => {
                parts.push(std::mem::take(&mut cur));
                in_str = false;
            }
            ('\\', true) => match chars.next() {
                Some('n') => cur.push('\n'),
                Some(o) => cur.push(o),
                None => {}
            },
            (c, true) => cur.push(c),
            _ => {}
        }
    }
    (parts.len() == 2).then(|| (parts.remove(0), parts.remove(0)))
}

/// The library folders of a `libraryfolders.vdf` (new format `"path"`, old format `"1" "<dir>"`).
pub fn parse_library_folders(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(vdf_pair)
        .filter(|(k, v)| {
            k.eq_ignore_ascii_case("path")
                || (!k.is_empty()
                    && k.bytes().all(|b| b.is_ascii_digit())
                    && v.contains(['/', '\\']))
        })
        .map(|(_, v)| PathBuf::from(v))
        .collect()
}

/// Every string value of `"key": "..."` in a JSON text (enough for Heroic's lists; no parser
/// needed for a flat key).
pub fn json_strings(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(&needle) {
        rest = &rest[i + needle.len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(':') else {
            continue;
        };
        let Some(after) = after.trim_start().strip_prefix('"') else {
            continue;
        };
        let mut s = String::new();
        let mut chars = after.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => s.push('\n'),
                    Some(o) => s.push(o),
                    None => break,
                },
                c => s.push(c),
            }
        }
        out.push(s);
    }
    out
}

/// `path    REG_SZ    C:\GOG Games\...` lines of `reg query` output.
pub fn parse_reg_paths(output: &str) -> Vec<PathBuf> {
    output
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            let rest = t.strip_prefix("path")?.trim_start();
            let rest = rest.strip_prefix("REG_SZ")?.trim();
            (!rest.is_empty()).then(|| PathBuf::from(rest))
        })
        .collect()
}

/// GOG's registry entries (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`, value `path`), read
/// with the `reg` tool that ships with Windows (no extra dependency).
fn gog_registry_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for key in [
        r"HKLM\SOFTWARE\WOW6432Node\GOG.com\Games",
        r"HKLM\SOFTWARE\GOG.com\Games",
    ] {
        if let Ok(o) = std::process::Command::new("reg")
            .args(["query", key, "/s", "/v", "path"])
            .output()
            && o.status.success()
        {
            out.extend(parse_reg_paths(&String::from_utf8_lossy(&o.stdout)));
        }
    }
    out
}

/// A temporary folder removed at the end (tests of this module and of the setup screen).
#[cfg(test)]
pub(crate) struct Tmp(pub PathBuf);

#[cfg(test)]
impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        let p = std::env::temp_dir().join(format!("rrt2-detect-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}

#[cfg(test)]
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The minimum files of a usable install (empty content; nothing of the original).
#[cfg(test)]
pub(crate) fn fake_install(dir: &Path) {
    for f in paths::REQUIRED {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::{Tmp, fake_install};

    #[test]
    fn names_that_look_like_the_game() {
        assert!(looks_like_rt2("Railroad Tycoon II Platinum"));
        assert!(looks_like_rt2("railroad-tycoon-2"));
        assert!(looks_like_rt2("RT2"));
        assert!(!looks_like_rt2("Rise of Nations"));
    }

    #[test]
    fn steam_libraries_come_from_the_vdf_on_both_platforms() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"/home/u/.local/share/Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n";
        assert_eq!(
            parse_library_folders(vdf),
            vec![
                PathBuf::from("/home/u/.local/share/Steam"),
                PathBuf::from("D:\\SteamLibrary")
            ]
        );
        assert_eq!(
            parse_library_folders(
                "\"LibraryFolders\"\n{\n\"TimeNextStatsReport\" \"123\"\n\"1\" \"/mnt/games\"\n}"
            ),
            vec![PathBuf::from("/mnt/games")]
        );
    }

    #[test]
    fn registry_and_json_helpers() {
        let reg = "HKEY_LOCAL_MACHINE\\SOFTWARE\\WOW6432Node\\GOG.com\\Games\\1445250876\n    path    REG_SZ    C:\\GOG Games\\Railroad Tycoon 2\n";
        assert_eq!(
            parse_reg_paths(reg),
            vec![PathBuf::from("C:\\GOG Games\\Railroad Tycoon 2")]
        );
        let json = r#"{"installed":[{"appName":"1","install_path":"/home/u/Games/Heroic/Railroad Tycoon 2","x":1}]}"#;
        assert_eq!(
            json_strings(json, "install_path"),
            vec!["/home/u/Games/Heroic/Railroad Tycoon 2"]
        );
    }

    #[test]
    fn finds_steam_gog_wine_lutris_and_the_configured_folder() {
        let t = Tmp::new("all");
        let home = &t.0;
        // Steam: a second library named in the vdf, with a manifest.
        let steam = home.join(".local/share/Steam");
        let lib2 = home.join("bigdisk/SteamLibrary");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\"0\"\n{{\n\"path\" \"{}\"\n}}\n}}\n",
                lib2.display()
            ),
        )
        .unwrap();
        std::fs::create_dir_all(lib2.join("steamapps")).unwrap();
        std::fs::write(
            lib2.join("steamapps/appmanifest_6860.acf"),
            "\"AppState\"\n{\n\"name\"\t\"Railroad Tycoon II Platinum\"\n\"installdir\"\t\"RT2Plat\"\n}\n",
        )
        .unwrap();
        fake_install(&lib2.join("steamapps/common/RT2Plat"));
        // Wine prefix with a GOG install.
        let gog = home.join(".wine/drive_c/GOG Games/Railroad Tycoon 2 Platinum");
        fake_install(&gog);
        // Proton prefix of some other game with Program Files.
        let proton = steam.join("steamapps/compatdata/99/pfx/drive_c/Program Files (x86)/Hasbro Interactive/Railroad Tycoon II");
        fake_install(&proton);
        // Lutris: plain folder under ~/Games plus a yml with an executable elsewhere.
        let games = home.join("Games/railroad-tycoon-2");
        fake_install(&games);
        let elsewhere = home.join("opt/rt2-custom");
        fake_install(&elsewhere);
        std::fs::create_dir_all(home.join(".config/lutris/games")).unwrap();
        std::fs::write(
            home.join(".config/lutris/games/rt2.yml"),
            format!("game:\n  exe: {}/RT2_PLAT.EXE\n", elsewhere.display()),
        )
        .unwrap();
        // Heroic.
        let heroic = home.join("Games/Heroic/Railroad Tycoon 2");
        fake_install(&heroic);
        // A folder that looks right but is incomplete, and the configured one (broken).
        let partial = home.join(".wine/drive_c/Program Files/Railroad Tycoon II Demo");
        std::fs::create_dir_all(&partial).unwrap();
        let cfg = home.join("moved-away");

        let mut roots = Roots::for_home(home);
        roots.configured = Some(cfg.clone());
        let found = detect(&roots);

        let at = |p: &Path| found.iter().find(|f| f.path == p);
        assert_eq!(found[0].path, cfg, "the configured folder is first");
        assert_eq!(found[0].source, Source::Configured);
        assert!(!found[0].valid());
        // `moved-away` does not exist: still listed, with every file missing.
        assert_eq!(found[0].missing.len(), paths::REQUIRED.len());
        assert_eq!(
            at(&lib2.join("steamapps/common/RT2Plat")).unwrap().source,
            Source::Steam
        );
        assert_eq!(at(&gog).unwrap().source, Source::Gog);
        assert_eq!(at(&proton).unwrap().source, Source::Steam);
        assert_eq!(at(&games).unwrap().source, Source::Lutris);
        assert_eq!(at(&elsewhere).unwrap().source, Source::Lutris);
        assert_eq!(at(&heroic).unwrap().source, Source::Heroic);
        let p = at(&partial).unwrap();
        assert!(!p.valid() && p.missing.contains(&"data/RT2.PK2"));
        // Usable ones come before the incomplete one; each folder appears once.
        let pos = |p: &Path| found.iter().position(|f| f.path == p).unwrap();
        assert!(pos(&gog) < pos(&partial));
        for (i, a) in found.iter().enumerate() {
            assert!(
                found.iter().skip(i + 1).all(|b| b.path != a.path),
                "{:?}",
                a.path
            );
        }
    }

    #[test]
    fn the_configured_folder_is_not_listed_twice() {
        let t = Tmp::new("dup");
        let home = &t.0;
        let wine = home.join(".wine/drive_c/Program Files/Railroad Tycoon II Platinum");
        fake_install(&wine);
        let mut roots = Roots::for_home(home);
        roots.configured = Some(wine.clone());
        let found = detect(&roots);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, Source::Configured);
        assert!(found[0].valid());
    }

    #[test]
    fn an_empty_machine_finds_nothing() {
        let t = Tmp::new("empty");
        assert!(detect(&Roots::for_home(&t.0)).is_empty());
    }
}
