//! Render one image of a LAD sprite bank from the PK2 archives to a PNG.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Parser;
use rt2_formats::lad::Lad;
use rt2_formats::palette::{PaletteBank, palette_name};
use rt2_formats::pk2::{Pk2, name_hash};

#[derive(Parser)]
#[command(about = "Render a sprite (LAD) entry of the PK2 archives to PNG")]
struct Args {
    /// Entry filename (e.g. `copp6.imb`) or hash as `0x1234abcd`
    entry: String,
    /// Game folder (the one containing `data/`); defaults to $RT2_DIR
    #[arg(short, long, env = "RT2_DIR")]
    dir: PathBuf,
    /// Output PNG
    #[arg(short, long)]
    out: PathBuf,
    /// Record (frame set) index
    #[arg(short, long, default_value_t = 0)]
    record: usize,
    /// Group index within the record
    #[arg(short, long, default_value_t = 0)]
    group: usize,
    /// Image index within the group
    #[arg(short, long, default_value_t = 0)]
    image: usize,
    /// Palette index inside the bank
    #[arg(short, long, default_value_t = 0)]
    palette: usize,
}

/// Archives in precedence order (alphabetically last wins).
const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];

fn find(raws: &[Vec<u8>], hash: u32) -> anyhow::Result<Option<Vec<u8>>> {
    for raw in raws {
        let pk2 = Pk2::parse(raw)?;
        if let Some(e) = pk2.find(hash) {
            return Ok(Some(pk2.contents(e).to_vec()));
        }
    }
    Ok(None)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut raws = Vec::new();
    for name in ARCHIVES {
        let path = args.dir.join("data").join(name);
        raws.push(std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?);
    }
    let hash = match args.entry.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16).context("bad hash")?,
        None => name_hash(&args.entry),
    };
    let blob = find(&raws, hash)?.with_context(|| format!("no entry with hash {hash:#010x}"))?;
    let lad = Lad::parse(&blob)?;
    let record = lad.records.get(args.record).context("no such record")?;
    let group = record.groups.get(args.group).context("no such group")?;
    let image = group.images.get(args.image).context("no such image")?;
    let pixels = image.decode()?;
    if pixels.width == 0 || pixels.height == 0 {
        bail!("image is empty ({}x{})", image.width, image.height);
    }

    let mut palette = std::array::from_fn(|i| (i as u16 >> 3) * 0x421);
    if image.flags & rt2_formats::lad::FLAG_PALETTE != 0 {
        let name = palette_name(record.palette_id());
        let bank = find(&raws, name_hash(&name))?.with_context(|| format!("missing {name}"))?;
        let bank = PaletteBank::parse(&bank)?;
        palette = *bank
            .palettes
            .get(args.palette)
            .context("palette index out of range")?;
    }
    let rgba = pixels.to_rgba(&palette);

    let file = std::fs::File::create(&args.out)?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        pixels.width as u32,
        pixels.height as u32,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    println!(
        "{}x{} at ({}, {}), palette {}",
        pixels.width,
        pixels.height,
        image.x,
        image.y,
        palette_name(record.palette_id())
    );
    Ok(())
}
