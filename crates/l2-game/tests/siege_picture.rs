//! **A siege is fought on the castle's tiles, not the field's.**
//!
//! ```text
//! cargo test -p l2-game --test siege_picture
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test siege_picture
//! ```
//!
//! `Battle_LoadAssets` (`0x004987B7`) fills the tile renderer's two sheet
//! pointers from slots 0 and 1 of the battle asset table at `0x004DA550`, and
//! the ladder that picks them is two flags:
//!
//! ```c
//! if (g_battleIsSiege == 0)   { if (slot == 1) continue; entry = 0; }      /* t32_bat1     */
//! else if (DAT_0057C910 == 0) { entry = slot == 1 ? 5 : 4; }               /* t32_wod1/2   */
//! else                        { entry = slot == 1 ? 3 : 2; }               /* t32_stn1/2   */
//! ```
//!
//! with `DAT_0057C910 = (uint)(1 < g_castleLevel)` (`Siege_LaunchAssault`, `0x004A8AAB`).
//! `Screen_DrawBattlefield` (`0x004233F7`) then ends the repaint with
//! `Palette_Set(0x568EE0)` or `Palette_Set(0x5675A0)` on the same siege flag.
//! Every battle here was drawn from `T32_bat1.pl8` under `T32_bat1.256`.
//!
//! **Which of the two sheets a cell comes from is cell byte `+2`**, bits
//! `0x1C`: `Battlefield_Draw32` (`0x004BCBDC`) takes slot 0 at 0 and slot 1 at
//! 4, and nothing else draws at all. The castle is slot 0 — masonry, towers,
//! gates, the keep door, the wall walk — and the ground it stands on is slot
//! 1: sixteen grass variants, the moat's 49-variant water set and the rubble a
//! collapsed wall leaves. That split is `Battlefield_BuildCastle`'s three
//! escape codes (`FUN_0047E1DC`, `FUN_0047DCCE`, `FUN_0047DFE0`), each of
//! which sets the selector, against every raster cell, which clears it.

use l2_game::battlefield::LiveBattle;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::siege::{code, frames_with_code, STRUCTURE_STONE, STRUCTURE_WOOD};
use l2_sim::terrain::{tileset, DIM};
use l2_sim::Troop;
use l2_view::scene::{self, Ground};
use l2_view::Canvas;

/// `.data` begins at RVA `0x4D2000` / file offset `0xD0200`, so the two
/// structure tables at `0x004D7B80` and `0x004D7D80` are these. Written out
/// rather than derived from the constants under test.
const STONE_AT: usize = 0xD5D80;
const WOOD_AT: usize = 0xD5F80;

fn staged(castle_level: Option<u8>) -> (Game, Machine) {
    let field = match castle_level {
        Some(level) => l2_sim::siege::our_castle(level),
        None => l2_sim::runner::blank_field(),
    };
    let human: &[(Troop, u16)] = &[(Troop::Swordsmen, 4)];
    let ai: &[(Troop, u16)] = &[(Troop::Pikemen, 4)];
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let mut live = LiveBattle::new(runner, 0, 0, 0, castle_level, 1, 1);
    live.paused = true;
    // On the castle, looking at the wall the gate is in.
    live.cam = (33, 17);
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.prefs.tool_tips = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

fn paint(m: &mut Machine, g: &mut Game, a: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    {
        let mut ctx = Ctx { game: g, assets: a };
        m.update(&mut ctx);
    }
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut canvas);
    canvas
}

// --------------------------------------------------------------- the tables

/// **The two structure tables are the binary's bytes**, re-read from the
/// player's own `Lords2.exe` every run.
///
/// They are the only thing that says which of a castle sheet's 256 frames is a
/// wall, a keep door or a drawbridge plank, and
/// [`l2_sim::siege::paint_our_castle`] derives every tile it lays from them.
/// A hand-typed table that drifted would draw a castle out of the wrong tiles
/// and nothing else would notice.
///
/// Ablation, run: frame 0xAF's code 8 -> 7 — red, *"0x004D7B80 stone: byte
/// 350 (frame 0xAF, code) is 7 here and 8 in Lords2.exe"*, and the drawbridge
/// test below goes red with it.
#[test]
fn the_castle_structure_tables_are_the_binarys_own() {
    let exe = l2_testkit::executable!();
    for (name, at, ours) in [
        ("0x004D7B80 stone", STONE_AT, &STRUCTURE_STONE),
        ("0x004D7D80 wood", WOOD_AT, &STRUCTURE_WOOD),
    ] {
        let theirs = &exe[at..at + 512];
        for i in 0..512 {
            assert_eq!(
                ours[i], theirs[i],
                "{name}: byte {i} (frame {:#04X}, {}) is {} here and {} in Lords2.exe",
                i / 2,
                if i % 2 == 0 { "code" } else { "passable" },
                ours[i],
                theirs[i]
            );
        }
    }
}

/// **Only the stone castles have a drawbridge, and the tile tables say so on
/// their own.**
///
/// `Readme.txt`: *"Note that only the Stone and Royal castles have
/// drawbridges."* Structure code 9 — `surface = 0x0B`, `flags = 0x40`, which
/// is the flag `Siege_LowerDrawbridge` (`FUN_00496B9F`) searches for — is
/// carried by four frames of the stone table and by **none** of the wooden
/// one. Two oracles, opposite ends, same answer.
///
/// Needs no install: both halves are in-tree constants, and the test above is
/// what holds them to the binary.
#[test]
fn the_wooden_castles_tile_table_has_no_drawbridge_and_the_stone_one_has_four() {
    for level in [0u8, 1] {
        assert!(
            frames_with_code(level, code::DRAWBRIDGE).is_empty(),
            "level {level} is a wooden castle and its table files no frame under code 9"
        );
    }
    for level in [2u8, 3, 4] {
        assert_eq!(
            frames_with_code(level, code::DRAWBRIDGE),
            vec![0xA4, 0xA5, 0xA6, 0xA7],
            "level {level}"
        );
    }
    // And the wall, which both families do have.
    assert_eq!(frames_with_code(4, code::WALL), vec![0xAC, 0xAD, 0xAE, 0xAF]);
    assert_eq!(frames_with_code(0, code::WALL), vec![0x76, 0x77, 0x78, 0x79]);
}

// ------------------------------------------------------------- the selection

/// **The sheets and the palette a battle runs under, by castle level** —
/// `Battle_LoadAssets`' ladder, end to end, with no install needed: the names
/// are the table's.
///
/// Ablation, run: `Ground::for_battle` answering `Field` for every siege — red
/// here, on the palette test below, and on the picture test, which then finds
/// 212,253 of 215,040 viewport pixels unpainted because most of a castle asks
/// for a slot-1 sheet that is not there.
#[test]
fn the_castle_level_picks_the_sheets_and_the_palette() {
    assert_eq!(Ground::for_battle(None), Ground::Field);
    assert_eq!(Ground::for_battle(None).tileset(), "T32_bat1.pl8");
    assert_eq!(Ground::for_battle(None).tileset2(), None);
    assert_eq!(Ground::for_battle(None).palette(), "T32_bat1.256");
    for level in [0u8, 1] {
        let g = Ground::for_battle(Some(level));
        assert_eq!(g, Ground::Wood, "level {level}: DAT_0057C910 is (1 < level)");
        assert_eq!(g.tileset(), "T32_wod1.pl8");
        assert_eq!(g.tileset2(), Some("T32_wod2.pl8"));
        assert_eq!(g.palette(), "T32_stn1.256", "there is no t32_wod1.256");
    }
    for level in [2u8, 3, 4] {
        let g = Ground::for_battle(Some(level));
        assert_eq!(g, Ground::Stone, "level {level}");
        assert_eq!(g.tileset(), "T32_stn1.pl8");
        assert_eq!(g.tileset2(), Some("T32_stn2.pl8"));
        assert_eq!(g.palette(), "T32_stn1.256");
    }
}

/// **The screen asks for the siege palette**, which is the whole of
/// `Screen_DrawBattlefield`'s last line. Placeholder assets, because a name is
/// a name.
///
/// Ablation, run: with `Ground::for_battle` forced to `Field` this is red on
/// both sieges. Deleting "T32_stn1.256" from `shell::PALETTES` is red in
/// `shell`'s own test instead.
#[test]
fn a_siege_presents_through_t32_stn1_and_a_field_battle_does_not() {
    let a = Assets::placeholder();
    for (level, want) in
        [(None, "T32_bat1.256"), (Some(0), "T32_stn1.256"), (Some(4), "T32_stn1.256")]
    {
        let (mut g, mut m) = staged(level);
        let _ = paint(&mut m, &mut g, &a);
        assert_eq!(m.palette_name(), Some(want), "castle level {level:?}");
    }
}

// ----------------------------------------------------------------- the cells

/// **A siege's ground, ditch and castle are on the two different sheets the
/// builder puts them on.**
///
/// `our_castle`'s arrangement is ours, but which sheet each kind of cell comes
/// from is not: `FUN_0047E1DC` and `FUN_0047DCCE` set the selector on open
/// ground and on the moat, and a cell taken from the raster leaves it clear.
///
/// Ablation, run: drop the `flags2` write from `paint_our_castle`'s `None`
/// arms — red on *"the moat is slot 1"*, and the picture test goes red with it.
#[test]
fn the_castle_is_on_sheet_zero_and_the_ground_and_moat_on_sheet_one() {
    let field = l2_sim::siege::our_castle(4);
    let walls: Vec<_> = field
        .cells
        .iter()
        .filter(|c| c.flags & l2_sim::siege::FLAG_WALL != 0)
        .collect();
    assert!(walls.len() > 50, "a level-4 ring is more than 50 cells: {}", walls.len());
    for c in &walls {
        assert_eq!(c.tileset(), 0, "a wall is masonry, which is slot 0");
        assert!(
            frames_with_code(4, code::WALL).contains(&c.gfx),
            "a wall's frame {:#04X} is one the table files under code 8",
            c.gfx
        );
    }

    let moat: Vec<_> = field
        .cells
        .iter()
        .filter(|c| c.surface == l2_sim::siege::SURFACE_WATER)
        .collect();
    assert!(!moat.is_empty(), "a level-4 castle has a ditch");
    for c in &moat {
        assert_eq!(c.tileset(), 1, "the moat is slot 1");
    }
    // The 49-variant water table's indices, which `terrain::build` also uses.
    assert!(moat.iter().map(|c| c.gfx).collect::<std::collections::BTreeSet<_>>().len() > 3);

    let open = field.cells.iter().filter(|c| c.surface == l2_sim::siege::SURFACE_FIELD);
    let mut seen = std::collections::BTreeSet::new();
    for c in open {
        assert_eq!(c.tileset(), 1, "open ground is slot 1");
        assert!(c.gfx < 0x10, "open ground is rand & 0x0F, not {:#04X}", c.gfx);
        seen.insert(c.gfx);
    }
    assert_eq!(seen.len(), 16, "all sixteen grass variants are used");
}

/// **A field battlefield never asks for the second sheet**, which is why the
/// original can leave slot 1 null for one: `Battlefield_BuildFromSkr` clears
/// bits `0x1C` on every cell it writes, and `t32_bat2.pl8`'s size in the asset
/// table is 0.
///
/// The elevation overlay is the same story from the other side — it is gated
/// on `elevation` 1…3, and `elevation` is the one cell byte that builder never
/// writes.
#[test]
fn a_field_battlefield_asks_for_neither_the_second_sheet_nor_the_overlay() {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[20 * DIM + 40] = 0x04;
    layer[60 * DIM + 40] = 0x0F;
    let field = l2_sim::terrain::build(&layer, 1);
    assert!(field.cells.iter().all(|c| c.flags2 & tileset::MASK == 0));
    assert!(field.cells.iter().all(|c| c.tileset() == 0));
    assert!(field.cells.iter().all(|c| c.elevation == 0));
}

// ---------------------------------------------------------------- the pixels

/// **The install's own sheets, drawn.** A level-4 siege and a field battle of
/// the same shape are painted through the whole screen stack; the two pictures
/// must differ over most of the viewport, and the siege's must have no holes.
///
/// The probe is the viewport `FUN_004BC020` stores — `x 0…480`, `y 24…472` —
/// written out rather than read from `l2_view::scene`.
///
/// Ablation, run: `Ground::for_battle` answering `Field` always — red, 212,253
/// of 215,040 viewport pixels never painted.
#[test]
fn a_siege_and_a_field_battle_of_the_same_shape_are_different_pictures() {
    let dir = l2_testkit::install!();
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let a = Assets::load(&platform.vfs).expect("assets load");
    let art = a.battle.as_ref().expect("the install has the battle art");
    assert!(art.has_ground(Ground::Stone), "the install ships T32_stn1.pl8 and T32_stn2.pl8");
    assert!(art.has_ground(Ground::Wood), "the install ships T32_wod1.pl8 and T32_wod2.pl8");
    assert_eq!(art.ground(Ground::Stone).tiles.frame_count(), 256, "T32_stn1.pl8");
    assert_eq!(
        art.ground(Ground::Stone).tiles2.as_ref().expect("slot 1").frame_count(),
        247,
        "T32_stn2.pl8"
    );

    let (mut gs, mut ms) = staged(Some(4));
    let siege = paint(&mut ms, &mut gs, &a);
    let (mut gf, mut mf) = staged(None);
    let plain = paint(&mut mf, &mut gf, &a);

    let (x0, y0, x1, y1) = (0usize, 24usize, 480usize, 472usize);
    let mut differ = 0;
    let mut holes = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            if siege.at(x, y) != plain.at(x, y) {
                differ += 1;
            }
            if siege.at(x, y) == 0 {
                holes += 1;
            }
        }
    }
    let total = (x1 - x0) * (y1 - y0);
    assert_eq!(holes, 0, "{holes} of {total} viewport pixels were never painted");
    assert!(
        differ * 2 > total,
        "only {differ} of {total} viewport pixels differ between a siege and a field battle"
    );
    eprintln!("siege picture: {differ} of {total} viewport pixels differ from the field's");
}

/// **A cell's selector decides which sheet its pixels come from**, at the
/// pixel. One cell is switched to slot 1 and back with everything else held
/// still, and the tile under it must change.
///
/// Ablation, run: `let sheet = Some(tiles)` in `draw_terrain` — red here, and
/// red on the picture test with 16,145 unpainted pixels.
#[test]
fn the_cell_selector_moves_a_tile_between_the_two_sheets() {
    let dir = l2_testkit::install!();
    let read = |n: &str| {
        std::fs::read(dir.join(n)).map_err(|e| format!("{n}: {e}"))
    };
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8").expect("stn1")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8").expect("stn2")).expect("stn2");

    let mut field = l2_sim::siege::our_castle(4);
    let cam = scene::Camera { x: 30, y: 20 };
    let cell = (cam.y + 5) * DIM + (cam.x + 5);
    let (px, py) = (scene::ORIGIN_X + 5 * scene::TILE, scene::ORIGIN_Y + 5 * scene::TILE);

    // Frame 4 of each sheet, which the two files do not agree on.
    field.cells[cell].gfx = 4;
    field.cells[cell].elevation = 0;
    field.cells[cell].terrain = 0;

    field.cells[cell].flags2 = 0;
    let mut a = Canvas::screen();
    scene::draw_terrain(&mut a, &field, &one, Some(&two), cam);

    field.cells[cell].flags2 = tileset::SECOND;
    let mut b = Canvas::screen();
    scene::draw_terrain(&mut b, &field, &one, Some(&two), cam);

    let mut moved = 0;
    for y in 0..scene::TILE {
        for x in 0..scene::TILE {
            let (x, y) = ((px + x) as usize, (py + y) as usize);
            if a.at(x, y) != b.at(x, y) {
                moved += 1;
            }
        }
    }
    assert!(moved > 100, "only {moved} of 1024 pixels moved with the selector");
    assert_eq!(a.diff_count(&b), moved, "nothing outside that one cell changed");
}

/// **The raised-ground overlay** — `Battlefield_Draw32`'s second pass, frame
/// `terrain + 0x8B` out of slot 1, over any cell at elevation 1, 2 or 3.
///
/// Ablation, run: gate the overlay block on `false` — red, *"the overlay
/// painted nothing"*.
#[test]
fn a_raised_cell_takes_a_second_blit_from_the_second_sheet() {
    let dir = l2_testkit::install!();
    let read = |n: &str| std::fs::read(dir.join(n)).expect("a sheet");
    let one = l2_view::sheet::Sheet::new(read("T32_stn1.pl8")).expect("stn1");
    let two = l2_view::sheet::Sheet::new(read("T32_stn2.pl8")).expect("stn2");

    let mut field = l2_sim::siege::our_castle(4);
    let cam = scene::Camera { x: 30, y: 20 };
    let cell = (cam.y + 6) * DIM + (cam.x + 6);
    field.cells[cell].flags2 = 0;
    field.cells[cell].gfx = 0xAC;
    field.cells[cell].terrain = 1;

    let mut flat = Canvas::screen();
    field.cells[cell].elevation = 0;
    scene::draw_terrain(&mut flat, &field, &one, Some(&two), cam);

    let mut raised = Canvas::screen();
    field.cells[cell].elevation = 2;
    scene::draw_terrain(&mut raised, &field, &one, Some(&two), cam);

    let moved = flat.diff_count(&raised);
    assert!(moved > 0, "the overlay painted nothing");
    // Terrain 1 → frame 0x8C, and it is drawn over the cell and nowhere else.
    assert!(
        moved <= (scene::TILE * scene::TILE) as usize,
        "{moved} pixels moved for a one-cell overlay"
    );
    assert!(two.frame(0x8C).is_some(), "T32_stn2.pl8 carries the overlay frame");
}
