//! The three screens, drawn and driven against a real install — headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game
//! ```
//!
//! **No window is opened.** Every assertion is on the canvas's `Vec<u8>` of
//! palette indices, which is the same shape the eventual pixel diff against
//! `Lords2.exe`'s framebuffer will take. A screen that had to be looked at to
//! be checked would be a screen that stops being checked.
//!
//! Several tests below read text back off the canvas with [`find_text`], which
//! renders the string it is looking for and searches for that exact pattern of
//! ink. It is paired every time with a near-miss that must *not* be found, so
//! "the panel shows 435" cannot pass by finding some other number.

use std::path::PathBuf;

use l2_game::game::{Assets, MAX_TAX_RATE};
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::county::{self as county, CountyScreen, Panel};
use l2_game::screens::map::{self, MapScreen};
use l2_game::screens::village::{self as village_screen, VillageScreen};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::village;
use l2_view::{text, Canvas};

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
        // The **assets** come from the install and the **position** comes from
        // the named fixture. They used to come from the same place, and every
        // number below - fourteen counties, the treasury, the selected county -
        // is the England turn-one position's rather than any save's.
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

fn send<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets, e: Event) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(e, &mut ctx)
}

/// Find a string drawn in `colour`, returning its top-left. Only the glyphs'
/// *set* pixels are matched; what is behind the letters is the panel's
/// business.
fn find_text(canvas: &Canvas, s: &str, colour: u8) -> Option<(i32, i32)> {
    let w = text::width(s);
    let h = text::GLYPH_H;
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    let mut probe = Canvas::new(w.max(1) as usize, h as usize);
    text::draw(&mut probe, 0, 0, s, 1);
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            // Cheap rejection on the first ink pixel before the full compare.
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// The same search, but for a string drawn in one of the **original's** fonts.
///
/// The county strip is drawn in `Fntl2_9.pl8` — the only place in the game that
/// font is used — so a search that only knows our own 5 × 7 glyphs cannot see
/// it. The probe is the same idea: render the string, keep the set pixels,
/// scan for that pattern.
fn find_font_text(
    canvas: &Canvas,
    font: &l2_game::shell::font::Font,
    s: &str,
    colour: u8,
) -> Option<(i32, i32)> {
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let w = font.width(s).max(1);
    let h = font.height(s).max(1);
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// One line of the county strip, found in whichever font actually drew it: the
/// original's 9-pixel one where the install has it, ours where it does not.
fn find_strip(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.small.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

/// One line of the strip drawn in the **body** font — the county's name, and
/// the three "sovereign land of …" lines on a county you do not hold.
fn find_body(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.body.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

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

/// Find a pixel belonging to a county, by scanning the pick plane rather than
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
    // number rather than a range: **two** of England's fourteen counties are on
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
/// Both are measured here rather than described: the town has a pixel, the
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
    // rectangle with a pixel of slack rather than a guess.
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

/// **A merchant is drawn, and clicking one opens the merchant.** C50.
///
/// The player: *"I don't see the merchants on the map and of course I can't
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

    // And clicking it opens screen 0x08.
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Push(ScreenId::Shell(0x08)), "the merchant screen");

    // The same merchant in somebody else's county is a refusal, not a trade.
    let theirs = (1..=game.kingdom.county_count as u8)
        .find(|&id| id != mine && !game.is_players(id))
        .expect("England has counties the player does not own");
    game.kingdom.campaign.units.get_mut(merchant).expect("the merchant").county = theirs;
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: cx, y: cy });
    assert_eq!(t, Transition::Stay, "a merchant in a county you do not own opens nothing");
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
    // At the far zoom's pinned origin all fourteen are reachable, which is why
    // the original disables scrolling there rather than leaving it stranded.
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

/// A click picks the county the player can actually see at that pixel, and a
/// second click on the same county opens it. The pixel is found through the
/// pick plane, so this exercises exactly the path the mouse takes.
#[test]
fn clicking_a_county_selects_it_and_clicking_it_again_opens_its_panel() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);

    // Whichever county the opening viewport happens to show most of.
    let counts = pick_counts(&screen);
    let target = (1..=14u8).max_by_key(|&id| counts[id as usize]).expect("a county is visible");
    assert!(counts[target as usize] > 0, "the opening view shows at least one county");
    let (px, py) = pixel_of(&screen, target).expect("and it has a pixel");

    game.select(0);
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Stay, "the first click only selects");
    assert_eq!(game.selected, target);

    let t = send(&mut screen, &mut game, &assets, Event::Click { x: px, y: py });
    assert_eq!(t, Transition::Push(ScreenId::County(target, Panel::Tax)), "the second click opens it");

    // A click on the sea clears the selection rather than picking a county at
    // random.
    let (sx, sy) = pixel_of(&screen, 0).expect("there is sea");
    send(&mut screen, &mut game, &assets, Event::Click { x: sx, y: sy });
    assert_eq!(game.selected, 0);
}

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
/// differs by about the area of four tiles rather than by the whole screen.
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

/// The minimap is the original's own raster out of `MAPnn.PL8`, and clicking it
/// selects the county under the pixel *and* moves the viewport onto it.
///
/// This is the one place we use the original's algorithm and not just reach its
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
    // means, and is not implied by the origin merely changing.
    draw(&mut screen, &mut game, &assets);
    assert!(pick_counts(&screen)[county as usize] > 0, "county {county} is now in view");
}

/// The selection is visible: outlining a county changes the picture, and
/// outlining a different one changes it differently.
#[test]
fn the_selected_county_is_outlined_on_the_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();

    game.select(0);
    let none = draw(&mut screen, &mut game, &assets);
    game.select(8);
    let eight = draw(&mut screen, &mut game, &assets);
    game.select(11);
    let eleven = draw(&mut screen, &mut game, &assets);

    assert!(none.diff_count(&eight) > 100, "an outline must be visible");
    assert!(eight.diff_count(&eleven) > 100, "and it must follow the selection");
    assert!(
        eight.count(assets.ink.highlight) > none.count(assets.ink.highlight),
        "the outline is drawn in the highlight colour"
    );
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

    let clock = find_text(&canvas, "WINTER 1268", ink.text).expect("the season and the year");
    assert_eq!(clock, (360, 6), "the original draws the clock at x 360, y 6");
    let gold = find_text(&canvas, "GOLD 1000", ink.text).expect("the treasury");
    assert_eq!(gold, (500, 6), "and the treasury at x 500");
    assert!(find_text(&canvas, "TURN 1", ink.dim).is_some());
    assert!(find_text(&canvas, "COUNTIES 1/14", ink.dim).is_some());

    // **The county strip, in the map's own sidebar.** `Screen_DrawCampaign`
    // calls `CountyStrip_Draw` — the map screen used to leave that plate empty
    // and write a box of our own numbers over the jobs plate below it.
    let pop = find_strip(&canvas, &assets, "435", ink.text).expect("the population");
    assert_eq!(pop, (508, 189), "at CountyStrip_Draw's own coordinates");
    assert_eq!(
        find_strip(&canvas, &assets, "72", ink.text),
        // **Left-aligned at 602, not right-anchored on it.** `Ui_DrawNumber`
        // has no anchoring argument: the population's call and the happiness's
        // differ only in their value and their x, so both are left origins.
        // See `docs/decisions.md` C42.
        Some((602, 189)),
        "and the happiness beside it"
    );

    // The near-misses. If the search could match anything it would match these.
    assert!(find_text(&canvas, "WINTER 1269", ink.text).is_none());
    assert!(find_text(&canvas, "GOLD 1001", ink.text).is_none());
    assert!(find_strip(&canvas, &assets, "436", ink.text).is_none());
}

/// **The four county panels are reachable from the map, and each from its own
/// quadrant.** `CountyStrip_Click` is the whole navigation into them; the map
/// screen used to carry one button of ours instead, which opened the county
/// screen on whichever panel it defaulted to — so a player could reach the tax
/// panel and no other.
#[test]
fn each_quadrant_of_the_strip_opens_its_own_panel_from_the_campaign_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    for panel in county::PANELS {
        let hot = panel.strip_hotspot();
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: hot.centre_x(), y: hot.y + hot.h / 2 },
        );
        assert_eq!(t, Transition::Push(ScreenId::County(8, panel)), "{panel:?}'s own quadrant");
    }
    // The thermometer's dead band opens nothing — the gap exists for the bar.
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: 558, y: 200 });
    assert_eq!(t, Transition::Stay, "the health bar is deliberately not clickable");
}

/// **The five sidebar buttons.** `g_sidebarButtons` (`0x004DC680`) is a table of
/// five, and every one of them sets a `g_screenId` we can draw. They were under
/// a rectangle of ours that opened the county panel and wrote COUNTY PANEL
/// across their artwork.
#[test]
fn the_five_sidebar_buttons_each_open_the_screen_the_original_opens() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    assert!(game.is_players(8), "county 8 is the player's, so the gated three are allowed");
    for b in map::SIDEBAR_BUTTONS {
        let r = b.rect();
        let map::SidebarAction::Screen(id) = b.action;
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        );
        // `map::sidebar_destination` is the one place a graduated screen is
        // named, so this asks it rather than assuming every button is a shell:
        // `0x17` is the raise-army screen now, and it takes the county.
        assert_eq!(
            t,
            Transition::Push(map::sidebar_destination(id, 8)),
            "{} opens {id:#04X}",
            b.name
        );
    }

    // Three of the five are gated on the county being yours, exactly as
    // `Sidebar_Button` gates them. County 1 belongs to realm 5.
    game.select(1);
    for b in map::SIDEBAR_BUTTONS {
        let r = b.rect();
        let map::SidebarAction::Screen(id) = b.action;
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        );
        if matches!(id, 0x17 | 0x18 | 0x1B) {
            assert_eq!(t, Transition::Stay, "{} is refused on another realm's county", b.name);
        } else {
            assert_eq!(t, Transition::Push(ScreenId::Shell(id)), "{} is not gated", b.name);
        }
    }
}

/// **The farm/industry split slider moves peasants.** `FUN_00439122` is the one
/// control on the campaign screen that reallocates labour in bulk, and it was
/// not wired at all — which is a fair part of *"I can't assign peasants"*.
#[test]
fn the_sidebar_split_slider_moves_the_countys_labour_between_farm_and_industry() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let before = game.kingdom.counties[8].industry_share;

    // x 533 on the track is ((533 - 531) * 2) & 0xFC = 4.
    send(&mut screen, &mut game, &assets, Event::Click { x: 533, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 4);
    assert_ne!(4, before, "and that is not where it started");

    // The far end of the track is 100 per cent industry.
    send(&mut screen, &mut game, &assets, Event::Click { x: 581, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 100);

    // The allocation followed it rather than being left describing the old
    // split: with everybody in the mines, the farm jobs empty.
    let farm: i32 = (0..3).map(|j| game.kingdom.counties[8].labour[j]).sum();
    send(&mut screen, &mut game, &assets, Event::Click { x: 531, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 0);
    let farm_after: i32 = (0..3).map(|j| game.kingdom.counties[8].labour[j]).sum();
    assert!(farm_after > farm, "0% industry puts more people on the land than 100% did");
}

/// **The right button has two jobs, and they are opposite ones.**
///
/// A player said *"right click would close a bunch of popups"*, and he is right:
/// `Screen_FrameInput` — the fourth and unnamed `g_screenId` dispatcher, and the one
/// that decides how every screen is *left* — has a right-release arm for almost
/// every screen id there is, and `L2.eng` group 12 index 0 is the game printing
/// *"Click Right to Exit"* on the value spinner.
///
/// On the campaign map it does the reverse: `if (onATile && rightReleased) {
/// g_screenId = 4; FUN_0043CAF4(); }` opens the information panel, which is the
/// pop-up the shipped `Readme.txt` errata describes on an army.
#[test]
fn the_right_button_closes_a_panel_and_opens_the_map_information_screen() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    game.select(8);

    // On the map: right-click on a tile opens screen 0x04.
    let screen = MapScreen::new();
    let (px, py) = (240, 240);
    assert!(screen.map_clip().contains(px, py), "that pixel is on the map");
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::RightClick { x: px, y: py }, &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Shell(0x04)), "the information panel");

    // And right-click again closes it, which is the same arm from the other
    // side: screen 0x04 has its own right-release branch back to the map.
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::RightClick { x: px, y: py }, &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));

    // A county panel closes on the right button from anywhere, the strip
    // included: every guard in the original's chain tests a *left* press or a
    // left release, so none of them consumes a right one.
    for panel in county::PANELS {
        for (x, y) in [(240, 240), (500, 195), (620, 470)] {
            let mut m = Machine::new(ScreenId::County(8, panel));
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.handle(Event::RightClick { x, y }, &mut ctx);
            assert_eq!(m.depth(), 0, "{panel:?} closed by a right click at ({x}, {y})");
        }
    }

    // So does the village and so does the job popup.
    for id in [ScreenId::Village(8), ScreenId::Job(8, 0)] {
        let mut m = Machine::new(id);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::RightClick { x: 200, y: 200 }, &mut ctx);
        assert_eq!(m.depth(), 0, "{id:?} closed by a right click");
    }
}

/// The county strip shows what `CountyStrip_Draw` puts in the 162 × 94 plate,
/// at the coordinates it puts them: population at (508, 189), happiness ending
/// at 602 on the same line, and the tax rate at (506, 226).
///
/// The exact coordinates are the point. A panel that merely *contained* the
/// right digits somewhere would pass a looser test and still be laid out
/// wrongly, which is the mistake this whole task exists to correct.
#[test]
fn the_county_strip_shows_the_saves_numbers_where_the_original_puts_them() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let ink = &assets.ink;

    let c = &game.kingdom.counties[8];
    assert_eq!((c.population, c.happiness, c.ration_achieved), (435, 72, 3));

    assert_eq!(
        find_strip(&canvas, &assets, "435", ink.text),
        Some((508, 189)),
        "the population, at Ui_DrawNumber(pop, ' ', ..., 0x1FC, 0xBD)"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "72", ink.text),
        // **Left-aligned at 602, not right-anchored on it.** `Ui_DrawNumber`
        // has no anchoring argument: the population's call and the happiness's
        // differ only in their value and their x, so both are left origins.
        // See `docs/decisions.md` C42.
        Some((602, 189)),
        "the happiness, right-anchored at 0x25A on the same line"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "0%", ink.text),
        Some((506, 226)),
        "the tax rate at 0x1FA"
    );
    // The county's name comes out of `L2.eng` group 100 at
    // `scenarioIndex * 20 + id` and is drawn in the **body** font, so it is
    // neither of the two the rest of the strip uses.
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 8);
    // Group 100 is twenty strings per map slot: index 0 is the *map's* name
    // ("Here Be Dragons!" for England), 1 … 14 are its fourteen counties and
    // 15 … 19 are the unused `CTY0` padding, so slot 1 starts at index 20 with
    // "The Normans". `scenarioIndex * 20 + countyId` lands on the county's own
    // name with no off-by-one, and county 8 of England is Dyfed.
    assert_eq!(name, "Dyfed", "L2.eng group 100, index map_slot * 20 + 8");
    // rationAchieved == rationWanted, so it is drawn plain rather than red.
    assert!(find_strip(&canvas, &assets, "Normal", ink.text).is_some());
    assert!(find_strip(&canvas, &assets, "Normal", ink.bad).is_none());

    // Near misses, one per number, so none of the three can match by accident.
    assert!(find_strip(&canvas, &assets, "436", ink.text).is_none());
    assert!(find_strip(&canvas, &assets, "73", ink.text).is_none());
    assert!(find_strip(&canvas, &assets, "Double", ink.text).is_none());
}

/// The population panel is `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)` with its rows
/// at the y coordinates `Panel_Population` draws them, and it is reached from
/// the strip's top-left quadrant and from nowhere else.
#[test]
fn the_population_panel_opens_from_its_own_quadrant_and_lays_out_where_it_should() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let ink = &assets.ink;

    let hot = Panel::Population.strip_hotspot();
    send(&mut screen, &mut game, &assets, Event::Click { x: hot.centre_x(), y: hot.y + 4 });
    assert_eq!(screen.panel(), Panel::Population);

    let canvas = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_text(&canvas, "LAST SEASON", ink.text),
        Some((48, 266)),
        "group 73 index 1, at Eng_DrawString(0x49, 1, 0x30, 0x10A)"
    );
    assert_eq!(
        find_text(&canvas, "417", ink.text),
        Some((336 - text::width("417"), 266)),
        "and its value right-anchored at 0x150"
    );
    assert_eq!(find_text(&canvas, "BIRTHS", ink.dim), Some((48, 298)), "0x12A");
    assert_eq!(find_text(&canvas, "DEATHS", ink.dim), Some((48, 314)), "0x13A");
    assert_eq!(find_text(&canvas, "ARMY", ink.dim), Some((48, 330)), "0x14A");
    assert_eq!(find_text(&canvas, "THIS SEASON", ink.highlight), Some((48, 386)), "0x182");
    assert!(find_text(&canvas, "435", ink.highlight).is_some(), "this season's population");

    // The graph is a labelled stub, because g_countyHistory is not simulated.
    assert!(find_text(&canvas, "NOT SIMULATED", ink.bad).is_some());

    // And the tax panel is gone, which is what "one panel at a time" means.
    assert!(find_text(&canvas, "TAX RATE", ink.dim).is_none());
}

/// A zero row draws **no number at all** — `Ui_DrawDelta(value, 0, ...)`
/// returns before it formats anything. This is the detail a reimplementation
/// gets wrong by printing `0`, so it is asserted both ways round.
#[test]
fn a_zero_delta_row_draws_its_label_and_no_number() {
    let (mut game, assets) = world!();
    let ink = &assets.ink;
    let mut screen = CountyScreen::new(8, Panel::Tax);
    screen.open(Panel::Population);

    game.kingdom.counties[8].births = 0;
    let blank = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&blank, "BIRTHS", ink.dim).is_some(), "the label is still drawn");
    assert!(find_text(&blank, "+0", ink.good).is_none(), "and nothing beside it");
    assert!(find_text(&blank, "0", ink.good).is_none());

    game.kingdom.counties[8].births = 63;
    let filled = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_text(&filled, "+63", ink.good),
        Some((336 - text::width("+63"), 298)),
        "a non-zero row draws a signed number in the value column"
    );
    assert!(blank.diff_count(&filled) > 0, "and the two frames differ");
}

/// Setting the tax rate changes the record *and* what is on the screen. Both
/// halves matter: a panel that showed a number it did not set, or set a number
/// it did not show, would pass one of them alone.
#[test]
fn setting_the_tax_rate_changes_the_county_and_the_picture() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let before = draw(&mut screen, &mut game, &assets);

    for _ in 0..7 {
        send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    }
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);

    let after = draw(&mut screen, &mut game, &assets);
    assert!(before.diff_count(&after) > 0);
    assert!(find_text(&after, "7%", assets.ink.highlight).is_some());
    assert!(find_text(&after, "0%", assets.ink.highlight).is_none());

    // Down moves to the ration panel, and Right there does not touch the tax.
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Down));
    assert_eq!(screen.panel(), Panel::Ration);
    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[8].tax_rate, 7);
    assert_eq!(game.kingdom.counties[8].ration_wanted, 4);
}

/// **The tax ceiling is 50, and it is the original's.**
///
/// `Tax_Increase` guards `taxRate < 0x32`; `g_taxHappinessOther` has exactly
/// 51 entries. `docs/screens-county.md` §6.3. The screen stops there and the
/// number on it stops there too — a clamp that the picture disagreed with
/// would be a clamp the player cannot see.
#[test]
fn the_tax_rate_stops_at_the_originals_own_ceiling_of_fifty() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let up = Panel::Tax.increase_button().expect("the tax panel has an up arrow");

    for _ in 0..60 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    }
    assert_eq!(game.kingdom.counties[8].tax_rate, MAX_TAX_RATE);
    assert_eq!(MAX_TAX_RATE, 50, "and the constant is the reading, not a round number");

    let canvas = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&canvas, "50%", assets.ink.highlight).is_some());
    assert!(find_text(&canvas, "51%", assets.ink.highlight).is_none(), "a near miss");
    assert!(find_text(&canvas, "60%", assets.ink.highlight).is_none());
}

/// The ration panel's slider is the third order the original's county panels
/// give, and the one we never had: `Ration_SliderClick` jumps the split to
/// `mouseX - 224`, and the two caps step it by one.
#[test]
fn the_ration_split_slider_sets_the_field_the_original_sets() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    screen.open(Panel::Ration);

    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 37, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 37, "the track jumps to mouseX - 224");

    let down = county::split_down_button();
    send(&mut screen, &mut game, &assets, Event::Click { x: down.centre_x(), y: down.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 36, "the left cap steps down one");

    let up = county::split_up_button();
    for _ in 0..3 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 8 });
    }
    assert_eq!(game.kingdom.counties[8].ration_split, 39);

    // The knob follows: two different splits must not draw the same picture.
    let a = draw(&mut screen, &mut game, &assets);
    game.kingdom.counties[8].ration_split = 90;
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 0, "the knob moves with the value");
}

/// County 1 belongs to realm 5. The strip says so, all four panels still open,
/// and every order is refused — by the mouse as well as by the keyboard.
#[test]
fn another_realms_county_can_be_looked_at_and_not_ordered() {
    let (mut game, assets) = world!();
    assert_eq!(game.kingdom.counties[1].owner, 5);

    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    // `CountyStrip_Draw`'s unowned branch: the name at (480, 180) rather than
    // 165, then group 15's two lines and the owner at 240 / 260 / 280 — all
    // four in the body font and in the owning realm's own colour.
    let realm5 = assets.ink.realm[5];
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);
    assert_eq!(
        find_body(&canvas, &assets, &name, assets.ink.text).map(|p| p.1),
        Some(180),
        "the name sits lower on the 162 x 274 plate"
    );
    assert!(find_body(&canvas, &assets, "REALM 5", realm5).is_some());
    assert!(find_body(&canvas, &assets, "REALM 4", realm5).is_none(), "a near miss");
    assert!(find_body(&canvas, &assets, "693", assets.ink.text).is_none(), "no numbers at all");

    send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Right));
    let up = Panel::Tax.increase_button().expect("the arrows are still drawn");
    send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 4 });
    assert_eq!(game.kingdom.counties[1].tax_rate, 0, "no order lands on another realm's county");

    screen.open(Panel::Ration);
    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 40, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[1].ration_split, 0, "and neither does the slider");
}

/// End turn, from the map, with the mouse — and the numbers move on screen.
///
/// This is the whole slice in one test: a England turn-one scenario, a real map, a click
/// on a button, `l2-kingdom`'s season pipeline, and the changed numbers read
/// back off the canvas.
#[test]
fn ending_the_turn_from_the_map_moves_the_numbers_and_the_screen_follows() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let before = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&before, "WINTER 1268", assets.ink.text).is_some());
    assert!(find_text(&before, "TURN 1", assets.ink.dim).is_some());

    let population = game.kingdom.counties[8].population;
    let gold = game.gold();

    let b = map::END_TURN_BUTTON;
    send(&mut screen, &mut game, &assets, Event::Click { x: b.centre_x(), y: b.y + 4 });

    assert_eq!(game.kingdom.turn_count, 2, "one season ran");
    assert_eq!(game.kingdom.season, 1, "Winter gave way to Spring");
    assert_ne!(game.kingdom.counties[8].population, population, "the county changed");
    assert_eq!(game.kingdom.counties[8].pop_last, population);
    assert_eq!(game.gold_last[game.player as usize], gold, "and the treasury is remembered");

    let after = draw(&mut screen, &mut game, &assets);
    assert!(find_text(&after, "SPRING 1268", assets.ink.text).is_some(), "the clock moved");
    assert!(find_text(&after, "TURN 2", assets.ink.dim).is_some());
    assert!(find_text(&after, "WINTER 1268", assets.ink.text).is_none());
    assert!(before.diff_count(&after) > 0);

    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&after, &assets, &shown, assets.ink.text).is_some(),
        "the right column shows the new population, {shown}"
    );
    assert!(
        find_strip(&after, &assets, &population.to_string(), assets.ink.text).is_none(),
        "and not the old one"
    );
}

/// The turn also runs through the machine, from the keyboard, with the county
/// panel's own numbers following. Four turns, so a season wrap is included.
#[test]
fn four_turns_run_through_the_machine_and_the_panel_keeps_up() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut ctx_seasons = Vec::new();
    for _ in 0..4 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::KeyDown(Key::Char('E')), &mut ctx);
        ctx_seasons.push((game.kingdom.season, game.kingdom.year));
    }
    assert_eq!(ctx_seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
    assert_eq!(game.turns_played, 4);

    let mut panel = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut panel, &mut game, &assets);
    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&canvas, &assets, &shown, assets.ink.text).is_some()
            || find_text(&canvas, &shown, assets.ink.good).is_some()
            || find_text(&canvas, &shown, assets.ink.bad).is_some(),
        "the panel shows the population it now has ({shown})"
    );
}

// ---------------------------------------------------------------- the village

/// **Two independent sources agreeing.** `g_jobClusterOrigins` is eight pairs
/// of integers in `Lords2.exe`'s `.data`; `vill_gd8.pl8` is a painted 45 x 40
/// mask in a file. Nothing connects them but the screen they describe — and
/// every cluster's own origin lands in that cluster's painted region, and all
/// eight regions are painted.
///
/// A misread origin, a misread grid stride, or the wrong 24-byte header offset
/// would each break this, and none of them could break it in a way that still
/// named all eight clusters correctly.
#[test]
fn the_painted_drop_grid_agrees_with_the_cluster_origins_in_the_executable() {
    let (_game, assets) = world!();
    let art = assets.village.as_ref().expect("vill.pl8 and vill_gd8.pl8");
    assert!(art.has_grid(), "vill_gd8.pl8 is 1,824 bytes: 24 of header and 45 x 40 of grid");
    let top = village::SCENE_Y;

    let mut seen = [0usize; village::CLUSTER_COUNT + 1];
    for row in 0..village::GRID_ROWS as i32 {
        for col in 0..village::GRID_COLS as i32 {
            let x = village::SCENE_X + col * village::GRID_CELL;
            let y = top + row * village::GRID_CELL;
            seen[art.cluster_at(x, y, top)] += 1;
        }
    }
    for cluster in 1..=village::CLUSTER_COUNT {
        assert!(seen[cluster] > 0, "cluster {cluster} has no painted region at all");
    }
    assert!(seen[0] > 0, "and there is ground that belongs to nobody");

    for cluster in 0..village::CLUSTER_COUNT {
        let (ox, oy) = village::cluster_origin(cluster, top);
        // The origin is the grid's top-left corner; the cluster's own middle is
        // two icons right and two rows down, which is where the artwork puts
        // the building the peasants stand at.
        let (mx, my) = (ox + 36, oy + 24);
        assert_eq!(
            art.cluster_at(mx, my, top),
            cluster + 1,
            "cluster {cluster}'s own middle ({mx}, {my}) is painted as {}",
            art.cluster_at(mx, my, top)
        );
    }
}

/// The village drawn against the shipped save: the picture is there, and so are
/// the icons standing on it.
#[test]
fn the_village_draws_the_picture_and_the_people_on_it() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);
    let canvas = draw(&mut screen, &mut game, &assets);

    // The scene is a *picture*, not a fill: it uses many palette indices.
    let top = village::SCENE_Y;
    let mut seen = [false; 256];
    for y in top..top + village::SCENE_H {
        for x in village::SCENE_X..village::SCENE_X + village::SCENE_W {
            seen[canvas.at(x as usize, y as usize) as usize] = true;
        }
    }
    let colours = seen.iter().filter(|&&s| s).count();
    assert!(colours > 32, "vill.pl8 frame 0 drew in {colours} palette indices");

    // And every cluster the county actually staffs has ink where its icons go.
    let c = &game.kingdom.counties[county as usize];
    let icons = VillageScreen::icons(c);
    let mut clusters_with_people = 0;
    for cluster in 0..village::CLUSTER_COUNT {
        if icons[cluster].iter().all(|&v| v == 0) {
            continue;
        }
        clusters_with_people += 1;
    }
    assert!(
        clusters_with_people >= 2,
        "county {county} staffs {clusters_with_people} clusters; the save has cattle and wood"
    );
}

/// **The village is an inset, and this is the test that says so.**
///
/// A player opened the game, clicked the town square, and reported a dialogue
/// with the map still visible around it. He was right; this file's earlier
/// reading — "its own case in `Screen_Draw`, therefore a full screen" — was
/// wrong (`docs/decisions.md` C22).
///
/// Painted onto a canvas of a marker colour, the village must leave the marker
/// showing everywhere outside the 480 × 320 band `Village_Draw` saves at
/// (0, `g_villageTopY`) — and in particular across the whole menu bar and all
/// but the first two columns of the county sidebar.
#[test]
fn the_village_paints_an_inset_and_leaves_the_rest_of_the_screen_alone() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);

    const MARKER: u8 = 0xAB;
    let mut canvas = Canvas::screen();
    canvas.clear(MARKER);
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut canvas);
    }

    let top = village::SCENE_Y;
    let band = |x: i32, y: i32| {
        (village::BAND_X..village::BAND_X + village::BAND_W).contains(&x)
            && (top..top + village::BAND_H_SAVED).contains(&y)
    };
    let mut escaped = Vec::new();
    for y in 0..480 {
        for x in 0..640 {
            if !band(x, y) && canvas.at(x as usize, y as usize) != MARKER {
                escaped.push((x, y));
            }
        }
    }
    assert!(
        escaped.is_empty(),
        "{} pixels painted outside the band, first at {:?}",
        escaped.len(),
        escaped.first()
    );

    // Said the other way round, on the two things the player could actually
    // see: the menu bar and the sidebar are untouched.
    for x in 0..640 {
        for y in 0..chrome::PANEL_TOP_Y {
            assert_eq!(canvas.at(x as usize, y as usize), MARKER, "menu bar at ({x}, {y})");
        }
    }
    for x in village::BAND_X + village::BAND_W..640 {
        for y in 0..480 {
            assert_eq!(canvas.at(x as usize, y as usize), MARKER, "sidebar at ({x}, {y})");
        }
    }
    // And the picture itself did get painted, so this is not passing by drawing
    // nothing at all.
    let mid = (village::SCENE_X + village::SCENE_W / 2) as usize;
    let painted = (top..top + village::SCENE_H)
        .filter(|&y| canvas.at(mid, y as usize) != MARKER)
        .count();
    assert!(painted > 200, "only {painted} of 320 rows of the picture were painted");
}

/// And the machine paints what is underneath first, which is what
/// `Village_Draw` does for itself by calling `Map_DrawFrame`.
///
/// With the campaign map on the stack and the village pushed on top, the
/// sidebar — which the village never touches — comes from the map screen.
#[test]
fn the_machine_paints_the_campaign_map_under_the_village() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let mut machine = Machine::new(ScreenId::Campaign);
    let mut canvas = Canvas::screen();
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('v')), &mut ctx);
    }
    assert_eq!(
        machine.ids(),
        vec![ScreenId::Campaign, ScreenId::Village(county)],
        "V on an owned county opens its village"
    );
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        machine.draw(&ctx, &mut canvas);
    }

    // The end-turn strip is the map screen's, at the bottom of the sidebar, and
    // the village cannot reach it.
    let mut sidebar = [false; 256];
    for y in chrome::PANEL_TOP_Y..480 {
        for x in 490..639 {
            sidebar[canvas.at(x as usize, y as usize) as usize] = true;
        }
    }
    let colours = sidebar.iter().filter(|&&s| s).count();
    assert!(colours > 8, "the sidebar under the village drew in {colours} indices");

    // And the village really is on top of it in the middle.
    let mid = (village::SCENE_X + village::SCENE_W / 2) as usize;
    let row = (village::SCENE_Y + village::SCENE_H / 2) as usize;
    let with_village = canvas.at(mid, row);
    let mut bare = Canvas::screen();
    {
        let mut only_map = Machine::new(ScreenId::Campaign);
        let ctx = Ctx { game: &mut game, assets: &assets };
        only_map.draw(&ctx, &mut bare);
    }
    assert_ne!(with_village, bare.at(mid, row), "the picture is over the map, not beside it");
}

/// The whole gesture against the real grid: band a cluster, release, drop on
/// another, and the workers land in the other cluster's job.
#[test]
fn a_drag_across_the_real_drop_grid_moves_the_county_s_peasants() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);

    let c = &game.kingdom.counties[county as usize];
    let slots = VillageScreen::slots(c);
    let icons = VillageScreen::icons(c);
    // The fullest cluster is the one worth emptying.
    let from = (0..village::CLUSTER_COUNT)
        .max_by_key(|&i| icons[i].iter().filter(|&&v| v != 0).count())
        .unwrap();
    let to = (0..village::CLUSTER_COUNT).find(|&i| slots[i] != slots[from]).unwrap();
    let (before_from, before_to) = (c.labour[slots[from]], c.labour[slots[to]]);
    assert!(before_from > 0, "cluster {from} has people in it");

    let (bx0, by0, bx1, by1) = village::cluster_band_box(from, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::Click { x: bx0, y: by0 });
    send(&mut screen, &mut game, &assets, Event::Pointer { x: bx1, y: by1 });
    send(&mut screen, &mut game, &assets, Event::Release { x: bx1, y: by1 });
    assert_eq!(screen.phase(), village_screen::Phase::Carry, "released holding a selection");
    let carried = screen.drag_count();
    assert!(carried > 0);

    let (ox, oy) = village::cluster_origin(to, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::Click { x: ox + 36, y: oy + 24 });
    assert_eq!(screen.phase(), village_screen::Phase::Idle, "and put it down");

    let c = &game.kingdom.counties[county as usize];
    let moved = before_from - c.labour[slots[from]];
    assert!(moved > 0, "somebody moved");
    assert_eq!(c.labour[slots[to]] - before_to, moved, "and they arrived");
    assert_eq!(
        moved,
        (carried * c.pop_band).min(before_from),
        "an icon is popBand people, clamped to what the job held"
    );
    assert_eq!(
        c.labour.iter().sum::<i32>(),
        game.kingdom.counties[county as usize].population,
        "and the nine still sum to the population"
    );
}

/// A click that never travels nine pixels is a click, and a click opens the job
/// popup for the cluster it landed on — the fifth screen, and the only place
/// the labour record's other two words are shown as a number.
#[test]
fn a_click_on_a_cluster_opens_its_job_popup() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);
    let c = &game.kingdom.counties[county as usize];
    let slots = VillageScreen::slots(c);

    // Cluster 2 is cattle farming, which is the job the shipped save staffs.
    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    assert_eq!(screen.phase(), village_screen::Phase::Idle, "one press is not a drag");
    let t = send(&mut screen, &mut game, &assets, Event::Release { x, y });
    assert_eq!(t, Transition::Push(ScreenId::Job(county, slots[2])));
}

// ---------------------------------------------------------------------------
// The field brush
// ---------------------------------------------------------------------------

/// Centre the map on a tile and give back its screen position.
fn on_screen(screen: &mut MapScreen, tile: usize) -> (i32, i32) {
    let (x, y) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(x as usize, y as usize);
    l2_view::campaign::tile_centre(screen.viewport(), screen.zoom(), x as usize, y as usize)
        .expect("a tile the viewport was just centred on is in the viewport")
}

/// **A player clicks one of their own fields and paints it to grain.**
///
/// Two clicks, both through `Screen::handle`: one on the tile, which is
/// `Map_Click`'s farmland branch, and one on the grain button, which is the
/// hotspot at `x 304 … 352, y 184 … 232`. Nothing here reaches into the
/// simulation; the assertion is that the county's grain field count moved.
#[test]
fn clicking_a_field_and_then_the_grain_button_sows_it() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let (tile, kind) = game
        .kingdom
        .field_tiles(county as usize)
        .into_iter()
        .find(|&(_, k)| k == l2_kingdom::field::FieldType::Fallow)
        .expect("a fallow field to paint");
    assert_eq!(kind, l2_kingdom::field::FieldType::Fallow);
    let before = game.kingdom.counties[county as usize].fields_grain;

    let mut screen = MapScreen::new();
    let (x, y) = on_screen(&mut screen, tile);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });

    // The grain button is the middle column of the three-button menu.
    send(&mut screen, &mut game, &assets, Event::Click { x: 328, y: 208 });
    assert_eq!(
        game.kingdom.counties[county as usize].fields_grain,
        before + 1,
        "the click reached Field_SetType"
    );
    assert_eq!(
        game.kingdom.campaign.map.terrain[tile],
        l2_kingdom::field::terrain::GRAIN,
        "and the map tile is the terrain the hotspot's id names"
    );
}

/// A click somewhere else while the popup is up dismisses it and paints
/// nothing, which is what a modal hotspot table does.
#[test]
fn a_click_off_the_brush_popup_changes_nothing() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);
    let (tile, _) = game.kingdom.field_tiles(county as usize)[0];
    let before = game.kingdom.counties[county as usize].clone();

    let mut screen = MapScreen::new();
    let (x, y) = on_screen(&mut screen, tile);
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    send(&mut screen, &mut game, &assets, Event::Click { x: 40, y: 400 });
    assert_eq!(game.kingdom.counties[county as usize], before);
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

// ---------------------------------------------------------------------------
// Looking at the screen
// ---------------------------------------------------------------------------

/// **PNG, not raw RGBA.** `docs/decisions.md` C21's conclusion is *show screens
/// early, to someone who knows the game*, and it cost this project a whole map
/// screen to learn. A `.rgb` dump does not do that — it needs a converter and a
/// remembered width before anyone can glance at it, which is enough friction
/// that nobody glances.
///
/// So these forty lines write a real PNG with no dependency: a stored-block
/// zlib stream (compression 0), which is legal deflate, plus the two checksums
/// PNG requires. It is bigger than the raw dump and it opens in anything.
mod png {
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, e) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
        c ^ 0xFFFF_FFFF
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut all = kind.to_vec();
        all.extend_from_slice(body);
        out.extend_from_slice(&all);
        out.extend_from_slice(&crc32(&all).to_be_bytes());
    }

    /// 8-bit truecolour, one row filter byte of 0 per scanline.
    pub fn encode(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(h * (1 + w * 3));
        for y in 0..h {
            raw.push(0);
            raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
        }
        let mut z = vec![0x78, 0x01];
        for (i, block) in raw.chunks(65_535).enumerate() {
            let last = (i + 1) * 65_535 >= raw.len();
            z.push(u8::from(last));
            z.extend_from_slice(&(block.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
            z.extend_from_slice(block);
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

fn save_png(canvas: &Canvas, assets: &Assets, name: &str) {
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(&assets.palette, &mut rgba);
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    std::fs::create_dir_all("out").unwrap();
    std::fs::write(format!("out/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}

/// Not a test: a way to look at the screen. `cargo test -p l2-game --test
/// screens shoot -- --ignored` writes PNGs into `out/`, which `.gitignore`
/// excludes. Renders of the game's own artwork are derived assets and must
/// never be committed (CLAUDE.md rule 1).
///
/// Five shots: the map at both zooms, then a fallow field clicked, its brush
/// popup, and the field after the grain button — which is the whole feature in
/// three pictures.
#[test]
#[ignore]
fn shoot() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    for (name, zoomed) in [("near", false), ("far", true)] {
        if zoomed {
            send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
        }
        let canvas = draw(&mut screen, &mut game, &assets);
        save_png(&canvas, &assets, &format!("campaign_{name}"));
    }

    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);
    let (tile, _) = game
        .kingdom
        .field_tiles(county as usize)
        .into_iter()
        .find(|&(_, k)| k == l2_kingdom::field::FieldType::Fallow)
        .expect("a fallow field");

    let mut screen = MapScreen::new();
    let (x, y) = on_screen(&mut screen, tile);
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "brush_before");

    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "brush_open");

    send(&mut screen, &mut game, &assets, Event::Click { x: 328, y: 208 });
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "brush_after");

    // **The county a player reported four defects against**, centred on its own
    // town, with the sidebar the four of them live in.
    //
    // `county_town` is what a bug report needs and a window is not: this runs
    // headlessly, touches no OS input queue and moves nobody's cursor. Every
    // screen in this crate takes input as a value, so there is never a reason
    // to drive our own engine through the desktop — `docs/agents.md`.
    let mut screen = MapScreen::new();
    let ctx = Ctx { game: &mut game, assets: &assets };
    if let Some(&tile) = MapScreen::town(&ctx, county).first() {
        let (tx, ty) = l2_kingdom::map::coords(tile);
        screen.centre_on_tile(tx as usize, ty as usize);
    }
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "county_town");

    // **The things on the map a player said were missing**: an army raised in
    // his own county, a merchant standing beside it, and the town's flag.
    {
        let realm = game.kingdom.realms[game.player as usize].clone();
        let basket = l2_kingdom::LevyBasket::seed(&realm, 300);
        let _ = game.raise_army(county, &basket, 10, None);
        let (ax, ay) = {
            let c = &game.kingdom.counties[county as usize];
            (c.anchor_x, c.anchor_y)
        };
        let merchant = game
            .kingdom
            .campaign
            .units
            .iter()
            .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
            .map(|(id, _)| id);
        if let Some(u) = merchant.and_then(|id| game.kingdom.campaign.units.get_mut(id)) {
            u.x = ax.saturating_sub(1);
            u.y = ay;
            u.county = county;
        }
        let mut screen = MapScreen::new();
        let canvas = draw(&mut screen, &mut game, &assets);
        save_png(&canvas, &assets, "units_and_flags");
    }

    // And the four panels, each from its own quadrant of the strip.
    for panel in county::PANELS {
        let mut m = Machine::new(ScreenId::County(county, panel));
        let mut c = Ctx { game: &mut game, assets: &assets };
        let mut canvas = Canvas::screen();
        m.draw(&c, &mut canvas);
        let _ = &mut c;
        save_png(&canvas, &assets, &format!("panel_{panel:?}").to_lowercase());
    }
}
