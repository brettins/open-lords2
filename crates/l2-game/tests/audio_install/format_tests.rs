#![allow(unused_imports)]
use super::*;
use super::track_tests::*;
use super::voice_tests::*;
use std::path::{Path, PathBuf};
use l2_game::audio::{names, track, wav};

#[test]
fn every_shipped_sound_is_11khz_8_bit_pcm_or_is_one_of_the_two_we_never_open() {
    let dir = l2_testkit::install!();
    let files = every_wav(&dir);
    assert!(files.len() > 700, "only {} wav files - is this an install?", files.len());

    let mut checked = 0;
    let mut skipped = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_ascii_lowercase();
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if len as usize > wav::MAX_BYTES {
            // `PUMKIN.WAV` and `PUMKIN2.WAV`, 160 MB each. The original never
            // plays them either - `FUN_004AEF7E` opens one only to measure it.
            skipped.push(name);
            continue;
        }
        let bytes = std::fs::read(path).expect("readable");
        let sound = wav::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        if name == "bp180_4.wav" {
            // **The one file in 771 that is not 11 kHz.** A Bishop line for
            // `L2.eng` group 180, mastered at 44,100 Hz — 403,764 bytes where
            // its three siblings are 80-119 kB. At its own rate it is 9.2
            // seconds, and theirs are 7.3, 9.5 and 10.8, so it is the same
            // take at four times the resolution: a
            // slip in whatever batch-converted the voice sessions.
            assert_eq!(sound.rate, 44_100, "{name} stopped being the outlier");
            checked += 1;
            continue;
        }
        assert_eq!(sound.rate, 11_025, "{name} is not 11 kHz");
        assert!(sound.channels == 1 || sound.channels == 2, "{name}: {} channels", sound.channels);
        assert!(sound.frames() > 0, "{name} has no samples");
        checked += 1;
    }
    skipped.sort();
    assert_eq!(skipped, vec!["pumkin.wav", "pumkin2.wav"], "an unexpected huge wav");
    eprintln!("{checked} sounds decoded, {} left unopened", skipped.len());
}

#[test]
fn the_music_is_stereo_and_the_voices_are_mono() {
    let dir = l2_testkit::install!();
    for n in 1..=5u8 {
        for name in [format!("scroll{n}.wav"), format!("battle{n}.wav")] {
            let p = find(&dir, &name).unwrap_or_else(|| panic!("{name} missing"));
            let s = wav::decode(&std::fs::read(&p).unwrap()).unwrap();
            assert_eq!(s.channels, 2, "{name}");
        }
    }
    let p = find(&dir, "kt170_1.wav").expect("a lord voice");
    assert_eq!(wav::decode(&std::fs::read(&p).unwrap()).unwrap().channels, 1);
}

#[test]
fn the_shipped_sounds_the_executable_never_names_are_the_eighteen_we_wrote_down() {
    let exe = l2_testkit::executable!();
    let dir = l2_testkit::install!();
    let hay = String::from_utf8_lossy(&exe).to_ascii_lowercase();
    let mut orphans: Vec<String> = every_wav(&dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_ascii_lowercase())
        .filter(|n| !hay.contains(n.as_str()))
        .collect();
    orphans.sort();
    let expected = [
        "arch_e5.wav",
        "bathit1.wav",
        "boilguy1.wav",
        "boilguy2.wav",
        "boilwood.wav",
        "bp100_3.wav",
        "dest_fld.wav",
        "ff_capt2.wav",
        "ff_msg1.wav",
        "ff_win.wav",
        "knig_e3.wav",
        "s017_02.wav",
        "s017_03.wav",
        "s071_07.wav",
        "s072_01.wav",
        "s072_02.wav",
        "s115_02.wav",
        "supply.wav",
    ];
    assert_eq!(orphans, expected, "the set of unreferenced sounds moved");
}

