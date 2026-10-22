//! Sound effect entries stored in the PK2 archives (`*.raw`), see `docs/formats/sound.md`.
//!
//! The PK2 blobs are headerless PCM. The format of each one is not stored with the data: the EXE
//! keeps a table of 0x12-byte descriptors at `0x55e7b0` and the sound player (`0x4eeeb0`, buffer
//! creation in `0x4ecb00`) reads it:
//!
//! ```text
//! char[13]  name      NUL-terminated 8.3 file name ("buttu61m.raw"); PK2 lookup is name_hash(name)
//! u8        channels  (always 1)
//! u8        bits      per sample: 16 (122 entries) or 8 (clock82m.raw)
//! u16       rate      Hz: 11025 (122 entries) or 22050 (clock82m.raw)
//! u8        pitch     1 = DirectSound buffer created with DSBCAPS_CTRLFREQUENCY (chug1..6 only)
//! ```
//!
//! The table index is the sound id the rest of the game passes around (event tables, widget
//! sound fields, engine records). Indices 0 and 114 are empty slots (the name is twelve spaces). 16-bit data is
//! signed little-endian; 8-bit data is unsigned (the DirectSound convention). The loader copies
//! the PK2 bytes verbatim into the buffer (`0x4f40e0`), no decoding.

use crate::pk2::{Pk2, name_hash};

/// Descriptor-table base address in `RT2_PLAT.EXE`.
pub const TABLE_ADDR: u32 = 0x55e7b0;
/// Size of one descriptor in bytes.
pub const DESCRIPTOR_SIZE: u32 = 0x12;

/// One row of the sound table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundEntry {
    /// Index in the table (the game's sound id).
    pub id: u8,
    /// PK2 file name as spelled in the EXE (case is irrelevant for lookup); empty for unused slots.
    pub name: &'static str,
    pub channels: u8,
    pub bits: u8,
    pub sample_rate: u32,
    /// The game may change the playback rate (engine chug pitch follows speed).
    pub variable_pitch: bool,
}

impl SoundEntry {
    /// Entry has a name, so a PK2 blob exists for it.
    pub fn is_used(&self) -> bool {
        !self.name.is_empty()
    }

    /// PK2 name hash of the entry's file name.
    pub fn hash(&self) -> u32 {
        name_hash(self.name)
    }

    /// Bytes per sample frame.
    pub fn frame_bytes(&self) -> usize {
        usize::from(self.channels) * usize::from(self.bits / 8)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SoundError {
    #[error("unsupported sample format: {channels} channels, {bits} bits")]
    Unsupported { channels: u8, bits: u8 },
    #[error("blob of {len} bytes is not a whole number of {frame}-byte frames")]
    Misaligned { len: usize, frame: usize },
}

/// Decoded PCM, interleaved when `channels > 1`, always signed 16-bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm {
    pub channels: u8,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

impl Pcm {
    /// Duration in seconds.
    pub fn seconds(&self) -> f64 {
        self.samples.len() as f64 / f64::from(self.channels) / f64::from(self.sample_rate)
    }
}

/// Decodes a PK2 sound blob to signed 16-bit PCM.
///
/// A blob that is not a whole number of frames is rejected; every original entry is aligned.
pub fn decode(entry: &SoundEntry, blob: &[u8]) -> Result<Pcm, SoundError> {
    let frame = entry.frame_bytes();
    if entry.channels == 0 || !matches!(entry.bits, 8 | 16) {
        return Err(SoundError::Unsupported {
            channels: entry.channels,
            bits: entry.bits,
        });
    }
    if !blob.len().is_multiple_of(frame) {
        return Err(SoundError::Misaligned {
            len: blob.len(),
            frame,
        });
    }
    let samples = if entry.bits == 8 {
        blob.iter()
            .map(|&b| (i16::from(b) - 128) << 8)
            .collect::<Vec<_>>()
    } else {
        blob.as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect()
    };
    Ok(Pcm {
        channels: entry.channels,
        sample_rate: entry.sample_rate,
        samples,
    })
}

/// Finds an entry by file name (ASCII case-insensitive; `buttu61m.raw` or `BUTTU61M.RAW`) or by
/// name without the extension.
pub fn find_by_name(name: &str) -> Option<&'static SoundEntry> {
    let name = name.trim();
    SOUNDS.iter().filter(|e| e.is_used()).find(|e| {
        e.name.eq_ignore_ascii_case(name)
            || e.name
                .strip_suffix(".raw")
                .or_else(|| e.name.strip_suffix(".RAW"))
                .is_some_and(|stem| stem.eq_ignore_ascii_case(name))
    })
}

/// Looks the blob up in a set of archives given in the game's precedence order (first match wins).
pub fn blob<'a>(archives: &[Pk2<'a>], entry: &SoundEntry) -> Option<&'a [u8]> {
    let hash = entry.hash();
    archives
        .iter()
        .find_map(|p| p.find(hash).map(|e| p.contents(e)))
}

/// The descriptor table at `0x55e7b0`: `(id, name, channels, bits, rate, variable_pitch)`.
const RAW_TABLE: [(u8, &str, u8, u8, u32, bool); 125] = [
    (0, "", 1, 16, 11025, false),
    (1, "buttu61m.raw", 1, 16, 11025, false),
    (2, "buttd61m.raw", 1, 16, 11025, false),
    (3, "scrol61m.raw", 1, 16, 11025, false),
    (4, "scro261m.raw", 1, 16, 11025, false),
    (5, "clock82m.raw", 1, 8, 22050, false),
    (6, "birds61m.raw", 1, 16, 11025, false),
    (7, "waves61m.raw", 1, 16, 11025, false),
    (8, "swamp61m.raw", 1, 16, 11025, false),
    (9, "river61m.raw", 1, 16, 11025, false),
    (10, "blizz61m.raw", 1, 16, 11025, false),
    (11, "jungL61m.raw", 1, 16, 11025, false),
    (12, "paget61m.raw", 1, 16, 11025, false),
    (13, "page261m.raw", 1, 16, 11025, false),
    (14, "addbu61m.raw", 1, 16, 11025, false),
    (15, "chain61m.raw", 1, 16, 11025, false),
    (16, "bulld61m.raw", 1, 16, 11025, false),
    (17, "swtch61m.raw", 1, 16, 11025, false),
    (18, "tab__61m.raw", 1, 16, 11025, false),
    (19, "chug161m.raw", 1, 16, 11025, true),
    (20, "chug261m.raw", 1, 16, 11025, true),
    (21, "chug361m.raw", 1, 16, 11025, true),
    (22, "chug461m.raw", 1, 16, 11025, true),
    (23, "chug561m.raw", 1, 16, 11025, true),
    (24, "chug661m.raw", 1, 16, 11025, true),
    (25, "whis161m.raw", 1, 16, 11025, false),
    (26, "whis261m.raw", 1, 16, 11025, false),
    (27, "whis361m.raw", 1, 16, 11025, false),
    (28, "whis461m.raw", 1, 16, 11025, false),
    (29, "whis561m.raw", 1, 16, 11025, false),
    (30, "whis661m.raw", 1, 16, 11025, false),
    (31, "whis761m.raw", 1, 16, 11025, false),
    (32, "whis861m.raw", 1, 16, 11025, false),
    (33, "whis961m.raw", 1, 16, 11025, false),
    (34, "horse61m.raw", 1, 16, 11025, false),
    (35, "trdtl61m.raw", 1, 16, 11025, false),
    (36, "cargo61m.raw", 1, 16, 11025, false),
    (37, "radio61m.raw", 1, 16, 11025, false),
    (38, "nexts61m.raw", 1, 16, 11025, false),
    (39, "brake61m.raw", 1, 16, 11025, false),
    (40, "brak261m.raw", 1, 16, 11025, false),
    (41, "brak361m.raw", 1, 16, 11025, false),
    (42, "brak461m.raw", 1, 16, 11025, false),
    (43, "LUMBR61M.RAW", 1, 16, 11025, false),
    (44, "PAPER61M.RAW", 1, 16, 11025, false),
    (45, "GRAVL61M.RAW", 1, 16, 11025, false),
    (46, "CMINE61M.RAW", 1, 16, 11025, false),
    (47, "STEEL61M.RAW", 1, 16, 11025, false),
    (48, "CATTL61M.RAW", 1, 16, 11025, false),
    (49, "LOGGI61M.RAW", 1, 16, 11025, false),
    (50, "cemnt61M.RAW", 1, 16, 11025, false),
    (51, "auto_61M.RAW", 1, 16, 11025, false),
    (52, "caner61M.RAW", 1, 16, 11025, false),
    (53, "grain61M.RAW", 1, 16, 11025, false),
    (54, "nuker61M.RAW", 1, 16, 11025, false),
    (55, "textl61M.RAW", 1, 16, 11025, false),
    (56, "elect61M.RAW", 1, 16, 11025, false),
    (57, "baux_61M.RAW", 1, 16, 11025, false),
    (58, "MEATP61M.RAW", 1, 16, 11025, false),
    (59, "LANDF61M.RAW", 1, 16, 11025, false),
    (60, "STATN61M.RAW", 1, 16, 11025, false),
    (61, "SHEEP61M.RAW", 1, 16, 11025, false),
    (62, "TIREF62M.RAW", 1, 16, 11025, false),
    (63, "OILRF61M.RAW", 1, 16, 11025, false),
    (64, "BAKER61M.RAW", 1, 16, 11025, false),
    (65, "alum_61M.RAW", 1, 16, 11025, false),
    (66, "uran_61m.raw", 1, 16, 11025, false),
    (67, "sugar61m.raw", 1, 16, 11025, false),
    (68, "oilwl61m.raw", 1, 16, 11025, false),
    (69, "port_61m.raw", 1, 16, 11025, false),
    (70, "dairp61m.raw", 1, 16, 11025, false),
    (71, "fert_61m.raw", 1, 16, 11025, false),
    (72, "chin161m.raw", 1, 16, 11025, false),
    (73, "chin261m.raw", 1, 16, 11025, false),
    (74, "chin361m.raw", 1, 16, 11025, false),
    (75, "laytr61m.raw", 1, 16, 11025, false),
    (76, "tr_in61m.raw", 1, 16, 11025, false),
    (77, "layrv61m.raw", 1, 16, 11025, false),
    (78, "addtr61m.raw", 1, 16, 11025, false),
    (79, "raise61m.raw", 1, 16, 11025, false),
    (80, "buckt61m.raw", 1, 16, 11025, false),
    (81, "bella61m.raw", 1, 16, 11025, false),
    (82, "bellb61m.raw", 1, 16, 11025, false),
    (83, "adds161m.raw", 1, 16, 11025, false),
    (84, "adds261m.raw", 1, 16, 11025, false),
    (85, "adds361m.raw", 1, 16, 11025, false),
    (86, "whtab61m.raw", 1, 16, 11025, false),
    (87, "hawk_61m.raw", 1, 16, 11025, false),
    (88, "cargr61m.raw", 1, 16, 11025, false),
    (89, "book_61m.raw", 1, 16, 11025, false),
    (90, "noteb61m.raw", 1, 16, 11025, false),
    (91, "buybu61m.raw", 1, 16, 11025, false),
    (92, "brush61m.raw", 1, 16, 11025, false),
    (93, "resou61m.raw", 1, 16, 11025, false),
    (94, "trdtd61m.raw", 1, 16, 11025, false),
    (95, "papfl61m.raw", 1, 16, 11025, false),
    (96, "papnw61m.raw", 1, 16, 11025, false),
    (97, "stock61m.raw", 1, 16, 11025, false),
    (98, "roost61m.raw", 1, 16, 11025, false),
    (99, "trdte61m.raw", 1, 16, 11025, false),
    (100, "LOCKC61m.raw", 1, 16, 11025, false),
    (101, "LOCKO61m.raw", 1, 16, 11025, false),
    (102, "GEESE61m.raw", 1, 16, 11025, false),
    (103, "AFRIC61m.raw", 1, 16, 11025, false),
    (104, "GAMES61m.raw", 1, 16, 11025, false),
    (105, "pigeo61m.raw", 1, 16, 11025, false),
    (106, "ENBRE61m.raw", 1, 16, 11025, false),
    (107, "ENBR261m.raw", 1, 16, 11025, false),
    (108, "ENCRA61m.raw", 1, 16, 11025, false),
    (109, "ENFIR61m.raw", 1, 16, 11025, false),
    (110, "ENROB61m.raw", 1, 16, 11025, false),
    (111, "ANNCL61m.raw", 1, 16, 11025, false),
    (112, "ENBR361m.raw", 1, 16, 11025, false),
    (113, "CMBKG61m.raw", 1, 16, 11025, false),
    (114, "", 1, 16, 11025, false),
    (115, "STOCS61m.raw", 1, 16, 11025, false),
    (116, "campg61m.raw", 1, 16, 11025, false),
    (117, "cmbk261m.raw", 1, 16, 11025, false),
    (118, "barrk61m.raw", 1, 16, 11025, false),
    (119, "Distl61m.raw", 1, 16, 11025, false),
    (120, "geoc_61m.raw", 1, 16, 11025, false),
    (121, "Munit61m.raw", 1, 16, 11025, false),
    (122, "weapf61m.raw", 1, 16, 11025, false),
    (123, "airpt61m.raw", 1, 16, 11025, false),
    (124, "flyby61m.raw", 1, 16, 11025, false),
];

const fn build() -> [SoundEntry; 125] {
    let mut out = [SoundEntry {
        id: 0,
        name: "",
        channels: 0,
        bits: 0,
        sample_rate: 0,
        variable_pitch: false,
    }; 125];
    let mut i = 0;
    while i < 125 {
        let r = RAW_TABLE[i];
        out[i] = SoundEntry {
            id: r.0,
            name: r.1,
            channels: r.2,
            bits: r.3,
            sample_rate: r.4,
            variable_pitch: r.5,
        };
        i += 1;
    }
    out
}

/// All 125 table rows, indexed by sound id.
pub static SOUNDS: [SoundEntry; 125] = build();

/// Entry for a sound id (`None` for out-of-range ids and the two empty slots).
pub fn by_id(id: u8) -> Option<&'static SoundEntry> {
    SOUNDS.get(usize::from(id)).filter(|e| e.is_used())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_shape() {
        assert_eq!(SOUNDS.iter().filter(|e| e.is_used()).count(), 123);
        assert!(by_id(0).is_none() && by_id(114).is_none() && by_id(125).is_none());
        assert_eq!(by_id(1).unwrap().name, "buttu61m.raw");
        assert_eq!(by_id(19).unwrap().name, "chug161m.raw");
        assert!(by_id(19).unwrap().variable_pitch);
        let clock = by_id(5).unwrap();
        assert_eq!((clock.bits, clock.sample_rate), (8, 22050));
        assert_eq!(by_id(124).unwrap().name, "flyby61m.raw");
        for (i, e) in SOUNDS.iter().enumerate() {
            assert_eq!(usize::from(e.id), i);
        }
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert_eq!(find_by_name("BUTTU61M.RAW").unwrap().id, 1);
        assert_eq!(find_by_name("buttu61m").unwrap().id, 1);
        assert_eq!(find_by_name("lumbr61m").unwrap().id, 43);
        assert!(find_by_name("nope").is_none());
        assert!(find_by_name("").is_none());
    }

    #[test]
    fn decode_formats() {
        let e16 = by_id(1).unwrap();
        let pcm = decode(e16, &[0x01, 0x00, 0xff, 0xff]).unwrap();
        assert_eq!(pcm.samples, vec![1, -1]);
        assert!(decode(e16, &[1, 2, 3]).is_err());
        let e8 = by_id(5).unwrap();
        let pcm = decode(e8, &[128, 255, 0]).unwrap();
        assert_eq!(pcm.samples, vec![0, 127 << 8, -32768]);
        assert_eq!(pcm.sample_rate, 22050);
    }
}
