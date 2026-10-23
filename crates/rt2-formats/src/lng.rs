//! LNG language files (`default2.lng`): numbered UI strings, see `docs/formats/lng.md`.
//!
//! ```text
//! ; comment
//! ;tCARGO_TYPES_0          <- names the table that starts at the next id
//!  812    "passenger"
//! ```
//!
//! Expansion packs append sections that redefine ids, so the last definition wins.

use std::collections::BTreeMap;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum LngError {
    #[error("line {line}: expected `<id> \"<text>\"`")]
    Malformed { line: usize },
}

/// A named run of consecutive ids, e.g. `CARGO_TYPES` starting at 812.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LngTable {
    pub name: String,
    pub first_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lng {
    pub strings: BTreeMap<u32, String>,
    pub tables: Vec<LngTable>,
}

impl Lng {
    pub fn get(&self, id: u32) -> Option<&str> {
        self.strings.get(&id).map(String::as_str)
    }

    /// The `index`-th entry of a named table.
    pub fn table_entry(&self, name: &str, index: u32) -> Option<&str> {
        let table = self.tables.iter().rev().find(|t| t.name == name)?;
        self.get(table.first_id + index)
    }

    /// Bytes are decoded as Latin-1; the shipped English files are pure ASCII.
    pub fn parse(data: &[u8]) -> Result<Self, LngError> {
        let text: String = data.iter().map(|&b| b as char).collect();
        let mut lng = Lng::default();
        let mut pending_table: Option<String> = None;

        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix(';') {
                if let Some(name) = table_marker(comment) {
                    pending_table = Some(name.to_owned());
                }
                continue;
            }
            // default2.lng contains stray developer notes; anything not led by an id is ignored.
            let id_len = line
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(line.len());
            if id_len == 0 {
                continue;
            }
            let malformed = LngError::Malformed { line: index + 1 };
            let (id, rest) = line.split_at(id_len);
            let id: u32 = id.parse().map_err(|_| malformed.clone())?;
            let quoted = rest
                .trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or(malformed)?;

            if let Some(name) = pending_table.take() {
                lng.tables.push(LngTable { name, first_id: id });
            }
            lng.strings.insert(id, unescape(quoted));
        }
        Ok(lng)
    }
}

/// `tCARGO_TYPES_0  - abbreviated cargo types` -> `CARGO_TYPES`.
fn table_marker(comment: &str) -> Option<&str> {
    let word = comment.split_whitespace().next()?;
    word.strip_prefix('t')?.strip_suffix("_0")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"; Language file\r\n;\r\n   1    \"\\n\"\r\n  13   \"Base\"   \r\n\r\n ;tCARGO_TYPES_0  - cargo\r\n 812    \"passenger\"\r\n 813    \"mail\"\r\n  13   \"Platinum\"\r\n";

    #[test]
    fn parses_strings_with_escapes() {
        let lng = Lng::parse(SAMPLE).unwrap();
        assert_eq!(lng.get(1), Some("\n"));
        assert_eq!(lng.get(812), Some("passenger"));
    }

    #[test]
    fn last_definition_wins() {
        assert_eq!(Lng::parse(SAMPLE).unwrap().get(13), Some("Platinum"));
    }

    #[test]
    fn table_marker_names_following_id() {
        let lng = Lng::parse(SAMPLE).unwrap();
        assert_eq!(
            lng.tables,
            vec![LngTable {
                name: "CARGO_TYPES".into(),
                first_id: 812
            }]
        );
        assert_eq!(lng.table_entry("CARGO_TYPES", 1), Some("mail"));
    }

    #[test]
    fn ignores_lines_without_id() {
        let lng = Lng::parse(b"comment out 2616 above\r\n7 \"x\"\r\n").unwrap();
        assert_eq!(lng.strings.len(), 1);
    }

    #[test]
    fn rejects_unquoted_text() {
        assert_eq!(
            Lng::parse(b"5 hello\r\n").unwrap_err(),
            LngError::Malformed { line: 1 }
        );
    }
}
