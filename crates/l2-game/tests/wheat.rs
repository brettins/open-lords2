//! **The wheat, through one growing year, on the campaign screen.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-game --test wheat -- --nocapture
//! ```
//!
//! A player, on the build that carried C124's fix: *"wheat fields still not
//! showing the different stages of wheat growth."* **"Still"** — the fix had a
//! test at the pixel, and the test painted the terrain byte by hand. Nothing
//! ever asked what a season writes there, so the fix could band the wrong crop
//! word for three seasons in four and stay green. `docs/decisions.md`
//! C195.
//!
//! So this file does what the player did. It sows a field with the brush's own
//! handler, presses End Turn through the screen machine four times — Spring,
//! Summer, Autumn, Winter, a whole year from the England turn-one position —
//! and after each turn paints the campaign screen and reads the pixels at the
//! field.
//!
//! # What is pinned, and from where
//!
//! Nothing below asks our code which frame to expect.
//!
//! * **The frame** is a literal: `Terrain_Set` (`0x0046D7F4`) writes
//!   `((frame - oldBase) & 3) + 'X' + variant * 4` for terrain `2 … 0x12`, and
//!   `'X'` is `0x58`. [`WHEAT_BASE`].
//! * **The variant** is [`originals_variant`], a transcription of
//!   `Grain_SeasonTick` (`0x0044C8AE`) and `FUN_0044CF6F` with their literal
//!   thresholds, reading the county's crop words and `+0x206` — which the four
//!   End Turns wrote, not this file.
//! * **The picture** is what our painter draws when handed that literal frame:
//!   a terrain-only paint of the same viewport with one override. The screen
//!   under test must equal it at the tile, and must differ from the other three
//!   variants there — so the probe cannot pass on a patch where the four crops
//!   look alike.
//!
//! # Stated ablations, and what each did
//!
//! Each is one line, deleted or changed, with this test and the kingdom unit
//! tests beside it run against it.
//!
//! | ablated | this test | unit test |
//! |---|---|---|
//! | the band reads `crop[2]` in Spring, Summer, Autumn — C124's word | **red** | red |
//! | the repaint call in `Kingdom::grain_season_tick` | **red** | — |
//! | `+ field_variant(terrain) * 4` in `campaign::field_frame` | **red** | — |
//! | the fog arm darkens every tile, so the field is fogged | **red** (the four crops stop being four pictures) | — |
//! | sowing does not write `+0x206` | **red** — after the fix below | red |
//! | the band divides by `fieldsGrain`, not `+0x206` | *green* | red |
//! | Winter reads `crop[1]`, not the harvest | *green* | red |
//! | `FUN_00469D21`'s shortfall arm | *green* | red |
//!
//! **The three greens are findings about this year, not about the lines.** No
//! field is destroyed and none painted after sowing, so `fieldsGrain` equals
//! `+0x206` all year; nobody is short of reapers, so the harvest equals the
//! standing crop; and the seed covers a sack a field, so the shortfall never
//! fires. Each line is pinned by a unit test in `l2_kingdom::land` instead.
//!
//! **And one was green for a reason that was this file's fault.** The first
//! version read `+0x206` back from the county to compute its expectation, so
//! deleting the line that writes it moved the expectation with the picture —
//! `docs/agents.md`'s probe computed from the thing being ablated. It is pinned
//! from what this file sowed now, and that ablation is red.

use std::path::PathBuf;

use l2_formats::maps::Plane;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::map::MapScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::map::{coords, index, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::{campaign, Canvas};

/// `Terrain_Set`'s base for terrain `2 … 0x12`: the `'X'` in its ladder.
const WHEAT_BASE: u8 = 0x58;

/// The bank byte `Terrain_Set` leaves on a roads-layer field tile whose file
/// byte is `0x08`: `(((0x08 | 1) & 0xE3) | 8) & 0x7F`.
const ROADS_BANK_BYTE: u8 = 0x09;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no wheat to draw");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        // **Tip screens: No.** Four End Turns from a new campaign run far past
        // frame 21, and a tip holds the campaign map's input on screen `0x27`;
        // that is `tests/tips.rs`'s subject.
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

/// **The original's variant for one county's grain tiles**, transcribed with
/// its literals rather than asked of `l2_kingdom`.
///
/// ```c
/// /* Grain_SeasonTick, 0x0044C8AE */
/// if (g_season == 1 || g_season == 2 || g_season == 3)
///      band = FUN_0044CF6F(crop[1], (byte)county.field_0x206);
/// else band = FUN_0044CF6F(crop[2], (byte)county.field_0x206);   /* g_season == 4 */
/// variant = band < 3 ? 0 : (band - 3) / 4 + 1;
///
/// /* FUN_0044CF6F, 0x0044CF6F */
/// if (crop < 1) return 2;  if (fields < 1) return 2;
/// if (crop / fields < 0x29) return 3;  if (crop / fields < 0x51) return 7;  return 0xB;
///
/// /* FUN_00469D21, the shortfall arm: every grain tile after the first */
/// if (lo == 2 && county.field_0x1a7 != 0 && notFirst) Terrain_Set(tile, 2, 0);
/// ```
fn originals_variant(season: u8, crop: [i32; 3], standing: i32, shortfall: bool, first: bool) -> u8 {
    if shortfall && !first {
        return 0;
    }
    let word = if season == 4 { crop[2] } else { crop[1] };
    let fields = standing & 0xFF;
    let band: i32 = if word < 1 || fields < 1 {
        2
    } else if word / fields < 0x29 {
        3
    } else if word / fields < 0x51 {
        7
    } else {
        0xB
    };
    if band < 3 {
        0
    } else {
        ((band - 3) / 4 + 1) as u8
    }
}

/// What C124's fix drew: `crop[2]` over `fieldsGrain`, in every season —
/// **as it stood inside the tick**. `Grain_SeasonTick` clears `crop[2]` at its
/// top and only the harvest refills it before the repaint, so outside Winter
/// that word was zero when the band was taken. (After the turn it can hold
/// `Grain_LabourEstimate`'s harvest forecast, which the repaint never saw.)
fn c124_variant(season: u8, crop: [i32; 3], fields_grain: i32) -> u8 {
    let word = if season == 4 { crop[2] } else { 0 };
    let band = if word < 1 || fields_grain < 1 {
        2
    } else if word / fields_grain < 0x29 {
        3
    } else if word / fields_grain < 0x51 {
        7
    } else {
        0xB
    };
    if band < 3 {
        0
    } else {
        ((band - 3) / 4 + 1) as u8
    }
}

fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// **End Turn, as a player presses it**: the key on the campaign map, then
/// frames until the turn comes round, then the fade back up.
fn end_turn(machine: &mut Machine, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    {
        let mut ctx = Ctx { game: &mut *game, assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    for _ in 0..4_000 {
        if game.kingdom.turn_count > before {
            break;
        }
        let mut ctx = Ctx { game: &mut *game, assets };
        machine.update(&mut ctx);
    }
    assert!(game.kingdom.turn_count > before, "the turn never came round");
    for _ in 0..=l2_view::fade::PHASES {
        let mut ctx = Ctx { game: &mut *game, assets };
        machine.update(&mut ctx);
    }
    assert_eq!(machine.top_id(), Some(ScreenId::Campaign), "back on the campaign map");
}

/// A field tile nothing is drawn *over*: inside the map, no town or castle or
/// industry site (plane-0 `0x40`/`0x80`, whose flags and animations are
/// overlays) within one tile, no unit within three, and no county anchor —
/// where our own owner marker goes — within one.
///
/// A neighbouring pasture's herd is **not** excluded, because [`tile_box`]
/// reads only the inner part of the tile's own diamond and a herd sprite is a
/// neighbour's diamond shifted `(+4, −4)` (`Sprite_TopIt`'s farm arm), which
/// never reaches it.
fn a_quiet_tile(game: &Game, candidates: &[usize]) -> usize {
    let map = &game.kingdom.campaign.map;
    let near = |t: usize, r: i32, test: &dyn Fn(u8, u8) -> bool| {
        let (x, y) = coords(t);
        (-r..=r).any(|dy| {
            (-r..=r).any(|dx| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                (0..MAP_DIM as i32).contains(&nx)
                    && (0..MAP_DIM as i32).contains(&ny)
                    && test(nx as u8, ny as u8)
            })
        })
    };
    let settled = |x: u8, y: u8| map.flags[index(x, y)] & 0xC0 != 0;
    let unit = |x: u8, y: u8| game.kingdom.campaign.units.iter().any(|(_, u)| (u.x, u.y) == (x, y));
    let anchor = |x: u8, y: u8| {
        game.kingdom.county_ids().any(|id| (game.anchor_x[id], game.anchor_y[id]) == (x, y))
    };
    let tests: [(&str, &dyn Fn(usize) -> bool); 4] = [
        ("inside the map", &|t| {
            let (x, y) = coords(t);
            (4..60).contains(&x) && (4..60).contains(&y)
        }),
        ("no settlement within 1", &|t| !near(t, 1, &settled)),
        ("no unit within 3", &|t| !near(t, 3, &unit)),
        ("no anchor within 1", &|t| !near(t, 1, &anchor)),
    ];
    if let Some(&tile) = candidates.iter().find(|&&t| tests.iter().all(|(_, test)| test(t))) {
        return tile;
    }
    let refused: Vec<(&str, usize)> =
        tests.iter().map(|(name, test)| (*name, candidates.iter().filter(|&&t| !test(t)).count())).collect();
    panic!("none of {} sown fields is quiet; refused by: {refused:?}", candidates.len());
}

/// The terrain pass alone, with this tile forced to `frame` — the picture the
/// original's `Map_DrawTile` (`0x004063C1`) makes of `tile.frame` through the
/// roads bank of the current zoom.
fn terrain_with(game: &mut Game, assets: &Assets, screen: &MapScreen, x: usize, y: usize, frame: u8) -> Canvas {
    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let ctx = Ctx { game: &mut *game, assets };
    let mut overrides = MapScreen::tile_graphics(&ctx);
    overrides.set(x, y, ROADS_BANK_BYTE, frame);
    let hidden = |tx: usize, ty: usize| ctx.game.hides_tile(index(tx as u8, ty as u8));
    let fog: campaign::Fog = if ctx.game.kingdom.options.exploration { Some(&hidden) } else { None };
    let mut canvas = Canvas::screen();
    canvas.clear(assets.ink.background);
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
        ctx.game.kingdom.season,
        fog,
    );
    canvas
}

/// The pixels of the inner three-fifths of one tile's diamond, clipped to the
/// map viewport: `|dx| / (w/2) + |dy| / (h/2) <= 3/5` about the tile's centre.
///
/// Inner, because the diamond's rim is shared with what its neighbours draw
/// after it — a herd shifted `(+4, −4)`, an apex row — and the claim is about
/// this tile's frame, not theirs.
fn tile_box(canvas: &Canvas, screen: &MapScreen, x: usize, y: usize) -> Vec<u8> {
    let zoom = screen.zoom();
    let (w, h) = (zoom.tile_w, zoom.tile_h);
    let (cx, cy) = campaign::tile_centre(screen.viewport(), zoom, x, y).expect("the tile is in view");
    let clip = screen.map_clip();
    let mut out = Vec::new();
    for py in cy - h / 2..=cy + h / 2 {
        for px in cx - w / 2..=cx + w / 2 {
            let (dx, dy) = ((px - cx).abs(), (py - cy).abs());
            if 10 * (dx * h + dy * w) <= 3 * w * h && clip.contains(px, py) {
                out.push(canvas.at(px as usize, py as usize));
            }
        }
    }
    out
}

/// Paint the campaign screen on `tile` at one zoom and return which of the four
/// wheat variants the tile shows, or `None` for none of them. Also returns
/// whether the four are four different pictures at this zoom.
fn drawn_variant(game: &mut Game, assets: &Assets, tile: usize, far: bool) -> (Option<u8>, bool) {
    let (x, y) = coords(tile);
    let (x, y) = (x as usize, y as usize);
    let stored = assets.slot(game.map_slot).expect("the map slot").at(Plane::GfxIndex, x, y);

    let mut screen = MapScreen::new();
    if far {
        let mut ctx = Ctx { game: &mut *game, assets };
        screen.handle(Event::KeyDown(Key::letter('z')), &mut ctx);
        assert_eq!(screen.zoom().id, campaign::FAR.id, "Z zooms out");
    } else {
        screen.centre_on_tile(x, y);
    }
    let drawn = draw(&mut screen, game, assets);
    let (cx, cy) = campaign::tile_centre(screen.viewport(), screen.zoom(), x, y)
        .expect("the field is in view at this zoom");
    assert!(screen.map_clip().contains(cx, cy), "({x}, {y}) is inside the map viewport");
    assert!(!game.hides_tile(tile), "({x}, {y}) is seen: the fog is on and the field is lit");

    let candidates: Vec<Vec<u8>> = (0..4u8)
        .map(|v| {
            let frame = WHEAT_BASE + (stored & 3) + 4 * v;
            tile_box(&terrain_with(game, assets, &screen, x, y, frame), &screen, x, y)
        })
        .collect();
    let distinct = (0..4).all(|a| (a + 1..4).all(|b| candidates[a] != candidates[b]));
    let got = tile_box(&drawn, &screen, x, y);
    (candidates.iter().position(|c| *c == got).map(|v| v as u8), distinct)
}

/// **A sown field draws the original's wheat frame after every End Turn of a
/// year, at both zooms, on a lit tile.**
#[test]
fn a_sown_field_draws_the_originals_wheat_frame_in_every_season_of_a_year() {
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let county = game.selected;
    assert!(game.is_players(county), "the fixture opens on the player's county");

    // Sow: every fallow field to grain, through the brush's own handler
    // (`Field_SetType`, `0x00438BEC`).
    let fallow: Vec<usize> = game
        .kingdom
        .field_tiles(county as usize)
        .into_iter()
        .filter(|(_, kind)| *kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    assert!(!fallow.is_empty(), "the player's county has fallow to sow");
    // **Seed in the granary — the one number this file places.** The England
    // turn-one county's store is empty (printed below): sown as it stands, the
    // crop is zero all year and the original draws variant 0 in every season
    // too, so a picture that never changed would be *right* and prove nothing.
    // A player who sows buys the seed first — and **before** painting, because
    // the brush's own `Labour_Allocate` / `County_RefreshEstimates` round is
    // what puts farmers on the fields, and it sizes the grain ceiling from the
    // store. Seeded after the brush, the fields get nobody and sow nothing.
    // Everything the picture reads — the crop words, `+0x206`, the terrain
    // byte — is still written by the brush and the four End Turns below.
    let store = game.kingdom.counties[county as usize].grain;
    let imported = game.kingdom.counties[county as usize].fields_grain_standing;
    game.kingdom.counties[county as usize].grain = 10_000;
    for &tile in &fallow {
        game.kingdom.paint_field(county as usize, tile, FieldType::Grain).expect("a fallow field takes grain");
    }
    eprintln!(
        "sown: the fixture's store was {store} and its +0x206 {imported}; {} fallow fields to grain; {} on the grain fields",
        fallow.len(),
        game.kingdom.counties[county as usize].labour[l2_kingdom::tables::JOB_GRAIN_FARMING],
    );
    let probe = a_quiet_tile(&game, &fallow);
    let first_grain = (0..MAP_TILES)
        .find(|&t| {
            let map = &game.kingdom.campaign.map;
            map.county[t] == county && map.flags[t] & 0x20 != 0 && (2..=0x0E).contains(&map.terrain[t])
        })
        .expect("a grain tile");

    // Our own markers over the selected county's fields are not the original's
    // (`map.rs`, "Ours: the player's own county's fields, marked") and at the far
    // zoom one covers the whole tile. Look at the map with another county
    // selected, so the tile carries only what the original draws.
    let elsewhere = game.kingdom.county_ids().find(|&id| !game.is_players(id as u8)).expect("a county not the player's");

    let mut machine = Machine::new(ScreenId::Campaign);
    let mut year = Vec::new();
    for _ in 0..4 {
        end_turn(&mut machine, &mut game, &assets);
        let season = game.kingdom.season;
        let c = &game.kingdom.counties[county as usize];
        // **`+0x206` is pinned from what this file sowed, not read back.** The
        // sowing arm writes `fieldsGrain`, or `1` on a shortfall, and nothing in
        // this year destroys a field. Reading our own `fields_grain_standing`
        // here was the first version, and deleting the line that writes it left
        // this test green: the expectation fell to variant 0 with the picture.
        let sown = if c.sow_shortfall { 1 } else { fallow.len() as i32 };
        assert_eq!(c.fields_grain_standing, sown, "season {season}: +0x206 is the fields sown");
        let want = originals_variant(season, c.crop, sown, c.sow_shortfall, probe == first_grain);
        let was = c124_variant(season, c.crop, c.fields_grain);
        eprintln!(
            "season {season}: grain {} crop {:?} fieldsGrain {} +0x206 {} shortfall {} terrain {:#04X} -> original variant {want}, C124 drew {was}",
            c.grain, c.crop, c.fields_grain, c.fields_grain_standing, c.sow_shortfall,
            game.kingdom.campaign.map.terrain[probe],
        );

        let selected = game.selected;
        game.selected = elsewhere as u8;
        let (near, near_distinct) = drawn_variant(&mut game, &assets, probe, false);
        let (far, far_distinct) = drawn_variant(&mut game, &assets, probe, true);
        game.selected = selected;
        eprintln!("  near draws {near:?} (distinct {near_distinct}), far draws {far:?} (distinct {far_distinct})");

        assert!(near_distinct, "season {season}: the four wheat frames are four pictures at the near zoom");
        assert_eq!(near, Some(want), "season {season}: the near zoom draws the original's variant");
        if far_distinct {
            assert_eq!(far, Some(want), "season {season}: the far zoom draws the original's variant");
        } else {
            assert!(far.is_some(), "season {season}: the far zoom draws a wheat frame");
        }
        year.push((season, want, was));
    }
    assert_eq!(year.iter().map(|y| y.0).collect::<Vec<_>>(), [1, 2, 3, 4], "a whole growing year");
    assert!(
        year.iter().any(|&(_, want, was)| want != was),
        "and the year includes a season C124's reading draws wrongly: {year:?}"
    );
}
