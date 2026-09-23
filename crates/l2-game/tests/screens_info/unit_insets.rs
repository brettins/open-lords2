#![allow(unused_imports)]
use super::*;
use common::*;

/// UnitPanel_Draw 0x0041B19D, R = 0xF for all three:
/// kinds 3, 2 `Ui_DrawInsetRect(0x20, R*0x10+0x38, 400, (0x18-R)*0x10)`,
/// kind 4 `Ui_DrawInsetRect(0x20, R*0x10+0x50, 400, (0x16-R)*0x10)`.
/// `Ui_DrawInsetRect` 0x00403DEB: right edge 0x10, left edge 0x1F.
/// Ablation: drop the `pen.inset` arm in `screen_impl.rs` → both edges fail.
#[test]
fn merchant_peasant_and_transport_panels_draw_their_inset() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    const R: i32 = 0x0F;
    for (kind, top, h) in [
        (UnitKind::Merchant, R * 16 + 0x38, (0x18 - R) * 16),
        (UnitKind::PeasantMob, R * 16 + 0x38, (0x18 - R) * 16),
        (UnitKind::Transport, R * 16 + 0x50, (0x16 - R) * 16),
    ] {
        let mut u = Unit::new(kind, 0, 10, 10);
        u.men = 100;
        let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
        let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
        // Below the icon and the body text: the edges' lowest rows.
        for y in [top + h - 12, top + h - 2] {
            assert_eq!(canvas.at(0x20 + 400 - 1, y as usize), 0x10, "{kind:?}: right edge at y {y}");
            assert_eq!(canvas.at(0x20, y as usize), 0x1F, "{kind:?}: left edge at y {y}");
        }
        game.kingdom.campaign.units.remove(id);
    }
}
