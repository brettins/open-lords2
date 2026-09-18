#![allow(unused_imports)]
use super::*;
use super::scoring::*;
use super::geometry::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::nobles::{self, NoblesScreen};
use l2_game::Game;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

/// **The court's button opens the page** — `FUN_004351C4`, and it is kind 5,
/// so the press does not do it: the press starts the twenty-frame timer and
/// `Screen::update` runs the handler.
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
#[test]
fn the_button_rebuilds_the_realm_totals_the_page_reads() {
    let (mut game, assets) = bare_world();
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

/// `Transition::Goto(Campaign)`, `docs/decisions.md` C190.
#[test]
fn the_corner_picture_and_a_right_click_go_to_the_map() {
    let (mut game, assets) = bare_world();

    for (what, event) in [
        ("the corner picture", Event::Click { x: nobles::OK.x + 4, y: nobles::OK.y + 4 }),
        ("a right release", Event::RightClick { x: 320, y: 240 }),
        ("escape", Event::KeyDown(Key::Escape)),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        m.push(ScreenId::Court);
        m.push(ScreenId::Nobles);
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(event, &mut c);
        assert_eq!(m.ids(), vec![ScreenId::Campaign], "{what} goes to the map");
    }

    let mut m = Machine::new(ScreenId::Court);
    m.push(ScreenId::Nobles);
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: nobles::OK.x + 4, y: nobles::OK.y + 4 }, &mut c);
    }
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "the map is built if it was not open");

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Nobles);
    let mut c = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::Click { x: 320, y: 240 }, &mut c);
    assert_eq!(m.top_id(), Some(ScreenId::Nobles), "a press in the middle does nothing");
}

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

