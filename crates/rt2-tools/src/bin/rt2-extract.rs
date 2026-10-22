//! List or extract the entries of a PK2 archive.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use rt2_formats::pk2::{Pk2, name_hash};

#[derive(Parser)]
#[command(about = "List or extract the entries of a PK2 archive")]
struct Args {
    /// PK2 archive to read
    archive: PathBuf,
    /// Extract every entry into this directory (lists entries when omitted)
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// File with one candidate filename per line, used to name entries by hash
    #[arg(short, long)]
    names: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let raw = std::fs::read(&args.archive)
        .with_context(|| format!("reading {}", args.archive.display()))?;
    let pk2 = Pk2::parse(&raw)?;

    let mut names = HashMap::new();
    if let Some(path) = &args.names {
        let list =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        for name in list.lines().map(str::trim).filter(|n| !n.is_empty()) {
            names.insert(name_hash(name), name.to_owned());
        }
    }

    if let Some(out) = &args.out {
        std::fs::create_dir_all(out)?;
    }
    for (index, entry) in pk2.entries().iter().enumerate() {
        let name = names.get(&entry.hash).map(String::as_str);
        match &args.out {
            None => println!(
                "{index:5} {:08x} {:10} {}",
                entry.hash,
                entry.size,
                name.unwrap_or("")
            ),
            Some(out) => {
                let file = match name {
                    Some(name) => name.to_owned(),
                    None => format!("{index:04}_{:08x}.bin", entry.hash),
                };
                let path = out.join(file);
                std::fs::write(&path, pk2.contents(entry))
                    .with_context(|| format!("writing {}", path.display()))?;
            }
        }
    }
    Ok(())
}
