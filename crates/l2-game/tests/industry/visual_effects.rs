#![allow(unused_imports)]
use super::*;
use super::wheel_rates::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::{Commodity, Tables};
use l2_mods::Platform;


const CYCLE: u32 = 768;

fn draw_map(screen: &mut MapScreen, game: &mut l2_game::Game, assets: &Assets) -> l2_view::Canvas {
    let mut canvas = l2_view::Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

fn tick(screen: &mut MapScreen, game: &mut l2_game::Game, assets: &Assets, n: u32) {
    for _ in 0..n {
        let mut ctx = Ctx { game, assets };
        screen.update(&mut ctx);
    }
}

fn differences(a: &l2_view::Canvas, b: &l2_view::Canvas) -> Vec<(i32, i32)> {
    a.pixels
        .iter()
        .zip(b.pixels.iter())
        .enumerate()
        .filter(|(_, (p, q))| p != q)
        .map(|(i, _)| ((i % 640) as i32, (i / 640) as i32))
        .collect()
}

/// **Ablation.** Delete `MapScreen::add_industry_graphics`'s `out.set(…)` and
/// the first claim goes red — the sites then draw the frame `L2_maps.dat`
/// stores and the wheel turns invisibly
/// branch was written to fix. Dropping `industry_key` from the base plane's
/// repaint key goes red the same way, on the cache
#[test]
fn a_turning_wheel_changes_the_screen_and_a_stopped_one_changes_nothing() {
    let (mut game, assets) = world!();
    let sites = working_sites(&game);
    let (tile, county, commodity) = idle_site(&game).expect("an idle working site on the fixture");

    for &(t, id, c) in &sites {
        if t != tile {
            game.kingdom.toggle_industry(id, MapToggle::Industry(Commodity::ALL[c]));
        }
    }

    let (x, y) = l2_kingdom::map::coords(tile);
    let mut screen = MapScreen::new();
    screen.centre_on_tile(x as usize, y as usize);
    tick(&mut screen, &mut game, &assets, 1);

    let frame_of = |s: &MapScreen| {
        s.industry_sites_for_test().into_iter().find(|&(t, ..)| t == tile).expect("the site").3
    };
    let before = draw_map(&mut screen, &mut game, &assets);
    let frame_before = frame_of(&screen);
    tick(&mut screen, &mut game, &assets, CYCLE);
    let frame_after = frame_of(&screen);
    let after = draw_map(&mut screen, &mut game, &assets);

    assert_ne!(
        frame_before, frame_after,
        "{CYCLE} ticks is a whole number of this wheel's cycle, so it is back on frame \
         {frame_before} and the pixel claim below would be asserting nothing"
    );

    let moved = differences(&before, &after);
    assert!(
        !moved.is_empty(),
        "county {county}'s site went from frame {frame_before} to {frame_after} and not \
         one pixel of the screen changed"
    );

    let (row, col) = l2_view::campaign::tile_to_cell(x as usize, y as usize);
    let (tx, ty) = l2_view::campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let z = screen.zoom();
    let (x0, x1) = (tx, tx + z.tile_w);
    let (y0, y1) = (ty - z.tile_h, ty + z.tile_h);
    for &(px, py) in &moved {
        assert!(
            (x0..x1).contains(&px) && (y0..y1).contains(&py),
            "a pixel at ({px}, {py}) changed, and the only thing moving is the site at \
             tile {tile}, whose rectangle is x {x0}..{x1}, y {y0}..{y1} — {} pixels moved \
             in all",
            moved.len()
        );
    }

    let on = game.kingdom.toggle_industry(county, MapToggle::Industry(Commodity::ALL[commodity]));
    assert!(!on, "county {county}'s site should now be off");
    tick(&mut screen, &mut game, &assets, 1);

    let still = draw_map(&mut screen, &mut game, &assets);
    tick(&mut screen, &mut game, &assets, CYCLE);
    let still_again = draw_map(&mut screen, &mut game, &assets);
    let drift = differences(&still, &still_again);
    assert!(
        drift.is_empty(),
        "with every site switched off, {CYCLE} ticks moved {} pixels — the first at {:?}",
        drift.len(),
        drift.first()
    );
}

/// Two versions were tried there, and **the first was green**: writing each
/// site's frame into its own county's `purse` leaks the phase into an encoded
/// field and this test did not see it, because a county holds up to four sites
/// and only the last writer survives — the two runs' last-per-county frames
/// happened to coincide. Folding *every* frame into one encoded field is red.
#[test]
fn two_maps_whose_wheels_are_out_of_phase_reach_the_same_kingdom() {
    const N: u32 = 400;

    let (mut warm, assets) = world!();
    let mut ahead = MapScreen::new();
    tick(&mut ahead, &mut warm, &assets, 47);

    let (mut a, _assets_a) = world!();
    let mut fresh = MapScreen::new();
    tick(&mut fresh, &mut a, &assets, N);

    let (mut b, _assets_b) = world!();
    tick(&mut ahead, &mut b, &assets, N);

    let frames = |s: &MapScreen| -> Vec<u8> {
        s.industry_sites_for_test().into_iter().map(|(_, _, _, f)| f).collect()
    };
    assert_ne!(
        frames(&fresh),
        frames(&ahead),
        "the two screens' wheels ended in the same phase, so this test compares two \
         identical worlds and asserts nothing"
    );
    assert_eq!(
        l2_kingdom::save::encode(&a.kingdom),
        l2_kingdom::save::encode(&b.kingdom),
        "two runs of {N} ticks over the same position reached different kingdoms, and the \
         only thing that differed between them was where the industry wheels were"
    );
}

/// `County_MakeIndependent` (`0x004AC3C6`) clears all four `enabled` flags and
/// touches no tile; every call site of `Industry_UpdateSiteTile` (`0x0044EDC2`)
/// in the binary is inside `Industry_Produce` (`0x0044EA92`) or
/// `Industry_ProduceAll` (`0x0044E852`). So the site keeps its *working*
/// terrain until the next season's industry pass writes `base + (enabled != 0)`
/// over it — and `Industry_ProduceAll`'s loop is
/// `for (c = 1; c <= g_countyCount; c++)`, with **no owner test**. The owner
/// test is one level down, guarding production alone.
///
/// **Ablation.** Put `if self.counties[id].owner == 0 { continue; }` back on the
/// site-tile loop in `Kingdom::industry` and the last assertion goes red.
#[test]
fn a_seceded_countys_mine_is_repainted_by_the_next_season() {
    let (mut game, _assets) = world!();
    let sites = working_sites(&game);
    let (tile, county, commodity) = *sites.first().expect("a working site on the fixture");
    let base = l2_kingdom::map::terrain::INDUSTRY_IDLE[commodity];

    assert_eq!(game.kingdom.campaign.map.terrain[tile], base + 1, "the site starts working");

    game.kingdom.make_county_independent(county);
    assert_eq!(game.kingdom.counties[county].owner, 0, "the county left its realm");
    assert!(!game.kingdom.counties[county].industry[commodity].enabled, "the switch went off");
    assert_eq!(
        game.kingdom.campaign.map.terrain[tile],
        base + 1,
        "secession repaints nothing, so the mine is still turning this season"
    );

    l2_game::turn::end_turn(&mut game).expect("the machine comes round");

    assert_eq!(
        game.kingdom.counties[county].owner, 0,
        "the county was retaken within the turn, so this proves nothing"
    );
    assert_eq!(
        game.kingdom.campaign.map.terrain[tile],
        base,
        "`Industry_ProduceAll` repaints an unowned county's site tiles too"
    );
}

