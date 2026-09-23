#![allow(unused_imports)]
use super::*;
use super::colours::*;
use super::super::*;
use super::super::campaign_tests::*;
use super::super::*;
use super::super::corpus::*;
use super::super::render::*;
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

/// A player reported *"when I put peasants into a quarry they show as idle,
/// which should be impossible"*. The **drawing** is faithful and he was reading
/// the picture correctly; what is missing is upstream of it. The quarry was
/// switched off, so `County_RefreshEstimates` left its `labour_useful` at zero,
/// and `Village_RebuildIcons` (`0x0045161E`) draws every worker past a zero
/// ceiling in the **surplus** frame — which is `g_peasantIcons[8]`, the idle
/// townsman's own frame, so the two are pixel-identical.
///
/// In the original that state is unreachable by this route: `FUN_00439CC2`,
/// which `Labour_Move` calls on every drop, **switches the industry on** when
/// men are dropped on a site whose resource the county has. Ours did not, so it
/// reached a picture the original only ever shows for a county that has.
#[test]
fn the_icon_table_is_the_exes_own_and_surplus_is_the_idle_sprite() {
    use l2_view::village as v;
    let exe = l2_testkit::executable!();
    let t = l2_testkit::pe::Table::at(&exe, v::ICON_VALUE_VA);
    let shipped: Vec<i32> = t.i32s(v::ICON_VALUE.len());
    let ours: Vec<i32> = v::ICON_VALUE.iter().map(|&b| b as i32).collect();
    assert_eq!(
        shipped, ours,
        "g_peasantIcons at {:#010X} is not what l2_view::village::ICON_VALUE says",
        v::ICON_VALUE_VA,
    );
    assert_eq!(
        v::ICON_VALUE[l2_kingdom_job_idle()], v::ICON_SURPLUS,
        "the idle townsman and the surplus worker are drawn with the same frame, and a \
         player has already read one as the other. If this ever stops being true, \
         docs/rules.md's paragraph on a switched-off industry is wrong.",
    );
}

fn l2_kingdom_job_idle() -> usize {
    8
}

/// **`Misc_cty.pl8` frames 0 … 0x16 are the village's peasant icons**, which
/// `docs/screens-county.md` §9 guessed, marked `[I]`, were "almost certainly
/// the top menu bar".
///
/// The icon table at `0x004D6808` names nine (normal, highlighted) pairs among
/// them, plus a shortfall pair and a surplus pair. This checks the shipped file
/// against that: **every frame the table names is 16 × 32 and decodes, and the
/// four it never names — 5, 6, 11 and 12 — are exactly the four frames in that
/// range that are 2 × 2 stubs.** Nineteen icons, nineteen frames, nothing over.
#[test]
fn the_peasant_icons_account_for_every_frame_the_icon_table_names() {
    use l2_view::village as v;
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        eprintln!("no Misc_cty.pl8 - skipping");
        return;
    };
    let sheet = Sheet::new(bytes.clone()).expect("Misc_cty.pl8 parses");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");

    let mut named = std::collections::BTreeSet::new();
    for value in v::ICON_VALUE.iter().copied().chain([v::ICON_SHORTFALL, v::ICON_SURPLUS]) {
        named.insert(value as usize - 1);
        named.insert(value as usize);
    }
    assert_eq!(*named.iter().max().unwrap(), 22, "the icons stop at frame 0x16");

    let mut canvas = Canvas::screen();
    for &f in &named {
        assert_eq!(
            (pl8.frames[f].width, pl8.frames[f].height),
            (16, 32),
            "frame {f} is named by the icon table and is not an icon"
        );
        let decoded = sheet.frame(f).expect("frame {f} decodes");
        canvas.blit(&decoded, 100, 100);
    }

    let unnamed: Vec<usize> = (0..=22).filter(|f| !named.contains(f)).collect();
    assert_eq!(unnamed, vec![5, 6, 11, 12], "four frames in the range go unused");
    for f in unnamed {
        assert_eq!(
            (pl8.frames[f].width, pl8.frames[f].height),
            (2, 2),
            "frame {f} is unused and should be a stub"
        );
    }
    eprintln!("Misc_cty: {} named icons, 4 stubs, nothing over", named.len());
}

/// `FUN_00439EDB` loops `i < 10` over `g_jobClusterToSlot`, which has eight
/// entries. The two past the end are the head of the table that follows, and
/// they decide what a double click on the idle townsfolk balances — so
/// they are read out of the user's own executable.
#[test]
fn the_cluster_to_slot_table_and_the_two_words_the_balance_loop_overruns_into() {
    use l2_view::village as v;
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe - skipping");
    };
    let table = l2_testkit::pe::Table::at(&exe, v::CLUSTER_TO_SLOT_VA);
    let read: Vec<usize> = table.i32s(10).into_iter().map(|v| v as usize).collect();
    assert_eq!(read[..8], v::CLUSTER_TO_SLOT, "the eight the table really has");
    assert_eq!(read, v::CLUSTER_TO_SLOT_BALANCE, "and the ten the balance loop reads");
    assert!(read[8..].contains(&4), "the overrun reaches iron mining");
}

#[test]
fn the_village_files_are_the_size_the_drawing_code_indexes_them_at() {
    use l2_view::village as v;
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };

    let scene_bytes = read(&dir, "vill.pl8").expect("vill.pl8");
    let scene = l2_formats::Pl8::parse(&scene_bytes).unwrap();
    assert_eq!(scene.frames.len(), 1);
    assert_eq!((scene.frames[0].width as i32, scene.frames[0].height as i32), (363, v::SCENE_H));

    // The tops: six frames, one per weather, and `L2.eng` group 66 has six
    // names. Village_Draw indexes both with the same county byte.
    let tops_bytes = read(&dir, "villtops.pl8").expect("villtops.pl8");
    let tops = l2_formats::Pl8::parse(&tops_bytes).unwrap();
    assert_eq!(tops.frames.len(), 6, "six weathers");
    for f in &tops.frames {
        assert_eq!((f.width as i32, f.height), (363, 70));
    }

    let grid = read(&dir, "vill_gd8.pl8").expect("vill_gd8.pl8");
    assert_eq!(grid.len(), 0x18 + v::GRID_LEN, "1,824 bytes: 24 + 45 * 40");
    assert_eq!(v::GRID_COLS as i32 * v::GRID_CELL, 360, "x 0x40 .. 0x1A8");
    assert_eq!(v::GRID_ROWS as i32 * v::GRID_CELL, v::SCENE_H, "y top .. top + 0x140");
    assert!(
        grid[0x18..].iter().all(|&b| b as usize <= v::CLUSTER_COUNT),
        "a cell names a cluster the village does not draw"
    );
    eprintln!("village: 363x320 scene, 6 weather tops, 45x40 grid");
}

/// **The realm pen table is `Lords2.exe`'s own ten bytes**, read back out of
/// the user's copy at `0x004DC1D0`.
///
/// `l2_view::chrome::REALM_PEN` is a transcription, and a transcription that
/// nothing checks is a table somebody eventually edits by eye. The stride is
/// the part that is not guessable from the values: the binary indexes from
/// **two bytes below** the data — `(&g_realmColour)[shieldIndex * 2]` with
/// `g_realmColour` at `0x004DC1CE` — so that the 1-based shield lands on the
/// first pair, and reading it the obvious way is off by one entry.
#[test]
fn the_realm_pen_table_matches_the_bytes_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let Some(base) = va_to_offset(&exe, chrome::REALM_PEN_VA) else {
        panic!("{:#010X} is not in any section", chrome::REALM_PEN_VA);
    };
    let want: Vec<u8> = chrome::REALM_PEN.iter().flatten().copied().collect();
    assert_eq!(&exe[base..base + want.len()], &want[..], "g_realmColour differs from ours");

    // The two bytes the binary's own base points at are *not* part of this
    // table — they are the tail of `g_lordChoice`. Asserting that pins the
    // 1-based indexing: if the table really started at `0x004DC1CE` these would
    // be shield 1's pen and they would have to be a colour pair.
    assert_eq!(
        &exe[base - 2..base],
        &[0x04, 0x02][..],
        "the two bytes below the table are g_lordChoice's tail, not a sixth pen"
    );

    assert!(
        exe[base + want.len()..base + want.len() + 8].iter().all(|&b| b == 0),
        "something follows the fifth pair"
    );
    eprintln!("realm pens: {} bytes match Lords2.exe at {:#010X}", want.len(), chrome::REALM_PEN_VA);
}

