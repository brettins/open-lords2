#![allow(unused_imports)]
use super::*;
use super::castles::*;
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
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

/// **The minimap tints by owner, and a realm's ramp is its own.**
///
/// A player: *"the minimap had default colors, it didn't identify who
/// owned a county."* The number that settles it is the count of distinct ramp
/// **rows** standing in the minimap rectangle. `MINIMAP_REALM_RAMP` is six rows
/// of four shades — row 0 the raster's own shading for unowned land, rows 1 … 5
/// one per realm colour — so:
///
/// * a minimap that tints by owner shows **one row per owning colour, plus row
///   0 wherever land is unowned**;
/// * a minimap that has lost the tint shows **exactly one row**, because
///   `chrome::realm_colour` clamps a zero colour *up* to 1 and every county
///   would collapse onto ramp 1.
///
/// On the England fixture that is five owned counties flying five different
/// colours out of fourteen, so six rows against one. The two cannot be
/// confused, which is the property that makes this worth asserting.
///
/// It also asserts the thing that makes the count mean anything: **no palette
/// index appears in two rows**, so "which row is this pixel from" has one
/// answer. C59.
#[test]
fn the_minimap_paints_one_ramp_row_per_owning_realm() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    // The ramp must be unambiguous before it can be counted.
    let mut row_of_index = [None::<usize>; 256];
    for (row, shades) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        for &i in shades {
            assert_eq!(
                row_of_index[i as usize], None,
                "palette index {i:#04x} is in two ramp rows, so a pixel cannot name its realm"
            );
            row_of_index[i as usize] = Some(row);
        }
    }

    // **The measurement is a difference, not a census**, and it has to be.
    // `MAPnn.PL8`'s raster also carries pixels the overlay leaves alone —
    // coastline, borders, the panel round it — and some of those indices are
    // by coincidence entries of a ramp row (0x38, the beige, is row 3's
    // darkest). Counting colours across the whole rectangle would therefore
    // report a realm nobody owns. What is unambiguous is what *moves* when the
    // ownership moves: the raster is byte-for-byte the same in both renders, so
    // every differing pixel is one the tint wrote.
    let tinted = draw(&mut MapScreen::new(), &mut game, &assets);
    let owners: Vec<u8> =
        game.kingdom.county_ids().map(|id| game.kingdom.counties[id].owner).collect();
    for id in game.kingdom.county_ids() {
        game.kingdom.counties[id].owner = 0;
    }
    let flat = draw(&mut MapScreen::new(), &mut game, &assets);
    for (id, owner) in game.kingdom.county_ids().zip(owners) {
        game.kingdom.counties[id].owner = owner;
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut moved = 0usize;
    let mut selected_pixels = 0usize;
    for y in 0..chrome::MINIMAP_DIM {
        for x in 0..chrome::MINIMAP_DIM {
            let (px, py) = ((chrome::MINIMAP_X + x) as usize, (chrome::MINIMAP_Y + y) as usize);
            let (a, b) = (tinted.at(px, py), flat.at(px, py));
            if a == chrome::MINIMAP_SELECTED {
                selected_pixels += 1;
            }
            if a == b {
                continue;
            }
            moved += 1;
            assert_eq!(
                row_of_index[b as usize],
                Some(0),
                "an unowned county must paint the raster's own shading, ramp row 0, \
                 and this pixel painted {b:#04x}"
            );
            let row = row_of_index[a as usize].unwrap_or_else(|| {
                panic!("owned land painted {a:#04x}, which is in no ramp row at all")
            });
            seen.insert(row);
        }
    }

    // What the save says the answer should be, worked out from the counties
    // From the painting.
    let mut wanted = std::collections::BTreeSet::new();
    for id in game.kingdom.county_ids() {
        let owner = game.kingdom.counties[id].owner as usize;
        if owner != 0 {
            wanted.insert(chrome::realm_colour(game.realm_colour[owner]) as usize);
        }
    }
    assert!(
        wanted.len() >= 2,
        "this fixture has {} owning colour(s), so it could not tell a tinted minimap \
         from an untinted one even if the tint were gone",
        wanted.len()
    );

    assert_eq!(
        seen, wanted,
        "the minimap's ramp rows must be exactly the ones the counties' owners ask for; \
         **one single row** is the shape of the failure to watch for — `realm_colour` \
         clamps a zero colour up to 1, so a tint that has lost its input does not go \
         blank, it goes uniformly red"
    );
    assert!(
        moved > 500,
        "only {moved} pixels changed when five counties changed hands, which is too few \
         to be five counties"
    );
    assert!(
        selected_pixels > 0,
        "the selected county's brightest shade is replaced by MINIMAP_SELECTED, and none was drawn"
    );
}

// ------------------------------------------------ seasons, fields, animation

/// **The map's artwork changes when the season does — driven by a real turn.**
///
/// `Gfx_LoadCountyMode` (`0x004984DC`) repoints all five near-zoom tile banks
/// at `(g_season - 1) * 8` in `g_resourceTable`, so the whole picture is
/// redrawn from different files. We hard-coded the `a` set and the map looked
/// the same in January and in August.
///
/// **This ends a turn.** A test that sets
/// `kingdom.season = 4` and then reads a lookup table is checking its own
/// fixture (`docs/agents.md`); the season has to be moved by the thing that
/// moves it in play. `l2_game::turn::end_turn` runs the whole phase machine.
#[test]
fn a_real_turn_turns_the_season_and_the_map_is_repainted_from_other_files() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before_season = game.kingdom.season;
    let before = draw(&mut screen, &mut game, &assets);

    l2_game::turn::end_turn(&mut game).expect("the turn completes without asking");
    let after_season = game.kingdom.season;
    assert_ne!(after_season, before_season, "one turn is one season");

    let after = draw(&mut screen, &mut game, &assets);
    let moved = before.diff_count(&after);

    // The viewport is 480 x 450 under the menu bar — about 216,000 pixels — and
    // a whole-bank swap repaints essentially all of it. The floor is what
    // matters: hard-coding one season made this zero.
    assert!(
        moved > 50_000,
        "the season turned from {before_season} to {after_season} and only {moved} pixels moved"
    );

    // …and it is the *artwork* that changed, not our own markers: the two
    // frames must use meaningfully different palettes. Winter is the bright one
    // and autumn has no green in it (`campaign::SEASON_SUFFIX`).
    let histogram = |c: &Canvas| {
        let mut h = [0u32; 256];
        for &p in &c.pixels {
            h[p as usize] += 1;
        }
        h
    };
    let (a, b) = (histogram(&before), histogram(&after));
    let differing = (0..256).filter(|&i| a[i].abs_diff(b[i]) > 200).count();
    assert!(differing > 8, "only {differing} palette entries changed their share of the frame");
}

/// **A town drawn through `Overrides` in one season is still a town in the
/// next** — which is the thing the season swap could have broken.
/// `install.rs::the_four_seasons_of_a_bank_are_the_same_frame_table` exists.
///
/// The overrides plane stores a **frame index**, not a picture. If frame 47 of
/// `Town1c.pl8` were a different cell of the sheet than frame 47 of
/// `Town1a.pl8`, every county town on the map would turn back into a quarry in
/// autumn — a defect a player would report as *"my buildings disappear"*. The
/// frame tables agree, so it does not happen, and this asserts the consequence
/// At the pixel.
#[test]
fn a_towns_overridden_graphic_survives_every_season() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let tile = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, 8).first().expect("county 8 has a town")
    };
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let overrides = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        MapScreen::tile_graphics(&ctx)
    };

    let paint = |season: u8, o: &campaign::Overrides| {
        let mut canvas = Canvas::screen();
        let mut tags = l2_view::Tags::screen();
        campaign::draw(
            &mut canvas,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            o,
            season,
            None,
        );
        canvas
    };
    let bare = campaign::Overrides::new();
    let mut seasons_that_differ = 0;
    for season in 1..=campaign::SEASONS as u8 {
        let with = paint(season, &overrides);
        let without = paint(season, &bare);
        let moved = with.diff_count(&without);
        assert!(
            moved > 500,
            "season {season}: the override changed only {moved} pixels, so the town is not \
             being restamped"
        );
        seasons_that_differ += 1;
    }
    assert_eq!(seasons_that_differ, 4, "all four seasons keep their towns");
}

/// **The minimap closes the management surface from any screen**, which is
/// `Screen_FrameInput`'s epilogue and not any screen's own arm.
///
/// `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`.
#[test]
fn a_press_on_the_minimap_drops_whatever_is_open_over_the_map() {
    let (mut game, assets) = world!();
    game.select(8);
    let hit = chrome::minimap_hit_area();
    let (mx, my) = (hit.x0 + 40, hit.y0 + 40);

    // The three graduated map overlays are here because graduating them out of
    // the shell table LOST this arm: the wrapper reproduced it once for all
    // seven shells and each screen now has to carry it. That regression was
    // invisible until this test and the graduation met in one merge.
    for over in [
        ScreenId::County(8, Panel::Tax),
        ScreenId::Job(8, 0),
        ScreenId::Court,
        ScreenId::Ratings,
        ScreenId::Supplies(8),
    ] {
        let mut m = over_the_map(over);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: mx, y: my });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "{over:?} gave way to the minimap");
        assert_eq!(m.depth(), 1, "{over:?}: the map is revealed, not rebuilt");
    }

    // The ablation: a press just OUTSIDE the raster leaves everything alone. An
    // unconditional `Pass` would close on both.
    let mut m = over_the_map(ScreenId::Job(8, 0));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: hit.x1 + 4, y: my });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, 0)), "outside the raster nothing happens");
}

// --- the fog of war ----------------------------------------------------------
//
// `l2_kingdom::explore` has every reader and writer of the original's seen bit.
// These are the painters' half, and each assertion is about a *tile*: what is
// drawn on one the person has not seen, and what is drawn once he has.
//
// **Ablations, run on this branch** — each line deleted, and the test named:
//
// | deleted | red |
// |---|---|
// | the fog arm of `campaign::draw` (`Map_DrawTile`) | `a_dark_tile_…` |
// | `if fog.is_some() { 0 }` on the surround | `with_the_fog_on_the_sea_…` |
// | the `hides_tile` test in `draw_units` (`Map_DrawArmies`) | `a_county_in_the_dark_…` |
// | `.filter(lit)` on the town banner (`Sprite_TopIt` arm 1) | `a_county_in_the_dark_…` |
// | `.filter(lit)` on the mercenary marker (arm 2) | `a_county_in_the_dark_…` |
// | the `hides_tile` test in `draw_herds` (arm 4) | `a_county_in_the_dark_…` |
// | the `hides_tile` test on our owner marker | `a_county_in_the_dark_…` |
// | `exploration &&` in `l2_kingdom::explore::hides` | `a_county_in_the_dark_…` (the control), and `explore::tests::the_painters_test_…` |
//
// **Two of those rows were green the first time they were run, and the test
// was wrong both times, not the gate.** The herd gate: the field chosen already
// had a herd, so the herd was in both renders. The option test: the control
// also repainted a field's terrain, which moves the fog-off render whatever the
// option test says. The comments at `pasture` and `give_away` say what changed.
//
// **Not covered, and said so:** the castle's garrison banner (England at turn
// one has no garrison), the industry wheel's pause in the dark (a clock drives
// it, and no test here ticks the map), and our field markers (drawn only for
// the person's own county, which is never dark).


