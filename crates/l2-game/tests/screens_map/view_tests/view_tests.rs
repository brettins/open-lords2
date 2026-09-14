#![allow(unused_imports)]
use super::*;

use super::*;
use super::interaction_tests::*;
use super::structures_tests::*;
use super::fog_and_march_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

/// **The screen the original draws is a window, not the whole map.**
///
/// This is the test the previous painter could not have passed: it drew all
/// 4,096 tiles at once, so every one of the fourteen counties was on screen at
/// once. `Map_SetZoom` gives the near view eight of the lattice's 65 columns,
/// so only a few counties can be — and the ones that are fill it.
#[test]
fn the_near_view_is_a_window_of_england_and_not_the_whole_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);

    // Real artwork, not a flat fill: the shipped banks use a lot of the palette.
    let mut used = [false; 256];
    for &p in &canvas.pixels {
        used[p as usize] = true;
    }
    let distinct = used.iter().filter(|u| **u).count();
    assert!(distinct > 32, "only {distinct} palette entries in the whole frame");

    // The opening viewport is fixed — `Map_InitMode`'s row 0x4A, col 0x14,
    // then `Game_SetupRealmsAndCounties`'s centre on the player's own town
    // (C48, see [`the_map_opens_on_the_players_own_county`]) — so this is a
    // **Two** of England's fourteen counties are on
    // screen when the game opens.
    let counts = pick_counts(&screen);
    assert_eq!(visible_counties(&screen), 2, "eight lattice columns hold two counties, not 14");
    for (id, n) in counts.iter().enumerate().skip(15) {
        assert_eq!(*n, 0, "there is no county {id} on this map");
    }

    // Nothing outside the viewport is pickable, whatever the tag plane holds.
    assert_eq!(screen.county_at(map::PANEL.x, 200), 0, "the panel is not the map");
    assert_eq!(screen.county_at(200, map::TOP_BAR - 1), 0, "nor is the menu bar");
    assert_eq!(screen.county_at(200, 474), 0, "nor below the near viewport");
}

/// **The map opens where the player's own county is, and an army raised there
/// is on the screen.** Corrections C47 and C48.
///
/// A player reported *"I raised an army and nothing appeared on the map"*, and
/// two separate faults each put his army out of shot on the England fixture:
///
/// * **C48** — we stopped at `Map_InitMode`'s row `0x4A` / column `0x14`, and
///   the original does not: `Game_SetupRealmsAndCounties` (`0x0049BD99`) ends
///   with `FUN_00432746(g_playerStartTable[g_localPlayer * 2])`, which centres
///   on the player's own town. County 8's town is fourteen lattice columns
///   outside the eight the near view holds, so the player opened the game
///   looking at somebody else's country.
/// * **C47** — `muster_tile` scanned the whole map for the county's lowest
///   free road tile. `County_FindFreeRoadTile` (`0x00428007`) searches a box of
///   radius 1, 2 then 3 **around the county's anchor**, so the original never
///   puts a new army more than three tiles from the county's centre.
///
/// Both are measured here: the town has a pixel, the
/// army's tile is within three of the anchor and has a pixel, and removing the
/// unit changes that many pixels and no others.
#[test]
fn the_map_opens_on_the_players_own_county_and_a_raised_army_is_in_shot() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // The town the original centres on is on screen, and so is the anchor the
    // muster searches around.
    let anchor = {
        let c = &game.kingdom.counties[county as usize];
        (c.anchor_x, c.anchor_y)
    };
    assert!(
        l2_view::campaign::tile_centre(
            screen.viewport(),
            screen.zoom(),
            anchor.0 as usize,
            anchor.1 as usize
        )
        .is_some(),
        "the county the game opens on has to be in the viewport it opens at",
    );
    assert!(
        pick_counts(&screen)[county as usize] > 0,
        "and the pick plane agrees the player's county is what he is looking at",
    );

    // Raise an army the way the raise screen does.
    let realm = game.kingdom.realms[game.player as usize].clone();
    let basket = l2_kingdom::LevyBasket::seed(&realm, 300);
    let id = game.raise_army(county, &basket, 10, None).expect("the county can raise one");
    let (ux, uy) = game.kingdom.campaign.units.get(id).map(|u| (u.x, u.y)).expect("the army");
    assert!(
        (ux as i32 - anchor.0 as i32).abs() <= 3 && (uy as i32 - anchor.1 as i32).abs() <= 3,
        "C47: ({ux}, {uy}) is more than three tiles from the anchor {anchor:?}",
    );

    let at = l2_view::campaign::tile_centre(
        screen.viewport(),
        screen.zoom(),
        ux as usize,
        uy as usize,
    );
    let (cx, cy) = at.expect("an army raised in the county the map is centred on is in shot");

    // And it is *drawn*: taking the unit away changes pixels, all of them
    // around the tile the unit stands on.
    let with = draw(&mut screen, &mut game, &assets);
    game.kingdom.campaign.units.remove(id);
    let without = draw(&mut screen, &mut game, &assets);
    let moved: Vec<(i32, i32)> = with
        .pixels
        .iter()
        .zip(without.pixels.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| ((i % with.width) as i32, (i / with.width) as i32))
        .collect();
    assert!(!moved.is_empty(), "the army painted nothing at all");
    // `Map_DrawArmies` anchors the figure's **bottom centre** on the tile's
    // bottom vertex — `tileOrigin + (halfPitch, halfPitch)`, which at the near
    // zoom is `tileCentre + (1, 15)` — and the army frames are 53 x 44. So the
    // ink hangs upwards from just below the tile centre, and this box is that
    // rectangle with a pixel of slack.
    let frame = (53, 44);
    for (x, y) in &moved {
        assert!(
            (x - cx).abs() <= frame.0 / 2 + 2
                && *y <= cy + campaign::NEAR.tile_h / 2 + 1
                && *y >= cy + campaign::NEAR.tile_h / 2 - frame.1 - 4,
            "the army's ink is at ({x}, {y}), nowhere near its tile ({cx}, {cy})",
        );
    }
}

/// **The county town flies a waving flag in its owner's colours.** C49.
///
/// The player: *"each county's town square would have a coloured flag waving on
/// it."* `FUN_004071A0` draws it from `Flags1a.pl8` at frame
/// `(shield − 1) * 8 + phase`, placed at `tileOrigin + (0x1A, −0x1C)` with no
/// centring, and the phase is a global counter mod `0x80` shifted right by four
/// — eight frames, 16 ms apiece, 2.05 s a wave.
///
/// All three halves are measured: the flag paints, it paints **inside the
/// 32 × 24 rectangle that offset names** and nowhere else, and advancing the
/// phase changes the picture.
#[test]
fn the_county_town_flies_its_owners_flag_and_it_waves() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    let with = draw(&mut screen, &mut game, &assets);
    // The original's own guard: `shieldIndex` is clamped 1..5 and a zero flies
    // nothing. Taking every realm's shield away is therefore the same picture
    // with the flags removed, and nothing else moved.
    for r in game.kingdom.realms.iter_mut() {
        r.shield_index = 0;
    }
    let without = draw(&mut screen, &mut game, &assets);
    let moved: Vec<(i32, i32)> = with
        .pixels
        .iter()
        .zip(without.pixels.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| ((i % with.width) as i32, (i / with.width) as i32))
        .collect();
    assert!(!moved.is_empty(), "no county on screen flew a flag");

    // Every changed pixel has to lie in one of the flag rectangles: the town
    // block's north-west tile, offset by `flag_at`, 32 x 24.
    let mut boxes: Vec<(i32, i32)> = Vec::new();
    for id in game.kingdom.county_ids() {
        let ctx = Ctx { game: &mut game, assets: &assets };
        let Some(&tile) = MapScreen::town(&ctx, id as u8).first() else { continue };
        let (tx, ty) = l2_kingdom::map::coords(tile);
        let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
        let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
        boxes.push((sx + campaign::NEAR.flag_at.0, sy + campaign::NEAR.flag_at.1));
    }
    for (x, y) in &moved {
        assert!(
            boxes
                .iter()
                .any(|(bx, by)| (bx..&(bx + 32)).contains(&x) && (by..&(by + 24)).contains(&y)),
            "flag ink at ({x}, {y}) is outside every 32 x 24 flag rectangle",
        );
    }

    // And it waves: one full phase of ticks repaints it. `flag_tick` runs
    // 0..0x7F and the phase is `tick >> 4`, so sixteen ticks is one frame.
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let a = draw(&mut screen, &mut game, &assets);
    let mut moved_by_the_wave = 0;
    for _ in 0..16 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        Screen::update(&mut screen, &mut ctx);
    }
    let b = draw(&mut screen, &mut game, &assets);
    moved_by_the_wave += a.diff_count(&b);
    assert!(moved_by_the_wave > 0, "sixteen ticks must advance the wave by one frame");
    let _ = county;
}

/// **The mercenary band standing in the town square** — `Sprite_TopIt`'s second
/// town arm, which we drew on the wrong tile at the wrong offset.
///
/// A player: *"I haven't seen any mercenary icons on the town square yet."*
/// Both halves of why were geometry:
///
/// * **The tile.** The arm fires on `(tile.part & 0xf) == 2`, and plane 3 is
///   `dx + W * dy` from the block's north-west corner, so for a 2 × 2 town that
///   is `(x, y + 1)` — the **third** tile in index order. We drew on the second,
///   `(x + 1, y)`, which is `part == 1` — and `County_FindTownTile` sets bank
///   bit `0x80`, the only gate on the overlay pass running at all, on the **0th
/// and the 2nd** tiles it meets in index order. The original never so much as
///   visits the tile we painted.
/// * **The offset.** `(+0x10, −0x12)` at the near zoom, not the banner's
///   `(+0x1A, −0x1C)`. Both literals below are read out of the decompilation and
///   **no expression in this test mentions `Zoom::mercenary_at`**, which is
///   `docs/agents.md`'s first way to ablate wrongly.
///
/// The assertion is a **set**: hand one county a band, diff the two renders, and
/// require every moved pixel to be one the marker itself would have written had
/// it been blitted alone at the place the two literals name.
#[test]
fn a_mercenary_band_stands_on_the_town_blocks_third_tile_and_nowhere_else() {
    let (mut game, assets) = world!();
    // `Mercenary_AdvanceAll` refreshes the offers once a season and the England
    // fixture is turn one, so nothing here has a band yet. That is also the
    // likeliest reason nobody had seen the marker in a short session, and is
    // This test writes the byte.
    for c in game.kingdom.counties.iter_mut() {
        c.mercenary_offer = 0;
    }
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let town: Vec<usize> = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        MapScreen::town(&ctx, county)
    };
    assert_eq!(town.len(), 4, "a county town is a 2 x 2 block");
    let (ox, oy) = l2_kingdom::map::coords(town[0]);
    // Put the whole block in shot.
    // hold it.
    screen.centre_on_tile(ox as usize, oy as usize);
    draw(&mut screen, &mut game, &assets);
    let without = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        l2_kingdom::map::coords(town[2]),
        (ox, oy + 1),
        "index order puts part 2 - (x, y + 1) - third",
    );

    game.kingdom.counties[county as usize].mercenary_offer = 3;
    let with = draw(&mut screen, &mut game, &assets);

    // Where the two literals say the marker goes.
    let (row, col) = campaign::tile_to_cell(ox as usize, oy as usize + 1);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let mut reference = without.clone();
    let frame = assets
        .map
        .flag_sheet(screen.zoom())
        .and_then(|s| s.frame(campaign::MERCENARY_MARKER_FRAME))
        .expect("Flags1a.pl8 frame 0x81");
    reference.blit_clipped(&frame, sx + 0x10, sy - 0x12, screen.map_clip());

    let moved = |a: &Canvas, b: &Canvas| -> Vec<usize> {
        a.pixels
            .iter()
            .zip(b.pixels.iter())
            .enumerate()
            .filter(|(_, (p, q))| p != q)
            .map(|(i, _)| i)
            .collect()
    };
    // A set of 640 x 480 indices is unreadable in a failure; its bounding box
    // and its size say where the marker went and are what a reader needs.
    let corner = |v: &[usize]| -> (usize, usize, usize, usize, usize) {
        let xs = v.iter().map(|i| i % with.width);
        let ys = v.iter().map(|i| i / with.width);
        (
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
            v.len(),
        )
    };
    let drawn = moved(&with, &without);
    let expected = moved(&reference, &without);
    assert!(!expected.is_empty(), "frame 0x81 writes nothing at the place the literals name");
    assert_eq!(
        corner(&drawn),
        corner(&expected),
        "the marker's ink (x0, y0, x1, y1, count) is not the 25 x 45 band frame 0x81 writes at \
         ({}, {}) — the tile is town()[2] and the offset is (+0x10, -0x12)",
        sx + 0x10,
        sy - 0x12,
    );
    assert_eq!(drawn, expected, "the marker's ink is the right size in the right place and is not the same pixels");

    // And it is gone again when the offer is.
    game.kingdom.counties[county as usize].mercenary_offer = 0;
    let gone = draw(&mut screen, &mut game, &assets);
    assert_eq!(gone.diff_count(&without), 0, "no offer, no band");
}

/// **A merchant is drawn, and clicking one opens the merchant.** C50.
///
/// The player: *"I don't see the merchants on the map, I can't
/// click them."* Both halves were true. The figure was a square marker in
/// `ink.dim`, because a merchant's owner byte is **6** and `Ink::realm` has six
/// entries — and the click fell into `NOT YOUR UNIT` for the same reason.
///
/// `Map_Click`'s merchant arm never reads the unit's owner. Its guard is
/// `g_counties[pickedCounty].owner == g_localPlayer`, so the question is whose
/// **county** the merchant is standing in, and this asserts it both ways.
#[test]
fn a_merchant_is_drawn_and_opens_the_merchant_screen_from_the_county_it_is_in() {
    let (mut game, assets) = world!();
    let mine = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let (merchant, _) = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, u)| (id, u.owner))
        .expect("the fixture ships six merchants");
    assert_eq!(
        game.kingdom.campaign.units.get(merchant).map(|u| u.owner),
        Some(6),
        "every merchant in the game is ownerless, which is why the guard cannot be its owner",
    );

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // Stand it on a tile of the player's own county that is in shot. The
    // county's anchor is in shot because the map opened on it (C48).
    let (ax, ay) = {
        let c = &game.kingdom.counties[mine as usize];
        (c.anchor_x, c.anchor_y)
    };
    {
        let u = game.kingdom.campaign.units.get_mut(merchant).expect("the merchant");
        u.x = ax;
        u.y = ay;
        u.county = mine;
    }
    let (cx, cy) =
        campaign::tile_centre(screen.viewport(), screen.zoom(), ax as usize, ay as usize)
            .expect("the anchor is in shot");

    // It paints. `Sprite1a.pl8` frames 0 … 47 are the merchant's, 40 x 32.
    let with = draw(&mut screen, &mut game, &assets);
    let put_back = game.kingdom.campaign.units.remove(merchant).expect("the merchant");
    let without = draw(&mut screen, &mut game, &assets);
    assert!(with.diff_count(&without) > 0, "the merchant painted nothing");
    game.kingdom.campaign.units.put(merchant, put_back);

    // And clicking it opens screen 0x08 **carrying the unit**, because the
    // price is that merchant's own morale — `DAT_00553C64`.
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Merchant(merchant)), "the merchant screen");

    // The same merchant in somebody else's county is a refusal, not a trade.
    let theirs = (1..=game.kingdom.county_count as u8)
        .find(|&id| id != mine && !game.is_players(id))
        .expect("England has counties the player does not own");
    game.kingdom.campaign.units.get_mut(merchant).expect("the merchant").county = theirs;
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Stay, "a merchant in a county you do not own opens nothing");
}

/// **A merchant is clickable across its whole tile and its whole figure, and a
/// different county being selected changes nothing.**
///
/// The player: *"there seems to be some weird thing where a certain county is
/// 'selected', and if I click a merchant while the map has a different county
/// selected it will open up the tax window."* The selection was innocent. The
/// hit test asked the unit's nine-pixel *marker* box while the figure drawn is
/// 40 × 32 on a 58 × 30 tile — so most clicks on a merchant missed the unit arm
/// entirely and fell through to our own "a second click on the selected county
/// opens its panel", which is the tax window.
///
/// `Map_ResolvePick` (`0x0046D5FE`) has no such problem: `g_pickedTileUnit =
/// g_tiles[t].unit`, so the whole tile is the merchant. This sweeps both — the
/// tile's diamond and the figure's opaque pixels — with an unrelated county
/// selected throughout, and requires every one of them to reach the merchant.
#[test]
fn a_merchant_is_clickable_over_its_whole_tile_whichever_county_is_selected() {
    let (mut game, assets) = world!();
    let mine = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let elsewhere = (1..=game.kingdom.county_count as u8)
        .find(|&id| id != mine)
        .expect("England has more than one county");
    let merchant = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let (ax, ay) = {
        let c = &game.kingdom.counties[mine as usize];
        (c.anchor_x, c.anchor_y)
    };
    {
        let u = game.kingdom.campaign.units.get_mut(merchant).expect("the merchant");
        u.x = ax;
        u.y = ay;
        u.county = mine;
    }
    // **A different county is selected for the whole sweep.** That is the
    // player's condition, and it must make no difference.
    game.select(elsewhere);
    draw(&mut screen, &mut game, &assets);

    let (cx, cy) =
        campaign::tile_centre(screen.viewport(), screen.zoom(), ax as usize, ay as usize)
            .expect("the anchor is in shot");
    let zoom = *screen.zoom();
    let (hw, hh) = (zoom.tile_w / 2, zoom.tile_h / 2);

    let hits = |screen: &MapScreen, game: &mut Game, pts: &[(i32, i32)]| {
        let mut ok = 0;
        for &(x, y) in pts {
            let ctx = Ctx { game, assets: &assets };
            if screen.unit_at(&ctx, x, y) == Some(merchant) {
                ok += 1;
            }
        }
        ok
    };

    // Half the diamond's rows, on its centre line and near its two side
    // vertices — points the old marker box could not reach.
    let mut ground = Vec::new();
    for dy in -hh + 2..hh - 1 {
        let span = hw - (dy.abs() * hw) / hh;
        for dx in [-span + 2, 0, span - 2] {
            ground.push((cx + dx, cy + dy));
        }
    }
    assert!(ground.len() > 60, "the diamond is 58 x 30 and this samples it");
    assert_eq!(
        hits(&screen, &mut game, &ground),
        ground.len(),
        "every pixel of the merchant's own tile is the merchant"
    );

    // And the figure, which stands up over the tiles behind its own.
    let sprite = {
        let u = game.kingdom.campaign.units.get(merchant).expect("the merchant");
        campaign::UnitSprite {
            sheet: u.sprite_sheet(),
            frame: game.unit_frame(merchant, u),
            nudge: u.sprite_nudge(),
            walk: campaign::walk_offset(&zoom, u.facing, u.sub_tile),
        }
    };
    let rect = campaign::unit_sprite_rect(
        &assets.map,
        screen.viewport(),
        &zoom,
        (ax as usize, ay as usize),
        sprite,
    );
    if let Some((ox, oy, art)) = rect {
        assert!(art.height as i32 > zoom.tile_h, "the figure is taller than its tile");
        let mut figure = Vec::new();
        for dy in 0..art.height as i32 {
            for dx in 0..art.width as i32 {
                if art.opaque[dy as usize * art.width as usize + dx as usize] {
                    figure.push((ox + dx, oy + dy));
                }
            }
        }
        assert!(figure.len() > 200, "the merchant is a figure, not a dot");
        assert_eq!(
            hits(&screen, &mut game, &figure),
            figure.len(),
            "every painted pixel of the merchant is the merchant"
        );
    }

    // The selection is untouched by the sweep, and the click still trades.
    assert_eq!(game.selected, elsewhere, "hit-testing selects nothing");
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Merchant(merchant)), "and it opens the merchant");
}


