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
use l2_game::screens::menubar;
use l2_game::screens::options::Page as OptionsPage;
use l2_game::screens::saveload::Mode as SaveLoadMode;
use l2_game::screens::village::{self as village_screen, VillageScreen};
use l2_game::shell::font;
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

/// One event into a whole [`Machine`], which is the only way to exercise an arm
/// that a screen answers with [`Transition::Pass`].
///
/// **Half the county sidebar is one of those.** `Screen_FrameInput`'s arms for
/// the village and for the four county panels open with six guards belonging to
/// the campaign map, so an assertion made against a bare `CountyScreen` cannot
/// see them at all — which is how the panels came to swallow the entire
/// right-hand column with a green suite.
fn send_stack(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

fn draw_stack(m: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    m.draw(&ctx, &mut canvas);
    canvas
}

/// The campaign map with one screen opened over it, which is what every county
/// panel, the village and the job popup actually are.
fn over_the_map(over: ScreenId) -> Machine {
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(over);
    m
}

/// **Run the frames a turn takes.** Pressing End Turn only starts one — the
/// phase machine is wound on once per fixed tick and the season fade follows —
/// so a test that wants the numbers afterwards has to tick. See
/// `l2_game::turn::TurnRun`.
fn run_turn<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    let mut done_at = None;
    for n in 1..2_000u32 {
        let mut ctx = Ctx { game, assets };
        screen.update(&mut ctx);
        if done_at.is_none() && game.kingdom.turn_count > before {
            done_at = Some(n);
        }
        if let Some(t) = done_at {
            if n >= t + l2_view::fade::PHASES as u32 {
                return;
            }
        }
    }
    panic!("the turn never came round");
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
/// **What the county strip's text is actually drawn in**: the literal `0x3F`
/// every `Ui_DrawText` call in `CountyStrip_Draw` passes, which is `rgb(0,0,0)`
/// in `Base01.256`. These assertions used to look for `ink.text` — white — and
/// a player reported the strip as white-on-parchment before anyone read the
/// argument.
const STRIP_INK: u8 = l2_game::shell::font::TEXT;
/// And the one exception: the achieved ration when it is not the wanted one.
const STRIP_BAD: u8 = l2_game::shell::font::HIGHLIGHT;

fn find_strip(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.small.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

/// One line of the strip drawn in the **body** font — the county's name, the
/// three "sovereign land of …" lines on a county you do not hold, and every
/// body line of the four county panels since they graduated to [`Pen`].
fn find_body(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.body.as_ref() {
        Some(f) => find_font_text(canvas, f, s, colour),
        None => find_text(canvas, s, colour),
    }
}

/// The same in the **heading** font, `Fntl2_22.pl8` — which the county panels'
/// titles and their three plain-number rows are drawn in
/// (`Eng_DrawString(…, &g_fontHeading, …)`).
fn find_heading(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(i32, i32)> {
    match assets.shell.heading.as_ref() {
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
            frame: u.sprite_frame(0),
            nudge: u.sprite_nudge(),
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
/// strength of a "last arm" quoted into three documents. There is no last arm.
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
/// rather than nothing happening, purely because a miss had somewhere to fall
/// through to. It asserts over the whole state rather than the screen id,
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

/// A click picks the county the player can actually see at that pixel. The
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
            game.kingdom.season,
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
    // A county the player holds that actually has a mine, from the save.
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
    let pop = find_strip(&canvas, &assets, "435", STRIP_INK).expect("the population");
    assert_eq!(pop, (508, 189), "at CountyStrip_Draw's own coordinates");
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
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
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
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
            assert_eq!(
                t,
                Transition::Push(map::sidebar_destination(id, game.selected)),
                "{} is not gated",
                b.name,
            );
            // The ungated two are the court (`0x09`, still a shell) and the
            // lords (`0x0B`, which has graduated) — so this asks
            // `sidebar_destination` too rather than assuming a shell. The
            // ungating is the point: **the diplomacy screen is about realms,
            // not counties**, and `FUN_0043611B` has no county gate at all.
            assert_eq!(t, Transition::Push(map::sidebar_destination(id, 1)), "{} is not gated", b.name);
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

/// **The blue outline appears, and only when it should.**
///
/// Two separate signals with two separate tests, which is the thing worth
/// pinning down: the **slider's** thumb gains its ring on
/// `county.labour[8].workers != 0` — anybody idle at all — and each **produce
/// icon** gains one on `labour[slot].useful < labour[slot].workers` — too many
/// people on *that* job. A player described both and thought they were the same
/// signal; they are the same picture and different tests.
///
/// Measured by counting pixels of the ring's own three palette entries inside
/// the sidebar, so it is the artwork being asserted and not a description of
/// it.
#[test]
fn the_strip_draws_the_blue_ring_on_the_slider_and_on_the_overstaffed_job() {
    use l2_view::chrome::misc_cty::RING_COLOURS;
    let (mut game, assets) = world!();
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
    let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;

    // Count the ring's colours in the sidebar column only.
    let ring_pixels = |canvas: &Canvas| -> usize {
        let mut n = 0;
        for y in 156..430usize {
            for x in 478..640usize {
                if RING_COLOURS.contains(&canvas.at(x, y)) {
                    n += 1;
                }
            }
        }
        n
    };

    // Nobody idle, and the dairy inside its ceiling.
    {
        let c = &mut game.kingdom.counties[8];
        c.labour[idle] = 0;
        c.labour[cattle] = 100;
        c.labour_wanted[cattle] = -1;
        c.labour_useful[cattle] = 200;
        c.herd = 400;
        c.fields_cattle = 4;
    }
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let quiet = ring_pixels(&draw(&mut screen, &mut game, &assets));

    // One idle townsman: the slider's thumb becomes frame 0x55.
    game.kingdom.counties[8].labour[idle] = 1;
    let with_slider = ring_pixels(&draw(&mut screen, &mut game, &assets));
    assert!(
        with_slider > quiet,
        "the slider's thumb gains its ring: {quiet} -> {with_slider} ring pixels"
    );

    // And more people milking than the herd can use: the cow gains one too.
    game.kingdom.counties[8].labour_useful[cattle] = 50;
    let with_both = ring_pixels(&draw(&mut screen, &mut game, &assets));
    assert!(
        with_both > with_slider,
        "the dairy icon gains its own ring: {with_slider} -> {with_both} ring pixels"
    );

    // Putting the ceiling back takes the cow's ring away again and leaves the
    // slider's, which is what makes them two tests rather than one.
    game.kingdom.counties[8].labour_useful[cattle] = 200;
    assert_eq!(ring_pixels(&draw(&mut screen, &mut game, &assets)), with_slider);
}

/// **And it is a drag, not a click.** A player reported *"the peasant slider of
/// industry isn't draggable, should be"*, and `FUN_00439122` agrees: it acts
/// while `DAT_004E65CC` — the button's *level* — is set and `DAT_004EA4B0` says
/// the pointer moved, and does nothing at all on the release. So the value
/// follows the pointer for as long as the button is held, and stops the moment
/// it is let go.
///
/// The whole gesture as a sequence of values, which is the only way to test a
/// drag without an input queue.
#[test]
fn the_split_slider_tracks_the_pointer_while_the_button_is_held() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let share = |g: &Game| g.kingdom.counties[8].industry_share;

    // Press on the track at x = 533, then travel along it without letting go.
    send(&mut screen, &mut game, &assets, Event::Click { x: 533, y: 270 });
    assert_eq!(share(&game), 4, "the press itself sets the value");
    for (x, want) in [(541, 20), (561, 60), (581, 100), (551, 40)] {
        send(&mut screen, &mut game, &assets, Event::Pointer { x, y: 270 });
        assert_eq!(share(&game), want, "held and moved to x = {x}");
    }

    // Off the sidebar entirely and the slider stops, without the drag ending —
    // the original re-tests the rectangle every frame and simply skips.
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 200, y: 270 });
    assert_eq!(share(&game), 40, "outside the rectangle nothing moves");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 561, y: 270 });
    assert_eq!(share(&game), 60, "and coming back resumes the same drag");

    // Let go. Now the same movement does nothing.
    send(&mut screen, &mut game, &assets, Event::Release { x: 561, y: 270 });
    assert_eq!(share(&game), 60, "the release itself changes nothing");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 533, y: 270 });
    assert_eq!(share(&game), 60, "and a bare pointer move is not a drag");

    // Off the track, each move steps by four rather than jumping.
    send(&mut screen, &mut game, &assets, Event::Click { x: 600, y: 270 });
    assert_eq!(share(&game), 64, "right of the track: +4");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 601, y: 270 });
    assert_eq!(share(&game), 68, "and again on the next move");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 500, y: 270 });
    assert_eq!(share(&game), 64, "left of the track: -4");
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
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "the information panel, and it now knows what the click resolved to",
    );

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

    let c = &game.kingdom.counties[8];
    assert_eq!((c.population, c.happiness, c.ration_achieved), (435, 72, 3));

    assert_eq!(
        find_strip(&canvas, &assets, "435", STRIP_INK),
        Some((508, 189)),
        "the population, at Ui_DrawNumber(pop, ' ', ..., 0x1FC, 0xBD)"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left-aligned at 602, not right-anchored on it.** `Ui_DrawNumber`
        // has no anchoring argument: the population's call and the happiness's
        // differ only in their value and their x, so both are left origins.
        // See `docs/decisions.md` C42.
        Some((602, 189)),
        "the happiness, right-anchored at 0x25A on the same line"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "0%", STRIP_INK),
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
    assert!(find_strip(&canvas, &assets, "Normal", STRIP_INK).is_some());
    assert!(find_strip(&canvas, &assets, "Normal", STRIP_BAD).is_none());

    // Near misses, one per number, so none of the three can match by accident.
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
    assert!(find_strip(&canvas, &assets, "73", STRIP_INK).is_none());
    assert!(find_strip(&canvas, &assets, "Double", STRIP_INK).is_none());
}

/// The population panel is `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)` with its rows
/// at the y coordinates `Panel_Population` draws them, and it is reached from
/// the strip's top-left quadrant and from nowhere else.
#[test]
fn the_population_panel_opens_from_its_own_quadrant_and_lays_out_where_it_should() {
    let (mut game, assets) = world!();
    let ink = &assets.ink;

    // **Through the machine, with the campaign map underneath**, because that
    // is where `CountyStrip_Click` lives: `Screen_FrameInput`'s arm for `0x14`
    // and its three siblings runs six guards belonging to the map's right-hand
    // column *before* any verb of the panel's own, and the strip's 2 × 2
    // hotspot is the third of them. The panel returns `Transition::Pass` for
    // the whole column and the map answers.
    //
    // This used to send the click straight to a bare `CountyScreen`, which
    // could not tell "the panel switched" from "the panel swallowed the whole
    // sidebar" — and it swallowed it. `docs/arms.json`
    // `0x0042FF10/inset-runs-the-sidebar-guards`.
    game.select(8);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::County(8, Panel::Tax));
    let hot = Panel::Population.strip_hotspot();
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: hot.centre_x(), y: hot.y + 4 }, &mut ctx);
    }
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Population)),
        "the strip switched panel rather than closing or stacking one"
    );
    assert_eq!(m.depth(), 2, "and it landed at the map's depth, not on top of the tax panel");

    // **The panel is drawn in the game's own fonts and from the game's own
    // `L2.eng`** — so the strings here are the file's words, lower case and
    // all, and the search is [`find_body`]/[`find_heading`] rather than the
    // 5 x 7 probe. It used to be our own transcriptions in our own font.
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        find_heading(&canvas, &assets, "Last season", font::TEXT),
        Some((48, 266)),
        "group 73 index 1, at Eng_DrawString(0x49, 1, 0x30, 0x10A)"
    );
    assert_eq!(
        find_heading(&canvas, &assets, "417", font::TEXT),
        Some((336, 266)),
        "and its value **left**-aligned from 0x150 — Ui_DrawNumber does not measure"
    );
    assert_eq!(find_body(&canvas, &assets, "Births", font::TEXT), Some((48, 298)), "0x12A");
    assert_eq!(find_body(&canvas, &assets, "Deaths", font::TEXT), Some((48, 314)), "0x13A");
    assert_eq!(find_body(&canvas, &assets, "Army", font::TEXT), Some((48, 330)), "0x14A");
    assert_eq!(
        find_heading(&canvas, &assets, "This Season", font::TEXT),
        Some((48, 386)),
        "0x182"
    );
    assert_eq!(
        find_heading(&canvas, &assets, "435", font::TEXT),
        Some((336, 386)),
        "this season's population, left-aligned from 0x150"
    );
    // `Eng_DrawString(100, slot * 0x14 + county, pen + 0x16, 0x38, heading)` —
    // the county's own name after the title, which the panel used to omit.
    assert!(
        find_heading(&canvas, &assets, "Population in", font::TEXT).is_some(),
        "group 73 index 0 at (20, 56)"
    );

    // The graph is a labelled stub, because g_countyHistory is not simulated.
    // Both of its lines are **ours**, in our own font, and say so.
    assert!(find_text(&canvas, "NOT SIMULATED", ink.bad).is_some());

    // And the tax panel is gone, which is what "one panel at a time" means.
    assert!(find_body(&canvas, &assets, "Tax rate", font::TEXT).is_none());
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
    assert!(
        find_body(&blank, &assets, "Births", font::TEXT).is_some(),
        "the label is still drawn"
    );
    assert!(find_body(&blank, &assets, "+0", font::TEXT).is_none(), "and nothing beside it");

    game.kingdom.counties[8].births = 63;
    let filled = draw(&mut screen, &mut game, &assets);

    // **The strong form: the two frames differ only inside the births row.**
    // A bare "0" is no longer a usable near-miss, because every line on the
    // panel is now drawn in the one colour the painter passes (0x3F) and some
    // other number on it contains the digit. This says the same thing without
    // depending on what else is on the page: turning births from 0 to 63 puts
    // ink in the births row and nowhere else, so the blank frame's own value
    // cell was empty.
    let band = 294..312;
    let outside: usize = (0..filled.height)
        .filter(|y| !band.contains(&(*y as i32)))
        .map(|y| {
            (0..filled.width).filter(|x| blank.at(*x, y) != filled.at(*x, y)).count()
        })
        .sum();
    assert_eq!(outside, 0, "births changed something outside its own row");
    assert!(
        (0..filled.width)
            .any(|x| band.clone().any(|y| blank.at(x, y as usize) != filled.at(x, y as usize))),
        "and it did change its own row"
    );

    // **Left-aligned from 0x150, plus the four pixels `Ui_DrawText` advances
    // for `Ui_DrawDelta`'s empty prefix.** It used to be right-anchored *to*
    // 336, which is what `docs/screens-county.md` §5.1 says and neither
    // `Ui_DrawNumber` nor `Ui_DrawDelta` does.
    assert_eq!(
        find_body(&filled, &assets, "+63", font::TEXT),
        Some((340, 298)),
        "a non-zero row draws a signed number in the value column"
    );
    assert!(blank.diff_count(&filled) > 0, "and the two frames differ");
    let _ = ink;
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
    // `Ui_DrawNumber(taxRate, ' ', "%", 0x100, 0xA8, body, 0x3F)`.
    assert!(find_body(&after, &assets, "7%", font::TEXT).is_some());
    assert!(find_body(&after, &assets, "0%", font::TEXT).is_none());

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
    assert!(find_body(&canvas, &assets, "50%", font::TEXT).is_some());
    assert!(find_body(&canvas, &assets, "51%", font::TEXT).is_none(), "a near miss");
    assert!(find_body(&canvas, &assets, "60%", font::TEXT).is_none());
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

/// **The slider's effect has to be visible in the same frame, and it was not.**
///
/// A player reported *"rations slider moves but is inoperable"*. It was writing
/// `ration_split` correctly the whole time — a test three functions up asserted
/// exactly that and passed — and every number on the panel stayed where it was
/// until the turn ended, because `Game::set_ration_split` wrote the field and
/// stopped. `Ration_SetSplit` re-runs the food pass, reallocates the county
/// twice and repaints.
///
/// So this asserts the *effect* and not the fixture: draw the panel, move the
/// slider, draw it again, and require pixels to differ **outside the slider's
/// own rectangle**. Masking the slider out is the whole point — a thumb that
/// moves is what the player could already see, and it is not evidence of
/// anything.
///
/// Ablation, which was run: replace `Kingdom::set_ration_split`'s body with the
/// old one-line write and this fails with zero pixels changed.
#[test]
fn moving_the_ration_slider_changes_a_number_on_the_panel_in_the_same_frame() {
    let (mut game, assets) = world!();
    let county = 8; // the player's, in the England turn-one fixture
    assert_eq!(game.kingdom.counties[county].owner, game.player);

    // **The fixture's own county cannot demonstrate this, and the reason is a
    // rule worth knowing.** County 8 at England turn one has 435 people and
    // 101 head; the standing herd feeds five people a head *without being
    // slaughtered*, so 505 mouths' worth of dairy covers 435 and the county eats
    // nothing at all. `herd_eaten` and `grain_eaten` are 0 at **every** split,
    // so the slider has nothing to divide and the panel is inert — in the
    // original as much as here. That is very likely what the player was looking
    // at, and it is `docs/rules.md`'s to explain rather than a defect.
    //
    // So the herd is cut to something the county has to eat *around*. The state
    // is the input and the screen is the subject; asserting on a county whose
    // numbers cannot move would be a test that passes for the wrong reason,
    // which is the whole family `docs/agents.md` catalogues.
    game.kingdom.counties[county].herd = 20;
    game.kingdom.counties[county].grain = 400;

    let mut screen = CountyScreen::new(county as u8, Panel::Ration);
    let track = county::split_track();

    // Start at one end so the move is as large as the control allows.
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x, y: track.y + 8 });
    let before = draw(&mut screen, &mut game, &assets);
    let split_before = game.kingdom.counties[county].ration_split;

    // A drag: the button goes down on the track and the pointer walks to the
    // far end. `Ration_SliderClick` fires on held-and-moved, not on the click.
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 4, y: track.y + 8 });
    for step in (4..=track.w).step_by(8) {
        send(
            &mut screen,
            &mut game,
            &assets,
            Event::Pointer { x: track.x + step, y: track.y + 8 },
        );
    }
    send(&mut screen, &mut game, &assets, Event::Release { x: track.x + track.w, y: track.y + 8 });
    let after = draw(&mut screen, &mut game, &assets);

    assert_ne!(
        game.kingdom.counties[county].ration_split, split_before,
        "the drag did not reach the field at all",
    );

    // Everything except the slider's own row. `split_track` is the track; the
    // two caps sit either side of it on the same row, so the mask is the whole
    // band.
    let masked = |c: &l2_view::Canvas, other: &l2_view::Canvas| {
        let mut n = 0usize;
        for y in 0..l2_view::canvas::HEIGHT {
            for x in 0..l2_view::canvas::WIDTH {
                let (xi, yi) = (x as i32, y as i32);
                let in_slider = yi >= track.y - 8
                    && yi < track.y + track.h + 8
                    && xi >= track.x - 64
                    && xi < track.x + track.w + 64;
                if !in_slider && c.at(x, y) != other.at(x, y) {
                    n += 1;
                }
            }
        }
        n
    };
    let changed = masked(&before, &after);
    assert!(
        changed > 0,
        "the slider moved and nothing else on the panel did. The thumb is not the \
         effect: Ration_SetSplit runs the food pass on the spot, so `Eaten`, `Achieved` \
         and the happiness deltas move with it. docs/decisions.md C118.",
    );
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
    // **The pen is the realm's shield colour**, `g_realmColour[shield]`, which
    // is what `CountyStrip_Draw` passes for all three lines. Not `Ink::realm`
    // and not the realm id — see `sovereign_lines` below and C62.
    let shield = game.kingdom.realms[5].shield_index;
    let realm5 = l2_view::chrome::realm_pen(shield).expect("realm 5 flies a shield");
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);
    assert_eq!(
        find_body(&canvas, &assets, &name, STRIP_INK).map(|p| p.1),
        Some(180),
        "the name sits lower on the 162 x 274 plate"
    );
    assert!(find_body(&canvas, &assets, "REALM 5", realm5).is_some());
    assert!(find_body(&canvas, &assets, "REALM 4", realm5).is_none(), "a near miss");
    assert!(find_body(&canvas, &assets, "693", STRIP_INK).is_none(), "no numbers at all");

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
    assert_eq!(
        game.kingdom.turn_count, 1,
        "the click starts the turn; it does not run it inside the click",
    );
    run_turn(&mut screen, &mut game, &assets);

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
        find_strip(&after, &assets, &shown, STRIP_INK).is_some(),
        "the right column shows the new population, {shown}"
    );
    assert!(
        find_strip(&after, &assets, &population.to_string(), STRIP_INK).is_none(),
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
        // Four turns, and each of them takes the frames it takes.
        let before = game.kingdom.turn_count;
        let mut done_at = None;
        for n in 1..2_000u32 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.update(&mut ctx);
            if done_at.is_none() && game.kingdom.turn_count > before {
                done_at = Some(n);
            }
            if done_at.is_some_and(|t| n >= t + l2_view::fade::PHASES as u32) {
                break;
            }
        }
        ctx_seasons.push((game.kingdom.season, game.kingdom.year));
    }
    assert_eq!(ctx_seasons, vec![(1, 1268), (2, 1268), (3, 1268), (4, 1269)]);
    assert_eq!(game.turns_played, 4);

    let mut panel = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut panel, &mut game, &assets);
    let shown = game.kingdom.counties[8].population.to_string();
    assert!(
        find_strip(&canvas, &assets, &shown, STRIP_INK).is_some()
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

/// **A county with a mine draws a mine, and a county with a quarry draws a
/// quarry — and neither draws the other.**
///
/// A player reported *"a county that clearly has iron has no iron mine in the
/// town centre"*. Two things were wrong at once and this asserts both:
///
/// * `Village_Draw` (`0x00412143`) blits three buildings out of `villani2.pl8`,
///   each gated on `county.industry[c].hasResource` — frame `0x29` at
///   `(0xac, top + 0xe5)` for wood, `0x28` at `(0x4c, top + 0x0c)` for stone
///   and `0x2b` at the *same spot* for iron. None of the three was drawn.
/// * `has_resource` was never imported, so every county claimed all four and
///   the mine could not have been chosen even if it had been drawn.
///
/// The check is exact: the shared spot is compared against the frame it should
/// be holding, pixel for pixel, and against the *other* county's frame, which
/// must differ. County 5 of England has neither, and its spot must hold neither.
#[test]
fn the_village_draws_the_mine_for_an_iron_county_and_the_quarry_for_a_stone_one() {
    let (mut game, assets) = world!();
    let art = assets.village.as_ref().expect("the village artwork");
    let top = village::SCENE_Y;

    // The three buildings, as `Village_Draw` has them.
    let (stone_slot, stone_frame, bx, by) = village::RESOURCE_BUILDINGS[1];
    let (iron_slot, iron_frame, ix, iy) = village::RESOURCE_BUILDINGS[2];
    assert_eq!((stone_slot, iron_slot), (3, 1), "stone is industry 3 and iron is industry 1");
    assert_eq!((bx, by), (ix, iy), "and they are drawn at the same spot");

    // A county of each kind, chosen by what the *save* says rather than by id.
    let kind = |id: usize| {
        let c = &game.kingdom.counties[id];
        (c.industry[1].has_resource, c.industry[3].has_resource)
    };
    let ids: Vec<usize> = game.kingdom.county_ids().collect();
    let iron = ids.iter().copied().find(|&id| kind(id) == (true, false)).expect("a mine county");
    let stone = ids.iter().copied().find(|&id| kind(id) == (false, true)).expect("a quarry county");
    let neither = ids.iter().copied().find(|&id| kind(id) == (false, false));
    assert!(
        ids.iter().all(|&id| kind(id) != (true, true)),
        "no county holds both, so the mine never covers the quarry"
    );

    // What each one *should* look like: the scene, the building, then
    // `Village_Animate`'s overlays at the frame a freshly opened village shows
    // — which is the order `Village_Draw` and `Screen_DrawWidgets` paint in,
    // and the reason the overlays are in this reference at all is that the iron
    // mine's own overlay lands inside the sampled square.
    let reference = |frame: Option<usize>| {
        let mut c = Canvas::screen();
        art.draw_scene(&mut c, top);
        let has = [false, frame == Some(iron_frame), false, frame == Some(stone_frame)];
        if frame.is_some() {
            art.draw_resources(&mut c, has, top);
        }
        art.draw_animations(&mut c, has, top, &village::AnimationClock::new());
        c
    };
    let region = |c: &Canvas| {
        let mut out = Vec::new();
        for y in by + top..by + top + 96 {
            for x in bx..bx + 96 {
                out.push(c.at(x as usize, y as usize));
            }
        }
        out
    };

    let mine = region(&reference(Some(iron_frame)));
    let pit = region(&reference(Some(stone_frame)));
    let bare = region(&reference(None));
    assert_ne!(mine, pit, "the mine and the quarry are different pictures");
    assert_ne!(mine, bare, "and the mine is not the bare scene");

    for (id, want, what) in [(iron, &mine, "a mine"), (stone, &pit, "a quarry")] {
        let mut screen = VillageScreen::new(id as u8);
        let drawn = draw(&mut screen, &mut game, &assets);
        assert_eq!(&region(&drawn), want, "county {id} must show {what}");
    }
    if let Some(id) = neither {
        let mut screen = VillageScreen::new(id as u8);
        let drawn = draw(&mut screen, &mut game, &assets);
        assert_eq!(region(&drawn), bare, "county {id} has neither and must show neither");
    }
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
///
/// **But not on the release.** `Village_ClickJob` (`0x0043A123`) is gated on
/// `DAT_004EABF0`, which the frame poll sets only once the click has stood for
/// 300 ms without a second one; until then the click might be the first half of
/// a double click, and a double click means something else entirely. So the
/// popup opens on a *tick*, and the ticks are what this test counts.
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
    assert_eq!(t, Transition::Stay, "the release only arms the click");

    // One tick short of the settle, and still nothing.
    for _ in 1..VillageScreen::CLICK_SETTLE_TICKS {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay);
    }
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(screen.update(&mut ctx), Transition::Push(ScreenId::Job(county, slots[2])));
}

/// **Double-clicking a job takes the people it cannot use out of it.**
///
/// A player reported *"I can't double click idle peasants in a task to remove
/// them from the task"*, and the original does exactly that:
/// `Village_DoubleClick` (`0x00439DF0`) is its own arm on screen `0x02`, fed by
/// `WM_LBUTTONDBLCLK` through `DAT_004EABC5`, and `FUN_00439F6A` moves either
/// the surplus out or the shortfall in.
///
/// Both directions, on the county's own cattle cluster, plus the thing that
/// makes it a *different* gesture: the job popup a single click would have
/// opened never appears.
#[test]
fn a_double_click_on_a_job_sheds_its_surplus_and_fills_its_shortfall() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let slots = VillageScreen::slots(&game.kingdom.counties[county as usize]);
    let cattle = slots[2];
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;

    // Put everybody on cattle and give the job a ceiling it is well over.
    {
        let c = &mut game.kingdom.counties[county as usize];
        c.labour = [0; l2_kingdom::tables::JOB_COUNT];
        c.labour[cattle] = 300;
        c.labour_wanted[cattle] = -1;
        c.labour_useful[cattle] = 100;
        c.population = 300;
    }

    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);
    let mut screen = VillageScreen::new(county);
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });

    let c = &game.kingdom.counties[county as usize];
    assert_eq!(c.labour[cattle], 100, "the job is left with exactly what it can use");
    assert_eq!(c.labour[idle], 200, "and the other two hundred are idle");
    assert_eq!(c.labour.iter().sum::<i32>(), 300, "nobody was created or lost");

    // The other direction: give the job a floor and double-click it again.
    game.kingdom.counties[county as usize].labour_wanted[cattle] = 250;
    game.kingdom.counties[county as usize].labour_useful[cattle] = 250;
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });
    let c = &game.kingdom.counties[county as usize];
    assert_eq!(c.labour[cattle], 250, "the shortfall came out of the idle pool");
    assert_eq!(c.labour[idle], 50);

    // And it is not a click: nothing is pending, so no job popup ever opens.
    for _ in 0..VillageScreen::CLICK_SETTLE_TICKS + 2 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay, "a double click opens no popup");
    }
}

/// **A double click cancels the single click it interrupted.** The frame poll
/// clears `DAT_004E65E8` — the pending click — the instant `DAT_004EABC5` is
/// set, which is why the job popup does not open behind the reassignment.
#[test]
fn a_double_click_cancels_the_pending_single_click() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let mut screen = VillageScreen::new(county);
    let (ox, oy) = village::cluster_origin(2, village::SCENE_Y);
    let (x, y) = (ox + 36, oy + 24);

    // The first half of the double click: press, release, click armed.
    send(&mut screen, &mut game, &assets, Event::Click { x, y });
    send(&mut screen, &mut game, &assets, Event::Release { x, y });
    // The second half arrives well inside the settle window.
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x, y });
    for _ in 0..VillageScreen::CLICK_SETTLE_TICKS + 2 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.update(&mut ctx), Transition::Stay);
    }
}

/// **Double-clicking the idle townsfolk themselves puts everybody to work.**
///
/// `FUN_00439EDB`'s cluster-6 branch: every job sheds its surplus first, and
/// only then does every job draw from the pool. One pass would let whichever
/// job came first take people the later ones needed.
#[test]
fn a_double_click_on_the_idle_cluster_balances_every_job_at_once() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    let slots = VillageScreen::slots(&game.kingdom.counties[county as usize]);
    let (cattle, grain) = (slots[2], slots[1]);
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
    {
        let c = &mut game.kingdom.counties[county as usize];
        c.labour = [0; l2_kingdom::tables::JOB_COUNT];
        // Cattle is over its ceiling by 200; grain is a hundred short. The pool
        // is empty, so grain can only be filled *after* cattle has shed.
        c.labour[cattle] = 300;
        c.labour_useful[cattle] = 100;
        c.labour_wanted[cattle] = -1;
        c.labour[grain] = 0;
        c.labour_wanted[grain] = 100;
        c.labour_useful[grain] = 100;
        c.population = 300;
    }

    let mut screen = VillageScreen::new(county);
    let (ox, oy) = village::cluster_origin(village::IDLE_CLUSTER, village::SCENE_Y);
    send(&mut screen, &mut game, &assets, Event::DoubleClick { x: ox + 36, y: oy + 24 });

    let c = &game.kingdom.counties[county as usize];
    assert_eq!(c.labour[cattle], 100, "cattle shed its surplus");
    assert_eq!(c.labour[grain], 100, "and grain was filled out of what it shed");
    assert_eq!(c.labour[idle], 100, "the hundred nobody wanted stay idle");
    assert_eq!(c.labour.iter().sum::<i32>(), 300);
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

    // **The sidebar with both blue outlines up**, which is the one shot a
    // reader can check the five interface fixes against: black strip text on
    // the parchment, the produce rows below the slider, a ring round the cow
    // because the dairy is overstaffed, and a ring round the slider's thumb
    // because somebody is idle.
    {
        let c = &mut game.kingdom.counties[county as usize];
        let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;
        c.labour[l2_kingdom::tables::JOB_IDLE_TOWNSFOLK] = 40;
        c.labour[cattle] = c.labour[cattle].max(120);
        c.labour_useful[cattle] = 40;
        c.labour_wanted[cattle] = -1;
        c.herd = c.herd.max(300);
        c.fields_cattle = c.fields_cattle.max(3);
    }
    let mut m = Machine::new(ScreenId::Campaign);
    let mut c = Ctx { game: &mut game, assets: &assets };
    let mut canvas = Canvas::screen();
    m.draw(&c, &mut canvas);
    let _ = &mut c;
    save_png(&canvas, &assets, "sidebar_blue_outline");

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

/// **The strings the two battle screens draw are the ones the original draws.**
///
/// Group and index are the only things a screen can get wrong that no pixel
/// assertion would notice: a window in the right place, full of the wrong
/// sentence, looks finished. So this reads the shipped `L2.eng` and pins the
/// six indices `Screen_BattlePrompt` and `Screen_BattleResult` actually use —
/// and the two they do not, which is the more interesting half.
///
/// **Group 80 indices 4, 5 and 6 and group 81 indices 1 through 7 are dead.**
/// Every `Eng_DrawString` on group 80 in the whole binary is index 0, 1, 2, 3
/// or 7, and every one on group 81 is index 0 or 8. *"The army of"*, *"are
/// victorious."* and the rest are never drawn anywhere: there is no victory
/// sentence on the result screen. They are asserted to exist and to be
/// unused, because a screen that composed one out of them would be inventing
/// a line the game never printed.
#[test]
fn the_battle_screens_draw_the_original_sentences() {
    let (_game, assets) = world!();
    let eng = &assets.shell;
    use l2_game::screens::battle::{GROUP_BANNER, GROUP_OWNERLESS, GROUP_PROMPT, GROUP_RESULT};

    assert_eq!(eng.text(GROUP_PROMPT, 0), "A Battle is to be fought.");
    assert_eq!(eng.text(GROUP_PROMPT, 1), "Will you take the field?");
    assert_eq!(
        eng.text(GROUP_PROMPT, 2),
        "Your opponent has the choice of whether or not to take the field."
    );
    assert_eq!(eng.text(GROUP_PROMPT, 7), "The Siege commences.");
    assert_eq!(eng.text(GROUP_RESULT, 0), "The Battle is decided.");
    assert_eq!(eng.text(GROUP_RESULT, 8), "The siege is over.");
    // Group 99 is one string, and it carries its own quotation marks.
    assert_eq!(eng.text(GROUP_OWNERLESS, 0), r#""The people.""#);

    // The dead ones exist and are not what any screen here reaches for.
    for (group, index) in [(GROUP_PROMPT, 4), (GROUP_PROMPT, 5), (GROUP_PROMPT, 6),
                           (GROUP_RESULT, 4), (GROUP_RESULT, 5), (GROUP_RESULT, 7)] {
        assert!(!eng.text(group, index).is_empty(), "{group}.{index} should exist");
    }

    // And all seven outcome pairs are there, heading and body, 2n and 2n + 1.
    for pair in 0..7usize {
        assert!(!eng.text(GROUP_BANNER, pair * 2).is_empty(), "banner {pair} heading");
        assert!(!eng.text(GROUP_BANNER, pair * 2 + 1).is_empty(), "banner {pair} body");
    }
    assert_eq!(eng.text(GROUP_BANNER, 0), "The Battle is won.");
    assert_eq!(eng.text(GROUP_BANNER, 12), "The conflict is over.");
    assert_eq!(eng.text(GROUP_BANNER, 14), "", "and there is no eighth pair");
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
/// being looked for, keep the pixels it would actually paint, and scan for that
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
/// there, and this is the assertion that says so in numbers rather than in a
/// screenshot somebody has to open.
///
/// Three claims, each with its own pixels:
///
/// 1. **It is drawn.** `Flags1a.pl8` frame `(shield − 1) * 8 + phase` — several
///    hundred opaque palette indices — stands somewhere on the canvas, exactly.
/// 2. **The frame is keyed on the shield.** Move the owning realm's
///    `shield_index` and the flag at the *same pixel* becomes the other
///    shield's frame. Nothing else on the campaign map reads `shield_index` —
///    the minimap and the menu-bar banner both read `realm_colour` — so this
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
/// pictures. A save that merely happened to have a garrison would almost always
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

/// **The minimap tints by owner, and a realm's ramp is its own.**
///
/// A player: *"the minimap had default colors, it didn't actually identify who
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
    // rather than from the picture.
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

/// **The sidebar stays live with the village open — and goes dead mid-drag.**
///
/// A player, having gone and checked against the original: *"The slider does
/// indeed still work with town square open and causes no issues."* He is right,
/// and `Screen_FrameInput`'s `g_screenId == 0x02` arm says so before any
/// village verb is reached — six guards, all of them the campaign map's
/// sidebar, `Labour_SplitSliderDrag` fourth among them. It did not work in
/// ours, because [`Machine::handle`] offered input to the top screen and
/// stopped there.
///
/// The second half is the half a person would never think to check, and it is
/// the reason this is a test rather than a one-line change: the `0x05`
/// (banding) and `0x06` (carrying) arms test **no sidebar guard at all**, so
/// the sidebar is dead for exactly as long as a peasant is in the air. Making
/// all three behave alike would look like a tidy-up and would be wrong. C59.
#[test]
fn the_sidebar_slider_still_works_with_the_village_open_but_not_mid_drag() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);
    let share = |g: &Game| g.kingdom.counties[county as usize].industry_share;

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));
    assert_eq!(m.depth(), 2, "the village is an inset over the map, not a replacement");

    // The press lands on the split slider's track, through the village.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: 561, y: 270 }, &mut c);
    }
    assert_eq!(share(&game), 60, "the sidebar's slider is dead with the village open");
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Village(county)], "and it stayed open");

    // Held and moved, still through the village: `FUN_00439122` runs on the
    // level and the movement, so this is the drag continuing.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Pointer { x: 541, y: 270 }, &mut c);
    }
    assert_eq!(share(&game), 20, "the drag tracks the pointer over the village too");

    // A click on the village's own half of the screen must NOT reach the map:
    // `Map_Click` is not in the `0x02` ladder. The county under the inset is
    // whatever it was; nothing selects a new one.
    let before = game.selected;
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: 200, y: 200 }, &mut c);
    }
    assert_eq!(game.selected, before, "a click on the map round the inset is not a map click");

    // And the drag states. Reaching `Phase::Band` needs a press inside the
    // village's own area and nine pixels of travel.
    let mut screen = VillageScreen::new(county);
    let top = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        VillageScreen::top_y(&ctx)
    };
    send(&mut screen, &mut game, &assets, Event::Click { x: 100, y: top + 60 });
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 140, y: top + 100 });
    assert_eq!(screen.phase(), village_screen::Phase::Band, "the band is up");
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: 561, y: 270 }),
        Transition::Stay,
        "screen 0x05 tests no sidebar guard, so the click must not pass to the map"
    );
}

/// **Closing a screen opened over the village closes the village with it**, and
/// that is the original's behaviour rather than a shortcut of ours.
///
/// The player, again from the real game: *"things that open a dialog will open
/// it and when you close that dialogue it will close town square and that
/// dialogue, probably something to fix so it only closes the dialog you
/// opened."* `docs/bugs.md` B63 records why it happens — `g_screenId` is one
/// byte and 57 of `Screen_FrameInput`'s 100 writes to it are the literal `0` —
/// and what a switch would cost. **We reproduce it on purpose**, so it needs a
/// test that fails if somebody quietly improves it. C59.
#[test]
fn a_screen_opened_over_the_village_takes_the_village_with_it_when_it_closes() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    // A sidebar button, clicked through the village. `FUN_00432967` is
    // `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` and it is the second
    // of the six guards. COURT rather than ARMY because a shell's right button
    // closes it and this test needs to watch it close.
    let button = map::SIDEBAR_BUTTONS[1].rect();
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x: button.x + 4, y: button.y + 4 }, &mut c);
    }
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Court],
        "the sidebar's screen replaced the village rather than stacking on it"
    );

    // And closing that screen lands on the map, not back on the village.
    {
        let mut c = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::RightClick { x: 320, y: 240 }, &mut c);
    }
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "a panel's exit is a constant 0, not a memory");
}

// ------------------------------------------------ seasons, fields, animation

/// **The map's artwork changes when the season does — driven by a real turn.**
///
/// `Gfx_LoadCountyMode` (`0x004984DC`) repoints all five near-zoom tile banks
/// at `(g_season - 1) * 8` in `g_resourceTable`, so the whole picture is
/// redrawn from different files. We hard-coded the `a` set and the map looked
/// the same in January and in August.
///
/// **This ends a turn rather than assigning to `season`.** A test that sets
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
/// next** — which is the thing the season swap could have broken and the reason
/// `install.rs::the_four_seasons_of_a_bank_are_the_same_frame_table` exists.
///
/// The overrides plane stores a **frame index**, not a picture. If frame 47 of
/// `Town1c.pl8` were a different cell of the sheet than frame 47 of
/// `Town1a.pl8`, every county town on the map would turn back into a quarry in
/// autumn — a defect a player would report as *"my buildings disappear"*. The
/// frame tables agree, so it does not happen, and this asserts the consequence
/// at the pixel rather than the claim in the file.
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
    // list is now the values the game can actually write to a farm tile —
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

/// **The village animates, and `villani1.pl8` is what the iron mine animates
/// from.**
///
/// `Village_Animate` (`0x00412421`) draws six overlays and the sixth is the
/// only read of `villani1.pl8` in the whole executable — a file this project
/// had recorded as *"loaded by nothing"*. Three of the six are unconditional
/// and run in every county.
///
/// The clock is checked as *pulses*, not as pixels alone: eighty milliseconds
/// is one step of the fast counter and a hundred and sixty of the slow one,
/// which is `FUN_004BBC80`'s divider chain and not a rate of ours.
#[test]
fn the_village_animates_and_the_iron_mine_comes_out_of_villani1() {
    let (mut game, assets) = world!();
    let art = assets.village.as_ref().expect("the village artwork");
    assert!(art.has_villani1(), "villani1.pl8 is in the install and the iron mine needs it");

    let iron = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].industry[1].has_resource)
        .expect("England has an iron county");

    let mut screen = VillageScreen::new(iron as u8);
    let first = draw(&mut screen, &mut game, &assets);

    // One slow pulse: 160 ms at the 16 ms tick is ten ticks, and every one of
    // the six overlays has moved at least once by then.
    let ticks = village::PULSE_SLOW_MS / VillageScreen::TICK_MS;
    assert_eq!(ticks, 10, "160 ms is ten fixed ticks");
    for _ in 0..ticks {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert!(screen.take_redraw(), "a moved animation asks for a repaint");
    let later = draw(&mut screen, &mut game, &assets);
    let moved = first.diff_count(&later);
    assert!(moved > 40, "the village is still after ten ticks: {moved} pixels moved");

    // The clock is display state and nothing else: stepping it must not touch
    // the world. If it ever did, this is the assertion that would say so.
    let before = game.kingdom.clone();
    for _ in 0..100 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert_eq!(game.kingdom, before, "the animation clock reached the simulation");
}

/// **`villani2.pl8`'s own frame table confirms every one of the six overlays,
/// independently of the decompilation.**
///
/// The six runs in [`village::OVERLAYS`] — their first frame and their length —
/// were read out of `Village_Animate`'s counter bounds. The sheet was never
/// looked at. Looking at it: `villani2.pl8`'s 44 frames fall into **five blocks
/// of equal-sized frames laid out in rows on the artist's canvas**, and the
/// blocks are
///
/// | frames | size | overlay |
/// |---|---|---|
/// | 0 … 6 | 26 × 29 | the stone quarry's, 7 frames |
/// | 7 … 14 | 39 × 40 | the lumber camp's, 8 |
/// | 15 … 24 | 15 × 12 | the third unconditional one, 10 |
/// | 25 … 32 | 32 × 42 | the first unconditional one, 8 |
/// | 33 … 39 | 19 × 18 | the second unconditional one, 7 |
///
/// which is **exactly** the six-entry table, start index and length, five times
/// over. What is left is frames 40, 41 and 43 — the three static buildings
/// `Village_Draw` blits at `0x28`, `0x29` and `0x2B` — and one 2 × 2 stub at
/// 42. Nothing over, nothing short.
///
/// A block boundary that fell one frame from where the counter wraps would show
/// up here as an animation that jumps to a different-sized picture, and this is
/// the assertion that would catch it. Sizes are the evidence and the
/// decompilation is the claim; they agree.
#[test]
fn the_animation_runs_are_the_blocks_the_sheet_is_laid_out_in() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so there is no sheet to read");
    };
    let path = std::fs::read_dir(&dir)
        .ok()
        .and_then(|d| {
            d.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.eq_ignore_ascii_case("villani2.pl8"))
            })
        })
        .expect("villani2.pl8 is in the install");
    let bytes = std::fs::read(path).expect("villani2.pl8 reads");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("villani2.pl8 decodes");
    assert_eq!(pl8.frames.len(), 44);

    for overlay in village::OVERLAYS.iter().filter(|o| !o.villani1) {
        let run = &pl8.frames[overlay.first..overlay.first + overlay.frames];
        let size = (run[0].width, run[0].height);
        for (i, f) in run.iter().enumerate() {
            assert_eq!(
                (f.width, f.height),
                size,
                "frame {} of the run at {} is a different size",
                overlay.first + i,
                overlay.first
            );
        }
        // The frame *before* the run and the frame *after* it must both be a
        // different size, or the boundary is not where the counter wraps.
        if overlay.first > 0 {
            let prev = &pl8.frames[overlay.first - 1];
            assert_ne!(
                (prev.width, prev.height),
                size,
                "the run at {} starts one frame late: {} is the same size",
                overlay.first,
                overlay.first - 1
            );
        }
        let after = overlay.first + overlay.frames;
        let next = &pl8.frames[after];
        assert_ne!(
            (next.width, next.height),
            size,
            "the run at {} is one frame short: {after} is the same size",
            overlay.first
        );
    }

    // The three static buildings are the three big frames past the animation
    // blocks, and the sheet has nothing else in it.
    for (_, frame, _, _) in village::RESOURCE_BUILDINGS {
        let f = &pl8.frames[frame];
        assert!(f.width > 100 && f.height > 70, "frame {frame:#04X} is not a building");
    }
    eprintln!("villani2: five animation blocks and three buildings account for all 44 frames");
}

/// **Every overlay's frame run is inside the sheet it is indexed against**, and
/// the two counters nothing draws are recorded rather than drawn.
///
/// A frame index off the end of a PL8 is a hole rather than a crash here, so
/// without this the six runs could be wrong by any amount and nothing would
/// say so.
///
/// # `villani1.pl8` holds 21 frames and the shipped game plays 18 of them
///
/// **[V]** for both numbers. The iron mine's counter, `DAT_004D2938`, wraps at
/// `0x11`, so it visits 0 … 17 and three frames of the file are never drawn.
///
/// And `Village_Animate`'s **dead** counter `DAT_004D2934` wraps at `0x14` —
/// **21 states, which is exactly the file's frame count** — and is read by
/// nothing in the executable. **[I]**, and deliberately only that: the two
/// numbers agreeing is a striking coincidence and it is not a demonstration
/// that the mine was meant to run off that counter. `docs/bugs.md` B65 records
/// the numbers and says the same thing.
#[test]
fn every_village_overlay_run_fits_inside_its_own_sheet() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so there are no sheets to index");
    };
    let read = |name: &str| -> Option<Vec<u8>> {
        std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.eq_ignore_ascii_case(name))
            })
            .and_then(|p| std::fs::read(p).ok())
    };
    let counts: Vec<usize> = ["villani1.pl8", "villani2.pl8"]
        .iter()
        .map(|n| {
            let b = read(n).unwrap_or_else(|| panic!("{n} is in the install"));
            l2_formats::Pl8::parse(&b).unwrap_or_else(|e| panic!("{n}: {e}")).frames.len()
        })
        .collect();
    assert_eq!(counts[0], 21, "villani1.pl8 holds 21 frames");
    assert_eq!(counts[1], 44, "villani2.pl8 holds 44");

    let mut clock = village::AnimationClock::new();
    let mut highest = [0usize; 2];
    // Several full turns of the longest run, so every counter visits every
    // value it can take.
    for _ in 0..18 * 4 {
        clock.tick(village::PULSE_SLOW_MS);
        for overlay in &village::OVERLAYS {
            let f = clock.frame_of(overlay);
            let sheet = usize::from(!overlay.villani1);
            assert!(
                f < counts[sheet],
                "frame {f} is off the end of {} ({} frames)",
                if overlay.villani1 { "villani1.pl8" } else { "villani2.pl8" },
                counts[sheet]
            );
            highest[sheet] = highest[sheet].max(f);
        }
    }
    // The iron mine stops at 17, three frames short of the file's end. That
    // is the original's, not a clamp of ours: `DAT_004D2938` wraps at `0x11`.
    assert_eq!(highest[0], 17, "the iron mine's counter visits 0 … 17");
    assert_eq!(counts[0] - (highest[0] + 1), 3, "three frames of villani1 are never drawn");

    // The two counters `Village_Animate` steps and nothing reads. Kept as a
    // claim so that finding a consumer later fails this and gets looked at.
    assert_eq!(village::DEAD_COUNTER_PERIODS.len(), 2);
    assert!(
        village::OVERLAYS.iter().all(|o| o.frames != 21),
        "no overlay uses the 21-state counter, which is why it is called dead"
    );
    // …and the coincidence, asserted so that it stays visible: the dead
    // counter has exactly as many states as villani1.pl8 has frames.
    assert_eq!(
        village::DEAD_COUNTER_PERIODS[0].1,
        counts[0],
        "the 21-state dead counter and villani1's 21 frames"
    );
}

// ---------------------------------------------------- the county-strip emboss

/// The glyph mask of `s` in the body font, as offsets from the string's origin.
fn body_mask(assets: &Assets, s: &str) -> Vec<(i32, i32)> {
    let font = assets.shell.body.as_ref().expect("the body font");
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let (w, h) = (font.width(s).max(1), font.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect()
}

/// **The emboss pair a line was actually drawn with, read back off the canvas.**
///
/// `Ui_DrawText` draws each glyph three times, in this order: at `y - 1` in the
/// *up* colour, at `y + 1` in the *down* colour, then at `y` in its own. So the
/// final colour of a pixel is decided by which of the three masks it is in,
/// later passes winning:
///
/// * in the glyph mask → the text colour;
/// * else in the mask shifted **down** one → the *down* shadow;
/// * else in the mask shifted **up** one → the *up* shadow.
///
/// Reading those two sets back is exact, and it is not a colour picked by eye:
/// every pixel of each set has to agree or this returns `None`. The two shadow
/// colours come out as palette indices, which is the form the binary states
/// them in.
fn emboss_at(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(u8, u8)> {
    let mask = body_mask(assets, s);
    let (ox, oy) = find_body(canvas, assets, s, colour)?;
    let inside = |dx: i32, dy: i32| mask.contains(&(dx, dy));
    let mut up: Option<u8> = None;
    let mut down: Option<u8> = None;
    for &(mx, my) in &mask {
        // One row below a glyph pixel, and not itself a glyph pixel: the
        // *down* shadow, drawn second and never overpainted.
        if !inside(mx, my + 1) {
            let got = canvas.at((ox + mx) as usize, (oy + my + 1) as usize);
            if *down.get_or_insert(got) != got {
                return None;
            }
        }
        // One row above, in neither the glyph mask nor the down mask: the *up*
        // shadow, which is drawn first and so loses both overlaps.
        if !inside(mx, my - 1) && !inside(mx, my - 2) {
            let got = canvas.at((ox + mx) as usize, (oy + my - 1) as usize);
            if *up.get_or_insert(got) != got {
                return None;
            }
        }
    }
    Some((up?, down?))
}

/// **Two different emboss colours in one plate, and one of them is the
/// original's own bug.**
///
/// A player reported both halves and was right about both:
///
/// > *"There's a bug in the original where the town name's embossing against
/// > the cloudy background … still has the emboss colour of the parchment that
/// > you see on a town you own, that blends it in with the parchment. But the
/// > OG correctly has the 'sovereign land of the baron' properly tinged in
/// > grey."*
///
/// `CountyStrip_Draw` (`0x0040F7D3`) is the function, and it draws the name
/// with `DAT_0058FE9C` clear and the three *Sovereign land of …* lines with it
/// set — the only thing in the whole plate that changes it:
///
/// ```c
/// Pl8_DrawFrameHere(g_miscCtySheet, 0x3a, 0x1de, 0x9c);            /* the cloudy plate */
/// Ui_DrawCentred(100, …, 0x1e0, 0xb4, 0xa0, &g_fontBody, 0x3f);    /* the name */
/// if (owner != 0) {
///   DAT_0058fe9c = 1;
///   Ui_DrawCentred(0xf, 0, …);  Ui_DrawCentred(0xf, 1, …);  FUN_004025d7(name, …);
///   DAT_0058fe9c = 0;
/// }
/// ```
///
/// and `Ui_DrawText`'s two branches give `0x10`/`0x1F` — `rgb(81,73,53)` over
/// `rgb(247,223,134)`, the parchment — and `0x3F`/`0x26` —
/// `rgb(0,0,0)` over `rgb(202,202,202)`, the grey. Both pairs are palette
/// indices out of the executable; neither was chosen to look right.
///
/// **The fixture used for each case.** `england-turn1.sav`, whose five owned
/// counties are 1, 4, 8, 11 and 13 with one realm each: county 8 is the
/// player's, county 1 belongs to realm 5, and county 2 belongs to nobody. There
/// is no fixture on this project in which one realm holds two counties, so the
/// owned case is the player's own county and nothing else.
#[test]
fn the_county_name_keeps_the_parchment_emboss_and_the_sovereign_lines_do_not() {
    let (mut game, assets) = world!();
    if assets.shell.body.is_none() {
        l2_testkit::skip!("no Fntl2_14.pl8, so nothing is embossed");
    }
    let parchment = l2_game::shell::font::SHADOW;
    let grey = l2_game::shell::font::SHADOW_GREY;
    assert_ne!(parchment, grey, "the two pairs are different, which is the whole point");

    // --- a county another lord holds: name in parchment, banner in grey.
    assert_eq!(game.kingdom.counties[1].owner, 5);
    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);

    assert_eq!(
        emboss_at(&canvas, &assets, &name, STRIP_INK),
        Some(parchment),
        "the county's name is embossed in the parchment pair — the original's own bug"
    );
    let realm5 = l2_view::chrome::realm_pen(game.kingdom.realms[5].shield_index)
        .expect("realm 5 flies a shield");
    let banner = assets.shell.text(15, 0).to_string();
    let banner = if banner.is_empty() { "SOVEREIGN LAND".to_string() } else { banner };
    assert_eq!(
        emboss_at(&canvas, &assets, &banner, realm5),
        Some(grey),
        "the Sovereign land line is embossed in the grey pair"
    );

    // --- the player's own county: the same parchment emboss, on the plate it
    // was designed for, and no banner at all.
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 8);
    assert_eq!(
        emboss_at(&canvas, &assets, &name, STRIP_INK),
        Some(parchment),
        "and on the owned plate the same pair is correct"
    );
    assert!(
        find_body(&canvas, &assets, &banner, realm5).is_none(),
        "your own county carries no Sovereign land line"
    );
}

/// **Unclaimed land has no *Sovereign land of* line — a third case, not a
/// second.** **[V]**
///
/// `CountyStrip_Draw`'s else-arm draws the cloudy plate and the name for any
/// county that is not yours, and guards the three extra lines with
/// `owner != 0`. `L2.eng` group 15 holds exactly two strings, `"Sovereign
/// land"` and `"of"`, and the third line is a lord's name out of
/// `g_playerNames` — there is no wording in the file for a county nobody owns,
/// because the original never needs one.
///
/// Ours drew `SOVEREIGN LAND / OF / UNCLAIMED`, a sentence the original cannot
/// produce. The player reported it in one line: *"Unclaimed lands have no
/// 'sovereign land of'."*
#[test]
fn an_unclaimed_county_shows_its_name_and_nothing_else() {
    let (mut game, assets) = world!();
    let unclaimed = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == 0)
        .expect("England turn one has counties nobody holds");

    let mut screen = CountyScreen::new(unclaimed as u8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, unclaimed as u8);

    // The name is there, on the cloudy plate's lower line…
    assert_eq!(
        find_body(&canvas, &assets, &name, STRIP_INK).map(|p| p.1),
        Some(180),
        "an unclaimed county still gets its name at 0xB4"
    );
    // …and nothing under it. Every colour the banner could be drawn in is
    // checked, so this cannot pass by looking for the wrong one.
    let banner = assets.shell.text(15, 0).to_string();
    let banner = if banner.is_empty() { "SOVEREIGN LAND".to_string() } else { banner };
    for colour in assets.ink.realm.iter().copied().chain([STRIP_INK, assets.ink.text]) {
        assert!(
            find_body(&canvas, &assets, &banner, colour).is_none(),
            "an unclaimed county must not claim a sovereign, and one was drawn in {colour}"
        );
    }
    assert!(
        find_body(&canvas, &assets, "UNCLAIMED", assets.ink.text).is_none(),
        "and it must not invent a lord called UNCLAIMED"
    );
}

/// **The quirk switch turns the county name's emboss grey, and only that.**
///
/// Default off — the original's behaviour is what ships — and it lives on
/// [`l2_game::game::Quirks`], which is display state that never reaches the
/// simulation. See `docs/bugs.md` B64.
#[test]
fn the_grey_county_name_quirk_changes_the_emboss_and_nothing_else() {
    let (mut game, mut assets) = world!();
    if assets.shell.body.is_none() {
        l2_testkit::skip!("no Fntl2_14.pl8, so nothing is embossed");
    }
    assert!(!assets.quirks.grey_county_name, "the original's behaviour is the default");

    let mut screen = CountyScreen::new(1, Panel::Tax);
    let plain = draw(&mut screen, &mut game, &assets);
    assets.quirks.grey_county_name = true;
    let fixed = draw(&mut screen, &mut game, &assets);

    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);
    assert_eq!(
        emboss_at(&plain, &assets, &name, STRIP_INK),
        Some(l2_game::shell::font::SHADOW),
        "off: the parchment pair"
    );
    assert_eq!(
        emboss_at(&fixed, &assets, &name, STRIP_INK),
        Some(l2_game::shell::font::SHADOW_GREY),
        "on: the grey pair the Sovereign lines already use"
    );

    // It moves the name's emboss and leaves everything else alone: the
    // difference is a few hundred pixels around one line, not a redrawn panel.
    let moved = plain.diff_count(&fixed);
    assert!(moved > 0, "the switch does something");
    assert!(moved < 4_000, "and only around the name: {moved} pixels");

    // And on the player's own county it changes nothing at all — the defect is
    // the parchment emboss over the *cloudy* plate, and the owned plate really
    // is parchment.
    let mut screen = CountyScreen::new(8, Panel::Tax);
    assets.quirks.grey_county_name = false;
    let plain = draw(&mut screen, &mut game, &assets);
    assets.quirks.grey_county_name = true;
    let fixed = draw(&mut screen, &mut game, &assets);
    assert_eq!(plain.diff_count(&fixed), 0, "your own county's name is right as it is");
}


// ===========================================================================
// The input arms of the right-hand column, the menu bar and the county panels
//
// `docs/arms.json`, groups `right-column`, `menu-bar`, `county-panels`,
// `village` and `management-screens`. Every test below names the arm it is
// about; the point of them is that an arm can only be shown to be live from the
// screen a player actually has on top, and half of these arms are answered by a
// screen that is not the top one.
// ===========================================================================

/// **The whole right-hand column is live under an open county panel**, and each
/// of its five controls is checked separately because they are five separate
/// functions in `Screen_FrameInput`'s ladder.
///
/// `docs/arms.json` `0x0042FF10/inset-runs-the-sidebar-guards`. This is the arm
/// that was missing: `CountyScreen::handle` tested the four strip quadrants
/// itself and returned `Stay` for everything else in the column, so with the tax
/// panel open a player could not touch the minimap, the five sidebar buttons,
/// the farm/industry slider, the produce rows or End Turn.
#[test]
fn a_county_panel_leaves_the_whole_sidebar_live_underneath_it() {
    let (mut game, assets) = world!();
    game.select(8);
    assert!(game.is_players(8), "the fixture's county 8 is the local player's");

    // 1. A sidebar button - the court, which is ungated.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let court = map::SIDEBAR_BUTTONS[1].rect();
    send_stack(&mut m, &mut game, &assets, Event::Click { x: court.centre_x(), y: court.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Court), "sidebar button 2 opens the court");
    assert_eq!(m.depth(), 2, "and it replaced the panel rather than stacking on it");

    // 2. A minimap mode icon. The panel stays open; what changes is the map.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let mode = map::MINIMAP_MODE_BUTTONS[0];
    send_stack(&mut m, &mut game, &assets, Event::Click { x: mode.centre_x(), y: mode.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::County(8, Panel::Tax)), "the panel is still open");

    // 3. The farm/industry split slider, which moves a real number.
    let mut m = over_the_map(ScreenId::County(8, Panel::Ration));
    game.kingdom.counties[8].industry_share = 40;
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 531, y: 270 });
    assert_eq!(
        game.kingdom.counties[8].industry_share, 0,
        "the press lands on the track's left edge, which is share 0"
    );

    // 4. A produce row, which opens the job popup for that row's labour slot.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let rows = county::farm_rows(&game.kingdom.counties[8]);
    assert!(!rows.is_empty(), "the fixture's county 8 farms something");
    let pitch = county::farm_pitch(rows.len());
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 500, y: 0x12E + pitch / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, rows[0])), "the first farm row's job popup");

    // 5. End Turn, which is record 5 of the same hotspot table and whose whole
    //    rectangle a BACK TO MAP button of ours used to sit on.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 550, y: 470 });
    assert_ne!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "End Turn is reachable through the panel"
    );
}

/// **The panel keeps its own two ways out**, so the pass above cannot be
/// "everything falls through", and a click on the panel itself is neither.
#[test]
fn a_county_panel_still_closes_on_its_corner_and_on_the_right_button() {
    let (mut game, assets) = world!();
    game.select(8);

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let ok = Panel::Tax.ok_button();
    send_stack(&mut m, &mut game, &assets, Event::Click { x: ok.centre_x(), y: ok.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Ui_OkButtonClicked's 24 x 24 corner");

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 300, y: 200 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "a right release anywhere");

    // The ablation: a click on the middle of the panel does nothing at all. Both
    // assertions above would still pass if `handle` closed on any click.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 200, y: 200 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "a click on the panel is not an exit"
    );
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

/// **`Menu_HitTitle` measures the words**, so the 32-pixel gap between two
/// titles is dead bar.
///
/// Install-gated by `world!`, so the widths are the shipped `Fntl2_14.pl8`'s
/// through the shipped `L2.eng`'s own captions rather than our 5 x 7 fallback's.
#[test]
fn the_menu_bar_titles_are_their_own_words_wide_with_a_dead_gap_between_them() {
    let (mut game, assets) = world!();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let t = menubar::titles(&ctx);

    assert_eq!(t[0].x, 10, "g_menuBarItems[0].x");
    for r in &t {
        assert_eq!(r.y, 6, "every record's y");
        assert_eq!(r.h, 12, "Menu_HitTitle's fixed 12-pixel height");
        assert!(r.w > 0 && r.w < 120, "a caption, not a rectangle: {r:?}");
    }
    assert_eq!(t[1].x - (t[0].x + t[0].w), 32, "g_penAdvance += 0x20");
    assert_eq!(t[2].x - (t[1].x + t[1].w), 32);

    let gap = t[0].x + t[0].w + 8;
    assert!(menubar::title_at(&ctx, gap, 10).is_none(), "the gap hits nothing");
    assert_eq!(menubar::title_at(&ctx, t[1].x + 1, 10), Some(1));
    assert!(menubar::title_at(&ctx, t[1].x + 1, 18).is_none(), "y 18 is past 6 + 12");
}

/// **The menu bar opens, hovers, picks and closes** - the four arms of screen
/// `0x32`, driven as a player drives them.
#[test]
fn the_menu_bar_opens_a_dropdown_and_its_items_reach_their_screens() {
    let (mut game, assets) = world!();
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };

    // Menu_OpenDropdown: a press on a title.
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[0].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)), "the File menu is open");

    // FUN_0040DD92's button-up half: the pointer on another title switches it.
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: titles[2].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(2)), "sliding onto Help switches the menu");
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: titles[0].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)));

    // FUN_0040E099 and the pick: File's second item is Load, screen 0x35.
    let row = menubar::item_rect(&titles, 0, 1);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: row.x + 4, y: row.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::SaveLoad(SaveLoadMode::Load)), "File / Load");
    assert_eq!(m.depth(), 2, "and it landed where the drop-down was, over the map");

    // FUN_0040DF62: a press that is on no row closes and does nothing else, and
    // the five pixels between two rows belong to nothing at all.
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[1].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(1)));
    let dead = menubar::item_rect(&titles, 1, 0);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: dead.x + 4, y: dead.y + dead.h + 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "a press between two rows closes the menu");

    // And the right button closes it.
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[1].x + 2, y: 10 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 300, y: 300 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

/// **The Options and Help menus reach the four option screens**, which had no
/// route into them but the index screen.
#[test]
fn the_options_menu_reaches_the_four_option_screens() {
    let (mut game, assets) = world!();
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };
    for (menu, item, page) in [
        (1usize, 0usize, OptionsPage::Advanced),
        (1, 1, OptionsPage::Sound),
        (1, 2, OptionsPage::Display),
        (2, 0, OptionsPage::Help),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[menu].x + 2, y: 10 });
        let row = menubar::item_rect(&titles, menu, item);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: row.x + 4, y: row.y + 4 });
        assert_eq!(m.top_id(), Some(ScreenId::Options(page)), "menu {menu} item {item}");
    }
}

/// **A right click during the village's drag gesture does not leave the
/// village.** `docs/arms.json` `0x0042FF10/carry-right-cancels`.
///
/// This is the arm that was *wrong* rather than missing: the screen popped from
/// every phase, so a player who picked peasants up and changed his mind lost the
/// village with them. `0x06`'s arm is `g_screenId = 0x02`, not 0.
#[test]
fn a_right_click_during_a_peasant_drag_cancels_the_drag_and_not_the_village() {
    let (mut game, assets) = world!();
    game.select(8);
    let top = 64;

    // Press, travel more than nine pixels, and the band is up (screen 0x05).
    let mut m = over_the_map(ScreenId::Village(8));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 100, y: top + 60 });
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: 160, y: top + 110 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Village(8)),
        "0x05 has no right-button arm at all, so the click is swallowed"
    );

    // Release: 0x06 if the band caught anybody, 0x02 if it did not. Either way
    // a right click now must keep the village.
    send_stack(&mut m, &mut game, &assets, Event::Release { x: 160, y: top + 110 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });

    // The idle village, by contrast, leaves on the right button - which is the
    // ablation: if the fix were "the village never leaves on a right click",
    // this would fail.
    let mut idle = over_the_map(ScreenId::Village(8));
    send_stack(&mut idle, &mut game, &assets, Event::RightClick { x: 160, y: top + 110 });
    assert_eq!(
        idle.top_id(),
        Some(ScreenId::Campaign),
        "the idle village really does leave on a right click"
    );
}

/// **`Labour_SplitSliderDrag`'s three zones are half-open, and x = 594 steps
/// up.** The one column that used to fall on the track is the whole test.
#[test]
fn the_split_slider_steps_up_at_594_and_refuses_a_county_you_do_not_hold() {
    // Pure arithmetic, so this half needs no install.
    assert_eq!(map::split_from_click(593, 40), 100, "593 is still the track, and the track clamps at 100");
    assert_eq!(map::split_from_click(594, 40), 44, "594 is the first pixel of the up zone");
    assert_eq!(map::split_from_click(530, 40), 36, "530 is the last pixel of the down zone");
    assert_eq!(map::split_from_click(531, 40), 0, "531 is the first pixel of the track");
    assert_eq!(map::split_from_click(639, 100), 100, "clamped at 100");
    assert_eq!(map::split_from_click(0, 0), 0, "and at 0");

    let (mut game, assets) = world!();
    // The ownership gate, which `Labour_SplitSliderDrag` tests on its second
    // line and ours did not test at all.
    let other = (1..game.kingdom.counties.len() as u8)
        .find(|&id| game.kingdom.counties[id as usize].owner != game.player);
    let Some(other) = other else { return };
    game.select(other);
    let before = game.kingdom.counties[other as usize].industry_share;
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 560, y: 270 });
    assert_eq!(
        game.kingdom.counties[other as usize].industry_share, before,
        "another lord's peasants do not move"
    );
}

/// **`CountyStrip_JobClick`'s geometry**: both columns, both pitch rules, and
/// the ways it refuses.
#[test]
fn the_produce_rows_map_to_labour_slots_by_column_and_pitch() {
    let mut c = l2_kingdom::county::County::new();
    assert!(county::farm_rows(&c).is_empty());
    assert_eq!(county::job_row_at(&c, 500, 0x140), None, "an empty column refuses");

    // Cattle, then grain, then reclamation - `FUN_0040FEC1`'s own order, which
    // is not the labour slots' order.
    c.fields_cattle = 1;
    c.fields_grain = 1;
    assert_eq!(county::farm_rows(&c), vec![1, 0]);
    assert_eq!(county::farm_pitch(2), 0x3C, "two rows keep the tall pitch");
    assert_eq!(county::job_row_at(&c, 500, 0x12E), Some(1), "row 0 is the dairy");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x3C), Some(0), "row 1 is grain");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x78), None, "row 2 is past the end");

    c.fields_reclaiming = 1;
    assert_eq!(county::farm_rows(&c), vec![1, 0, 2]);
    assert_eq!(county::farm_pitch(3), 0x2D, "three rows close up");
    assert_eq!(county::job_row_at(&c, 500, 0x12E + 0x5A), Some(2), "reclamation, tight pitch");

    // The industry column is a different list, order and pitch rule.
    for i in &mut c.industry {
        i.enabled = false;
    }
    c.industry[0].enabled = true;
    c.industry[1].enabled = true;
    assert_eq!(county::industry_rows(&c), vec![6, 4], "wood then iron, not the array order");
    assert_eq!(county::job_row_at(&c, 0x230, 0x12E), Some(6), "x 0x230 is the industry column");
    assert_eq!(county::job_row_at(&c, 0x22F, 0x12E), Some(1), "and 0x22F is still the farm one");
    c.industry[3].enabled = true;
    c.industry[2].enabled = true;
    c.castle_degraded = 1;
    assert_eq!(county::industry_rows(&c), vec![6, 4, 5, 7, 3], "five rows, the castle last");
    assert_eq!(county::industry_pitch(5), 0x1E, "five rows get the 30-pixel pitch");
    assert_eq!(county::industry_pitch(3), 0x2D);
    assert_eq!(county::industry_pitch(2), 0x3C);

    // The plate's own bounds.
    assert_eq!(county::job_row_at(&c, 500, 0x12D), None, "one row above the plate");
    assert_eq!(county::job_row_at(&c, 500, 0x1AE), None, "one row below it");
    assert_eq!(county::job_row_at(&c, 477, 0x140), None, "one column left of the sidebar");
}

/// herd.**
///
/// A player with the build in front of him: *"why do the pastures not have cows
/// in them?"* Because `FUN_004071A0`'s overlay pass had three of its four arms
/// and not the farm one.
///
/// A canvas diff would pass on a garbage sprite or on the wrong frame of the
/// right sheet, so this asserts three things a diff cannot:
///
/// 1. **the herd sprite lands where the original puts it** — tile origin plus
///    (+4, −4) — by requiring the pixels the sheet holds at a named position to
///    be on the canvas at the position the placement predicts;
/// 2. **it is not there before**. The base layer is painted first and compared,
///    so what is being measured is the overlay pass rather than the meadow;
/// 3. **the picture changes with the herd**, driven from `County::herd` through
///    the season pass rather than by writing a terrain byte — which is
///    `docs/agents.md`'s *"a field is only tested if something a test reads was
///    written by something the game runs."*
#[test]
fn the_pastures_have_cattle_in_them_and_the_herd_chooses_which() {
    let (mut game, assets) = world!();

    // A real pasture of the England position, and the county that grazes it.
    let (county, tile) = {
        let k = &game.kingdom;
        (1..=k.county_count)
            .find_map(|id| {
                (0..l2_kingdom::MAX_FIELDS)
                    .filter_map(|s| k.counties[id].field_tile(s))
                    .find(|&t| {
                        l2_kingdom::field::classify(k.campaign.map.terrain[t])
                            == l2_kingdom::field::FieldType::Pasture
                    })
                    .map(|t| (id, t))
            })
            .expect("the England position has pastures")
    };
    let terrain = game.kingdom.campaign.map.terrain[tile];
    assert!(
        (0x14..=0x16).contains(&terrain),
        "the original's own save already carries a stocked pasture here, not {terrain:#04X}",
    );

    let (fx, fy) = l2_kingdom::map::coords(tile);
    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    let (frame_index, (dx, dy)) =
        campaign::herd_sprite(terrain, 0).expect("a stocked pasture draws animals");

    // **The offset is pinned against the decompilation, not against itself.**
    // The first version of this test computed the probe pixel *from*
    // `campaign::HERD_AT` and then asserted the sprite was there — so setting
    // that constant to `(0, 0)` moved both sides of the comparison and the test
    // stayed green. A check passing for an accidental reason, caught by
    // ablating the exact line it claims to be about. `FUN_004071A0`'s farm arm
    // is `local_30 = 4; local_34 = -4;` for content `0x14 … 0x16`, and that
    // literal has to be written here or nothing anchors it.
    assert_eq!((dx, dy), (4, -4), "FUN_004071A0 offsets a stocked pasture by (+4, -4)");
    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8");
    let sprite = sheet.frame(frame_index).expect("the herd frame");

    // Where `draw_herd` puts it, computed the way the caller does.
    let (row, col) = campaign::tile_to_cell(fx as usize, fy as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let (ox, oy) = (sx + dx, sy + dy);

    // A pixel of the sprite that is opaque and not at its edge, so a one-pixel
    // placement error moves it off.
    let probe = (0..sprite.opaque.len())
        .find(|&i| {
            let (px, py) = (i % sprite.width as usize, i / sprite.width as usize);
            sprite.opaque[i]
                && px > 2
                && py > 2
                && px + 3 < sprite.width as usize
                && py + 3 < sprite.height as usize
        })
        .expect("the herd sprite has an interior");
    let (px, py) = (probe % sprite.width as usize, probe / sprite.width as usize);
    let want = sprite.indices[probe];
    let (tx, ty) = ((ox + px as i32) as usize, (oy + py as i32) as usize);

    let mut with = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut with);
    }
    assert_eq!(
        with.at(tx, ty),
        want,
        "the herd sprite is not at the tile origin plus {:?}",
        (dx, dy),
    );

    // Claim 2: the meadow underneath is a different colour there, so what was
    // asserted above is the overlay and not the terrain.
    let mut without = Canvas::screen();
    {
        let slot = assets.slot(game.map_slot).expect("the map slot");
        let lattice = campaign::Lattice::build(&slot);
        let overrides = {
            let ctx = Ctx { game: &mut game, assets: &assets };
            MapScreen::tile_graphics(&ctx)
        };
        let mut tags = l2_view::Tags::screen();
        campaign::draw(
            &mut without,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            &overrides,
            game.kingdom.season,
        );
    }
    assert_ne!(
        without.at(tx, ty),
        want,
        "the bare meadow already had this pixel, so the assertion above measures the artwork",
    );

    // Claim 3: kill the herd, run a season, and the animals go — through
    // `Herd_UpdateCrowding`, not through a terrain byte a test wrote.
    game.kingdom.counties[county].herd = 0;
    game.kingdom.advance_season();
    assert_eq!(
        game.kingdom.campaign.map.terrain[tile],
        l2_kingdom::field::terrain::PASTURE,
        "an empty herd leaves bare pasture",
    );
    assert!(
        campaign::herd_sprite(l2_kingdom::field::terrain::PASTURE, 0).is_none(),
        "and nothing is drawn on it",
    );
    let mut empty = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut empty);
    }
    assert_ne!(empty.at(tx, ty), want, "the cattle are still on the map with no herd to draw");
}

/// **The herd's animation phase never reaches the simulation.**
///
/// `docs/netcode.md` D-12: an animation clock is display state.
///
/// **The first version of this test was wrong, and the way it was wrong is
/// the one `docs/agents.md` warns about.** It ticked `MapScreen::update` a
/// hundred times and required the kingdom's checksum not to move. It moved,
/// and not because of the clock: `update` also runs `Units_Tick`, picks up a
/// suspended turn and edge-scrolls. The experiment was structurally incapable
/// of measuring the thing it was run to measure, and it returned a clean
/// number either way.
///
/// So this asserts what is actually true and actually checkable, in two
/// halves that are different in kind:
///
/// 1. **The compiler owns the safety.** `Screen::draw` takes `&Ctx`, so a
///    renderer cannot reach the simulation at all - which is a stronger
///    guarantee than any number this test could compare, and the reason the
///    phase lives on the screen rather than on the `Kingdom`. Drawing the
///    same world at every phase and comparing the checksum is a *witness* to
///    that, not the proof.
/// 2. **The clock has to actually animate**, or the phase is display state
///    nobody would notice was broken. Six phases of a stocked pasture must
///    produce more than one picture.
#[test]
fn the_grazing_clock_changes_the_picture_and_cannot_change_the_world() {
    let (mut game, assets) = world!();
    let tile = {
        let k = &game.kingdom;
        (1..=k.county_count)
            .find_map(|id| {
                (0..l2_kingdom::MAX_FIELDS)
                    .filter_map(|s| k.counties[id].field_tile(s))
                    .find(|&t| (0x14..=0x16).contains(&k.campaign.map.terrain[t]))
            })
            .expect("a stocked pasture")
    };
    let (fx, fy) = l2_kingdom::map::coords(tile);
    let terrain = game.kingdom.campaign.map.terrain[tile];

    // Six phases, six frames of one meadow. Distinct *frames* first, because
    // that is the claim about the ladder rather than about the artwork.
    let frames: Vec<usize> = (0..campaign::HERD_PHASES)
        .map(|p| campaign::herd_sprite(terrain, p).expect("stocked").0)
        .collect();
    let mut distinct = frames.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 6, "the six phases are six frames: {frames:?}");
    assert_eq!(
        campaign::herd_sprite(terrain, campaign::HERD_PHASES).expect("stocked").0,
        frames[0],
        "and the seventh wraps to the first",
    );

    // And they are six different pictures on the sheet, not six names for one.
    let mut screen = MapScreen::new();
    screen.centre_on_tile(fx as usize, fy as usize);
    if let Some(sheet) = assets.map.flag_sheet(screen.zoom()) {
        let mut shapes: Vec<Vec<u8>> = frames
            .iter()
            .map(|&f| sheet.frame(f).expect("the frame").indices.clone())
            .collect();
        let before = shapes.len();
        shapes.sort();
        shapes.dedup();
        assert_eq!(shapes.len(), before, "the six phases are one picture repeated");
    }

    // The witness: drawing at every phase leaves the world byte for byte the
    // same. `draw` takes `&Ctx`, so this cannot fail without the signature
    // changing first - which is the point.
    let before = l2_kingdom::save::checksum(&game.kingdom);
    for _ in 0..campaign::HERD_PHASES {
        let mut canvas = Canvas::screen();
        let ctx = Ctx { game: &mut game, assets: &assets };
        screen.draw(&ctx, &mut canvas);
    }
    assert_eq!(
        l2_kingdom::save::checksum(&game.kingdom),
        before,
        "drawing the map moved the simulation",
    );
}

/// **The Sovereign land lines are drawn in the owning realm's shield colour,
/// and the colour follows the shield rather than the realm id.**
///
/// A player, on a build with the previous code: *"The sovereign land text has
/// the wrong colours. When I start, the counties seem to have the right colours
/// — with Bishop being magenta, the Knight being yellow, the Countess being
/// blue, the Baron is black (at least, because I picked red) — but the text
/// doesn't match that."*
///
/// He is describing two things that should agree and did not. The minimap tint,
/// the menu-bar banner and the campaign flag all go through the realm's
/// **shield index**; these three lines went through `Ink::realm`, a table of our
/// own invention indexed by the **realm number**. `CountyStrip_Draw` passes
/// `g_realms[owner].field_0x8`, which `Realms_AssignLords` fills from
/// `g_realmColour[shieldIndex]`.
///
/// **Changing the shield is what makes this a test of the key** rather than of
/// the table. A fixed table keyed by the realm id passes any check that only
/// ever looks at one game; it fails the moment the same realm flies a different
/// colour, which is exactly what happens when a different human picks red.
/// `docs/decisions.md` C112.
#[test]
fn the_sovereign_lines_take_the_realms_shield_colour_and_follow_it() {
    let (mut game, assets) = world!();
    if assets.shell.body.is_none() {
        l2_testkit::skip!("no Fntl2_14.pl8, so there is nothing to read a pen off");
    }
    assert_eq!(game.kingdom.counties[1].owner, 5, "county 1 belongs to realm 5");
    let banner = assets.shell.text(15, 0).to_string();
    let banner = if banner.is_empty() { "SOVEREIGN LAND".to_string() } else { banner };

    // Every shield in turn, on the *same* county and the *same* realm. Only the
    // shield moves, so only the key can explain the colour.
    let mut seen = Vec::new();
    for shield in 1..=5u8 {
        game.kingdom.realms[5].shield_index = shield;
        let mut screen = CountyScreen::new(1, Panel::Tax);
        let canvas = draw(&mut screen, &mut game, &assets);
        let pen = l2_view::chrome::realm_pen(shield).expect("1..=5 has a pen");

        assert!(
            find_body(&canvas, &assets, &banner, pen).is_some(),
            "shield {shield}: the banner is not drawn in its pen {pen:#04X}"
        );
        // …and not in any of the other four. That is what rules out a table
        // that happens to agree on one entry.
        for other in 1..=5u8 {
            let wrong = l2_view::chrome::realm_pen(other).expect("1..=5");
            if wrong == pen {
                continue;
            }
            assert!(
                find_body(&canvas, &assets, &banner, wrong).is_none(),
                "shield {shield}: the banner is also drawn in shield {other}'s pen {wrong:#04X}"
            );
        }
        // The lord's name line takes the same pen as the banner — the original
        // computes `colour` once and passes it to all three calls, so a
        // per-line pen would be ours and not its.
        assert!(
            find_body(&canvas, &assets, "REALM 5", pen).is_some(),
            "shield {shield}: the lord's name is not in the same pen as the banner"
        );
        seen.push(pen);
    }
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), 5, "five shields must give five different pens");

    // And the emboss underneath is still the grey pair, unchanged by any of it.
    game.kingdom.realms[5].shield_index = 5;
    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let pen = l2_view::chrome::realm_pen(5).expect("shield 5");
    assert_eq!(
        emboss_at(&canvas, &assets, &banner, pen),
        Some(l2_game::shell::font::SHADOW_GREY),
        "the pen changed and the emboss did not"
    );
}


/// **The cattle row's forecast, and the sign is the claim.**
///
/// A player, mid-session: *"Sidebar doesn't show grain being planted as a
/// negative number."* `docs/draws-map.md` §5.10 has the diagnosis — the grain
/// row's value is county `+0x22C` and nothing in this workspace computes it —
/// and this is the **cattle** row, whose value does exist
/// ([`l2_kingdom::land::herd_preview`] is `Herd_LabourEstimate`'s tail) and
/// which therefore proves the drawing half before the expensive half lands on
/// it.
///
/// `Ui_DrawDelta` (`0x00402E0C`) is asserted in the three ways it can be wrong,
/// and each is a different line of it:
///
/// 1. **a negative forecast draws `-n` in `colourNeg`** — `0xF9`, the ninth
///    argument at all eight produce-row call sites;
/// 2. **a positive one draws `+n` in `colourPos`** — `0xFA`, and the `'+'` is
///    not decoration: the sign is the only thing on the row that says which way
///    the herd is going;
/// 3. **zero draws nothing at all**, because every produce row passes `mode`
///    0 and the function's first line is
///    `if ((value != 0) || (mode != 0))`.
///
/// The search is [`find_font_text`], so it is the **glyphs of the user's own
/// `Fntl2_9.pl8`** being matched at a colour, not a description of them — and
/// claim 3 is the one that cannot pass by accident, because it requires the
/// *absence* of a pattern the same run has just proved the renderer can draw.
///
/// Ablations, all three run: making the lead always `'+'` fails claim 1;
/// dropping the `value == 0` early return fails claim 3 (a `+0` appears);
/// swapping `DELTA_POS` and `DELTA_NEG` fails 1 and 2 together.
#[test]
fn the_cattle_row_draws_its_forecast_with_a_sign() {
    let (mut game, assets) = world!();
    // `colourPos` and `colourNeg`, typed from the call site in `FUN_004100AF`
    // rather than imported from the constants under test.
    const POS: u8 = 0xFA;
    const NEG: u8 = 0xF9;

    let county = 8;
    game.select(county as u8);
    // The row is only drawn when `FUN_0040FEC1` lists it, which is
    // `fieldsCattle != 0 || herd != 0`.
    game.kingdom.counties[county].fields_cattle = 4;
    game.kingdom.counties[county].herd = 400;

    let mut screen = MapScreen::new();
    let shown = |game: &mut Game, s: &str, colour: u8| -> Option<(i32, i32)> {
        let canvas = draw(&mut MapScreen::new(), game, &assets);
        let f = assets.shell.small.as_ref().expect("Fntl2_9.pl8");
        find_font_text(&canvas, f, s, colour)
    };
    let _ = &mut screen;

    // 1 — the herd is shrinking. This is the player's complaint, on the row
    // whose data path is complete.
    game.kingdom.counties[county].herd_change_expected = -7;
    let neg = shown(&mut game, "-7 ", NEG).expect("a shrinking herd shows -7");
    assert!(
        neg.0 >= 478 && neg.1 >= 302,
        "the delta belongs on the produce plate at (478, 302), not at {neg:?}",
    );
    // And it is not drawn in the positive colour, which is the half that would
    // survive a swapped pair.
    assert!(shown(&mut game, "-7 ", POS).is_none(), "a negative delta is 0xF9, not 0xFA");

    // 2 — growing, and the '+' is drawn.
    game.kingdom.counties[county].herd_change_expected = 7;
    let pos = shown(&mut game, "+7 ", POS).expect("a growing herd shows +7");
    assert_eq!(pos.1, neg.1, "both signs sit on the same row");
    assert!(shown(&mut game, "7 ", NEG).is_none(), "a positive delta is 0xFA, not 0xF9");

    // 3 — and a quiet season draws nothing. Neither sign, in either colour.
    game.kingdom.counties[county].herd_change_expected = 0;
    for s in ["+0 ", "-0 ", "0 "] {
        for c in [POS, NEG] {
            assert!(
                shown(&mut game, s, c).is_none(),
                "mode 0 with a zero value draws nothing at all, but {s:?} appeared in {c:#04x}",
            );
        }
    }
}

/// **The End Turn label goes away while the turn runs, and comes back.**
///
/// A player: *"in the original, the text 'END TURN' would disappear when you
/// click it, until the new turn was ready."* `Screen_DrawEndTurn`
/// (`0x0041A734`) blits the strip unconditionally and draws the label only when
/// `g_realms[g_localPlayer].aiStep < 999` — `Turn_End` (`0x0043AC23`) sets that
/// to 999 on the click and `Turn_BeginPlayersTurn` puts it back to 0 at the top
/// of the next turn. So it is a **conditional draw**, and the interval is
/// exactly *turn in flight*.
///
/// The assertion is idempotence rather than a pixel count, for the reason
/// `docs/agents.md` gives: draw the page, copy it, draw again, require equality.
/// Text is an opaque blit, so a second draw over itself changes nothing — but
/// only if it was there the first time. No threshold, and nothing to re-tune
/// when the artwork changes.
///
/// Three states, and the middle one is the claim:
///
/// 1. **idle** — the label is on the strip;
/// 2. **turn in flight** — it is not, and the strip is otherwise unchanged;
/// 3. **turn finished** — it is back.
///
/// Ablating the `if !turn::turn_in_flight(...)` guard fails claim 2.
#[test]
fn the_end_turn_label_disappears_while_the_turn_runs() {
    let (mut game, assets) = world!();
    game.select(8);
    let mut screen = MapScreen::new();

    // The band the label is centred in: `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE,
    // 0xA2, ...)`, so x 478..640 and the strip's own twenty rows from y 460.
    let label_band = |canvas: &Canvas| -> Vec<u8> {
        let mut out = Vec::new();
        for y in 460..480usize {
            for x in 478..640usize {
                out.push(canvas.at(x, y));
            }
        }
        out
    };

    // 1 — idle. The label is there, and drawing the whole screen twice over
    // itself changes nothing.
    let idle = draw(&mut screen, &mut game, &assets);
    let idle_again = draw(&mut screen, &mut game, &assets);
    assert_eq!(label_band(&idle), label_band(&idle_again), "an idle repaint is idempotent");

    // 2 — end the turn and catch it in flight. `run_turn` would carry it all the
    // way through, so this steps once and checks the state it left.
    send(&mut screen, &mut game, &assets, Event::Click { x: 500, y: 470 });
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert!(
        l2_game::turn::turn_in_flight(&game),
        "the click should have started a turn, or this test asserts nothing",
    );
    let running = draw(&mut screen, &mut game, &assets);
    assert_ne!(
        label_band(&idle),
        label_band(&running),
        "the End Turn strip is unchanged while the turn runs, so the label never went",
    );

    // 3 — and it comes back when the turn is ready. The strip's band must match
    // the idle one exactly: the label returns, in the same place, in the same
    // colour, on the same plate.
    run_turn(&mut screen, &mut game, &assets);
    assert!(!l2_game::turn::turn_in_flight(&game), "the turn should have finished");
    let done = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        label_band(&idle),
        label_band(&done),
        "the label did not come back the way it went",
    );
}
