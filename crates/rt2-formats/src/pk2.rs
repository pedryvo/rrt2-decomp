//! PK2 archive: a flat container of files addressed by a 32-bit name hash.
//!
//! Layout (all little-endian), see `docs/formats/pk2.md`:
//!
//! ```text
//! u32                 entry count N
//! N × { u32 hash, u32 offset, u32 size }
//! file data           offsets are relative to the end of the directory
//! ```
//!
//! Entries are not stored compressed. When several archives hold the same hash, the one whose
//! file name sorts last (case-insensitively) wins: `RT2_11PL` > `RT2_09GO` > `RT2_06` > `RT2`.

use std::ops::Range;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Pk2Error {
    #[error("archive is truncated: need {needed} bytes, have {len}")]
    Truncated { needed: usize, len: usize },
    #[error(
        "entry {index} (hash {hash:#010x}) spans {start}..{end}, past end of archive ({len} bytes)"
    )]
    EntryOutOfBounds {
        index: usize,
        hash: u32,
        start: usize,
        end: usize,
        len: usize,
    },
}

/// Case fold used by the filename hash: the original's `FUN_004f0d70`
/// (`'a'..='z'` and bytes above `0xEF` lose `0x20`).
pub fn fold_name_byte(c: u8) -> u8 {
    if c.is_ascii_lowercase() || c > 0xef {
        c - 0x20
    } else {
        c
    }
}

/// Filename hash used as the directory key (loop at `0x4f5070`, case fold at `0x4f0d70`).
/// Names are bare (no directory) and matched case-insensitively.
pub fn name_hash(name: &str) -> u32 {
    name.bytes().fold(7u32, |h, c| {
        u32::from(fold_name_byte(c))
            .wrapping_add(0x3039)
            .wrapping_add(h.wrapping_mul(0x41c6_4e6e))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pk2Entry {
    pub hash: u32,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug)]
pub struct Pk2<'a> {
    data: &'a [u8],
    data_start: usize,
    entries: Vec<Pk2Entry>,
}

fn read_u32(data: &[u8], at: usize) -> Result<u32, Pk2Error> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(Pk2Error::Truncated {
            needed: at + 4,
            len: data.len(),
        })
}

impl<'a> Pk2<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, Pk2Error> {
        let count = read_u32(data, 0)? as usize;
        let data_start = 4 + count * 12;
        if data.len() < data_start {
            return Err(Pk2Error::Truncated {
                needed: data_start,
                len: data.len(),
            });
        }
        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let at = 4 + index * 12;
            let entry = Pk2Entry {
                hash: read_u32(data, at)?,
                offset: read_u32(data, at + 4)?,
                size: read_u32(data, at + 8)?,
            };
            let start = data_start + entry.offset as usize;
            let end = start + entry.size as usize;
            if end > data.len() {
                return Err(Pk2Error::EntryOutOfBounds {
                    index,
                    hash: entry.hash,
                    start,
                    end,
                    len: data.len(),
                });
            }
            entries.push(entry);
        }
        Ok(Self {
            data,
            data_start,
            entries,
        })
    }

    pub fn entries(&self) -> &[Pk2Entry] {
        &self.entries
    }

    pub fn range(&self, entry: &Pk2Entry) -> Range<usize> {
        let start = self.data_start + entry.offset as usize;
        start..start + entry.size as usize
    }

    pub fn contents(&self, entry: &Pk2Entry) -> &'a [u8] {
        &self.data[self.range(entry)]
    }

    pub fn find(&self, hash: u32) -> Option<&Pk2Entry> {
        self.entries.iter().find(|e| e.hash == hash)
    }

    pub fn find_name(&self, name: &str) -> Option<&Pk2Entry> {
        self.find(name_hash(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(files: &[(u32, &[u8])]) -> Vec<u8> {
        let mut out = (files.len() as u32).to_le_bytes().to_vec();
        let mut offset = 0u32;
        for (hash, body) in files {
            out.extend(hash.to_le_bytes());
            out.extend(offset.to_le_bytes());
            out.extend((body.len() as u32).to_le_bytes());
            offset += body.len() as u32;
        }
        for (_, body) in files {
            out.extend(*body);
        }
        out
    }

    #[test]
    fn parses_entries_and_contents() {
        let raw = build(&[(0xdead_beef, b"hello"), (0x0123_4567, b"world!")]);
        let pk2 = Pk2::parse(&raw).unwrap();
        assert_eq!(pk2.entries().len(), 2);
        assert_eq!(pk2.contents(pk2.find(0xdead_beef).unwrap()), b"hello");
        assert_eq!(pk2.contents(pk2.find(0x0123_4567).unwrap()), b"world!");
        assert!(pk2.find(1).is_none());
    }

    #[test]
    fn name_hash_matches_original() {
        // setup.imb is entry 0 of RT2_11PL.PK2.
        assert_eq!(name_hash("setup.imb"), 0x0c9b_172f);
        assert_eq!(name_hash("SETUP.IMB"), name_hash("setup.imb"));
    }

    #[test]
    fn empty_archive() {
        let raw = build(&[]);
        assert!(Pk2::parse(&raw).unwrap().entries().is_empty());
    }

    #[test]
    fn rejects_truncated_directory() {
        let mut raw = build(&[(1, b"abc")]);
        raw.truncate(10);
        assert!(matches!(Pk2::parse(&raw), Err(Pk2Error::Truncated { .. })));
    }

    #[test]
    fn rejects_entry_past_end() {
        let mut raw = build(&[(1, b"abc")]);
        raw.pop();
        assert!(matches!(
            Pk2::parse(&raw),
            Err(Pk2Error::EntryOutOfBounds { index: 0, .. })
        ));
    }
}
