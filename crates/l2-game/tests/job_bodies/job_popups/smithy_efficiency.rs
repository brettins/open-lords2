#![allow(unused_imports)]
use super::*;

/// Panel_JobBlacksmith 0x00413155: `Ui_DrawNumber((int)(char)industry[2].efficiency,
/// '@', &DAT_004d3e94, …)` — a byte read signed, so 200 prints -56.
/// Ablation: drop `as u8 as i8` in `blacksmith_part.rs` → "200%" is drawn instead.
#[test]
fn the_smithy_efficiency_is_read_as_a_signed_byte() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = (1..game.kingdom.counties.len())
        .find(|&i| game.kingdom.counties[i].owner == player)
        .expect("the player holds a county");
    game.kingdom.options.advanced_farming = true;
    game.kingdom.counties[id].industry[Commodity::Weapons.index()].efficiency = 200;
    let smiths = game.kingdom.counties[id].labour[JOB_BLACKSMITH].to_string();

    let canvas = draw_job(&mut game, &assets, id, JOB_BLACKSMITH);
    let f = body(&assets);
    let at = word_at(&canvas, &assets, 76, 4, 0x10 + advance(f, &format!("@{smiths} ")), 0x1A4);
    expect_at(&canvas, f, "-56%", INK, at + 4, 0x1A4);
}
