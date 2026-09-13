//! **The mouse pointer changes, and the village is where a player noticed it
//! did not.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!   cargo test -p l2-game --test cursor
//! ```
//!
//! `Cursor_Set` (`0x004B1CF3`) is the binary's only caller of `SetCursor`;
//! `Battle_Frame` (`0x004B99C0`) is its only caller and chooses two ways — the
//! battlefield hover ladder, and `g_cursorByScreen[g_screenId]` (`0x004E3098`)
//! for everything else. `docs/screens.md` §9.
//!
//! The village occupies **three** screen ids and the table reads all three
//! differently: `0x02` idle is the question mark, `0x05` the rubber band falls
//! through to the arrow, `0x06` the carried selection is the peasant figure. A
//! player reported the question mark missing and, unprompted, *"only when
//! you're not selecting"* — which is the table, row for row.
//!
//! **Ablation, run:** `VillageScreen::mode_screen_id` answering `None` turns
//! `the_band_is_the_plain_arrow` and `carrying_a_selection_is_the_peasant` red
//! (both fall back to `0x02`'s question mark); `cursor::by_screen` answering
//! `Pointer::Arrow` for everything turns three of the four red.

use std::path::PathBuf;

use l2_game::cursor::{by_screen, Pointer};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::village as vill;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no village grid to drag on");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

fn handle(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

/// `g_cursorByScreen` is a constant of the image and needs no install: 64
/// dwords, five of them non-zero.
#[test]
fn the_table_is_its_five_non_zero_rows() {
    for screen in 0u8..64 {
        let want = match screen {
            0x02 | 0x0E => Pointer::Question,
            0x06 => Pointer::Peasant,
            0x07 => Pointer::ArrowAlt,
            // `Map_BeginMoveSelection` (`0x0043723A`) is the only writer of
            // `0x10`: choosing where an army marches.
            0x10 => Pointer::Scythe,
            _ => Pointer::Arrow,
        };
        assert_eq!(by_screen(screen), want, "g_cursorByScreen[{screen:#04x}]");
    }
    // The kinds `Cursor_Set` switches on.
    assert_eq!(Pointer::Question.kind(), 2);
    assert_eq!(Pointer::Peasant.kind(), 12);
    assert_eq!(Pointer::Scythe.kind(), 14);
}

/// **The pointer over the town square, and off it again.**
#[test]
fn the_village_is_the_question_mark_and_the_map_is_not() {
    let (mut game, assets) = world!();
    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    assert_eq!(m.pointer(&game), Pointer::Arrow, "the campaign map: g_screenId 0, kind 0");

    m.push(ScreenId::Village(county));
    assert_eq!(m.pointer(&game), Pointer::Question, "the village idle: 0x02, kind 2");

    // Off it: the OK button closes the village and the arrow comes back.
    let ok = l2_game::screens::village::VillageScreen::ok_button(vill::SCENE_Y);
    // `Ui_OkButton` is hit-tested on the left **release** (`FUN_0040E7E4`).
    handle(&mut m, &mut game, &assets, Event::Click { x: ok.x + 4, y: ok.y + 4 });
    handle(&mut m, &mut game, &assets, Event::Release { x: ok.x + 4, y: ok.y + 4 });
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "the village closed");
    assert_eq!(m.pointer(&game), Pointer::Arrow, "and the pointer is the plain arrow again");
}

/// `0x05` — the band is being drawn, and the table gives that row nothing.
#[test]
fn the_band_is_the_plain_arrow() {
    let (mut game, assets) = world!();
    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    // A press and a drag past `FUN_004393EB`'s dead zone, with no release.
    let (x0, y0) = (vill::SCENE_X + 20, vill::SCENE_Y + 40);
    handle(&mut m, &mut game, &assets, Event::Click { x: x0, y: y0 });
    handle(&mut m, &mut game, &assets, Event::Pointer { x: x0 + 60, y: y0 + 30 });
    assert_eq!(m.top_screen_byte(&game), Some(0x05), "the drag reached 0x05");
    assert_eq!(m.pointer(&game), Pointer::Arrow);
}

/// `0x06` — a selection in hand is the peasant figure, the peasants you are
/// carrying.
#[test]
fn carrying_a_selection_is_the_peasant() {
    let (mut game, assets) = world!();
    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    let c = &game.kingdom.counties[county as usize];
    let icons = l2_game::screens::village::VillageScreen::icons(c);
    let (cluster, slot) = (0..vill::CLUSTER_COUNT)
        .flat_map(|cl| (0..vill::ICONS_PER_CLUSTER).map(move |s| (cl, s)))
        .find(|&(cl, s)| icons[cl][s] != 0 && icons[cl][s] != vill::ICON_SHORTFALL)
        .expect("somebody is standing in this village");
    let (px, py) = vill::icon_hit_point(cluster, slot, vill::SCENE_Y);
    handle(&mut m, &mut game, &assets, Event::Click { x: px - vill::DRAG_DEAD_ZONE, y: py });
    handle(&mut m, &mut game, &assets, Event::Pointer { x: px, y: py });
    handle(&mut m, &mut game, &assets, Event::Release { x: px, y: py });

    assert_eq!(m.top_screen_byte(&game), Some(0x06), "released holding a selection");
    assert_eq!(m.pointer(&game), Pointer::Peasant);
}
