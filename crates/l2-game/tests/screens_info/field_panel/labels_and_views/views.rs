#![allow(unused_imports)]
use super::*;
use super::panel::*;
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




