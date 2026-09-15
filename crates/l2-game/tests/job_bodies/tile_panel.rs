#![allow(unused_imports)]
use super::*;
use super::job_popups::*;
use super::events_and_letters::*;
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

fn draw_tile_panel(game: &mut Game, assets: &Assets, tile: usize) -> Canvas {
    let mut screen = l2_game::screens::info::InfoScreen::new(
        l2_game::screens::info::Target::Tile(tile),
    );
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

#[track_caller]
fn a_castle_tile(k: &Kingdom, county: usize) -> usize {
    let map = &k.campaign.map;
    (0..map.terrain.len())
        .find(|&t| {
            map.county[t] as usize == county
                && (l2_kingdom::map::terrain::CASTLE_FROM..=l2_kingdom::map::terrain::CASTLE_TO)
                    .contains(&map.terrain[t])
        })
        .unwrap_or_else(|| panic!("setup: county {county} has no castle tile"))
}

/// `FUN_0041BEFE` gives an intact, unruined castle row `0x0E`, so every y is
/// `0x0E * 0x10 + k`. The tax and barracks words are the same two table reads
/// `Castle_DrawStatusBlock` makes, one word low
/// prints `CASTLE_TAX_BONUS_PCT[2]` = 100 and `CASTLE_GARRISON_CAP[2]` = 200 —
/// the shift `a_county_with_no_castle_reads_barracks_for_2500_from_the_next_table`
/// pins at the other end of the run.
#[test]
fn the_tile_panel_draws_an_intact_castles_tax_bonus_barracks_and_garrison() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's, with a keep", |c| {
        c.owner == player && c.castle_type == 3 && c.castle_degraded == 0 && !c.castle_ruined
    });
    let tile = a_castle_tile(&game.kingdom, id);
    assert_eq!(game.kingdom.counties[id].garrison_unit, 0, "setup: nobody is barracked yet");

    const R: i32 = 0x0E * 0x10;
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    let f = body(&assets);
    expect_at(&canvas, heading(&assets), &eng(&assets, 30, 8), INK, 0x28, R + 0x40);
    let at = word_at(&canvas, &assets, 71, 0x10, 0x68, R + 0x88);
    expect_at(&canvas, f, "100", INK, at + 4, R + 0x88);
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x68, R + 0x98);
    expect_at(&canvas, f, "200", INK, at + 4, R + 0x98);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 200 "), R + 0x98);
    assert!(
        !is_at(&canvas, f, &eng(&assets, 71, 0x0E), INK, 0x68, R + 0xC4),
        "an empty castle offers no \"View these troops?\""
    );

    // **A garrison, and then the other owner's.** The fixture ships merchants
    // and no army; the painter reads only `owner` and `+0x168`, and the branch
    // it picks is the owner test.
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");
    game.kingdom.counties[id].garrison_unit = unit;
    {
        let u = game.kingdom.campaign.units.get_mut(unit).expect("the merchant");
        u.owner = player;
        u.men = 40;
    }
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    expect_at(&canvas, f, "40", INK, 0x68 + 4, R + 0xA8);
    word_at(&canvas, &assets, 71, 0x0D, 0x68 + advance(f, " 40 "), R + 0xA8);
    word_at(&canvas, &assets, 71, 0x0E, 0x68, R + 0xC4);

    let others = (1..=5u8).find(|&r| r != player).expect("another realm");
    game.kingdom.campaign.units.get_mut(unit).expect("the merchant").owner = others;
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    word_at(&canvas, &assets, 71, 0x13, 0x68, R + 0xA8);
    word_at(&canvas, &assets, 71, 0x0E, 0x68, R + 0xC4);
}

/// **`TileInfo_DrawCastle`'s degraded arm** — the other half, and the one the
/// panel shares with the job page: `Castle_DrawStatusBlock(county, 8, 0x30,
/// DAT_00553D2C)` under `Ui_DrawCount(labour[3], 0x26, 0x68, R + 0x88)`.
///
/// `FUN_0041BEFE` gives it row `0x0A`, the tallest tile layout in the game, and
/// only to the county's owner. So the block's column is `8 + 0x60 = 0x68` and
/// its rows start at `0x0A * 0x10 + 0x30 + 0x68`.
#[test]
fn the_tile_panel_of_a_castle_under_construction_is_the_status_block() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's, with a keep", |c| {
        c.owner == player && c.castle_type == 3
    });
    let tile = a_castle_tile(&game.kingdom, id);
    let k = &mut game.kingdom;
    let owner = k.counties[id].owner as usize;
    k.realms[owner].stone = 0;
    k.realms[owner].wood = 0;
    let t = k.tables;
    assert!(
        l2_kingdom::industry::order_castle(&t, &mut k.counties[id], &mut k.realms[owner], 4),
        "setup: a stone castle may be ordered over a keep"
    );
    let c = k.counties[id].clone();
    assert_eq!(c.castle_degraded, 1, "setup: work is under way");
    assert!(c.castle_stone_owed > 0 && c.castle_wood_owed > 0, "setup: both owed");

    const R: i32 = 0x0A * 0x10;
    const TOP: i32 = R + 0x30;
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    let f = body(&assets);
    expect_at(&canvas, heading(&assets), &eng(&assets, 30, 0x0E), INK, 0x28, R + 0x40);
    count_at(&canvas, &assets, c.labour[JOB_CASTLE_BUILDING], 0x26, 0x68, R + 0x88);
    let at = word_at(&canvas, &assets, 71, 0x10, 0x68, TOP + 0x68);
    expect_at(&canvas, f, "125", INK, at + 4, TOP + 0x68);
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x68, TOP + 0x78);
    expect_at(&canvas, f, "400", INK, at + 4, TOP + 0x78);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 400 "), TOP + 0x78);
    let at = count_at(&canvas, &assets, c.castle_stone_owed, 0x0E, 0x68, TOP + 0x90);
    word_at(&canvas, &assets, 71, 6, at, TOP + 0x90);
    let at = count_at(&canvas, &assets, c.castle_wood_owed, 0x10, 0x68, TOP + 0xA0);
    word_at(&canvas, &assets, 71, 7, at, TOP + 0xA0);
}

/// **The field panel's four weather and event figures**, which the module docs
/// in `screens/info.rs` said were not carried — `docs/stored-fields.json` has
/// all four *imported*, and `Panel_JobGrain` has been drawing them since C164.
///
/// `TileInfo_DrawGrain` and `TileInfo_DrawHerd` put them at `(0x28, R + 0x94)`
/// and `(0x28, R + 0xA4)` where the job popup uses `(0x40, 0xB0)` and
/// `(0x40, 0xC0)`; `FUN_0041BEFE` gives a real field of the player's `R = 5`.
#[test]
fn the_field_panel_says_what_the_weather_and_the_events_did() {
    use l2_game::screens::info::{mode, FARM_TILE_INFO};
    let (mut game, assets) = england_world!();
    let player = game.player;
    game.kingdom.options.advanced_farming = true;
    let field = |k: &Kingdom, want: usize| {
        let map = &k.campaign.map;
        (0..map.terrain.len())
            .find(|&t| {
                let c = map.county[t] as usize;
                c != 0
                    && c <= k.county_count
                    && k.counties[c].owner == player
                    && map.flags[t] & l2_kingdom::map::flags::FARMLAND != 0
                    && FARM_TILE_INFO
                        .get(map.terrain[t] as usize)
                        .is_some_and(|row| row[3] == want)
            })
            .map(|t| (map.county[t] as usize, t))
            .unwrap_or_else(|| panic!("setup: the player has no field in mode {want:#x}"))
    };
    let (id, pasture) = field(&game.kingdom, mode::CATTLE);
    assert!(game.kingdom.counties[id].herd > 0, "setup: county {id} has a herd to move");
    const R: i32 = 5 * 0x10;

    for (weather, index) in
        [(Weather::Sunny, 0x10), (Weather::Frost, 0x11), (Weather::Cloudy, 0x12)]
    {
        {
            let t = game.kingdom.tables;
            let c = &mut game.kingdom.counties[id];
            c.weather = weather;
            l2_kingdom::land::herd_season_tick(&t, c, 1, 2);
        }
        let v = game.kingdom.counties[id].herd_weather_change;
        let canvas = draw_tile_panel(&mut game, &assets, pasture);
        if index == 0x12 {
            assert_eq!(v, 0, "setup: Cloudy leaves the herd alone");
            word_at(&canvas, &assets, 77, 0x12, 0x28, R + 0xA4);
        } else {
            assert!(v != 0, "setup: {weather:?} moves the herd");
            let at = count_at(&canvas, &assets, v.abs(), 4, 0x28, R + 0xA4);
            word_at(&canvas, &assets, 77, index, at, R + 0xA4);
        }
    }

    {
        let c = &mut game.kingdom.counties[id];
        c.event_id = 0x89;
        c.herd_event_change = -7;
    }
    let canvas = draw_tile_panel(&mut game, &assets, pasture);
    let at = count_at(&canvas, &assets, -7, 4, 0x28, R + 0x94);
    word_at(&canvas, &assets, 77, 0x15, at, R + 0x94);

    assert!(
        paint_all_fallow_to_grain(&mut game.kingdom, id) > 0,
        "setup: county {id} has fallow fields to sow"
    );
    let (grain_id, wheat) = field(&game.kingdom, mode::WHEAT);
    {
        let c = &mut game.kingdom.counties[grain_id];
        c.event_id = 0x87;
        c.grain_event_change = -25;
        c.grain_weather_change = 12;
    }
    let canvas = draw_tile_panel(&mut game, &assets, wheat);
    let at = count_at(&canvas, &assets, -25, 2, 0x28, R + 0x94);
    word_at(&canvas, &assets, 77, 0x19, at, R + 0x94);
    let at = count_at(&canvas, &assets, 12, 2, 0x28, R + 0xA4);
    word_at(&canvas, &assets, 77, 0x10, at, R + 0xA4);
}


