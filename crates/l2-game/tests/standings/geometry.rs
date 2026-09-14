#![allow(unused_imports)]
use super::*;
use super::scoring::*;
use super::interaction::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::nobles::{self, NoblesScreen};
use l2_game::Game;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

// ------------------------------------------------------- the geometry, gated

/// **The seven tabs, the three lookup tables and the banner sheet, out of the
/// player's own copy of the game.**
///
/// `g_nobleTabs` (`0x004DC890`) is seven 24-byte `Hotspot_Test` records; the
/// three tables at `0x004D2B20`, `0x004D2B40` and `0x004D2B58` are the tab
/// marker's x, the column x and **which realm stands in which column**, which
/// is not the realm order.
///
/// Ablation, run: change one entry of [`nobles::COLUMN_REALM`] and the third
/// assertion fails naming the slot.
#[test]
fn the_pages_geometry_is_the_exes_own_tables() {
    let exe = l2_testkit::executable!();

    // `Hotspot_Test` records: `{x0, y0, x1, y1}` as four `i16`, the handler at
    // `+0x08`, the kind at `+0x0F` and `g_uiHotspotId` at `+0x10`.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_C890);
    for i in 0..nobles::CATEGORIES {
        let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
        let (x0, y0, x1, y1) = (s(0), s(1), s(2), s(3));
        let rect = nobles::TABS[i];
        assert_eq!((rect.x, rect.y), (x0, y0), "tab {i} origin");
        assert_eq!((rect.w, rect.h), (x1 - x0, y1 - y0), "tab {i} size");
        let handler = (0..4).fold(0u32, |a, k| a | (t.u8_at(i * 24 + 8 + k) as u32) << (8 * k));
        assert_eq!(handler, 0x0043_524E, "tab {i} is FUN_0043524E");
        assert_eq!(t.u8_at(i * 24 + 0x0F), 1, "tab {i} is Hotspot_Test kind 1");
        assert_eq!(t.u8_at(i * 24 + 0x10) as usize, i, "tab {i} publishes id {i}");
    }

    // `g_nobleTabX` — the marker's x per category, eight pixels inside the tab.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_2B20);
    for i in 0..nobles::CATEGORIES {
        assert_eq!(t.i32_at(i), nobles::TAB_X[i], "marker x {i}");
        assert_eq!(nobles::TAB_X[i] - nobles::TABS[i].x, 8, "marker {i} sits 8 inside its tab");
    }

    // `g_nobleColumnX` — entries **1…5**; entry 0 is a zero the loop never
    // reads,
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_2B40);
    for (slot, &x) in nobles::COLUMN_X.iter().enumerate() {
        assert_eq!(t.i32_at(slot + 1), x, "column x, slot {}", slot + 1);
    }

    // `g_nobleColumnRealm` — and the point of asserting it is that it is not
    // 1, 2, 3, 4, 5: the original seats realm 1 in the middle.
    let t = l2_testkit::pe::Table::at(&exe, 0x004D_2B58);
    for (slot, &realm) in nobles::COLUMN_REALM.iter().enumerate() {
        assert_eq!(t.i32_at(slot + 1) as u8, realm, "column realm, slot {}", slot + 1);
    }
    assert_ne!(
        nobles::COLUMN_REALM,
        [1, 2, 3, 4, 5],
        "the seating order is the table's, not the realm order",
    );
}

/// **`Flags.pl8` has six frames and the sixth is not a sixth realm.**
///
/// `docs/draws.md` recorded frame 5 as *"for the leader"*. It is 23 × 60 where
/// the five banners are 51 × 92, and the painter indexes it by the category.
/// The dimensions are the evidence that it is a different thing: a sixth
/// banner would be 51 × 92 too.
#[test]
fn the_banner_sheet_holds_five_banners_and_one_marker() {
    let dir = l2_testkit::install!();
    let bytes = match std::fs::read(dir.join("Flags.pl8")) {
        Ok(b) => b,
        Err(e) => l2_testkit::skip!("Flags.pl8: {e}"),
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Flags.pl8 parses");
    assert_eq!(pl8.frames.len(), 6, "five banners and one marker");
    for (i, f) in pl8.frames.iter().take(5).enumerate() {
        assert_eq!((f.width, f.height), (51, 92), "banner {i}");
    }
    let marker = &pl8.frames[nobles::MARKER_FRAME];
    assert_eq!((marker.width, marker.height), (23, 60), "the tab marker is its own size");
    // The banner's height is what puts the pole's top where it is: the flag is
    // drawn at `top + 0x2D` and the pole starts at `top + 0x89`.
    assert_eq!(
        nobles::POLE_TOP - nobles::FLAG_TOP,
        pl8.frames[0].height as i32,
        "the pole begins exactly at the banner's bottom edge",
    );
}

/// **`L2.eng` group 35 is this screen's whole vocabulary** — `CLAUDE.md` rule
/// 6, and this painter is its only consumer in the binary.
///
/// Seven categories and one *"undecided."*, and the test asserts the words
///
#[test]
fn the_page_draws_its_words_out_of_group_35() {
    let dir = l2_testkit::install!();
    let bytes = match std::fs::read(dir.join("L2.eng")) {
        Ok(b) => b,
        Err(e) => l2_testkit::skip!("L2.eng: {e}"),
    };
    let eng = l2_game::shell::Eng::parse(bytes).expect("L2.eng parses");
    let want = [
        "Most counties,",
        "Most castles,",
        "Most troops,",
        "Most crowns,",
        "Happiest people,",
        "Most people,",
        "Greatest noble,",
        "undecided.",
    ];
    assert_eq!(eng.group(nobles::GROUP).len(), want.len(), "group 35 is eight strings");
    for (i, w) in want.iter().enumerate() {
        assert_eq!(eng.text(nobles::GROUP, i), *w, "35/{i}");
    }
    assert_eq!(nobles::UNDECIDED, want.len() - 1, "the last one is the refusal");
}

/// **The page paints the poles at the height the percentages say**, with no
/// artwork at all — which is what makes this checkable on a machine with no
/// copy of the game.
///
/// The pole is four one-pixel columns in colours `0x10`, `0x12`, `0x14`,
/// `0x12`, from `(100 - pct) * 2 + 0x89` down to `0x158`. A leader's pole is
/// therefore 208 pixels and a realm on nothing has 8.
#[test]
fn the_poles_are_drawn_to_the_height_the_percentages_say() {
    let (mut game, assets) = bare_world();
    for r in 1..l2_kingdom::MAX_REALMS {
        game.kingdom.realms[r].in_play = true;
        game.kingdom.realms[r].strength = 1;
        game.kingdom.realms[r].shield_index = r as u8;
    }
    game.kingdom.realms[1].gold = 1_000;
    game.kingdom.realms[2].gold = 500;
    game.kingdom.realms[3].gold = 0;
    game.kingdom.realms[4].gold = 0;
    game.kingdom.realms[5].gold = 0;
    game.nobles_category = nobles::CROWNS as u8;

    let mut screen = NoblesScreen::new();
    let mut canvas = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        <NoblesScreen as l2_game::screen::Screen>::draw(&mut screen, &ctx, &mut canvas);
    }

    // How tall is the lit column of realm `r`'s pole?
    let height = |realm: u8| {
        let slot = nobles::COLUMN_REALM.iter().position(|&x| x == realm).expect("a column");
        let x = nobles::COLUMN_X[slot] + nobles::POLE[2].0;
        (0..480).filter(|&y| canvas.at(x as usize, y) == nobles::POLE[2].1).count() as i32
    };
    let expect = |pct: i32| nobles::POLE_BOTTOM - ((100 - pct) * nobles::PIXELS_PER_PERCENT + nobles::POLE_TOP) + 1;

    assert_eq!(height(1), expect(100), "the leader's pole is full height");
    assert_eq!(height(2), expect(50), "half the leader's crowns is half the pole");
    assert_eq!(height(3), expect(0), "nothing in the bank is still a pole");

    // And a realm out of play is not drawn at all.
    game.kingdom.realms[5].in_play = false;
    let mut canvas = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        <NoblesScreen as l2_game::screen::Screen>::draw(&mut screen, &ctx, &mut canvas);
    }
    let slot = nobles::COLUMN_REALM.iter().position(|&x| x == 5).expect("a column");
    let x = (nobles::COLUMN_X[slot] + nobles::POLE[2].0) as usize;
    assert_eq!(
        (0..480).filter(|&y| canvas.at(x, y) == nobles::POLE[2].1).count(),
        0,
        "a realm out of play has no pole",
    );
}

