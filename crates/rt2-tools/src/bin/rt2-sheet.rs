//! Contact sheets and a catalogue listing of the LAD sprite banks in the PK2 archives.
//!
//! `list` prints one TSV row per LAD bank; `sheet` renders the first image of each record of a
//! range of banks (rows) into a PNG. The output is game imagery: keep it out of the repository.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use rt2_formats::lad::{FLAG_PALETTE, Lad, LadRecord};
use rt2_formats::palette::{PaletteBank, palette_name, rgb555};
use rt2_formats::pk2::{Pk2, name_hash};

#[derive(Parser)]
#[command(about = "List and render contact sheets of the LAD sprite banks")]
struct Args {
    /// Game folder (the one containing `data/`); defaults to $RT2_DIR
    #[arg(short, long, env = "RT2_DIR")]
    dir: PathBuf,
    /// Known filenames, one per line (hashes are matched against the PK2 directories)
    #[arg(short, long, default_value = "re/names/exe_filenames.txt")]
    names: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// TSV: ordinal, archive/index, hash, name, records, groups, images, first image size, palette id/count
    List,
    /// Per record: header words, group/image layout (one line per record)
    Dump {
        ordinal: usize,
        /// First record to print
        #[arg(long, default_value_t = 0)]
        skip: usize,
        /// Records to print
        #[arg(long, default_value_t = 1000)]
        count: usize,
        /// Also print the palette-index histogram of every image of each record
        #[arg(long)]
        hist: bool,
    },
    /// Per record of one bank: mean colour and share of water-keyed (blue) pixels of the first image
    Stats {
        ordinal: usize,
        #[arg(short, long, default_value_t = 22)]
        pal: usize,
    },
    /// One thumbnail per bank (record/group/image given), labelled with the ordinal
    Grid {
        from: usize,
        to: usize,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(short, long, default_value_t = 64)]
        cell: usize,
        #[arg(long, default_value_t = 20)]
        cols: usize,
        #[arg(short, long, default_value_t = 0)]
        record: usize,
        #[arg(short, long, default_value_t = 0)]
        group: usize,
        #[arg(short, long, default_value_t = 0)]
        image: usize,
        #[arg(short, long, default_value_t = 0)]
        pal: usize,
        /// Brightness multiplier
        #[arg(long, default_value_t = 1)]
        gain: u32,
    },
    /// Render banks `from..to` (ordinals of `list`) as rows of record thumbnails
    Sheet {
        from: usize,
        to: usize,
        #[arg(short, long)]
        out: PathBuf,
        /// Thumbnail cell size in pixels
        #[arg(short, long, default_value_t = 64)]
        cell: usize,
        /// Thumbnail columns
        #[arg(long, default_value_t = 16)]
        cols: usize,
        /// Records shown per bank (wraps into several rows of `cols`)
        #[arg(long, default_value_t = 16)]
        max: usize,
        /// First record shown
        #[arg(long, default_value_t = 0)]
        skip: usize,
        /// Group shown for each record
        #[arg(short, long, default_value_t = 0)]
        group: usize,
        /// Image within the group
        #[arg(short, long, default_value_t = 0)]
        image: usize,
        /// Palette index inside each record's palette bank
        #[arg(short, long, default_value_t = 0)]
        pal: usize,
        /// Brightness multiplier
        #[arg(long, default_value_t = 1)]
        gain: u32,
    },
}

const ARCHIVES: [(&str, &str); 4] = [
    ("RT2_11PL.PK2", "PL"),
    ("RT2_09GO.PK2", "GO"),
    ("RT2_06.PK2", "06"),
    ("RT2.PK2", "RT2"),
];

struct Bank {
    tag: String,
    index: usize,
    hash: u32,
    blob: Vec<u8>,
}

/// Every LAD bank (precedence order) and every blob by hash.
type Loaded = (Vec<Bank>, HashMap<u32, Vec<u8>>);

fn load_banks(dir: &Path) -> anyhow::Result<Loaded> {
    let mut banks = Vec::new();
    let mut all: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for (file, tag) in ARCHIVES {
        let path = dir.join("data").join(file);
        let raw = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let pk2 = Pk2::parse(&raw)?;
        for (index, e) in pk2.entries().iter().enumerate() {
            let blob = pk2.contents(e);
            // Highest-precedence archive wins; shadowed duplicates are skipped.
            if !seen.insert(e.hash) {
                continue;
            }
            all.insert(e.hash, blob.to_vec());
            if is_lad(blob) {
                banks.push(Bank {
                    tag: tag.to_string(),
                    index,
                    hash: e.hash,
                    blob: blob.to_vec(),
                });
            }
        }
    }
    // Stable order: RT2 first (its directory order), then the patches.
    banks.sort_by_key(|b| (b.tag != "RT2", b.tag.clone(), b.index));
    Ok((banks, all))
}

fn is_lad(blob: &[u8]) -> bool {
    match Lad::parse_with_len(blob) {
        Ok((lad, used)) => {
            used == blob.len()
                && !lad.records.is_empty()
                && lad.records.iter().all(|r| matches!(r.kind(), 1 | 3))
        }
        Err(_) => false,
    }
}

fn load_names(path: &Path) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let line = line.trim();
            if !line.is_empty() {
                map.entry(name_hash(line))
                    .or_insert_with(|| line.to_string());
            }
        }
    }
    map
}

/// 3x5 glyphs for `0-9a-f`, one row per 3 bits.
const GLYPHS: [[u8; 5]; 16] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
    [2, 5, 7, 5, 5],
    [6, 5, 6, 5, 6],
    [7, 4, 4, 4, 7],
    [6, 5, 5, 5, 6],
    [7, 4, 6, 4, 7],
    [7, 4, 6, 4, 4],
];

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize, bg: [u8; 3]) -> Self {
        let mut px = Vec::with_capacity(w * h * 4);
        for _ in 0..w * h {
            px.extend([bg[0], bg[1], bg[2], 255]);
        }
        Self { w, h, px }
    }
    fn put(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 4;
            self.px[i..i + 3].copy_from_slice(&c);
        }
    }
    fn text(&mut self, x: usize, y: usize, s: &str, c: [u8; 3]) {
        for (n, ch) in s.chars().enumerate() {
            let Some(d) = ch.to_digit(16) else { continue };
            for (ry, row) in GLYPHS[d as usize].iter().enumerate() {
                for rx in 0..3 {
                    if row >> (2 - rx) & 1 != 0 {
                        self.put(x + n * 4 + rx, y + ry, c);
                    }
                }
            }
        }
    }
}

struct Palettes<'a> {
    all: &'a HashMap<u32, Vec<u8>>,
    which: usize,
    cache: HashMap<u32, Option<[u16; 256]>>,
}

impl Palettes<'_> {
    fn get(&mut self, record: &LadRecord) -> [u16; 256] {
        let id = record.palette_id();
        let (all, which) = (self.all, self.which);
        let pal = self.cache.entry(id).or_insert_with(|| {
            let blob = all.get(&name_hash(&palette_name(id)))?;
            let bank = PaletteBank::parse(blob).ok()?;
            bank.palettes.get(which).or(bank.palettes.first()).copied()
        });
        pal.unwrap_or_else(|| std::array::from_fn(|i| (i as u16 >> 3) * 0x421))
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (banks, all) = load_banks(&args.dir)?;
    let names = load_names(&args.names);
    match args.cmd {
        Cmd::List => {
            for (n, b) in banks.iter().enumerate() {
                let lad = Lad::parse(&b.blob)?;
                let groups: usize = lad.records.iter().map(|r| r.groups.len()).sum();
                let images: usize = lad
                    .records
                    .iter()
                    .flat_map(|r| &r.groups)
                    .map(|g| g.images.len())
                    .sum();
                let first = lad
                    .records
                    .iter()
                    .flat_map(|r| &r.groups)
                    .flat_map(|g| &g.images)
                    .next()
                    .map_or(String::from("-"), |i| format!("{}x{}", i.width, i.height));
                let pal = lad.records.first().map_or(String::from("-"), |r| {
                    let count = all
                        .get(&name_hash(&palette_name(r.palette_id())))
                        .and_then(|b| PaletteBank::parse(b).ok())
                        .map_or(0, |b| b.palettes.len());
                    if r.uses_palette() {
                        format!("{}/{count}", r.palette_id())
                    } else {
                        String::from("-")
                    }
                });
                println!(
                    "{n}\t{}/{:04}\t{:08x}\t{}\t{}\t{groups}\t{images}\t{first}\t{pal}",
                    b.tag,
                    b.index,
                    b.hash,
                    names.get(&b.hash).map_or("?", String::as_str),
                    lad.records.len(),
                );
            }
        }
        Cmd::Dump {
            ordinal,
            skip,
            count,
            hist,
        } => {
            let b = banks.get(ordinal).context("no such bank")?;
            let lad = Lad::parse(&b.blob)?;
            for (n, r) in lad.records.iter().enumerate().skip(skip).take(count) {
                let hdr: Vec<String> = r.header.iter().map(|w| format!("{w:x}")).collect();
                print!("r{n} pal={} hdr={} |", r.palette_id(), hdr.join(","));
                for g in &r.groups {
                    print!(" [");
                    if !g.frame_map.is_empty() {
                        print!("map{:?} ", g.frame_map);
                    }
                    for i in &g.images {
                        print!("{:x}:{}x{}@{},{} ", i.flags, i.width, i.height, i.x, i.y);
                    }
                    print!("]");
                }
                println!();
                if hist {
                    for (gi, g) in r.groups.iter().enumerate() {
                        for (ii, i) in g.images.iter().enumerate() {
                            let Ok(px) = i.decode() else { continue };
                            let mut counts = std::collections::BTreeMap::new();
                            for (v, _) in px.indices.iter().zip(&px.opaque).filter(|(_, o)| **o) {
                                *counts.entry(*v).or_insert(0usize) += 1;
                            }
                            let top: Vec<String> =
                                counts.iter().map(|(v, c)| format!("{v}:{c}")).collect();
                            println!("  g{gi}i{ii} opaque indices: {}", top.join(" "));
                        }
                    }
                }
            }
        }
        Cmd::Stats { ordinal, pal } => {
            let b = banks.get(ordinal).context("no such bank")?;
            let lad = Lad::parse(&b.blob)?;
            let mut pals = Palettes {
                all: &all,
                which: pal,
                cache: HashMap::new(),
            };
            for (n, rec) in lad.records.iter().enumerate() {
                let Some(img) = rec.groups.first().and_then(|g| g.images.first()) else {
                    continue;
                };
                let Ok(px) = img.decode() else { continue };
                let palette = pals.get(rec);
                let (mut sum, mut land, mut water, mut total) = ([0u32; 3], 0u32, 0u32, 0u32);
                for (&i, &op) in px.indices.iter().zip(&px.opaque) {
                    if !op {
                        continue;
                    }
                    total += 1;
                    let [r, g, b] = rgb555(palette[usize::from(i)]);
                    if i32::from(b) > i32::from(r) + 80 {
                        water += 1;
                    } else {
                        land += 1;
                        sum[0] += u32::from(r);
                        sum[1] += u32::from(g);
                        sum[2] += u32::from(b);
                    }
                }
                let m = |v: u32| v.checked_div(land).unwrap_or(0);
                println!(
                    "{n}\t{}x{}\twater {:.2}\tmean {:02x}{:02x}{:02x}",
                    px.width,
                    px.height,
                    f64::from(water) / f64::from(total.max(1)),
                    m(sum[0]),
                    m(sum[1]),
                    m(sum[2])
                );
            }
        }
        Cmd::Grid {
            from,
            to,
            out,
            cell,
            cols,
            record,
            group,
            image,
            pal,
            gain,
        } => {
            let to = to.min(banks.len());
            if from >= to {
                bail!("empty range");
            }
            let n_rows = (to - from).div_ceil(cols);
            let pitch = cell + 1;
            let mut canvas = Canvas::new(cols * pitch, n_rows * (pitch + 6), [40; 3]);
            let mut pals = Palettes {
                all: &all,
                which: pal,
                cache: HashMap::new(),
            };
            for (k, n) in (from..to).enumerate() {
                let (x0, y0) = ((k % cols) * pitch, (k / cols) * (pitch + 6));
                canvas.text(x0, y0, &format!("{n:04}"), [255, 255, 0]);
                let lad = Lad::parse(&banks[n].blob)?;
                if let Some(rec) = lad.records.get(record) {
                    draw_thumb(
                        &mut canvas,
                        x0,
                        y0 + 6,
                        cell,
                        rec,
                        (group, image),
                        &mut pals,
                        gain,
                    );
                }
            }
            write_png(&out, &canvas)?;
        }
        Cmd::Sheet {
            from,
            to,
            out,
            cell,
            cols,
            max,
            skip,
            group,
            image,
            pal,
            gain,
        } => {
            let to = to.min(banks.len());
            if from >= to {
                bail!("empty range");
            }
            let label_w = 20;
            let row_h = cell + 2;
            let rows_of = |n: usize| n.saturating_sub(skip).min(max).div_ceil(cols).max(1);
            let total_rows: usize = (from..to)
                .map(|n| {
                    Lad::parse(&banks[n].blob)
                        .map(|l| rows_of(l.records.len()))
                        .unwrap_or(1)
                })
                .sum();
            let mut canvas = Canvas::new(label_w + cols * (cell + 1), total_rows * row_h, [40; 3]);
            let mut next_row = 0;
            let mut pals = Palettes {
                all: &all,
                which: pal,
                cache: HashMap::new(),
            };
            for (n, bank) in banks.iter().enumerate().take(to).skip(from) {
                let lad = Lad::parse(&bank.blob)?;
                let y0 = next_row * row_h;
                next_row += rows_of(lad.records.len());
                canvas.text(1, y0 + 1, &format!("{n:04}"), [255, 255, 0]);
                canvas.text(
                    1,
                    y0 + 8,
                    &format!("{:x}", lad.records.len().min(0xfff)),
                    [0, 255, 255],
                );
                for (k, rec) in lad.records.iter().skip(skip).take(max).enumerate() {
                    let (col, y) = (k % cols, y0 + (k / cols) * row_h);
                    let x0 = label_w + col * (cell + 1);
                    draw_thumb(
                        &mut canvas,
                        x0,
                        y,
                        cell,
                        rec,
                        (group, image),
                        &mut pals,
                        gain,
                    );
                }
            }
            write_png(&out, &canvas)?;
        }
    }
    Ok(())
}

fn write_png(out: &Path, canvas: &Canvas) -> anyhow::Result<()> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::File::create(out)?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        canvas.w as u32,
        canvas.h as u32,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&canvas.px)?;
    Ok(())
}

/// Draws the `(group, image)` of `rec` into a `cell`-sized checkerboard cell at `(x0, y0)`,
/// shrunk by an integer factor when larger than the cell and centred (not hotspot-aligned).
#[allow(clippy::too_many_arguments)]
fn draw_thumb(
    canvas: &mut Canvas,
    x0: usize,
    y0: usize,
    cell: usize,
    rec: &LadRecord,
    (group, image): (usize, usize),
    pals: &mut Palettes,
    gain: u32,
) {
    for yy in 0..cell {
        for xx in 0..cell {
            let c = if (xx / 8 + yy / 8) % 2 == 0 { 70 } else { 90 };
            canvas.put(x0 + xx, y0 + yy, [c; 3]);
        }
    }
    let Some(img) = rec.groups.get(group).and_then(|g| g.images.get(image)) else {
        return;
    };
    let Ok(px) = img.decode() else { return };
    if px.width == 0 || px.height == 0 {
        return;
    }
    let pal = if img.flags & FLAG_PALETTE != 0 {
        pals.get(rec)
    } else {
        std::array::from_fn(|i| (i as u16 >> 3) * 0x421)
    };
    let rgba = px.to_rgba(&pal);
    let scale = px.width.max(px.height).div_ceil(cell).max(1);
    let (dw, dh) = (px.width / scale, px.height / scale);
    let (ox, oy) = ((cell - dw) / 2, (cell - dh) / 2);
    for dy in 0..dh {
        for dx in 0..dw {
            let i = ((dy * scale) * px.width + dx * scale) * 4;
            if rgba[i + 3] != 0 {
                let c = |k: usize| (u32::from(rgba[i + k]) * gain).min(255) as u8;
                canvas.put(x0 + ox + dx, y0 + oy + dy, [c(0), c(1), c(2)]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_clips_and_draws_glyphs() {
        let mut c = Canvas::new(8, 6, [0, 0, 0]);
        c.put(100, 100, [1, 2, 3]);
        c.text(0, 0, "1", [255, 0, 0]);
        // Glyph "1": top row is `.X.`, so pixel (1, 0) is lit and (0, 0) is not.
        assert_eq!(&c.px[4..7], [255, 0, 0]);
        assert_eq!(&c.px[0..3], [0, 0, 0]);
    }

    #[test]
    fn text_skips_non_hex_characters() {
        let mut c = Canvas::new(8, 6, [9, 9, 9]);
        c.text(0, 0, "zz", [255, 255, 255]);
        assert!(c.px.chunks(4).all(|p| p[..3] == [9, 9, 9]));
    }

    #[test]
    fn lad_detection_rejects_garbage() {
        assert!(!is_lad(&[]));
        assert!(!is_lad(&[1, 0, 0, 0, 7, 0, 0, 0]));
    }

    #[test]
    fn names_file_maps_hashes() {
        let dir = std::env::temp_dir().join("rt2-sheet-names-test.txt");
        std::fs::write(&dir, "setup.imb\n\n ground.imb \n").unwrap();
        let names = load_names(&dir);
        assert_eq!(names.get(&name_hash("setup.imb")).unwrap(), "setup.imb");
        assert_eq!(names.get(&name_hash("GROUND.IMB")).unwrap(), "ground.imb");
        std::fs::remove_file(dir).ok();
    }
}
