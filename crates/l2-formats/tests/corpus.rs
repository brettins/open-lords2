//! Validates the decoders against a real game install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -- --nocapture
//! ```
//!
//! Skips (rather than fails) when unset, so the suite still runs on a machine
//! without the game. No assets live in this repository.

use l2_formats::{Palette, Pl8, Shape, Storage};
use std::{collections::BTreeMap, fs};

/// Files that use a supported encoding but still do not decode cleanly.
///
/// **Empty, and it should stay that way.** All 291 files decode. This list is
/// kept as the mechanism, not as a bucket: any new failure fails the build with
/// its name, and anything added here needs a reason recorded alongside it.
const KNOWN_FAILING: &[&str] = &[];

/// Files that validate today. Must not regress.
const VALIDATED_BASELINE: usize = 291;

fn asset_dir() -> Option<String> {
    l2_testkit::install_dir().map(|d| d.display().to_string())
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
        l2_testkit::skip!("LORDS2_DIR not set or not a directory - skipping corpus test");
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
        l2_testkit::skip!("LORDS2_DIR not set - skipping palette test");
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

/// The end-offset invariant proves we consume the right *bytes*. It cannot
/// prove we produce the right *pixels* - a decoder that validates perfectly
/// while emitting entirely blank frames would pass it. This checks that decoded
/// frames actually contain something, per storage family.
#[test]
fn decoded_frames_are_not_blank() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };

    let mut stats: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for path in files_with_ext(&dir, "pl8") {
        let bytes = fs::read(&path).expect("read pl8");
        let Ok(pl8) = Pl8::parse(&bytes) else { continue };
        if !pl8.is_supported() || pl8.validate().is_err() {
            continue;
        }
        let family = match pl8.storage {
            Storage::Raw => "raw",
            Storage::Rle => "rle",
            Storage::Isometric => "iso",
            Storage::Unknown(_) => continue,
        };
        for i in 0..pl8.frames.len() {
            // Region maps are never painted, so blankness is correct for them.
            if pl8.frames[i].shape == l2_formats::Shape::RegionMap {
                continue;
            }
            let Ok(f) = pl8.decode(i) else { continue };
            let e = stats.entry(family).or_default();
            e.0 += 1;
            if f.opaque.iter().any(|&o| o) {
                e.1 += 1;
            }
        }
    }

    for (family, (total, painted)) in &stats {
        let pct = 100.0 * *painted as f64 / *total as f64;
        println!("{family}: {painted}/{total} frames paint at least one pixel ({pct:.1}%)");
    }

    // Blank frames are legitimate (empty terrain, animation padding), but a
    // family that is almost entirely blank means the decoder is broken.
    for (family, (total, painted)) in &stats {
        assert!(*total > 0, "{family}: no frames decoded");
        assert!(
            *painted * 2 > *total,
            "{family}: only {painted} of {total} frames paint anything - decoder likely broken"
        );
    }
}

/// Pins two counts that an audit found had drifted in the documentation.
///
/// Both were stated in prose, carried across a scope boundary, and restated
/// wrongly elsewhere - "24 frames" for what is 32, and "15 files" for what is
/// 18. Prose cannot defend a number; a test can. If either figure changes, the
/// documentation quoting it is now wrong and this fails until both are fixed.
#[test]
fn counts_that_the_documentation_quotes_still_hold() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };

    let (mut shape1_with_rows, mut non_iso_family_with_iso_frames) = (0usize, 0usize);
    for path in files_with_ext(&dir, "pl8") {
        let bytes = fs::read(&path).expect("read pl8");
        let Ok(pl8) = Pl8::parse(&bytes) else { continue };
        let mut has_iso = false;
        for f in &pl8.frames {
            if matches!(
                f.shape,
                Shape::Diamond | Shape::DiamondFull | Shape::DiamondLeft | Shape::DiamondRight
            ) {
                has_iso = true;
                if f.shape == Shape::Diamond && f.overhang_rows > 0 {
                    shape1_with_rows += 1;
                }
            }
        }
        if has_iso && pl8.storage != Storage::Isometric {
            non_iso_family_with_iso_frames += 1;
        }
    }

    println!("shape-1 frames declaring overhang rows: {shape1_with_rows}");
    println!("files holding iso frames outside family 2: {non_iso_family_with_iso_frames}");
    assert_eq!(shape1_with_rows, 32, "docs say 32 - Batlfix2 24 plus 2 each in Town1a-d");
    assert_eq!(non_iso_family_with_iso_frames, 18, "docs say 18 iso files outside family 2");
}

/// A rectangle that declares overhang rows reserves them whether or not it
/// stores them.
///
/// This is the contract every caller of `decode` relies on, and it was broken
/// in a way nothing here could see: the canvas height was decided on the *byte
/// span*, so a frame that stored its rows came back `h + rows` tall and a frame
/// that declared the same rows and stored none came back `h` tall. The two body
/// fonts sit on opposite sides of that split — `Fntl2_14.pl8` stores 47 blocks
/// of overhang rows (45 of them nothing but `00 <width>`, one skip run covering
/// a wholly transparent row) while `Fntl2_9.pl8` declares exactly the same
/// counts and stores nothing at all — so no single rule at the call site could
/// be right about both. `Fntl2_14`'s `rows = 3` glyphs drew three pixels low on
/// every management screen until a player compared our text with the original's
/// and named the letters.
///
/// The invariant is cheap, file-wide and has no opinion about pixels, which is
/// what makes it worth pinning.
#[test]
fn a_rectangle_reserves_the_rows_it_declares_whether_or_not_it_stores_them() {
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };

    let (mut checked, mut declaring) = (0usize, 0usize);
    for path in files_with_ext(&dir, "pl8") {
        let bytes = fs::read(&path).expect("read pl8");
        let Ok(pl8) = Pl8::parse(&bytes) else { continue };
        // RLE files have no bare rectangle for the rows to sit above, and
        // shape 1 ignores the count outright - both are settled elsewhere.
        if pl8.storage == Storage::Rle {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        for (i, f) in pl8.frames.iter().enumerate() {
            if f.shape != Shape::Rect {
                continue;
            }
            let Ok(decoded) = pl8.decode(i) else { continue };
            checked += 1;
            if f.overhang_rows > 0 {
                declaring += 1;
            }
            assert_eq!(
                decoded.height,
                f.height + f.overhang_rows as u16,
                "{name} frame {i}: a shape-0 canvas is the record's height plus \
                 its declared overhang rows, stored or not",
            );
            assert_eq!(decoded.width, f.width, "{name} frame {i}: width is untouched");
        }
    }

    println!("shape-0 rectangles checked: {checked}, of which declaring rows: {declaring}");
    // Both sides of the split must be present, or the assertion above passes
    // vacuously.
    assert!(declaring >= 100, "expected both fonts' worth of declared rows, saw {declaring}");
}
