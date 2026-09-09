//! **The audio layer against a real install.** Headless: no device is opened
//! and no sound is made, so this runs on CI's runner exactly as it runs here —
//! by skipping, because CI has no install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_install
//! ```
//!
//! What these check is the pair of claims the rest of `src/audio` rests on:
//!
//! * **every name the recovered tables hold is a file that ships**, so a track
//!   or a bank slot cannot silently resolve to nothing;
//! * **every sound the game plays is PCM at 11,025 Hz, 8-bit**, which is what
//!   `wav.rs` was allowed to be as small as it is because of.
//!
//! Both are corpus checks over all 771 files rather than over one that
//! happened to work — `docs/decisions.md` C1.

use std::path::{Path, PathBuf};

use l2_game::audio::{names, track, wav};

/// The install is inconsistent about casing — `Scroll1.wav` on disk,
/// `scroll1.wav` in `Lords2.exe`'s tables — so every lookup here is
/// case-insensitive, the same rule the mod overlay applies.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).find_map(|e| {
        let p = e.path();
        let n = p.file_name()?.to_str()?.to_ascii_lowercase();
        (n == want).then_some(p)
    })
}

fn every_wav(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav"))
        })
        .collect();
    v.sort();
    v
}

#[test]
fn every_track_the_ladder_can_choose_is_in_the_install() {
    let dir = l2_testkit::install!();
    // Both ladders, exhaustively: the five campaign tracks a share of the map
    // can reach and the four a battle can. A missing one is silence at exactly
    // the moment the game gets interesting.
    for share in [0, 7, 8, 14, 15, 28, 29, 42, 43, 100] {
        let m = track::campaign(4, share);
        assert!(find(&dir, m.file()).is_some(), "{share}% -> {} is missing", m.file());
    }
    let mut cycle = track::BattleCycle::default();
    for _ in 0..4 {
        for kind in [track::BattleKind::Field, track::BattleKind::Siege] {
            let m = cycle.next(kind);
            assert!(find(&dir, m.file()).is_some(), "{:?} -> {} is missing", kind, m.file());
        }
    }
}

#[test]
fn the_two_sample_banks_ship_apart_from_the_hole_in_both() {
    let dir = l2_testkit::install!();
    for (label, bank) in
        [("kingdom", &names::KINGDOM_BANK[..]), ("battle", &names::BATTLE_BANK[..])]
    {
        for (i, name) in bank.iter().enumerate() {
            if *name == "null.wav" {
                // Slot 1 of both banks. `null.wav` is not in the install and
                // never was: the original loads a bank of a fixed size and
                // this is how it spells an unused slot.
                assert!(find(&dir, name).is_none(), "null.wav actually exists?");
                continue;
            }
            assert!(find(&dir, name).is_some(), "{label} bank slot {i} ({name}) is missing");
        }
    }
}

#[test]
fn all_448_lord_voices_exist_and_the_convention_generates_them() {
    let dir = l2_testkit::install!();
    let mut found = 0;
    for group in 170..=197u16 {
        for variant in 0..16u8 {
            let name = names::lord_voice(group, variant).expect("in range");
            assert!(find(&dir, &name).is_some(), "{name} is missing");
            found += 1;
        }
    }
    // 28 groups x 4 lords x 4 takes. If the convention were wrong this would
    // fail on the first file rather than here, but the count is the claim.
    assert_eq!(found, 448);
}

#[test]
fn the_message_fanfares_ship() {
    let dir = l2_testkit::install!();
    for name in [
        names::fanfare::MESSAGE,
        names::fanfare::CAPTURED,
        names::fanfare::BATTLE,
        names::fanfare::AFTER_BATTLE,
    ] {
        assert!(find(&dir, name).is_some(), "{name} is missing");
    }
    // The finding this test exists to keep: `Ff_win.wav` ships and
    // `Battle_ReturnToCampaign` plays `ff_lose.wav` at both of its two sites.
    // The file is here; nothing in `Lords2.exe` names it.
    assert!(find(&dir, "ff_win.wav").is_some(), "ff_win.wav ships even so");
}

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
        // Only the header is needed and reading 771 whole files is 36 MB, so
        // read enough for `fmt ` and let the reader see a short `data`.
        let bytes = std::fs::read(path).expect("readable");
        let sound = wav::decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        if name == "bp180_4.wav" {
            // **The one file in 771 that is not 11 kHz.** A Bishop line for
            // `L2.eng` group 180, mastered at 44,100 Hz — 403,764 bytes where
            // its three siblings are 80-119 kB. At its own rate it is 9.2
            // seconds, and theirs are 7.3, 9.5 and 10.8, so it is the same
            // take at four times the resolution rather than a wrong file: a
            // slip in whatever batch-converted the voice sessions.
            //
            // It is only harmless because the resampler is per sound. A mixer
            // that assumed 11 kHz - which every other file would have let it
            // get away with - would play this one at a quarter speed.
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
    // Not a rule anything depends on, but the thing that makes the mixer's
    // mono-to-stereo path load-bearing: if every file were stereo it would
    // never run and would rot.
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

/// **End to end from the England turn-one save**: the music the player would
/// actually hear on the position the game starts from.
///
/// The two clauses of the ladder both answer `Scroll1` here — one county of
/// fourteen is 7 % — so this cannot separate them, and
/// `track::tests::the_last_county_is_scroll1_whatever_share_that_is` is what
/// does. What this pins is that the *inputs* come out of a real save with the
/// values the ladder expects, which is the half a pure test cannot reach.
#[test]
fn england_turn_one_would_play_scroll1() {
    let save = l2_testkit::england!();
    let game = l2_game::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT)
        .expect("the fixture loads");
    let realm = &game.kingdom.realms[game.player as usize];
    assert_eq!(realm.county_count, 1, "the England start is one county");
    assert_eq!(game.kingdom.county_count, 14);

    // Through the real entry point, on a real machine, from the real save.
    let machine = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Campaign);
    let scene = l2_game::audio::scene(&machine, &game);
    assert_eq!(
        scene,
        l2_game::audio::Scene::Campaign { county_count: 1, share_of_map_pct: 7 },
        "PctOf(1, 14)"
    );
    assert_eq!(track::campaign(1, 7), track::Music::Scroll(1));

    // And the reason `scene` computes the share instead of reading it: the
    // importer leaves the derived field at zero until a turn has been ended.
    // If this ever stops being 0 the recompute still agrees with it, so this
    // is a note rather than a requirement - but it is why the note exists.
    assert_eq!(realm.share_of_map_pct, 0, "the derived field is not populated on import");

    // The front end, off the bottom of the stack, is silent.
    let front = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Setup(
        l2_game::screens::setup::SetupPage::Title,
    ));
    assert_eq!(l2_game::audio::scene(&front, &game), l2_game::audio::Scene::FrontEnd);
}

/// **The mixer against a real 3.8 MB track.** Everything but the sound card:
/// the file is decoded, resampled to a 48 kHz device and mixed, and the result
/// has to be audible and finite.
///
/// The failure this exists for is silence — a resampler that steps by zero, a
/// bias that centres 8-bit audio on 0 instead of 128, a gain shifted the wrong
/// way. All three produce a buffer of zeroes and no error anywhere.
#[test]
fn a_real_track_comes_out_of_the_mixer_audible() {
    let dir = l2_testkit::install!();
    let path = find(&dir, "scroll1.wav").expect("scroll1.wav");
    let sound = wav::decode(&std::fs::read(&path).unwrap()).expect("decodes");
    // 3.8 MB of 11 kHz stereo is a bit under three minutes. If this is wildly
    // off, the header was misread.
    let seconds = sound.frames() as f64 / sound.rate as f64;
    assert!((100.0..300.0).contains(&seconds), "scroll1 is {seconds:.0}s");

    let mut m = l2_game::audio::mixer::Mixer::new(48_000);
    m.set_music("scroll1.wav".into(), std::sync::Arc::new(sound));
    assert_eq!(m.music_name(), Some("scroll1.wav"));

    // A tenth of a second of output.
    let mut buf = vec![0f32; 48_000 / 10 * 2];
    m.fill(&mut buf);
    assert!(buf.iter().all(|s| s.is_finite() && (-1.0..=1.0).contains(s)), "out of range");
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "the first tenth of a second is silent (peak {peak})");

    // And it is still playing a minute in, which exercises the cursor and the
    // loop rather than only the first buffer.
    for _ in 0..600 {
        m.fill(&mut buf);
    }
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "went silent partway through (peak {peak})");
}

#[test]
fn the_shipped_sounds_the_executable_never_names_are_the_eighteen_we_wrote_down() {
    let exe = l2_testkit::executable!();
    let dir = l2_testkit::install!();
    // A byte scan of the executable for each shipped file name. The claim in
    // `names.rs` is 753 of 771 named and 18 not; the eighteen are worth
    // pinning because `Ff_win.wav` is among them and that is a bug in a
    // shipped game, not a gap in this port.
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
