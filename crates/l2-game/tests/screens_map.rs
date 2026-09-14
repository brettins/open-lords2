//! The campaign map: the near view, its tiles, towns, castles, merchants, the minimap, the fog and a marching army.
//!
//! Split out of `tests/screens.rs`; the shared helpers are in `tests/common/`.

#[macro_use]
mod common;

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

/// Every pixel of the map **viewport**, and which county it belongs to.
///
/// The viewport is the zoom's, not the screen's: `docs/screens.md` §1.4 — x
/// stops at 478 where the right panel starts, and y at 474 (near) or 408 (far).
fn pick_counts(screen: &MapScreen) -> [usize; 17] {
    let mut counts = [0usize; 17];
    let clip = screen.map_clip();
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            let id = screen.county_at(x, y) as usize;
            if id < 17 {
                counts[id] += 1;
            }
        }
    }
    counts
}

/// Find a pixel belonging to a county, by scanning the pick plane.
/// hard-coding a coordinate a layout change would invalidate.
fn pixel_of(screen: &MapScreen, county: u8) -> Option<(i32, i32)> {
    let clip = screen.map_clip();
    (clip.y0..clip.y1)
        .flat_map(|y| (clip.x0..clip.x1).map(move |x| (x, y)))
        .find(|&(x, y)| screen.county_at(x, y) == county)
}

fn visible_counties(screen: &MapScreen) -> usize {
    pick_counts(screen)[1..=14].iter().filter(|&&c| c > 0).count()
}

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

/// Zooming out reaches the rest of the map, and scrolling moves the near view.
/// Both halves matter: a viewport that could not move would be the minimap the
/// user complained about, in a smaller rectangle.
#[test]
fn zooming_out_shows_more_of_the_map_and_scrolling_moves_the_near_view() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before = draw(&mut screen, &mut game, &assets);
    let near_visible = visible_counties(&screen);

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    let far = draw(&mut screen, &mut game, &assets);
    let far_visible = visible_counties(&screen);
    assert!(
        far_visible > near_visible,
        "the far view shows {far_visible} counties and the near one {near_visible}"
    );
    // At the far zoom's pinned origin all fourteen are reachable.
    // The original disables scrolling there.
    assert_eq!(far_visible, 14);
    assert!(before.diff_count(&far) > 10_000, "and it is a different picture");

    // Back in, and now scroll. One step is one map tile, so the origin moves by
    // exactly one lattice column and the picture must change.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    let home = screen.viewport();
    let a = draw(&mut screen, &mut game, &assets);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(screen.viewport().col, home.col + 1);
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 10_000, "scrolling one column must repaint the map");
}

/// **A click on a county's open ground does not select it. `Map_Click` has no
/// county-selection arm at all.**
///
/// This test has now been wrong twice, in opposite directions, and both times
/// the error was a reading of the same 1,263-byte function.
///
/// It first asserted that a second click on the selected county opened its tax
/// panel — our convenience, which a player reported: *"there's some weird thing
/// where if you click anywhere on grass it opens up the tax window too."* It
/// was then rewritten to assert that the click *selects and recentres*, on the
/// A "last arm" is not quoted into three documents.
/// The quoted code is the **prologue of the industry branch**, guarded by tile
/// flag `0x80` and by the county being the local player's, and `Map_Click`'s
/// three writes to `g_selectedCounty` are all inside branches that open
/// something: the village, an industry toggle, the merchant.
///
/// So selection from the map is a *side effect of arriving somewhere*, never a
/// verb of its own. `docs/decisions.md` C61.
#[test]
fn a_click_on_a_countys_open_ground_selects_nothing() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    let counts = pick_counts(&screen);
    let target = (1..=14u8).max_by_key(|&id| counts[id as usize]).expect("a county is visible");
    let (px, py) = pixel_of(&screen, target).expect("and it has a pixel");

    game.select(0);
    let before = game.kingdom.clone();
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Stay, "it opens nothing");
    assert_eq!(game.selected, 0, "and selects nothing");
    assert_eq!(game.kingdom, before, "and changes no part of the world");
}

/// **A click on plain ground changes nothing at all** — not the screen, not the
/// selection, not one byte of the kingdom.
///
/// This is the assertion that could not exist while we had a county-selection
/// arm that opened a panel, and it is the one that would have caught all three
/// of the hit-test defects a player found in a single evening: the mine's dead
/// upper half (C57), the merchant's nine-pixel box (C58) and `pick_tile`'s 56
/// dead pixels around every tile centre (C60). Every one of them was a
/// *geometric* shortfall, and every one became **the wrong screen opening**
/// A miss had somewhere to fall
/// through to. It asserts over the whole state,
/// because "nothing happened" is the claim.
#[test]
fn a_click_on_plain_ground_changes_nothing_at_all() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // The sea is the one thing on the map guaranteed to carry no county, no
    // unit and no flags.
    let (sx, sy) = pixel_of(&screen, 0).expect("there is sea");

    let selected_before = game.selected;
    let before = game.kingdom.clone();
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: sx, y: sy });

    assert_eq!(t, Transition::Stay, "a click on nothing opens nothing");
    assert_eq!(game.selected, selected_before, "and does not clear the selection either");
    assert_eq!(game.kingdom, before, "and changes no part of the world");
}

/// A click picks the county the player can see at that pixel. The
/// pixel is found through the pick plane, so this exercises exactly the path
/// the mouse takes.

/// **The county town is not four quarries.**
///
/// `L2_maps.dat` stores a town's 2 × 2 block as `Town1a.pl8` frames 0 … 3, and
/// those four frames are the *stone quarry* artwork — which is exactly how
/// `County_PlaceResourceSites` identifies a quarry (frame 0 stone, 20 wood, 30
/// iron). The original never shows them: `Counties_PlaceSites` re-stamps the
/// block to frames 47 … 50, 51 … 54 or 55 … 58 by the county's population, and
/// the population pass re-stamps it every season.
///
/// We rendered the stored bytes, so every town on the map came out as four
/// pits. This asserts the rewrite, and that it is population-banded.
#[test]
fn every_county_town_is_re_stamped_off_the_quarry_frames_and_onto_a_village() {
    let (mut game, assets) = world!();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let overrides = MapScreen::town_graphics(&ctx);
    assert!(!overrides.is_empty(), "fourteen towns were rewritten");

    let mut towns = 0;
    for id in ctx.game.kingdom.county_ids() {
        let tiles = MapScreen::town(&ctx, id as u8);
        assert_eq!(tiles.len(), 4, "county {id}'s town is a 2 x 2 block");
        let pop = ctx.game.kingdom.counties[id].population;
        let base: u8 = if pop < 801 {
            47
        } else if pop < 1201 {
            51
        } else {
            55
        };
        let mut frames = Vec::new();
        for tile in tiles {
            let (x, y) = l2_kingdom::map::coords(tile);
            let (bank, frame) = overrides.get(x as usize, y as usize).expect("a town tile");
            assert_eq!(bank, 0x0C, "the town stays in the Town1a bank");
            assert!(
                (base..base + 4).contains(&frame),
                "county {id} has {pop} people, so its town is frames {base}..{}; got {frame}",
                base + 4
            );
            frames.push(frame);
        }
        frames.sort_unstable();
        assert_eq!(frames, vec![base, base + 1, base + 2, base + 3], "one of each quadrant");
        towns += 1;
    }
    assert_eq!(towns, 14, "England has fourteen counties and fourteen towns");
}

/// And it reaches the picture: **the same viewport, painted twice** — once
/// through the override and once straight from the file — differs, and it
/// Differs by about the area of four tiles.
///
/// Both halves go through `campaign::draw` and nothing else, so nothing but the
/// tile frames can account for the difference. Reverting the rewrite turns this
/// test red.
#[test]
fn the_rewritten_town_actually_changes_what_is_drawn() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    // Put county 8's town — the player's — in the middle of the view.
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = *MapScreen::town(&ctx, 8).first().expect("county 8 has a town");
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let ctx = Ctx { game: &mut game, assets: &assets };
    let overrides = MapScreen::town_graphics(&ctx);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = l2_view::campaign::Lattice::build(&slot);
    let paint = |o: &l2_view::campaign::Overrides| {
        let mut canvas = Canvas::screen();
        let mut tags = l2_view::Tags::screen();
        l2_view::campaign::draw(
            &mut canvas,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            o,
            game.kingdom.season,
            None,
        );
        canvas
    };
    let with = paint(&overrides);
    let without = paint(&l2_view::campaign::Overrides::new());

    // A near-zoom tile is 58 x 30 and its diamond is about half of that, so one
    // town is four of them - somewhere around 3,500 pixels. Two towns can be in
    // view at once, so the ceiling is generous; the floor is what matters.
    let moved = with.diff_count(&without);
    assert!(moved > 500, "the town's tiles are painted from different frames: {moved} pixels");
    assert!(moved < 30_000, "and only the towns changed, not the whole viewport: {moved}");
}

/// **Clicking the town opens the village**, which is `Map_Click`'s second arm:
/// `if (flags & 0x40) { g_screenId = 2; Village_Draw(1); }`. That arm was
/// missing, so the click fell through to "select the county" and the village
/// had no route in but a key of ours.
#[test]
fn clicking_the_county_town_centres_the_map_on_it_and_opens_the_village() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = *MapScreen::town(&ctx, 8).first().expect("county 8 has a town");
    let (tx, ty) = l2_kingdom::map::coords(tile);

    game.select(8);
    screen.centre_on_tile(tx as usize, ty as usize);
    draw(&mut screen, &mut game, &assets);
    let (cx, cy) =
        l2_view::campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize)
            .expect("the town is on screen after centring on it");

    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Village(8)), "the town opens the village");
}

/// **Every painted pixel of the mine switches the mine.**
///
/// A player reported *"I can't click the iron mine on the world map to
/// enable/disable that"*, and he was describing geometry. `Town1a.pl8` frame 30
/// is 58 × 47 on a 58 × 30 tile: seventeen rows of headframe hang above the
/// tile's diamond, and more of the building falls inside the diamond's bounding
/// box but outside the rhombus. Swept pixel by pixel against the old hit test,
/// **1,314 pixels of the mine were painted and only 857 of them were on the
/// tile** — the entire upper half of the building was dead, and a click there
/// fell through to "open the county panel" instead.
///
/// The sweep is the assertion. It is not vacuous in either direction: the frame
/// really does overhang (asserted), and a pixel *outside* the building that is
/// also outside the diamond must still not toggle, or the fallback would be a
/// bounding box and not a mask.
#[test]
fn every_painted_pixel_of_a_mine_reaches_the_industry_toggle() {
    let (mut game, assets) = world!();
    // A county the player holds that has a mine, from the save.
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8) && game.kingdom.counties[id].industry[1].has_resource)
        .expect("the player starts with a mine somewhere");
    let ctx = Ctx { game: &mut game, assets: &assets };
    let tile = MapScreen::settlements_for_test(&ctx, county as u8)
        .into_iter()
        .find(|&t| ctx.game.kingdom.campaign.map.terrain[t] == 1)
        .expect("and that county has an iron site on the map");
    let (tx, ty) = l2_kingdom::map::coords(tile);

    let mut screen = MapScreen::new();
    game.select(county as u8);
    screen.centre_on_tile(tx as usize, ty as usize);
    draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let frame = slot.at(l2_formats::maps::Plane::GfxIndex, tx as usize, ty as usize) as usize;
    let sheet = assets.map.bank(screen.zoom(), game.kingdom.season, 3).expect("the Town bank");
    let art = sheet.frame(frame).expect("the mine's frame");
    let overhang = art.height as i32 - screen.zoom().tile_h;
    assert!(overhang > 0, "the mine overhangs its tile; without that this test proves nothing");

    let mut painted = 0;
    let mut reached = 0;
    let mut off_the_art_and_off_the_tile = 0;
    for dy in 0..art.height as i32 {
        for dx in 0..art.width as i32 {
            let opaque = art.opaque[dy as usize * art.width as usize + dx as usize];
            let before = game.kingdom.counties[county].industry[1].enabled;
            let (x, y) = (sx + dx, sy - overhang + dy);
            send(&mut screen, &mut game, &assets, Event::Click { x, y });
            let toggled = game.kingdom.counties[county].industry[1].enabled != before;
            if opaque {
                painted += 1;
                reached += usize::from(toggled);
            } else if toggled && dy < overhang {
                // Above the diamond entirely, and not on the building.
                off_the_art_and_off_the_tile += 1;
            }
        }
    }
    assert!(painted > 1_000, "the mine is a building, not a smudge: {painted} pixels");
    assert_eq!(reached, painted, "every painted pixel of the mine must switch it");
    assert_eq!(
        off_the_art_and_off_the_tile, 0,
        "the fallback is the frame's opacity mask, not its bounding box"
    );
}

/// The minimap is the original's own raster out of `MAPnn.PL8`, and clicking it
/// selects the county under the pixel *and* moves the viewport onto it.
///
/// This is the one place we use the original's algorithm and reach its
/// answer: `Minimap_Click` reads the same county byte out of the same file.
#[test]
fn clicking_the_minimap_selects_that_county_and_brings_it_into_view() {
    let (mut game, assets) = world!();
    let minimap = assets.minimap(game.map_slot).expect("Map01.pl8 holds slot 0");

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let visible = pick_counts(&screen);

    // A minimap pixel of a county that is *not* in the opening view, so "the
    // map moved onto it" cannot pass by accident.
    let (mx, my) = (0..128)
        .flat_map(|y| (0..128).map(move |x| (x, y)))
        .map(|(x, y)| (x + chrome::MINIMAP_HIT_X, y + chrome::MINIMAP_HIT_Y))
        .find(|&(x, y)| {
            let c = minimap.county_at(x, y);
            c != 0 && (c as usize) < 17 && visible[c as usize] == 0
        })
        .expect("some county is off screen at the opening viewport");
    let county = minimap.county_at(mx, my);

    let before = screen.viewport();
    send(&mut screen, &mut game, &assets, Event::Click { x: mx, y: my });
    assert_eq!(game.selected, county, "the raster decides which county");
    assert_ne!(screen.viewport(), before, "and the map moves");

    // Having moved, that county is now on screen — which is what "centred"
    // The origin's changing does not imply this.
    draw(&mut screen, &mut game, &assets);
    assert!(pick_counts(&screen)[county as usize] > 0, "county {county} is now in view");
}

/// **The selection is not drawn on the map at all**, which is the assertion
/// that could not exist while our yellow outline did.
///
/// The original draws no selection over the terrain: its borders live in the
/// tile data — `docs/formats/maps-layers.md` §2.1, plane-0 bit `0x02` — and its
/// *selection* is which county the right panel describes. Ours outlined the
/// Selected county in the highlight colour.
/// A yellow outline around the county, selected on the
/// map."*
///
/// This is the inverse of the test it replaces, and it is a stronger claim than
/// *"the outline is gone"*: it says the **map area is byte-identical** under
/// two different selections, so any future selection paint — an outline, a
/// tint, a halo — fails it, not only the one that was removed. The right column
/// is deliberately outside the window, because that is where the selection
/// legitimately shows.
///
/// **The counties are derived, not named**, and none of them is the player's.
/// The field markers under `brush` were drawn for
/// the *selected* county when the player owns it — the visible half of
/// `ours/brush-popup-on-the-map`, since removed, and the markers now debug
/// overlay only. Naming two counties by number would have made this
/// test a statement about one fixture's ownership roll, which
/// `docs/environment.md` says is rolled per game.
///
/// # This test passed with the outline put back, and that is why it looks like
/// this
///
/// The first version took the **first two** counties the player does not own —
/// 1 and 2 on the England fixture — and required the map to be byte-identical
/// between them. The map the fixture opens on shows counties **8 and 9**.
/// Neither 1 nor 2 has a single pixel on screen, so the two canvases were
/// Identical for a reason independent of what the painter draws,
/// and re-adding the yellow outline turned **nothing** red. The claim
/// `docs/decisions.md` C132 makes for it — *"a stronger claim than the outline
/// is gone, because any future selection paint fails it"* — was false as
/// written. `docs/decisions.md` C138.
///
/// So the shape here is: a **baseline** selection that is off-camera, and then
/// Every foreign county that is *on* camera selected in turn against
/// it. The `assert!` that at least one county is visible is what stops the
/// sweep being empty, which is the only way this can go vacuous again.
#[test]
fn the_selection_is_not_drawn_on_the_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // Which counties the opening viewport shows, read off the pick
    // plane — the same plane a click goes through, so "visible" here means the
    // painter put pixels of it on screen.
    let counts = pick_counts(&screen);
    let foreign_and_visible: Vec<u8> = (1..=14u8)
        .filter(|&id| counts[id as usize] > 0 && !game.is_players(id))
        .collect();
    let foreign_and_hidden: Vec<u8> = (1..=14u8)
        .filter(|&id| counts[id as usize] == 0 && !game.is_players(id))
        .collect();
    assert!(
        !foreign_and_visible.is_empty(),
        "nothing the player does not own is on screen, so this test would assert nothing. \
         Visible: {:?}",
        (0..=14u8).filter(|&id| counts[id as usize] > 0).collect::<Vec<_>>(),
    );
    let baseline = *foreign_and_hidden.first().expect("England has a county off the opening view");

    // `Map_SetZoom` gives the map 480 pixels at every zoom and the right column
    // the rest.
    let map_area = |a: &Canvas, b: &Canvas| {
        let mut differ = 0;
        for y in 0..480usize {
            for x in 0..480usize {
                if a.at(x, y) != b.at(x, y) {
                    differ += 1;
                }
            }
        }
        differ
    };

    game.select(baseline);
    let base = draw(&mut screen, &mut game, &assets);

    for id in foreign_and_visible {
        game.select(id);
        let with = draw(&mut screen, &mut game, &assets);
        let differ = map_area(&base, &with);
        assert_eq!(
            differ, 0,
            "county {id} fills {} pixels of the viewport, and selecting it instead of the \
             off-screen county {baseline} changed {differ} of them. The original draws no \
             selection over the terrain at all.",
            counts[id as usize],
        );
    }
}

/// The menu bar reads the clock and the treasury out of the world, and the
/// right column reads the selected county. Checked by finding the actual
/// digits, at the coordinates `Screen_DrawMenuBar` puts them.
#[test]
fn the_map_chrome_shows_the_clock_the_treasury_and_the_selected_county() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    // **The year, then the season, in `g_fontBody` — not our 5 × 7 font, and
    // not in that order.** This used to look for `"WINTER 1268"` in `ink.text`,
    // which is what the two `l2_view::text::draw` calls at the tail of
    // `draw_menu_bar` drew and what a player called *"still placeholder font in
    // the top right for gold and summer"*. `Screen_DrawMenuBar` draws
    // `Ui_DrawYear(g_year, 0x168, 6, 3)` and puts the season at
    // `g_penAdvance + 0x16C`, both in `&g_fontBody` at colour `0x3F`.
    let clock = find_body(&canvas, &assets, " 1268 ", font::TEXT).expect("the year");
    assert_eq!(clock, (360, 6), "the original draws the year at x 360, y 6");
    let season = find_body(&canvas, &assets, "Winter", font::TEXT).expect("the season");
    assert!(season.0 > clock.0, "the season follows the year, at g_penAdvance + 0x16C");
    // `Ui_DrawCount(gold, 0, 500, 6, &g_fontBody, 0x3F)` — the number and then
    // `L2.eng` group 8's *"Crowns."*. `GOLD` was a word of ours.
    let gold = find_body(&canvas, &assets, "1000 ", font::TEXT).expect("the treasury");
    assert_eq!(gold.1, 6, "and the treasury on the same row");
    assert!(find_body(&canvas, &assets, "Crowns.", font::TEXT).is_some(), "with its noun");
    // `TURN n` and `COUNTIES n/m` are ours and are debug overlay now: a normal
    // session draws neither (`the_debug_overlay_is_off_by_default_…`).
    assert!(find_text(&canvas, "TURN 1", ink.dim).is_none());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_none());

    // **The county strip, in the map's own sidebar.** `Screen_DrawCampaign`
    // calls `CountyStrip_Draw` — the map screen used to leave that plate empty
    // and write a box of our own numbers over the jobs plate below it.
    // `Ui_DrawNumber`'s `x` is the *string's* origin and the string opens with
    // the sign column, so the digits sit one `SPACE_ADVANCE` right of the call
    // site's literal. The strip's own test carries the whole argument.
    let lead = l2_game::shell::font::SPACE_ADVANCE;
    let pop = find_strip(&canvas, &assets, "435", STRIP_INK).expect("the population");
    assert_eq!(pop, (0x1FC + lead, 189), "at CountyStrip_Draw's own coordinates");
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left origins, not right-anchored.** `docs/decisions.md` C42.
        Some((0x25A + lead, 189)),
        "and the happiness beside it"
    );

    // The near-misses. If the search could match anything it would match these.
    assert!(find_body(&canvas, &assets, " 1269 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "1001 ", font::TEXT).is_none());
    assert!(find_body(&canvas, &assets, "Summer", font::TEXT).is_none());
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
}

// ---------------------------------------------------------------------------
// **Pixels, asserted.** C59.
//
// Three visual features have now been reported missing by a human *after* being
// merged — the town flag, the merchant sprite, the minimap tint. Two of the
// three did have a pixel test:
// [`the_county_town_flies_its_owners_flag_and_it_waves`] and
// [`a_merchant_is_drawn_and_opens_the_merchant_screen_from_the_county_it_is_in`]
// both diff two canvases and assert the ink landed in the right box. **The
// minimap's tint had none**, and it is the one the player was still describing
// as wrong.
//
// Both tests below are stronger than a diff, in the same way: a diff says
// *something* changed inside a rectangle, so it passes on a garbage sprite or
// on the wrong frame of the right sheet. These match the **artwork itself** —
// the frame's own palette indices, several hundred of them, standing where the
// blit put them — and then vary one field of the save and require the picture
// to follow it. That is what makes them able to catch a flag that draws, but
// draws the wrong realm's.
//
// The house technique is to turn the visual claim into a number the file can
// settle and then assert the number. Two precedents: *a tick cannot come 4th of
// 84 frames by ink*, and *a two-pixel ring is four pixels of width*.
// ---------------------------------------------------------------------------

/// A sprite's **exact ink**, found anywhere on the canvas.
///
/// The same idea as [`find_text`] and for the same reason: render the thing
/// Being looked for: keep the pixels it paints.
/// pattern. A PL8 blit copies only its opaque bytes, so a match is the frame's
/// own palette indices standing where the frame was blitted — several hundred
/// of them for a flag. That cannot arise from terrain.
///
/// Every place this frame's artwork stands, and how many opaque pixels had to
/// agree to make each one a match.
fn sprite_positions(
    canvas: &Canvas,
    frame: &l2_formats::pl8::DecodedFrame,
) -> (Vec<(i32, i32)>, usize) {
    let (w, h) = (frame.width as i32, frame.height as i32);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| frame.opaque[(y * w + x) as usize])
        .map(|(x, y)| (x, y, frame.indices[(y * w + x) as usize]))
        .collect();
    let mut found = Vec::new();
    if wanted.is_empty() {
        return (found, 0);
    }
    for oy in 0..=(canvas.height as i32 - h) {
        for ox in 0..=(canvas.width as i32 - w) {
            let (fx, fy, fi) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != fi {
                continue;
            }
            if wanted.iter().all(|&(x, y, i)| canvas.at((ox + x) as usize, (oy + y) as usize) == i)
            {
                found.push((ox, oy));
            }
        }
    }
    (found, wanted.len())
}

/// The first of them, scanning rows then columns.
fn find_sprite(
    canvas: &Canvas,
    frame: &l2_formats::pl8::DecodedFrame,
) -> Option<((i32, i32), usize)> {
    let (found, ink) = sprite_positions(canvas, frame);
    found.first().map(|&p| (p, ink))
}

/// The map centred on a county's town, drawn.
fn town_view(game: &mut Game, assets: &Assets, county: u8) -> (MapScreen, Canvas) {
    let mut screen = MapScreen::new();
    {
        let ctx = Ctx { game, assets };
        let town = MapScreen::town(&ctx, county);
        let &tile = town.first().expect("a county has a town");
        let (tx, ty) = l2_kingdom::map::coords(tile);
        screen.centre_on_tile(tx as usize, ty as usize);
    }
    let canvas = draw(&mut screen, game, assets);
    (screen, canvas)
}

/// **The county town flies its owner's flag, and it waves.**
///
/// A player: *"I didn't see the colorful waving flag over my county."* It is
/// There: the assertion in numbers.
/// screenshot somebody has to open.
///
/// Three claims, each with its own pixels:
///
/// 1. **It is drawn.** `Flags1a.pl8` frame `(shield − 1) * 8 + phase` — several
///    hundred opaque palette indices — stands somewhere on the canvas, exactly.
/// 2. **The frame is keyed on the shield.** Move the owning realm's
/// `shield_index` and the flag at the *same pixel* becomes the other
///    shield's frame. Nothing else on the campaign map reads `shield_index` —
/// the minimap and the menu-bar banner both read `realm_colour` — so this
///    isolates the flag from everything drawn beside it.
/// 3. **The wave advances.** Sixteen ticks is one phase (`phase = tick >> 4`),
///    and after them the flag at the same pixel is the next frame of the eight.
///
/// If any of the three stops being true the feature has gone, and this fails
/// instead of a person noticing weeks later. C59.
#[test]
fn the_county_town_flies_its_owners_flag_and_the_wave_advances() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let owner = game.kingdom.counties[county as usize].owner as usize;
    let shield = game.kingdom.realms[owner].shield_index;
    assert!(
        (1..=5).contains(&shield),
        "realm {owner} carries shield {shield}, which flies nothing"
    );

    let (mut screen, canvas) = town_view(&mut game, &assets, county);
    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8 is in the install");
    let frame_of = |shield: u8, phase: u8| {
        let i = campaign::flag_frame(shield, phase).expect("a shield of 1 ..= 5 has a frame");
        sheet.frame(i).expect("Flags1a.pl8 holds forty 32 x 24 frames")
    };

    // 1 — it is drawn.
    let f = frame_of(shield, 0);
    assert_eq!((f.width, f.height), (32, 24), "the first forty frames are 32 x 24");
    let (at, ink) = find_sprite(&canvas, &f)
        .expect("the county town flies its owner's flag, and it is not on the canvas");
    assert!(
        ink >= 200,
        "the flag matched on only {ink} opaque pixels, which is too few to be the flag"
    );

    assert!(at.0 < campaign::PANEL_X, "the flag is on the map, not in the sidebar");

    // 2 — the frame is keyed on the shield, at the same pixel.
    let other = if shield == 5 { 1 } else { shield + 1 };
    game.kingdom.realms[owner].shield_index = other;
    let (_, moved) = town_view(&mut game, &assets, county);
    assert_eq!(
        find_sprite(&moved, &frame_of(other, 0)).map(|(p, _)| p),
        Some(at),
        "with shield {other} the same pixel must fly shield {other}'s flag"
    );
    game.kingdom.realms[owner].shield_index = shield;

    // 3 — the wave advances. `Map_DrawFrame`: `phase = (tick & 0x7F) >> 4`.
    for _ in 0..16 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let _ = screen.update(&mut ctx);
    }
    let waved = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_sprite(&waved, &frame_of(shield, 1)).map(|(p, _)| p),
        Some(at),
        "sixteen ticks is one phase, so the same pixel must now fly phase 1"
    );
}

/// **A garrisoned castle flies its *garrison's* shield, and an empty one flies
/// nothing.**
///
/// The second half of `FUN_004071A0`, and the half that is easy to get wrong by
/// reading the county instead of the unit standing in it:
///
/// ```c
/// else if (flags & 0x80) {                       /* the castle */
///   if (content <= 0x14 || !county.garrisonUnit) return;
///   shield = units[county.garrisonUnit].shield;  /* NOT county.owner */
/// }
/// ```
///
/// So a castle taken from somebody whose garrison is still theirs flies
/// **their** colours, and the two flags of one county disagree. That is the
/// case worth testing, and it is the case a fixture cannot supply: the position
/// is set up here — a county the player holds, its castle built, and somebody
/// *else's* army standing in it — so the two flags must be two different
/// Pictures. A save with a garrison would almost always
/// have one whose shield matched its host's, and would prove nothing about
/// which of the two fields the branch reads.
///
/// The measurement is a **count**, not a position: the town of the same county
/// is flying a flag of its own a few tiles away, so what is asserted is that
/// taking the garrison out removes **exactly one** flag and leaves the other
/// standing. C59.
#[test]
fn a_garrisoned_castle_flies_the_garrisons_shield_and_an_empty_one_flies_nothing() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8))
        .expect("the player holds a county");
    game.select(county as u8);

    let owner = game.kingdom.counties[county].owner as usize;
    let town_shield = game.kingdom.realms[owner].shield_index;
    // Somebody else's shield, so the castle's flag and the town's cannot be
    // confused for one another.
    let garrison_shield = if town_shield == 5 { 1 } else { town_shield + 1 };

    // A built castle with a foreign army inside it. `content <= 0x14` is the
    // bare plot and flies nothing even with a garrison standing on it, so the
    // castle has to be built for this to be the branch under test.
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("the fixture carries units");
    game.kingdom.campaign.units.get_mut(unit).expect("the slot exists").shield = garrison_shield;
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = c.castle_type.max(1);
        c.garrison_unit = unit;
    }

    let mut screen = MapScreen::new();
    {
        let c = &game.kingdom.counties[county];
        screen.centre_on_tile(c.anchor_x as usize, c.anchor_y as usize);
    }
    let held = draw(&mut screen, &mut game, &assets);

    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8 is in the install");
    let frame_of = |shield: u8| {
        sheet
            .frame(campaign::flag_frame(shield, 0).expect("a shield of 1 ..= 5 has a frame"))
            .expect("Flags1a.pl8 holds forty 32 x 24 frames")
    };
    let castle_flag = frame_of(garrison_shield);
    let town_flag = frame_of(town_shield);

    let (before, ink) = sprite_positions(&held, &castle_flag);
    assert!(
        !before.is_empty(),
        "the garrisoned castle of county {county} flies no flag: shield {garrison_shield}"
    );
    assert!(ink >= 200, "matched on {ink} opaque pixels, too few to be a flag");
    let towns_before = sprite_positions(&held, &town_flag).0.len();

    // Take the garrison away. `if (!county.garrisonUnit) return`.
    game.kingdom.counties[county].garrison_unit = 0;
    let empty = draw(&mut screen, &mut game, &assets);
    assert!(
        sprite_positions(&empty, &castle_flag).0.is_empty(),
        "an empty castle must fly nothing, and shield {garrison_shield}'s flag is still there"
    );
    assert_eq!(
        sprite_positions(&empty, &town_flag).0.len(),
        towns_before,
        "and the town's own flag reads the county's owner, so it must not have moved"
    );

    // The other half of the guard: a garrison on a bare plot flies nothing
    // either, whatever its shield.
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = 0;
        c.garrison_unit = unit;
    }
    let bare = draw(&mut screen, &mut game, &assets);
    assert!(
        sprite_positions(&bare, &castle_flag).0.is_empty(),
        "`content <= 0x14` is the bare plot, and an unbuilt castle flies nothing"
    );
}

/// **A besieged castle carries the besieger's camp mark and the seasons he has
/// left** — `FUN_00407F82` (`0x00407F82`), called from `Sprite_TopIt`'s castle
/// arm before the garrison's banner.
///
/// The report was that a siege is invisible on the map. It was: we drew a dot
/// over the *army*, gated as a debug overlay because the original draws nothing
/// there, and nothing at all over the castle, where the original draws this.
///
/// ```c
/// besieger = g_units[county.garrisonUnit].besiegedBy;
/// if (besieger != 0 && g_mapZoom == 0)
///     FUN_00407f82(g_units[besieger].siegeSeasonsLeft, 8, -0x38);
/// ```
///
/// Two pictures, and the assertion is the pair: `Flags1a.pl8` frame `0x82`
/// (24 × 28, the sheet's **last** frame — it holds 131) at `(+8, −0x38)` from
/// the castle tile's origin, and the count centred in that frame's own width
/// ten pixels lower, flat and in `0xF9`. Neither is reachable from a fixture,
/// so the siege is staged here: the link the game keeps is on the *garrison*,
/// `+0x19A`, and the number is on the besieger, `+0x19C`.
///
/// **Ablated**: dropping either draw, moving the mark to the flag's own
/// `(+0x1A, −0x1C)`, putting the count on the garrison instead of the besieger,
/// or centring it in anything but frame `0x82`'s width turns this red.
#[test]
fn a_besieged_castle_carries_the_besiegers_mark_and_his_seasons_left() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8))
        .expect("the player holds a county");
    game.select(county as u8);

    // A built castle, an army inside it, and a second army camped outside with
    // four seasons of work left.
    let mut ids = game.kingdom.campaign.units.iter().map(|(id, _)| id);
    let garrison = ids.next().expect("the fixture carries units");
    let besieger = ids.next().expect("the fixture carries a second unit");
    drop(ids);
    const SEASONS: u8 = 4;
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = c.castle_type.max(1);
        c.garrison_unit = garrison;
    }
    game.kingdom.campaign.units.get_mut(besieger).expect("the slot exists").siege_seasons_left =
        SEASONS;

    // The castle's own tile, found the way the painter finds it.
    let castle = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        let terrain = ctx.game.kingdom.campaign.map.terrain.clone();
        MapScreen::settlements_for_test(&ctx, county as u8)
            .into_iter()
            .find(|&t| {
                l2_kingdom::industry::map_toggle_for_graphic(terrain[t])
                    == Some(l2_kingdom::industry::MapToggle::Castle)
            })
            .expect("the county's castle plot is on the map")
    };
    let (cx, cy) = l2_kingdom::map::coords(castle);

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    screen.centre_on_tile(cx as usize, cy as usize);
    let free = draw(&mut screen, &mut game, &assets);

    let mark = assets
        .map
        .flag_sheet(screen.zoom())
        .and_then(|s| s.frame(campaign::BESIEGER_MARKER_FRAME))
        .expect("Flags1a.pl8 frame 0x82");
    assert_eq!((mark.width, mark.height), (24, 28), "frame 0x82 is the 24 x 28 camp mark");
    assert!(
        sprite_positions(&free, &mark).0.is_empty(),
        "an unbesieged castle carries no camp mark"
    );

    // `+0x19A` on the *garrison* is the link, and it is what the arm reads.
    game.kingdom.campaign.units.get_mut(garrison).expect("the slot exists").besieged_by =
        besieger as u8;
    let besieged = draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(cx as usize, cy as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let (mx, my) = (sx + screen.zoom().besieger_at.0, sy + screen.zoom().besieger_at.1);

    // The whole frame, pixel for pixel, at the one place the two literals name.
    let mut reference = free.clone();
    reference.blit_clipped(&mark, mx, my, screen.map_clip());
    // `Ui_DrawNumberRight(seasons, ' ', " ", x, y + 10, frameWidth, &g_fontBody,
    // 0xF9)` — which centres, and is flat because `DAT_005AEA40` is set for it.
    let count = format!(" {SEASONS} ");
    let style = l2_game::shell::font::Style {
        colour: campaign::BESIEGER_COUNT_INK,
        shadow: None,
        caps: None,
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let ny = my + campaign::BESIEGER_COUNT_DY;
    assert!(campaign::BESIEGER_COUNT_TOP < ny, "the count clears the menu bar on this tile");
    body.draw_centred(&mut reference, mx, ny, i32::from(mark.width), &count, &style);

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
    // and its size say where the ink went and are what a reader needs.
    let corner = |v: &[usize]| -> (usize, usize, usize, usize, usize) {
        let xs = v.iter().map(|i| i % besieged.width);
        let ys = v.iter().map(|i| i / besieged.width);
        (
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
            v.len(),
        )
    };
    let (drawn, expected) = (moved(&besieged, &free), moved(&reference, &free));
    assert!(!expected.is_empty(), "frame 0x82 writes nothing at the place the literals name");
    assert_eq!(
        corner(&drawn),
        corner(&expected),
        "the siege's ink (x0, y0, x1, y1, count) is not frame 0x82 at ({mx}, {my}) with the \
         count centred in its 24 pixels at ({mx}, {ny})"
    );
    assert_eq!(drawn, expected, "the siege's ink is the right size in the right place and is not the same pixels");
    // Independently of the set: the digits are findable where they were put.
    let off = ((i32::from(mark.width) - body.width(&count)) / 2).max(0);
    assert_eq!(
        find_body(&besieged, &assets, &count, campaign::BESIEGER_COUNT_INK),
        Some((mx + off, ny)),
        "the seasons left are the besieger's `+0x19C`, centred in frame 0x82's own width"
    );

    // **The far zoom draws nothing, and that is the original's.**
    // `Sprite_TopIt` calls `FUN_00407F82(…, 2, -0x28)` at `g_mapZoom == 2` and
    // the whole of that function is inside `if (g_mapZoom == 0)`, so the call
    // returns having drawn nothing. `docs/bugs.md`.
    let mut far = MapScreen::new();
    draw(&mut far, &mut game, &assets);
    send(&mut far, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    assert_eq!(far.zoom().id, campaign::FAR.id, "Z is the zoom toggle");
    let zoomed = draw(&mut far, &mut game, &assets);
    if let Some(far_mark) =
        assets.map.flag_sheet(far.zoom()).and_then(|s| s.frame(campaign::BESIEGER_MARKER_FRAME))
    {
        assert!(
            sprite_positions(&zoomed, &far_mark).0.is_empty(),
            "the far zoom's call site is dead in the original and must be dead here"
        );
    }

    // And the mark goes when the siege does.
    game.kingdom.campaign.units.get_mut(garrison).expect("the slot exists").besieged_by = 0;
    let lifted = draw(&mut screen, &mut game, &assets);
    assert_eq!(lifted.diff_count(&free), 0, "no siege, no mark and no count");
}

/// **The minimap tints by owner, and a realm's ramp is its own.**
///
/// A player: *"the minimap had default colors, it didn't identify who
/// owned a county."* The number that settles it is the count of distinct ramp
/// **rows** standing in the minimap rectangle. `MINIMAP_REALM_RAMP` is six rows
/// of four shades — row 0 the raster's own shading for unowned land, rows 1 … 5
/// one per realm colour — so:
///
/// * a minimap that tints by owner shows **one row per owning colour, plus row
///   0 wherever land is unowned**;
/// * a minimap that has lost the tint shows **exactly one row**, because
///   `chrome::realm_colour` clamps a zero colour *up* to 1 and every county
///   would collapse onto ramp 1.
///
/// On the England fixture that is five owned counties flying five different
/// colours out of fourteen, so six rows against one. The two cannot be
/// confused, which is the property that makes this worth asserting.
///
/// It also asserts the thing that makes the count mean anything: **no palette
/// index appears in two rows**, so "which row is this pixel from" has one
/// answer. C59.
#[test]
fn the_minimap_paints_one_ramp_row_per_owning_realm() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    // The ramp must be unambiguous before it can be counted.
    let mut row_of_index = [None::<usize>; 256];
    for (row, shades) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        for &i in shades {
            assert_eq!(
                row_of_index[i as usize], None,
                "palette index {i:#04x} is in two ramp rows, so a pixel cannot name its realm"
            );
            row_of_index[i as usize] = Some(row);
        }
    }

    // **The measurement is a difference, not a census**, and it has to be.
    // `MAPnn.PL8`'s raster also carries pixels the overlay leaves alone —
    // coastline, borders, the panel round it — and some of those indices are
    // by coincidence entries of a ramp row (0x38, the beige, is row 3's
    // darkest). Counting colours across the whole rectangle would therefore
    // report a realm nobody owns. What is unambiguous is what *moves* when the
    // ownership moves: the raster is byte-for-byte the same in both renders, so
    // every differing pixel is one the tint wrote.
    let tinted = draw(&mut MapScreen::new(), &mut game, &assets);
    let owners: Vec<u8> =
        game.kingdom.county_ids().map(|id| game.kingdom.counties[id].owner).collect();
    for id in game.kingdom.county_ids() {
        game.kingdom.counties[id].owner = 0;
    }
    let flat = draw(&mut MapScreen::new(), &mut game, &assets);
    for (id, owner) in game.kingdom.county_ids().zip(owners) {
        game.kingdom.counties[id].owner = owner;
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut moved = 0usize;
    let mut selected_pixels = 0usize;
    for y in 0..chrome::MINIMAP_DIM {
        for x in 0..chrome::MINIMAP_DIM {
            let (px, py) = ((chrome::MINIMAP_X + x) as usize, (chrome::MINIMAP_Y + y) as usize);
            let (a, b) = (tinted.at(px, py), flat.at(px, py));
            if a == chrome::MINIMAP_SELECTED {
                selected_pixels += 1;
            }
            if a == b {
                continue;
            }
            moved += 1;
            assert_eq!(
                row_of_index[b as usize],
                Some(0),
                "an unowned county must paint the raster's own shading, ramp row 0, \
                 and this pixel painted {b:#04x}"
            );
            let row = row_of_index[a as usize].unwrap_or_else(|| {
                panic!("owned land painted {a:#04x}, which is in no ramp row at all")
            });
            seen.insert(row);
        }
    }

    // What the save says the answer should be, worked out from the counties
    // From the painting.
    let mut wanted = std::collections::BTreeSet::new();
    for id in game.kingdom.county_ids() {
        let owner = game.kingdom.counties[id].owner as usize;
        if owner != 0 {
            wanted.insert(chrome::realm_colour(game.realm_colour[owner]) as usize);
        }
    }
    assert!(
        wanted.len() >= 2,
        "this fixture has {} owning colour(s), so it could not tell a tinted minimap \
         from an untinted one even if the tint were gone",
        wanted.len()
    );

    assert_eq!(
        seen, wanted,
        "the minimap's ramp rows must be exactly the ones the counties' owners ask for; \
         **one single row** is the shape of the failure to watch for — `realm_colour` \
         clamps a zero colour up to 1, so a tint that has lost its input does not go \
         blank, it goes uniformly red"
    );
    assert!(
        moved > 500,
        "only {moved} pixels changed when five counties changed hands, which is too few \
         to be five counties"
    );
    assert!(
        selected_pixels > 0,
        "the selected county's brightest shade is replaced by MINIMAP_SELECTED, and none was drawn"
    );
}

// ------------------------------------------------ seasons, fields, animation

/// **The map's artwork changes when the season does — driven by a real turn.**
///
/// `Gfx_LoadCountyMode` (`0x004984DC`) repoints all five near-zoom tile banks
/// at `(g_season - 1) * 8` in `g_resourceTable`, so the whole picture is
/// redrawn from different files. We hard-coded the `a` set and the map looked
/// the same in January and in August.
///
/// **This ends a turn.** A test that sets
/// `kingdom.season = 4` and then reads a lookup table is checking its own
/// fixture (`docs/agents.md`); the season has to be moved by the thing that
/// moves it in play. `l2_game::turn::end_turn` runs the whole phase machine.
#[test]
fn a_real_turn_turns_the_season_and_the_map_is_repainted_from_other_files() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before_season = game.kingdom.season;
    let before = draw(&mut screen, &mut game, &assets);

    l2_game::turn::end_turn(&mut game).expect("the turn completes without asking");
    let after_season = game.kingdom.season;
    assert_ne!(after_season, before_season, "one turn is one season");

    let after = draw(&mut screen, &mut game, &assets);
    let moved = before.diff_count(&after);

    // The viewport is 480 x 450 under the menu bar — about 216,000 pixels — and
    // a whole-bank swap repaints essentially all of it. The floor is what
    // matters: hard-coding one season made this zero.
    assert!(
        moved > 50_000,
        "the season turned from {before_season} to {after_season} and only {moved} pixels moved"
    );

    // …and it is the *artwork* that changed, not our own markers: the two
    // frames must use meaningfully different palettes. Winter is the bright one
    // and autumn has no green in it (`campaign::SEASON_SUFFIX`).
    let histogram = |c: &Canvas| {
        let mut h = [0u32; 256];
        for &p in &c.pixels {
            h[p as usize] += 1;
        }
        h
    };
    let (a, b) = (histogram(&before), histogram(&after));
    let differing = (0..256).filter(|&i| a[i].abs_diff(b[i]) > 200).count();
    assert!(differing > 8, "only {differing} palette entries changed their share of the frame");
}

/// **A town drawn through `Overrides` in one season is still a town in the
/// next** — which is the thing the season swap could have broken.
/// `install.rs::the_four_seasons_of_a_bank_are_the_same_frame_table` exists.
///
/// The overrides plane stores a **frame index**, not a picture. If frame 47 of
/// `Town1c.pl8` were a different cell of the sheet than frame 47 of
/// `Town1a.pl8`, every county town on the map would turn back into a quarry in
/// autumn — a defect a player would report as *"my buildings disappear"*. The
/// frame tables agree, so it does not happen, and this asserts the consequence
/// At the pixel.
#[test]
fn a_towns_overridden_graphic_survives_every_season() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let tile = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, 8).first().expect("county 8 has a town")
    };
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let overrides = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        MapScreen::tile_graphics(&ctx)
    };

    let paint = |season: u8, o: &campaign::Overrides| {
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
            o,
            season,
            None,
        );
        canvas
    };
    let bare = campaign::Overrides::new();
    let mut seasons_that_differ = 0;
    for season in 1..=campaign::SEASONS as u8 {
        let with = paint(season, &overrides);
        let without = paint(season, &bare);
        let moved = with.diff_count(&without);
        assert!(
            moved > 500,
            "season {season}: the override changed only {moved} pixels, so the town is not \
             being restamped"
        );
        seasons_that_differ += 1;
    }
    assert_eq!(seasons_that_differ, 4, "all four seasons keep their towns");
}

/// **The minimap closes the management surface from any screen**, which is
/// `Screen_FrameInput`'s epilogue and not any screen's own arm.
///
/// `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`.
#[test]
fn a_press_on_the_minimap_drops_whatever_is_open_over_the_map() {
    let (mut game, assets) = world!();
    game.select(8);
    let hit = chrome::minimap_hit_area();
    let (mx, my) = (hit.x0 + 40, hit.y0 + 40);

    // The three graduated map overlays are here because graduating them out of
    // the shell table LOST this arm: the wrapper reproduced it once for all
    // seven shells and each screen now has to carry it. That regression was
    // invisible until this test and the graduation met in one merge.
    for over in [
        ScreenId::County(8, Panel::Tax),
        ScreenId::Job(8, 0),
        ScreenId::Court,
        ScreenId::Ratings,
        ScreenId::Supplies(8),
    ] {
        let mut m = over_the_map(over);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: mx, y: my });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "{over:?} gave way to the minimap");
        assert_eq!(m.depth(), 1, "{over:?}: the map is revealed, not rebuilt");
    }

    // The ablation: a press just OUTSIDE the raster leaves everything alone. An
    // unconditional `Pass` would close on both.
    let mut m = over_the_map(ScreenId::Job(8, 0));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: hit.x1 + 4, y: my });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, 0)), "outside the raster nothing happens");
}

// --- the fog of war ----------------------------------------------------------
//
// `l2_kingdom::explore` has every reader and writer of the original's seen bit.
// These are the painters' half, and each assertion is about a *tile*: what is
// drawn on one the person has not seen, and what is drawn once he has.
//
// **Ablations, run on this branch** — each line deleted, and the test named:
//
// | deleted | red |
// |---|---|
// | the fog arm of `campaign::draw` (`Map_DrawTile`) | `a_dark_tile_…` |
// | `if fog.is_some() { 0 }` on the surround | `with_the_fog_on_the_sea_…` |
// | the `hides_tile` test in `draw_units` (`Map_DrawArmies`) | `a_county_in_the_dark_…` |
// | `.filter(lit)` on the town banner (`Sprite_TopIt` arm 1) | `a_county_in_the_dark_…` |
// | `.filter(lit)` on the mercenary marker (arm 2) | `a_county_in_the_dark_…` |
// | the `hides_tile` test in `draw_herds` (arm 4) | `a_county_in_the_dark_…` |
// | the `hides_tile` test on our owner marker | `a_county_in_the_dark_…` |
// | `exploration &&` in `l2_kingdom::explore::hides` | `a_county_in_the_dark_…` (the control), and `explore::tests::the_painters_test_…` |
//
// **Two of those rows were green the first time they were run, and the test
// was wrong both times, not the gate.** The herd gate: the field chosen already
// had a herd, so the herd was in both renders. The option test: the control
// also repainted a field's terrain, which moves the fog-off render whatever the
// option test says. The comments at `pasture` and `give_away` say what changed.
//
// **Not covered, and said so:** the castle's garrison banner (England at turn
// one has no garrison), the industry wheel's pause in the dark (a clock drives
// it, and no test here ticks the map), and our field markers (drawn only for
// the person's own county, which is never dark).

/// A fresh map screen opened on the person, then centred on `(x, y)` and drawn.
/// Fresh, so the painted-terrain cache cannot answer for the painter.
fn paint_at(game: &mut Game, assets: &Assets, x: u8, y: u8) -> (MapScreen, Canvas) {
    let mut screen = MapScreen::new();
    draw(&mut screen, game, assets);
    screen.centre_on_tile(x as usize, y as usize);
    let canvas = draw(&mut screen, game, assets);
    (screen, canvas)
}

/// How many pixels inside the map viewport differ.
fn map_pixels_differ(a: &Canvas, b: &Canvas, clip: l2_view::Clip) -> usize {
    a.pixels
        .iter()
        .zip(b.pixels.iter())
        .enumerate()
        .filter(|&(i, (p, q))| {
            let (x, y) = ((i % a.width) as i32, (i / a.width) as i32);
            p != q && clip.contains(x, y)
        })
        .count()
}

/// **`Map_DrawTile` (`0x004063C1`): `if (g_optExploration == 1 && (bank & 0x20)
/// == 0) { bank = 0; frame = 0; }`**, and `Map_DrawTileApex` draws nothing.
///
/// The check is idempotence (`docs/agents.md`): blit the
/// `base` bank's frame 0 over the drawn tile again, and a tile that was already
/// that picture does not change by a pixel. A tile drawn as its own terrain
/// does. The tile is chosen with its own picture *not* frame 0, every tile
/// within two of it dark, and no unit within three — so nothing but the
/// terrain pass can have put a pixel there.
#[test]
fn a_dark_tile_draws_the_base_banks_first_frame_until_it_is_seen_or_the_fog_is_off() {
    use l2_formats::maps::Plane;
    use l2_kingdom::map::{coords, index, MAP_TILES};

    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let map = game.kingdom.campaign.map.clone();
    let tile = (0..MAP_TILES)
        .find(|&t| {
            let (x, y) = coords(t);
            // Well inside the map, so the centred near view is not clamped
            // against an edge and the tile really is in the middle of it.
            if !(16..48).contains(&x) || !(16..48).contains(&y) || map.county[t] == 0 {
                return false;
            }
            let own = (
                slot.at(Plane::GfxBank, x as usize, y as usize) & 0x1C,
                slot.at(Plane::GfxIndex, x as usize, y as usize),
            );
            own != (0, 0)
                && (-2..=2).all(|dy: i32| {
                    (-2..=2).all(|dx: i32| {
                        game.hides_tile(index((x as i32 + dx) as u8, (y as i32 + dy) as u8))
                    })
                })
                && game.kingdom.campaign.units.iter().all(|(_, u)| {
                    (u.x as i32 - x as i32).abs() > 3 || (u.y as i32 - y as i32).abs() > 3
                })
        })
        .expect("England at turn one is dark almost everywhere");
    let (x, y) = coords(tile);

    // **What "base frame 0" is: it is two
    // different pictures.** Over the player's own files, all four seasons:
    //
    // * **near zoom, 58 × 30: not one opaque pixel.** Every byte is palette
    //   index 0, which every blitter skips, so the fog arm paints nothing and a
    //   dark tile is the black ground the map is drawn on — the game's own
    //   *"blacked out"*;
    // * **far zoom, 10 × 6: a filled 36-pixel diamond** of ten green indices,
    //   the same in every season. Zoomed out, the dark is plain grass.
    //
    // The first version of this test asserted "blank at both zooms" and was
    // wrong at the far one; it was measured after that. It is also why the
    // check below is not `docs/agents.md`'s re-blit idempotence: re-blitting a
    // blank frame changes nothing over *any* tile, and that version passed on a
    // lit tile until its own option-off control caught it.
    for season in 1..=4u8 {
        let near = assets.map.bank(&campaign::NEAR, season, 0).and_then(|s| s.frame(0));
        let near = near.expect("the near base bank has a frame 0");
        assert_eq!((near.width, near.height), (58, 30), "season {season}");
        assert_eq!(near.opaque.iter().filter(|&&o| o).count(), 0, "season {season}: near, blank");
        let far = assets.map.bank(&campaign::FAR, season, 0).and_then(|s| s.frame(0));
        let far = far.expect("the far base bank has a frame 0");
        assert_eq!((far.width, far.height), (10, 6), "season {season}");
        assert_eq!(far.opaque.iter().filter(|&&o| o).count(), 36, "season {season}: far, a diamond");
    }
    // The ink behind a dark tile, then: how many pixels of the tile's middle are
    // something else. The middle only — a 13 × 7 patch round the centre, well
    // inside the diamond, which no neighbour's diamond reaches.
    let background = assets.ink.background;
    let lit_pixels = |screen: &MapScreen, canvas: &Canvas| {
        let (cx, cy) = campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
            .expect("the tile is in view");
        let mut n = 0;
        for py in cy - 3..=cy + 3 {
            for px in cx - 6..=cx + 6 {
                if canvas.pixels[py as usize * canvas.width + px as usize] != background {
                    n += 1;
                }
            }
        }
        n
    };

    let (screen, dark) = paint_at(&mut game, &assets, x, y);
    let centre = campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
        .expect("the tile is in the view it was centred on");
    assert!(screen.map_clip().contains(centre.0, centre.1), "({x}, {y}) is inside the map viewport");
    assert_eq!(
        lit_pixels(&screen, &dark),
        0,
        "({x}, {y}) is unseen and must be drawn as base frame 0"
    );

    // The option off, nothing more seen: the tile is its own terrain, and the
    // plane stops mattering at all — every tile seen draws the same canvas.
    let mut off = game.clone();
    off.kingdom.options.exploration = false;
    let (screen, off_canvas) = paint_at(&mut off, &assets, x, y);
    assert!(lit_pixels(&screen, &off_canvas) > 0, "the fog off shows ({x}, {y})");
    let mut off_all_seen = off.clone();
    off_all_seen.kingdom.campaign.explored.reveal_square(off.player, 32, 32, 64);
    let (_, off_all) = paint_at(&mut off_all_seen, &assets, x, y);
    assert_eq!(off_canvas.diff_count(&off_all), 0, "with the option off, what was seen draws nothing different");

    // Seen, with the fog still on: the county's reveal lights the tile.
    let county = map.county[tile];
    game.kingdom.campaign.explored.reveal_county(game.player, &map, county);
    let (screen, lit) = paint_at(&mut game, &assets, x, y);
    assert!(lit_pixels(&screen, &lit) > 0, "seen, ({x}, {y}) is its own terrain");
}

/// **The six surround arms of `Map_RenderIso`, `Map_RenderAlignedRow` and
/// `Map_RenderOffsetRow`**: `frame = g_optExploration == 1 ? 0 : cell -
/// 0x0FFF0000`. Everything seen, the corner of the map in view: turning the
/// option on changes the picture, and it changes only because of the surround —
/// An unseen tile is left.
#[test]
fn with_the_fog_on_the_sea_round_the_map_is_the_base_banks_first_frame() {
    let (mut game, assets) = world!();
    game.kingdom.campaign.explored.reveal_square(game.player, 32, 32, 64);
    game.kingdom.options.exploration = false;
    let (screen, off) = paint_at(&mut game, &assets, 0, 0);
    game.kingdom.options.exploration = true;
    let (_, on) = paint_at(&mut game, &assets, 0, 0);
    assert!(
        map_pixels_differ(&off, &on, screen.map_clip()) > 0,
        "the off-map surround is drawn from the lattice with the option off and as frame 0 with it on"
    );
}

/// **A county in the dark gives nothing away on the map.** Everything the
/// original draws *over* a tile is behind the fog test on that tile —
/// `Sprite_TopIt`'s first statement and `Map_DrawArmies`' whole body — so the
/// county's owner, its banner, a mercenary band in its square, the cattle in
/// its pastures and an army standing in it are all invisible.
///
/// Stated as an equality: change every one of them and the map viewport does
/// not move by a pixel.
/// vacuous**: the same changes with the fog off move it.
#[test]
fn a_county_in_the_dark_gives_nothing_away_on_the_map() {
    use l2_kingdom::map::{flags, MAP_TILES};

    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let county = game
        .kingdom
        .county_ids()
        .map(|id| id as u8)
        .find(|&id| {
            let c = &game.kingdom.counties[id as usize];
            c.owner != 0
                && c.owner != game.player
                && (0..MAP_TILES)
                    .all(|t| game.kingdom.campaign.map.county[t] != id || game.hides_tile(t))
        })
        .expect("an AI county the person has not seen");
    let town = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, county).first().expect("the county has a town")
    };
    let (tx, ty) = l2_kingdom::map::coords(town);
    // A field of the county's that has **no herd on it yet**, so that making
    // it a crowded pasture is an appearance and not a change of picture. The
    // first version took any field, found one already at `0x16`, and the herd
    // gate's ablation stayed green: the herd was in both renders.
    let pasture = (0..MAP_TILES).find(|&t| {
        let m = &game.kingdom.campaign.map;
        m.county[t] == county
            && m.flags[t] & flags::FARMLAND != 0
            && campaign::herd_sprite(m.terrain[t], 0).is_none()
    });
    assert!(pasture.is_some(), "county {county} has a field with no herd on it");

    // `with_herd` is false for the control below.
    // convenience: turning a field into a pasture also repaints the field's own
    // terrain, which the fog-off render shows whatever `hides_tile` says. With
    // it in, the control moved on the terrain alone, and deleting
    // `exploration &&` from `l2_kingdom::explore::hides` — which hides every
    // overlay on an unseen tile *with the option off* — stayed green.
    let give_away = |g: &mut Game, with_herd: bool| {
        let owner = g.kingdom.counties[county as usize].owner as usize;
        g.kingdom.realms[owner].shield_index = 0; // arm 1: the banner goes
        g.kingdom.counties[county as usize].owner = 0; // our owner marker's colour
        g.kingdom.counties[county as usize].mercenary_offer = 1; // arm 2: a band appears
        if let Some(t) = pasture.filter(|_| with_herd) {
            g.kingdom.campaign.map.terrain[t] = 0x16; // arm 4: a crowded herd
        }
        g.kingdom
            .campaign
            .units
            .spawn(l2_kingdom::Unit::new(l2_kingdom::UnitKind::Army, 2, tx, ty))
            .expect("a free unit slot");
    };

    let mut lit = game.clone();
    lit.kingdom.options.exploration = false;

    let (screen, before) = paint_at(&mut game, &assets, tx, ty);
    give_away(&mut game, true);
    let (_, after) = paint_at(&mut game, &assets, tx, ty);
    assert_eq!(
        map_pixels_differ(&before, &after, screen.map_clip()),
        0,
        "county {county} is unseen, and something of it was drawn on the map"
    );

    let (screen, before) = paint_at(&mut lit, &assets, tx, ty);
    give_away(&mut lit, false);
    let (_, after) = paint_at(&mut lit, &assets, tx, ty);
    assert!(
        map_pixels_differ(&before, &after, screen.map_clip()) > 0,
        "the control: with the fog off the same changes are on the map"
    );
}

// --- a unit walking across a tile, and the balls in front of it ---------------
//
// A player, on build `A5B112C2B`: *"The army marching animation is jumping from
// square to square, I remember there being an animation and some interpolation
// between walking squares."* And on `73DF34969`: *"The balls of the army
// movement are missing the gold ball of action, it's just a grey ball like I
// can't get there when I attack a town."*
//
// Every expected number below is a literal out of `Lords2.exe` — the walk
// tables at `0x004D8108`/`0x004D8188` and `0x004D8308`/`0x004D8388`,
// `g_unitWalkFrames`, `Map_DrawPathMarker`'s `(0x14, 6)` and `0x4E` — and none
// is computed from the constant it is checking.
//
// **Ablations, run on this branch** — each line changed, and what went red:
//
// | changed | red |
// |---|---|
// | `walk: (0, 0)` in `map.rs`'s `unit_sprite` | `a_marching_army_…` at tick 1 |
// | `UnitFrames::frame` always answering from the record (no one-tick lag) | `a_marching_army_…` at tick 1 (75 → 81) |
// | `sprite_frame(0)` for `walk_phase()` in `UnitFrames::written` | `a_marching_army_…` at tick 6 (82 → 81) |
// | `movement::step` stopping the unit on the commit that empties its path | `a_marching_army_…` at tick 1 (`moving` already false), `units_tick::tests::the_wait_follows_the_units` (17 against 25) |
// | the `hides_tile` test in `draw_units` | `a_unit_walking_into_the_dark_…` |
// | `action` passed as `false` to `path_marker_frame` | `a_march_hovered_onto_an_enemy_town_…` |
// | the centred placement `draw_path_marker` used to have | `a_march_hovered_onto_an_enemy_town_…` |

/// Whether every pixel a frame paints stands, exactly, with its top-left at `at`.
fn ink_at(canvas: &Canvas, art: &l2_formats::pl8::DecodedFrame, at: (i32, i32)) -> bool {
    let (w, h) = (art.width as i32, art.height as i32);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if !art.opaque[i] {
                continue;
            }
            let (px, py) = (at.0 + x, at.1 + y);
            if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                return false;
            }
            if canvas.at(px as usize, py as usize) != art.indices[i] {
                return false;
            }
            any = true;
        }
    }
    any
}

/// Where `Map_DrawArmies` stands a figure **at rest** on a tile: the tile
/// origin, plus `(g_mapTileHalfStep, g_mapHalfPitch)`, plus the kind's nudge,
/// then `x -= w/2; y -= h`. Nothing here reads a walk table.
fn at_rest(screen: &MapScreen, tile: (u8, u8), nudge: (i32, i32), art: &l2_formats::pl8::DecodedFrame) -> (i32, i32) {
    let (row, col) = campaign::tile_to_cell(tile.0 as usize, tile.1 as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let z = screen.zoom();
    (sx + z.half_pitch + nudge.0 - art.width as i32 / 2, sy + z.half_pitch + nudge.1 - art.height as i32)
}

/// The first tile, rows then columns well inside the map, whose **east**
/// neighbour is open ground too and for which `ok(from, to)` holds — open
/// meaning no plane-0 bit a step or a painter reads, so a unit crosses it at the
/// open-ground pace and nothing but the unit is drawn over it — with no unit
/// within three tiles of either.
fn open_step_east(game: &Game, ok: impl Fn(usize, usize) -> bool) -> Option<((u8, u8), (u8, u8))> {
    use l2_kingdom::map::{flags, index};
    let map = &game.kingdom.campaign.map;
    let busy = flags::IMPASSABLE | flags::ROAD | flags::PLOT | flags::FARMLAND | flags::CASTLE | flags::SETTLEMENT;
    let open = |x: u8, y: u8| {
        let t = index(x, y);
        map.county[t] != 0 && map.flags[t] & busy == 0
    };
    for y in 16..48u8 {
        for x in 16..47u8 {
            if !open(x, y) || !open(x + 1, y) || !ok(index(x, y), index(x + 1, y)) {
                continue;
            }
            let crowded = game.kingdom.campaign.units.iter().any(|(_, u)| {
                (u.y as i32 - y as i32).abs() <= 3 && (u.x as i32 - x as i32).abs() <= 4
            });
            if !crowded {
                return Some(((x, y), (x + 1, y)));
            }
        }
    }
    None
}

/// **An army part-way across a tile is drawn where the original draws it, with
/// the frame the original draws it with, at both zooms — tick by tick, for one
/// whole open-ground crossing.**
///
/// The army starts facing north on a tile of the person's own county and is
/// ordered one tile east. What the binary says happens, and what each row of
/// `MARCH` below asserts:
///
/// * **Tick 1 commits the tile.** `Unit_Spawn` leaves the latch set, so the
///   first tick enters `(x + 1, y)`, turns the unit east and writes `+0x149 =
///   1`. The figure is drawn **at the new tile, dragged back by index 1** —
///   `(−28, −14)` near, `(−5, −2)` far — which stands it two pixels right and
///   one down of where it stood at rest on the tile it left. **And still facing
///   north**, frame 75: `Army_Tick` wrote `+0x07` before `Unit_Step` turned it.
/// * **Open ground admits one tick in four**, and each admission adds 2, so the
///   figure moves on ticks 5, 9 … 29 through indices 3 … 15 and its frame
///   follows **one tick later**, through `g_unitWalkFrames` = `0, 1, 2, 1, 0, 1,
///   2, 1`: 81, 82, 83, 82, 81, 82, 83, 82.
/// * **Tick 33 reaches the tile's edge**, `+0x149` goes back to 0 and, the path
///   being empty, the unit stops — standing exactly on its tile — and tick 34 is
///   the standing frame, 81.
///
/// **What makes each assertion the claim.** The figure is found by its own ink —
/// every opaque palette index of the frame, at one exact position — among all
/// four frames it could be, so a wrong frame at the right place and the right
/// frame at the wrong place both fail. Both tiles are asserted seen with the fog
/// **on**, and the whole figure inside the map viewport, on every tick: C138 was
/// Assertion about an army not on screen.
#[test]
fn a_marching_army_is_drawn_part_way_across_its_tile_with_the_originals_walk_frames() {
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let player = game.player;
    let ((fx, fy), (tx, ty)) = {
        let g = &game;
        open_step_east(g, |from, to| {
            let map = &g.kingdom.campaign.map;
            g.is_players(map.county[from])
                && map.county[from] == map.county[to]
                && !g.hides_tile(from)
                && !g.hides_tile(to)
        })
        .expect("the person's county has two open, seen tiles side by side")
    };
    let mut army = Unit::new(UnitKind::Army, player, fx, fy);
    army.men = 100; // under 301: the first bank, 0x48
    army.troops[0] = 100;
    army.county = game.kingdom.campaign.map.county_at(fx, fy);
    army.owner_is_human = true;
    army.facing = 0; // north, so the first commit is also a turn
    let id = game.kingdom.campaign.units.spawn(army).expect("a free unit slot");
    assert_eq!(game.order_unit_move(id, (tx, ty)), Some(1), "one step east");

    let mut near = MapScreen::new();
    draw(&mut near, &mut game, &assets);
    near.centre_on_tile(tx as usize, ty as usize);
    let mut far = MapScreen::new();
    draw(&mut far, &mut game, &assets);
    send(&mut far, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    assert_eq!(far.zoom().id, campaign::FAR.id, "the second screen is at the far zoom");
    for (screen, what) in [(&near, "near"), (&far, "far")] {
        assert!(
            campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize).is_some(),
            "{what}: the tile walked into is in view"
        );
    }
    let frame = |z: &campaign::Zoom, n: usize| {
        assets
            .map
            .sprite_sheet(z, 0)
            .and_then(|s| s.frame(n))
            .unwrap_or_else(|| panic!("zoom {}: sprite sheet A frame {n}", z.id))
    };

    // `0x48 + 3 * ((facing + 1) & 7) + g_unitWalkFrames[phase]`, typed.
    const NORTH: usize = 75; // facing 0, phase 0
    const STAND: usize = 81; // facing 2, walk frame 0
    const MID: usize = 82; //   facing 2, walk frame 1
    const STRIDE: usize = 83; // facing 2, walk frame 2
    // (tick, +0x149 after it, near offset, far offset, frame drawn)
    #[rustfmt::skip]
    let march: [(u32, u8, (i32, i32), (i32, i32), usize); 34] = [
        (1, 1, (-28, -14), (-5, -2), NORTH), (2, 1, (-28, -14), (-5, -2), STAND),
        (3, 1, (-28, -14), (-5, -2), STAND), (4, 1, (-28, -14), (-5, -2), STAND),
        (5, 3, (-24, -12), (-5, -2), STAND), (6, 3, (-24, -12), (-5, -2), MID),
        (7, 3, (-24, -12), (-5, -2), MID), (8, 3, (-24, -12), (-5, -2), MID),
        (9, 5, (-20, -10), (-4, -2), MID), (10, 5, (-20, -10), (-4, -2), STRIDE),
        (11, 5, (-20, -10), (-4, -2), STRIDE), (12, 5, (-20, -10), (-4, -2), STRIDE),
        (13, 7, (-16, -8), (-3, -1), STRIDE), (14, 7, (-16, -8), (-3, -1), MID),
        (15, 7, (-16, -8), (-3, -1), MID), (16, 7, (-16, -8), (-3, -1), MID),
        (17, 9, (-12, -6), (-2, -1), MID), (18, 9, (-12, -6), (-2, -1), STAND),
        (19, 9, (-12, -6), (-2, -1), STAND), (20, 9, (-12, -6), (-2, -1), STAND),
        (21, 11, (-8, -4), (-1, 0), STAND), (22, 11, (-8, -4), (-1, 0), MID),
        (23, 11, (-8, -4), (-1, 0), MID), (24, 11, (-8, -4), (-1, 0), MID),
        (25, 13, (-4, -2), (-1, 0), MID), (26, 13, (-4, -2), (-1, 0), STRIDE),
        (27, 13, (-4, -2), (-1, 0), STRIDE), (28, 13, (-4, -2), (-1, 0), STRIDE),
        (29, 15, (0, 0), (0, 0), STRIDE), (30, 15, (0, 0), (0, 0), MID),
        (31, 15, (0, 0), (0, 0), MID), (32, 15, (0, 0), (0, 0), MID),
        (33, 0, (0, 0), (0, 0), MID), (34, 0, (0, 0), (0, 0), STAND),
    ];
    let candidates = [NORTH, STAND, MID, STRIDE];

    for &(tick, step, near_walk, far_walk, drawn) in &march {
        l2_game::turn::tick_units_only(&mut game);
        let u = game.kingdom.campaign.units.get(id).expect("the army");
        assert_eq!((u.tile(), u.sub_tile), ((tx, ty), step), "tick {tick}: where the simulation has it");
        assert_eq!(u.moving, tick < 33, "tick {tick}: walking until the last tile is crossed, and no longer");
        assert!(!game.hides_tile(l2_kingdom::map::index(tx, ty)), "tick {tick}: the tile is seen");

        let canvas = draw(&mut near, &mut game, &assets);
        let place = |n: usize| {
            let art = frame(&campaign::NEAR, n);
            let (rx, ry) = at_rest(&near, (tx, ty), (0, -4), &art);
            ((rx + near_walk.0, ry + near_walk.1), art)
        };
        let found: Vec<usize> =
            candidates.iter().copied().filter(|&n| { let (at, art) = place(n); ink_at(&canvas, &art, at) }).collect();
        assert_eq!(found, vec![drawn], "tick {tick}, +0x149 = {step}: the near figure, {near_walk:?} from rest");
        let ((ox, oy), art) = place(drawn);
        let clip = near.map_clip();
        assert!(
            clip.contains(ox, oy) && clip.contains(ox + art.width as i32 - 1, oy + art.height as i32 - 1),
            "tick {tick}: the whole figure is inside the map viewport"
        );
        if tick == 1 {
            let (bx, by) = at_rest(&near, (fx, fy), (0, -4), &art);
            assert_eq!(
                (ox, oy),
                (bx + 2, by + 1),
                "the first sub-step stands two right and one down of rest on the tile it left: 28 and 14 against 30 and 15"
            );
        }

        let canvas = draw(&mut far, &mut game, &assets);
        let art = frame(&campaign::FAR, drawn);
        let (rx, ry) = at_rest(&far, (tx, ty), (0, -4), &art);
        assert!(
            ink_at(&canvas, &art, (rx + far_walk.0, ry + far_walk.1)),
            "tick {tick}, +0x149 = {step}: the far figure, {far_walk:?} from rest"
        );
    }
}

/// **`Map_DrawArmies`' fog test is on the tile the unit is walking *into*.**
///
/// `Unit_MoveInFacing` (`0x00466D84`) unlinks the unit from the tile it is
/// leaving and links it to the next one **at the commit**, before a single
/// sub-step of the crossing is drawn, and `Map_DrawArmies` is reached from the
/// render pass of the tile the unit is linked to, behind that tile's seen bit.
/// So a unit walking out of sight vanishes on the tick it commits — though its
/// figure would have stood a whole tile back, over ground the person can see —
/// and one walking into sight is drawn for the whole crossing, over the dark.
///
/// A merchant, because a merchant walks blind: `Unit_Step` reveals round an
/// **army** only, so its own step cannot light the tile it walks into — and a
/// person's army never walks into the dark at all, because it lit the square
/// round the tile it is leaving before it left.
///
/// Stated as equalities: at every sub-step of the crossing, the map viewport
/// with the merchant and without it. **The control** lights the tile it walked
/// into and requires the same merchant, at the same sub-steps, to be on the map.
#[test]
fn a_unit_walking_into_the_dark_is_hidden_for_its_whole_crossing_and_one_walking_out_is_not() {
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let (lit, dark) = {
        let g = &game;
        open_step_east(g, |from, to| !g.hides_tile(from) && g.hides_tile(to))
            .expect("a seen tile with an unseen one east of it")
    };

    // (from, to, whether the crossing is drawn)
    for (from, to, shown) in [(lit, dark, false), (dark, lit, true)] {
        let mut g = game.clone();
        let mut trader = Unit::new(UnitKind::Merchant, l2_kingdom::units_tick::OWNERLESS, from.0, from.1);
        trader.county = g.kingdom.campaign.map.county_at(from.0, from.1);
        let id = g.kingdom.campaign.units.spawn(trader).expect("a free unit slot");
        let map = g.kingdom.campaign.map.clone();
        l2_kingdom::movement::order_move(&map, &mut g.kingdom.campaign.units, id, to, l2_kingdom::movement::Routing::Direct)
            .expect("one step");

        let mut last = None;
        let mut sub_steps = 0;
        for tick in 1..=40 {
            l2_game::turn::tick_units_only(&mut g);
            let u = g.kingdom.campaign.units.get(id).expect("the merchant");
            if !u.moving {
                break;
            }
            assert_eq!(u.tile(), to, "tick {tick}: committed onto the tile it is walking into");
            if last == Some(u.sub_tile) {
                continue;
            }
            last = Some(u.sub_tile);
            sub_steps += 1;
            let step = u.sub_tile;

            let mut seen = [0usize; 2];
            for (i, light) in [false, true].into_iter().enumerate() {
                let mut h = g.clone();
                if light {
                    h.kingdom.campaign.explored.reveal_square(h.player, to.0 as i32, to.1 as i32, 0);
                }
                if !shown && !light {
                    assert!(h.hides_tile(l2_kingdom::map::index(to.0, to.1)), "the tile walked into is dark");
                }
                let (screen, with) = paint_at(&mut h, &assets, to.0, to.1);
                h.kingdom.campaign.units.remove(id).expect("the merchant");
                let (_, without) = paint_at(&mut h, &assets, to.0, to.1);
                seen[i] = map_pixels_differ(&with, &without, screen.map_clip());
            }
            if shown {
                assert!(seen[0] > 0, "+0x149 = {step}: walking out of the dark into sight, it is drawn over the dark");
            } else {
                assert_eq!(seen[0], 0, "+0x149 = {step}: walking into the dark, not one pixel of it is drawn");
                assert!(seen[1] > 0, "+0x149 = {step}, the control: with that tile seen, the same merchant is on the map");
            }
        }
        assert_eq!(sub_steps, 8, "a crossing is eight sub-steps: +0x149 = 1, 3 … 15");
    }
}

/// **The gold ball of action, where the original puts it — and grey only where
/// the original greys.** Driven through the screen stack: arrows to scroll,
/// a click on the army, the pointer over an enemy county's town.
///
/// `Map_DrawPathMarker` (`0x004081A6`):
///
/// ```c
/// local_14 = flags & 0x50;
/// if ((flags & 0x80) != 0 && content != 0x14) local_14 = 1;
/// n = g_moveDistLocal[tile] - 1;  if (allowance - used < n) n = 0;
/// frame = local_14 == 0 ? 0x38 + n : 0x4E;
/// g_drawX += 0x14;  g_drawY += 6;                   /* no centring */
/// ```
///
/// A town costs 100 to enter, so its own cost is always past the budget and the
/// cost arm greys it — the player's *"just a grey ball like I can't get there"*
/// — but the town's flag bit is tested first. The army stands two or three tiles
/// from the town on plain ground, so every ball before the town is a cost ball,
/// and the route is asserted twice: with a full allowance (each step is `0x38 +
/// its cost`, the town gold) and with no moves left (each step grey, **the town
/// still gold**). The costs come from the flood fill, which is not what is under
/// test; the frame arithmetic and the `(0x14, 6)` are, and they are typed.
///
/// **Not asserted, and why:** that an enemy *army* draws an ordinary cost ball.
/// The binary says it does — `local_14` reads no unit — but the army's own
/// figure is drawn after the balls and stands over its tile's ball, so no pixel
/// of it can be seen.
#[test]
fn a_march_hovered_onto_an_enemy_town_ends_in_the_gold_ball_and_only_steps_out_of_reach_are_grey() {
    use l2_kingdom::map::{coords, flags, index, MAP_TILES};
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let player = game.player;
    let map = game.kingdom.campaign.map.clone();
    // Plain: no bit `local_14` reads, and walkable.
    let plain = |t: usize| {
        map.county[t] != 0
            && map.flags[t] & (flags::IMPASSABLE | flags::CASTLE | flags::PLOT | flags::SETTLEMENT) == 0
    };
    // An enemy county's town tile, and a stand two or three tiles from it whose
    // route there crosses only plain ground inside the army's fifteen moves.
    let cost = map.cost_map();
    let mut chosen = None;
    'search: for t in 0..MAP_TILES {
        let (x, y) = coords(t);
        if !(8..56).contains(&x) || !(8..56).contains(&y) || map.flags[t] & flags::CASTLE == 0 {
            continue;
        }
        let owner = game.kingdom.counties.get(map.county[t] as usize).map_or(0, |c| c.owner);
        if owner == 0 || owner == player {
            continue;
        }
        #[rustfmt::skip]
        let offsets = [(-3i32, 0i32), (-3, 3), (0, 3), (3, 3), (3, 0), (3, -3), (0, -3), (-3, -3),
                       (-2, 0), (-2, 2), (0, 2), (2, 2), (2, 0), (2, -2), (0, -2), (-2, -2)];
        for (dx, dy) in offsets {
            let (sx, sy) = ((x as i32 + dx) as u8, (y as i32 + dy) as u8);
            if !plain(index(sx, sy)) {
                continue;
            }
            let fill = l2_kingdom::movement::flood_fill(&cost, (sx, sy), l2_kingdom::movement::Routing::Direct);
            let Some(path) = l2_kingdom::movement::extract_path(&cost, &fill, (x, y)) else { continue };
            let steps: Vec<((u8, u8), i32)> = path
                .iter()
                .filter(|&&p| p != (x, y))
                .map(|&p| (p, fill.cost_to(p.0, p.1).unwrap_or(99)))
                .collect();
            if path.last() == Some(&(x, y))
                && !steps.is_empty()
                && steps.iter().all(|&(p, c)| plain(index(p.0, p.1)) && (1..=15).contains(&c))
            {
                chosen = Some(((x, y), (sx, sy), steps));
                break 'search;
            }
        }
    }
    let (town, stand, steps) = chosen.expect("an enemy county's town a short plain march from somewhere");

    // Nothing else of the world's may stand over the balls.
    let crowd: Vec<usize> = game
        .kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| (u.x as i32 - town.0 as i32).abs() <= 5 && (u.y as i32 - town.1 as i32).abs() <= 5)
        .map(|(id, _)| id)
        .collect();
    for id in crowd {
        game.kingdom.campaign.units.remove(id);
    }
    let mut army = Unit::new(UnitKind::Army, player, stand.0, stand.1);
    army.men = 300;
    army.troops[0] = 300;
    army.county = map.county_at(stand.0, stand.1);
    army.owner_is_human = true;
    let army = game.kingdom.campaign.units.spawn(army).expect("a free unit slot");

    // The machine's map screen, and a second one as its ruler: both open on
    // the person's town and are scrolled by the same keys.
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ruler = MapScreen::new();
    draw_stack(&mut m, &mut game, &assets);
    draw(&mut ruler, &mut game, &assets);
    let want = campaign::Viewport::centred_on_tile(town.0 as usize, town.1 as usize, ruler.zoom());
    for _ in 0..200 {
        let v = ruler.viewport();
        let key = if v.col < want.col {
            Key::Right
        } else if v.col > want.col {
            Key::Left
        } else if v.row + 1 < want.row {
            Key::Down
        } else if v.row > want.row + 1 {
            Key::Up
        } else {
            break;
        };
        send_stack(&mut m, &mut game, &assets, Event::KeyDown(key));
        send(&mut ruler, &mut game, &assets, Event::KeyDown(key));
        if ruler.viewport() == v {
            break;
        }
    }
    let centre = |t: (u8, u8)| {
        campaign::tile_centre(ruler.viewport(), ruler.zoom(), t.0 as usize, t.1 as usize)
            .unwrap_or_else(|| panic!("{t:?} is in view"))
    };
    let (ax, ay) = centre(stand);
    let (hx, hy) = centre(town);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: ax, y: ay });
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: hx, y: hy });

    let sheet = assets.map.flag_sheet(&campaign::NEAR).expect("Flags1a.pl8");
    let ball = |n: usize| sheet.frame(n).unwrap_or_else(|| panic!("Flags1a.pl8 frame {n:#x}"));
    // `g_drawX + 0x14`, `g_drawY + 6` from the tile origin. Typed.
    let at = |t: (u8, u8)| {
        let (row, col) = campaign::tile_to_cell(t.0 as usize, t.1 as usize);
        let (x, y) = campaign::cell_to_screen(ruler.viewport(), ruler.zoom(), row, col);
        (x + 20, y + 6)
    };
    const GREY: usize = 0x38;
    const GOLD: usize = 0x4E;

    // The army's own figure is drawn after the balls and stands over those of
    // the tiles behind it: they cannot be seen, so they are not asked about.
    let figure = assets.map.sprite_sheet(&campaign::NEAR, 0).and_then(|s| s.frame(0x48)).expect("an army frame");
    let (fx0, fy0) = at_rest(&ruler, stand, (0, -4), &figure);
    let clear = |t: (u8, u8)| {
        let ((bx, by), b) = (at(t), ball(GREY));
        bx + b.width as i32 <= fx0
            || bx >= fx0 + figure.width as i32
            || by + b.height as i32 <= fy0
            || by >= fy0 + figure.height as i32
    };
    let visible: Vec<((u8, u8), i32)> = steps.iter().copied().filter(|&(t, _)| clear(t)).collect();
    assert!(!visible.is_empty(), "at least one step's ball is clear of the army's figure: {steps:?}");

    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(ink_at(&canvas, &ball(GOLD), at(town)), "the enemy town {town:?} ends the route in frame 0x4E, at (+0x14, +6)");
    assert!(!ink_at(&canvas, &ball(GREY), at(town)), "and not in the grey ball");
    for &(t, c) in &visible {
        assert!(ink_at(&canvas, &ball(GREY + c as usize), at(t)), "the step at {t:?}, cost {c} of 15, is frame 0x38 + {c}");
        assert!(!ink_at(&canvas, &ball(GREY), at(t)), "and not grey: {t:?} is in reach");
    }

    game.kingdom.campaign.units.get_mut(army).expect("the army").moves_used = 15;
    let canvas = draw_stack(&mut m, &mut game, &assets);
    for &(t, c) in &visible {
        assert!(ink_at(&canvas, &ball(GREY), at(t)), "with no moves left the step at {t:?}, cost {c}, is grey");
    }
    assert!(
        ink_at(&canvas, &ball(GOLD), at(town)),
        "and the town is still gold — `local_14` is tested before the cost is looked at"
    );
}

/// **`FUN_0043CAF4`'s extra step: the right button also selects a county.**
///
/// ```c
/// if ((g_pickedTileCounty != 0) && (g_pickedTileCounty != g_selectedCounty) &&
///     (g_pickedTileUnit == 0)) {
///   DAT_0053f0dc = g_counties[g_pickedTileCounty].townTile;
///   if (DAT_0053f0dc != 0) {
///     g_selectedCounty = g_pickedTileCounty;
///     Map_CentreOnTile(DAT_0053f0dc);
///   }
///   DAT_004eb260 = 1; FUN_004050c0();
/// }
/// FUN_0041b032();
/// ```
///
/// Both halves of the third guard are asserted: bare ground of another county
/// selects it and recentres on that county's **town**, and a unit standing on
/// ground of a third county selects nothing, because the panel that comes up
/// is about the army. The information panel opens either way.
///
/// Ablated, each red on its own assertion: the whole block removed — the
/// selection stays where it started; the `g_pickedTileUnit == 0` guard dropped
/// — the unit's county is selected where nothing should move.
#[test]
fn right_clicking_another_countys_ground_selects_it_and_a_unit_on_it_does_not() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let here = game.selected;
    assert!(here != 0, "setup: the game opens with a county selected");

    // Another county with a town square, and a tile of it nothing stands on.
    let empty_tile = |game: &Game, id: u8| {
        let map = &game.kingdom.campaign.map;
        (0..map.county.len()).find(|&t| {
            let (x, y) = l2_kingdom::map::coords(t);
            map.county[t] == id && game.kingdom.campaign.units.at(x, y).is_none()
        })
    };
    let (there, tile, town) = (1..=game.kingdom.county_count as u8)
        .filter(|&id| id != here)
        .find_map(|id| {
            let ctx = Ctx { game: &mut game, assets: &assets };
            let town = *MapScreen::town(&ctx, id).first()?;
            Some((id, empty_tile(&game, id)?, town))
        })
        .expect("some other county has a town and a tile with nobody on it");

    let (x, y) = on_screen(&mut screen, tile);
    let opened = send(&mut screen, &mut game, &assets, Event::RightClick { x, y });
    assert_eq!(
        opened,
        Transition::Push(ScreenId::Info(l2_game::screens::info::Target::Tile(tile))),
        "the right click still opens the information panel on the tile"
    );
    assert_eq!(game.selected, there, "and it selected the county the tile belongs to");
    // `Map_CentreOnTile(townTile)` — the town, not the tile that was clicked.
    let (tx, ty) = l2_kingdom::map::coords(town);
    assert!(
        campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize).is_some(),
        "the view moved to county {there}'s town square"
    );

    // **The third guard.** Put an army on ground of a third county and right
    // click it: the unit half comes up and the selection does not move.
    let (elsewhere, ground) = (1..=game.kingdom.county_count as u8)
        .filter(|&id| id != there)
        .find_map(|id| Some((id, empty_tile(&game, id)?)))
        .expect("a third county with an empty tile");
    // **A merchant, because the fixture has six and no army** — the guard is
    // `g_pickedTileUnit != 0` and says nothing about the kind.
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");
    let (gx, gy) = l2_kingdom::map::coords(ground);
    {
        let u = game.kingdom.campaign.units.get_mut(unit).expect("the merchant");
        u.x = gx;
        u.y = gy;
        u.county = elsewhere;
    }
    let (x, y) = on_screen(&mut screen, ground);
    let opened = send(&mut screen, &mut game, &assets, Event::RightClick { x, y });
    assert_eq!(
        opened,
        Transition::Push(ScreenId::Info(l2_game::screens::info::Target::Unit(unit))),
        "the right click opens the unit half"
    );
    assert_eq!(game.selected, there, "and a unit under the cursor selects nothing");
}
