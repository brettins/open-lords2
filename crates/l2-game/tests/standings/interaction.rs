#![allow(unused_imports)]
use super::*;
use super::scoring::*;
use super::geometry::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::nobles::{self, NoblesScreen};
use l2_game::Game;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

/// **The court's button opens the page** — `FUN_004351C4`, and it is kind 5,
/// so the press does not do it: the press starts the twenty-frame timer and
/// `Screen::update` runs the handler.
///
/// Ablation, run: answer the click with `Transition::Push` directly and the
/// first assertion fails, because the page would already be up.
#[test]
fn the_greatest_nobles_button_opens_the_standings_twenty_ticks_later() {
    let (mut game, assets) = bare_world();
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Court);

    let b = l2_game::screens::court::NOBLES_BUTTON;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: b.x + 4, y: b.y + 4 }, &mut c);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Court), "the press only starts the timer");

    let mut opened_at = None;
    for tick in 1..=64u32 {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.update(&mut c);
        if m.top_id() == Some(ScreenId::Nobles) {
            opened_at = Some(tick);
            break;
        }
    }
    assert_eq!(
        opened_at,
        Some(l2_game::screens::court::DEFERRED_FRAMES as u32),
        "the handler runs on the twentieth frame, as `Widget_Test` kind 5 does",
    );
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Court, ScreenId::Nobles]);
}

/// **The button runs the recount before the page is drawn** —
/// `FUN_00435211`, `Score_RankRealms()` then `Realm_UpdateTotals(r)` for
/// r in 1..=5.
///
/// This is the statement that would be easiest to drop and hardest to notice:
/// without it the page shows whatever the last AI turn left in the realm
/// totals. The test leaves a realm with a stale zero and asserts the button
/// fills it in.
///
/// Ablation, run: remove the `recount` call from `open_the_standings` and the
/// county count stays 0.
#[test]
fn the_button_rebuilds_the_realm_totals_the_page_reads() {
    let (mut game, assets) = bare_world();
    // Two counties for realm 1 and one for realm 2, with the realm records
    // left as a fresh game leaves them: zero.
    game.kingdom.county_count = 3;
    for (id, owner) in [(1u8, 1u8), (2, 1), (3, 2)] {
        game.kingdom.counties[id as usize].owner = owner;
        game.kingdom.counties[id as usize].population = 100;
    }
    for r in 1..=2usize {
        game.kingdom.realms[r].in_play = true;
        game.kingdom.realms[r].strength = 1;
        game.kingdom.realms[r].county_count = 0;
    }

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Court);
    let b = l2_game::screens::court::NOBLES_BUTTON;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: b.x + 4, y: b.y + 4 }, &mut c);
    }
    for _ in 0..l2_game::screens::court::DEFERRED_FRAMES {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.update(&mut c);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Nobles));
    assert_eq!(game.kingdom.realms[1].county_count, 2, "Realm_UpdateTotals ran");
    assert_eq!(game.kingdom.realms[2].county_count, 1);
    assert_eq!(game.kingdom.realms[1].population_total, 200);
}

/// **The seven tabs pick the category and ask for the name to be spoken** —
/// `FUN_0043524E`, which is `DAT_0055CE7C = g_uiHotspotId; g_redrawRequest =
/// 2; FUN_004B3994(g_uiHotspotId);`.
///
/// Two things are asserted that a simpler wiring would fail: the id comes from
/// **which** tab was hit, and the counter
/// moves even when the tab pressed is the one already showing — because the
/// original's call is unconditional and a diff on the category would swallow
/// that press.
#[test]
fn each_tab_selects_its_own_category_and_speaks_it() {
    let (mut game, assets) = bare_world();
    let mut m = Machine::new(ScreenId::Nobles);

    for (i, tab) in nobles::TABS.iter().enumerate() {
        let before = game.nobles_spoken;
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: tab.centre_x(), y: tab.y + 4 }, &mut c);
        assert_eq!(game.nobles_category as usize, i, "tab {i} selects category {i}");
        assert_eq!(game.nobles_spoken, before + 1, "tab {i} asks to be spoken");
        assert_eq!(m.top_id(), Some(ScreenId::Nobles), "a tab does not leave the page");
    }

    // The same tab again: the category does not move and the voice still does.
    let tab = nobles::TABS[nobles::CATEGORIES - 1];
    let before = game.nobles_spoken;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: tab.centre_x(), y: tab.y + 4 }, &mut c);
    }
    assert_eq!(game.nobles_category as usize, nobles::CATEGORIES - 1);
    assert_eq!(game.nobles_spoken, before + 1, "FUN_0043524E plays unconditionally");

    // A press just below the row does nothing — the ablation that stops the
    // arm being "anything in the bottom third of the screen".
    let before = (game.nobles_category, game.nobles_spoken);
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: tab.centre_x(), y: nobles::TAB_Y1 + 4 }, &mut c);
    }
    assert_eq!((game.nobles_category, game.nobles_spoken), before, "outside the row, nothing");
}

/// **The two ways out**, which is the whole of `Screen_FrameInput`'s `0x20`
/// ladder — `Ui_OkButtonClicked` in the corner box, and a right release
/// anywhere.
///
/// The original sets `g_screenId = 0` — the map — and ours pops onto the court
/// the button was pressed on. `docs/arms.json`
/// `0x0042FF10/standings-ok` records the difference and why it is deliberate.
#[test]
fn the_corner_picture_and_a_right_click_leave_the_page() {
    let (mut game, assets) = bare_world();

    for (what, event) in [
        ("the corner picture", Event::Click { x: nobles::OK.x + 4, y: nobles::OK.y + 4 }),
        ("a right release", Event::RightClick { x: 320, y: 240 }),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        m.push(ScreenId::Court);
        m.push(ScreenId::Nobles);
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(event, &mut c);
        assert_eq!(m.top_id(), Some(ScreenId::Court), "{what} leaves the page");
    }

    // A left press anywhere that is not the corner box and not a tab keeps it
    // up: the arm consults `Ui_OkButtonClicked` and the tab table, nothing
    // else. The ratings screen next door closes on *any* press, and this one
    // must not be given that behaviour by accident.
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Nobles);
    let mut c = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::Click { x: 320, y: 240 }, &mut c);
    assert_eq!(m.top_id(), Some(ScreenId::Nobles), "a press in the middle does nothing");
}

/// **The page does not swallow the campaign minimap**, which is
/// `Screen_FrameInput`'s epilogue and belongs to every screen but `0x12`.
#[test]
fn the_page_passes_the_minimap_down() {
    let (mut game, assets) = bare_world();
    let hit = l2_view::chrome::minimap_hit_area();
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Nobles);
    let mut c = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::Click { x: hit.x0 + 40, y: hit.y0 + 40 }, &mut c);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the map is revealed");
    assert_eq!(m.depth(), 1);
}

