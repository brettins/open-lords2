#![allow(unused_imports)]
use super::*;
use super::views::*;
use super::super::*;
use super::super::interaction::*;
use super::super::*;
use super::super::tile_panel_part::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// Every `y` below is `row * 16 + k` with `row = 5` (`FUN_0041BEFE`, a real
/// field of yours) or `0x11` (anybody else's), and every `k` and `x` is a
/// literal of `TileInfo_Draw`, `TileInfo_DrawGrain` or `TileInfo_DrawHerd`, not
/// a constant of `screens/info.rs`. The words are read out of `L2.eng` here and
/// asserted to be the ones the indices name first, so an index off by one
/// fails on the word and not on somebody's screen.
#[test]
fn the_field_panel_says_what_the_field_is_in_the_players_own_words() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::field::FieldType;
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count)
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the player holds a county");
    let fallow: Vec<usize> = game
        .kingdom
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, k)| k == FieldType::Fallow)
        .map(|(t, _)| t)
        .collect();
    assert!(fallow.len() >= 2, "county {county} has two fallow fields to sow and graze");
    let (wheat, meadow) = (fallow[0], fallow[1]);
    game.kingdom.paint_field(county, wheat, FieldType::Grain).expect("the grain brush");
    game.kingdom.paint_field(county, meadow, FieldType::Pasture).expect("the pasture brush");

    for (group, index, word) in [
        (30, 6, "Farmland"),
        (30, 19, "- Wheat."),
        (30, 21, "- Cattle."),
        (77, 1, "to be sown, yielding"),
        (77, 2, "in 4 seasons."),
        (77, 5, "Calf births expected"),
        (77, 6, "Cow deaths expected"),
        (77, 7, "Change due to farming"),
        (77, 27, "Change due to eating"),
        (77, 28, "Overall change"),
    ] {
        assert_eq!(assets.shell.text(group, index), word, "L2.eng {group}/{index}");
    }

    let panel = |game: &mut Game, tile: usize| {
        let mut m = over_the_map(ScreenId::Info(Target::Tile(tile)));
        draw_stack(&mut m, game, &assets)
    };
    let y = |row: i32, k: i32| row * 16 + k;
    let t = font::TEXT;

    let c = panel(&mut game, wheat);
    assert_eq!(find_heading(&c, &assets, "Farmland", t), Some((0x28, y(5, 0x40))), "heading");
    let mode = find_heading(&c, &assets, "- Wheat.", t).expect("the mode follows the heading");
    assert!(mode.1 == y(5, 0x40) && mode.0 > 0x28, "on the heading's line and after it: {mode:?}");
    let sack = find_body(&c, &assets, "Sack", t).expect("the store's noun");
    assert!(sack.1 == y(5, 0x68) && sack.0 > 0x128, "the store at (0x128, 0xB8): {sack:?}");
    assert_eq!(game.kingdom.season_next, 1);
    let sown = find_body(&c, &assets, "to be sown, yielding", t).expect("the sowing line");
    assert_eq!(sown.1, y(5, 0xC0));
    assert_eq!(find_body(&c, &assets, "in 4 seasons.", t).map(|p| p.1), Some(y(5, 0xD0)));
    assert_eq!(find_body(&c, &assets, "Change due to eating", t), Some((0x28, y(5, 0xE0))));
    assert_eq!(find_body(&c, &assets, "Overall change", t), Some((0x28, y(5, 0xF0))));
    assert!(find_body(&c, &assets, "This wheat field", t).is_none());
    assert!(find_text(&c, "TILE HALF", assets.ink.dim).is_none(), "and no placeholder");

    let c = panel(&mut game, meadow);
    let mode = find_heading(&c, &assets, "- Cattle.", t).expect("the pasture's mode");
    assert_eq!(mode.1, y(5, 0x40));
    assert_eq!(find_body(&c, &assets, "Calf births expected", t), Some((0x28, y(5, 0xC0))));
    assert_eq!(find_body(&c, &assets, "Cow deaths expected", t), Some((0x28, y(5, 0xD0))));
    assert_eq!(find_body(&c, &assets, "Change due to farming", t), Some((0x28, y(5, 0xE0))));
    assert_eq!(find_body(&c, &assets, "Change due to eating", t), Some((0x28, y(5, 0xF0))));
    assert_eq!(find_body(&c, &assets, "Overall change", t), Some((0x28, y(5, 0x100))));
    let crowding = (8..=11)
        .find_map(|i| find_body(&c, &assets, assets.shell.text(77, i), t))
        .expect("fieldsCattle is not zero, so a crowding line is drawn");
    assert_eq!(crowding, (0x68, y(5, 0x78)));

    let other = (1..game.kingdom.realms.len() as u8).find(|&r| r != game.player).expect("a rival");
    game.kingdom.counties[county].owner = other;
    let c = panel(&mut game, wheat);
    assert_eq!(find_heading(&c, &assets, "Farmland", t), Some((0x28, y(0x11, 0x40))));
    assert!(find_body(&c, &assets, "Overall change", t).is_none(), "no report on a rival's field");
    let mut screen = InfoScreen::new(Target::Tile(wheat));
    let ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(screen.layout(&ctx).row, 0x11);
    let _ = &mut screen;
}

/// Every `y` is `row * 16 + k` with `row = 0x11` (`FUN_0041BEFE`'s
/// `if (g_pickedTileGraphic < 0xd) DAT_00553d2c = 0x11`) and every `k` and `x`
/// is a literal of `TileInfo_Draw` — `0x28`/`0x40` heading, `0x68`/`100` body,
/// `0x68`/`0x74` the status line — not a constant of `screens/info.rs`. The
/// words come out of the player's `L2.eng` and are asserted to be the ones the
/// indices name, so an index off by one fails on the word.
///
/// **One ablation that does *not* fail, and it is a finding**: adding
/// `flags::BOUNDARY` to `settlement_tile`'s exclusion set changes nothing,
/// because no resource site in the England position carries `0x02`. The bit is
/// not in `TileInfo_Draw`'s ladder either, so the set is right for the reason
/// The painter gives; the fixture does not catch it.
#[test]
fn a_mine_says_it_is_a_mine_how_big_it_is_and_whether_it_is_working() {
    use l2_game::screens::info::Target;
    use l2_kingdom::map::flags;
    use l2_kingdom::tables::Commodity;
    let (mut game, assets) = world!();
    let t = font::TEXT;
    let y = |k: i32| 0x11 * 16 + k;

    for (index, word) in [
        (9, "Mine (iron)."),
        (10, "Quarry (stone)."),
        (11, "Lumber mill (timber)."),
        (82, "Blacksmith (armour)."),
        (53, "A small mine."),
        (54, "A medium mine."),
        (55, "A large mine."),
        (56, "A very large mine."),
        (57, "A destroyed mine."),
        (61, "A small quarry."),
        (69, "A small lumber mill."),
        (83, "A small blacksmiths."),
        (77, "This industry is shut down."),
        (78, "This industry is operational."),
    ] {
        assert_eq!(assets.shell.text(30, index), word, "L2.eng 30/{index}");
    }

    let map = &game.kingdom.campaign.map;
    let sites: Vec<usize> = (0..map.flags.len())
        .filter(|&i| map.flags[i] & flags::SETTLEMENT != 0 && map.terrain[i] < 0x0D)
        .collect();
    assert!(!sites.is_empty(), "the England position places resource sites");
    for &i in &sites {
        let f = map.flags[i];
        assert_eq!(
            f & (flags::ROAD | flags::ROUGH | flags::NO_COUNTY | flags::PLOT | flags::FARMLAND | flags::CASTLE),
            0,
            "tile {i} is a settlement and nothing else the ladder tests first",
        );
    }

    let four = {
        let map = &game.kingdom.campaign.map;
        let pick = |c: Commodity| {
            sites
                .iter()
                .copied()
                .map(|i| (i, map.county[i] as usize))
                .find(|&(i, _)| {
                    l2_kingdom::industry::map_toggle_for_graphic(map.terrain[i])
                        == Some(l2_kingdom::industry::MapToggle::Industry(c))
                })
                .unwrap_or_else(|| panic!("the position places a {c:?} site somewhere"))
        };
        [
            (Commodity::Iron, pick(Commodity::Iron), "Mine (iron).", "A small mine."),
            (Commodity::Stone, pick(Commodity::Stone), "Quarry (stone).", "A small quarry."),
            (
                Commodity::Weapons,
                pick(Commodity::Weapons),
                "Blacksmith (armour).",
                "A small blacksmiths.",
            ),
            (Commodity::Wood, pick(Commodity::Wood), "Lumber mill (timber).", "A small lumber mill."),
        ]
    };
    let (county, mine) = {
        let map = &game.kingdom.campaign.map;
        sites
            .iter()
            .copied()
            .map(|i| (map.county[i] as usize, i))
            .find(|&(c, i)| {
                map.terrain[i] < 4 && game.kingdom.counties[c].owner == game.player
            })
            .expect("the player holds a county with a mine in it")
    };

    let panel = |game: &mut Game, tile: usize| {
        let mut m = over_the_map(ScreenId::Info(Target::Tile(tile)));
        draw_stack(&mut m, game, &assets)
    };

    for (c, (tile, at), heading, small) in four {
        for i in 0..4 {
            game.kingdom.counties[at].industry[i].output = 0;
            game.kingdom.counties[at].industry[i].disabled_seasons = 0;
            game.kingdom.counties[at].industry[i].enabled = true;
        }
        let canvas = panel(&mut game, tile);
        assert_eq!(
            find_heading(&canvas, &assets, heading, t),
            Some((0x28, y(0x40))),
            "{c:?}: the heading at (0x28, row*16 + 0x40)",
        );
        assert_eq!(
            find_body(&canvas, &assets, small, t).map(|p| p.1),
            Some(y(100)),
            "{c:?}: the band at row*16 + 100",
        );
        assert_eq!(
            find_body(&canvas, &assets, "This industry is operational.", t),
            Some((0x68, y(0x74))),
            "{c:?}: the status line at (0x68, row*16 + 0x74)",
        );
        assert!(find_text(&canvas, "TILE HALF", assets.ink.dim).is_none(), "{c:?}: no placeholder");
    }

    for (output, word) in [
        (0, "A small mine."),
        (9, "A small mine."),
        (10, "A medium mine."),
        (24, "A medium mine."),
        (25, "A large mine."),
        (49, "A large mine."),
        (50, "A very large mine."),
        (999, "A very large mine."),
    ] {
        game.kingdom.counties[county].industry[Commodity::Iron.index()].output = output;
        let canvas = panel(&mut game, mine);
        assert_eq!(
            find_body(&canvas, &assets, word, t).map(|p| p.1),
            Some(y(100)),
            "output {output} is {word:?}",
        );
    }

    game.kingdom.counties[county].industry[Commodity::Iron.index()].output = 999;
    game.kingdom.counties[county].industry[Commodity::Iron.index()].disabled_seasons = 2;
    game.kingdom.counties[county].industry[Commodity::Iron.index()].enabled = true;
    let canvas = panel(&mut game, mine);
    assert!(
        find_body(&canvas, &assets, "A destroyed mine.", t).is_some(),
        "disabled_seasons overrides the largest output",
    );
    assert!(find_body(&canvas, &assets, "A very large mine.", t).is_none());
    assert_eq!(
        find_body(&canvas, &assets, "This industry is operational.", t),
        Some((0x68, y(0x74))),
        "destroyed and operational at once",
    );

    game.kingdom.counties[county].industry[Commodity::Iron.index()].enabled = false;
    let canvas = panel(&mut game, mine);
    assert_eq!(
        find_body(&canvas, &assets, "This industry is shut down.", t),
        Some((0x68, y(0x74))),
    );

    let other = (1..game.kingdom.realms.len() as u8).find(|&r| r != game.player).expect("a rival");
    game.kingdom.counties[county].owner = other;
    let canvas = panel(&mut game, mine);
    assert_eq!(find_heading(&canvas, &assets, "Mine (iron).", t), Some((0x28, y(0x40))));
    assert!(find_body(&canvas, &assets, "A destroyed mine.", t).is_some(), "a rival's mine too");
}

