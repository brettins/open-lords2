#![allow(unused_imports)]
use super::*;
use super::asset_tests::*;
use super::*;
use super::corpus::*;
use super::render::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// **The campaign map's walk tables are the bytes in the user's own
/// `Lords2.exe`** — `Map_DrawArmies` (`0x00408438`) reads `[facing * 16 +
/// +0x149]` out of `0x004D8108`/`0x004D8188` at the near zoom and
/// `0x004D8308`/`0x004D8388` at the far one, as `i8`.
///; this is what says that copy was the user's, all 256
#[test]
fn the_campaign_walk_tables_are_the_bytes_in_the_binary() {
    use l2_view::campaign;
    let exe = l2_testkit::executable!();
    for (zoom, xs, ys) in [(campaign::NEAR, 0x004D_8108u32, 0x004D_8188u32), (campaign::FAR, 0x004D_8308, 0x004D_8388)] {
        let (tx, ty) = (l2_testkit::pe::Table::at(&exe, xs), l2_testkit::pe::Table::at(&exe, ys));
        for facing in 0..8u8 {
            for step in 0..16u8 {
                let i = facing as usize * 16 + step as usize;
                let want = (tx.u8_at(i) as i8 as i32, ty.u8_at(i) as i8 as i32);
                assert_eq!(
                    campaign::walk_offset(&zoom, facing, step),
                    want,
                    "zoom {}, facing {facing}, +0x149 = {step}: {:#010X} / {:#010X}",
                    zoom.id,
                    xs,
                    ys
                );
            }
        }
    }
}

/// `walk_offset` is a formula standing in for an 8 x 17 table of `(i32, i32)`
/// pairs at `0x004E4030`. This reads that table out of the binary and checks
/// the formula reproduces all 136 entries exactly.
///
/// Width matters here and is easy to get wrong: the entries are pairs of
/// **i32**, which the 0x88-byte per-facing stride confirms — 17 entries times
/// 8 bytes — and which the neighbouring 16-pixel table at `0x004E3BF0` confirms
/// again, sitting exactly `8 * 0x88` bytes earlier.
#[test]
fn the_walk_offsets_match_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    const TABLE_VA: u32 = 0x004E_4030;
    const STRIDE: u32 = 0x88;
    let Some(base) = va_to_offset(&exe, TABLE_VA) else {
        panic!("0x{TABLE_VA:08X} is not inside any initialised section");
    };

    let read_i32 = |off: usize| i32::from_le_bytes(exe[off..off + 4].try_into().unwrap());
    let mut compared = 0;
    for facing in 0..8u32 {
        for walking in 0..17u32 {
            let off = base + (facing * STRIDE + walking * 8) as usize;
            let want = (read_i32(off), read_i32(off + 4));
            let got = figures::walk_offset(facing as u8, walking as u8);
            assert_eq!(got, want, "facing {facing}, walking {walking}");
            compared += 1;
        }
    }
    assert_eq!(compared, 8 * 17);

    assert_eq!(figures::walk_offset(0, 1), (0, 30));
    eprintln!("walk offsets: {compared} entries match the binary");
}

#[test]
fn every_campaign_tile_bank_matches_the_pitch_the_binary_steps_by() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let mut checked = 0;
    let mut seen: Vec<&str> = Vec::new();
    for zoom in campaign::ZOOMS {
        for set in zoom.banks {
            for name in set {
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name);
                let Some(bytes) = read(&dir, name) else {
                    panic!("{name} is not in the install");
                };
                let pl8 = l2_formats::Pl8::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(!pl8.frames.is_empty(), "{name} has no frames");
                for (i, f) in pl8.frames.iter().enumerate() {
                    assert_eq!(
                        f.width as i32, zoom.tile_w,
                        "{name} frame {i} is {} wide, but zoom {} steps by {}",
                        f.width, zoom.id, zoom.pitch
                    );
                    assert_eq!(f.height as i32, zoom.tile_h, "{name} frame {i}");
                    checked += 1;
                }
                assert_eq!(zoom.pitch, zoom.tile_w + 2);
                assert_eq!(zoom.row_step, zoom.tile_h / 2);
            }
        }
    }
    assert_eq!(seen.len(), 25, "the distinct bank files across both zooms");
    assert_eq!(checked, 4 * (140 + 25 + 140 + 61 + 100) + (140 + 25 + 140 + 61 + 100));
    eprintln!("campaign tiles: {checked} frames in {} files", seen.len());
}

#[test]
fn the_four_seasons_of_a_bank_are_the_same_frame_table() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let zoom = campaign::NEAR;
    let mut frames_checked = 0;
    let mut anchors_checked = 0;
    let mut row_differences = Vec::new();
    for bank in 0..5 {
        let files: Vec<_> = (0..campaign::SEASONS)
            .map(|s| {
                let name = zoom.banks[s][bank];
                let bytes = read(&dir, name).unwrap_or_else(|| panic!("{name} is not installed"));
                (name, bytes)
            })
            .collect();
        let parsed: Vec<_> = files
            .iter()
            .map(|(n, b)| (*n, l2_formats::Pl8::parse(b).unwrap_or_else(|e| panic!("{n}: {e}"))))
            .collect();
        let (spring_name, spring) = &parsed[0];
        for (name, other) in &parsed[1..] {
            assert_eq!(
                other.frames.len(),
                spring.frames.len(),
                "{name} has {} frames and {spring_name} has {}",
                other.frames.len(),
                spring.frames.len()
            );
            for (i, (a, b)) in spring.frames.iter().zip(other.frames.iter()).enumerate() {
                assert_eq!(
                    (a.x, a.y),
                    (b.x, b.y),
                    "{name} frame {i} sits at a different place on the sheet than {spring_name}'s"
                );
                anchors_checked += 1;
                assert_eq!((a.width, a.height), (b.width, b.height), "{name} frame {i} size");
                if a.overhang_rows != b.overhang_rows {
                    row_differences.push((*name, i, a.overhang_rows, b.overhang_rows));
                }
                frames_checked += 1;
            }
        }
    }
    assert_eq!(anchors_checked, 3 * (140 + 25 + 140 + 61 + 100), "every frame of every bank");
    assert_eq!(frames_checked, anchors_checked);

    let mut differing: Vec<usize> =
        row_differences.iter().map(|(_, i, _, _)| *i).collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    differing.sort_unstable();
    assert_eq!(
        differing,
        vec![109, 111, 113, 114, 115, 116, 117, 118, 119],
        "the only per-season structural differences should be nine Roads frames"
    );
    assert!(
        row_differences.iter().all(|(n, ..)| n.starts_with("Roads1")),
        "an overhang difference outside the roads bank: {row_differences:?}"
    );
    for (_, i, ..) in &row_differences {
        let base = (i / 4) * 4;
        assert!(
            (108..=120).contains(&base),
            "frame {i} differs by season but is not in a crop block"
        );
    }
    assert!(
        row_differences.iter().all(|(_, _, a, b)| a.abs_diff(*b) <= 2),
        "an overhang differs by more than two rows: {row_differences:?}"
    );
    eprintln!(
        "seasons: {frames_checked} frames compared, {} overhang differences, 0 index moves",
        row_differences.len()
    );
}

#[test]
fn the_far_zoom_names_one_season_four_times_and_the_unused_files_do_not_match() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    for season in 1..=campaign::SEASONS {
        assert_eq!(
            campaign::FAR.banks[season - 1],
            campaign::FAR.banks[0],
            "season {season} of the far zoom should name spring's files"
        );
    }
    let a = read(&dir, "Town2a.pl8").expect("Town2a.pl8");
    let b = read(&dir, "Town2b.pl8").expect("Town2b.pl8");
    let (a, b) = (l2_formats::Pl8::parse(&a).unwrap(), l2_formats::Pl8::parse(&b).unwrap());
    assert_eq!(a.frames.len(), 61, "the live far-zoom town bank");
    assert_eq!(b.frames.len(), 94, "the dead one is a different sheet entirely");
    eprintln!("far zoom: Town2a has 61 frames, the unused Town2b has 94");
}

#[test]
fn the_minimap_realm_ramp_matches_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let Some(base) = va_to_offset(&exe, chrome::MINIMAP_REALM_RAMP_VA) else {
        panic!("0x{:08X} is not inside any initialised section", chrome::MINIMAP_REALM_RAMP_VA);
    };
    for (colour, want) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        let rec = &exe[base + colour * 8..base + colour * 8 + 8];
        assert_eq!(&rec[..4], want, "realm colour {colour}");
        assert_eq!(rec[4], chrome::MINIMAP_SELECTED, "realm colour {colour} selected entry");
        assert_eq!(&rec[5..], &want[1..], "realm colour {colour} tail");
    }
    eprintln!("minimap ramp: 6 realm colours match the binary");
}

#[test]
fn the_minimap_rating_ramp_matches_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    assert_eq!(
        chrome::MINIMAP_RATING_RAMP_VA + 8,
        chrome::MINIMAP_REALM_RAMP_VA,
        "the two ramps are adjacent, and that is why they get confused"
    );
    let Some(base) = va_to_offset(&exe, chrome::MINIMAP_RATING_RAMP_VA) else {
        panic!("0x{:08X} is not inside any initialised section", chrome::MINIMAP_RATING_RAMP_VA);
    };
    assert_eq!(
        &exe[base..base + chrome::MINIMAP_RATING_RAMP.len()],
        &chrome::MINIMAP_RATING_RAMP,
        "the six rating colours"
    );
    eprintln!("minimap ramp: 6 rating colours match the binary");
}

#[test]
fn the_rating_ramp_is_the_colour_bar_the_legend_strip_draws() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        l2_testkit::skip!("Misc_cty.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");
    let frame = pl8
        .decode(l2_view::chrome::misc_cty::MINIMAP_SIDE_ACTIVE)
        .expect("frame 0x5B decodes");
    assert_eq!((frame.width, frame.height), (29, 123), "the 29 x 123 mode strip");

    let w = frame.width as usize;
    let mut runs: Vec<(u8, usize)> = Vec::new();
    for y in 0..frame.height as usize {
        let v = frame.indices[y * w + 5];
        match runs.last_mut() {
            Some((c, n)) if *c == v => *n += 1,
            _ => runs.push((v, 1)),
        }
    }
    let bar: Vec<u8> = runs.iter().filter(|(_, n)| *n >= 10).map(|(c, _)| *c).collect();
    let mut want = chrome::MINIMAP_RATING_RAMP;
    want.reverse();
    assert_eq!(bar, want, "the legend bar is the rating ramp, best first");
    eprintln!("minimap legend: frame 0x5B's colour bar is the rating ramp reversed");
}

#[test]
fn every_used_map_slot_has_a_minimap_and_every_empty_one_does_not() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(maps) = read(&dir, "L2_maps.dat") else {
        l2_testkit::skip!("L2_maps.dat not present - skipping");
    };
    let set = l2_formats::MapSet::parse(&maps).expect("L2_maps.dat parses");
    let used = set.used_slots();

    let mut with_minimap = 0;
    for slot in 0..60usize {
        let name = chrome::Minimap::file_for_slot(slot);
        match read(&dir, &name) {
            Some(bytes) => {
                let m = chrome::Minimap::load(&bytes, slot)
                    .unwrap_or_else(|e| panic!("slot {slot} in {name}: {e}"));
                assert_eq!(m.counties.len(), 128 * 128);
                assert!(used.contains(&slot), "slot {slot} has a minimap but no map");
                with_minimap += 1;
            }
            None => assert!(!used.contains(&slot), "slot {slot} is used but {name} is missing"),
        }
    }
    assert_eq!(with_minimap, 44, "11 shipped MAPnn.PL8 files times 4 slots");
    assert_eq!(with_minimap, used.iter().filter(|&&s| s < 60).count());
    eprintln!("minimaps: {with_minimap} slots, matching the used-slot census");
}

