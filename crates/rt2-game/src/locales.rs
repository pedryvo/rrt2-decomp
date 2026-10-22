//! Locale discovery and choice (docs/i18n.md "Distribuicao").
//!
//! The translation files in `assets/lang/` are embedded in the binary at build time (`build.rs`),
//! so a single executable carries every language. A folder of `*.toml` files overrides the
//! embedded ones by locale (and adds new languages) without a rebuild: `$RRT2_LANG_DIR`, else a
//! `lang/` folder next to the executable.

use std::path::{Path, PathBuf};

use rt2_formats::i18n::Translation;

include!(concat!(env!("OUT_DIR"), "/lang_embedded.rs"));

/// The language of the original game, always available (from `default2.lng`).
pub const ENGLISH: &str = "en";
pub const ENGLISH_NAME: &str = "English";

/// The override folder, if any.
pub fn override_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("RRT2_LANG_DIR") {
        return Some(PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("lang");
    dir.is_dir().then_some(dir)
}

/// Every available translation: the embedded files, then those of `dir` (replacing the same
/// locale). Sorted by locale tag so the selector order is stable. Broken files are skipped with
/// a log line.
pub fn discover(dir: Option<&Path>) -> Vec<Translation> {
    let mut out: Vec<Translation> = Vec::new();
    let mut add = |what: &str, text: &str| match Translation::parse(text) {
        Ok(t) => {
            out.retain(|o| !o.locale.eq_ignore_ascii_case(&t.locale));
            out.push(t);
        }
        Err(e) => eprintln!("i18n: skipping {what}: {e}"),
    };
    for (name, text) in EMBEDDED {
        add(name, text);
    }
    if let Some(dir) = dir
        && let Ok(rd) = std::fs::read_dir(dir)
    {
        let mut files: Vec<PathBuf> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .collect();
        files.sort();
        for f in files {
            if let Ok(text) = std::fs::read_to_string(&f) {
                add(&f.display().to_string(), &text);
            }
        }
    }
    out.sort_by(|a, b| a.locale.cmp(&b.locale));
    out
}

/// `pt_BR.UTF-8` -> `pt-BR`; `C`/`POSIX` and empty values are not a language.
pub fn normalize(tag: &str) -> Option<String> {
    let base = tag.split(['.', '@']).next()?.replace('_', "-");
    (!base.is_empty() && base != "C" && base != "POSIX").then_some(base)
}

/// The user's locale from the environment (`LC_ALL`, `LC_MESSAGES`, `LANG`).
pub fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| {
        std::env::var(k)
            .ok()
            .filter(|v| !v.is_empty())
            .and_then(|v| normalize(&v))
    })
}

/// The available locale for a request: exact (case-insensitive), else the same primary
/// language (`pt-PT` -> `pt-BR`, `zh-TW` -> `zh-CN`), else English.
pub fn pick(requested: &str, available: &[Translation]) -> String {
    let req = normalize(requested).unwrap_or_default();
    if let Some(t) = available
        .iter()
        .find(|t| t.locale.eq_ignore_ascii_case(&req))
    {
        return t.locale.clone();
    }
    let primary = |s: &str| s.split('-').next().unwrap_or("").to_ascii_lowercase();
    let want = primary(&req);
    available
        .iter()
        .find(|t| primary(&t.locale) == want)
        .map_or_else(|| ENGLISH.to_owned(), |t| t.locale.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(locale: &str) -> Translation {
        Translation {
            locale: locale.into(),
            ..Translation::default()
        }
    }

    #[test]
    fn normalizes_posix_locale_names() {
        assert_eq!(normalize("pt_BR.UTF-8").as_deref(), Some("pt-BR"));
        assert_eq!(normalize("de_DE@euro").as_deref(), Some("de-DE"));
        assert_eq!(normalize("C"), None);
        assert_eq!(normalize(""), None);
    }

    #[test]
    fn picks_exact_then_primary_language_then_english() {
        let av = [tr("de"), tr("pt-BR"), tr("zh-CN")];
        assert_eq!(pick("pt_BR.UTF-8", &av), "pt-BR");
        assert_eq!(pick("PT-br", &av), "pt-BR");
        assert_eq!(pick("pt-PT", &av), "pt-BR");
        assert_eq!(pick("zh-TW", &av), "zh-CN");
        assert_eq!(pick("fr-FR", &av), "en");
        assert_eq!(pick("C", &av), "en");
    }

    #[test]
    fn a_folder_overrides_the_embedded_file_of_the_same_locale() {
        let dir = std::env::temp_dir().join(format!("rrt2-lang-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("zz.toml"),
            "[meta]\nlocale = \"zz\"\nlanguage = \"Zed\"\n[strings]\n1 = \"um\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("bad.toml"), "not toml [").unwrap();
        let found = discover(Some(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        let zz = found.iter().find(|t| t.locale == "zz").expect("zz");
        assert_eq!(zz.language, "Zed");
        assert_eq!(zz.strings[&1], "um");
    }
}
