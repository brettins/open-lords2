#![allow(unused_imports)]
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

/// Zooming out reaches the rest of the map, and scrolling moves the near view.
/// Both halves matter: a viewport that could not move would be the minimap the
/// user complained about, in a smaller rectangle.
#[test]
fn zooming_out_shows_more_of_the_map_and_scrolling_moves_the_near_view() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before = draw(&mut screen, &mut game, &assets);
    let near_visible = visible_counties(&screen);

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    let far = draw(&mut screen, &mut game, &assets);
    let far_visible = visible_counties(&screen);
    assert!(
        far_visible > near_visible,
        "the far view shows {far_visible} counties and the near one {near_visible}"
    );
    // At the far zoom's pinned origin all fourteen are reachable.
    // The original disables scrolling there.
    assert_eq!(far_visible, 14);
    assert!(before.diff_count(&far) > 10_000, "and it is a different picture");

    // Back in, and now scroll. One step is one map tile, so the origin moves by
    // exactly one lattice column and the picture must change.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    let home = screen.viewport();
    let a = draw(&mut screen, &mut game, &assets);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(screen.viewport().col, home.col + 1);
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 10_000, "scrolling one column must repaint the map");
}

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

/// **The county town is not four quarries.**
///
/// `L2_maps.dat` stores a town's 2 × 2 block as `Town1a.pl8` frames 0 … 3, and
/// those four frames are the *stone quarry* artwork — which is exactly how
/// `County_PlaceResourceSites` identifies a quarry (frame 0 stone, 20 wood, 30
/// iron). The original never shows them: `Counties_PlaceSites` re-stamps the
/// block to frames 47 … 50, 51 … 54 or 55 … 58 by the county's population, and
/// the population pass re-stamps it every season.
///
/// We rendered the stored bytes, so every town on the map came out as four
/// pits. This asserts the rewrite, and that it is population-banded.
#[test]
fn every_county_town_is_re_stamped_off_the_quarry_frames_and_onto_a_village() {
    let (mut game, assets) = world!();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let overrides = MapScreen::town_graphics(&ctx);
    assert!(!overrides.is_empty(), "fourteen towns were rewritten");

    let mut towns = 0;
    for id in ctx.game.kingdom.county_ids() {
        let tiles = MapScreen::town(&ctx, id as u8);
        assert_eq!(tiles.len(), 4, "county {id}'s town is a 2 x 2 block");
        let pop = ctx.game.kingdom.counties[id].population;
        let base: u8 = if pop < 801 {
            47
        } else if pop < 1201 {
            51
        } else {
            55
        };
        let mut frames = Vec::new();
        for tile in tiles {
            let (x, y) = l2_kingdom::map::coords(tile);
            let (bank, frame) = overrides.get(x as usize, y as usize).expect("a town tile");
            assert_eq!(bank, 0x0C, "the town stays in the Town1a bank");
            assert!(
                (base..base + 4).contains(&frame),
                "county {id} has {pop} people, so its town is frames {base}..{}; got {frame}",
                base + 4
            );
            frames.push(frame);
        }
        frames.sort_unstable();
        assert_eq!(frames, vec![base, base + 1, base + 2, base + 3], "one of each quadrant");
        towns += 1;
    }
    assert_eq!(towns, 14, "England has fourteen counties and fourteen towns");
}

/// And it reaches the picture: **the same viewport, painted twice** — once
/// through the override and once straight from the file — differs, and it
/// Differs by about the area of four tiles.
///
/// Both halves go through `campaign::draw` and nothing else, so nothing but the
/// tile frames can account for the difference. Reverting the rewrite turns this
/// test red.
#[test]
fn the_rewritten_town_actually_changes_what_is_drawn() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    // Put county 8's town — the player's — in the middle of the view.
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = *MapScreen::town(&ctx, 8).first().expect("county 8 has a town");
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let ctx = Ctx { game: &mut game, assets: &assets };
    let overrides = MapScreen::town_graphics(&ctx);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = l2_view::campaign::Lattice::build(&slot);
    let paint = |o: &l2_view::campaign::Overrides| {
        let mut canvas = Canvas::screen();
        let mut tags = l2_view::Tags::screen();
        l2_view::campaign::draw(
            &mut canvas,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            o,
            game.kingdom.season,
            None,
        );
        canvas
    };
    let with = paint(&overrides);
    let without = paint(&l2_view::campaign::Overrides::new());

    // A near-zoom tile is 58 x 30 and its diamond is about half of that, so one
    // town is four of them - somewhere around 3,500 pixels. Two towns can be in
    // view at once, so the ceiling is generous; the floor is what matters.
    let moved = with.diff_count(&without);
    assert!(moved > 500, "the town's tiles are painted from different frames: {moved} pixels");
    assert!(moved < 30_000, "and only the towns changed, not the whole viewport: {moved}");
}

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

/// **Every painted pixel of the mine switches the mine.**
///
/// A player reported *"I can't click the iron mine on the world map to
/// enable/disable that"*, and he was describing geometry. `Town1a.pl8` frame 30
/// is 58 × 47 on a 58 × 30 tile: seventeen rows of headframe hang above the
/// tile's diamond, and more of the building falls inside the diamond's bounding
/// box but outside the rhombus. Swept pixel by pixel against the old hit test,
/// **1,314 pixels of the mine were painted and only 857 of them were on the
/// tile** — the entire upper half of the building was dead, and a click there
/// fell through to "open the county panel" instead.
///
/// The sweep is the assertion. It is not vacuous in either direction: the frame
/// really does overhang (asserted), and a pixel *outside* the building that is
/// also outside the diamond must still not toggle, or the fallback would be a
/// bounding box and not a mask.
#[test]
fn every_painted_pixel_of_a_mine_reaches_the_industry_toggle() {
    let (mut game, assets) = world!();
    // A county the player holds that has a mine, from the save.
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8) && game.kingdom.counties[id].industry[1].has_resource)
        .expect("the player starts with a mine somewhere");
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = MapScreen::settlements_for_test(&ctx, county as u8)
        .into_iter()
        .find(|&t| ctx.game.kingdom.campaign.map.terrain[t] == 1)
        .expect("and that county has an iron site on the map");
    let (tx, ty) = l2_kingdom::map::coords(tile);

    let mut screen = MapScreen::new();
    game.select(county as u8);
    screen.centre_on_tile(tx as usize, ty as usize);
    draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let frame = slot.at(l2_formats::maps::Plane::GfxIndex, tx as usize, ty as usize) as usize;
    let sheet = assets.map.bank(screen.zoom(), game.kingdom.season, 3).expect("the Town bank");
    let art = sheet.frame(frame).expect("the mine's frame");
    let overhang = art.height as i32 - screen.zoom().tile_h;
    assert!(overhang > 0, "the mine overhangs its tile; without that this test proves nothing");

    let mut painted = 0;
    let mut reached = 0;
    let mut off_the_art_and_off_the_tile = 0;
    for dy in 0..art.height as i32 {
        for dx in 0..art.width as i32 {
            let opaque = art.opaque[dy as usize * art.width as usize + dx as usize];
            let before = game.kingdom.counties[county].industry[1].enabled;
            let (x, y) = (sx + dx, sy - overhang + dy);
            send(&mut screen, &mut game, &assets, Event::Click { x, y });
            let toggled = game.kingdom.counties[county].industry[1].enabled != before;
            if opaque {
                painted += 1;
                reached += usize::from(toggled);
            } else if toggled && dy < overhang {
                // Above the diamond entirely, and not on the building.
                off_the_art_and_off_the_tile += 1;
            }
        }
    }
    assert!(painted > 1_000, "the mine is a building, not a smudge: {painted} pixels");
    assert_eq!(reached, painted, "every painted pixel of the mine must switch it");
    assert_eq!(
        off_the_art_and_off_the_tile, 0,
        "the fallback is the frame's opacity mask, not its bounding box"
    );
}

/// The minimap is the original's own raster out of `MAPnn.PL8`, and clicking it
/// selects the county under the pixel *and* moves the viewport onto it.
///
/// This is the one place we use the original's algorithm and reach its
/// answer: `Minimap_Click` reads the same county byte out of the same file.
#[test]
fn clicking_the_minimap_selects_that_county_and_brings_it_into_view() {
    let (mut game, assets) = world!();
    let minimap = assets.minimap(game.map_slot).expect("Map01.pl8 holds slot 0");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let visible = pick_counts(&screen);

    // A minimap pixel of a county that is *not* in the opening view, so "the
    // map moved onto it" cannot pass by accident.
    let (mx, my) = (0..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .map(|(x, y)| (x + chrome::MINIMAP_HIT_X, y + chrome::MINIMAP_HIT_Y))
        .find(|&(x, y)| {
            let c = minimap.county_at(x, y);
            c != 0 && (c as usize) < 17 && visible[c as usize] == 0
        })
        .expect("some county is off screen at the opening viewport");
    let county = minimap.county_at(mx, my);

    let before = screen.viewport();
    send(&mut screen, &mut game, &assets, Event::Click { x: mx, y: my });
    assert_eq!(game.selected, county, "the raster decides which county");
    assert_ne!(screen.viewport(), before, "and the map moves");

    // Having moved, that county is now on screen — which is what "centred"
    // The origin's changing does not imply this.
    draw(&mut screen, &mut game, &assets);
    assert!(pick_counts(&screen)[county as usize] > 0, "county {county} is now in view");
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

/// The menu bar reads the clock and the treasury out of the world, and the
/// right column reads the selected county. Checked by finding the actual
/// digits, at the coordinates `Screen_DrawMenuBar` puts them.
#[test]
fn the_map_chrome_shows_the_clock_the_treasury_and_the_selected_county() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    // **The year, then the season, in `g_fontBody` — not our 5 × 7 font, and
    // not in that order.** This used to look for `"WINTER 1268"` in `ink.text`,
    // which is what the two `l2_view::text::draw` calls at the tail of
    // `draw_menu_bar` drew and what a player called *"still placeholder font in
    // the top right for gold and summer"*. `Screen_DrawMenuBar` draws
    // `Ui_DrawYear(g_year, 0x168, 6, 3)` and puts the season at
    // `g_penAdvance + 0x16C`, both in `&g_fontBody` at colour `0x3F`.
    let clock = find_body(&canvas, &assets, " 1268 ", font::TEXT).expect("the year");
    assert_eq!(clock, (360, 6), "the original draws the year at x 360, y 6");
    let season = find_body(&canvas, &assets, "Winter", font::TEXT).expect("the season");
    assert!(season.0 > clock.0, "the season follows the year, at g_penAdvance + 0x16C");
    // `Ui_DrawCount(gold, 0, 500, 6, &g_fontBody, 0x3F)` — the number and then
    // `L2.eng` group 8's *"Crowns."*. `GOLD` was a word of ours.
    let gold = find_body(&canvas, &assets, "1000 ", font::TEXT).expect("the treasury");
    assert_eq!(gold.1, 6, "and the treasury on the same row");
    assert!(find_body(&canvas, &assets, "Crowns.", font::TEXT).is_some(), "with its noun");
    // `TURN n` and `COUNTIES n/m` are ours and are debug overlay now: a normal
    // session draws neither (`the_debug_overlay_is_off_by_default_…`).
    assert!(find_text(&canvas, "TURN 1", ink.dim).is_none());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_none());

    // **The county strip, in the map's own sidebar.** `Screen_DrawCampaign`
    // calls `CountyStrip_Draw` — the map screen used to leave that plate empty
    // and write a box of our own numbers over the jobs plate below it.
    // `Ui_DrawNumber`'s `x` is the *string's* origin and the string opens with
    // the sign column, so the digits sit one `SPACE_ADVANCE` right of the call
    // site's literal. The strip's own test carries the whole argument.
    let lead = l2_game::shell::font::SPACE_ADVANCE;
    let pop = find_strip(&canvas, &assets, "435", STRIP_INK).expect("the population");
    assert_eq!(pop, (0x1FC + lead, 189), "at CountyStrip_Draw's own coordinates");
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left origins, not right-anchored.** `docs/decisions.md` C42.
        Some((0x25A + lead, 189)),
        "and the happiness beside it"
    );

    // The near-misses. If the search could match anything it would match these.
    assert!(find_body(&canvas, &assets, " 1269 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "1001 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "Summer", font::TEXT).is_none());
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
}

// ---------------------------------------------------------------------------
// **Pixels, asserted.** C59.
//
// Three visual features have now been reported missing by a human *after* being
// merged — the town flag, the merchant sprite, the minimap tint. Two of the
// three did have a pixel test:
// [`the_county_town_flies_its_owners_flag_and_it_waves`] and
// [`a_merchant_is_drawn_and_opens_the_merchant_screen_from_the_county_it_is_in`]
// both diff two canvases and assert the ink landed in the right box. **The
// minimap's tint had none**, and it is the one the player was still describing
// as wrong.
//
// Both tests below are stronger than a diff, in the same way: a diff says
// *something* changed inside a rectangle, so it passes on a garbage sprite or
// on the wrong frame of the right sheet. These match the **artwork itself** —
// the frame's own palette indices, several hundred of them, standing where the
// blit put them — and then vary one field of the save and require the picture
// to follow it. That is what makes them able to catch a flag that draws, but
// draws the wrong realm's.
//
// The house technique is to turn the visual claim into a number the file can
// settle and then assert the number. Two precedents: *a tick cannot come 4th of
// 84 frames by ink*, and *a two-pixel ring is four pixels of width*.
// ---------------------------------------------------------------------------

