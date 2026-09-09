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
    for zoom in campaign::ZOOMS {
        for name in zoom.banks {
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
    assert_eq!(checked, 2 * (140 + 25 + 140 + 61 + 100), "all ten banks, every frame");
    eprintln!("campaign tiles: {checked} frames match the renderer's pitch");
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
