#![allow(unused_imports)]
use super::*;
use super::corpus::*;
use super::oracle::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// Every graphic index a shipped battlefield asks for must exist in the
/// tileset. This is what would catch a wrong tileset, a wrong table, or a
/// wrong formula — all at once, over all twenty maps.
#[test]
fn every_shipped_battlefield_asks_only_for_tiles_that_exist() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(skr) = read(&dir, "USER.SKR") else {
        l2_testkit::skip!("USER.SKR not present - skipping");
    };
    let tiles = Sheet::new(read(&dir, scene::TILESET).expect(scene::TILESET)).unwrap();
    assert_eq!(tiles.frame_count(), 252, "T32_bat1.pl8 should hold 252 frames");

    let skr = l2_formats::Skr::parse(&skr).expect("USER.SKR parses");
    let mut cells = 0usize;
    for map in 0..skr.map_count() {
        let field = terrain::build(skr.terrain(map).unwrap(), 1);
        for c in &field.cells {
            assert!(
                tiles.frame(c.gfx as usize).is_some(),
                "map {map}: graphic {} is not a frame of the tileset",
                c.gfx
            );
            cells += 1;
        }
        // Both markers must have been found and expanded.
        assert_ne!(field.home_side0, (0, 0), "map {map} lost its 0x04 marker");
        assert_ne!(field.home_side4, (0, 0), "map {map} lost its 0x0F marker");
    }
    assert_eq!(cells, 20 * 6400);
    eprintln!("battlefields: {cells} cells across 20 maps, every tile present");
}

/// The first map of `USER.SKR` is the only one that is not the editor's blank
/// template: a river, two bridges and woodland. Drawing it must fill the
/// viewport completely — a hole would mean a missing or mis-indexed tile.
#[test]
fn the_sample_battlefield_renders_with_no_holes() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(skr) = read(&dir, "USER.SKR") else {
        l2_testkit::skip!("USER.SKR not present - skipping");
    };
    let tiles = Sheet::new(read(&dir, scene::TILESET).expect(scene::TILESET)).unwrap();
    let skr = l2_formats::Skr::parse(&skr).unwrap();
    let field = terrain::build(skr.terrain(0).unwrap(), 1);

    // Map 0 really does have water and woodland in it; if it did not, "no
    // holes" would be a much weaker claim.
    let water = field.cells.iter().filter(|c| c.terrain == terrain::id::WATER).count();
    let wood = field.cells.iter().filter(|c| c.terrain == terrain::id::WOODLAND).count();
    assert!(water > 100, "map 0 should be mostly river; found {water} water cells");
    assert!(wood > 0, "map 0 should have woodland");

    // Sweep the whole 80 x 80 map through the 15 x 14 viewport.
    let mut canvas = Canvas::screen();
    for cam_y in (0..terrain::DIM - scene::VIEW_ROWS).step_by(7) {
        for cam_x in (0..terrain::DIM - scene::VIEW_COLS).step_by(7) {
            canvas.clear(0);
            let cam = Camera { x: cam_x, y: cam_y };
            scene::draw_terrain(&mut canvas, &field, &tiles, None, cam);
            for row in 0..scene::VIEW_ROWS as i32 * scene::TILE {
                for col in 0..scene::VIEW_COLS as i32 * scene::TILE {
                    let y = (scene::ORIGIN_Y + row) as usize;
                    let x = (scene::ORIGIN_X + col) as usize;
                    assert_ne!(
                        canvas.at(x, y),
                        0,
                        "hole at screen ({x}, {y}) with the camera at ({cam_x}, {cam_y})"
                    );
                }
            }
        }
    }
    // Outside the viewport nothing was painted.
    assert_eq!(canvas.at(0, 0), 0, "the strip above the viewport should be untouched");
    assert_eq!(canvas.at(600, 300), 0, "the panel area should be untouched");
}

fn load_assets(dir: &Path) -> BattleAssets {
    BattleAssets::load(
        |name| read(dir, name).ok_or_else(|| format!("{name} not found")),
        Colour::Red,
        Colour::Blue,
    )
    .expect("battle assets load")
}

/// The point of the whole exercise: a battle that can be *watched*. Deploy the
/// armies `USER.SKR` map 0 ships, step the simulation, and check that
/// the picture changes as it does.
#[test]
fn a_battle_on_the_sample_map_animates_rather_than_sitting_still() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(skr_bytes) = read(&dir, "USER.SKR") else {
        l2_testkit::skip!("USER.SKR not present - skipping");
    };
    let assets = load_assets(&dir);
    let skr = l2_formats::Skr::parse(&skr_bytes).unwrap();
    let field = terrain::build(skr.terrain(0).unwrap(), 1);

    // Map 0's own army table, at the men-per-figure docs/battle.md derives for
    // it (600 v 450 men, size class 2).
    let att = skr.army(0, l2_formats::Side::Attacker).unwrap().counts;
    let def = skr.army(0, l2_formats::Side::Defender).unwrap().counts;
    assert_eq!(att[0], 200, "map 0's attacker should be the sample army");
    let army_a = battle::army_from_counts(&att, 16);
    let army_b = battle::army_from_counts(&def, 16);

    // docs/battle.md §5.4 derives this map independently, from the size ladder
    // and the per-unit rounding: size class 2, 16 men per figure, 41 figures
    // for the attacker and 33 for the defender. Landing on the same 41 and 33
    // by a different route is a real check on both.
    let raised = |army: &[(Troop, u16)]| -> usize { army.iter().map(|(_, n)| *n as usize).sum() };
    assert_eq!(raised(&army_a), 41, "attacker figures");
    assert_eq!(raised(&army_b), 33, "defender figures");

    let mut runner = BattleRunner::deploy(field, &army_a, &army_b);
    assert_eq!(runner.fighters.len(), 74, "USER.SKR map 0 should raise 74 figures");
    // Both armies are raised into units, which is what the AI dispatches on.
    assert!(runner.units.live().count() >= 8, "the armies did not become units");

    // Side 0 is the player's. A deployed army stands still until it is ordered
    // — `BattleUnit_Recentre` seeds an un-ordered unit's destination from its
    // own position — so send it at the enemy's marker, which is the click a
    // player would make. Side 4 is the AI's and is left to decide for itself.
    let enemy = runner.home(SIDE_B);
    runner.order_side(SIDE_A, enemy.0, enemy.1);

    let mut shots: Vec<Canvas> = Vec::new();
    for frame in 0..6 {
        if frame > 0 {
            runner.run(60);
        }
        let cam = scene::follow(&runner);
        let mut canvas = Canvas::screen();
        let drawn = scene::draw(&mut canvas, &runner, &assets, scene::Ground::Field, cam, None);
        assert!(drawn > 0, "frame {frame} drew no figures at all");
        shots.push(canvas);
    }

    // Consecutive frames must differ: a static screenshot would mean the
    // simulation is not reaching the picture.
    for w in shots.windows(2) {
        let d = w[0].diff_count(&w[1]);
        assert!(d > 200, "only {d} pixels changed between frames");
    }

    // And drawing the same state twice must be identical - the renderer is a
    // pure reader of simulation state.
    let cam = scene::follow(&runner);
    let mut a = Canvas::screen();
    let mut b = Canvas::screen();
    scene::draw(&mut a, &runner, &assets, scene::Ground::Field, cam, None);
    scene::draw(&mut b, &runner, &assets, scene::Ground::Field, cam, None);
    assert_eq!(a.diff_count(&b), 0, "the renderer is not deterministic");
    eprintln!(
        "battle: {} figures, {} ticks, {} alive",
        runner.fighters.len(),
        runner.tick,
        runner.fighters.iter().enumerate().filter(|(i, _)| runner.is_alive(*i)).count()
    );
}

/// Figures must be painted on top of the terrain — if the sprite blit
/// silently did nothing, every other assertion here would still pass.
#[test]
fn figures_are_visible_against_the_terrain_behind_them() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let assets = load_assets(&dir);
    let runner = BattleRunner::deploy(
        battle::blank_field(),
        &[(Troop::Knights, 6)],
        &[(Troop::Swordsmen, 6)],
    );
    let cam = scene::follow(&runner);

    let mut terrain_only = Canvas::screen();
    scene::draw_terrain(&mut terrain_only, &runner.field, assets.tiles(), None, cam);

    let mut with_men = terrain_only.clone();
    let drawn = scene::draw_figures(&mut with_men, &runner, &assets, cam);
    assert!(drawn > 0, "no figures drawn");

    let changed = terrain_only.diff_count(&with_men);
    assert!(changed > 500, "figures only changed {changed} pixels");
    eprintln!("figures: {drawn} drawn, {changed} pixels over the terrain");
}

// ---------------------------------------------------------- the campaign map

/// The right column's frames are 162 wide and their heights tile y 24..480
/// exactly. `chrome`'s constants say where each one goes; this reads how tall
/// each one is and checks the column closes.
#[test]
fn the_right_panel_frames_in_the_file_tile_the_column_exactly() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        l2_testkit::skip!("Misc_cty.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");
    let h = |i: usize| {
        let f = &pl8.frames[i];
        assert_eq!(f.width, 162, "frame {i} is {} wide, not 162", f.width);
        f.height as i32
    };
    use l2_view::chrome::misc_cty as f;
    let mut y = chrome::PANEL_TOP_Y;
    y += h(f::PANEL_TOP);
    assert_eq!(y, chrome::PANEL_MIDDLE_Y);
    // The foreign layout: one frame covers the whole middle.
    assert_eq!(y + h(f::PANEL_FOREIGN), chrome::PANEL_STATUS_Y);
    // The owned layout: three frames cover the same span.
    y += h(f::PANEL_OWN_A);
    assert_eq!(y, chrome::PANEL_OWN_B_Y);
    y += h(f::PANEL_OWN_B);
    assert_eq!(y, chrome::PANEL_OWN_C_Y);
    y += h(f::PANEL_OWN_C);
    assert_eq!(y, chrome::PANEL_STATUS_Y);
    y += h(f::PANEL_STATUS);
    assert_eq!(y, chrome::PANEL_END_TURN_Y);
    y += h(f::PANEL_END_TURN);
    assert_eq!(y, 480, "the column reaches the bottom of the screen");

    // The five realm banners are the 13 x 16 frames, and 16 is what fits the
    // 24-pixel menu bar at y 4.
    for colour in 1..=5usize {
        let b = &pl8.frames[f::BANNER + colour];
        assert_eq!((b.width, b.height), (13, 16), "banner for realm colour {colour}");
        assert!(4 + b.height as i32 <= campaign::TOP_BAR_H);
    }
    eprintln!("right panel: 7 frames tile y 24..480, 5 banners fit the bar");
}

/// `Panels.pl8`'s framed-box kit, checked against the file
/// `chrome`'s own constants: 52 border frames of 16 x 16, 144 texture frames of
/// 16 x 16, then eight of 24 x 24, then a second border set.
#[test]
fn the_panels_kit_in_the_file_has_the_shape_the_drawing_code_indexes() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Panels.pl8") else {
        l2_testkit::skip!("Panels.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Panels.pl8 parses");
    use l2_view::chrome::panels as p;

    // Border set A, the interior texture and border set B are all 16 x 16.
    for i in (0..p::STRIP).chain(p::SET_B..p::SET_B + 52) {
        assert_eq!((pl8.frames[i].width, pl8.frames[i].height), (16, 16), "frame {i}");
    }
    // The eight strip frames between them are 24 x 24, and nothing else is.
    for i in p::STRIP..p::STRIP + p::STRIP_LEN {
        assert_eq!((pl8.frames[i].width, pl8.frames[i].height), (24, 24), "frame {i}");
    }
    assert_ne!(pl8.frames[p::STRIP - 1].height, 24);
    assert_ne!(pl8.frames[p::SET_B].height, 24);

    // 25 strip cells from x 0 plus 2 from x 592 covers the screen exactly.
    assert_eq!(25 * p::STRIP_CELL, 600);
    assert_eq!(0x250 + 2 * p::STRIP_CELL, 640);
    eprintln!("panels: {} frames, kit boundaries match", pl8.frames.len());
}

/// **The blue outline, named to a frame and to three palette indices.**
///
/// A player remembered *"a blue outline for idle peasants"* and *"the peasant
/// slider I think had a blue outline if there were idle peasants as well."* The
/// binary says where it is — `CountyStrip_Draw` swaps five icon frames on
/// `labour[slot].useful < labour[slot].workers` and the slider's thumb on
/// `labour[8].workers != 0` — and this says *what it is*, out of the shipped
/// artwork:
///
/// * every ringed frame is **exactly four wider and four taller** than its
///   plain twin, which is what a two-pixel ring around an unchanged picture
/// measures as, and the drawing code moves it two pixels up and left;
/// * every non-transparent pixel of that two-pixel border is one of
///   **three palette entries, and all three are blue** — `95` = `rgb(0,0,121)`,
///   `65` = `rgb(157,202,234)`, `64` = `rgb(194,230,255)`.
///
/// The second clause is the one that makes "blue" a measurement. A ring of any
/// other colour would fail it, and so would a frame that happened to be
/// the right size.
#[test]
fn the_ringed_strip_icons_are_their_plain_twins_inside_a_blue_two_pixel_ring() {
    use l2_view::chrome::misc_cty;
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let bytes = read(&dir, "Misc_cty.pl8").expect("Misc_cty.pl8");
    let sheet = Sheet::new(bytes).expect("Misc_cty.pl8 parses");
    let palette = l2_formats::Palette::from_bytes(&read(&dir, "Base01.256").expect("Base01.256"))
        .expect("Base01.256 parses");

    // The ring's three entries really are blue, in the palette the campaign
// screen runs under. Asserted, because the whole claim
    // rests on the word.
    // Blue channel highest, red lowest, and a clear gap between them: the ring
    // runs from `rgb(0,0,121)` to a near-white highlight at `rgb(194,230,255)`,
// so "blue" here is the *ordering* of the channels
    // distance, which the pale end would fail.
    for i in misc_cty::RING_COLOURS {
        let [r, g, b] = palette.rgb(i);
        assert!(
            b >= g && g >= r && b as i32 - r as i32 >= 40,
            "palette entry {i} is rgb({r},{g},{b}), which is not blue"
        );
    }

    let pairs: Vec<(usize, usize, bool)> = misc_cty::RINGED_PAIRS
        .iter()
        .map(|&(p, r)| (p, r, true))
        .chain([
            (misc_cty::SPLIT_THUMB, misc_cty::SPLIT_THUMB_IDLE, true),
            // The castle is the one exception, and it is written down as one.
            (misc_cty::CASTLE_PLAIN, misc_cty::CASTLE_RINGED, false),
        ])
        .collect();
    assert_eq!(pairs.len(), 11, "nine icons, the slider's thumb and the castle");
    // The eleven ringed frames are a contiguous run, `0x4B` … `0x55`, with
    // nothing else in the sheet ringed and nothing in the run left over.
    let mut ringed: Vec<usize> = pairs.iter().map(|&(_, r, _)| r).collect();
    ringed.sort_unstable();
    assert_eq!(ringed, (0x4B..=0x55).collect::<Vec<_>>(), "the ringed run is 0x4B ..= 0x55");

    for (plain, ringed, same_picture) in pairs {
        let a = sheet.frame(plain).unwrap_or_else(|| panic!("frame {plain:#04x} decodes"));
        let b = sheet.frame(ringed).unwrap_or_else(|| panic!("frame {ringed:#04x} decodes"));
        if same_picture {
            assert_eq!(
                (b.width - a.width, b.height - a.height),
                (4, 4),
                "{ringed:#04x} is {}x{} and {plain:#04x} is {}x{}: a two-pixel ring is +4 on \
                 each axis",
                b.width,
                b.height,
                a.width,
                a.height
            );
        } else {
            assert_ne!(
                (b.width - a.width, b.height - a.height),
                (4, 4),
                "{ringed:#04x} was the documented exception and is no longer one"
            );
        }

        let (w, h) = (b.width as usize, b.height as usize);
        let (mut ring, mut blue) = (0usize, 0usize);
        for y in 0..h {
            for x in 0..w {
                if x >= 2 && y >= 2 && x + 2 < w && y + 2 < h {
                    continue;
                }
                let p = b.indices[y * w + x];
                if p == 0 {
                    continue;
                }
                ring += 1;
                if misc_cty::RING_COLOURS.contains(&p) {
                    blue += 1;
                }
            }
        }
        assert!(ring > 0, "{ringed:#04x} has no border pixels at all");
        assert_eq!(ring, blue, "{ringed:#04x}: {} of {ring} border pixels are not blue", ring - blue);
        eprintln!("Misc_cty {plain:#04x} -> {ringed:#04x}: {ring} border pixels, all blue");
    }
}

/// **The path-preview balls, and the `[I]` they settle.**
///
/// `Map_DrawPathMarker` (`0x004081A6`) draws `g_flagsSheet` frame `0x38 + n`
/// where `n` is the accumulated cost of reaching the tile, collapsing to
/// `0x38` for a step past the remaining budget, and `0x4E` on a castle,
/// settlement or plot. `docs/armies.md` §2.3 marked the mechanism **[V]** and
/// recorded one thing as **[I]**: *"that frame `0x38` is specifically the grey
/// one — nobody has looked at the sheet."*
///
/// This is looking at the sheet. Three things come out of it, and each is the
/// kind of claim that only a measurement can make:
///
/// 1. the run `0x38 … 0x4E` is **23 frames of exactly one shape** — same size,
///    same 177-pixel silhouette — so it is one picture recoloured, which is
///    what "indexed by cost" predicts and what a set of *different* pictures
///    would have refuted;
/// 2. **`0x38` is the only frame in the ramp with no colour in it at all**, so
/// the grey one is the out-of-range one and the inference is now a fact;
/// 3. `0x4E` is the opposite extreme — not one grey pixel — which is the
///    castle marker being a different picture.
///
/// A player reported these as *"colored dot images for the army walking dots"*,
/// and what selects the colour is the **cost**: not the realm, not the shield,
/// not the unit's kind. See `l2_view::campaign::path_marker_frame`.
#[test]
fn the_path_marker_ramp_is_one_ball_recoloured_and_only_the_first_is_grey() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Flags1a.pl8").expect("Flags1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");
    let pal_bytes = read(&dir, "Base01.256").expect("Base01.256");
    let pal = l2_formats::Palette::from_bytes(&pal_bytes).expect("palette");

    let first = sheet.frame(campaign::PATH_MARKER_FIRST).expect("frame 0x38");
    assert_eq!((first.width, first.height), (15, 15), "the ball is 15 x 15");

    // How many opaque pixels of a frame are a true grey, and how many carry
    // colour.
    let split = |i: usize| {
        let f = sheet.frame(i).unwrap_or_else(|| panic!("frame {i:#04x} is missing"));
        assert_eq!(
            (f.width, f.height),
            (first.width, first.height),
            "frame {i:#04x} is a different size from the rest of the ramp",
        );
        assert_eq!(
            f.opaque, first.opaque,
            "frame {i:#04x} has a different silhouette: the ramp is one picture recoloured",
        );
        let (mut grey, mut colour) = (0usize, 0usize);
        for (&p, &o) in f.indices.iter().zip(f.opaque.iter()) {
            if !o {
                continue;
            }
            let [r, g, b] = pal.rgb(p);
            if r == g && g == b {
                grey += 1;
            } else {
                colour += 1;
            }
        }
        (grey, colour)
    };

    let (grey, colour) = split(campaign::PATH_MARKER_FIRST);
    assert_eq!(colour, 0, "frame 0x38 is the grey one, and that was an inference until now");
    assert_eq!(grey, 177, "and all 177 of its opaque pixels are grey");

    for i in campaign::PATH_MARKER_FIRST + 1..=campaign::PATH_MARKER_LAST {
        let (_, colour) = split(i);
        assert!(colour > 0, "frame {i:#04x} carries colour and 0x38 does not");
    }

    // 0x4E, the castle/settlement marker, is the same size and silhouette and
    // is the one frame in the block with no grey in it at all.
    let (grey, colour) = split(campaign::PATH_MARKER_LAST + 1);
    assert_eq!(grey, 0, "0x4E is not a ball");
    assert_eq!(colour, 177);

    eprintln!(
        "path markers: {:#04x}..={:#04x} are 15x15, one silhouette, 0x38 alone is grey",
        campaign::PATH_MARKER_FIRST,
        campaign::PATH_MARKER_LAST + 1,
    );
}

/// **The click has to be able to reach what the renderer drew.**
///
/// This is the assertion the army-movement defect would have failed. The hit
/// test used to be a 9 x 9 square around the tile centre while
/// `campaign::draw_unit` blits a **40 x 32** `Sprite1a.pl8` frame anchored on
/// the diamond's bottom vertex — so a player clicking the figure he could see
/// mostly missed, and *"can't seem to move my army"*.
///
/// It never showed up in a test because every test runs on placeholder assets,
/// where no sheet exists, `draw_unit` returns false and the fallback marker is
/// drawn at the tile centre — the one configuration in which the old hit test
/// and the picture agreed. So this test needs the install, and it compares the
/// two directly: **the pixels the sprite paints, against the tile the
/// pick resolves them to.**
///
/// The rule it holds to is the original's, which has no way to disagree with
/// itself: `g_pickedTileUnit` is *"the unit index on the tile
/// `Map_ResolvePick` just resolved"*, so the pick is a tile pick and the figure
/// is drawn on that tile. What is asserted here is that a healthy majority of
/// the drawn figure resolves to the tile it is standing on — not all of it,
/// because a 40 x 32 sprite overhangs a 58 x 30 diamond and the
/// original overhangs it too.
#[test]
fn a_click_on_the_drawn_army_resolves_to_the_tile_it_stands_on() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Sprite1a.pl8").expect("Sprite1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");
    let frame = sheet.frame(0).expect("frame 0");
    assert_eq!(
        (frame.width, frame.height),
        (40, 32),
        "the near-zoom figure is 40 x 32, which is the number the 9 x 9 hit box was up against",
    );

    let zoom = &campaign::NEAR;
    // Where `draw_unit` puts the frame, for an army's nudge of (0, -4).
    // x = sx + half_pitch + nx - w/2 ; y = sy + half_pitch + ny - h
    let (nx, ny) = (0i32, -4i32);
    let origin_x = zoom.half_pitch + nx - frame.width as i32 / 2;
    let origin_y = zoom.half_pitch + ny - frame.height as i32;

    // And the diamond the pick resolves against, in the same tile-local space:
    // centre (tile_w/2 is the picture, half_pitch is the lattice — the lattice
    // is what `Map_PickTile` divides by).
    let (hw, hh) = (zoom.half_pitch, zoom.row_step);
    let (cx, cy) = (zoom.half_pitch, zoom.tile_h / 2);

    let (mut on_tile, mut off_tile) = (0usize, 0usize);
    for row in 0..frame.height as i32 {
        for col in 0..frame.width as i32 {
            let i = (row * frame.width as i32 + col) as usize;
            if !frame.opaque[i] {
                continue;
            }
            let (px, py) = (origin_x + col, origin_y + row);
            if (px - cx).abs() * hh + (py - cy).abs() * hw <= hw * hh {
                on_tile += 1;
            } else {
                off_tile += 1;
            }
        }
    }
    let total = on_tile + off_tile;
    assert!(total > 0, "the frame has pixels in it");
    // The old 9 x 9 box could hold at most 81 pixels of a figure this size.
    // The diamond holds a real share of it.
    assert!(
        on_tile * 2 > total,
        "most of the drawn figure has to pick its own tile: {on_tile} of {total}",
    );
    assert!(
        on_tile > 81,
        "and more of it than the 9 x 9 box that was there before could ever hold: {on_tile}",
    );
    eprintln!("army sprite {}x{}: {on_tile} of {total} opaque pixels pick their own tile", frame.width, frame.height);
}

/// **The cattle in the pastures, measured against `Flags1a.pl8` itself.**
///
/// `FUN_004071A0`'s farm arm picks
/// `0x55 + (terrain - 0x14) * 6 + phase` for a stocked pasture and
/// `0x67 + (terrain - 0x10) * 6 + phase` for the vestigial half nothing writes.
/// Reading that ladder out of the decompiler gives you two frame numbers; it
/// cannot tell you whether they point at cattle, and a canvas diff would pass
/// on the wrong frame of the right sheet. So this asks the sheet.
///
/// Four claims, and each is one a measurement can refute:
///
/// 1. every frame [`campaign::herd_sprite`] can return for a **reachable**
///    pasture is `58 × 30` — *exactly the near-zoom tile diamond*, so the
///    sprite is a full-meadow overlay and not a small figure. Eighteen frames,
///    three terrains by six phases, with no gap and no stray size;
/// 2. every frame it returns for the **vestigial** half is a `2 × 2` stub — the
/// art for that block, which is the second, independent
///    reason to believe nothing ever writes terrain `0x0F … 0x12` on a farm
///    tile;
/// 3. the three reachable groups carry **strictly more opaque pixels** as the
///    crowding rises. That is the claim *"a more crowded meadow has more
///    animals on it"* stated as something the file can contradict, and it is
///    what makes the band → frame mapping the right way round
/// merely consistent;
/// 4. `0x13`, the empty herd, and `0x0F` return **nothing at all** — bare grass
///    for a county that has lost every animal.
///
/// Ablating the `* 6` in [`campaign::herd_sprite`] fails claim 1 (the ladder
/// walks into the `2 × 2` stubs); ablating the `+ 0x55` fails it too; swapping
/// the two group bases fails claim 3.
#[test]
fn the_pasture_herd_frames_are_full_tiles_and_grow_with_the_crowding() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Flags1a.pl8").expect("Flags1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");

    // Claim 4, first, because the other three assume it.
    for empty in [0x0Fu8, 0x13] {
        for phase in 0..campaign::HERD_PHASES {
            assert!(
                campaign::herd_sprite(empty, phase).is_none(),
                "terrain {empty:#04x} is pasture with no animals on it",
            );
        }
    }

    // Claims 1 and 3: the three reachable bands.
    let tile = (l2_view::campaign::NEAR.tile_w as u16, l2_view::campaign::NEAR.tile_h as u16);
    let mut opaque_per_band = Vec::new();
    let mut frames_seen = Vec::new();
    for terrain in 0x14u8..=0x16 {
        let mut band = Vec::new();
        for phase in 0..campaign::HERD_PHASES {
            let (frame, at) = campaign::herd_sprite(terrain, phase).expect("a stocked pasture");
            assert_eq!(at, campaign::HERD_AT, "terrain {terrain:#04x} is the 0x55 block");
            let f = sheet
                .frame(frame)
                .unwrap_or_else(|| panic!("frame {frame:#04x} is missing from Flags1a.pl8"));
            assert_eq!(
                (f.width, f.height),
                tile,
                "frame {frame:#04x} is not a tile-sized meadow overlay",
            );
            band.push(f.opaque.iter().filter(|&&o| o).count());
            frames_seen.push(frame);
        }
        // Six phases of one scene: the animals move, the meadow does not, so
        // the frames are close in weight without being identical.
        let (lo, hi) = (*band.iter().min().unwrap(), *band.iter().max().unwrap());
        assert!(hi > 0, "terrain {terrain:#04x} draws nothing");
        assert!(
            hi - lo < hi / 4,
            "terrain {terrain:#04x}'s six phases are not one scene animated: {band:?}",
        );
        opaque_per_band.push(band.iter().sum::<usize>() / band.len());
    }
    frames_seen.sort_unstable();
    frames_seen.dedup();
    assert_eq!(frames_seen.len(), 18, "three bands by six phases, all distinct");
    assert_eq!(frames_seen[0], campaign::HERD_FIRST_FRAME);
    assert_eq!(frames_seen[17], campaign::HERD_FIRST_FRAME + 17);

    assert!(
        opaque_per_band[0] < opaque_per_band[1] && opaque_per_band[1] < opaque_per_band[2],
        "a more crowded meadow must carry more animals: {opaque_per_band:?}",
    );

    // Claim 2: the vestigial block is padding, not a second herd.
    for terrain in 0x10u8..=0x12 {
        for phase in 0..campaign::HERD_PHASES {
            let (frame, at) = campaign::herd_sprite(terrain, phase).expect("the dead ladder");
            assert_eq!(at, campaign::HERD_DEAD_AT);
            let f = sheet.frame(frame).unwrap_or_else(|| panic!("frame {frame:#04x} missing"));
            assert_eq!(
                (f.width, f.height),
                (2, 2),
                "frame {frame:#04x} is real art, so the block is not vestigial after all",
            );
        }
    }

    eprintln!(
        "Flags1a 0x55..0x66: 58x30 meadows, {} / {} / {} opaque pixels by band; 0x67..0x78 are 2x2 stubs",
        opaque_per_band[0], opaque_per_band[1], opaque_per_band[2],
    );
}

/// **`Flags1a.pl8`'s two blocks that `docs/draws-map.md` identified, asserted against the
/// player's own file — with every index a literal out of the decompilation.**
///
/// Neither block is drawn by this engine yet, and that is exactly why they are pinned now:
/// the expensive mistake on this screen has always been building the *right* mechanism onto
/// the *wrong* frame block (`docs/decisions.md` C49 lost four documents to it), and a block
/// boundary is something the file can refute before any code exists.
///
/// **Nothing here is computed from one of our constants.** Every number below is typed from
/// `Sprite_TopIt` (`0x004071A0`) and `Map_DrawArmies` (`0x00408438`) — which is the trap the
/// cattle offset fell into once, where the probe was derived from the constant it was
/// testing and ablation therefore proved nothing.
///
/// Three claims:
///
/// 1. **The herd ladder saturates; it does not overflow.** `Sprite_TopIt`'s farm arm is
///    `frame = (content - 0x14) * 6 + phase + 0x55` guarded by
///    `if (2 < (byte)(content - 0x14)) return`, and `phase` is `DAT_0057D388 >> 4` with the
///    counter wrapped at `0x60`, so `0 … 5`. The largest index reachable is therefore
///    `2*6 + 5 + 0x55 = 0x66`, and **`0x66` must be the last `58 × 30` frame of the run**
///    while `0x67` must not be one. That is the whole of the answer to *"is the three-band
///    graphic against the four-band meter a live out-of-bounds?"* — it is not.
///
/// 2. **`0x28 … 0x37` is one sixteen-frame `32 × 24` block**, the damage animation
///    `Sprite_TopIt` draws over a razed dwelling (`flags 0x10`, `content 0x13`) and over an
///    industry site shut down for three seasons or more. `0x27` before it is the last of the
///    forty flag frames and `0x38` after it is the first `15 × 15` path ball, so the block's
///    two boundaries are both checkable, and an index off by one breaks a size.
///
/// 3. **`0x79 … 0x80` is one eight-frame block** — the peasant mob's banner,
///    `Map_DrawArmies`' `frame = 0x79 + phase` for `unit.kind == 2`, with eight phases
///    because `DAT_0057D390` is `DAT_0057D378 >> 4` wrapped at `0x80`. `0x78` before it is
///    the last of the `2 × 2` stubs the herd test already names, and `0x81` after it is the
///    mercenary marker `Sprite_TopIt` draws on the town's north-east quadrant.
///
/// **The frames are `16 × 42`, and the first draft of this test said `32 × 24`** — the
///    size of the *realm* flags at the head of the sheet, assumed. The
///    file said so on the first run. Recorded because it is the whole argument for
///    asserting a block you have not built yet: a tall narrow standard on a pole is a
///    different picture from a wide waving banner, and nothing but the sheet was ever
///    going to say which.
#[test]
fn the_flags_sheet_damage_and_mob_banner_blocks_are_where_sprite_topit_says() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let bytes = read(&dir, "Flags1a.pl8").expect("Flags1a.pl8");
    let sheet = Sheet::new(bytes).expect("parse");
    let size = |i: usize| {
        let f = sheet.frame(i).unwrap_or_else(|| panic!("Flags1a.pl8 has no frame {i:#04x}"));
        (f.width, f.height)
    };

    // 1 — the herd ladder's top, from Sprite_TopIt's own arithmetic.
    let top = 2 * 6 + 5 + 0x55;
    assert_eq!(top, 0x66, "(content-0x14)*6 + phase + 0x55 tops out at 0x66");
    assert_eq!(size(top), (58, 30), "frame 0x66 is the last full-tile meadow");
    assert_ne!(size(top + 1), (58, 30), "frame 0x67 must be past the end of the meadows");

    // 2 — the damage block, and both of its boundaries.
    assert_eq!(size(0x27), (32, 24), "0x27 is the fortieth flag frame");
    for f in 0x28..=0x37 {
        assert_eq!(size(f), (32, 24), "frame {f:#04x} is not part of the damage block");
    }
    assert_eq!(size(0x38), (15, 15), "0x38 is the first path ball, not damage");

    // The animation burns down: warm pixels fall away and grey rises. Palette-free, so it
    // cannot be fooled by a different Base01.256 - "warm" is the run of orange/brown entries
// 0xC0..0xD8 the flame is drawn from, read off the sheet.
    let warm = |i: usize| {
        let f = sheet.frame(i).expect("frame");
        f.indices
            .iter()
            .zip(f.opaque.iter())
            .filter(|(&v, &o)| o && (0xC0..=0xD8).contains(&v))
            .count()
    };
    assert!(
        warm(0x2E) > warm(0x37),
        "the damage animation should end colder than its middle: {} then {}",
        warm(0x2E),
        warm(0x37),
    );

    // 3 — the mob banner, eight phases, after the 2x2 stubs.
    assert_eq!(size(0x78), (2, 2), "0x78 is the last of the vestigial stubs");
    for f in 0x79..=0x80 {
        assert_eq!(size(f), (16, 42), "frame {f:#04x} is not part of the mob banner");
    }
    assert_ne!(size(0x81), (16, 42), "0x81 is the mercenary marker, not a ninth phase");
}

/// **The wheat ripens, and the file says which way.**
///
/// A player: *"The wheat fields don't show the wheat growing."*
/// [`campaign::field_variant`] is the fix; this is the artwork that makes the
/// direction a measurement. `Grain_SeasonTick` writes the
/// crop's density band — 2, 3, 7 or 11 — onto every grain tile and derives
/// `Terrain_Set`'s variant from it, and **all four bands share base 88**, so
/// the variant is the only thing in the picture that moves.
///
/// Three claims, each refutable by the user's own `Roads1a.pl8`:
///
/// 1. frames **88 … 103** are sixteen tile-sized diamonds — four variants of
///    four variations — and **104** is where the next base begins, so the run
///    ends exactly where `field_base` says it does;
/// 2. the count of **ripe-gold** pixels rises strictly with the variant at every
///    one of the four `stored & 3` positions. That is *"a riper field has more
///    ripe wheat in it"* stated as something the file can contradict, and it is
///    what makes the band → variant mapping the right way round
/// merely consistent;
/// 3. the blocks on either side — fallow at 84 … 87 and pasture at 104 … 107 —
///    carry almost none of it, which bounds the run from outside.
///
/// "Gold" is read off the shipped palette: `r > 140`,
/// `g > 110`, `b < 110`, `r >= g` — and the *ramp* is what is asserted
/// than any count, because our palette widens 6-bit VGA by 255/63 where the
/// original multiplies by 4 and an absolute threshold would sit on that seam.
/// Ablating the `+ field_variant(terrain) * 4`
/// in [`campaign::field_frame`] does not fail this test — it is about the sheet,
/// not about us — so `a_fields_picture_follows_its_crop_state` asserts
/// the arithmetic and this asserts what the arithmetic is *for*.
#[test]
fn the_four_wheat_variants_ripen_and_the_block_ends_where_the_next_base_begins() {
    let Some(dir) = asset_dir() else {
        eprintln!("skipping: no install");
        return;
    };
    let sheet = Sheet::new(read(&dir, "Roads1a.pl8").expect("Roads1a.pl8")).expect("parse");
    let palette = l2_formats::Palette::from_bytes(&read(&dir, "Base01.256").expect("Base01.256"))
        .expect("the shipped palette");

    let gold = |frame: usize| -> usize {
        let f = sheet.frame(frame).unwrap_or_else(|| panic!("no frame {frame}"));
        // The diamond is 58 wide; the height is 30 **plus** whatever apex rows
        // the frame reserves, and some of this run reserves four. That is the
        // overhang byte `maps-layers.md` §1.1a measured, not a different size.
        assert_eq!(f.width, 58, "frame {frame} is not a tile diamond");
        assert!(f.height >= 30, "frame {frame} is shorter than a tile");
        f.indices
            .iter()
            .zip(f.opaque.iter())
            .filter(|(_, &o)| o)
            .filter(|(&v, _)| {
                let [r, g, b] = palette.rgb(v);
                r > 140 && g > 110 && b < 110 && r >= g
            })
            .count()
    };

    // 2 — four rising ramps, one per stored variation.
    for variation in 0..4usize {
        let ramp: Vec<usize> = (0..4).map(|v| gold(88 + v * 4 + variation)).collect();
        for w in ramp.windows(2) {
            assert!(
                w[1] > w[0],
                "variation {variation}: the gold does not rise across the variants: {ramp:?}",
            );
        }
        // 3 — and the neighbours are not wheat.
        assert!(
            gold(84 + variation) < ramp[0],
            "fallow frame {} is as gold as the youngest wheat",
            84 + variation,
        );
        assert!(
            gold(104 + variation) < ramp[0],
            "pasture frame {} is as gold as the youngest wheat",
            104 + variation,
        );
    }

    // 1 — the block is sixteen frames and it ends at 104.
    assert_eq!(l2_view::campaign::field_base(2).0, 88);
    assert_eq!(l2_view::campaign::field_base(0x13).0, 104);
    assert_eq!(88 + 4 * 4, 104, "four variants of four variations tile the gap exactly");
}

