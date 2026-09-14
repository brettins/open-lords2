#![allow(unused_imports)]
use super::*;
use super::tile_panel_part::*;
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

/// **A left click on your own field opens what a right click opens**, and the
/// grain button on it sows the field.
///
/// A player: *"Clicking on a field still brings up placeholder … right click
/// and left click on fields in game."*
/// `Map_Click`'s farmland arm is `_DAT_005681CC = 3; g_screenId = 4;
/// FUN_0041B032();` and the right button's `FUN_0043CAF4` ends in the same two
/// statements, so both land on screen `0x04`'s tile half for the picked tile.
/// The brush's grain button is `g_infoFieldBrush` record 1 at `(304, 376)`,
/// 48 square — the literal from the table, not our constant.
///
/// Driven through the [`Machine`] at the pixel a player would click, on a field
/// That is on screen. **Ablation, run:** deleting the `Push` in the
/// farmland arm of `screens/map/mod.rs` fails the second assertion — the left click
/// stays on the campaign map.
#[test]
fn a_left_click_on_your_own_field_opens_what_a_right_click_opens() {
    use l2_game::screens::info::Target;
    let (mut game, assets) = world!();
    let players: Vec<u8> =
        (1..=game.kingdom.county_count as u8).filter(|&id| game.is_players(id)).collect();
    let (county, tile, at) = players
        .into_iter()
        .find_map(|id| {
            visible_field(&mut game, &assets, id, |k| k == l2_kingdom::field::FieldType::Fallow)
                .map(|(t, at)| (id, t, at))
        })
        .expect("one of the player's fallow fields is in view at the opening viewport");
    let grain_before = game.kingdom.counties[county as usize].fields_grain;
    let panel = Some(ScreenId::Info(Target::Tile(tile)));

    let mut right = Machine::new(ScreenId::Campaign);
    draw_stack(&mut right, &mut game, &assets);
    send_stack(&mut right, &mut game, &assets, Event::RightClick { x: at.0, y: at.1 });
    assert_eq!(right.top_id(), panel, "the right button opens the information panel on that field");
    assert_eq!(right.depth(), 2);
    // The same button closes it, and the map under it keeps its viewport.
    send_stack(&mut right, &mut game, &assets, Event::RightClick { x: at.0, y: at.1 });
    assert_eq!(right.ids(), vec![ScreenId::Campaign]);

    let mut left = Machine::new(ScreenId::Campaign);
    draw_stack(&mut left, &mut game, &assets);
    send_stack(&mut left, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(left.top_id(), panel, "and the left button opens the same screen, on the same tile");
    assert_eq!(left.depth(), 2);

    // The grain button, on the panel the left click opened.
    send_stack(&mut left, &mut game, &assets, Event::Click { x: 328, y: 400 });
    assert_eq!(
        game.kingdom.counties[county as usize].fields_grain,
        grain_before + 1,
        "the button reached Field_SetType"
    );
    assert_eq!(game.kingdom.campaign.map.terrain[tile], l2_kingdom::field::terrain::GRAIN);
    assert_eq!(left.ids(), vec![ScreenId::Campaign], "FUN_00438B02 ends in g_screenId = 0");

    // **The owner test is inside the arm.** The same field in somebody else's
    // hands: the left click falls out of `Map_Click`, and the right one still
    // opens the panel, which has no owner test at all.
    let other = (1..game.kingdom.realms.len() as u8)
        .find(|&r| r != game.player)
        .expect("there is another realm");
    // The two machines that have already opened are reused: a fresh one would
    // open on whichever county the player now holds and look somewhere else.
    game.kingdom.counties[county as usize].owner = other;
    send_stack(&mut left, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(left.ids(), vec![ScreenId::Campaign], "a foreign field opens nothing on the left button");
    send_stack(&mut right, &mut game, &assets, Event::RightClick { x: at.0, y: at.1 });
    assert_eq!(right.top_id(), panel, "and the right button still opens the panel");
}

/// **The field panel says what the field is, in the player's own words, with
/// the field's own figures** — `TileInfo_Draw`'s farmland arm and its two
/// reports, found in their own boxes.
///
/// Every `y` below is `row * 16 + k` with `row = 5` (`FUN_0041BEFE`, a real
/// field of yours) or `0x11` (anybody else's), and every `k` and `x` is a
/// literal of `TileInfo_Draw`, `TileInfo_DrawGrain` or `TileInfo_DrawHerd`, not
/// a constant of `screens/info.rs`. The words are read out of `L2.eng` here and
/// asserted to be the ones the indices name first, so an index off by one
/// fails on the word and not on somebody's screen.
///
/// The fields are painted with the brush (`paint_field` is `Field_SetType`),
/// so the forecasts on the panel are whatever `County_RefreshEstimates` made
/// of that — nothing here writes a county figure.
///
/// **Ablations, run:** deleting the `draw_farmland` call in `InfoScreen::draw`
/// fails at *"Farmland"*; deleting the mode's `pen.heading` fails at
/// *"- Wheat."*; deleting `draw_grain_report`'s call fails at the store's noun,
/// the report's first line.
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

    // The indices name these words in the player's file.
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

    // --- wheat ------------------------------------------------------------
    let c = panel(&mut game, wheat);
    assert_eq!(find_heading(&c, &assets, "Farmland", t), Some((0x28, y(5, 0x40))), "heading");
    let mode = find_heading(&c, &assets, "- Wheat.", t).expect("the mode follows the heading");
    assert!(mode.1 == y(5, 0x40) && mode.0 > 0x28, "on the heading's line and after it: {mode:?}");
    // `Ui_DrawCount(grain, 2, 0x128, row*16 + 0x68)` — the store's noun, on
    // the top line and right of the store's own column.
    let sack = find_body(&c, &assets, "Sack", t).expect("the store's noun");
    assert!(sack.1 == y(5, 0x68) && sack.0 > 0x128, "the store at (0x128, 0xB8): {sack:?}");
    // The England position faces Spring, so the grain report is the sowing one.
    assert_eq!(game.kingdom.season_next, 1);
    let sown = find_body(&c, &assets, "to be sown, yielding", t).expect("the sowing line");
    assert_eq!(sown.1, y(5, 0xC0));
    assert_eq!(find_body(&c, &assets, "in 4 seasons.", t).map(|p| p.1), Some(y(5, 0xD0)));
    assert_eq!(find_body(&c, &assets, "Change due to eating", t), Some((0x28, y(5, 0xE0))));
    assert_eq!(find_body(&c, &assets, "Overall change", t), Some((0x28, y(5, 0xF0))));
    // The table's wheat description is read and never drawn.
    assert!(find_body(&c, &assets, "This wheat field", t).is_none());
    assert!(find_text(&c, "TILE HALF", assets.ink.dim).is_none(), "and no placeholder");

    // --- cattle -----------------------------------------------------------
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

    // --- somebody else's --------------------------------------------------
    // The heading on the lower row the layout gives a foreign tile, and no
    // report: both report painters open with the owner test.
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

/// **The four resource sites say which site they are, how big it is and
/// whether it is working** — `TileInfo_Draw`'s `flags & 0x80` arm below graphic
/// `0x0D`. A player clicked a mine like he clicks a field and got nothing
/// back.
///
/// Every `y` is `row * 16 + k` with `row = 0x11` (`FUN_0041BEFE`'s
/// `if (g_pickedTileGraphic < 0xd) DAT_00553d2c = 0x11`) and every `k` and `x`
/// is a literal of `TileInfo_Draw` — `0x28`/`0x40` heading, `0x68`/`100` body,
/// `0x68`/`0x74` the status line — not a constant of `screens/info.rs`. The
/// words come out of the player's `L2.eng` and are asserted to be the ones the
/// indices name, so an index off by one fails on the word.
///
/// **The band is size, not fertility**, which is the correction this arm
/// carried: the boundaries are walked at 9/10, 24/25 and 49/50 on
/// `industry.output` (the original's `total − totalSnapshot`), and
/// `disabled_seasons` overrides all four.
///
/// **Ablations, run:** deleting the `draw_resource_site` call in
/// `InfoScreen::draw` fails at *"Mine (iron)."*; dropping `+ site_band(site)`
/// from the body fails at *"A medium mine."*; keying the status line on
/// `disabled_seasons` instead of `enabled` fails at the operational-and-
/// destroyed pair; swapping `SITE_INFO`'s iron and stone rows fails at the
/// heading naming the commodity.
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

    // The indices name these words in the player's file — the four headings,
    // one full band ladder, and the two status lines.
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

    // **`TileInfo_Draw`'s ladder reaches `0x80` only after six other bits have
    // also carried road or rough would take two different arms of two painters.
    // Asserted.
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

    // One site of each commodity — **and not all four are in one county.**
    // Every county in the England position carries a blacksmith (graphic 7), a
    // lumber mill (10 or 11) and *either* a mine (1) *or* a quarry (4), never
    // both; county 5 has neither. So the four are picked wherever they are, and
    // the band walk below uses the player's own.
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
    // The player's own mine, for the band walk and the ownership contrast.
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

    // --- the four headings, each over its own body and status line ---------
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

    // --- the size band, walked across all three boundaries ----------------
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

    // --- destroyed bypasses the buckets, and is NOT the status line --------
    // `disabledSeasons` picks the body; `enabled` picks the tail. Two bytes, so
// a trampled site reads operational when switched off
    // destroyed at once — the original's, reproduced.
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

    // --- and the switch, which is the other byte --------------------------
    game.kingdom.counties[county].industry[Commodity::Iron.index()].enabled = false;
    let canvas = panel(&mut game, mine);
    assert_eq!(
        find_body(&canvas, &assets, "This industry is shut down.", t),
        Some((0x68, y(0x74))),
    );

    // --- no ownership gate ------------------------------------------------
    // Both sides of `g_localPlayer == g_pickedCountyOwner` are the identical
// call in this arm, so a rival's mine says as much as yours.
    let other = (1..game.kingdom.realms.len() as u8).find(|&r| r != game.player).expect("a rival");
    game.kingdom.counties[county].owner = other;
    let canvas = panel(&mut game, mine);
    assert_eq!(find_heading(&canvas, &assets, "Mine (iron).", t), Some((0x28, y(0x40))));
    assert!(find_body(&canvas, &assets, "A destroyed mine.", t).is_some(), "a rival's mine too");
}

/// **`DAT_004D2EC8`, against the image**, row for row. The transcription in
/// `screens::info::FARM_TILE_INFO` is two artefacts one person maintains; the
/// player's `Lords2.exe` is not. Ablation: changing any one number of the table
/// fails here naming the row.
#[test]
fn the_farmland_table_is_the_images_own() {
    let exe = l2_testkit::executable!();
    let u32_at = |o: usize| u32::from_le_bytes(exe[o..o + 4].try_into().expect("four bytes"));
    let u16_at = |o: usize| u16::from_le_bytes(exe[o..o + 2].try_into().expect("two bytes"));
    let pe = u32_at(0x3C) as usize;
    let sections = u16_at(pe + 6) as usize;
    let table = pe + 24 + u16_at(pe + 20) as usize;
    let file_offset = |va: u32| -> usize {
        let rva = va - 0x0040_0000;
        for i in 0..sections {
            let s = table + i * 40;
            let (vsize, vaddr, raw) = (u32_at(s + 8), u32_at(s + 12), u32_at(s + 20));
            if rva >= vaddr && rva < vaddr + vsize {
                return (raw + (rva - vaddr)) as usize;
            }
        }
        panic!("{va:#X} is in no section");
    };
    let base = file_offset(0x004D_2EC8);
    for (row, want) in l2_game::screens::info::FARM_TILE_INFO.iter().enumerate() {
        let got: Vec<usize> = (0..4).map(|col| u32_at(base + row * 16 + col * 4) as usize).collect();
        assert_eq!(&got[..], &want[..], "DAT_004D2EC8 row {row:#04X}");
    }
}

/// **The debug overlay is off by default, and Ctrl+D draws it.**
///
/// Two players' reports: *"these debug squares still on the town square on map
/// and the fields"* and *"debug outlines and text for the 4 icons at the bottom
/// right"*. The original draws nothing at either place — `Sprite_TopIt` puts
/// the banner on the town's quadrant 0 and a herd on a pasture and nothing else,
/// and `Sidebar_ButtonClicked` is a hit test that draws nothing — so by default
/// neither may be on the canvas.
///
/// **Every absence is asserted at a place the overlay really draws on screen**:
/// the same frame with the overlay on must show our square at that pixel, or the
/// check is skipped for that tile — and at least one of each must remain, which
/// is what stops the absence being vacuous (`docs/decisions.md` C138).
///
/// The square's shape is the literal one — a filled `(2h+1)²` block inside a
/// one-pixel ring of the background — with `h` 2 for a county and 3 for a field.
///
/// **Ablations, run:** deleting `.filter(|_| debug)` on the county markers fails
/// the first absence; deleting `debug &&` on the field markers the second;
/// deleting the `, true` of the sidebar's focus guard the third; deleting the
/// Ctrl+D arm in `Machine::handle` the first *presence*.
#[test]
fn the_debug_overlay_is_off_by_default_and_ctrl_d_draws_it() {
    let (mut game, assets) = world!();
    assert!(!game.prefs.debug_overlay, "a normal session starts with it off");
    // `open_on_the_player` centres on the selected county when it is the
    // player's, so the county is selected *before* anything is drawn and the
    // ruler is drawn after it.
    let players: Vec<u8> =
        (1..=game.kingdom.county_count as u8).filter(|&id| game.is_players(id)).collect();
    let county = players
        .into_iter()
        .find(|&id| {
            game.select(id);
            visible_field(&mut game, &assets, id, |_| true).is_some()
        })
        .expect("one of the player's counties has a field in view once selected");
    game.select(county);

    let mut m = Machine::new(ScreenId::Campaign);
    draw_stack(&mut m, &mut game, &assets);
    let mut probe = MapScreen::new();
    draw(&mut probe, &mut game, &assets);
    let clip = probe.map_clip();
    // The pointer over a sidebar icon, which is what drew our outline.
    let b = map::SIDEBAR_BUTTONS[1].rect();
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: b.x + b.w / 2, y: b.y + b.h / 2 });
    let off = draw_stack(&mut m, &mut game, &assets);

    send_stack(&mut m, &mut game, &assets, Event::KeyDown(Key::CtrlChar('D')));
    assert!(game.prefs.debug_overlay, "Ctrl+D turns it on");
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "and no screen saw the key");
    let on = draw_stack(&mut m, &mut game, &assets);

    let background = assets.ink.background;
    let square = |c: &Canvas, (cx, cy): (i32, i32), h: i32| -> bool {
        let at = |x: i32, y: i32| c.at(x as usize, y as usize);
        let ink = at(cx, cy);
        ink != background
            && (-h..=h).all(|dy| (-h..=h).all(|dx| at(cx + dx, cy + dy) == ink))
            && (-h - 1..=h + 1).all(|d| {
                at(cx + d, cy - h - 1) == background
                    && at(cx + d, cy + h + 1) == background
                    && at(cx - h - 1, cy + d) == background
                    && at(cx + h + 1, cy + d) == background
            })
    };
    let inside = |(x, y): (i32, i32), r: i32| clip.contains(x - r, y - r) && clip.contains(x + r, y + r);

    // 1. The county squares, at each county's anchor tile.
    let anchors: Vec<(i32, i32)> = game
        .kingdom
        .county_ids()
        .filter_map(|id| {
            let (ax, ay) = (game.anchor_x[id] as usize, game.anchor_y[id] as usize);
            campaign::tile_centre(probe.viewport(), probe.zoom(), ax, ay)
        })
        .filter(|&p| inside(p, 3) && square(&on, p, 2))
        .collect();
    assert!(!anchors.is_empty(), "with the overlay on, a county square is on screen");
    for &p in &anchors {
        assert!(!square(&off, p, 2), "a county square at {p:?} with the overlay off");
    }

    // 2. The field squares, on the selected county's fields.
    let fields: Vec<(i32, i32)> = game
        .kingdom
        .field_tiles(county as usize)
        .into_iter()
        .filter_map(|(t, _)| {
            let (tx, ty) = l2_kingdom::map::coords(t);
            campaign::tile_centre(probe.viewport(), probe.zoom(), tx as usize, ty as usize)
        })
        .filter(|&p| inside(p, 4) && square(&on, p, 3))
        .collect();
    assert!(!fields.is_empty(), "with the overlay on, a field square is on screen");
    for &p in &fields {
        assert!(!square(&off, p, 3), "a field square at {p:?} with the overlay off");
    }

    // 3. The sidebar icon's outline, all four edges in the highlight.
    let hl = assets.ink.highlight;
    let outlined = |c: &Canvas| {
        let at = |x: i32, y: i32| c.at(x as usize, y as usize);
        (b.x..b.x + b.w).all(|x| at(x, b.y) == hl && at(x, b.y + b.h - 1) == hl)
            && (b.y..b.y + b.h).all(|y| at(b.x, y) == hl && at(b.x + b.w - 1, y) == hl)
    };
    assert!(outlined(&on), "the overlay outlines the icon under the pointer");
    assert!(!outlined(&off), "and nothing outlines it by default");

    // 4. Our words under the menu bar.
    assert!(find_text(&on, "TURN 1", assets.ink.dim).is_some());
    assert!(find_text(&off, "TURN 1", assets.ink.dim).is_none());

    // And off again is the default picture to the pixel.
    send_stack(&mut m, &mut game, &assets, Event::KeyDown(Key::CtrlChar('D')));
    assert!(!game.prefs.debug_overlay);
    let again = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(again.diff_count(&off), 0, "Ctrl+D twice leaves exactly the default frame");
}

/// **A click on a county that is not yours paints nothing**, which is the owner
/// test `Map_Click` makes before it reaches any of the three hotspots.
#[test]
fn another_lords_fields_are_not_yours_to_paint() {
    let (mut game, assets) = world!();
    let theirs = (1..=game.kingdom.county_count as u8)
        .find(|&id| !game.is_players(id) && game.kingdom.counties[id as usize].owner != 0)
        .expect("somebody else holds a county");
    let (tile, _) = game.kingdom.field_tiles(theirs as usize)[0];
    let before = game.kingdom.counties[theirs as usize].clone();

    let mut screen = MapScreen::new();
    let (x, y) = on_screen(&mut screen, tile);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    send(&mut screen, &mut game, &assets, Event::Click { x: 328, y: 208 });
    assert_eq!(game.kingdom.counties[theirs as usize], before);
}

/// **A click on one of your own buildings switches its industry.**
///
/// `Map_Click`'s plane-0 dispatch tests bit `0x80` before farmland, and the
/// industry comes from a ladder on the tile's terrain byte. The England
/// position gives every county one iron site, one stone, one weapons and one
/// wood, so a click on each is a click on a different industry.
#[test]
fn clicking_a_mine_switches_that_industry_off_and_on_again() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let map = &game.kingdom.campaign.map;
    let sites: Vec<(usize, l2_kingdom::industry::MapToggle)> = (0..map.terrain.len())
        .filter(|&t| {
            map.county[t] == county && map.flags[t] & l2_kingdom::map::flags::SETTLEMENT != 0
        })
        .filter_map(|t| l2_kingdom::industry::map_toggle_for_graphic(map.terrain[t]).map(|w| (t, w)))
        .collect();
    assert!(sites.len() >= 4, "one site per industry: {sites:?}");

    let mut screen = MapScreen::new();
    for (tile, what) in sites {
        let l2_kingdom::industry::MapToggle::Industry(c) = what else { continue };
        let slot = c.index();
        let before = game.kingdom.counties[county as usize].industry[slot].enabled;
        let (x, y) = on_screen(&mut screen, tile);
        send(&mut screen, &mut game, &assets, Event::Click { x, y });
        assert_ne!(
            game.kingdom.counties[county as usize].industry[slot].enabled, before,
            "{c:?} at tile {tile} did not switch"
        );
        send(&mut screen, &mut game, &assets, Event::Click { x, y });
        assert_eq!(
            game.kingdom.counties[county as usize].industry[slot].enabled, before,
            "{c:?} did not switch back"
        );
    }
}

/// **A field shows its crop, and the picture comes from the terrain byte.**
///
/// `Terrain_Set` (`0x0046D7F4`) is the game's single writer of a tile's
/// `content` byte and it chooses the frame in the same statement; until this
/// was drawn, `l2-game`'s field brush painted markers of ours and every field
/// on the map looked like the bare frame 80 the file stores.
///
/// The check is on the **ladder**, at the pixel: the four states this walks
/// through are four different pictures, and each is the four-frame block
/// [`campaign::field_base`] names.
#[test]
fn a_fields_picture_follows_its_crop_state() {
    let (mut game, assets) = world!();
    let field = {
        let map = &game.kingdom.campaign.map;
        (0..map.terrain.len())
            .find(|&t| map.flags[t] & l2_kingdom::map::flags::FARMLAND != 0)
            .expect("England has fields")
    };
    let (fx, fy) = l2_kingdom::map::coords(field);
    let stored = assets
        .slot(game.map_slot)
        .expect("the map slot")
        .at(l2_formats::maps::Plane::GfxIndex, fx as usize, fy as usize);

    // Every state in the ladder gives a frame in its own block, and the tile's
    // own variation — the two low bits the file stored — never moves.
    //
    // **`0x05` used to be in this list and it was asserting a falsehood.** The
    // list is now the values the game writes to a farm tile.
    // `Terrain_Set`'s twenty-four call sites pass `0`, `1`, `2 … 0x0E` through
    // `FUN_00469D21`, `0x13 … 0x16`, `0x17`, `0x18` and `0x19 … 0x1C` — and the
    // crop states are handled by their own claim below, because they are the
    // one place `Terrain_Set`'s third parameter is not zero.
    let variation = stored & 3;
    for terrain in [0x00u8, 0x01, 0x02, 0x14, 0x17, 0x18, 0x19, 0x1C, 0x1F] {
        let (bank, frame) = campaign::field_graphic(terrain, stored);
        let (base, layer) = campaign::field_base(terrain);
        assert_eq!(frame, base + variation, "terrain {terrain:#04X} keeps its variation");
        assert_eq!(bank & campaign::BANK_MASK, layer, "terrain {terrain:#04X} bank layer");
    }

    // **The wheat grows, and the variant is the only thing that says so.**
    //
    // A player: *"The wheat fields don't show the wheat growing."*
    // `Grain_SeasonTick` writes the crop's density band onto every grain tile —
    // `FUN_0044CF6F` returns **2, 3, 7 or 11** and nothing else — and derives
    // `Terrain_Set`'s variant from it as `band < 3 ? 0 : (band - 3) / 4 + 1`.
    // All four bands share base 88, so `base + variation` is the *same picture*
    // at every stage: without the variant term the field is drawn just-sown all
    // year. `docs/formats/maps-layers.md` §5.5 called that parameter dead, and
    // this is the assertion that says otherwise.
    let bands = [2u8, 3, 7, 11];
    let mut frames = Vec::new();
    for (n, band) in bands.iter().enumerate() {
        assert_eq!(
            campaign::field_base(*band).0,
            88,
            "every crop band shares base 88, which is why the variant is load-bearing",
        );
        assert_eq!(campaign::field_variant(*band), n as u8, "band {band} is variant {n}");
        let frame = campaign::field_frame(*band, stored);
        assert_eq!(frame, 88 + variation + 4 * n as u8);
        frames.push(frame);
    }
    frames.sort_unstable();
    frames.dedup();
    assert_eq!(frames.len(), 4, "the four crop bands are four different pictures");
    // And the run ends where the next base begins: 88 + 4 blocks of 4 = 104.
    assert_eq!(campaign::field_base(0x13).0, 104);
    // Harvested stubble is in the **base** bank and everything else is in
    // roads — the one place the ladder crosses banks.
    assert_eq!(campaign::field_base(0x17).1, campaign::BANK_BASE);
    assert_eq!(campaign::field_base(0x16).1, campaign::BANK_ROADS);

    // And it reaches the picture. Paint the same viewport with the field in
    // four different states and require four different pictures.
    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let (cx, cy) =
        campaign::tile_centre(screen.viewport(), screen.zoom(), fx as usize, fy as usize)
            .expect("the field is centred, so it is in view");
    let mut seen: Vec<Vec<u8>> = Vec::new();
    for terrain in [0x00u8, 0x01, 0x0A, 0x14] {
        game.kingdom.campaign.map.terrain[field] = terrain;
        let overrides = {
            let ctx = Ctx { game: &mut game, assets: &assets };
            MapScreen::tile_graphics(&ctx)
        };
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
            &overrides,
            game.kingdom.season,
            None,
        );
        // Just the tile, so a neighbouring field's state cannot carry the test.
        let mut patch = Vec::new();
        for y in cy - 10..cy + 10 {
            for x in cx - 20..cx + 20 {
                patch.push(canvas.at(x as usize, y as usize));
            }
        }
        assert!(
            !seen.contains(&patch),
            "terrain {terrain:#04X} draws the same picture as an earlier state"
        );
        seen.push(patch);
    }
    assert_eq!(seen.len(), 4, "wild, fallow, grain and pasture are four pictures");
}

// ------------------------------- the rest of `TileInfo_Draw`'s group-30 ladder

