//! **The industry sites on the campaign map: the wheel, its rate, and the
//! pixels.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!   cargo test -p l2-game --test industry
//! ```
//!
//! `Sprite_TopIt`'s (`0x004071A0`) arm 5 rewrites an industry tile's **own**
//! terrain frame and draws no overlay at all, so *"this mine is running"* is
//! entirely that its picture moves — and **how fast it moves is a mechanic**,
//! banded from the season's output onto four rungs of `Tick_Pulses`
//! (`0x004BBC80`). A player is told how busy a mine is by nothing else.
//!
//! # Which of the three appearances these reach
//!
//! `docs/draws-map.md` §6 names three: *idle*, *working* (the animated one) and
//! *wrecked*. The branch that added them reported that every fixture is turn one
//! and only *idle* could be exercised.
//!
//! **That is wrong, and the fixture is better than it was given credit for.**
//! England turn one has the five owned counties' **forests switched on** —
//! terrain 11, which is `INDUSTRY_IDLE[wood] + 1`, the *working* value — so
//! working is reachable without staging anything, and switching one off with
//! `Kingdom::toggle_industry` reaches idle by the map click's own road. What is
//! still out of reach here is **wrecked**: it needs `Unit_TrampleTile` and three
//! seasons of `disabled_seasons`, and no test in this file claims it.

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::{Commodity, Tables};
use l2_mods::Platform;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there are no assets to draw with");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

/// **`Tick_Pulses`' four rungs, in milliseconds**, and the bands
/// `Sprite_TopIt` picks between them with.
///
/// ```c
/// n = county.industry[k].total - county.industry[k].totalSnapshot;
/// if      (n < 0x0A) step = DAT_0058FD08;   /* 640 ms — the eighth counter */
/// else if (n < 0x19) step = DAT_0057D3C8;   /* 320 ms — the fourth */
/// else if (n < 0x32) step = g_pulse160;
/// else               step = g_pulse80;
/// ```
///
/// Typed here rather than read from [`l2_view::campaign::industry_period_ms`],
/// because a probe computed from the constant under test cannot fail when the
/// constant is ablated — `docs/agents.md`, *how to ablate wrongly*, one.
const PULSE_MS: [u32; 4] = [640, 320, 160, 80];

/// The band edges, from the same three comparisons: `0x0A`, `0x19`, `0x32`.
/// One output either side of each, so a band that is off by one goes red.
const BAND_EDGES: [(i32, usize); 8] =
    [(0, 0), (9, 0), (10, 1), (24, 1), (25, 2), (49, 2), (50, 3), (173, 3)];

/// How many of **our** ticks each rung is, at `l2_game::TICK_MS`. Asserted
/// against `TICK_MS` below rather than divided out of it, so that a change to
/// the frame rate is a red test and not a silently rescaled expectation.
const EVERY: [u32; 4] = [40, 20, 10, 5];

/// The window every count below is taken over. A multiple of all four rungs, so
/// no band is measured across a partial period.
const TICKS: u32 = 240;

/// Step one screen `n` times and report how many times each site's frame
/// changed, keyed by tile.
fn turns_of_each_wheel(
    screen: &mut MapScreen,
    game: &mut l2_game::Game,
    assets: &Assets,
    n: u32,
) -> std::collections::BTreeMap<usize, u32> {
    let mut last: std::collections::BTreeMap<usize, u8> =
        screen.industry_sites_for_test().into_iter().map(|(t, _, _, f)| (t, f)).collect();
    let mut turns: std::collections::BTreeMap<usize, u32> =
        last.keys().map(|&t| (t, 0)).collect();
    for _ in 0..n {
        {
            let mut ctx = Ctx { game, assets };
            screen.update(&mut ctx);
        }
        for (tile, _, _, frame) in screen.industry_sites_for_test() {
            // Every site gets a row whether or not it ever moves — a wheel that
            // stood still is the assertion in
            // [`a_site_the_player_switched_off_stops_turning`], and a missing
            // key would be an absence rather than a zero.
            turns.entry(tile).or_insert(0);
            let seen = last.entry(tile).or_insert(frame);
            if *seen != frame {
                *turns.entry(tile).or_insert(0) += 1;
                *seen = frame;
            }
        }
    }
    turns
}

/// Every working site on the map, as the *terrain* says — the same test
/// `step_industry` makes, but written out from `Industry_UpdateSiteTile`'s
/// `content = base + (enabled != 0)` rather than borrowed from
/// `l2_kingdom::map::industry_state`.
/// The first working site whose **record says its season was idle** —
/// `total == totalSnapshot`, so `output` is 0 — read from what the importer
/// carried rather than assumed. The rate tests need a wheel on the slow rung,
/// and until C161 they took the first site and got one only
/// because the load had thrown every running total away.
fn idle_site(game: &l2_game::Game) -> Option<(usize, usize, usize)> {
    working_sites(game)
        .into_iter()
        .find(|&(_, id, c)| game.kingdom.counties[id].industry[c].output == 0)
}

fn working_sites(game: &l2_game::Game) -> Vec<(usize, usize, usize)> {
    // `Industry_UpdateSiteTile`'s four bases, in commodity order: wood 10,
    // iron 1, weapons 7, stone 4.
    const BASE: [u8; 4] = [10, 1, 7, 4];
    let map = &game.kingdom.campaign.map;
    let mut out = Vec::new();
    for id in 1..=game.kingdom.county_count {
        for c in Commodity::ALL {
            let Some(tile) = l2_kingdom::map::industry_site(map, id as u8, c) else { continue };
            if map.terrain[tile] == BASE[c.index()] + 1 {
                out.push((tile, id, c.index()));
            }
        }
    }
    out
}

/// **The rate is the mechanic: the busier the site, the faster the wheel.**
///
/// Two halves, and the first is the road.
///
/// * **From the save.** England turn one's own records put its forests on
///   different rungs: county 1's running total is 166 against a snapshot of 0,
///   which is past `0x32`, and at least one other working forest records an idle
///   season. So a loaded game draws one wheel turning eight times as fast as
///   another **from its first frame**. This used to say *"the England position's
///   five forests have produced nothing, so every wheel is on the 640 ms
///   rung"* — true of `Industry::new()`, which is what the importer gave every
///   county until `docs/decisions.md` C161, and false of the file.
/// * **From the simulation.** End one season on the idle site with the game's
///   own pass, and its output reaches the top band: the same wheel is now on
///   the 80 ms rung and turns **eight times as often**. Nothing here writes
///   `Industry::output`.
/// * **From the table.** Then one output either side of each of the three band
///   edges — `0x0A`, `0x19`, `0x32` — staged on the record, because no fixture
///   reaches the middle two bands and a boundary that is off by one is exactly
///   what a test of a banded rate is for.
///
/// The counts are `TICKS / EVERY[band]` with both typed from the decompilation:
/// **no expression in this test mentions
/// [`l2_view::campaign::industry_period_ms`]**, which is the constant being
/// ablated.
///
/// **Ablation.** Flatten `industry_period_ms`'s four arms to a single `640` and
/// the second half goes red on six of its eight outputs and the first half on
/// its ratio. Flattening it to `80` fails them the other way. Widening one band
/// edge by one — `n < 0x0A` to `n <= 0x0A` — fails exactly the `(10, 1)` row,
/// which is the row that edge exists for.
#[test]
fn a_busy_site_turns_its_wheel_eight_times_as_often_as_an_idle_one() {
    let (mut game, assets) = world!();

    // The premise of `EVERY`. A frame rate that moved would otherwise rescale
    // every expectation below without saying so.
    assert_eq!(l2_game::TICK_MS, 16, "`EVERY` is `PULSE_MS` in 16 ms ticks");
    for (rung, ms) in PULSE_MS.iter().enumerate() {
        assert_eq!(ms / l2_game::TICK_MS, EVERY[rung], "rung {rung}");
        assert_eq!(TICKS % EVERY[rung], 0, "rung {rung} does not divide the window");
    }

    // The five owned counties' forests are switched on in the shipped position
    // — this is the *working* appearance, on the fixture, with nothing staged.
    let sites = working_sites(&game);
    assert!(
        !sites.is_empty(),
        "England turn one has no working industry site, so this test cannot see a wheel"
    );
    let (tile, county, commodity) = idle_site(&game)
        .expect("no working site on England turn one records an idle season, so the slow rung is unseen");
    let &(busy_tile, busy_county, busy_commodity) = sites
        .iter()
        .find(|&&(_, id, c)| game.kingdom.counties[id].industry[c].output >= 0x32)
        .expect("no working site on England turn one records a busy season");

    let mut screen = MapScreen::new();
    let slow = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert_eq!(
        slow[&tile],
        TICKS / EVERY[0],
        "an on-but-unproductive site is on the 640 ms rung: {} turns in {TICKS} ticks",
        TICKS / EVERY[0]
    );
    // **The first frame of a loaded game, from the file's own record.** Before
    // the importer carried `+0x2A0`/`+0x2A4`, this wheel sat on the slow rung
    // with every other until a season ended.
    assert_eq!(
        slow[&busy_tile],
        TICKS / EVERY[3],
        "county {busy_county}'s commodity {busy_commodity} records {} this season, so the loaded \
         game should turn its wheel on the 80 ms rung from the first frame",
        game.kingdom.counties[busy_county].industry[busy_commodity].output
    );

    // The season, by the game's own road — `Industry_ProduceAll` is what writes
    // `output`, and `Industry_UpdateSiteTile` runs behind it.
    game.kingdom.advance_season();
    let produced = game.kingdom.counties[county].industry[commodity].output;
    assert!(
        produced >= 0x32,
        "county {county} cut {produced} — the season did not reach the top band, so the \
         ratio below is not the one being claimed"
    );

    let mut screen = MapScreen::new();
    let fast = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert_eq!(
        fast[&tile],
        TICKS / EVERY[3],
        "a site producing {produced} is on the 80 ms rung: {} turns in {TICKS} ticks",
        TICKS / EVERY[3]
    );
    assert_eq!(
        fast[&tile],
        slow[&tile] * 8,
        "eight rungs apart is eight times the turns — {} against {}",
        fast[&tile],
        slow[&tile]
    );

    // And the table, one output either side of every edge.
    for (output, band) in BAND_EDGES {
        let (mut game, assets) = world!();
        for &(_, id, c) in &working_sites(&game) {
            game.kingdom.counties[id].industry[c].output = output;
        }
        let mut screen = MapScreen::new();
        let turns = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
        assert_eq!(
            turns[&tile],
            TICKS / EVERY[band],
            "output {output} belongs on the {} ms rung, so the wheel should turn {} \
             times in {TICKS} ticks and it turned {}",
            PULSE_MS[band],
            TICKS / EVERY[band],
            turns[&tile]
        );
    }
}

/// **A site that is switched off does not turn at all**, and switching it off is
/// a map click rather than a flag.
///
/// `Industry_UpdateSiteTile` writes `content = base + (enabled != 0)`, so "off"
/// is a terrain value `Sprite_TopIt`'s arm 5b never animates — the whole of the
/// on/off appearance. This is the half that makes the test above a claim about
/// *industry* and not about a counter: a wheel that turned on every site
/// whatever its switch would pass the rate test and fail this one.
///
/// **Ablation.** Delete the `if !working { … continue; }` guard in
/// `MapScreen::step_industry` and this goes red while the rate test stays green.
#[test]
fn a_site_the_player_switched_off_stops_turning() {
    let (mut game, assets) = world!();
    let sites = working_sites(&game);
    let (tile, county, commodity) = *sites.first().expect("a working site on the fixture");

    let mut screen = MapScreen::new();
    let before = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert!(before[&tile] > 0, "the wheel was not turning before the switch was touched");

    // `Industry_ToggleFromMap` (`0x0043D309`), which is what a click on the site
    // reaches. It writes the enable byte *and* calls `Industry_UpdateSiteTile`.
    let on = game.kingdom.toggle_industry(county, MapToggle::Industry(Commodity::ALL[commodity]));
    assert!(!on, "the toggle should have switched county {county}'s site off");

    let mut screen = MapScreen::new();
    let after = turns_of_each_wheel(&mut screen, &mut game, &assets, TICKS);
    assert_eq!(
        after[&tile], 0,
        "a switched-off site turned {} times in {TICKS} ticks",
        after[&tile]
    );
}

// ---------------------------------------------------------------- the pixels

/// A window over which the map's **other two** clocks return to the phase they
/// started on: `Map_DrawFrame` wraps the flag counter at `0x80` and the herd
/// counter at `0x60`, so a multiple of 384 is a whole number of both. Two frames
/// this far apart differ only by what the industry wheels did.
///
/// **768 and not 384, and the reason is the trap this whole file is about.** An
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
/// stores and the wheel turns invisibly, which is exactly the defect this
/// branch was written to fix. Dropping `industry_key` from the base plane's
/// repaint key goes red the same way, on the cache rather than on the override.
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
    // rather than below, where it would read as "the wheel does not reach the
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
/// that is actually available.
///
/// `armoury.rs`'s
/// `a_hundred_ticks_of_the_armoury_leave_the_kingdom_byte_identical` encodes the
/// kingdom, ticks, and requires the bytes back. **That form does not work on
/// this screen**, and finding out why is most of the value here:
/// `MapScreen::update` also runs `turn::tick_units_only`, so a thousand ticks of
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
/// record, which is both the simulation's map and the renderer's. That is the
/// one place this branch departs from it, in storage and not in behaviour, and
/// this is the assertion that keeps the departure honest.
///
/// # Ablating it, and the two things that came out of trying
///
/// **`step_industry` cannot write to the kingdom at all**: it takes `&Ctx`, and
/// the attempt is a borrow-check error rather than a red test. That is the
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
    // slowest rung is 40 ticks, so a seven-tick lead moves no wheel at all and
    // both screens then take the same number of steps from the same frame. The
    // assertion below caught exactly that, which is why it is written first.
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
/// Ours had that test on the repaint, so a seceded county's wheel turned for the
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
