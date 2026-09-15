#![allow(unused_imports)]
use super::*;
use super::terrain_part::*;
use super::battle_part::*;
use super::ui::*;
use super::*;
use super::corpus::*;
use super::oracle::*;
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

/// `FUN_004071A0`'s farm arm picks
/// `0x55 + (terrain - 0x14) * 6 + phase` for a stocked pasture and
/// `0x67 + (terrain - 0x10) * 6 + phase` for the vestigial half nothing writes.
///
/// Ablating the `* 6` in [`campaign::herd_sprite`] fails claim 1 (the ladder
/// walks into the `2 × 2` stubs); ablating the `+ 0x55` fails it too; swapping
/// the two group bases fails claim 3.
#[test]
fn the_pasture_herd_frames_are_full_tiles_and_grow_with_the_crowding() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Flags1a.pl8").expect("Flags1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");

    for empty in [0x0Fu8, 0x13] {
        for phase in 0..campaign::HERD_PHASES {
            assert!(
                campaign::herd_sprite(empty, phase).is_none(),
                "terrain {empty:#04x} is pasture with no animals on it",
            );
        }
    }

    let tile = (l2_view::campaign::NEAR.tile_w as u16, l2_view::campaign::NEAR.tile_h as u16);
    let mut opaque_per_band = Vec::new();
    let mut frames_seen = Vec::new();
    for terrain in 0x14u8..=0x16 {
        let mut band = Vec::new();
        for phase in 0..campaign::HERD_PHASES {
            let (frame, at) = campaign::herd_sprite(terrain, phase).expect("a stocked pasture");
            assert_eq!(at, campaign::HERD_AT, "terrain {terrain:#04x} is the 0x55 block");
            let f = sheet
                .frame(frame)
                .unwrap_or_else(|| panic!("frame {frame:#04x} is missing from Flags1a.pl8"));
            assert_eq!(
                (f.width, f.height),
                tile,
                "frame {frame:#04x} is not a tile-sized meadow overlay",
            );
            band.push(f.opaque.iter().filter(|&&o| o).count());
            frames_seen.push(frame);
        }
        let (lo, hi) = (*band.iter().min().unwrap(), *band.iter().max().unwrap());
        assert!(hi > 0, "terrain {terrain:#04x} draws nothing");
        assert!(
            hi - lo < hi / 4,
            "terrain {terrain:#04x}'s six phases are not one scene animated: {band:?}",
        );
        opaque_per_band.push(band.iter().sum::<usize>() / band.len());
    }
    frames_seen.sort_unstable();
    frames_seen.dedup();
    assert_eq!(frames_seen.len(), 18, "three bands by six phases, all distinct");
    assert_eq!(frames_seen[0], campaign::HERD_FIRST_FRAME);
    assert_eq!(frames_seen[17], campaign::HERD_FIRST_FRAME + 17);

    assert!(
        opaque_per_band[0] < opaque_per_band[1] && opaque_per_band[1] < opaque_per_band[2],
        "a more crowded meadow must carry more animals: {opaque_per_band:?}",
    );

    for terrain in 0x10u8..=0x12 {
        for phase in 0..campaign::HERD_PHASES {
            let (frame, at) = campaign::herd_sprite(terrain, phase).expect("the dead ladder");
            assert_eq!(at, campaign::HERD_DEAD_AT);
            let f = sheet.frame(frame).unwrap_or_else(|| panic!("frame {frame:#04x} missing"));
            assert_eq!(
                (f.width, f.height),
                (2, 2),
                "frame {frame:#04x} is real art, so the block is not vestigial after all",
            );
        }
    }

    eprintln!(
        "Flags1a 0x55..0x66: 58x30 meadows, {} / {} / {} opaque pixels by band; 0x67..0x78 are 2x2 stubs",
        opaque_per_band[0], opaque_per_band[1], opaque_per_band[2],
    );
}

/// the expensive mistake on this screen has always been building the *right* mechanism onto
/// the *wrong* frame block (`docs/decisions.md` C49 lost four documents to it), and a block
/// boundary is something the file can refute before any code exists.
///
/// **Nothing here is computed from one of our constants.** Every number below is typed from
/// `Sprite_TopIt` (`0x004071A0`) and `Map_DrawArmies` (`0x00408438`) — which is the trap the
/// cattle offset fell into once, where the probe was derived from the constant it was
/// testing and ablation therefore proved nothing.
///
/// 1. **The herd ladder saturates; it does not overflow.** `Sprite_TopIt`'s farm arm is
///    `frame = (content - 0x14) * 6 + phase + 0x55` guarded by
///    `if (2 < (byte)(content - 0x14)) return`, and `phase` is `DAT_0057D388 >> 4` with the
///    counter wrapped at `0x60`, so `0 … 5`. The largest index reachable is therefore
///    `2*6 + 5 + 0x55 = 0x66`, and **`0x66` must be the last `58 × 30` frame of the run**
///    while `0x67` must not be one. That is the whole of the answer to *"is the three-band
///    graphic against the four-band meter a live out-of-bounds?"* — it is not.
///
/// 3. **`0x79 … 0x80` is one eight-frame block** — the peasant mob's banner,
///    `Map_DrawArmies`' `frame = 0x79 + phase` for `unit.kind == 2`, with eight phases
///    because `DAT_0057D390` is `DAT_0057D378 >> 4` wrapped at `0x80`. `0x78` before it is
///    the last of the `2 × 2` stubs the herd test already names, and `0x81` after it is the
///    mercenary marker `Sprite_TopIt` draws on the town's north-east quadrant.
#[test]
fn the_flags_sheet_damage_and_mob_banner_blocks_are_where_sprite_topit_says() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Flags1a.pl8").expect("Flags1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");
    let size = |i: usize| {
        let f = sheet.frame(i).unwrap_or_else(|| panic!("Flags1a.pl8 has no frame {i:#04x}"));
        (f.width, f.height)
    };

    let top = 2 * 6 + 5 + 0x55;
    assert_eq!(top, 0x66, "(content-0x14)*6 + phase + 0x55 tops out at 0x66");
    assert_eq!(size(top), (58, 30), "frame 0x66 is the last full-tile meadow");
    assert_ne!(size(top + 1), (58, 30), "frame 0x67 must be past the end of the meadows");

    assert_eq!(size(0x27), (32, 24), "0x27 is the fortieth flag frame");
    for f in 0x28..=0x37 {
        assert_eq!(size(f), (32, 24), "frame {f:#04x} is not part of the damage block");
    }
    assert_eq!(size(0x38), (15, 15), "0x38 is the first path ball, not damage");

    let warm = |i: usize| {
        let f = sheet.frame(i).expect("frame");
        f.indices
            .iter()
            .zip(f.opaque.iter())
            .filter(|(&v, &o)| o && (0xC0..=0xD8).contains(&v))
            .count()
    };
    assert!(
        warm(0x2E) > warm(0x37),
        "the damage animation should end colder than its middle: {} then {}",
        warm(0x2E),
        warm(0x37),
    );

    assert_eq!(size(0x78), (2, 2), "0x78 is the last of the vestigial stubs");
    for f in 0x79..=0x80 {
        assert_eq!(size(f), (16, 42), "frame {f:#04x} is not part of the mob banner");
    }
    assert_ne!(size(0x81), (16, 42), "0x81 is the mercenary marker, not a ninth phase");
}

#[test]
fn the_four_wheat_variants_ripen_and_the_block_ends_where_the_next_base_begins() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let sheet = Sheet::new(read(&dir, "Roads1a.pl8").expect("Roads1a.pl8")).expect("parse");
    let palette = l2_formats::Palette::from_bytes(&read(&dir, "Base01.256").expect("Base01.256"))
        .expect("the shipped palette");

    let gold = |frame: usize| -> usize {
        let f = sheet.frame(frame).unwrap_or_else(|| panic!("no frame {frame}"));
        assert_eq!(f.width, 58, "frame {frame} is not a tile diamond");
        assert!(f.height >= 30, "frame {frame} is shorter than a tile");
        f.indices
            .iter()
            .zip(f.opaque.iter())
            .filter(|(_, &o)| o)
            .filter(|(&v, _)| {
                let [r, g, b] = palette.rgb(v);
                r > 140 && g > 110 && b < 110 && r >= g
            })
            .count()
    };

    for variation in 0..4usize {
        let ramp: Vec<usize> = (0..4).map(|v| gold(88 + v * 4 + variation)).collect();
        for w in ramp.windows(2) {
            assert!(
                w[1] > w[0],
                "variation {variation}: the gold does not rise across the variants: {ramp:?}",
            );
        }
        assert!(
            gold(84 + variation) < ramp[0],
            "fallow frame {} is as gold as the youngest wheat",
            84 + variation,
        );
        assert!(
            gold(104 + variation) < ramp[0],
            "pasture frame {} is as gold as the youngest wheat",
            104 + variation,
        );
    }

    assert_eq!(l2_view::campaign::field_base(2).0, 88);
    assert_eq!(l2_view::campaign::field_base(0x13).0, 104);
    assert_eq!(88 + 4 * 4, 104, "four variants of four variations tile the gap exactly");
}


