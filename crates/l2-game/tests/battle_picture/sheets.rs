//! `FUN_004987B7` (`0x004987B7`) loads each side's men from asset-table slot
//! `g_units[army].shield * 7 + 0x12 + troop`: a blue lord's men come from `a2b_`.
//! Ablation: pass fixed `Red, Blue` to `set_sides` in the battlefield draw and this goes red.

use super::*;
use l2_view::figures::Colour;

#[test]
fn each_side_s_men_take_their_lord_s_colour_sheet() {
    let Some((assets, _platform)) = install() else {
        l2_testkit::skip!("no game install, so no a2?_*.pl8");
    };
    let (mut g, mut m) =
        staged(30, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 8)], |(x, y)| (x as i32 - 4, y as i32 - 6));
    g.kingdom.realms[1].shield_index = 5;
    g.kingdom.realms[2].shield_index = 3;
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    let art = assets.battle.as_ref().expect("battle sheets");
    assert_eq!(art.sides(), [Colour::Black, Colour::Blue], "side 4 (AI, shield 3), side 0 (player, shield 5)");
}
