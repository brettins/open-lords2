#![allow(unused_imports)]
use super::*;
use super::orders::*;
use super::super::*;
use super::super::fog_tests::*;
use super::super::*;
use super::super::view_tests::*;
use super::super::interaction_tests::*;
use super::super::structures_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

/// * **Tick 1 commits the tile.** `Unit_Spawn` leaves the latch set, so the
///   first tick enters `(x + 1, y)`, turns the unit east and writes `+0x149 =
///   1`. The figure is drawn **at the new tile, dragged back by index 1** —
///   `(−28, −14)` near, `(−5, −2)` far — which stands it two pixels right and
///   one down of where it stood at rest on the tile it left. **And still facing
///   north**, frame 75: `Army_Tick` wrote `+0x07` before `Unit_Step` turned it.
///
/// * **Tick 33 reaches the tile's edge**, `+0x149` goes back to 0 and, the path
///   being empty, the unit stops — standing exactly on its tile — and tick 34 is
///   the standing frame, 81.
///
/// **What makes each assertion the claim.** The figure is found by its own ink —
/// every opaque palette index of the frame, at one exact position — among all
/// four frames it could be, so a wrong frame at the right place and the right
/// frame at the wrong place both fail. Both tiles are asserted seen with the fog
/// **on**, and the whole figure inside the map viewport, on every tick: C138 was
/// Assertion about an army not on screen.
#[test]
fn a_marching_army_is_drawn_part_way_across_its_tile_with_the_originals_walk_frames() {
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let player = game.player;
    let ((fx, fy), (tx, ty)) = {
        let g = &game;
        open_step_east(g, |from, to| {
            let map = &g.kingdom.campaign.map;
            g.is_players(map.county[from])
                && map.county[from] == map.county[to]
                && !g.hides_tile(from)
                && !g.hides_tile(to)
        })
        .expect("the person's county has two open, seen tiles side by side")
    };
    let mut army = Unit::new(UnitKind::Army, player, fx, fy);
    army.men = 100; // under 301: the first bank, 0x48
    army.troops[0] = 100;
    army.county = game.kingdom.campaign.map.county_at(fx, fy);
    army.owner_is_human = true;
    army.facing = 0; // north, so the first commit is also a turn
    let id = game.kingdom.campaign.units.spawn(army).expect("a free unit slot");
    assert_eq!(game.order_unit_move(id, (tx, ty)), Some(1), "one step east");

    let mut near = MapScreen::new();
    draw(&mut near, &mut game, &assets);
    near.centre_on_tile(tx as usize, ty as usize);
    let mut far = MapScreen::new();
    draw(&mut far, &mut game, &assets);
    send(&mut far, &mut game, &assets, l2_game::screens::map::MINIMAP_MODE_BUTTONS[3].click());
    assert_eq!(far.zoom().id, campaign::FAR.id, "the second screen is at the far zoom");
    for (screen, what) in [(&near, "near"), (&far, "far")] {
        assert!(
            campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize).is_some(),
            "{what}: the tile walked into is in view"
        );
    }
    let frame = |z: &campaign::Zoom, n: usize| {
        assets
            .map
            .sprite_sheet(z, 0)
            .and_then(|s| s.frame(n))
            .unwrap_or_else(|| panic!("zoom {}: sprite sheet A frame {n}", z.id))
    };

    const NORTH: usize = 75; // facing 0, phase 0
    const STAND: usize = 81; // facing 2, walk frame 0
    const MID: usize = 82; //   facing 2, walk frame 1
    const STRIDE: usize = 83; // facing 2, walk frame 2
    // (tick, +0x149 after it, near offset, far offset, frame drawn)
    #[rustfmt::skip]
    let march: [(u32, u8, (i32, i32), (i32, i32), usize); 34] = [
        (1, 1, (-28, -14), (-5, -2), NORTH), (2, 1, (-28, -14), (-5, -2), STAND),
        (3, 1, (-28, -14), (-5, -2), STAND), (4, 1, (-28, -14), (-5, -2), STAND),
        (5, 3, (-24, -12), (-5, -2), STAND), (6, 3, (-24, -12), (-5, -2), MID),
        (7, 3, (-24, -12), (-5, -2), MID), (8, 3, (-24, -12), (-5, -2), MID),
        (9, 5, (-20, -10), (-4, -2), MID), (10, 5, (-20, -10), (-4, -2), STRIDE),
        (11, 5, (-20, -10), (-4, -2), STRIDE), (12, 5, (-20, -10), (-4, -2), STRIDE),
        (13, 7, (-16, -8), (-3, -1), STRIDE), (14, 7, (-16, -8), (-3, -1), MID),
        (15, 7, (-16, -8), (-3, -1), MID), (16, 7, (-16, -8), (-3, -1), MID),
        (17, 9, (-12, -6), (-2, -1), MID), (18, 9, (-12, -6), (-2, -1), STAND),
        (19, 9, (-12, -6), (-2, -1), STAND), (20, 9, (-12, -6), (-2, -1), STAND),
        (21, 11, (-8, -4), (-1, 0), STAND), (22, 11, (-8, -4), (-1, 0), MID),
        (23, 11, (-8, -4), (-1, 0), MID), (24, 11, (-8, -4), (-1, 0), MID),
        (25, 13, (-4, -2), (-1, 0), MID), (26, 13, (-4, -2), (-1, 0), STRIDE),
        (27, 13, (-4, -2), (-1, 0), STRIDE), (28, 13, (-4, -2), (-1, 0), STRIDE),
        (29, 15, (0, 0), (0, 0), STRIDE), (30, 15, (0, 0), (0, 0), MID),
        (31, 15, (0, 0), (0, 0), MID), (32, 15, (0, 0), (0, 0), MID),
        (33, 0, (0, 0), (0, 0), MID), (34, 0, (0, 0), (0, 0), STAND),
    ];
    let candidates = [NORTH, STAND, MID, STRIDE];

    for &(tick, step, near_walk, far_walk, drawn) in &march {
        l2_game::turn::tick_units_only(&mut game);
        let u = game.kingdom.campaign.units.get(id).expect("the army");
        assert_eq!((u.tile(), u.sub_tile), ((tx, ty), step), "tick {tick}: where the simulation has it");
        assert_eq!(u.moving, tick < 33, "tick {tick}: walking until the last tile is crossed, and no longer");
        assert!(!game.hides_tile(l2_kingdom::map::index(tx, ty)), "tick {tick}: the tile is seen");

        let canvas = draw(&mut near, &mut game, &assets);
        let place = |n: usize| {
            let art = frame(&campaign::NEAR, n);
            let (rx, ry) = at_rest(&near, (tx, ty), (0, -4), &art);
            ((rx + near_walk.0, ry + near_walk.1), art)
        };
        let found: Vec<usize> =
            candidates.iter().copied().filter(|&n| { let (at, art) = place(n); ink_at(&canvas, &art, at) }).collect();
        assert_eq!(found, vec![drawn], "tick {tick}, +0x149 = {step}: the near figure, {near_walk:?} from rest");
        let ((ox, oy), art) = place(drawn);
        let clip = near.map_clip();
        assert!(
            clip.contains(ox, oy) && clip.contains(ox + art.width as i32 - 1, oy + art.height as i32 - 1),
            "tick {tick}: the whole figure is inside the map viewport"
        );
        if tick == 1 {
            let (bx, by) = at_rest(&near, (fx, fy), (0, -4), &art);
            assert_eq!(
                (ox, oy),
                (bx + 2, by + 1),
                "the first sub-step stands two right and one down of rest on the tile it left: 28 and 14 against 30 and 15"
            );
        }

        let canvas = draw(&mut far, &mut game, &assets);
        let art = frame(&campaign::FAR, drawn);
        let (rx, ry) = at_rest(&far, (tx, ty), (0, -4), &art);
        assert!(
            ink_at(&canvas, &art, (rx + far_walk.0, ry + far_walk.1)),
            "tick {tick}, +0x149 = {step}: the far figure, {far_walk:?} from rest"
        );
    }
}

/// `Unit_MoveInFacing` (`0x00466D84`) unlinks the unit from the tile it is
/// leaving and links it to the next one **at the commit**, before a single
/// sub-step of the crossing is drawn, and `Map_DrawArmies` is reached from the
/// render pass of the tile the unit is linked to, behind that tile's seen bit.
#[test]
fn a_unit_walking_into_the_dark_is_hidden_for_its_whole_crossing_and_one_walking_out_is_not() {
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    game.kingdom.options.exploration = true;
    let (lit, dark) = {
        let g = &game;
        open_step_east(g, |from, to| !g.hides_tile(from) && g.hides_tile(to))
            .expect("a seen tile with an unseen one east of it")
    };

    for (from, to, shown) in [(lit, dark, false), (dark, lit, true)] {
        let mut g = game.clone();
        let mut trader = Unit::new(UnitKind::Merchant, l2_kingdom::units_tick::OWNERLESS, from.0, from.1);
        trader.county = g.kingdom.campaign.map.county_at(from.0, from.1);
        let id = g.kingdom.campaign.units.spawn(trader).expect("a free unit slot");
        let map = g.kingdom.campaign.map.clone();
        l2_kingdom::movement::order_move(&map, &mut g.kingdom.campaign.units, id, to, l2_kingdom::movement::Routing::Direct)
            .expect("one step");

        let mut last = None;
        let mut sub_steps = 0;
        for tick in 1..=40 {
            l2_game::turn::tick_units_only(&mut g);
            let u = g.kingdom.campaign.units.get(id).expect("the merchant");
            if !u.moving {
                break;
            }
            assert_eq!(u.tile(), to, "tick {tick}: committed onto the tile it is walking into");
            if last == Some(u.sub_tile) {
                continue;
            }
            last = Some(u.sub_tile);
            sub_steps += 1;
            let step = u.sub_tile;

            let mut seen = [0usize; 2];
            for (i, light) in [false, true].into_iter().enumerate() {
                let mut h = g.clone();
                if light {
                    h.kingdom.campaign.explored.reveal_square(h.player, to.0 as i32, to.1 as i32, 0);
                }
                if !shown && !light {
                    assert!(h.hides_tile(l2_kingdom::map::index(to.0, to.1)), "the tile walked into is dark");
                }
                let (screen, with) = paint_at(&mut h, &assets, to.0, to.1);
                h.kingdom.campaign.units.remove(id).expect("the merchant");
                let (_, without) = paint_at(&mut h, &assets, to.0, to.1);
                seen[i] = map_pixels_differ(&with, &without, screen.map_clip());
            }
            if shown {
                assert!(seen[0] > 0, "+0x149 = {step}: walking out of the dark into sight, it is drawn over the dark");
            } else {
                assert_eq!(seen[0], 0, "+0x149 = {step}: walking into the dark, not one pixel of it is drawn");
                assert!(seen[1] > 0, "+0x149 = {step}, the control: with that tile seen, the same merchant is on the map");
            }
        }
        assert_eq!(sub_steps, 8, "a crossing is eight sub-steps: +0x149 = 1, 3 … 15");
    }
}

