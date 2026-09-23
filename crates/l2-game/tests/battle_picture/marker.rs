//! `FUN_004BE070` (`0x004BE070`) marks a picked man with `Misc_bat.pl8` frame
//! `men << 2 + 0x2E` (size class 0: `DAT_004D9680[0] = 1`, `DAT_004D96A8[0] = 2`)
//! at his origin `+ (8, -0xC)`; we drew a `widget::frame` box. Ablation: restore
//! the box in `draw_selection_markers` and both asserts go red.

use super::*;

#[test]
fn a_picked_man_carries_the_original_s_strength_marker_not_our_box() {
    let Some((assets, _platform)) = install() else {
        l2_testkit::skip!("no game install, so no Misc_bat.pl8");
    };
    let (mut g, mut m) = staged(30, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 8)], |(x, y)| {
        (x as i32 - 4, y as i32 - 6)
    });
    let man = human_figure(&g);
    let (fx, fy) = (live(&g).runner.fighters[man].x, live(&g).runner.fighters[man].y);
    let at = pixel(live(&g), (fx, fy));
    click_at(&mut m, &mut g, &assets, at);
    assert_eq!(live(&g).runner.selected_fighters(1), vec![man]);
    assert_eq!(live(&g).runner.size_class(), 0, "sixteen men is the smallest class");

    let boxes = l2_game::widget::framed();
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    assert_eq!(l2_game::widget::framed(), boxes, "our selection box was drawn");

    let lv = live(&g);
    let fig = &lv.runner.fighters[man];
    let men = lv.runner.sim.figures[fig.sim].men as usize;
    let cam = l2_view::scene::Camera::clamped(lv.cam.0, lv.cam.1);
    let (x, y) = l2_view::scene::figure_origin(fig, cam);
    let sheet = assets.chrome.as_ref().and_then(|c| c.misc_bat()).expect("Misc_bat.pl8");
    let marker = sheet.frame((men << 2) + 0x2E).expect("the marker frame");
    let mut over = canvas.clone();
    over.blit(&marker, x + 8, y - 0xC);
    assert!(over == canvas, "frame {} is not at ({}, {})", (men << 2) + 0x2E, x + 8, y - 0xC);
}
