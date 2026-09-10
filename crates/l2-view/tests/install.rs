//! Checks the renderer against a real game install, headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-view
//! ```
//!
//! Skips (rather than fails) when unset, so the suite still runs on a machine
//! without the game. **No window is opened and no process is started.** Every
//! assertion here is on a `Vec<u8>` of palette indices, which is also the shape
//! the eventual pixel diff against `Lords2.exe`'s framebuffer will take.
//!
//! Three kinds of check:
//!
//! * **Corpus** — the sprite frame layout is asserted over all 42 shipped `a2`
//!   sheets, not over one that happened to work (`docs/decisions.md` C1).
//! * **Oracle** — the sub-cell walk offsets are read out of `Lords2.exe`'s
//!   `.data` through the PE section headers and compared against the formula
//!   the renderer uses. Prior art and decompiler listings are leads; the bytes
//!   decide (C5, C8).
//! * **Render** — `USER.SKR`'s twenty battlefields are built and drawn, and a
//!   battle is stepped and drawn, with assertions on the resulting pixels.

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

fn asset_dir() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

/// The install is inconsistent about casing, so every lookup is
/// case-insensitive — the same rule the mod overlay applies.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|f| f.eq_ignore_ascii_case(name))
    })
}

fn read(dir: &Path, name: &str) -> Option<Vec<u8>> {
    fs::read(find(dir, name)?).ok()
}

/// The six troop types drawn from a per-facing stride. Knights are checked
/// separately because their frames come from a table, not a stride.
const STRIDE_TROOPS: [Troop; 6] = [
    Troop::Peasants,
    Troop::Crossbowmen,
    Troop::Macemen,
    Troop::Swordsmen,
    Troop::Pikemen,
    Troop::Archers,
];

/// The identity that makes the frame layout believable: a sheet holds exactly
/// eight facings of `poses_per_facing` poses, then eighteen shared frames — six
/// unclassified and twelve of dying.
///
/// One sheet agreeing would prove nothing. Thirty-six do, across six player
/// colours, and the same `poses_per_facing` also has to place the dying
/// handler's base at exactly `8 * poses + 6`. Getting the value wrong for any
/// troop breaks both identities at once.
#[test]
fn the_frame_layout_accounts_for_every_frame_of_every_shipped_sheet() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let mut checked = 0;
    for colour in Colour::ALL {
        for troop in STRIDE_TROOPS {
            let name = figures::sprite_file(colour, troop).unwrap();
            let Some(bytes) = read(&dir, &name) else {
                panic!("{name} missing from the install");
            };
            let sheet = Sheet::new(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            let poses = figures::poses_per_facing(troop) as usize;
            assert_eq!(
                sheet.frame_count(),
                8 * poses + 18,
                "{name}: {} frames, expected 8 x {poses} + 18",
                sheet.frame_count()
            );
            // The dying block is the last twelve of the eighteen extras, and
            // every dying frame must be inside the sheet.
            for facing in 0..8u8 {
                for phase in [0u8, 40, 80] {
                    let f = figures::frame(troop, Anim::Dying, facing, phase);
                    assert!(f >= 8 * poses + 6, "{name}: dying frame {f} below its base");
                    assert!(f < sheet.frame_count(), "{name}: dying frame {f} off the sheet");
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 36, "expected 6 colours x 6 troop types");
    eprintln!("frame layout: {checked} sheets agree");
}

/// **`Ui_OkButton` does not draw a tick. It draws a hole.**
///
/// A player described the corner of a county panel as *"a little mouse icon
/// around a black hole"*, against five documents and four source files calling
/// it a tick. Nobody had decoded the frame; decoding it settled it
/// (`docs/decisions.md` C46), and this is what stops the label drifting back.
///
/// The claim is made as a **rank** rather than as a word, because a threshold
/// picked to pass is not evidence: of the sheet's 84 frames, `Ui_OkButton`'s
/// two are the **4th and 5th darkest**, at 15.3% and 11.5% of their area in
/// near-black ink against a median frame's 1.2%. The widgets that really are
/// thin strokes sit where you would expect — the tax-up arrow at 0.2%, the
/// slider knob at 0.0%. A tick cannot come 4th out of 84.
#[test]
fn the_ok_button_frames_are_a_hole_rather_than_a_tick() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let bytes = read(&dir, "System.pl8").expect("System.pl8 is in the install");
    let sheet = Sheet::new(bytes).expect("System.pl8 decodes");
    let palette = l2_formats::Palette::from_bytes(&read(&dir, "Base01.256").expect("Base01.256"))
        .expect("the palette decodes");
    // Index 0 is this sheet's transparency, so ink is everything else, and the
    // hole is the ink that is nearly black under the campaign palette.
    let darkness = |index: usize| -> f64 {
        let Some(f) = sheet.frame(index) else { return 0.0 };
        let dark = f
            .indices
            .iter()
            .filter(|&&i| {
                let [r, g, b] = palette.rgb(i);
                i != 0 && (r as u32 + g as u32 + b as u32) < 90
            })
            .count();
        dark as f64 / f.indices.len().max(1) as f64
    };

    let dim = chrome::system::OK_DIM as usize;
    let mut all: Vec<(usize, f64)> =
        (0..sheet.frame_count()).map(|i| (i, darkness(i))).collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    let median = all[all.len() / 2].1;

    for (label, index) in [("mode 0", chrome::system::OK), ("mode 1", chrome::system::OK_ALT)] {
        let f = sheet.frame(index).unwrap_or_else(|| panic!("{label}: frame {index:#04X}"));
        assert_eq!(
            (f.width as usize, f.height as usize),
            (dim, dim),
            "{label} is not 24 x 24"
        );
        let rank = all.iter().position(|&(i, _)| i == index).expect("it is in the sheet") + 1;
        let pct = darkness(index);
        assert!(
            rank <= 8,
            "{label}: frame {index:#04X} is only the {rank}th darkest of {} — a hole should be \
             near the top",
            sheet.frame_count()
        );
        assert!(
            pct > median * 8.0,
            "{label}: {:.1}% near-black against a median frame's {:.1}%",
            pct * 100.0,
            median * 100.0
        );
        eprintln!(
            "Ui_OkButton {label}: frame {index:#04X} is {:.1}% near-black, rank {rank}/{} \
             (median {:.1}%)",
            pct * 100.0,
            sheet.frame_count(),
            median * 100.0
        );
    }

    // **And the skin trap, measured.** `System2.pl8` is the same size with the
    // same 84-frame table, and §4.2 already records that 69 of its frames are
    // entirely index 0. Frame 0x10 is one of them — so a panel that draws its
    // corner in **mode 1** under skin 0 draws nothing at all, while mode 0 is
    // painted in both. That is why `Chrome::load` prefers `System.pl8`.
    let alt = Sheet::new(read(&dir, "System2.pl8").expect("System2.pl8 is in the install"))
        .expect("System2.pl8 decodes");
    let blank = alt.frame(chrome::system::OK_ALT).expect("frame 0x10");
    assert!(
        blank.indices.iter().all(|&i| i == 0),
        "System2.pl8's mode 1 frame is expected to be entirely transparent"
    );
    let painted = alt.frame(chrome::system::OK).expect("frame 0x33");
    assert!(
        painted.indices.iter().any(|&i| i != 0),
        "but its mode 0 frame is painted, which is why only mode 1 vanishes"
    );
}

/// Knights come off an 8 x 8 `(body, target)` table whose live entries are
/// spaced three apart and top out at 53. With the walk cycle's maximum of 2
/// that reaches frame 55 — and `A2*_knig.pl8` holds exactly 56 real frames
/// plus four 2x2 stubs.
#[test]
fn the_knight_frame_table_fits_the_knight_sheets() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    for colour in Colour::ALL {
        let name = figures::sprite_file(colour, Troop::Knights).unwrap();
        let sheet = Sheet::new(read(&dir, &name).expect(&name)).unwrap();
        assert_eq!(sheet.frame_count(), 60, "{name}");
        for facing in 0..8u8 {
            for phase in 0..40u8 {
                let f = figures::frame(Troop::Knights, Anim::Walking, facing, phase);
                assert!((8..56).contains(&f), "{name}: facing {facing} -> frame {f}");
                assert!(sheet.frame(f).is_some(), "{name}: frame {f} does not decode");
            }
        }
    }
    // The horse under him: eight facings of six.
    let horse = Sheet::new(read(&dir, figures::HORSE_FILE).expect("A2_horse.pl8")).unwrap();
    assert_eq!(horse.frame_count(), 48, "A2_horse.pl8 should be 8 x 6");
    for facing in 0..8u8 {
        for phase in 0..40u8 {
            assert!(figures::horse_frame(facing, phase) < 48);
        }
    }
}

/// Map a virtual address to a file offset through the PE section headers.
/// `Lords2.exe` has no ASLR and a fixed image base of `0x400000`, so a virtual
/// address is a constant.
fn va_to_offset(exe: &[u8], va: u32) -> Option<usize> {
    let pe = u32::from_le_bytes(exe[0x3C..0x40].try_into().ok()?) as usize;
    let sections = u16::from_le_bytes(exe[pe + 6..pe + 8].try_into().ok()?) as usize;
    let opt_size = u16::from_le_bytes(exe[pe + 20..pe + 22].try_into().ok()?) as usize;
    for i in 0..sections {
        let s = pe + 24 + opt_size + i * 40;
        let rva = u32::from_le_bytes(exe[s + 12..s + 16].try_into().ok()?);
        let raw_size = u32::from_le_bytes(exe[s + 16..s + 20].try_into().ok()?);
        let raw_ptr = u32::from_le_bytes(exe[s + 20..s + 24].try_into().ok()?);
        let start = 0x0040_0000 + rva;
        if va >= start && va < start + raw_size {
            return Some((raw_ptr + (va - start)) as usize);
        }
    }
    None
}

/// `walk_offset` is a formula standing in for an 8 x 17 table of `(i32, i32)`
/// pairs at `0x004E4030`. This reads that table out of the binary and checks
/// the formula reproduces all 136 entries exactly.
///
/// Width matters here and is easy to get wrong: the entries are pairs of
/// **i32**, which the 0x88-byte per-facing stride confirms — 17 entries times
/// 8 bytes — and which the neighbouring 16-pixel table at `0x004E3BF0` confirms
/// again, sitting exactly `8 * 0x88` bytes earlier.
#[test]
fn the_walk_offsets_match_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    const TABLE_VA: u32 = 0x004E_4030;
    const STRIDE: u32 = 0x88;
    let Some(base) = va_to_offset(&exe, TABLE_VA) else {
        panic!("0x{TABLE_VA:08X} is not inside any initialised section");
    };

    let read_i32 = |off: usize| i32::from_le_bytes(exe[off..off + 4].try_into().unwrap());
    let mut compared = 0;
    for facing in 0..8u32 {
        for walking in 0..17u32 {
            let off = base + (facing * STRIDE + walking * 8) as usize;
            let want = (read_i32(off), read_i32(off + 4));
            let got = figures::walk_offset(facing as u8, walking as u8);
            assert_eq!(got, want, "facing {facing}, walking {walking}");
            compared += 1;
        }
    }
    assert_eq!(compared, 8 * 17);

    // The sign of facing 0's offset is what independently fixes the facing
    // numbering: north trails to the south.
    assert_eq!(figures::walk_offset(0, 1), (0, 30));
    eprintln!("walk offsets: {compared} entries match the binary");
}

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
            scene::draw_terrain(&mut canvas, &field, &tiles, cam);
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
/// armies `USER.SKR` map 0 actually ships, step the simulation, and check that
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
        let drawn = scene::draw(&mut canvas, &runner, &assets, cam);
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
    scene::draw(&mut a, &runner, &assets, cam);
    scene::draw(&mut b, &runner, &assets, cam);
    assert_eq!(a.diff_count(&b), 0, "the renderer is not deterministic");
    eprintln!(
        "battle: {} figures, {} ticks, {} alive",
        runner.fighters.len(),
        runner.tick,
        runner.fighters.iter().enumerate().filter(|(i, _)| runner.is_alive(*i)).count()
    );
}

/// Figures must actually be painted on top of the terrain — if the sprite blit
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
    scene::draw_terrain(&mut terrain_only, &runner.field, &assets.tiles, cam);

    let mut with_men = terrain_only.clone();
    let drawn = scene::draw_figures(&mut with_men, &runner, &assets, cam);
    assert!(drawn > 0, "no figures drawn");

    let changed = terrain_only.diff_count(&with_men);
    assert!(changed > 500, "figures only changed {changed} pixels");
    eprintln!("figures: {drawn} drawn, {changed} pixels over the terrain");
}

// ---------------------------------------------------------- the campaign map

/// The tile artwork's own dimensions, read out of the shipped PL8 frame tables,
/// must agree with the pitch `Map_SetZoom` steps by.
///
/// This is `docs/screens.md` §1.2, and it is the resolution of the 58-versus-60
/// discrepancy `maps-layers.md` §6 left open: **pitch = frame width + 2** and
/// **row step = frame height / 2**, at both zooms, over all ten banks. The
/// constants in `campaign` come from the instruction stream; the widths come
/// from the files; nothing here is written down twice.
#[test]
fn every_campaign_tile_bank_matches_the_pitch_the_binary_steps_by() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let mut checked = 0;
    let mut seen: Vec<&str> = Vec::new();
    for zoom in campaign::ZOOMS {
        for set in zoom.banks {
            for name in set {
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name);
                let Some(bytes) = read(&dir, name) else {
                    panic!("{name} is not in the install");
                };
                let pl8 = l2_formats::Pl8::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(!pl8.frames.is_empty(), "{name} has no frames");
                for (i, f) in pl8.frames.iter().enumerate() {
                    assert_eq!(
                        f.width as i32, zoom.tile_w,
                        "{name} frame {i} is {} wide, but zoom {} steps by {}",
                        f.width, zoom.id, zoom.pitch
                    );
                    assert_eq!(f.height as i32, zoom.tile_h, "{name} frame {i}");
                    checked += 1;
                }
                assert_eq!(zoom.pitch, zoom.tile_w + 2);
                assert_eq!(zoom.row_step, zoom.tile_h / 2);
            }
        }
    }
    // Twenty near-zoom files — four seasons of base, mtns, roads, town and
    // castle, none of them repeating — plus five far-zoom ones, which are one
    // season named four times. Twenty-five names from a 2 × 4 × 5 table of
    // forty, and the difference is the far zoom's season-invariance.
    assert_eq!(seen.len(), 25, "the distinct bank files across both zooms");
    assert_eq!(checked, 4 * (140 + 25 + 140 + 61 + 100) + (140 + 25 + 140 + 61 + 100));
    eprintln!("campaign tiles: {checked} frames in {} files", seen.len());
}

/// **The measurement the seasonal artwork rests on: do the four seasonal files
/// of a bank share a frame table?**
///
/// If they do not, `campaign::Overrides` does not survive a season — the game
/// rewrites a town's tiles to `Town1a.pl8` frames 47 … 50 and if frame 47 of
/// `Town1c.pl8` were a different picture, every town on the map would turn back
/// into a quarry every autumn. It is a cheap thing to check and it decides
/// whether the season is a lookup table or a real piece of work.
///
/// **It is a lookup table.** Across all five near-zoom banks and all 466 frames
/// of each season:
///
/// * the **frame count** is identical in all four files of every bank;
/// * the **canvas anchor** `(X, Y)` — where the artist put the cell on the
///   sheet — is identical for every frame of every bank, 1,864 of 1,864;
/// * the diamond's `width`, `height` and `shape` are identical for every frame
///   of every bank.
///
/// The **only** structural difference anywhere is the `rows` byte — how many
/// overhang scanlines stand above the diamond — on nine `Roads1?.pl8` frames,
/// 109 and 111 and 113 … 119, and it differs by one or two. Those are frames
/// 108 … 120, which [`campaign::FIELD_BASES`] identifies as the four
/// reclamation crops: a crop that grows needs a row more of picture above the
/// tile in the season it is taller. That is artwork varying, not an index
/// moving.
///
/// So frame *n* of a bank is the same cell of the same sheet in every season,
/// and an override recorded in spring is still correct in winter.
#[test]
fn the_four_seasons_of_a_bank_are_the_same_frame_table() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let zoom = campaign::NEAR;
    let mut frames_checked = 0;
    let mut anchors_checked = 0;
    let mut row_differences = Vec::new();
    for bank in 0..5 {
        let files: Vec<_> = (0..campaign::SEASONS)
            .map(|s| {
                let name = zoom.banks[s][bank];
                let bytes = read(&dir, name).unwrap_or_else(|| panic!("{name} is not installed"));
                (name, bytes)
            })
            .collect();
        let parsed: Vec<_> = files
            .iter()
            .map(|(n, b)| (*n, l2_formats::Pl8::parse(b).unwrap_or_else(|e| panic!("{n}: {e}"))))
            .collect();
        let (spring_name, spring) = &parsed[0];
        for (name, other) in &parsed[1..] {
            assert_eq!(
                other.frames.len(),
                spring.frames.len(),
                "{name} has {} frames and {spring_name} has {}",
                other.frames.len(),
                spring.frames.len()
            );
            for (i, (a, b)) in spring.frames.iter().zip(other.frames.iter()).enumerate() {
                // The anchor is the artist's own sheet coordinate. If frame `i`
                // moved on the sheet between seasons, the index would not mean
                // the same cell — this is the assertion that matters.
                assert_eq!(
                    (a.x, a.y),
                    (b.x, b.y),
                    "{name} frame {i} sits at a different place on the sheet than {spring_name}'s"
                );
                anchors_checked += 1;
                assert_eq!((a.width, a.height), (b.width, b.height), "{name} frame {i} size");
                if a.overhang_rows != b.overhang_rows {
                    row_differences.push((*name, i, a.overhang_rows, b.overhang_rows));
                }
                frames_checked += 1;
            }
        }
    }
    assert_eq!(anchors_checked, 3 * (140 + 25 + 140 + 61 + 100), "every frame of every bank");
    assert_eq!(frames_checked, anchors_checked);

    // The nine overhang differences, and nothing else. Naming them exactly is
    // what turns "we looked and it was fine" into a claim that fails if the
    // artwork ever stops matching this reading.
    let mut differing: Vec<usize> =
        row_differences.iter().map(|(_, i, _, _)| *i).collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    differing.sort_unstable();
    assert_eq!(
        differing,
        vec![109, 111, 113, 114, 115, 116, 117, 118, 119],
        "the only per-season structural differences should be nine Roads frames"
    );
    assert!(
        row_differences.iter().all(|(n, ..)| n.starts_with("Roads1")),
        "an overhang difference outside the roads bank: {row_differences:?}"
    );
    // Each of the nine is one of the four-frame crop blocks 108/112/116/120.
    for (_, i, ..) in &row_differences {
        let base = (i / 4) * 4;
        assert!(
            (108..=120).contains(&base),
            "frame {i} differs by season but is not in a crop block"
        );
    }
    assert!(
        row_differences.iter().all(|(_, _, a, b)| a.abs_diff(*b) <= 2),
        "an overhang differs by more than two rows: {row_differences:?}"
    );
    eprintln!(
        "seasons: {frames_checked} frames compared, {} overhang differences, 0 index moves",
        row_differences.len()
    );
}

/// **The far zoom is not seasonal, and the shipped files say otherwise.**
///
/// `g_resourceTable`'s zoom-2 half names `base2a`/`mtns2a`/… in all four of its
/// season blocks (see [`campaign::FAR`]), so `Base2b.pl8` and its eleven
/// siblings are dead weight in the install. This asserts the consequence of
/// getting that wrong: the dead `Town2b.pl8` is **not** interchangeable with
/// the live `Town2a.pl8`, so a renderer that derived the far zoom's filenames
/// from the season suffix would draw a different sheet for three seasons in
/// four.
#[test]
fn the_far_zoom_names_one_season_four_times_and_the_unused_files_do_not_match() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    for season in 1..=campaign::SEASONS {
        assert_eq!(
            campaign::FAR.banks[season - 1],
            campaign::FAR.banks[0],
            "season {season} of the far zoom should name spring's files"
        );
    }
    // …and the reason it matters.
    let a = read(&dir, "Town2a.pl8").expect("Town2a.pl8");
    let b = read(&dir, "Town2b.pl8").expect("Town2b.pl8");
    let (a, b) = (l2_formats::Pl8::parse(&a).unwrap(), l2_formats::Pl8::parse(&b).unwrap());
    assert_eq!(a.frames.len(), 61, "the live far-zoom town bank");
    assert_eq!(b.frames.len(), 94, "the dead one is a different sheet entirely");
    eprintln!("far zoom: Town2a has 61 frames, the unused Town2b has 94");
}

/// The minimap's realm colour ramp is `Lords2.exe`'s own data, transcribed into
/// `chrome::MINIMAP_REALM_RAMP`. This reads the 48 bytes back out of the user's
/// binary and fails if a single one differs.
///
/// The stride matters and is not guessable from the values: `Minimap_DrawOverlay`
/// indexes `realmColour * 8 + (shade - 10)`, so the records are **eight** bytes
/// of which four are used. The unused half of each record is the same four
/// bytes with the first replaced by 0x20 — which is exactly the substitution
/// the function makes by hand for the selected county, and is the corroboration
/// that the stride is right.
#[test]
fn the_minimap_realm_ramp_matches_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let Some(base) = va_to_offset(&exe, chrome::MINIMAP_REALM_RAMP_VA) else {
        panic!("0x{:08X} is not inside any initialised section", chrome::MINIMAP_REALM_RAMP_VA);
    };
    for (colour, want) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        let rec = &exe[base + colour * 8..base + colour * 8 + 8];
        assert_eq!(&rec[..4], want, "realm colour {colour}");
        // The second half of the record is the first with the selected-county
        // substitution already applied.
        assert_eq!(rec[4], chrome::MINIMAP_SELECTED, "realm colour {colour} selected entry");
        assert_eq!(&rec[5..], &want[1..], "realm colour {colour} tail");
    }
    eprintln!("minimap ramp: 6 realm colours match the binary");
}

/// The **rating** ramp the three statistic overlays index, read back out of the
/// user's own binary at `chrome::MINIMAP_RATING_RAMP_VA`.
///
/// It sits eight bytes below the realm ramp, and the two are separate tables:
/// `Minimap_DrawOverlay` indexes this one with a bare `[band]` and that one with
/// `[colour * 8 + step]`. The two bytes between them are indexed by nothing, and
/// this asserts they are there rather than quietly folding six entries into
/// eight.
#[test]
fn the_minimap_rating_ramp_matches_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    assert_eq!(
        chrome::MINIMAP_RATING_RAMP_VA + 8,
        chrome::MINIMAP_REALM_RAMP_VA,
        "the two ramps are adjacent, and that is why they get confused"
    );
    let Some(base) = va_to_offset(&exe, chrome::MINIMAP_RATING_RAMP_VA) else {
        panic!("0x{:08X} is not inside any initialised section", chrome::MINIMAP_RATING_RAMP_VA);
    };
    assert_eq!(
        &exe[base..base + chrome::MINIMAP_RATING_RAMP.len()],
        &chrome::MINIMAP_RATING_RAMP,
        "the six rating colours"
    );
    eprintln!("minimap ramp: 6 rating colours match the binary");
}

/// **The rating ramp's direction, from the artwork rather than from the code.**
///
/// `Misc_cty.pl8` frame 91 is the strip the original swaps in beside the minimap
/// while an overlay is up, and it carries a six-swatch colour bar with a tick
/// against one end and a cross against the other. Reading the bar's pixels top
/// to bottom gives `chrome::MINIMAP_RATING_RAMP` **reversed** — so index 0 is
/// the crossed end and index 5 the ticked one, which is what makes
/// "`happiness / 20`" a rating and not an arbitrary number.
///
/// This is a cross-check between two things nobody coordinated: a table of
/// palette indices in the code segment and a painted legend in an art file.
#[test]
fn the_rating_ramp_is_the_colour_bar_the_legend_strip_draws() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        l2_testkit::skip!("Misc_cty.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");
    let frame = pl8
        .decode(l2_view::chrome::misc_cty::MINIMAP_SIDE_ACTIVE)
        .expect("frame 0x5B decodes");
    assert_eq!((frame.width, frame.height), (29, 123), "the 29 x 123 mode strip");

    // Column 5 runs down the middle of the swatch bar. Collect the runs.
    let w = frame.width as usize;
    let mut runs: Vec<(u8, usize)> = Vec::new();
    for y in 0..frame.height as usize {
        let v = frame.indices[y * w + 5];
        match runs.last_mut() {
            Some((c, n)) if *c == v => *n += 1,
            _ => runs.push((v, 1)),
        }
    }
    // The swatches are the only runs more than ten rows tall.
    let bar: Vec<u8> = runs.iter().filter(|(_, n)| *n >= 10).map(|(c, _)| *c).collect();
    let mut want = chrome::MINIMAP_RATING_RAMP;
    want.reverse();
    assert_eq!(bar, want, "the legend bar is the rating ramp, best first");
    eprintln!("minimap legend: frame 0x5B's colour bar is the rating ramp reversed");
}

/// `Minimap_Load`'s file and frame arithmetic, checked against the install:
/// every used map slot must resolve to a `MAPnn.PL8` that exists and holds two
/// 128 x 128 frames where the formula says, and every *empty* slot must resolve
/// to one of the four files the game does not ship.
///
/// The second half is what makes this evidence rather than a smoke test: 11
/// files x 4 slots = 44 is exactly the used-slot census in
/// `docs/formats/maps-layers.md` §6, arrived at from a completely different
/// direction.
#[test]
fn every_used_map_slot_has_a_minimap_and_every_empty_one_does_not() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(maps) = read(&dir, "L2_maps.dat") else {
        l2_testkit::skip!("L2_maps.dat not present - skipping");
    };
    let set = l2_formats::MapSet::parse(&maps).expect("L2_maps.dat parses");
    let used = set.used_slots();

    let mut with_minimap = 0;
    for slot in 0..60usize {
        let name = chrome::Minimap::file_for_slot(slot);
        match read(&dir, &name) {
            Some(bytes) => {
                let m = chrome::Minimap::load(&bytes, slot)
                    .unwrap_or_else(|e| panic!("slot {slot} in {name}: {e}"));
                assert_eq!(m.counties.len(), 128 * 128);
                assert!(used.contains(&slot), "slot {slot} has a minimap but no map");
                with_minimap += 1;
            }
            None => assert!(!used.contains(&slot), "slot {slot} is used but {name} is missing"),
        }
    }
    assert_eq!(with_minimap, 44, "11 shipped MAPnn.PL8 files times 4 slots");
    assert_eq!(with_minimap, used.iter().filter(|&&s| s < 60).count());
    eprintln!("minimaps: {with_minimap} slots, matching the used-slot census");
}

/// The right column's frames are 162 wide and their heights tile y 24..480
/// exactly. `chrome`'s constants say where each one goes; this reads how tall
/// each one actually is and checks the column closes.
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

/// `Panels.pl8`'s framed-box kit, checked against the file rather than against
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

/// **`Misc_cty.pl8` frames 0 … 0x16 are the village's peasant icons**, which
/// `docs/screens-county.md` §9 guessed, marked `[I]`, were "almost certainly
/// the top menu bar".
///
/// The icon table at `0x004D6808` names nine (normal, highlighted) pairs among
/// them, plus a shortfall pair and a surplus pair. This checks the shipped file
/// against that: **every frame the table names is 16 × 32 and decodes, and the
/// four it never names — 5, 6, 11 and 12 — are exactly the four frames in that
/// range that are 2 × 2 stubs.** Nineteen icons, nineteen frames, nothing over.
#[test]
fn the_peasant_icons_account_for_every_frame_the_icon_table_names() {
    use l2_view::village as v;
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        eprintln!("no Misc_cty.pl8 - skipping");
        return;
    };
    let sheet = Sheet::new(bytes.clone()).expect("Misc_cty.pl8 parses");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");

    // Every frame the drawing code can ask for: value - 1, and value while the
    // icon is selected.
    let mut named = std::collections::BTreeSet::new();
    for value in v::ICON_VALUE.iter().copied().chain([v::ICON_SHORTFALL, v::ICON_SURPLUS]) {
        named.insert(value as usize - 1);
        named.insert(value as usize);
    }
    assert_eq!(*named.iter().max().unwrap(), 22, "the icons stop at frame 0x16");

    let mut canvas = Canvas::screen();
    for &f in &named {
        assert_eq!(
            (pl8.frames[f].width, pl8.frames[f].height),
            (16, 32),
            "frame {f} is named by the icon table and is not an icon"
        );
        let decoded = sheet.frame(f).expect("frame {f} decodes");
        canvas.blit(&decoded, 100, 100);
    }

    let unnamed: Vec<usize> = (0..=22).filter(|f| !named.contains(f)).collect();
    assert_eq!(unnamed, vec![5, 6, 11, 12], "four frames in the range go unused");
    for f in unnamed {
        assert_eq!(
            (pl8.frames[f].width, pl8.frames[f].height),
            (2, 2),
            "frame {f} is unused and should be a stub"
        );
    }
    eprintln!("Misc_cty: {} named icons, 4 stubs, nothing over", named.len());
}

/// **The blue outline, named to a frame and to three palette indices.**
///
/// A player remembered *"a blue outline for idle peasants"* and *"the peasant
/// slider I think had a blue outline if there were idle peasants as well."* The
/// binary says where it is — `CountyStrip_Draw` swaps five icon frames on
/// `labour[slot].useful < labour[slot].workers` and the slider's thumb on
/// `labour[8].workers != 0` — and this says *what it is*, out of the shipped
/// artwork rather than out of a description:
///
/// * every ringed frame is **exactly four wider and four taller** than its
///   plain twin, which is what a two-pixel ring around an unchanged picture
///   measures as, and the drawing code moves it two pixels up and left;
/// * every non-transparent pixel of that two-pixel border is one of
///   **three palette entries, and all three are blue** — `95` = `rgb(0,0,121)`,
///   `65` = `rgb(157,202,234)`, `64` = `rgb(194,230,255)`.
///
/// The second clause is the one that makes "blue" a measurement. A ring of any
/// other colour would fail it, and so would a frame that merely happened to be
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
    // screen runs under. Asserted rather than assumed, because the whole claim
    // rests on the word.
    // Blue channel highest, red lowest, and a clear gap between them: the ring
    // runs from `rgb(0,0,121)` to a near-white highlight at `rgb(194,230,255)`,
    // so "blue" here is the *ordering* of the channels rather than a hue
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

/// **The ten words `Village_BalanceAll` reads out of an eight-word table.**
///
/// `FUN_00439EDB` loops `i < 10` over `g_jobClusterToSlot`, which has eight
/// entries. The two past the end are the head of the table that follows, and
/// they decide what a double click on the idle townsfolk actually balances — so
/// they are read out of the user's own executable rather than believed.
#[test]
fn the_cluster_to_slot_table_and_the_two_words_the_balance_loop_overruns_into() {
    use l2_view::village as v;
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe - skipping");
    };
    let table = l2_testkit::pe::Table::at(&exe, v::CLUSTER_TO_SLOT_VA);
    let read: Vec<usize> = table.i32s(10).into_iter().map(|v| v as usize).collect();
    assert_eq!(read[..8], v::CLUSTER_TO_SLOT, "the eight the table really has");
    assert_eq!(read, v::CLUSTER_TO_SLOT_BALANCE, "and the ten the balance loop reads");
    // The point of the two extra words: they bring slot 4, iron mining, into a
    // gesture that the eight-entry table can otherwise only reach through
    // cluster 0's override.
    assert!(read[8..].contains(&4), "the overrun reaches iron mining");
}

/// The village's own three files, and the arithmetic that ties them together.
#[test]
fn the_village_files_are_the_size_the_drawing_code_indexes_them_at() {
    use l2_view::village as v;
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };

    // The scene: one frame, 363 x 320, drawn at (0x40, g_villageTopY).
    let scene_bytes = read(&dir, "vill.pl8").expect("vill.pl8");
    let scene = l2_formats::Pl8::parse(&scene_bytes).unwrap();
    assert_eq!(scene.frames.len(), 1);
    assert_eq!((scene.frames[0].width as i32, scene.frames[0].height as i32), (363, v::SCENE_H));

    // The tops: six frames, one per weather, and `L2.eng` group 66 has six
    // names. Village_Draw indexes both with the same county byte.
    let tops_bytes = read(&dir, "villtops.pl8").expect("villtops.pl8");
    let tops = l2_formats::Pl8::parse(&tops_bytes).unwrap();
    assert_eq!(tops.frames.len(), 6, "six weathers");
    for f in &tops.frames {
        assert_eq!((f.width as i32, f.height), (363, 70));
    }

    // The drop grid: 24 bytes of header and 45 x 40 cells, and nothing else.
    let grid = read(&dir, "vill_gd8.pl8").expect("vill_gd8.pl8");
    assert_eq!(grid.len(), 0x18 + v::GRID_LEN, "1,824 bytes: 24 + 45 * 40");
    assert_eq!(v::GRID_COLS as i32 * v::GRID_CELL, 360, "x 0x40 .. 0x1A8");
    assert_eq!(v::GRID_ROWS as i32 * v::GRID_CELL, v::SCENE_H, "y top .. top + 0x140");
    // Every cell names a cluster or nothing; nothing names a ninth cluster.
    assert!(
        grid[0x18..].iter().all(|&b| b as usize <= v::CLUSTER_COUNT),
        "a cell names a cluster the village does not draw"
    );
    eprintln!("village: 363x320 scene, 6 weather tops, 45x40 grid");
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
///    the grey one is the out-of-range one and the inference is now a fact;
/// 3. `0x4E` is the opposite extreme — not one grey pixel — which is the
///    castle marker being a different picture rather than another ball.
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
/// two directly: **the pixels the sprite actually paints, against the tile the
/// pick resolves them to.**
///
/// The rule it holds to is the original's, which has no way to disagree with
/// itself: `g_pickedTileUnit` is *"the unit index on the tile
/// `Map_ResolvePick` just resolved"*, so the pick is a tile pick and the figure
/// is drawn on that tile. What is asserted here is that a healthy majority of
/// the drawn figure resolves to the tile it is standing on — not all of it,
/// because a 40 x 32 sprite genuinely overhangs a 58 x 30 diamond and the
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
///    art for that block was never drawn, which is the second, independent
///    reason to believe nothing ever writes terrain `0x0F … 0x12` on a farm
///    tile;
/// 3. the three reachable groups carry **strictly more opaque pixels** as the
///    crowding rises. That is the claim *"a more crowded meadow has more
///    animals on it"* stated as something the file can contradict, and it is
///    what makes the band → frame mapping the right way round rather than
///    merely consistent;
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
///    **The frames are `16 × 42`, and the first draft of this test said `32 × 24`** — the
///    size of the *realm* flags at the head of the sheet, assumed rather than read. The
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
    // 0xC0..0xD8 the flame is drawn from, read off the sheet rather than from a name.
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

/// **The realm pen table is `Lords2.exe`'s own ten bytes**, read back out of
/// the user's copy at `0x004DC1D0`.
///
/// `l2_view::chrome::REALM_PEN` is a transcription, and a transcription that
/// nothing checks is a table somebody eventually edits by eye. The stride is
/// the part that is not guessable from the values: the binary indexes from
/// **two bytes below** the data — `(&g_realmColour)[shieldIndex * 2]` with
/// `g_realmColour` at `0x004DC1CE` — so that the 1-based shield lands on the
/// first pair, and reading it the obvious way is off by one entry.
#[test]
fn the_realm_pen_table_matches_the_bytes_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let Some(base) = va_to_offset(&exe, chrome::REALM_PEN_VA) else {
        panic!("{:#010X} is not in any section", chrome::REALM_PEN_VA);
    };
    let want: Vec<u8> = chrome::REALM_PEN.iter().flatten().copied().collect();
    assert_eq!(&exe[base..base + want.len()], &want[..], "g_realmColour differs from ours");

    // The two bytes the binary's own base points at are *not* part of this
    // table — they are the tail of `g_lordChoice`. Asserting that pins the
    // 1-based indexing: if the table really started at `0x004DC1CE` these would
    // be shield 1's pen and they would have to be a colour pair.
    assert_eq!(
        &exe[base - 2..base],
        &[0x04, 0x02][..],
        "the two bytes below the table are g_lordChoice's tail, not a sixth pen"
    );

    // And the ten bytes are followed by zeros: five shields and no more.
    assert!(
        exe[base + want.len()..base + want.len() + 8].iter().all(|&b| b == 0),
        "something follows the fifth pair"
    );
    eprintln!("realm pens: {} bytes match Lords2.exe at {:#010X}", want.len(), chrome::REALM_PEN_VA);
}

/// **The pen really is keyed by the shield**, checked against every saved game
/// this project keeps — including the ones that separate the two candidate
/// keys.
///
/// `l2_view::chrome::realm_pen` *derives* the pen from the shield rather than
/// reading realm `+0x08` out of the save, because both writers in the binary
/// derive it the same way and nothing else touches the field
/// (`Realms_AssignLords` at new game, `FUN_0042BA40` for a custom battle). This
/// is that claim tested against data: for every realm of every fixture, the
/// byte the game stored at `+0x08` must equal our table indexed by `+0x0A`.
///
/// **The fixtures are what make this decisive rather than circular.** In
/// `england-turn1.sav` realm *n* happens to fly shield *n*, so it cannot tell
/// "keyed by the shield" from "keyed by the realm id" — and the realm id is
/// exactly the wrong key our county strip was using. Six of the other fixtures
/// have **realm 1 flying shield 5**, and there realm 1's stored pen is `0x04`,
/// blue, which is shield 5's. That is the observation the fix rests on.
#[test]
fn every_saved_realms_stored_pen_is_its_shields_pen() {
    let Some(dir) = l2_testkit::fixtures_dir() else {
        l2_testkit::skip!("LORDS2_FIXTURES not set - skipping");
    };
    let Some(install) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - the save schema comes from the executable");
    };
    let Some(exe) = read(&install, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };

    let mut realms_checked = 0;
    let mut files_checked = 0;
    // Did any fixture actually exercise a realm whose id differs from its
    // shield? Without one this test would pass on the wrong key too.
    let mut separating = 0;
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the fixture directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".sav"))
        .collect();
    names.sort();
    for name in &names {
        let Ok(bytes) = std::fs::read(dir.join(name)) else { continue };
        // Loud, not silent: a save this cannot open is a schema problem, and
        // skipping it quietly is how a test ends up asserting over nothing.
        let save = l2_formats::save::Save::open(&exe, &bytes)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        files_checked += 1;
        for index in 1..l2_formats::save::REALM_RECORDS {
            let Ok(realm) = save.realm(index) else { continue };
            if realm.shield_index == 0 {
                continue;
            }
            let stored = save
                .u8_at(
                    l2_formats::save::REALM_BASE
                        + (index * l2_formats::save::REALM_STRIDE) as u32
                        + 0x08,
                )
                .expect("realm +0x08 is inside the realm block");
            assert_eq!(
                Some(stored),
                chrome::realm_pen(realm.shield_index),
                "{name}: realm {index} flies shield {} and stored pen {stored:#04X}",
                realm.shield_index
            );
            if index as u8 != realm.shield_index {
                separating += 1;
            }
            realms_checked += 1;
        }
    }
    assert!(files_checked >= 1, "no fixture saves were readable");
    assert!(
        separating >= 2,
        "no fixture has a realm whose id differs from its shield, so this cannot tell the \
         two keys apart - it passed for the wrong reason"
    );
    eprintln!(
        "realm pens: {realms_checked} realms across {files_checked} saves, {separating} of them \
         with id != shield"
    );
}

/// **The two tables that decide which lord flies which colour**, read out of
/// the user's own executable — and the arithmetic that says which of them the
/// campaign uses.
///
/// A player described the rule as *"the game will always try to give the Knight
/// yellow, the Countess blue, the Bishop purple/pink … and it'll move a noble's
/// colour around if you pick it."* Every colour is right and the mechanism is
/// two mechanisms:
///
/// * **`g_lordChoice`** (`0x004DC17C`) is per **colour**, four candidate lords
///   per shield. `Realms_AssignLords` hands out the lowest free shield by realm
///   order and then picks the lord *from the shield*, so no lord has a
///   preference on the campaign path — the appearance of one is this table's
///   first column.
/// * **`g_battleLordShield`** (`0x004D4CA8`) is per **lord**, `{preferred,
///   alternate}`, and really is preference-then-fallback. It belongs to the
///   custom battle (`FUN_0042BA40`) and nothing else.
///
/// `docs/rules.md` §7a has both halves and the table of what actually happens
/// when a person takes each of the five colours.
#[test]
fn the_lord_colour_tables_are_the_bytes_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let u32_at = |va: u32| -> u32 {
        let o = va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"));
        u32::from_le_bytes(exe[o..o + 4].try_into().unwrap())
    };
    let u8_at = |va: u32| -> u8 {
        exe[va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"))]
    };

    // --- `g_battleLordShield`: the per-lord preference, and the only one.
    // Knight, Baron, Countess, Bishop are lord ids 1 … 4 (`L2.eng` group 7).
    const BATTLE_LORD_SHIELD_VA: u32 = 0x004D_4CA8;
    let pair = |lord: u32| {
        (u32_at(BATTLE_LORD_SHIELD_VA + lord * 8), u32_at(BATTLE_LORD_SHIELD_VA + lord * 8 + 4))
    };
    assert_eq!(pair(1), (2, 4), "the Knight prefers yellow, falling back to magenta");
    assert_eq!(pair(2), (1, 5), "the Baron prefers red, falling back to blue");
    assert_eq!(pair(3), (5, 1), "the Countess prefers blue, falling back to red");
    assert_eq!(pair(4), (4, 2), "the Bishop prefers magenta, falling back to yellow");
    // Three of the four first columns are the player's own words. The Baron is
    // the one he could not remember, and in *this* table it is red — while the
    // campaign gives him black, which is the whole point of §7a.
    assert_ne!(pair(2).0, 3, "the Baron's battle preference is not the campaign's black");

    // --- `g_lordChoice`: per colour, and it is the campaign's.
    const LORD_CHOICE_VA: u32 = 0x004D_C17C;
    let candidates = |group: u32, shield: u32| -> [u8; 4] {
        let base = LORD_CHOICE_VA + group * 0x14 + shield * 4;
        [u8_at(base), u8_at(base + 1), u8_at(base + 2), u8_at(base + 3)]
    };
    // England is slot 0, so group 0 — the arrangement every default game shows.
    assert_eq!(candidates(0, 2), [1, 3, 4, 2], "yellow leads with the Knight");
    assert_eq!(candidates(0, 3), [2, 1, 3, 4], "black leads with the Baron");
    assert_eq!(candidates(0, 4), [4, 3, 2, 1], "magenta leads with the Bishop");
    assert_eq!(candidates(0, 5), [3, 2, 4, 1], "blue leads with the Countess");
    assert_eq!(candidates(0, 1), [2, 1, 3, 4], "red leads with the Baron");

    // Slot 0 is never indexed — shields are 1 … 5 — and the index runs one row
    // off its own group, so slot 5 of group *g* is slot 0 of group *g+1*.
    for group in 0..3 {
        assert_eq!(
            candidates(group, 5),
            candidates(group + 1, 0),
            "group {group}'s slot 5 should be group {}'s slot 0",
            group + 1
        );
    }
    // …and group 3's overrun lands on `g_realmColour`'s own base, overlapping
    // it by two bytes. That is what the two bytes below the pen table are, and
    // `the_realm_pen_table_matches_the_bytes_in_the_binary` asserts their
    // values from the other side.
    let overrun = LORD_CHOICE_VA + 3 * 0x14 + 5 * 4;
    assert_eq!(overrun, 0x004D_C1CC);
    assert_eq!(overrun + 2, chrome::REALM_PEN_VA - 2, "the two tables overlap by two bytes");
    assert_eq!(candidates(3, 5)[2..], [0x04, 0x02], "and those two bytes are shared");

    eprintln!(
        "lord colours: g_lordChoice is per-colour, g_battleLordShield per-lord; \
         the two tables overlap by two bytes at {:#010X}",
        overrun + 2
    );
}

/// **What the campaign actually produces for each of the five colours a person
/// can take** — the rule, run against the tables the test above pinned.
///
/// This is `Realms_AssignLords`' walk, and it is the thing no fixture can
/// check: every `.sav` this project holds has the human on shield 1 or shield
/// 5, never a middle colour, so none of them exercises a collision the two
/// candidate readings disagree about.
///
/// The row that matters is the second. *"The game always tries to give the
/// Knight yellow"* is true of four rows out of five and false of that one, and
/// it fails in a way no preference rule would produce: the Knight takes black
/// because black's candidate list reaches him second, and the **Baron** takes
/// red. `docs/rules.md` §7a.
#[test]
fn taking_a_middle_colour_moves_the_lords_and_not_only_their_colours() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let u8_at = |va: u32| -> u8 {
        exe[va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"))]
    };

    /// `Realms_AssignLords`, for one human on realm 1 and four AI lords.
    /// Returns `(shield, lord)` for realms 2 … 5.
    let assign = |human_shield: u8| -> Vec<(u8, u8)> {
        let mut shield_taken = [false; 6];
        shield_taken[human_shield as usize] = true;
        let mut lord_taken = [false; 8];
        let mut out = Vec::new();
        for _realm in 2..=5u8 {
            // The lowest shield nobody has taken.
            let shield = (1..=5u8).find(|s| !shield_taken[*s as usize]).expect("a free shield");
            shield_taken[shield as usize] = true;
            // Then the lord, from that shield. England is slot 0 -> group 0.
            let base = 0x004D_C17C + u32::from(shield) * 4;
            let lord = (0..4)
                .map(|n| u8_at(base + n))
                .find(|&c| c != 0 && !lord_taken[c as usize & 7])
                .unwrap_or(0);
            lord_taken[lord as usize & 7] = true;
            out.push((shield, lord));
        }
        out
    };

    const KNIGHT: u8 = 1;
    const BARON: u8 = 2;
    const COUNTESS: u8 = 3;
    const BISHOP: u8 = 4;

    // The default, and the one arrangement a fixture can confirm: it is exactly
    // what `england-turn1.sav` holds.
    assert_eq!(
        assign(1),
        vec![(2, KNIGHT), (3, BARON), (4, BISHOP), (5, COUNTESS)],
        "taking red gives the arrangement every default game shows"
    );

    // **Take yellow and the Knight does not keep it, and does not simply shift
    // one along.** He becomes the black lord; the Baron becomes the red one.
    assert_eq!(
        assign(2),
        vec![(1, BARON), (3, KNIGHT), (4, BISHOP), (5, COUNTESS)],
        "taking yellow makes the Baron red and the Knight black"
    );

    assert_eq!(assign(3), vec![(1, BARON), (2, KNIGHT), (4, BISHOP), (5, COUNTESS)]);
    assert_eq!(
        assign(4),
        vec![(1, BARON), (2, KNIGHT), (3, COUNTESS), (5, BISHOP)],
        "taking magenta moves the Countess to black and the Bishop to blue"
    );
    assert_eq!(assign(5), vec![(1, BARON), (2, KNIGHT), (3, COUNTESS), (4, BISHOP)]);

    // The description, stated as the count that makes it a description: the
    // Knight has yellow in four of the five, and never in the one where the
    // person took it.
    let knight_yellow = (1..=5u8)
        .filter(|&h| assign(h).iter().any(|&(s, l)| l == KNIGHT && s == 2))
        .count();
    assert_eq!(knight_yellow, 4, "true four times in five, which is why it reads as a rule");
    assert!(
        !assign(2).iter().any(|&(s, l)| l == KNIGHT && s == 2),
        "and false exactly where the person took it"
    );
    eprintln!("lord colours: the Knight has yellow in {knight_yellow} of 5 openings");
}
