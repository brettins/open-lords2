//! Validates the decoders against a real game install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -- --nocapture
//! ```
//!
//! Skips (rather than fails) when unset, so the suite still runs on a machine
//! without the game. No assets live in this repository.

use l2_formats::{Palette, Pl8, Storage};
use std::{collections::BTreeMap, env, fs, path::Path};

/// Files that use a supported storage mode but still do not decode cleanly.
/// These are unexplained, not excused - the list exists so that any *new*
/// failure fails the build, and so that shrinking it is visible progress.
///
/// Observed structure, recorded for whoever picks this up:
///   * overshoot by exactly 24 bytes: Base2a, Roads2a, Castle2a, Town2a, Town2b-d
///   * overshoot by exactly 840 (24 * 35): Castle1a-d, Town1a-d
///   * undershoot: Fntl2_14 (6), Font_10 (10), T16_bat1 (61), T32_bat (190)
///   * row overrun: Font_c2
const KNOWN_FAILING: &[&str] = &[
    "Base2a.pl8", "Castle1a.pl8", "Castle1b.pl8", "Castle1c.pl8", "Castle1d.pl8",
    "Castle2a.pl8", "Castle2b.pl8", "Castle2c.pl8", "Castle2d.pl8", "Fntl2_14.pl8",
    "Font_10.pl8", "Font_c2.pl8", "Roads2a.pl8", "T16_bat1.pl8", "T32_bat.pl8",
    "Town1a.pl8", "Town1b.pl8", "Town1c.pl8", "Town1d.pl8", "Town2a.pl8",
    "Town2b.pl8", "Town2c.pl8", "Town2d.pl8",
];

/// Files that validate today. Must not regress.
const VALIDATED_BASELINE: usize = 236;

fn asset_dir() -> Option<String> {
    env::var("LORDS2_DIR").ok().filter(|d| Path::new(d).is_dir())
}

fn files_with_ext(dir: &str, ext: &str) -> Vec<std::path::PathBuf> {
    let mut out: Vec<_> = fs::read_dir(dir)
        .expect("read asset dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn pl8_corpus_validates() {
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set or not a directory - skipping corpus test");
        return;
    };

    let mut by_mode: BTreeMap<String, usize> = BTreeMap::new();
    let (mut ok, mut undecoded, mut frames) = (0usize, 0usize, 0usize);
    let mut failures: Vec<(String, String)> = Vec::new();

    for path in files_with_ext(&dir, "pl8") {
        let bytes = fs::read(&path).expect("read pl8");
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let pl8 = match Pl8::parse(&bytes) {
            Ok(p) => p,
            Err(e) => {
                failures.push((name, format!("parse failed: {e}")));
                continue;
            }
        };

        let key = match pl8.storage {
            Storage::Raw => format!("raw:{}", pl8.zoom),
            Storage::Rle => format!("rle:{}", pl8.zoom),
            Storage::Isometric => format!("iso:{}", pl8.zoom),
            Storage::Unknown(m) => format!("mode{m}:{}", pl8.zoom),
        };
        *by_mode.entry(key).or_default() += 1;

        if !pl8.is_supported() {
            undecoded += 1;
            continue;
        }

        match pl8.validate() {
            Ok(()) => {
                ok += 1;
                frames += pl8.frames.len();
            }
            Err(e) => failures.push((name, e.to_string())),
        }
    }

    println!("PL8 corpus: {ok} validated ({frames} frames), {undecoded} in undecoded storage modes, {} failing", failures.len());
    println!("storage modes: {by_mode:?}");

    let unexpected: Vec<_> = failures
        .iter()
        .filter(|(name, _)| !KNOWN_FAILING.contains(&name.as_str()))
        .collect();
    for (name, why) in &unexpected {
        println!("  UNEXPECTED FAILURE {name}: {why}");
    }

    let fixed: Vec<_> = KNOWN_FAILING
        .iter()
        .filter(|k| !failures.iter().any(|(n, _)| n == *k))
        .collect();
    if !fixed.is_empty() {
        println!("  now passing (remove from KNOWN_FAILING): {fixed:?}");
    }

    assert!(unexpected.is_empty(), "{} newly failing file(s)", unexpected.len());
    assert!(
        ok >= VALIDATED_BASELINE,
        "coverage regressed: {ok} validated, baseline {VALIDATED_BASELINE}"
    );
}

#[test]
fn palettes_are_768_bytes_of_6bit_vga() {
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping palette test");
        return;
    };

    // 63 must scale to a true 255, not the 252 a naive << 2 would give.
    assert_eq!(Palette::from_bytes(&[63u8; 768]).unwrap().rgb(0), [255, 255, 255]);
    assert!(Palette::from_bytes(&[0u8; 767]).is_err());

    let mut count = 0;
    for path in files_with_ext(&dir, "256") {
        let bytes = fs::read(&path).expect("read palette");
        assert!(
            bytes.iter().all(|&v| v <= 63),
            "{:?} has values above the 6-bit VGA range",
            path.file_name().unwrap()
        );
        Palette::from_bytes(&bytes).expect("parse palette");
        count += 1;
    }
    println!("validated {count} palette files");
    assert!(count > 0, "no .256 palettes found");
}
