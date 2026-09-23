//! Page 13's two scroll arrows — `g_skirmishFileWidgets` (`0x004DDDD8`), read
//! with `node tools/oracle/widgets.js widgets 4ddda8 6`.
#![allow(unused_imports)]
use super::*;
use super::skirmish_tests::page12;
use l2_game::screens::setup::skirmish::TroopsTable;
use l2_game::screens::setup::{FILE_SCROLL, WIDGET_SIDE};
use l2_game::screens::setup::{SetupPage, SetupScreen};

fn page13(files: usize) -> SetupScreen {
    let mut s = SetupScreen::new(SetupPage::SkirmishFile);
    s.set_skirmish_files((0..files).map(|i| format!("B{i:02}.SKR")).collect());
    s
}

/// `Widget_Test` (`0x0040DA1E`): *"the hit box is a square of side record
/// +0x06"* — `table[3]` on both axes — so the two records at (480, 256) and
/// (480, 288) with `+0x06 = 24` are 24 by 24, **not** the `(x2, y2)` that page
/// 12's `Hotspot_Test` table at `0x004DCF68` holds in the same two shorts.
///
/// **Ablation:** `crates/l2-game/src/screens/setup/ui/skirmish_file.rs`, the
/// `FILE_SCROLL` loop in `skirmish_file_arms` — with the two records gone the
/// press lands on the full-screen `Action::Item(0)` cancel instead and the top
/// never moves.
#[test]
fn page_13s_two_arrows_move_the_file_lists_scroll_base() {
    let (mut game, assets) = world!();
    let (up, down) = (FILE_SCROLL[0], FILE_SCROLL[1]);
    assert_eq!((up.0, up.1, up.2), (480, 256, -1), "record 2 of 0x004DDDA8");
    assert_eq!((down.0, down.1, down.2), (480, 288, 1), "record 3");

    let mut s = page13(20);
    let mid = WIDGET_SIDE / 2;
    click(&mut s, &mut game, &assets, down.0 + mid, down.1 + mid);
    assert_eq!(s.skirmish_file_top(), 1, "hotspot +1 on g_fileListTop");
    click(&mut s, &mut game, &assets, down.0 + mid, down.1 + mid);
    assert_eq!(s.skirmish_file_top(), 2);
    click(&mut s, &mut game, &assets, up.0 + mid, up.1 + mid);
    assert_eq!(s.skirmish_file_top(), 1, "hotspot -1");
    click(&mut s, &mut game, &assets, up.0 + mid, up.1 + mid);
    click(&mut s, &mut game, &assets, up.0 + mid, up.1 + mid);
    assert_eq!(s.skirmish_file_top(), 0, "`if (DAT_004EA1A0 < 0) DAT_004EA1A0 = 0`");

    // One pixel outside the square is not the button.
    let mut s = page13(20);
    click(&mut s, &mut game, &assets, down.0 + WIDGET_SIDE, down.1 + mid);
    assert_eq!(s.skirmish_file_top(), 0, "the box is 24 wide, not 25");
}

/// `SaveLoad_Scroll`'s list-2 clamp (`00430000.c:1476-1479`) is
/// `if (DAT_004EB25C - 5 < top) top -= 1;` and then
/// `if (DAT_004EB25C < 10) top = 0;` — **five** and **ten**, where the box
/// draws and hit-tests ten rows. Kept: a nine-file list cannot scroll at all
/// and a twelve-file list stops with five blank lines showing.
///
/// **Ablation:** `scroll_skirmish_files` in
/// `crates/l2-game/src/screens/setup/ui/skirmish_start.rs`, the `count < 10`
/// line — the nine-file case then scrolls.
#[test]
fn the_file_lists_clamp_counts_five_rows_where_the_box_draws_ten() {
    let (mut game, assets) = world!();
    let (down, mid) = (FILE_SCROLL[1], WIDGET_SIDE / 2);

    let mut s = page13(9);
    for _ in 0..4 {
        click(&mut s, &mut game, &assets, down.0 + mid, down.1 + mid);
    }
    assert_eq!(s.skirmish_file_top(), 0, "under ten names the list does not scroll");

    let mut s = page13(12);
    for _ in 0..9 {
        click(&mut s, &mut game, &assets, down.0 + mid, down.1 + mid);
    }
    assert_eq!(s.skirmish_file_top(), 7, "count - 5, five blank rows under the last name");
}

/// **The custom muster's four count buttons** — `DAT_004DE000`, whose 28 or 44
/// kind-4 records sit at x 95, 111, 134 and 150, `y = 106 + 30 * row`, with the
/// row as the hotspot.
///
/// `FUN_0042C4E2` (`0x0042C4E2`) is the whole cap: `g_troopStrengthWeight[slot]
/// <= DAT_00521C90`, and `FUN_0042C476` makes that allowance the table row's
/// own strength. So buying one peasant back after selling one is free and
/// buying a knight with two peasants' worth of change is refused.
///
/// **Ablation:** `crates/l2-game/src/screens/setup/ui/skirmish_counts.rs`, the
/// `STRENGTH_WEIGHT[slot] <= self.spare_strength(table)` line in `may_buy` —
/// without it the first press buys a peasant with nothing spent and the third
/// assertion is red.
#[test]
fn the_custom_musters_counts_are_bought_out_of_the_rows_own_strength() {
    use l2_game::screens::setup::{ALL_FEWER, ONE_FEWER, ONE_MORE, COUNT_X, COUNT_Y0, COUNT_PITCH};

    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    s.skirmish_mut().custom = true;
    let start = s.skirmish().fill_armies(s.troops()).0.counts;
    let at = |col: usize, row: usize| (COUNT_X[col] + 8, COUNT_Y0 + row as i32 * COUNT_PITCH + 8);

    // Nothing is spare until something is sold: FUN_0042C476 is
    // `DAT_0051FBBC - sum(count * weight)` over the row it was filled from.
    assert_eq!(s.skirmish().spare_strength(s.troops()), 0);
    let (x, y) = at(ONE_MORE, 0);
    click(&mut s, &mut game, &assets, x, y);
    assert_eq!(s.skirmish().fill_armies(s.troops()).0.counts[0], start[0], "nothing to spend");

    let (x, y) = at(ONE_FEWER, 6);
    click(&mut s, &mut game, &assets, x, y);
    assert_eq!(s.skirmish().spare_strength(s.troops()), 22, "one knight back in the purse");
    let (x, y) = at(ONE_MORE, 0);
    click(&mut s, &mut game, &assets, x, y);
    assert_eq!(
        s.skirmish().fill_armies(s.troops()).0.counts[0],
        start[0] + 1,
        "a peasant costs 2 of the 22",
    );

    // `FUN_0043D8B9` is a `while` loop over `FUN_0043D7A7`.
    let (x, y) = at(ALL_FEWER, 6);
    click(&mut s, &mut game, &assets, x, y);
    assert_eq!(s.skirmish().fill_armies(s.troops()).0.counts[6], 0, "the whole column goes");

    // Every arm that calls `Skirmish_FillArmies` puts the table row back.
    s.skirmish_mut().pick_row(1);
    s.skirmish_mut().pick_row(0);
    assert_eq!(s.skirmish().fill_armies(s.troops()).0.counts, start, "Skirmish_FillArmies");
}
