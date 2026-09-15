#![allow(unused_imports)]
use super::*;
use super::rendering_tests::*;
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

/// `Readme.txt`: *"Note that only the Stone and Royal castles have
/// drawbridges."* Structure code 9 — `surface = 0x0B`, `flags = 0x40`, which
/// is the flag `Siege_LowerDrawbridge` (`FUN_00496B9F`) searches for — is
/// carried by four frames of the stone table and by **none** of the wooden
/// one. Two oracles, opposite ends, same answer.
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
    assert_eq!(frames_with_code(4, code::WALL), vec![0xAC, 0xAD, 0xAE, 0xAF]);
    assert_eq!(frames_with_code(0, code::WALL), vec![0x76, 0x77, 0x78, 0x79]);
}


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


