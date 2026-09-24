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

/// FUN_004BE33F (`0x004BE33F`): the ordered cell carries misc_bat frame 0x3F at
/// `(cell - cam) * 32 + view + (2, 9)` while `DAT_00553F84` (0x1E ticks) runs.
/// Ablation: drop `draw_order_mark` from the battlefield draw and the blit assert goes red.
#[test]
fn an_order_leaves_the_destination_marker_for_thirty_ticks() {
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
    let to = (fx + 3, fy + 2);
    let there = pixel(live(&g), to);
    click_at(&mut m, &mut g, &assets, there);
    assert_eq!(live(&g).order_mark, to, "the order was not given");
    assert_eq!(live(&g).order_mark_ticks, 0x1E);

    let sheet = assets.chrome.as_ref().and_then(|c| c.misc_bat()).expect("Misc_bat.pl8");
    let mark = sheet.frame(0x3F).expect("frame 0x3F");
    let lv = live(&g);
    let (x, y) = ((to.0 as i32 - lv.cam.0) * 32 + 2, (to.1 as i32 - lv.cam.1) * 32 + 24 + 9);
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    let mut over = canvas.clone();
    over.blit(&mark, x, y);
    assert!(over == canvas, "frame 0x3F is not at ({x}, {y})");

    g.battle.as_mut().unwrap().order_mark_ticks = 1;
    g.battle.as_mut().unwrap().tick();
    assert_eq!(live(&g).order_mark_ticks, 0, "FUN_00478952 counts it down each battle tick");
}
