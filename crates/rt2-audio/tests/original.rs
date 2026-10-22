//! Audio data against an original install. Skipped unless `RT2_DIR` is set; needs no sound card.

use rt2_audio::{
    GameFiles,
    music::{MusicCommand, MusicDirector},
    rng::AudioRng,
    video,
};

fn files() -> Option<GameFiles> {
    GameFiles::from_env()
}

#[test]
fn sfx_bank_loads_every_entry() {
    let Some(files) = files() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let bank = rt2_audio::audio::load_bank(&files).unwrap();
    assert_eq!(bank.len(), 123);
}

#[test]
fn playlist_categories_match_the_install() {
    let Some(files) = files() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let names = files.music_files().unwrap();
    let mut d = MusicDirector::new(names, AudioRng::new(3));
    assert!(d.tracks().len() > 10 && d.tracks().len() <= 60);
    let by_cat = |c: u8| d.tracks().iter().filter(|t| t.category == c).count();
    assert_eq!((by_cat(0), by_cat(2), by_cat(3)), (1, 1, 1));
    assert_eq!(
        d.tracks().iter().find(|t| t.category == 0).unwrap().file,
        "1AIntro.mp3"
    );
    // Every mp3 file is on disk and requests produce playable commands.
    for t in d.tracks() {
        assert!(files.music_dir().join(&t.file).is_file(), "{}", t.file);
    }
    let cmds = d.request(1, 0);
    assert!(matches!(cmds[..], [MusicCommand::Play { .. }]));
}

#[test]
fn every_animation_name_exists() {
    let Some(files) = files() else {
        eprintln!("RT2_DIR not set; skipping");
        return;
    };
    let on_disk: Vec<String> = video::list_animations(&files.anim_dir())
        .unwrap()
        .iter()
        .filter_map(|p| Some(p.file_stem()?.to_string_lossy().to_lowercase()))
        .collect();
    let mut n = 0;
    while let Some(name) = video::animation_name(n + 1) {
        assert!(on_disk.contains(&name.to_lowercase()), "{name}");
        n += 1;
    }
    assert_eq!(n, 104);
    assert_eq!(on_disk.len(), 104);
}
