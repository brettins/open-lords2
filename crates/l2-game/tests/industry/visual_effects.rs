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

// ---------------------------------------------------------------- the pixels

/// A window over which the map's **other two** clocks return to the phase they
/// counter at `0x60`.
/// this far apart differ only by what the industry wheels did.
///
/// **768 and not 384.
/// unproductive site is on the 640 ms rung — one step every 40 ticks — and
/// wood's working run is nine frames long, so 384 ticks is *exactly nine steps
/// of a nine-frame cycle* and the wheel comes back to the frame it started on.
/// The first draft of this test asserted that the screen changed, watched the
/// wheel turn all the way round, and reported that the wheel does not reach the
/// screen. The frame assertion below is what turns that into a failure that
/// names its own cause.
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

/// Every pixel at which two screen canvases differ.
fn differences(a: &l2_view::Canvas, b: &l2_view::Canvas) -> Vec<(i32, i32)> {
    a.pixels
        .iter()
        .zip(b.pixels.iter())
        .enumerate()
        .filter(|(_, (p, q))| p != q)
        .map(|(i, _)| ((i % 640) as i32, (i / 640) as i32))
        .collect()
}

/// **The wheel reaches the screen, and only in its own tile.**
///
/// `docs/agents.md`: *a canvas diff passes on a garbage sprite or the wrong
/// frame of the right sheet*, so this is not *"some pixels changed"*. It is two
/// claims that fail in opposite directions:
///
/// * with one site working and every other switched off, `CYCLE` ticks change
///   the picture, and **every** pixel that changed lies inside that site's own
///   tile rectangle. A wheel drawn at the wrong tile, or an override that
///   repainted the map, fails the second half. Measured: 196 pixels move, in a
///   45 × 36 box inside a 58 × 60 rectangle.
/// * with that last site switched off too, `CYCLE` ticks leave the frame
///   **byte-identical**. That is the idempotence form `docs/agents.md` prefers
///   to a threshold: no number to tune, and nothing to re-tune when the artwork
///   changes.
///
/// **Ablation.** Delete `MapScreen::add_industry_graphics`'s `out.set(…)` and
/// the first claim goes red — the sites then draw the frame `L2_maps.dat`
/// stores and the wheel turns invisibly
/// branch was written to fix. Dropping `industry_key` from the base plane's
/// repaint key goes red the same way, on the cache
#[test]
fn a_turning_wheel_changes_the_screen_and_a_stopped_one_changes_nothing() {
    let (mut game, assets) = world!();
    let sites = working_sites(&game);
    // An idle one, because `CYCLE` is chosen against the slow rung's period — see
    // [`idle_site`] for why the first site no longer is.
    let (tile, county, commodity) = idle_site(&game).expect("an idle working site on the fixture");

    // Every other site switched off, so anything that moves in the frame is this
    // one. `Industry_ToggleFromMap`'s own road, not a flag.
    for &(t, id, c) in &sites {
        if t != tile {
            game.kingdom.toggle_industry(id, MapToggle::Industry(Commodity::ALL[c]));
        }
    }

    let (x, y) = l2_kingdom::map::coords(tile);
    let mut screen = MapScreen::new();
    screen.centre_on_tile(x as usize, y as usize);
    // One tick to build the site list and settle the frames, then the pair.
    tick(&mut screen, &mut game, &assets, 1);

    let frame_of = |s: &MapScreen| {
        s.industry_sites_for_test().into_iter().find(|&(t, ..)| t == tile).expect("the site").3
    };
    let before = draw_map(&mut screen, &mut game, &assets);
    let frame_before = frame_of(&screen);
    tick(&mut screen, &mut game, &assets, CYCLE);
    let frame_after = frame_of(&screen);
    let after = draw_map(&mut screen, &mut game, &assets);

    // The precondition, stated so that a window which happens to be a whole
    // number of the wheel's own cycle fails *here*, where it names the cause,
    //
    // screen".
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

    // The site's own diamond, plus one tile's height of overhang above it —
    // `Town1a.pl8`'s mine is 58 x 47 against a 58 x 30 tile, so the headframe is
    // drawn above the diamond and `campaign::draw` subtracts the difference.
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

    // And the other direction: switch the last one off, and nothing moves.
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

/// **The wheel's phase is not in the kingdom**, and this asks it the only way
///
///
/// `armoury.rs`'s
/// `a_hundred_ticks_of_the_armoury_leave_the_kingdom_byte_identical` encodes the
/// kingdom, ticks, and requires the bytes back. **That form does not work on
/// this screen**, and finding out why is most of the value here:
/// `MapScreen::update` also runs `turn::tick_units_only`.
/// the campaign map legitimately move merchants and the encoding legitimately
/// changes. A byte-identity assertion here would have been red for a reason
/// with nothing to do with industry.
///
/// So the claim is stated as a **difference between two runs that differ only in
/// the wheels' phase**. Two copies of the same position, the same number of
/// ticks, and two screens whose industry counters are seven ticks apart —
/// `MapScreen::industry_tick` and every site's frame are the display state
/// `docs/netcode.md` D-12 says nothing below `l2-game` may read. If the phase
/// reached the kingdom, the two encodings would differ.
///
/// The original has no such constraint: its animation frame lives in the tile
/// record
/// one place this branch departs from it, in storage and not in behaviour, and
/// this is the assertion that keeps the departure honest.
///
/// # Ablating it, and the two things that came out of trying
///
/// **`step_industry` cannot write to the kingdom at all**: it takes `&Ctx`, and
/// the attempt is a borrow-check error
/// stronger guarantee `docs/agents.md` asks for — *prefer a shape that cannot be
/// wrong to a check that notices when it is* — and it means the leak has to be
/// staged in `update`, where a `&mut Ctx` exists.
///
/// Two versions were tried there, and **the first was green**: writing each
/// site's frame into its own county's `purse` leaks the phase into an encoded
/// field and this test did not see it, because a county holds up to four sites
/// and only the last writer survives — the two runs' last-per-county frames
/// happened to coincide. Folding *every* frame into one encoded field is red.
///
/// So the honest scope: this asserts that the phase does not reach the encoding
/// **in a way that survives to the end of the run**, which is what a lockstep
/// peer would see. A leak that collides is a leak it cannot detect, and there is
/// no cheap check that can.
#[test]
fn two_maps_whose_wheels_are_out_of_phase_reach_the_same_kingdom() {
    const N: u32 = 400;

    // A screen warmed on a throwaway copy of the position, so that its industry
    // counter and its site frames run ahead of a fresh one. **47 and not 7**: the
    // slowest rung is 40 ticks.
    // both screens then take the same number of steps from the same frame. The
    // assertion below caught exactly that
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

/// **A county that breaks away stops showing a working mine** — and what
/// repaints it is the season, not the secession.
///
/// `County_MakeIndependent` (`0x004AC3C6`) clears all four `enabled` flags and
/// touches no tile; every call site of `Industry_UpdateSiteTile` (`0x0044EDC2`)
/// in the binary is inside `Industry_Produce` (`0x0044EA92`) or
/// `Industry_ProduceAll` (`0x0044E852`). So the site keeps its *working*
/// terrain until the next season's industry pass writes `base + (enabled != 0)`
/// over it — and `Industry_ProduceAll`'s loop is
/// `for (c = 1; c <= g_countyCount; c++)`, with **no owner test**. The owner
/// test is one level down, guarding production alone.
///
/// Ours had that test on the repaint.
/// rest of the game. A player would have read it as a mine still being worked
/// by a realm that no longer owns the county.
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

    // `County_MakeIndependent`: the switches go off and the map is not touched.
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

