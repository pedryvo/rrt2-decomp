//! Minimal PE32 reader for the original executable, used only to load data tables at run time.
//!
//! The tables (cargo, engines, industries...) are copyrighted game data and are never stored in
//! this repository. The caller supplies the bytes of `RT2_PLAT.EXE` (no I/O happens here).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExeError {
    #[error("not a PE32 image: {0}")]
    BadImage(&'static str),
    #[error("virtual range {va:#x}..+{len:#x} is not backed by file data")]
    Unmapped { va: u32, len: usize },
}

#[derive(Clone, Debug)]
struct Section {
    va: u32,
    raw_off: u32,
    raw_size: u32,
}

/// The executable's bytes plus the section map needed to resolve virtual addresses.
#[derive(Clone, Debug)]
pub struct Exe {
    bytes: Vec<u8>,
    sections: Vec<Section>,
    image_base: u32,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, ExeError> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(ExeError::BadImage("truncated header"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, ExeError> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(ExeError::BadImage("truncated header"))
}

impl Exe {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ExeError> {
        if bytes.get(0..2) != Some(b"MZ") {
            return Err(ExeError::BadImage("missing MZ"));
        }
        let pe = u32_at(&bytes, 0x3c)? as usize;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(ExeError::BadImage("missing PE signature"));
        }
        let nsec = usize::from(u16_at(&bytes, pe + 6)?);
        let opt_size = usize::from(u16_at(&bytes, pe + 20)?);
        let opt = pe + 24;
        if u16_at(&bytes, opt)? != 0x10b {
            return Err(ExeError::BadImage("not PE32"));
        }
        let image_base = u32_at(&bytes, opt + 28)?;
        let mut sections = Vec::new();
        for i in 0..nsec {
            let s = opt + opt_size + i * 40;
            sections.push(Section {
                va: image_base.wrapping_add(u32_at(&bytes, s + 12)?),
                raw_size: u32_at(&bytes, s + 16)?,
                raw_off: u32_at(&bytes, s + 20)?,
            });
        }
        Ok(Exe {
            bytes,
            sections,
            image_base,
        })
    }

    pub fn image_base(&self) -> u32 {
        self.image_base
    }

    /// Bytes at virtual address `va`, `len` long; the range must lie in one section's file data.
    pub fn read(&self, va: u32, len: usize) -> Result<&[u8], ExeError> {
        let err = ExeError::Unmapped { va, len };
        for s in &self.sections {
            if va >= s.va && (va - s.va) as usize + len <= s.raw_size as usize {
                let off = s.raw_off as usize + (va - s.va) as usize;
                return self.bytes.get(off..off + len).ok_or(err);
            }
        }
        Err(err)
    }

    pub fn u32_at(&self, va: u32) -> Result<u32, ExeError> {
        let b = self.read(va, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// `n` consecutive little-endian `f32` values.
    pub fn f32s(&self, va: u32, n: usize) -> Result<Vec<f32>, ExeError> {
        Ok(self
            .read(va, n * 4)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a tiny PE32 image with one section at RVA 0x1000, raw offset 0x200.
    fn fake() -> Vec<u8> {
        let mut b = vec![0u8; 0x300];
        b[0..2].copy_from_slice(b"MZ");
        b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        b[0x86..0x88].copy_from_slice(&1u16.to_le_bytes()); // sections
        b[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes()); // optional header size
        let opt = 0x98;
        b[opt..opt + 2].copy_from_slice(&0x10bu16.to_le_bytes());
        b[opt + 28..opt + 32].copy_from_slice(&0x400000u32.to_le_bytes());
        let s = opt + 0xe0;
        b[s + 12..s + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        b[s + 16..s + 20].copy_from_slice(&0x10u32.to_le_bytes());
        b[s + 20..s + 24].copy_from_slice(&0x200u32.to_le_bytes());
        b[0x200..0x204].copy_from_slice(&1.5f32.to_le_bytes());
        b
    }

    #[test]
    fn maps_virtual_addresses() {
        let exe = Exe::from_bytes(fake()).unwrap();
        assert_eq!(exe.image_base(), 0x400000);
        assert_eq!(exe.f32s(0x401000, 1).unwrap(), vec![1.5]);
        assert!(exe.read(0x401010, 1).is_err());
        assert!(exe.read(0x500000, 1).is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(Exe::from_bytes(vec![0; 16]).is_err());
    }
}
