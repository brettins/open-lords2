#![allow(unused_imports)]
use super::*;
use super::view_and_navigation_tests::*;
use super::town_and_mine_tests::*;
use super::chrome_tests::*;
use super::*;
use super::view_tests::*;
use super::structures_tests::*;
use super::fog_and_march_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

/// **A click on a county's open ground does not select it. `Map_Click` has no
/// county-selection arm at all.**
///
/// This test has now been wrong twice, in opposite directions, and both times
/// the error was a reading of the same 1,263-byte function.
///
/// It first asserted that a second click on the selected county opened its tax
/// panel — our convenience, which a player reported: *"there's some weird thing
/// where if you click anywhere on grass it opens up the tax window too."* It
/// was then rewritten to assert that the click *selects and recentres*, on the
/// A "last arm" is not quoted into three documents.
/// The quoted code is the **prologue of the industry branch**, guarded by tile
/// flag `0x80` and by the county being the local player's, and `Map_Click`'s
/// three writes to `g_selectedCounty` are all inside branches that open
/// something: the village, an industry toggle, the merchant.
///
/// So selection from the map is a *side effect of arriving somewhere*, never a
/// verb of its own. `docs/decisions.md` C61.
#[test]
fn a_click_on_a_countys_open_ground_selects_nothing() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    let counts = pick_counts(&screen);
    let target = (1..=14u8).max_by_key(|&id| counts[id as usize]).expect("a county is visible");
    let (px, py) = pixel_of(&screen, target).expect("and it has a pixel");

    game.select(0);
    let before = game.kingdom.clone();
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Stay, "it opens nothing");
    assert_eq!(game.selected, 0, "and selects nothing");
    assert_eq!(game.kingdom, before, "and changes no part of the world");
}

/// **A click on plain ground changes nothing at all** — not the screen, not the
/// selection, not one byte of the kingdom.
///
/// This is the assertion that could not exist while we had a county-selection
/// arm that opened a panel, and it is the one that would have caught all three
/// of the hit-test defects a player found in a single evening: the mine's dead
/// upper half (C57), the merchant's nine-pixel box (C58) and `pick_tile`'s 56
/// dead pixels around every tile centre (C60). Every one of them was a
/// *geometric* shortfall, and every one became **the wrong screen opening**
/// A miss had somewhere to fall
/// through to. It asserts over the whole state,
/// because "nothing happened" is the claim.
#[test]
fn a_click_on_plain_ground_changes_nothing_at_all() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // The sea is the one thing on the map guaranteed to carry no county, no
    // unit and no flags.
    let (sx, sy) = pixel_of(&screen, 0).expect("there is sea");

    let selected_before = game.selected;
    let before = game.kingdom.clone();
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: sx, y: sy });

    assert_eq!(t, Transition::Stay, "a click on nothing opens nothing");
    assert_eq!(game.selected, selected_before, "and does not clear the selection either");
    assert_eq!(game.kingdom, before, "and changes no part of the world");
}

/// A click picks the county the player can see at that pixel. The
/// pixel is found through the pick plane, so this exercises exactly the path
/// the mouse takes.

/// **Clicking the town opens the village**, which is `Map_Click`'s second arm:
/// `if (flags & 0x40) { g_screenId = 2; Village_Draw(1); }`. That arm was
/// missing, so the click fell through to "select the county" and the village
/// had no route in but a key of ours.
#[test]
fn clicking_the_county_town_centres_the_map_on_it_and_opens_the_village() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = *MapScreen::town(&ctx, 8).first().expect("county 8 has a town");
    let (tx, ty) = l2_kingdom::map::coords(tile);

    game.select(8);
    screen.centre_on_tile(tx as usize, ty as usize);
    draw(&mut screen, &mut game, &assets);
    let (cx, cy) =
        l2_view::campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize)
            .expect("the town is on screen after centring on it");

    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Village(8)), "the town opens the village");
}

/// **The selection is not drawn on the map at all**, which is the assertion
/// that could not exist while our yellow outline did.
///
/// The original draws no selection over the terrain: its borders live in the
/// tile data — `docs/formats/maps-layers.md` §2.1, plane-0 bit `0x02` — and its
/// *selection* is which county the right panel describes. Ours outlined the
/// Selected county in the highlight colour.
/// A yellow outline around the county, selected on the
/// map."*
///
/// This is the inverse of the test it replaces, and it is a stronger claim than
/// *"the outline is gone"*: it says the **map area is byte-identical** under
/// two different selections, so any future selection paint — an outline, a
/// tint, a halo — fails it, not only the one that was removed. The right column
/// is deliberately outside the window, because that is where the selection
/// legitimately shows.
///
/// **The counties are derived, not named**, and none of them is the player's.
/// The field markers under `brush` were drawn for
/// the *selected* county when the player owns it — the visible half of
/// `ours/brush-popup-on-the-map`, since removed, and the markers now debug
/// overlay only. Naming two counties by number would have made this
/// test a statement about one fixture's ownership roll, which
/// `docs/environment.md` says is rolled per game.
///
/// # This test passed with the outline put back, and that is why it looks like
/// this
///
/// The first version took the **first two** counties the player does not own —
/// 1 and 2 on the England fixture — and required the map to be byte-identical
/// between them. The map the fixture opens on shows counties **8 and 9**.
/// Neither 1 nor 2 has a single pixel on screen, so the two canvases were
/// Identical for a reason independent of what the painter draws,
/// and re-adding the yellow outline turned **nothing** red. The claim
/// `docs/decisions.md` C132 makes for it — *"a stronger claim than the outline
/// is gone, because any future selection paint fails it"* — was false as
/// written. `docs/decisions.md` C138.
///
/// So the shape here is: a **baseline** selection that is off-camera, and then
/// Every foreign county that is *on* camera selected in turn against
/// it. The `assert!` that at least one county is visible is what stops the
/// sweep being empty, which is the only way this can go vacuous again.
#[test]
fn the_selection_is_not_drawn_on_the_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // Which counties the opening viewport shows, read off the pick
    // plane — the same plane a click goes through, so "visible" here means the
    // painter put pixels of it on screen.
    let counts = pick_counts(&screen);
    let foreign_and_visible: Vec<u8> = (1..=14u8)
        .filter(|&id| counts[id as usize] > 0 && !game.is_players(id))
        .collect();
    let foreign_and_hidden: Vec<u8> = (1..=14u8)
        .filter(|&id| counts[id as usize] == 0 && !game.is_players(id))
        .collect();
    assert!(
        !foreign_and_visible.is_empty(),
        "nothing the player does not own is on screen, so this test would assert nothing. \
         Visible: {:?}",
        (0..=14u8).filter(|&id| counts[id as usize] > 0).collect::<Vec<_>>(),
    );
    let baseline = *foreign_and_hidden.first().expect("England has a county off the opening view");

    // `Map_SetZoom` gives the map 480 pixels at every zoom and the right column
    // the rest.
    let map_area = |a: &Canvas, b: &Canvas| {
        let mut differ = 0;
        for y in 0..480usize {
            for x in 0..480usize {
                if a.at(x, y) != b.at(x, y) {
                    differ += 1;
                }
            }
        }
        differ
    };

    game.select(baseline);
    let base = draw(&mut screen, &mut game, &assets);

    for id in foreign_and_visible {
        game.select(id);
        let with = draw(&mut screen, &mut game, &assets);
        let differ = map_area(&base, &with);
        assert_eq!(
            differ, 0,
            "county {id} fills {} pixels of the viewport, and selecting it instead of the \
             off-screen county {baseline} changed {differ} of them. The original draws no \
             selection over the terrain at all.",
            counts[id as usize],
        );
    }
}

