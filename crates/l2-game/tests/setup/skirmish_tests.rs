//! **Page 12 answers, and *Go* fights what it is showing.**
//!
//! Every click here is a real pointer coordinate inside the widget table at
//! `0x004DCF68` — `node tools/oracle/widgets.js widgets 4dcf68 22` — so a
//! rectangle that moves breaks these and a method rename does not.
#![allow(unused_imports)]
use super::*;
use l2_game::screens::setup::skirmish::{Skirmish, SkirmishArmy, TroopsTable, ROW_BASE};
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::screen::{ScreenId, Transition};

/// A table whose every count is the row, the side and the column it came from,
/// so a wrong lookup cannot look right.
fn table() -> TroopsTable {
    let mut rows = Vec::new();
    for r in 0..55u16 {
        let mut row = [[[0i16; 11]; 5]; 2];
        for side in 0..2usize {
            for diff in 0..5usize {
                for t in 0..11usize {
                    row[side][diff][t] =
                        (r as i16 + 1) * 100 + side as i16 * 10 + diff as i16 + t as i16 * 0;
                }
            }
        }
        rows.push(row);
    }
    TroopsTable { rows }
}

fn page12(game: &mut Game, assets: &Assets) -> SetupScreen {
    let mut s = SetupScreen::new(SetupPage::Skirmish);
    s.set_troops(table());
    let _ = (game, assets);
    s
}

#[test]
fn the_four_categories_are_the_hotspot_itself() {
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    // Top to bottom: castles, field, field, file — 2, 0, 1, 3.
    for (y, kind) in [(291, 2usize), (322, 0), (353, 1)] {
        click(&mut s, &mut game, &assets, 470, y + 15);
        assert_eq!(s.skirmish().kind, kind, "the category strip is 2, 0, 1, 3");
    }
    // `FUN_0043DC1D`: hotspot 3 with no `.skr` loaded falls back to 0.
    click(&mut s, &mut game, &assets, 470, 384 + 15);
    assert_eq!(s.skirmish().kind, 0, "no file, no file category");
}

#[test]
fn a_list_row_is_the_row_the_fill_reads() {
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    click(&mut s, &mut game, &assets, 500, 185 + 3 * 16 + 7);
    assert_eq!((s.skirmish().slot, s.skirmish().row), (3, 3));
    // `FUN_0043D9CD`, the lower arrow: the top moves and the row with it.
    click(&mut s, &mut game, &assets, 610, 276);
    assert_eq!((s.skirmish().top, s.skirmish().row), (1, 4));
    // Ablation: the row is what reaches the table, so the fill must move too.
    let (mine, _) = s.skirmish().fill_armies(&table());
    assert_eq!(mine.counts[0], 512, "row 4, the attacker's half, column 2");
}

#[test]
fn the_scroll_clamp_leaves_the_row_behind() {
    // `FUN_0043D9CD`'s clamp is an `else`: at either end `DAT_0056D590` is not
    // recomputed. Kept, because the game does it.
    let mut s = Skirmish::default();
    s.scroll(1);
    assert_eq!((s.top, s.row), (1, 1));
    for _ in 0..10 {
        s.scroll(1);
    }
    assert_eq!(s.top, 4, "ten rows, six shown");
    assert_eq!(s.row, 4, "and the last move that was not clamped set it");
    s.scroll(1);
    assert_eq!((s.top, s.row), (4, 4), "a clamped scroll changes nothing at all");
}

#[test]
fn the_handicap_seesaws_between_one_and_three() {
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    assert_eq!(s.skirmish().difficulty, [2, 2], "FUN_0042B919 starts both at 2");
    for _ in 0..3 {
        click(&mut s, &mut game, &assets, 100, 400);
    }
    assert_eq!(s.skirmish().difficulty, [1, 3], "one down, the other up, clamped at 1 and 3");
    for _ in 0..4 {
        click(&mut s, &mut game, &assets, 355, 400);
    }
    assert_eq!(s.skirmish().difficulty, [3, 1]);
    // Ablation: the column is what the fill reads, so the armies must differ.
    let (mine, theirs) = s.skirmish().fill_armies(&table());
    assert_ne!(mine.counts[0], theirs.counts[0], "different columns, different armies");
}

#[test]
fn swapping_sides_swaps_which_half_of_the_table_is_ours() {
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    assert!(s.skirmish().local_attacks, "DAT_0053EF5C = 1 and realm 1 is the player");
    let before = s.skirmish().fill_armies(&table()).0;
    click(&mut s, &mut game, &assets, 90, 280);
    assert!(!s.skirmish().local_attacks);
    let after = s.skirmish().fill_armies(&table()).0;
    assert_eq!(before.counts[0] - after.counts[0], 10, "the attacker's half is side 1");
    assert_eq!(s.skirmish().slots(), (2, 1), "FUN_0042BA40: the defender is army B");
}

#[test]
fn the_file_field_opens_page_13_and_a_row_of_it_chooses() {
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    s.set_skirmish_files(vec!["AGINCOURT.SKR".into(), "HASTINGS.SKR".into()]);
    click(&mut s, &mut game, &assets, 560, 399);
    assert_eq!(s.page(), SetupPage::SkirmishFile, "FUN_0043DDF4 raises g_setupPage 13");
    // `FUN_00434174`'s own rectangle, row 1.
    click(&mut s, &mut game, &assets, 200, 0xB0 + 16 + 8);
    assert_eq!(s.page(), SetupPage::Skirmish, "and it returns to the page it came from");
    assert_eq!(s.skirmish().file.as_deref(), Some("HASTINGS.SKR"));
    assert_eq!(s.skirmish().kind, 3, "taking a name is what makes category 3 reachable");
    // Ablation: a row past the end of the list is not a click.
    click(&mut s, &mut game, &assets, 470, 384 + 15);
    assert_eq!(s.skirmish().kind, 3, "and now hotspot 3 keeps category 3");
}

#[test]
fn go_raises_the_battlefield_with_the_two_armies_the_page_is_showing() {
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    click(&mut s, &mut game, &assets, 500, 185 + 2 * 16 + 7);
    let (mine, theirs) = s.skirmish().fill_armies(&table());
    assert_eq!(mine.men, 7 * 312, "seven men types of row 2, side 1, column 2");
    assert_eq!(theirs.men, 7 * 302, "and the defender is the other half of the same row");

    let t = click(&mut s, &mut game, &assets, 600, 440);
    assert_eq!(t, Transition::Push(ScreenId::Battlefield), "FUN_0043D5B7 calls Battle_Start");
    let b = game.battle.as_ref().expect("the skirmish raised a battle");
    assert!(b.skirmish, "DAT_0057A0F0");
    assert_eq!((b.attacker, b.defender), (1, 2), "g_battleArmyA and g_battleArmyB");
    // Ablation: the fill is what raises it. With no troops table there are no
    // armies, and the button says nothing rather than deploying nobody.
    let mut empty = SetupScreen::new(SetupPage::Skirmish);
    game.battle = None;
    let t = click(&mut empty, &mut game, &assets, 600, 440);
    assert_eq!(t, Transition::Stay, "no table, no battle");
    assert!(game.battle.is_none());
}

#[test]
fn back_leaves_for_page_one() {
    // `FUN_0043D649`: `g_setupPage = 1`, not 2.
    let (mut game, assets) = world!();
    let mut s = page12(&mut game, &assets);
    click(&mut s, &mut game, &assets, 480, 440);
    assert_eq!(s.page(), SetupPage::Title);
}

#[test]
fn the_strength_weights_are_eleven_wide() {
    // `g_troopStrengthWeight` (`0x004D4B98`) — the four engines are not free,
    // and `Skirmish_FillArmies` is the caller that reads that far.
    let counts = [0i16; 11];
    let mut c = counts;
    c[10] = 1;
    let a = SkirmishArmy::from_counts_for_test(c);
    assert_eq!(a.strength, 150, "the eleventh weight");
    assert_eq!(a.men, 0, "and an engine is not a man");
}
