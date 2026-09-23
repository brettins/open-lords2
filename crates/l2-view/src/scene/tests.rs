use super::*;
use l2_sim::runner::Army;

#[test]
fn the_viewport_matches_the_original_and_fits_the_screen() {
    assert_eq!(VIEW_COLS as i32 * TILE, 480);
    assert_eq!(VIEW_ROWS as i32 * TILE + ORIGIN_Y, 472);
    assert!(VIEW_COLS as i32 * TILE <= crate::canvas::WIDTH as i32);
    assert!(ORIGIN_Y + VIEW_ROWS as i32 * TILE <= crate::canvas::HEIGHT as i32);
}

/// The clip, against `FUN_004BC020`'s four stores worked out by hand for the
/// battle's arguments `(…, 0, 0x18, 0xF, 0xE, 0x20)`.
#[test]
fn the_sprite_clip_is_the_one_fun_004bc020_stores() {
    assert_eq!(FIELD_CLIP, Clip::new(0, 0x18, 0xF * 0x20, 0xE * 0x20 + 0x18));
}

#[test]
fn the_camera_never_lets_the_viewport_leave_the_map() {
    assert_eq!(Camera::clamped(-5, -5), Camera { x: 0, y: 0 });
    assert_eq!(Camera::clamped(500, 500), Camera { x: 65, y: 66 });
    let c = Camera::centred_on(40, 40);
    assert!(c.x + VIEW_COLS <= DIM && c.y + VIEW_ROWS <= DIM);
}

#[test]
fn a_man_walking_east_is_drawn_further_east_every_tick() {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * DIM + 40] = 0x04;
    layer[74 * DIM + 40] = 0x0F;
    let field = l2_sim::terrain::build(&layer, 1);
    let mut runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: &[(Troop::Peasants, 1)], owner: 2, human: false },
        Army { troops: &[(Troop::Macemen, 1)], owner: 1, human: true },
    );
    let man = runner.fighters.iter().position(|f| f.side == SIDE_A).unwrap();
    let (x0, y0) = (runner.fighters[man].x, runner.fighters[man].y);
    runner.order_side(SIDE_A, x0 + 5, y0);
    let cam = Camera::clamped(x0 as i32 - 4, y0 as i32 - 6);

    let mut xs = vec![figure_origin(&runner.fighters[man], cam).0];
    for _ in 0..(5 * 18 + 40) {
        runner.step();
        let (x, y) = figure_origin(&runner.fighters[man], cam);
        assert_eq!(y, ORIGIN_Y + (y0 as i32 - cam.y as i32) * TILE, "he left his row");
        xs.push(x);
    }
    for (t, w) in xs.windows(2).enumerate() {
        assert!(w[1] >= w[0], "tick {t}: drawn {} pixels west — {xs:?}", w[0] - w[1]);
    }
    assert_eq!(xs.last().unwrap() - xs[0], 5 * TILE, "{xs:?}");
    let steps = xs.windows(2).filter(|w| w[1] > w[0]).count();
    assert!(steps >= 5 * 8, "{steps} forward steps over five cells: {xs:?}");
    assert_eq!((runner.fighters[man].x, runner.fighters[man].y), (x0 + 5, y0));
}
