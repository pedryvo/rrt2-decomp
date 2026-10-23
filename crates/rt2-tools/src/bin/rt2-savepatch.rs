//! Writes raw fields into a 0x2348 save and recomputes its checksum: builds the prepared saves of
//! the rare-event fidelity cases (`re/scripts/wine/prepare_rare.py`, docs/dynamic-analysis.md
//! 5.9). The original loads the patched file like any other save, so the comparison starts from
//! the original's own conversion of it (`advance_save.py` with 0 days).
//!
//! A patch is `TARGET+OFF:TYPE=VALUE`: `TARGET` is `state`, `trailer` or `POOL[SLOT]` (pools:
//! `structures stations persons companies cars objects anim pool58 pool7c pool80 track trains
//! places territories pool88`), `OFF` a byte offset (decimal or `0x`), `TYPE` one of
//! `u8 i8 u16 i16 u32 i32 f32 f64`. `--get TARGET+OFF:TYPE` prints a field; `--list POOL` the live
//! slots of a pool.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Parser;
use rt2_formats::map::{Pool, Save};

#[derive(Parser)]
#[command(about = "Patch raw fields of a .GAM save (prepared fidelity saves)")]
struct Args {
    input: PathBuf,
    /// Output file (omit with only --get/--list)
    output: Option<PathBuf>,
    /// `TARGET+OFF:TYPE=VALUE` (repeatable)
    #[arg(long = "set")]
    sets: Vec<String>,
    /// `TARGET+OFF:TYPE` (repeatable): print the field
    #[arg(long = "get")]
    gets: Vec<String>,
    /// Print the live slots of a pool
    #[arg(long)]
    list: Option<String>,
}

fn pool_mut<'a>(s: &'a mut Save, name: &str) -> anyhow::Result<&'a mut Pool> {
    Ok(match name {
        "structures" => &mut s.structures,
        "stations" => &mut s.stations,
        "persons" => &mut s.persons,
        "companies" => &mut s.companies,
        "cars" => &mut s.pool_70,
        "objects" => &mut s.objects,
        "anim" => &mut s.pool_78,
        "pool58" => &mut s.pool_58,
        "pool7c" => &mut s.pool_7c,
        "pool80" => &mut s.pool_80,
        "track" => &mut s.track_pieces,
        "trains" => &mut s.trains,
        "places" => &mut s.places,
        "territories" => &mut s.territories,
        "pool88" => &mut s.pool_88,
        _ => bail!("unknown pool {name}"),
    })
}

fn parse_num(s: &str) -> anyhow::Result<i128> {
    let (neg, t) = match s.strip_prefix('-') {
        Some(t) => (true, t),
        None => (false, s),
    };
    let v = match t.strip_prefix("0x") {
        Some(h) => i128::from_str_radix(h, 16)?,
        None => t.parse()?,
    };
    Ok(if neg { -v } else { v })
}

/// `TARGET+OFF:TYPE` -> (target, offset, type).
fn parse_field(s: &str) -> anyhow::Result<(String, usize, String)> {
    let (lhs, ty) = s.rsplit_once(':').context("missing :TYPE")?;
    let (target, off) = lhs.rsplit_once('+').context("missing +OFF")?;
    Ok((target.to_string(), parse_num(off)? as usize, ty.to_string()))
}

fn bytes_of<'a>(s: &'a mut Save, target: &str) -> anyhow::Result<&'a mut Vec<u8>> {
    match target {
        "state" => Ok(&mut s.state),
        "trailer" => Ok(&mut s.trailer),
        _ => {
            let (pool, slot) = target
                .strip_suffix(']')
                .and_then(|t| t.split_once('['))
                .context("target must be state, trailer or POOL[SLOT]")?;
            let slot = parse_num(slot)? as usize;
            let p = pool_mut(s, pool)?;
            p.records
                .iter_mut()
                .find(|r| r.slot == slot)
                .map(|r| &mut r.data)
                .with_context(|| format!("{pool}[{slot}] is not a live record"))
        }
    }
}

fn width(ty: &str) -> anyhow::Result<usize> {
    Ok(match ty {
        "u8" | "i8" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" | "f32" => 4,
        "f64" => 8,
        _ => bail!("unknown type {ty}"),
    })
}

fn encode(ty: &str, v: &str) -> anyhow::Result<Vec<u8>> {
    Ok(match ty {
        "f32" => v.parse::<f32>()?.to_le_bytes().to_vec(),
        "f64" => v.parse::<f64>()?.to_le_bytes().to_vec(),
        _ => {
            let n = parse_num(v)?;
            n.to_le_bytes()[..width(ty)?].to_vec()
        }
    })
}

fn decode(ty: &str, b: &[u8]) -> String {
    match ty {
        "u8" => b[0].to_string(),
        "i8" => (b[0] as i8).to_string(),
        "u16" => u16::from_le_bytes([b[0], b[1]]).to_string(),
        "i16" => i16::from_le_bytes([b[0], b[1]]).to_string(),
        "u32" => format!("{:#x}", u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        "i32" => i32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
        "f32" => f32::from_le_bytes([b[0], b[1], b[2], b[3]]).to_string(),
        _ => f64::from_le_bytes(b[..8].try_into().expect("8 bytes")).to_string(),
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let raw = std::fs::read(&args.input).with_context(|| args.input.display().to_string())?;
    let mut save =
        Save::parse(&raw).with_context(|| format!("parsing {}", args.input.display()))?;
    if let Some(p) = &args.list {
        let pool = pool_mut(&mut save, p)?;
        let slots: Vec<String> = pool.order.iter().map(|s| s.to_string()).collect();
        println!("{p}: {} records: {}", pool.records.len(), slots.join(" "));
    }
    for g in &args.gets {
        let (target, off, ty) = parse_field(g)?;
        let w = width(&ty)?;
        let b = bytes_of(&mut save, &target)?;
        let v = b.get(off..off + w).context("offset out of range")?;
        println!("{g} = {}", decode(&ty, v));
    }
    for s in &args.sets {
        let (lhs, val) = s.split_once('=').context("missing =VALUE")?;
        let (target, off, ty) = parse_field(lhs)?;
        let enc = encode(&ty, val)?;
        let b = bytes_of(&mut save, &target)?;
        b.get_mut(off..off + enc.len())
            .context("offset out of range")?
            .copy_from_slice(&enc);
    }
    if let Some(out) = &args.output {
        // Note: `to_bytes` writes the decoded scalars (minutes, date, time, speed, game_seed)
        // over the raw blocks; patch those with the decoded fields, not here.
        std::fs::write(out, save.to_bytes()).with_context(|| out.display().to_string())?;
    } else if !args.sets.is_empty() {
        bail!("--set needs an output file");
    }
    Ok(())
}
