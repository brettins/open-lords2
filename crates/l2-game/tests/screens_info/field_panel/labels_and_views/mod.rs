#![allow(unused_imports)]
use super::*;
use super::interaction::*;
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

/// the same frame with the overlay on must show our square at that pixel, or the
/// check is skipped for that tile — and at least one of each must remain, which
/// is what stops the absence being vacuous (`docs/decisions.md` C138).
#[test]
fn the_debug_overlay_is_off_by_default_and_ctrl_d_draws_it() {
    let (mut game, assets) = world!();
    assert!(!game.prefs.debug_overlay, "a normal session starts with it off");
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

    let hl = assets.ink.highlight;
    let outlined = |c: &Canvas| {
        let at = |x: i32, y: i32| c.at(x as usize, y as usize);
        (b.x..b.x + b.w).all(|x| at(x, b.y) == hl && at(x, b.y + b.h - 1) == hl)
            && (b.y..b.y + b.h).all(|y| at(b.x, y) == hl && at(b.x + b.w - 1, y) == hl)
    };
    assert!(outlined(&on), "the overlay outlines the icon under the pointer");
    assert!(!outlined(&off), "and nothing outlines it by default");

    assert!(find_text(&on, "TURN 1", assets.ink.dim).is_some());
    assert!(find_text(&off, "TURN 1", assets.ink.dim).is_none());

    send_stack(&mut m, &mut game, &assets, Event::KeyDown(Key::CtrlChar('D')));
    assert!(!game.prefs.debug_overlay);
    let again = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(again.diff_count(&off), 0, "Ctrl+D twice leaves exactly the default frame");
}

/// `Terrain_Set` (`0x0046D7F4`) is the game's single writer of a tile's
/// `content` byte and it chooses the frame in the same statement; until this
/// was drawn, `l2-game`'s field brush painted markers of ours and every field
/// on the map looked like the bare frame 80 the file stores.
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

    // A player: *"The wheat fields don't show the wheat growing."*
    // `Grain_SeasonTick` writes the crop's density band onto every grain tile —
    // `FUN_0044CF6F` returns **2, 3, 7 or 11** and nothing else — and derives
    // `Terrain_Set`'s variant from it as `band < 3 ? 0 : (band - 3) / 4 + 1`.
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
    assert_eq!(campaign::field_base(0x13).0, 104);
    assert_eq!(campaign::field_base(0x17).1, campaign::BANK_BASE);
    assert_eq!(campaign::field_base(0x16).1, campaign::BANK_ROADS);

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



