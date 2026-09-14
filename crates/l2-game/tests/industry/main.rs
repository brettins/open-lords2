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

mod wheel_rates;
pub use wheel_rates::*;
mod visual_effects;
pub use visual_effects::*;

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
/// Typed here
/// because a probe computed from the constant under test cannot fail when the
/// constant is ablated — `docs/agents.md`, *how to ablate wrongly*, one.
const PULSE_MS: [u32; 4] = [640, 320, 160, 80];

/// The band edges, from the same three comparisons: `0x0A`, `0x19`, `0x32`.
/// One output either side of each.
const BAND_EDGES: [(i32, usize); 8] =
    [(0, 0), (9, 0), (10, 1), (24, 1), (25, 2), (49, 2), (50, 3), (173, 3)];

/// How many of **our** ticks each rung is, at `l2_game::TICK_MS`. Asserted
/// against `TICK_MS` below
/// the frame rate is a red test and not a silently rescaled expectation.
///
/// A rung is *gates*, not milliseconds: `Tick_Pulses` (`0x004BBC80`) fires on
/// the first tick 20 ms after the last and then sets `stamp = now`, so at a
/// 16 ms tick a gate is two ticks and 640 ms is 64 ticks, not 40. C179.
const EVERY: [u32; 4] = [64, 32, 16, 8];

/// The window every count below is taken over. A multiple of all four rungs, so
/// no band is measured across a partial period.
const TICKS: u32 = 384;

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
            // key would be an absence
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
/// `content = base + (enabled != 0)`
/// `l2_kingdom::map::industry_state`.
/// The first working site whose **record says its season was idle** —
/// `total == totalSnapshot`, so `output` is 0 — read from what the importer
/// carried
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

