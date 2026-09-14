#![allow(unused_imports)]
use super::*;
use super::industry_and_building::*;
use super::*;
use super::events_and_letters::*;
use super::tile_panel::*;
use l2_game::game::Assets;
use l2_game::message::category;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::job::JobScreen;
use l2_game::shell::font::{Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::event::{EventKind, RealmPurse};
use l2_kingdom::field::FieldType;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::tables::{
    Commodity, Season, Tables, Weather, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING,
    JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
    JOB_WOOD_CUTTING,
};
use l2_mods::Platform;
use l2_view::Canvas;

/// **`Panel_JobGrain`'s two signed rows and its growing branch, on stored
/// numbers.** `safeturn.sav` faces Autumn, so the painter takes the `else` arm:
/// `Ui_DrawCount(+0x2FC, 2, 0x40, 0xD8)` + 77/3 + `Ui_DrawCount(2, 0x42, …)`,
/// then 77/0 + `crop[0]` + 77/4 on `0xE8`.
///
/// Non-zero on disk: the store, grain eaten, the overall change, and the season
/// count (Autumn next gives 2). **Zero here:** `+0x2FC` and `crop[0]`
/// (zero in every save; the rule-driven test below makes them move) and
/// `+0x278` (the no-event line is what is asserted).
///
/// Ablations, run: the `delta(…grain_eaten…)` line deleted → `"-135"` not at
/// `(0x130, 0x108)`; the season mapping's `3 => 2` → `3 => 3` → `"2"` not at
/// `(0xF5, 0xD8)`.
#[test]
fn the_grain_popup_draws_the_store_the_eating_and_the_overall_change() {
    let (mut game, assets) = fixture_world!("safeturn.sav");
    let k = &game.kingdom;
    assert_eq!(k.season_next, 3, "setup: safeturn.sav faces Autumn");
    assert!(!k.options.advanced_farming, "setup: advanced farming is off in this save");
    let id = county_where(k, "eating grain with a store", |c| c.grain > 0 && c.grain_eaten > 0);
    let c = k.counties[id].clone();
    assert!(c.grain_change_expected != 0, "setup: county {id}'s overall change is non-zero");
    assert_eq!(c.grain_event_change, 0, "setup: no event touched the store");

    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);

    // `Ui_DrawCount(grain, 2, 0x130, 0x88)`.
    count_at(&canvas, &assets, c.grain, 2, 0x130, 0x88);
    // `+0x278 == 0`: 77/0x18 at (0x40, 0xB0).
    word_at(&canvas, &assets, 77, 0x18, 0x40, 0xB0);
    // Advanced farming off: no fertility line, no weather line.
    assert!(!is_at(&canvas, body(&assets), &eng(&assets, 22, 3), INK, 0x80, 0x98));
    assert!(!is_at(&canvas, body(&assets), &eng(&assets, 77, 0x12), INK, 0x40, 0xC0));

    // Facing Autumn: `+0x2FC` harvested in 2 Seasons.
    let at = count_at(&canvas, &assets, c.grain_grown_expected, 2, 0x40, 0xD8);
    let at = word_at(&canvas, &assets, 77, 3, at, 0xD8);
    count_at(&canvas, &assets, 2, 0x42, at, 0xD8);
    // From `crop[0]` Sacks sown in spring.
    let at = word_at(&canvas, &assets, 77, 0, 0x40, 0xE8);
    let at = count_at(&canvas, &assets, c.crop[0], 2, at, 0xE8);
    word_at(&canvas, &assets, 77, 4, at, 0xE8);

    // The two signed rows.
    word_at(&canvas, &assets, 77, 0x1B, 0x40, 0x108);
    signed_at(&canvas, &assets, -c.grain_eaten, 0x108);
    word_at(&canvas, &assets, 77, 0x1C, 0x40, 0x118);
    signed_at(&canvas, &assets, c.grain_change_expected, 0x118);
}

/// **The grain year, as the rule walks it: sown, growing, harvested.**
///
/// No save on this machine has grain in the ground, so the fields are painted
/// with the player's brush and the seasons advanced,
/// `crates/l2-kingdom/tests/fields/main.rs` does. Each stage then has a non-zero
/// figure in its own box:
///
/// * facing Spring, `Ui_DrawCount(+0x230, 2, 0x40, 0xD8)` + 77/1 and
///   `Ui_DrawCount(+0x230 * g_grainYieldPerSack, 2, 0x40, 0xE8)` + 77/2;
/// * facing Summer, `+0x2FC` + 77/3 + **3** Seasons, and `crop[0]` after 77/0;
/// * facing Winter, `crop[2]` + 77/3 + **1** Season, singular.
///
/// And last, with advanced farming on and a *Sunny* band, `Grain_Grow`'s own
/// weather swing on `0xC0`, and the fertility phrase on `0x98`. The band is the
/// one hand-set input: `Weather_UpdateAll` rolls it, and this names one.
///
/// Ablations, run: `wrapping_mul(yield_per_sack)` → `wrapping_mul(1)` → `"240"`
/// not at `(0x44, 0xE8)`; `weather_line` deleted from `grain` → `"120"` not at
/// `(0x44, 0xC0)`.
#[test]
fn the_grain_popup_follows_the_crop_the_rule_sows_grows_and_harvests() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let k = &mut game.kingdom;
    assert_eq!(k.season_next, 1, "setup: the England position faces Spring");
    let id = county_where(k, "the player's", |c| c.owner == player);
    k.counties[id].grain = 10_000;
    assert!(paint_all_fallow_to_grain(k, id) > 0, "setup: county {id} had fields to paint");
    k.refresh_estimates(id);
    let sown = k.counties[id].grain_sown_expected;
    assert!(sown > 0, "setup: painted grain fields forecast a sowing, not {sown}");
    let yield_per_sack = k.tables.grain.yield_per_sack;

    // Facing Spring.
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, sown, 2, 0x40, 0xD8);
    word_at(&canvas, &assets, 77, 1, at, 0xD8);
    let at = count_at(&canvas, &assets, sown * yield_per_sack, 2, 0x40, 0xE8);
    word_at(&canvas, &assets, 77, 2, at, 0xE8);

    // Facing Summer: the crop is in the ground.
    game.kingdom.advance_season();
    game.kingdom.refresh_estimates(id);
    assert_eq!(game.kingdom.season_next, 2, "setup: one season on faces Summer");
    let c = game.kingdom.counties[id].clone();
    assert!(c.grain_grown_expected > 0 && c.crop[0] > 0, "setup: a crop is growing: {c:?}");
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, c.grain_grown_expected, 2, 0x40, 0xD8);
    let at = word_at(&canvas, &assets, 77, 3, at, 0xD8);
    count_at(&canvas, &assets, 3, 0x42, at, 0xD8);
    let at = word_at(&canvas, &assets, 77, 0, 0x40, 0xE8);
    let at = count_at(&canvas, &assets, c.crop[0], 2, at, 0xE8);
    word_at(&canvas, &assets, 77, 4, at, 0xE8);

    // Facing Winter: the harvest, one Season off.
    game.kingdom.advance_season();
    game.kingdom.advance_season();
    game.kingdom.refresh_estimates(id);
    assert_eq!(game.kingdom.season_next, 4, "setup: facing Winter");
    let c = game.kingdom.counties[id].clone();
    assert!(c.crop[2] > 0, "setup: there is a harvest to forecast: {:?}", c.crop);
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, c.crop[2], 2, 0x40, 0xD8);
    let at = word_at(&canvas, &assets, 77, 3, at, 0xD8);
    count_at(&canvas, &assets, 1, 0x42, at, 0xD8);

    // Advanced farming: the fertility phrase and the weather's swing.
    game.kingdom.options.advanced_farming = true;
    {
        let t = game.kingdom.tables;
        let c = &mut game.kingdom.counties[id];
        c.weather = Weather::Sunny;
        l2_kingdom::land::grow(&t, c, true);
    }
    let c = game.kingdom.counties[id].clone();
    assert!(c.grain_weather_change != 0, "setup: a Sunny season moves the growing crop");
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let band = ((c.fertility + 100) / 29) as usize;
    word_at(&canvas, &assets, 22, band, 0x80, 0x98);
    let (shown, index) = if c.grain_weather_change > 0 {
        (c.grain_weather_change, 0x10)
    } else {
        (-c.grain_weather_change, 0x11)
    };
    let at = count_at(&canvas, &assets, shown, 2, 0x40, 0xC0);
    word_at(&canvas, &assets, 77, index, at, 0xC0);
}

// ------------------------------------------------------------------ cattle

/// **`Panel_JobCattle`, on the England position's stored herds**, which carry
/// every row this painter has in both signs: births, deaths, slaughter and the
/// overall change are all non-zero, a herd eaten by nobody draws the zero, and
/// the crowding bands 10, 20 and 40 are each some county's.
///
/// Zero here: `+0x274` (the no-event line is asserted; the event test
/// below moves it).
///
/// Ablations, run: the crowding arm `20 => 9` → `20 => 11` → *"Average herd
/// crowding."* not at `(0x40, 0xA0)`; `wrapping_sub` → `wrapping_add` in the
/// farming row → `"+6"` not at `(0x130, 0xF8)`.
#[test]
fn the_cattle_popup_draws_the_herd_its_crowding_and_three_signed_rows() {
    let (mut game, assets) = england_world!();
    let k = &game.kingdom;
    let crowded = county_where(k, "a herd at crowding 20 that no one eats", |c| {
        c.herd_crowding == 20 && c.herd_eaten == 0 && c.herd_births_expected > c.herd_deaths_expected
    });
    let eaten = county_where(k, "a herd being eaten", |c| c.herd_eaten > 0 && c.herd_crowding == 10);
    let shrinking =
        county_where(k, "a massively overcrowded herd", |c| c.herd_crowding == 40 && c.herd_change_expected < 0);

    for (id, band) in [(crowded, 9), (eaten, 8), (shrinking, 11)] {
        let c = game.kingdom.counties[id].clone();
        let canvas = draw_job(&mut game, &assets, id, JOB_CATTLE_FARMING);
        count_at(&canvas, &assets, c.herd, 4, 0x130, 0x88);
        word_at(&canvas, &assets, 77, band, 0x40, 0xA0);
        word_at(&canvas, &assets, 77, 0x13, 0x40, 0xB0);
        word_at(&canvas, &assets, 77, 5, 0x40, 0xD8);
        count_at(&canvas, &assets, c.herd_births_expected, 4, 0x130, 0xD8);
        word_at(&canvas, &assets, 77, 6, 0x40, 0xE8);
        count_at(&canvas, &assets, c.herd_deaths_expected, 4, 0x130, 0xE8);
        word_at(&canvas, &assets, 77, 7, 0x40, 0xF8);
        signed_at(&canvas, &assets, c.herd_births_expected - c.herd_deaths_expected, 0xF8);
        word_at(&canvas, &assets, 77, 0x1B, 0x40, 0x108);
        signed_at(&canvas, &assets, -c.herd_eaten, 0x108);
        word_at(&canvas, &assets, 77, 0x1C, 0x40, 0x118);
        signed_at(&canvas, &assets, c.herd_change_expected, 0x118);
    }
}

/// **The weather's line, both signs and none**, with advanced farming on and
/// `Herd_SeasonTick` writing `+0x270` from a *Sunny*, a *Frost* and a *Cloudy*
/// band. The band is the hand-set input; the figure is the rule's.
///
/// Ablation, run: the `v < 0` arm of `weather_line` given `0x10` → 77/0x11 not
/// at the chained x on `0xC0`.
#[test]
fn the_cattle_popup_says_what_the_weather_did_to_the_herd() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    game.kingdom.options.advanced_farming = true;
    let id = county_where(&game.kingdom, "the player's, with a herd", |c| c.owner == player && c.herd > 40);

    for (weather, index) in [(Weather::Sunny, 0x10), (Weather::Frost, 0x11), (Weather::Cloudy, 0x12)] {
        {
            let t = game.kingdom.tables;
            let c = &mut game.kingdom.counties[id];
            c.weather = weather;
            l2_kingdom::land::herd_season_tick(&t, c, 1, 2);
        }
        let v = game.kingdom.counties[id].herd_weather_change;
        let canvas = draw_job(&mut game, &assets, id, JOB_CATTLE_FARMING);
        match index {
            0x12 => {
                assert_eq!(v, 0, "setup: Cloudy leaves the herd alone");
                word_at(&canvas, &assets, 77, 0x12, 0x40, 0xC0);
            }
            _ => {
                assert!(v != 0, "setup: {weather:?} moves the herd");
                let at = count_at(&canvas, &assets, v.abs(), 4, 0x40, 0xC0);
                word_at(&canvas, &assets, 77, index, at, 0xC0);
            }
        }
    }
}

// ------------------------------------------------------------------ events

