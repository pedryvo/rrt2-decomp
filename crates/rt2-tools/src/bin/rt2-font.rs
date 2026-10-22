//! Render sample text with one of the game's bitmap fonts to a PNG.

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Parser;
use rt2_formats::font::{Font, font_spec};
use rt2_formats::lad::Lad;
use rt2_formats::lng::Lng;
use rt2_formats::palette::rgb555;
use rt2_formats::pk2::{Pk2, name_hash};

#[derive(Parser)]
#[command(about = "Render text with an RT2 bitmap font (copp6.imb ... htra46.imb) to PNG")]
struct Args {
    /// Font number 1..=10 (the N of `fontN.dat`), or `all` for a specimen sheet
    font: String,
    /// Text to render (default: a few strings from the LNG file)
    #[arg(short, long)]
    text: Option<String>,
    /// Game folder (the one containing `data/`); defaults to $RT2_DIR
    #[arg(short, long, env = "RT2_DIR")]
    dir: PathBuf,
    /// Output PNG
    #[arg(short, long)]
    out: PathBuf,
    /// Integer pixel zoom
    #[arg(short, long, default_value_t = 3)]
    zoom: usize,
    /// Draw the shadow layer too
    #[arg(short, long)]
    shadow: bool,
}

const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];

fn load_font(dir: &std::path::Path, index: u8) -> anyhow::Result<Font> {
    let spec = font_spec(index)?;
    let hash = name_hash(spec.bank);
    for name in ARCHIVES {
        let raw = std::fs::read(dir.join("data").join(name))?;
        let pk2 = Pk2::parse(&raw)?;
        if let Some(e) = pk2.find(hash) {
            let lad = Lad::parse(pk2.contents(e))?;
            return Ok(Font::from_lad(spec, &lad)?);
        }
    }
    bail!("{} not found in any PK2", spec.bank)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let lng = std::fs::read(args.dir.join("default2.lng"))
        .ok()
        .and_then(|raw| Lng::parse(&raw).ok());
    let sample = |ids: &[u32]| {
        ids.iter()
            .filter_map(|&id| lng.as_ref()?.get(id))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let text = args.text.unwrap_or_else(|| {
        let s = sample(&[13, 812, 813]);
        if s.is_empty() {
            "Railroad Tycoon II\n0123456789 $1,234.56".to_owned()
        } else {
            format!("{s}\nThe quick brown fox 0123456789 $1,234.56")
        }
    });
    let fonts: Vec<u8> = if args.font == "all" {
        (1..=10).collect()
    } else {
        vec![args.font.parse().context("font must be 1..=10 or `all`")?]
    };

    let bg = [0x30, 0x34, 0x40];
    let fg = rgb555(0x7fff);
    let mut sheet: Vec<(usize, usize, Vec<u8>)> = Vec::new();
    for index in fonts {
        let font = load_font(&args.dir, index)?;
        let img = font.render(&text, fg, Some(bg), args.shadow);
        sheet.push((img.width, img.height, img.pixels));
    }
    let z = args.zoom.max(1);
    let (w, h) = (
        sheet.iter().map(|s| s.0).max().unwrap_or(1) * z,
        sheet.iter().map(|s| s.1 * z + z * 2).sum::<usize>(),
    );
    let mut out = vec![0u8; w * h * 4];
    for p in out.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[0x20, 0x20, 0x28, 255]);
    }
    let mut y0 = 0;
    for (sw, sh, px) in &sheet {
        for y in 0..sh * z {
            for x in 0..sw * z {
                let s = ((y / z) * sw + x / z) * 4;
                let d = ((y0 + y) * w + x) * 4;
                out[d..d + 4].copy_from_slice(&px[s..s + 4]);
            }
        }
        y0 += sh * z + z * 2;
    }
    let file = std::fs::File::create(&args.out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&out)?;
    println!("{w}x{h} written to {}", args.out.display());
    Ok(())
}
