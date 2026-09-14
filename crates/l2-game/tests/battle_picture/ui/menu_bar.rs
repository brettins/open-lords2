#![allow(unused_imports)]
use super::*;
use super::battle_events::*;
use super::*;
use super::render::*;
use super::motion::*;
use super::panel::*;
use super::entities::*;
use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::menubar;
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// **The bar's three titles are on the battlefield, and they answer.**
///
/// The probe is `Menu_HitTitle`'s own geometry through the install's own font
/// the arm hit-tests. The near-miss is the 32 pixels `g_penAdvance += 0x20`
/// leaves after a title, which `Menu_HitTitle` does **not** answer —
/// that passed by clicking anywhere in the band fails here.
///
/// Ablation: drop the `menubar::title_at` guard from the `0x29` ladder — red on
/// the push; drop the `draw_menu_bar` call — red on the caption.
#[test]
fn the_menu_bar_is_live_on_the_battlefield() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install, so the bar has no words and no font");
    };
    let (mut g, mut m) = skirmish();
    let titles = {
        let ctx = Ctx { game: &mut g, assets: &assets };
        menubar::titles(&ctx)
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 loaded");

    // --- painted ------------------------------------------------------------
    let mut canvas = Canvas::screen();
    frame(&mut m, &mut g, &assets, &mut canvas);
    for i in 0..3 {
        let caption = {
            let ctx = Ctx { game: &mut g, assets: &assets };
            menubar::title_text(&ctx, i)
        };
        assert!(
            find_font_text(&canvas, body, &caption, l2_game::shell::font::TEXT).is_some(),
            "{caption:?} is not on the battlefield's menu bar"
        );
    }

    // --- and they open ------------------------------------------------------
    //
    // `Menu_OpenDropdown` writes `g_screenId = 0x32`, which is a push here.
    let at = (titles[0].x + titles[0].w / 2, titles[0].y + titles[0].h / 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: at.0, y: at.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)), "File did not open over the battle");

    // The near-miss: the dead 32 pixels after the last title. `Menu_HitTitle`
    // walks three records, returns 0, and the ladder falls through.
    let (mut g2, mut m2) = skirmish();
    let gap = (titles[2].x + titles[2].w + 16, titles[2].y + titles[2].h / 2);
    send(&mut m2, &mut g2, &assets, Event::Pointer { x: gap.0, y: gap.1 });
    send(&mut m2, &mut g2, &assets, Event::Click { x: gap.0, y: gap.1 });
    assert_eq!(m2.top_id(), Some(ScreenId::Battlefield), "the gap between titles opened a menu");
}

/// **What a battle takes off the bar is the shields and the date, and nothing
/// else** — `Screen_DrawMenuBar`'s two `if (g_battlePhase == 0)` guards, neither
/// of which is on a title or on the treasury.
///
/// Asserted as the difference between the same painter's two callers rather
/// than against a coordinate: the season must be absent and the gold present. A
/// guard written the wrong way round fails one half or the other.
#[test]
fn a_battle_takes_the_date_off_the_bar_and_leaves_the_treasury() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install, so the bar has no words and no font");
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 loaded");
    let (mut g, mut m) = skirmish();
    g.kingdom.realms[1].gold = 1234;
    // **Group 29 is 1-based**: index 0 is *"No Season"*, which is what a
    // default-constructed kingdom reads and which would make the probe a
// fallback string
    g.kingdom.season = l2_kingdom::tables::Season::Summer as u8;
    let season = l2_game::screens::map::season_text(&assets, g.kingdom.season);
    assert_eq!(season, "Summer", "the probe must be L2.eng group 29's own word");

    let mut canvas = Canvas::screen();
    frame(&mut m, &mut g, &assets, &mut canvas);
    assert!(
        find_font_text(&canvas, body, &season, l2_game::shell::font::TEXT).is_none(),
        "{season:?} is on the bar during a battle; g_battlePhase guards it"
    );
    assert!(
        find_font_text(&canvas, body, "1234", l2_game::shell::font::TEXT).is_some(),
        "the treasury is not on the bar during a battle; it is outside both guards"
    );
}

/// **The battle goes on under an open menu** — `Battle_Frame`'s
/// `else if ((g_battlePhase == 2) && ticksDue)` arm, which has no `g_screenId`
/// test any more than the turn's arm above it does.
///
/// The drop-down is a screen, so opening *File* puts a screen over the
/// battlefield, and `Screen::update` is the top screen's alone. Measured on the
/// simulation's own `BattleRunner::tick`
///
/// Ablation: delete the `wind_battle` call from `Machine::wind_turn` — the
/// second count comes out zero and this goes red.
#[test]
fn a_battle_runs_on_under_an_open_menu() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install");
    };
    let (mut g, mut m) = skirmish();
    let titles = {
        let ctx = Ctx { game: &mut g, assets: &assets };
        menubar::titles(&ctx)
    };

    // Ten frames with nothing over the battlefield, for the reference.
    let mut canvas = Canvas::screen();
    let before = live(&g).runner.tick;
    for _ in 0..10 {
        frame(&mut m, &mut g, &assets, &mut canvas);
    }
    let plain = live(&g).runner.tick - before;
    assert!(plain > 0, "the battle did not tick at all with nothing over it");

    // Open File, and run ten more.
    let at = (titles[0].x + titles[0].w / 2, titles[0].y + titles[0].h / 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: at.0, y: at.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)), "File did not open");
    let under = live(&g).runner.tick;
    for _ in 0..10 {
        frame(&mut m, &mut g, &assets, &mut canvas);
    }
    assert_eq!(
        live(&g).runner.tick - under,
        plain,
        "the battle froze under the open menu; Battle_Frame has no g_screenId test"
    );
}

/// **The battle screen's right column is 160 pixels wide and the campaign map's
/// is 162.**
///
/// A player asked: *"The sidebar iirc was much smaller in combat? or something?
/// I might be misremembering."* Measured, and the two columns differ by **two
/// pixels**. What differs is what is *in* them: `CountyStrip_Draw`
/// (`0x0040F7D3`) and the campaign minimap (`FUN_00410F4B`) both return at once
/// when `g_battlePhase != 0`, so the strip, the produce rows, the labour slider
/// and the five sidebar buttons are off this screen, and an overview, a banner
/// grid and five battle buttons are on it instead.
///
/// Every number is a literal from a hit test or a blit, never an expression of
/// the constant under test:
///
/// * `Hotspot_Test(0x1E0, 0x1C0, &DAT_004DC710, 5)` — `Battle_ButtonClicked`;
/// * `BattleMap_Click` rejects `g_mouseX < 0x1E0 || 0x27F < g_mouseX` and
///   `g_mouseY < 0x18 || 0xB7 < g_mouseY`;
/// * `FUN_004BC020(…, 0, 0x18, 0xF, 0xE, 0x20)` — the field, 15 × 14 tiles of
///   32 from (0, 24), whose right edge is therefore 480;
/// * `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` — the campaign map's,
///   two pixels to the left of the battle's.
#[test]
fn the_battle_column_is_two_pixels_narrower_than_the_campaign_one() {
    const SCREEN_W: i32 = 640;
    // `BattleMap_Click`'s own bounds, written out.
    assert_eq!((bf::OVERVIEW.x, bf::OVERVIEW.y), (0x1E0, 0x18));
    assert_eq!(bf::OVERVIEW.x + bf::OVERVIEW.w - 1, 0x27F);
    assert_eq!(bf::OVERVIEW.y + bf::OVERVIEW.h - 1, 0xB7);
    // `Hotspot_Test(0x1E0, 0x1C0, …, 5)` — the five buttons fill the column.
    assert_eq!(bf::BUTTON_ORIGIN, (0x1E0, 0x1C0));
    assert_eq!(bf::BUTTON_SIZE * bf::BUTTON_COUNT as i32, SCREEN_W - 0x1E0);
    // The field stops exactly where the column starts.
    assert_eq!(bf::VIEW.x + bf::VIEW.w, 0x1E0);
    assert_eq!((bf::VIEW.x, bf::VIEW.y, bf::VIEW.h), (0, 0x18, 14 * 32));

    // And the campaign map's column, which is the thing being compared with.
    assert_eq!(
        l2_view::campaign::PANEL_X,
        0x1DE,
        "Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)"
    );
    assert_eq!(
        (SCREEN_W - 0x1E0, SCREEN_W - l2_view::campaign::PANEL_X),
        (160, 162),
        "the battle column is 160 wide and the campaign column 162"
    );
}

