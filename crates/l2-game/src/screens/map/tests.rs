use super::*;

#[test]
fn the_screen_is_partitioned_with_no_gap_and_no_overlap() {
    assert_eq!(
        MAP_AREA.x + MAP_AREA.w,
        PANEL.x,
        "the map stops where the panel starts"
    );
    assert_eq!(PANEL.x + PANEL.w, 640);
    assert_eq!(PANEL.y, TOP_BAR);
    for b in SIDEBAR_BUTTONS {
        let r = b.rect();
        assert_eq!(
            r.y + r.h + 1,
            END_TURN_BUTTON.y,
            "{b:?}: the table leaves one dead row above the end-turn strip",
        );
        assert!(PANEL.contains(r.x, r.y), "{b:?} starts outside the sidebar");
        assert!(r.x + r.w <= PANEL.x + PANEL.w, "{b:?} runs past the screen edge");
    }
    assert_eq!(END_TURN_BUTTON.y + END_TURN_BUTTON.h, 479);
    assert_eq!(END_TURN_BUTTON.x + END_TURN_BUTTON.w, 639, "and the last column with it");
    for z in [NEAR, FAR] {
        assert_eq!(z.clip().x1, PANEL.x);
        assert_eq!(z.clip().y0, TOP_BAR);
    }
    let hit = chrome::minimap_hit_area();
    assert!(PANEL.contains(hit.x0, hit.y0));
    assert!(PANEL.contains(hit.x1 - 1, hit.y1 - 1));
}

#[test]
fn the_five_sidebar_buttons_tile_the_strip_and_all_five_name_a_screen() {
    let ends = [33, 65, 97, 129, 161];
    for (b, end) in SIDEBAR_BUTTONS.iter().zip(ends) {
        assert_eq!(b.x + b.w, end, "{b:?} does not end where the table says");
    }
    for (i, a) in SIDEBAR_BUTTONS.iter().enumerate() {
        for b in SIDEBAR_BUTTONS.iter().skip(i + 1) {
            let (ra, rb) = (a.rect(), b.rect());
            assert!(ra.x + ra.w <= rb.x || rb.x + rb.w <= ra.x, "{a:?} overlaps {b:?}");
        }
    }
    for b in SIDEBAR_BUTTONS {
        let SidebarAction::Screen(id) = b.action;
        assert_ne!(
            sidebar_destination(id, 1),
            ScreenId::Campaign,
            "{b:?} names screen {id:#04X}, which sidebar_destination does not build",
        );
    }
    assert_eq!(
        sidebar_destination(0x17, 4),
        ScreenId::RaiseArmy(4),
        "the ARMY button opens the raise-army screen for the selected county",
    );
}

#[test]
fn the_minimap_mode_buttons_are_the_originals_including_its_off_by_three() {
    for r in MINIMAP_MODE_BUTTONS {
        assert!(r.x >= PANEL.x && r.x + r.w <= 640, "{r:?} escapes the sidebar");
        assert!(r.y >= TOP_BAR, "{r:?} is under the menu bar");
    }
    let second = MINIMAP_MODE_BUTTONS[1];
    let third = MINIMAP_MODE_BUTTONS[2];
    assert_eq!(second.y + second.h, third.y + 2, "band 2 runs two rows into band 3");
    let first = MINIMAP_MODE_BUTTONS[0].contains(620, 96);
    assert!(!first);
    assert!(second.contains(620, 96), "and the first match wins, so y 96 is mode 2");
}

/// `FUN_00439122`, the farm/industry split slider, arithmetic and all.
#[test]
fn the_split_slider_snaps_to_fours_on_the_track_and_steps_by_four_off_it() {
    assert_eq!(split_from_click(531, 50), 0, "the track starts at x 531");
    assert_eq!(split_from_click(581, 50), 100, "and ends at 581");
    assert_eq!(split_from_click(532, 50), 0, "((1)*2) & 0xFC");
    assert_eq!(split_from_click(533, 50), 4, "((2)*2) & 0xFC");
    assert_eq!(split_from_click(534, 50), 4, "and 6 masks back to 4");
    assert_eq!(split_from_click(530, 50), 46, "left of the track steps down");
    assert_eq!(split_from_click(600, 50), 54, "right of it steps up");
    assert_eq!(split_from_click(530, 0), 0, "and both ends clamp");
    assert_eq!(split_from_click(600, 100), 100);
}

#[test]
fn the_pointer_at_an_edge_asks_for_the_direction_that_edge_means() {
    let mut s = MapScreen::new();
    s.pointer_in = true;
    let cases = [
        ((320, 0), Some(Dir::N)),
        ((CANVAS_W - 1, 0), Some(Dir::NE)),
        ((CANVAS_W - 1, 240), Some(Dir::E)),
        ((CANVAS_W - 1, CANVAS_H - 1), Some(Dir::SE)),
        ((320, CANVAS_H - 1), Some(Dir::S)),
        ((0, CANVAS_H - 1), Some(Dir::SW)),
        ((0, 240), Some(Dir::W)),
        ((0, 0), Some(Dir::NW)),
        ((320, 240), None),
        ((1, 1), None),
        ((CANVAS_W - 2, CANVAS_H - 2), None),
    ];
    for (at, want) in cases {
        s.pointer = at;
        assert_eq!(s.edge_direction(), want, "pointer at {at:?}");
    }
    s.pointer = (0, 240);
    s.pointer_in = false;
    assert_eq!(s.edge_direction(), None, "the pointer is not over the window");
}

#[test]
fn a_pointer_at_the_edge_scrolls_and_a_pointer_just_inside_it_does_not() {
    use crate::input::window;
    let assets = crate::game::Assets::placeholder();
    let start = Viewport::new(40, 20);

    let (ww, wh) = (1898u32, 1562u32);
    let run = |at: (f64, f64)| -> Viewport {
        let mut s = MapScreen::new();
        s.view = start;
        let (x, y) = window::to_canvas(ww, wh, at.0, at.1);
        let mut ctx = Ctx { game: &mut crate::Game::new(1), assets: &assets };
        s.handle(Event::Pointer { x, y }, &mut ctx);
        s.update(&mut ctx);
        s.viewport()
    };

    assert_eq!(run((309.0, 781.0)), Viewport::new(40, 19), "the picture's left column");
    assert_eq!(run((1588.0, 781.0)), Viewport::new(40, 21), "and its right one");

    assert_eq!(run((0.0, 781.0)), Viewport::new(40, 19), "the far left of the window");
    assert_eq!(run((1897.0, 781.0)), Viewport::new(40, 21), "the far right");
    assert_eq!(run((949.0, 0.0)), Viewport::new(38, 20), "the top");
    assert_eq!(run((949.0, 1561.0)), Viewport::new(42, 20), "the bottom");
    assert_eq!(run((0.0, 0.0)), Viewport::new(38, 19), "the top-left corner");

    assert_eq!(run((311.0, 781.0)), start, "one canvas pixel in from the left");
    assert_eq!(run((1586.0, 781.0)), start, "and from the right");
    assert_eq!(run((949.0, 781.0)), start, "and the middle of the screen");

}

#[test]
fn holding_the_pointer_at_the_edge_scrolls_at_the_originals_rate() {
    let mut game = crate::Game::new(1);
    let assets = crate::game::Assets::placeholder();
    let mut s = MapScreen::new();
    s.view = Viewport::new(40, 20);
    s.pointer = (CANVAS_W - 1, 240);
    s.pointer_in = true;

    let every = s.scroll_interval_ticks();
    assert_eq!(every, 3, "the default 60 is 50 ms, which is three of our 16 ms ticks");

    let mut moved = 0;
    for tick in 1..=every * 3 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        s.update(&mut ctx);
        if s.viewport().col > 20 + moved {
            moved += 1;
            assert!(s.take_redraw(), "a tick that moved the map has to be drawn");
        }
        assert_eq!(
            s.viewport(),
            Viewport::new(40, 20 + moved),
            "tick {tick}: one tile every {every} ticks and no more",
        );
    }
    assert_eq!(moved, 3, "three steps in nine ticks, not nine");

    s.pointer = (240, 240);
    let settled = s.viewport();
    for _ in 0..every * 2 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        s.update(&mut ctx);
    }
    assert_eq!(s.viewport(), settled);
}

#[test]
fn the_scroll_throttle_reproduces_the_originals_interval_at_every_setting() {
    let mut s = MapScreen::new();
    let table = [
        (0, None, u32::MAX),
        (10, Some(110), 7),
        (20, Some(98), 6),
        (30, Some(86), 5),
        (40, Some(74), 5),
        (50, Some(62), 4),
        (60, Some(50), 3),
        (70, Some(38), 2),
        (80, Some(26), 2),
        (90, Some(14), 1),
        (100, Some(2), 1),
    ];
    for (speed, ms, ticks) in table {
        s.set_scroll_speed(speed);
        if let Some(ms) = ms {
            let q = (100 - speed) / 10;
            assert_eq!(q * 12 + 2, ms, "speed {speed}: the original's own arithmetic");
        }
        assert_eq!(s.scroll_interval_ticks(), ticks, "speed {speed}");
    }
    s.set_scroll_speed(-40);
    assert_eq!(s.scroll_interval_ticks(), u32::MAX, "below zero is still 'never'");
    s.set_scroll_speed(4_000);
    assert_eq!(s.scroll_interval_ticks(), 1);
}

#[test]
fn scroll_speed_zero_never_scrolls() {
    let mut game = crate::Game::new(1);
    let assets = crate::game::Assets::placeholder();
    let mut s = MapScreen::new();
    s.view = Viewport::new(40, 20);
    s.pointer = (CANVAS_W - 1, 240);
    s.pointer_in = true;
    s.set_scroll_speed(0);
    for _ in 0..200 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        s.update(&mut ctx);
    }
    assert_eq!(s.viewport(), Viewport::new(40, 20));
}

#[test]
fn the_diamonds_leave_no_pixel_unpicked() {
    for zoom in [NEAR, FAR] {
        let mut s = MapScreen::new();
        s.zoom = zoom;
        s.view = Viewport::new(60, 30).clamped(&zoom);
        let (tx, ty) = (1..l2_kingdom::MAP_DIM - 1)
            .flat_map(|y| (1..l2_kingdom::MAP_DIM - 1).map(move |x| (x, y)))
            .find(|&(x, y)| {
                [(x, y), (x + 1, y), (x, y + 1), (x - 1, y), (x, y - 1)].iter().all(|&(a, b)| {
                    campaign::tile_centre(s.view, &zoom, a, b).is_some_and(|(cx, cy)| {
                        let c = s.map_clip();
                        cx - zoom.pitch > c.x0
                            && cx + zoom.pitch < c.x1
                            && cy - zoom.row_step * 2 > c.y0
                            && cy + zoom.row_step * 2 < c.y1
                    })
                })
            })
            .expect("a tile with room around it");
        let (cx, cy) = campaign::tile_centre(s.view, &zoom, tx, ty).expect("in view");

        let mut misses = Vec::new();
        for dy in -zoom.row_step..=zoom.row_step {
            for dx in -zoom.half_pitch..=zoom.half_pitch {
                if s.pick_tile(cx + dx, cy + dy).is_none() {
                    misses.push((dx, dy));
                }
            }
        }
        assert!(
            misses.is_empty(),
            "zoom {}: {} pixels around a tile centre pick nothing, first at {:?} \
             — the diamonds do not tile the plane",
            zoom.id,
            misses.len(),
            misses.first(),
        );
    }
}

#[test]
fn the_pick_diamond_is_the_lattice_and_not_the_picture() {
    for zoom in [NEAR, FAR] {
        assert_eq!(zoom.half_pitch * 2, zoom.pitch, "zoom {}", zoom.id);
    }
    assert_eq!(NEAR.tile_w / 2, 29);
    assert_eq!(NEAR.half_pitch, 30);
    assert_eq!(FAR.tile_w / 2, 5);
    assert_eq!(FAR.half_pitch, 6);
}

#[test]
fn the_screen_opens_at_the_originals_zoom_and_scroll_origin() {
    let s = MapScreen::new();
    assert_eq!(s.zoom().id, NEAR.id);
    assert_eq!(s.viewport(), Viewport::new(0x4A, 0x14));
    assert_eq!(s.map_clip(), NEAR.clip());
}

#[test]
fn the_zoom_toggle_saves_and_restores_the_near_views_position() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut s = MapScreen::new();
    s.view = Viewport::new(60, 30);
    s.toggle_zoom(&mut ctx);
    assert_eq!(s.zoom().id, FAR.id);
    assert!(ctx.game.map_zoom_far, "the far zoom did not reach Game");
    assert_eq!(
        s.viewport(),
        Viewport::new(0, 14),
        "row clamps to 0, col 0x0E stands"
    );
    assert!(!s.scroll(Dir::E));
    assert!(!s.scroll(Dir::N));
    s.toggle_zoom(&mut ctx);
    assert_eq!(s.zoom().id, NEAR.id);
    assert!(!ctx.game.map_zoom_far, "the near zoom did not reach Game");
    assert_eq!(s.viewport(), Viewport::new(60, 30), "back where it was");
    assert!(s.scroll(Dir::E), "and the near view scrolls again");
    assert_eq!(s.viewport(), Viewport::new(60, 31));
}

/// Ablation: make `mode_screen_id` answer `None`
/// goes red.
#[test]
fn picking_up_an_army_is_screen_0x10_and_putting_it_down_is_not() {
    let assets = crate::game::Assets::placeholder();
    let mut game = crate::Game::new(1);
    let unit = game
        .kingdom
        .campaign
        .units
        .spawn(l2_kingdom::unit::Unit::new(l2_kingdom::UnitKind::Army, 1, 10, 10))
        .expect("a free slot");
    let mut s = MapScreen::new();
    assert_eq!(s.mode_screen_id(), None, "the map is 0, which its ScreenId already says");
    let ctx = Ctx { game: &mut game, assets: &assets };
    s.begin_move_selection(&ctx, unit);
    assert_eq!(s.mode_screen_id(), Some(0x10));
    s.cancel_move_selection();
    assert_eq!(s.mode_screen_id(), None);
}
