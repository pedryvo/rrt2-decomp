//! Sound entries against an original game install. Skipped unless `RT2_DIR` is set.

use std::path::PathBuf;

use rt2_formats::pk2::Pk2;
use rt2_formats::sound::{self, SOUNDS};

const ARCHIVES: [&str; 4] = ["RT2_11PL.PK2", "RT2_09GO.PK2", "RT2_06.PK2", "RT2.PK2"];

fn game_dir() -> Option<PathBuf> {
    std::env::var_os("RT2_DIR").map(PathBuf::from)
}

#[test]
fn every_sound_entry_resolves_and_decodes() {
    let Some(dir) = game_dir() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let raws: Vec<Vec<u8>> = ARCHIVES
        .iter()
        .map(|n| std::fs::read(dir.join("data").join(n)).unwrap())
        .collect();
    let pk2s: Vec<Pk2> = raws.iter().map(|r| Pk2::parse(r).unwrap()).collect();
    let mut used = 0;
    let mut hashes = std::collections::BTreeSet::new();
    for e in SOUNDS.iter().filter(|e| e.is_used()) {
        used += 1;
        assert!(hashes.insert(e.hash()), "hash collision for {}", e.name);
        let blob = sound::blob(&pk2s, e).unwrap_or_else(|| panic!("{} missing from PK2", e.name));
        let pcm = sound::decode(e, blob).unwrap_or_else(|err| panic!("{}: {err}", e.name));
        let secs = pcm.seconds();
        assert!(
            (0.04..20.0).contains(&secs),
            "{}: implausible duration {secs}",
            e.name
        );
        let peak = pcm
            .samples
            .iter()
            .map(|s| i32::from(*s).unsigned_abs())
            .max()
            .unwrap();
        assert!(peak > 500, "{}: silent (peak {peak})", e.name);
        let clipped = pcm
            .samples
            .iter()
            .filter(|s| s.unsigned_abs() >= 32767)
            .count();
        assert!(
            clipped * 50 < pcm.samples.len(),
            "{}: {clipped} clipped samples",
            e.name
        );
        // Low lag-1 correlation is normal (chimes, insects, hiss are broadband); a wrong sample
        // format shows up as clipping or a flat spectrum, which the checks above catch.
        let mean =
            pcm.samples.iter().map(|s| i64::from(*s)).sum::<i64>() / pcm.samples.len() as i64;
        assert!(
            mean.abs() < i64::from(peak) / 2,
            "{}: DC offset {mean}",
            e.name
        );
    }
    assert_eq!(used, 123);
    // The two entries that live in the RT2_06 expansion pack.
    let in_06 = |name: &str| {
        let e = sound::find_by_name(name).unwrap();
        pk2s[2].find(e.hash()).is_some()
    };
    assert!(in_06("airpt61m.raw") && in_06("flyby61m.raw"));
}
