#![allow(unused_imports)]
use super::*;
use super::overlays_part::*;
use super::end_turn::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

/// **The tables, out of the player's own copy of the game.**
///
/// Ablation, change `SIDEBAR_BUTTONS[0].w` from 33 to 32 and the
/// first assertion fails naming record 0. The probe is the exe and the subject
/// is our constant,
/// which is the trap `docs/agents.md` records: *ablating a constant while
/// computing your probe from that same constant tests nothing at all.*
#[test]
fn the_right_columns_geometry_is_the_exes_own_tables() {
    let exe = l2_testkit::executable!();
    let sidebar = l2_testkit::pe::Table::at(&exe, SIDEBAR_TABLE);

    for (i, b) in map::SIDEBAR_BUTTONS.iter().enumerate() {
        assert_eq!(
            b.rect(),
            hotspot(&sidebar, i, SIDEBAR_OFFSET),
            "g_sidebarButtons record {i} ({}) is not where the exe puts it",
            b.name,
        );
    }
    // **Record 5 is End Turn**, and it is the one a button of ours was drawn on
    // top of. The table has six records and the module's array has five,
    // because the sixth dispatches to `Turn_End`
    // `Sidebar_Button` — so it is a constant of its own and this is what pins
    // it.
    assert_eq!(
        map::END_TURN_BUTTON,
        hotspot(&sidebar, 5, SIDEBAR_OFFSET),
        "the End Turn strip is not record 5",
    );

    let modes = l2_testkit::pe::Table::at(&exe, MINIMAP_MODE_TABLE);
    for (i, r) in map::MINIMAP_MODE_BUTTONS.iter().enumerate() {
        assert_eq!(*r, hotspot(&modes, i, (610, 32)), "g_minimapModeButtons record {i}");
    }
    // The second record's `y1` is `0x42` where the pattern wants `0x3F`, so band
    // 2 is 34 pixels tall and overlaps band 3's first two rows. That is the
    // original's own data and this is where it is pinned.
    assert_eq!(map::MINIMAP_MODE_BUTTONS[1].h, 34, "the overlapping band is the exe's");

    // And the army-division screen.
    let split = l2_testkit::pe::Table::at(&exe, SPLIT_TABLE);
    assert_eq!(divide::SPLIT_TICK, widget(&split, 0, (0, 0)), "the confirm tick");
    assert_eq!(divide::SPLIT_CROSS, widget(&split, 1, (0, 0)), "the cancel cross");
    for row in 0..8usize {
        assert_eq!(divide::parent_button(row), widget(&split, 2 + row * 2, (0, 0)), "row {row} parent");
        assert_eq!(
            divide::daughter_button(row),
            widget(&split, 3 + row * 2, (0, 0)),
            "row {row} daughter",
        );
    }
}

