#![allow(unused_imports)]
use super::*;
use super::terrain_part::*;
use super::battle_part::*;
use super::sprites::*;
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

// ---------------------------------------------------------- the campaign map

/// The right column's frames are 162 wide and their heights tile y 24..480
/// exactly. `chrome`'s constants say where each one goes; this reads how tall
/// each one is and checks the column closes.
#[test]
fn the_right_panel_frames_in_the_file_tile_the_column_exactly() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        l2_testkit::skip!("Misc_cty.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");
    let h = |i: usize| {
        let f = &pl8.frames[i];
        assert_eq!(f.width, 162, "frame {i} is {} wide, not 162", f.width);
        f.height as i32
    };
    use l2_view::chrome::misc_cty as f;
    let mut y = chrome::PANEL_TOP_Y;
    y += h(f::PANEL_TOP);
    assert_eq!(y, chrome::PANEL_MIDDLE_Y);
    // The foreign layout: one frame covers the whole middle.
    assert_eq!(y + h(f::PANEL_FOREIGN), chrome::PANEL_STATUS_Y);
    // The owned layout: three frames cover the same span.
    y += h(f::PANEL_OWN_A);
    assert_eq!(y, chrome::PANEL_OWN_B_Y);
    y += h(f::PANEL_OWN_B);
    assert_eq!(y, chrome::PANEL_OWN_C_Y);
    y += h(f::PANEL_OWN_C);
    assert_eq!(y, chrome::PANEL_STATUS_Y);
    y += h(f::PANEL_STATUS);
    assert_eq!(y, chrome::PANEL_END_TURN_Y);
    y += h(f::PANEL_END_TURN);
    assert_eq!(y, 480, "the column reaches the bottom of the screen");

    // The five realm banners are the 13 x 16 frames, and 16 is what fits the
    // 24-pixel menu bar at y 4.
    for colour in 1..=5usize {
        let b = &pl8.frames[f::BANNER + colour];
        assert_eq!((b.width, b.height), (13, 16), "banner for realm colour {colour}");
        assert!(4 + b.height as i32 <= campaign::TOP_BAR_H);
    }
    eprintln!("right panel: 7 frames tile y 24..480, 5 banners fit the bar");
}

/// `Panels.pl8`'s framed-box kit, checked against the file
/// `chrome`'s own constants: 52 border frames of 16 x 16, 144 texture frames of
/// 16 x 16, then eight of 24 x 24, then a second border set.
#[test]
fn the_panels_kit_in_the_file_has_the_shape_the_drawing_code_indexes() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Panels.pl8") else {
        l2_testkit::skip!("Panels.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Panels.pl8 parses");
    use l2_view::chrome::panels as p;

    // Border set A, the interior texture and border set B are all 16 x 16.
    for i in (0..p::STRIP).chain(p::SET_B..p::SET_B + 52) {
        assert_eq!((pl8.frames[i].width, pl8.frames[i].height), (16, 16), "frame {i}");
    }
    // The eight strip frames between them are 24 x 24, and nothing else is.
    for i in p::STRIP..p::STRIP + p::STRIP_LEN {
        assert_eq!((pl8.frames[i].width, pl8.frames[i].height), (24, 24), "frame {i}");
    }
    assert_ne!(pl8.frames[p::STRIP - 1].height, 24);
    assert_ne!(pl8.frames[p::SET_B].height, 24);

    // 25 strip cells from x 0 plus 2 from x 592 covers the screen exactly.
    assert_eq!(25 * p::STRIP_CELL, 600);
    assert_eq!(0x250 + 2 * p::STRIP_CELL, 640);
    eprintln!("panels: {} frames, kit boundaries match", pl8.frames.len());
}

/// **The blue outline, named to a frame and to three palette indices.**
///
/// A player remembered *"a blue outline for idle peasants"* and *"the peasant
/// slider I think had a blue outline if there were idle peasants as well."* The
/// binary says where it is — `CountyStrip_Draw` swaps five icon frames on
/// `labour[slot].useful < labour[slot].workers` and the slider's thumb on
/// `labour[8].workers != 0` — and this says *what it is*, out of the shipped
/// artwork:
///
/// * every ringed frame is **exactly four wider and four taller** than its
///   plain twin, which is what a two-pixel ring around an unchanged picture
/// measures as, and the drawing code moves it two pixels up and left;
/// * every non-transparent pixel of that two-pixel border is one of
///   **three palette entries, and all three are blue** — `95` = `rgb(0,0,121)`,
///   `65` = `rgb(157,202,234)`, `64` = `rgb(194,230,255)`.
///
/// The second clause is the one that makes "blue" a measurement. A ring of any
/// other colour would fail it, and so would a frame that happened to be
/// the right size.
#[test]
fn the_ringed_strip_icons_are_their_plain_twins_inside_a_blue_two_pixel_ring() {
    use l2_view::chrome::misc_cty;
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let bytes = read(&dir, "Misc_cty.pl8").expect("Misc_cty.pl8");
    let sheet = Sheet::new(bytes).expect("Misc_cty.pl8 parses");
    let palette = l2_formats::Palette::from_bytes(&read(&dir, "Base01.256").expect("Base01.256"))
        .expect("Base01.256 parses");

    // The ring's three entries really are blue, in the palette the campaign
// screen runs under. Asserted, because the whole claim
    // rests on the word.
    // Blue channel highest, red lowest, and a clear gap between them: the ring
    // runs from `rgb(0,0,121)` to a near-white highlight at `rgb(194,230,255)`,
// so "blue" here is the *ordering* of the channels
    // distance, which the pale end would fail.
    for i in misc_cty::RING_COLOURS {
        let [r, g, b] = palette.rgb(i);
        assert!(
            b >= g && g >= r && b as i32 - r as i32 >= 40,
            "palette entry {i} is rgb({r},{g},{b}), which is not blue"
        );
    }

    let pairs: Vec<(usize, usize, bool)> = misc_cty::RINGED_PAIRS
        .iter()
        .map(|&(p, r)| (p, r, true))
        .chain([
            (misc_cty::SPLIT_THUMB, misc_cty::SPLIT_THUMB_IDLE, true),
            // The castle is the one exception, and it is written down as one.
            (misc_cty::CASTLE_PLAIN, misc_cty::CASTLE_RINGED, false),
        ])
        .collect();
    assert_eq!(pairs.len(), 11, "nine icons, the slider's thumb and the castle");
    // The eleven ringed frames are a contiguous run, `0x4B` … `0x55`, with
    // nothing else in the sheet ringed and nothing in the run left over.
    let mut ringed: Vec<usize> = pairs.iter().map(|&(_, r, _)| r).collect();
    ringed.sort_unstable();
    assert_eq!(ringed, (0x4B..=0x55).collect::<Vec<_>>(), "the ringed run is 0x4B ..= 0x55");

    for (plain, ringed, same_picture) in pairs {
        let a = sheet.frame(plain).unwrap_or_else(|| panic!("frame {plain:#04x} decodes"));
        let b = sheet.frame(ringed).unwrap_or_else(|| panic!("frame {ringed:#04x} decodes"));
        if same_picture {
            assert_eq!(
                (b.width - a.width, b.height - a.height),
                (4, 4),
                "{ringed:#04x} is {}x{} and {plain:#04x} is {}x{}: a two-pixel ring is +4 on \
                 each axis",
                b.width,
                b.height,
                a.width,
                a.height
            );
        } else {
            assert_ne!(
                (b.width - a.width, b.height - a.height),
                (4, 4),
                "{ringed:#04x} was the documented exception and is no longer one"
            );
        }

        let (w, h) = (b.width as usize, b.height as usize);
        let (mut ring, mut blue) = (0usize, 0usize);
        for y in 0..h {
            for x in 0..w {
                if x >= 2 && y >= 2 && x + 2 < w && y + 2 < h {
                    continue;
                }
                let p = b.indices[y * w + x];
                if p == 0 {
                    continue;
                }
                ring += 1;
                if misc_cty::RING_COLOURS.contains(&p) {
                    blue += 1;
                }
            }
        }
        assert!(ring > 0, "{ringed:#04x} has no border pixels at all");
        assert_eq!(ring, blue, "{ringed:#04x}: {} of {ring} border pixels are not blue", ring - blue);
        eprintln!("Misc_cty {plain:#04x} -> {ringed:#04x}: {ring} border pixels, all blue");
    }
}

/// **The path-preview balls, and the `[I]` they settle.**
///
/// `Map_DrawPathMarker` (`0x004081A6`) draws `g_flagsSheet` frame `0x38 + n`
/// where `n` is the accumulated cost of reaching the tile, collapsing to
/// `0x38` for a step past the remaining budget, and `0x4E` on a castle,
/// settlement or plot. `docs/armies.md` §2.3 marked the mechanism **[V]** and
/// recorded one thing as **[I]**: *"that frame `0x38` is specifically the grey
/// one — nobody has looked at the sheet."*
///
/// This is looking at the sheet. Three things come out of it, and each is the
/// kind of claim that only a measurement can make:
///
/// 1. the run `0x38 … 0x4E` is **23 frames of exactly one shape** — same size,
///    same 177-pixel silhouette — so it is one picture recoloured, which is
///    what "indexed by cost" predicts and what a set of *different* pictures
///    would have refuted;
/// 2. **`0x38` is the only frame in the ramp with no colour in it at all**, so
/// the grey one is the out-of-range one and the inference is now a fact;
/// 3. `0x4E` is the opposite extreme — not one grey pixel — which is the
///    castle marker being a different picture.
///
/// A player reported these as *"colored dot images for the army walking dots"*,
/// and what selects the colour is the **cost**: not the realm, not the shield,
/// not the unit's kind. See `l2_view::campaign::path_marker_frame`.
#[test]
fn the_path_marker_ramp_is_one_ball_recoloured_and_only_the_first_is_grey() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Flags1a.pl8").expect("Flags1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");
    let pal_bytes = read(&dir, "Base01.256").expect("Base01.256");
    let pal = l2_formats::Palette::from_bytes(&pal_bytes).expect("palette");

    let first = sheet.frame(campaign::PATH_MARKER_FIRST).expect("frame 0x38");
    assert_eq!((first.width, first.height), (15, 15), "the ball is 15 x 15");

    // How many opaque pixels of a frame are a true grey, and how many carry
    // colour.
    let split = |i: usize| {
        let f = sheet.frame(i).unwrap_or_else(|| panic!("frame {i:#04x} is missing"));
        assert_eq!(
            (f.width, f.height),
            (first.width, first.height),
            "frame {i:#04x} is a different size from the rest of the ramp",
        );
        assert_eq!(
            f.opaque, first.opaque,
            "frame {i:#04x} has a different silhouette: the ramp is one picture recoloured",
        );
        let (mut grey, mut colour) = (0usize, 0usize);
        for (&p, &o) in f.indices.iter().zip(f.opaque.iter()) {
            if !o {
                continue;
            }
            let [r, g, b] = pal.rgb(p);
            if r == g && g == b {
                grey += 1;
            } else {
                colour += 1;
            }
        }
        (grey, colour)
    };

    let (grey, colour) = split(campaign::PATH_MARKER_FIRST);
    assert_eq!(colour, 0, "frame 0x38 is the grey one, and that was an inference until now");
    assert_eq!(grey, 177, "and all 177 of its opaque pixels are grey");

    for i in campaign::PATH_MARKER_FIRST + 1..=campaign::PATH_MARKER_LAST {
        let (_, colour) = split(i);
        assert!(colour > 0, "frame {i:#04x} carries colour and 0x38 does not");
    }

    // 0x4E, the castle/settlement marker, is the same size and silhouette and
    // is the one frame in the block with no grey in it at all.
    let (grey, colour) = split(campaign::PATH_MARKER_LAST + 1);
    assert_eq!(grey, 0, "0x4E is not a ball");
    assert_eq!(colour, 177);

    eprintln!(
        "path markers: {:#04x}..={:#04x} are 15x15, one silhouette, 0x38 alone is grey",
        campaign::PATH_MARKER_FIRST,
        campaign::PATH_MARKER_LAST + 1,
    );
}

