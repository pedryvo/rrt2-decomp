//! Render the geometry of a UI window (`*.win`) to a PNG: control rectangles, ids, sprites.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::Parser;
use rt2_formats::font::{Cell, Font, decode_text_image, font_spec};
use rt2_formats::lad::Lad;
use rt2_formats::palette::{PaletteBank, palette_name, rgb555};
use rt2_formats::pk2::{Pk2, name_hash};
use rt2_formats::ui::{ControlKind, Window};

#[derive(Parser)]
#[command(about = "Render a UI window layout (.win) from the PK2 archives to PNG")]
struct Args {
    /// Window file name, e.g. `filerq.win`
    name: String,
    /// Game folder (the one containing `data/`); defaults to $RT2_DIR
    #[arg(short, long, env = "RT2_DIR")]
    dir: PathBuf,
    /// Output PNG
    #[arg(short, long)]
    out: PathBuf,
    /// Only print the control list
    #[arg(long)]
    dump: bool,
    /// Do not draw sprites, only rectangles
    #[arg(long)]
    no_sprites: bool,
    /// Instead of a window, render every record of this UI bank (`mainbut.imb`) side by side
    /// (`name` is ignored)
    #[arg(long)]
    bank_sheet: Option<String>,
}

const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];
const SCREEN: (usize, usize) = (1024, 768);

fn find(dir: &Path, name: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let hash = name_hash(name);
    for a in ARCHIVES {
        let raw = std::fs::read(dir.join("data").join(a))?;
        let pk2 = Pk2::parse(&raw)?;
        if let Some(e) = pk2.find(hash) {
            return Ok(Some(pk2.contents(e).to_vec()));
        }
    }
    Ok(None)
}

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn put(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        if rgba[3] == 255 {
            self.px[i..i + 4].copy_from_slice(&rgba);
        } else {
            for c in 0..3 {
                let (s, d) = (u32::from(rgba[c]), u32::from(self.px[i + c]));
                self.px[i + c] =
                    ((s * u32::from(rgba[3]) + d * (255 - u32::from(rgba[3]))) / 255) as u8;
            }
        }
    }

    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, fill: Option<[u8; 4]>, line: [u8; 4]) {
        for yy in y..y + h {
            for xx in x..x + w {
                let edge = xx == x || yy == y || xx == x + w - 1 || yy == y + h - 1;
                if edge {
                    self.put(xx, yy, line);
                } else if let Some(f) = fill {
                    self.put(xx, yy, f);
                }
            }
        }
    }

    fn text(&mut self, font: &Font, x: i32, y: i32, text: &str, colour: [u8; 3]) {
        let img = font.render(text, colour, None, false);
        for iy in 0..img.height {
            for ix in 0..img.width {
                let p = &img.pixels[(iy * img.width + ix) * 4..][..4];
                if p[3] != 0 {
                    self.put(
                        x + ix as i32 - 2,
                        y + iy as i32 - 2,
                        [p[0], p[1], p[2], 255],
                    );
                }
            }
        }
    }
}

fn load_font(dir: &Path, index: u8) -> anyhow::Result<Font> {
    let spec = font_spec(index)?;
    let blob = find(dir, spec.bank)?.context("font bank missing")?;
    Ok(Font::from_lad(spec, &Lad::parse(&blob)?)?)
}

/// Draws image 0 of group 0 of sprite record `record` of `bank` at `(x, y)` (hotspot).
fn draw_sprite(
    dir: &Path,
    cv: &mut Canvas,
    bank: &str,
    record: usize,
    x: i32,
    y: i32,
) -> anyhow::Result<bool> {
    let Some(blob) = find(dir, bank)? else {
        return Ok(false);
    };
    let lad = Lad::parse(&blob)?;
    let Some(rec) = lad.records.get(record) else {
        return Ok(false);
    };
    let Some(img) = rec.groups.first().and_then(|g| g.images.first()) else {
        return Ok(false);
    };
    let Some(layer) = decode_text_image(img) else {
        return Ok(false);
    };
    let mut pal = std::array::from_fn(|i| (i as u16 >> 3) * 0x421);
    if img.flags & rt2_formats::lad::FLAG_PALETTE != 0 {
        let name = palette_name(rec.palette_id());
        if let Some(b) = find(dir, &name)?
            && let Some(p) = PaletteBank::parse(&b)?.palettes.first()
        {
            pal = *p;
        }
    }
    for ly in 0..layer.height {
        for lx in 0..layer.width {
            if let Cell::Index(i) = layer.cells[ly * layer.width + lx] {
                let [r, g, b] = rgb555(pal[usize::from(i)]);
                cv.put(
                    x + layer.x + lx as i32,
                    y + layer.y + ly as i32,
                    [r, g, b, 255],
                );
            }
        }
    }
    Ok(true)
}

/// Renders image 0 of every record of a UI bank in a wrapping row, magenta background.
fn bank_sheet(args: &Args, bank: &str) -> anyhow::Result<()> {
    let blob = find(&args.dir, bank)?.with_context(|| format!("{bank} not found"))?;
    let lad = Lad::parse(&blob)?;
    let (cols, cell) = (8usize, 260usize);
    let rows = lad.records.len().div_ceil(cols);
    let mut cv = Canvas {
        w: cols * cell,
        h: rows * cell,
        px: vec![0; cols * cell * rows * cell * 4],
    };
    for p in cv.px.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[255, 0, 255, 255]);
    }
    let font = load_font(&args.dir, 1)?;
    for i in 0..lad.records.len() {
        let (x, y) = ((i % cols * cell) as i32, (i / cols * cell) as i32);
        cv.rect(x, y, cell as i32, cell as i32, None, [0, 0, 0, 255]);
        draw_sprite(&args.dir, &mut cv, bank, i, x + 4, y + 16)?;
        cv.text(&font, x + 2, y + 2, &i.to_string(), [255, 255, 255]);
    }
    let file = std::fs::File::create(&args.out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), cv.w as u32, cv.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&cv.px)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(bank) = &args.bank_sheet {
        return bank_sheet(&args, bank);
    }
    let blob = find(&args.dir, &args.name)?.with_context(|| format!("{} not found", args.name))?;
    let win = Window::parse(&blob)?;
    if args.dump {
        print!("{}", win.describe());
    }
    if win.width == 0 || win.height == 0 {
        bail!("window has no size");
    }
    let mut cv = Canvas {
        w: SCREEN.0,
        h: SCREEN.1,
        px: vec![0; SCREEN.0 * SCREEN.1 * 4],
    };
    for p in cv.px.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[0x18, 0x1c, 0x24, 255]);
    }
    // Framed dialogs are centred on the screen; full-screen windows start at the origin.
    let (ox, oy) = if win.framed != 0 {
        (
            (SCREEN.0 as i32 - win.width as i32) / 2,
            (SCREEN.1 as i32 - win.height as i32) / 2,
        )
    } else {
        (0, 0)
    };
    cv.rect(
        ox,
        oy,
        win.width as i32,
        win.height as i32,
        Some([0x2c, 0x34, 0x44, 255]),
        [0xd0, 0xd0, 0xe0, 255],
    );
    if win.framed != 0 {
        let (cx, cy, cw, ch) = win.client;
        cv.rect(
            ox + cx as i32,
            oy + cy as i32,
            cw as i32,
            ch as i32,
            None,
            [0x60, 0x90, 0xc0, 255],
        );
    }
    let font = load_font(&args.dir, 1)?;
    let mut controls: Vec<_> = win.controls.iter().collect();
    controls.sort_by_key(|c| c.order);
    let mut drawn = 0;
    for c in controls {
        let (x, y) = (ox + i32::from(c.rect.x), oy + i32::from(c.rect.y));
        let (w, h) = (i32::from(c.rect.w), i32::from(c.rect.h));
        let colour = match c.kind {
            ControlKind::Button => [0xe0, 0xa0, 0x40, 255],
            ControlKind::Label => [0x60, 0xd0, 0x80, 255],
            ControlKind::Image => [0x70, 0x90, 0xf0, 255],
            ControlKind::List => [0xf0, 0x70, 0xb0, 255],
            _ => [0xa0, 0xa0, 0xa0, 255],
        };
        let mut sprite = false;
        if !args.no_sprites && !c.bank.is_empty() {
            let rec = match c.kind {
                ControlKind::Image => c.image_record().map(usize::from),
                ControlKind::Button => c.button_frames().map(|f| usize::from(f.0)),
                _ => None,
            };
            if let Some(rec) = rec {
                sprite = draw_sprite(&args.dir, &mut cv, &c.bank, rec, x, y)?;
                drawn += usize::from(sprite);
            }
        }
        let fill = (!sprite).then_some([colour[0] / 4, colour[1] / 4, colour[2] / 4, 255]);
        cv.rect(x, y, w, h, fill, colour);
        let label = match c.kind {
            ControlKind::Button => match c.button_hotkey() {
                Some(k) if k >= 0x20 => format!("{} [{}]", c.id, k as char),
                _ => c.id.to_string(),
            },
            _ => c.id.to_string(),
        };
        cv.text(&font, x + 2, y + 2, &label, [255, 255, 255]);
    }
    let file = std::fs::File::create(&args.out)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), cv.w as u32, cv.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&cv.px)?;
    println!(
        "{}: {} controls, {drawn} sprites drawn -> {}",
        args.name,
        win.controls.len(),
        args.out.display()
    );
    Ok(())
}
