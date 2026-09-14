#![allow(unused_imports)]
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

// ------------------------------------------------------------------- grain

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

/// **`Panel_JobIndustry` for iron, wood and stone, on a save whose blacksmiths
/// are working.** `battle-during.sav` has a county making weapons from both
/// wood and iron, so `+0x280` and `+0x284` — the two figures C164 found drawn
/// by this painter and read by nothing of ours — are non-zero, and so are the
/// outputs. Advanced farming is switched on, as a player does from the options
/// screen, for the efficiency line.
///
/// **The county is chosen so the two smiths' figures differ** (42 and 21).
/// `siege-old_turn.sav`'s county is 56 and 56, where swapping them in the
/// painter stays green — an ablation that cannot fail is a test that cannot.
///
/// Zero here: stone's output and `+0x288`/`+0x28C` (no castle is
/// being built; the castle test below makes the stone figure move). **And the
/// output and efficiency cannot tell wood from iron on any save here**: every
/// county that has both produces the same amount of each at the same 80%.
///
/// Ablations, run: `smiths_iron` → `smiths_wood` in the iron arm → `"21"` not at
/// `(0x44, 0xC0)`; the efficiency's `"%"` → `""` → `"80%"` not at
/// `(0x137, 0xA0)`. **And one stayed green, which is the finding above:**
/// `JOB_IRON_MINING => (Commodity::Wood, …)`, the iron popup reading the wood
/// record, passes — 31 and 31, 80 and 80.
#[test]
fn the_industry_popup_counts_output_and_what_the_blacksmiths_will_use() {
    let (mut game, assets) = fixture_world!("battle-during.sav");
    let t = game.kingdom.tables;
    let id = county_where(&game.kingdom, "smithing unequal wood and iron", |c| {
        let f = l2_kingdom::industry::panel_figures(&t, c);
        f[0] > 0 && f[1] > 0 && f[0] != f[1] && c.industry[Commodity::Iron.index()].next_season > 0
    });
    game.kingdom.options.advanced_farming = true;
    let c = game.kingdom.counties[id].clone();
    let [smiths_wood, smiths_iron, castle_wood, castle_stone] =
        l2_kingdom::industry::panel_figures(&t, &c);

    for (job, record, noun) in [
        (JOB_IRON_MINING, Commodity::Iron, 0x0C),
        (JOB_WOOD_CUTTING, Commodity::Wood, 0x10),
        (JOB_STONE_QUARRYING, Commodity::Stone, 0x0E),
    ] {
        let r = c.industry[record.index()];
        let canvas = draw_job(&mut game, &assets, id, job);
        // `Eng_DrawString(76, 0, 0x40, 0xA0)` + `Ui_DrawNumber(eff, '@', "%", pen + 0x40)`.
        let at = word_at(&canvas, &assets, 76, 0, 0x40, 0xA0);
        expect_at(&canvas, body(&assets), &format!("{}%", r.efficiency), INK, at + 4, 0xA0);
        let at = count_at(&canvas, &assets, r.next_season, noun, 0x40, 0xB0);
        word_at(&canvas, &assets, 76, 1, at, 0xB0);
        match job {
            JOB_IRON_MINING => {
                let at = count_at(&canvas, &assets, smiths_iron, noun, 0x40, 0xC0);
                word_at(&canvas, &assets, 76, 2, at, 0xC0);
            }
            JOB_WOOD_CUTTING => {
                let at = count_at(&canvas, &assets, smiths_wood, noun, 0x40, 0xC0);
                word_at(&canvas, &assets, 76, 2, at, 0xC0);
                let at = count_at(&canvas, &assets, castle_wood, noun, 0x40, 0xD0);
                word_at(&canvas, &assets, 76, 3, at, 0xD0);
            }
            _ => {
                let at = count_at(&canvas, &assets, castle_stone, noun, 0x40, 0xC0);
                word_at(&canvas, &assets, 76, 3, at, 0xC0);
            }
        }
    }
}

// ------------------------------------------------------------------ castle

/// **`Castle_DrawStatusBlock` via `FUN_00414220`, for a castle the rule is
/// building.** A county of the player's orders the next castle up with an empty
/// store (`industry::order_castle`, the build screen's order), so both materials
/// are owed and `Castle_BuildEstimate` answers a hundred seasons. Every figure
/// on the block is then non-zero except one.
///
/// The column is `x + 0x60 = 0x40` and the rows `y + 0x68 …` = `0xA8`, `0xB8`,
/// `0xD0`, `0xE0`, `0xF0`. The stone quarry's popup is checked too while the
/// stone is owed, because `+0x28C` is that figure.
///
/// **Zero here: the builders.** Delivering the materials opens the
/// castle's ceiling (`labour_useful` 1500) and even an industry split of 100
/// staffs nobody, because castle building's share at `+0x130 + 3*4` is 0 after
/// `order_castle` — wood cutting takes all 435.
/// drawn by any test either. Measured and not chased: whether `Castle_Order`
/// leaves that share alone too was not read.
///
/// Ablations, run: `CASTLE_TAX_BONUS_BASE + type` → `+ 1 + type` → `"125"` not
/// at `(0x106, 0xA8)`; `seasons == 0` inverted → the builders' `"0"` not at
/// `(0x44, 0xF0)`.
#[test]
fn a_castle_under_construction_reports_materials_builders_and_seasons() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let k = &mut game.kingdom;
    let id = county_where(k, "the player's, with a keep", |c| c.owner == player && c.castle_type == 3);
    let owner = k.counties[id].owner as usize;
    k.realms[owner].stone = 0;
    k.realms[owner].wood = 0;
    let t = k.tables;
    assert!(
        l2_kingdom::industry::order_castle(&t, &mut k.counties[id], &mut k.realms[owner], 4),
        "setup: a stone castle may be ordered over a keep"
    );
    let c = k.counties[id].clone();
    assert!(c.castle_stone_owed > 0 && c.castle_wood_owed > 0, "setup: both owed: {c:?}");
    assert_eq!(l2_kingdom::industry::castle_seasons_left(&t, &c), 100, "setup: nothing delivered");

    let canvas = draw_job(&mut game, &assets, id, JOB_CASTLE_BUILDING);
    let f = body(&assets);
    // 71/0x10 + `Ui_DrawNumber(125, ' ', " %", …)` — a stone castle's bonus.
    let at = word_at(&canvas, &assets, 71, 0x10, 0x40, 0xA8);
    expect_at(&canvas, f, "125", INK, at + 4, 0xA8);
    // 71/0xB + `Ui_DrawNumber(400, ' ', " ", …)` + 71/0xC.
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x40, 0xB8);
    expect_at(&canvas, f, "400", INK, at + 4, 0xB8);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 400 "), 0xB8);
    let at = count_at(&canvas, &assets, c.castle_stone_owed, 0x0E, 0x40, 0xD0);
    word_at(&canvas, &assets, 71, 6, at, 0xD0);
    let at = count_at(&canvas, &assets, c.castle_wood_owed, 0x10, 0x40, 0xE0);
    word_at(&canvas, &assets, 71, 7, at, 0xE0);
    let at = count_at(&canvas, &assets, c.labour[JOB_CASTLE_BUILDING], 0x26, 0x40, 0xF0);
    let at = word_at(&canvas, &assets, 71, 8, at, 0xF0);
    count_at(&canvas, &assets, 100, 0x42, at, 0xF0);

    // The quarry popup's second line is the stone still owed.
    let canvas = draw_job(&mut game, &assets, id, JOB_STONE_QUARRYING);
    let at = count_at(&canvas, &assets, c.castle_stone_owed, 0x0E, 0x40, 0xC0);
    word_at(&canvas, &assets, 76, 3, at, 0xC0);

}

/// **A county with no castle reads its barracks from the neighbouring table.**
///
/// `siege-aftersie.sav` has a county of the player's with `castleType` 0, which
/// is stored and not staged. `Castle_DrawStatusBlock` indexes
/// `&DAT_004D8A0C + type * 4`, so type 0 reads the last word of
/// `CASTLE_WORKFORCE`, **2500**, and `&DAT_004D8A24 + 0` reads the garrison
/// table's trailing **0**. `[V]` on the bytes and the painter; `[I]` that a
/// player sees it — the original was not run for this.
///
/// Zero here: both materials, and `+0x1A6` (the no-build line is what
/// is asserted).
///
/// Ablations, run: `CASTLE_BARRACKS_BASE` → `(0x004D_8A10 - 0x004D_89E8) / 4`
/// → `"2500"` not at `(0xB5, 0xB8)`; `seasons == 0` inverted → *"No castle
/// building in progress."* not at `(0x40, 0xF0)`.
#[test]
fn a_county_with_no_castle_reads_barracks_for_2500_from_the_next_table() {
    let (mut game, assets) = fixture_world!("siege-aftersie.sav");
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's with no castle", |c| {
        c.owner == player && c.castle_type == 0 && c.castle_degraded == 0
    });
    let canvas = draw_job(&mut game, &assets, id, JOB_CASTLE_BUILDING);
    let f = body(&assets);
    let at = word_at(&canvas, &assets, 71, 0x10, 0x40, 0xA8);
    expect_at(&canvas, f, "0", INK, at + 4, 0xA8);
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x40, 0xB8);
    expect_at(&canvas, f, "2500", INK, at + 4, 0xB8);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 2500 "), 0xB8);
    let at = count_at(&canvas, &assets, 0, 0x0E, 0x40, 0xD0);
    word_at(&canvas, &assets, 71, 6, at, 0xD0);
    let at = count_at(&canvas, &assets, 0, 0x10, 0x40, 0xE0);
    word_at(&canvas, &assets, 71, 7, at, 0xE0);
    word_at(&canvas, &assets, 71, 0x11, 0x40, 0xF0);
}

// ------------------------------------------- the tile panel's same two groups
//
// `TileInfo_DrawCastle` (`0x0041DA2F`) and `TileInfo_DrawGrain`/`…Herd` draw
// groups 71 and 77 out of a county, at a y built
// from `DAT_00553D2C` instead of a literal. The helpers above measure both
// the tile panel's arms are checked here.

/// **`Panel_JobReclamation`, on the stored zeros.** Every save on this machine
/// has no field under reclamation, so this is **zero**: *"0 fields
/// being reclaimed"* on `0xB8` and 77/0xF on `200`. It pins the two lines'
/// places and the plural at zero
/// `Ui_DrawCount` here — `(byte) +0x204 == 1` picks 0xC, anything else 0xD.
///
/// Ablation, run: the `'@'` lead → `' '` has no effect on the pixels (both are
/// glyph-less); the `fields == 1` test inverted → 77/0xD not at the chained x.
#[test]
fn the_reclamation_popup_draws_its_two_lines_on_a_county_reclaiming_nothing() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's", |c| c.owner == player);
    let c = game.kingdom.counties[id].clone();
    assert_eq!((c.fields_reclaiming, c.reclaim_seasons_to_next), (0, 0), "setup: nothing reclaimed");
    let canvas = draw_job(&mut game, &assets, id, JOB_FIELD_RECLAMATION);
    let f = body(&assets);
    expect_at(&canvas, f, "0", INK, 0x40 + 4, 0xB8);
    word_at(&canvas, &assets, 77, 0x0D, 0x40 + 4 + advance(f, "0"), 0xB8);
    word_at(&canvas, &assets, 77, 0x0F, 0x40, 200);
}

// -------------------------------------------------------------- blacksmith

/// Our zero-based job 7, the blacksmith — `g_jobPanelJob == 8`.
const JOB_BLACKSMITH: usize = 7;
/// `g_weaponCost[5]` — armour, **4 wood and 18 iron**, the only row whose two
/// costs differ enough to tell a transposed pair apart at a glance.
const ARMOUR: usize = 5;
/// `DAT_004D29C8[5]` — group 8's singular index for armour, whose plural at 29
/// is *"Armour"* again.
const ARMOUR_NOUN: usize = 28;

/// **`Panel_JobBlacksmith` (`0x00413155`) — its words, its two figures and its
/// two costs**, on a county whose smithy is staffed.
///
/// The line that matters most is `Ui_DrawCentred(75, 0, 0, 0x1CC, 0x1CC)` —
/// *"Click on a weapon to change production."* **`Panel_JobBlacksmith` is group
/// 75's only consumer in the whole binary**, so this is the page's own
/// vocabulary, and it is the sentence a
/// player reported missing by reporting the control: *"I can't choose what type
/// of weapon my blacksmiths are making."*
///
/// The centred line is asserted for **its row and its ink** and not its x,
/// because `Ui_DrawCentred`'s x is `(width - measure) / 2` over the whole string
/// and re-deriving it here would be re-deriving `FUN_004025D7`. Every other
/// piece is at a literal out of the painter or chained from one.
///
/// Advanced Farming is off in every save on this machine, so the `76/4` + `76/5`
/// branch is **unexercised** and `76/8` — *"Smiths working."* — is the
/// one that runs. Said here.
///
/// Ablations, run: the two cost numbers swapped → `"18"` is at 440 and not at
/// `0x16A + 4`; 76/6's `SMITHY_ROW_OUTPUT` → `SMITHY_ROW_WORKERS` → the smiths
/// figure goes red first, because the moved word is drawn over it;
/// `WEAPON_NOUN[weapon]` → a constant `0x12` (Pike) → *"Armour"* is on no row at
/// all.
#[test]
fn the_blacksmith_page_draws_group_75_and_the_weapon_it_forges() {
    let (mut game, assets) = fixture_world!("siege-lastturn.sav");
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's with a smithy site", |c| {
        c.owner == player && c.industry[Commodity::Weapons.index()].has_resource && c.pop_band > 0
    });
    // **No save on this machine has a staffed smithy**, so the state is reached
    // by the two roads a player reaches it by: the map click that switches the
    // site on (`Industry_ToggleFromMap`) and the village drag that staffs it
    // (`Labour_Move`). A hand-set `labour[7]` would have drawn a page whose
    // forecast nothing computed.
    if !game.kingdom.counties[id].industry[Commodity::Weapons.index()].enabled {
        game.kingdom.toggle_industry(id, l2_kingdom::industry::MapToggle::Industry(Commodity::Weapons));
    }
    let spare = game.kingdom.counties[id].labour[JOB_WOOD_CUTTING] / 2;
    game.kingdom.move_labour(id, JOB_WOOD_CUTTING, JOB_BLACKSMITH, spare);
    assert!(game.kingdom.set_weapon_type(id, ARMOUR), "the click the page answers");
    assert!(game.kingdom.counties[id].labour[JOB_BLACKSMITH] > 0, "setup: the smithy is staffed");
    assert!(
        !game.kingdom.options.advanced_farming,
        "setup: this fixture has Advanced Farming off, so 76/8 is the workers line"
    );
    let c = game.kingdom.counties[id].clone();

    let canvas = draw_job(&mut game, &assets, id, JOB_BLACKSMITH);
    let f = body(&assets);
    let h = heading(&assets);

    // `Eng_DrawString(74, 8, 0x10, 0x186, &g_fontHeading, 0x3F)` — the page's
    // title, in the heading face and not the body's.
    expect_at(&canvas, h, &eng(&assets, 74, 8), INK, 0x10, 0x186);

    // `Ui_DrawCentred(75, 0, 0, 0x1CC, 0x1CC, &g_fontBody, 0x3F)`.
    let say = eng(&assets, 75, 0);
    assert!(
        !xs_on_row(&canvas, f, &say, INK, 0x1CC).is_empty(),
        "{say:?} — the only string of group 75 anything draws — is not on row 0x1CC"
    );

    // `Ui_DrawNumber(labour[7], '@', " ", 0x10, 0x1A4)` then 76/8. The `'@'`
    // lead has no glyph and advances four; the `" "` suffix is a space.
    let smiths = c.labour[JOB_BLACKSMITH].to_string();
    expect_at(&canvas, f, &smiths, INK, 0x10 + 4, 0x1A4);
    word_at(&canvas, &assets, 76, 8, 0x10 + advance(f, &format!("@{smiths} ")), 0x1A4);

    // 76/6, the count in the weapon's own noun, 76/7.
    let at = word_at(&canvas, &assets, 76, 6, 0x10, 0x1B4);
    let made = c.industry[Commodity::Weapons.index()].next_season;
    let at = count_at(&canvas, &assets, made, ARMOUR_NOUN, at, 0x1B4);
    word_at(&canvas, &assets, 76, 7, at, 0x1B4);

    // The cost well. **Iron is drawn first**, at 0x16A, and it is `g_weaponCost`'s
    // *second* word: armour is 4 wood and 18 iron, so these two are the check
    // that the pair is not read the wrong way round.
    expect_at(&canvas, f, "18", INK, 0x16A + 4, 0x18E);
    expect_at(&canvas, f, "4", INK, 0x1B4 + 4, 0x18E);
}

/// **The six weapon hotspots are the player's own exe's**, `DAT_004DCA10` read
/// at a 24-byte stride, plus `Hotspot_Test`'s two offsets.
///
/// `Hotspot_Test(0, 0x18, &DAT_004DCA10, 6)` **adds** its first two arguments to
/// every record before the test, and `0x18` is exactly where
/// `Sprite_WGenSprite(0, 0, 0x18)` puts the smithy picture — so the table is in
/// the picture's coordinates. Read as screen coordinates every weapon sits 24
/// pixels high, and a click a player aims at the pike lands on the bow. That is
/// the defect this test exists to make impossible.
///
/// The record is `{x0, y0, x1, y1}` as `i16`s at `+0x00`, `+0x02`, `+0x04`,
/// `+0x06`, and `Hotspot_Test` is **half-open** on both axes: `x0 <= mx < x1`.
/// `docs/input.md` §1.
///
/// Ablation, run: `HOTSPOT_ORIGIN` → `(0, 0)` turns the two corner claims red.
#[test]
fn the_weapon_hotspots_are_the_exes_own_table_at_the_pictures_origin() {
    use l2_game::screens::job::{weapon_at, HOTSPOT_ORIGIN, WEAPON_HOTSPOTS};

    let exe = l2_testkit::executable!();
    let table = l2_testkit::pe::Table::at(&exe, 0x004D_CA10);
    let (dx, dy) = HOTSPOT_ORIGIN;
    assert_eq!((dx, dy), (0, 0x18), "Hotspot_Test's two offsets, and 0x18 is the picture's y");

    for (w, &(x0, y0, x1, y1)) in WEAPON_HOTSPOTS.iter().enumerate() {
        // Twelve `i16`s a record.
        let at = |field: usize| i32::from(table.u16_at(w * 12 + field) as i16);
        assert_eq!((at(0), at(1), at(2), at(3)), (x0, y0, x1, y1), "weapon {w}'s rectangle");
        // The kind byte at `+0x0F` — 1, the down edge, for all six.
        assert_eq!(table.u8_at(w * 24 + 0x0F), 1, "weapon {w} is kind 1");
        // And `g_uiHotspotId` at `+0x10` is the weapon type itself, which is
        // what makes table order weapon order.
        assert_eq!(table.i32_at(w * 6 + 4), w as i32, "weapon {w}'s g_uiHotspotId");

        // Inside the rectangle, in screen coordinates, is this weapon; the far
        // corner itself is not, because the test is half-open.
        assert_eq!(weapon_at(x0 + dx, y0 + dy), Some(w), "weapon {w}'s near corner");
        assert_eq!(weapon_at(x1 + dx - 1, y1 + dy - 1), Some(w), "weapon {w}'s last pixel");
        assert_ne!(weapon_at(x1 + dx, y1 + dy), Some(w), "weapon {w}'s far corner is past it");
    }
    // The corner picture at (448, 448) is below every one of them, so the OK
    // button and the weapons cannot collide.
    assert_eq!(weapon_at(0x1C0, 0x1C0), None, "the OK button is on no weapon");
}

