#![allow(unused_imports)]
use super::*;
use super::cell_selection::*;
use super::banner_and_wall_rendering::*;
use super::*;
use super::tables_and_sheets::*;
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

/// The probe is the viewport `FUN_004BC020` stores — `x 0…480`, `y 24…472` —
/// written out
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

