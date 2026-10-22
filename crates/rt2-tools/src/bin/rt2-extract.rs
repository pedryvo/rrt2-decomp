//! List or extract the entries of a PK2 archive.

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use rt2_formats::pk2::Pk2;

#[derive(Parser)]
#[command(about = "List or extract the entries of a PK2 archive")]
struct Args {
    /// PK2 archive to read
    archive: PathBuf,
    /// Extract every entry into this directory (lists entries when omitted)
    #[arg(short, long)]
    out: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let raw = std::fs::read(&args.archive)
        .with_context(|| format!("reading {}", args.archive.display()))?;
    let pk2 = Pk2::parse(&raw)?;

    if let Some(out) = &args.out {
        std::fs::create_dir_all(out)?;
    }
    for (index, entry) in pk2.entries().iter().enumerate() {
        match &args.out {
            None => println!("{index:5} {:08x} {:10}", entry.hash, entry.size),
            Some(out) => {
                let path = out.join(format!("{index:04}_{:08x}.bin", entry.hash));
                std::fs::write(&path, pk2.contents(entry))
                    .with_context(|| format!("writing {}", path.display()))?;
            }
        }
    }
    Ok(())
}
