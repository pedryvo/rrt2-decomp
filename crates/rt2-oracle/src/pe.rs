//! Minimal PE32 parser: sections and the import table.

use crate::OracleError;

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub rva: u32,
    pub virtual_size: u32,
    pub raw_offset: u32,
    pub raw_size: u32,
    pub characteristics: u32,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub dll: String,
    pub name: String,
    /// RVA of the IAT slot.
    pub iat_rva: u32,
}

#[derive(Debug, Clone)]
pub struct PeImage {
    pub image_base: u32,
    pub size_of_headers: u32,
    pub entry_rva: u32,
    pub sections: Vec<Section>,
    pub imports: Vec<Import>,
}

fn u16_at(d: &[u8], o: usize) -> Result<u16, OracleError> {
    d.get(o..o + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| OracleError::Pe(format!("truncated at {o:#x}")))
}

fn u32_at(d: &[u8], o: usize) -> Result<u32, OracleError> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| OracleError::Pe(format!("truncated at {o:#x}")))
}

fn cstr(d: &[u8], o: usize) -> Result<String, OracleError> {
    let tail = d
        .get(o..)
        .ok_or_else(|| OracleError::Pe(format!("string out of file at {o:#x}")))?;
    let end = tail.iter().position(|&b| b == 0).unwrap_or(tail.len());
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

impl PeImage {
    pub fn parse(d: &[u8]) -> Result<Self, OracleError> {
        if d.get(..2) != Some(b"MZ") {
            return Err(OracleError::Pe("missing MZ".into()));
        }
        let pe = u32_at(d, 0x3c)? as usize;
        if d.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(OracleError::Pe("missing PE signature".into()));
        }
        let nsec = usize::from(u16_at(d, pe + 6)?);
        let opt_size = usize::from(u16_at(d, pe + 20)?);
        let opt = pe + 24;
        if u16_at(d, opt)? != 0x10b {
            return Err(OracleError::Pe("not a PE32 image".into()));
        }
        let entry_rva = u32_at(d, opt + 16)?;
        let image_base = u32_at(d, opt + 28)?;
        let size_of_headers = u32_at(d, opt + 60)?;
        let import_rva = u32_at(d, opt + 104)? as usize;

        let mut sections = Vec::new();
        for i in 0..nsec {
            let s = opt + opt_size + i * 40;
            sections.push(Section {
                name: cstr(&d[..(s + 8).min(d.len())], s)?,
                virtual_size: u32_at(d, s + 8)?,
                rva: u32_at(d, s + 12)?,
                raw_size: u32_at(d, s + 16)?,
                raw_offset: u32_at(d, s + 20)?,
                characteristics: u32_at(d, s + 36)?,
            });
        }

        let rva_to_off = |rva: usize| -> Result<usize, OracleError> {
            sections
                .iter()
                .find(|s| {
                    rva >= s.rva as usize
                        && rva < s.rva as usize + s.virtual_size.max(s.raw_size) as usize
                })
                .map(|s| rva - s.rva as usize + s.raw_offset as usize)
                .ok_or_else(|| OracleError::Pe(format!("rva {rva:#x} not in any section")))
        };

        let mut imports = Vec::new();
        if import_rva != 0 {
            let mut desc = rva_to_off(import_rva)?;
            loop {
                let oft = u32_at(d, desc)? as usize;
                let name_rva = u32_at(d, desc + 12)? as usize;
                let ft = u32_at(d, desc + 16)? as usize;
                if name_rva == 0 && ft == 0 {
                    break;
                }
                let dll = cstr(d, rva_to_off(name_rva)?)?;
                let lookup = if oft != 0 { oft } else { ft };
                let mut k = 0usize;
                loop {
                    let thunk = u32_at(d, rva_to_off(lookup + 4 * k)?)?;
                    if thunk == 0 {
                        break;
                    }
                    let name = if thunk & 0x8000_0000 != 0 {
                        format!("#{}", thunk & 0xffff)
                    } else {
                        cstr(d, rva_to_off(thunk as usize)? + 2)?
                    };
                    imports.push(Import {
                        dll: dll.clone(),
                        name,
                        iat_rva: (ft + 4 * k) as u32,
                    });
                    k += 1;
                }
                desc += 20;
            }
        }
        Ok(Self {
            image_base,
            size_of_headers,
            entry_rva,
            sections,
            imports,
        })
    }
}
