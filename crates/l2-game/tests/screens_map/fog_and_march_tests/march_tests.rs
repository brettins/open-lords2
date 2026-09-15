#![allow(unused_imports)]
use super::*;
use super::fog_tests::*;
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::structures_tests::*;
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

// Every expected number below is a literal out of `Lords2.exe` — the walk
// tables at `0x004D8108`/`0x004D8188` and `0x004D8308`/`0x004D8388`,
// `g_unitWalkFrames`, `Map_DrawPathMarker`'s `(0x14, 6)` and `0x4E` — and none
// is computed from the constant it is checking.

fn ink_at(canvas: &Canvas, art: &l2_formats::pl8::DecodedFrame, at: (i32, i32)) -> bool {
    let (w, h) = (art.width as i32, art.height as i32);
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if !art.opaque[i] {
                continue;
            }
            let (px, py) = (at.0 + x, at.1 + y);
            if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                return false;
            }
            if canvas.at(px as usize, py as usize) != art.indices[i] {
                return false;
            }
            any = true;
        }
    }
    any
}

fn at_rest(screen: &MapScreen, tile: (u8, u8), nudge: (i32, i32), art: &l2_formats::pl8::DecodedFrame) -> (i32, i32) {
    let (row, col) = campaign::tile_to_cell(tile.0 as usize, tile.1 as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let z = screen.zoom();
    (sx + z.half_pitch + nudge.0 - art.width as i32 / 2, sy + z.half_pitch + nudge.1 - art.height as i32)
}

fn open_step_east(game: &Game, ok: impl Fn(usize, usize) -> bool) -> Option<((u8, u8), (u8, u8))> {
    use l2_kingdom::map::{flags, index};
    let map = &game.kingdom.campaign.map;
    let busy = flags::IMPASSABLE | flags::ROAD | flags::PLOT | flags::FARMLAND | flags::CASTLE | flags::SETTLEMENT;
    let open = |x: u8, y: u8| {
        let t = index(x, y);
        map.county[t] != 0 && map.flags[t] & busy == 0
    };
    for y in 16..48u8 {
        for x in 16..47u8 {
            if !open(x, y) || !open(x + 1, y) || !ok(index(x, y), index(x + 1, y)) {
                continue;
            }
            let crowded = game.kingdom.campaign.units.iter().any(|(_, u)| {
                (u.y as i32 - y as i32).abs() <= 3 && (u.x as i32 - x as i32).abs() <= 4
            });
            if !crowded {
                return Some(((x, y), (x + 1, y)));
            }
        }
    }
    None
}

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
    send(&mut far, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
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

/// `Map_DrawPathMarker` (`0x004081A6`):
#[test]
fn a_march_hovered_onto_an_enemy_town_ends_in_the_gold_ball_and_only_steps_out_of_reach_are_grey() {
    use l2_kingdom::map::{coords, flags, index, MAP_TILES};
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let player = game.player;
    let map = game.kingdom.campaign.map.clone();
    let plain = |t: usize| {
        map.county[t] != 0
            && map.flags[t] & (flags::IMPASSABLE | flags::CASTLE | flags::PLOT | flags::SETTLEMENT) == 0
    };
    let cost = map.cost_map();
    let mut chosen = None;
    'search: for t in 0..MAP_TILES {
        let (x, y) = coords(t);
        if !(8..56).contains(&x) || !(8..56).contains(&y) || map.flags[t] & flags::CASTLE == 0 {
            continue;
        }
        let owner = game.kingdom.counties.get(map.county[t] as usize).map_or(0, |c| c.owner);
        if owner == 0 || owner == player {
            continue;
        }
        #[rustfmt::skip]
        let offsets = [(-3i32, 0i32), (-3, 3), (0, 3), (3, 3), (3, 0), (3, -3), (0, -3), (-3, -3),
                       (-2, 0), (-2, 2), (0, 2), (2, 2), (2, 0), (2, -2), (0, -2), (-2, -2)];
        for (dx, dy) in offsets {
            let (sx, sy) = ((x as i32 + dx) as u8, (y as i32 + dy) as u8);
            if !plain(index(sx, sy)) {
                continue;
            }
            let fill = l2_kingdom::movement::flood_fill(&cost, (sx, sy), l2_kingdom::movement::Routing::Direct);
            let Some(path) = l2_kingdom::movement::extract_path(&cost, &fill, (x, y)) else { continue };
            let steps: Vec<((u8, u8), i32)> = path
                .iter()
                .filter(|&&p| p != (x, y))
                .map(|&p| (p, fill.cost_to(p.0, p.1).unwrap_or(99)))
                .collect();
            if path.last() == Some(&(x, y))
                && !steps.is_empty()
                && steps.iter().all(|&(p, c)| plain(index(p.0, p.1)) && (1..=15).contains(&c))
            {
                chosen = Some(((x, y), (sx, sy), steps));
                break 'search;
            }
        }
    }
    let (town, stand, steps) = chosen.expect("an enemy county's town a short plain march from somewhere");

    let crowd: Vec<usize> = game
        .kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| (u.x as i32 - town.0 as i32).abs() <= 5 && (u.y as i32 - town.1 as i32).abs() <= 5)
        .map(|(id, _)| id)
        .collect();
    for id in crowd {
        game.kingdom.campaign.units.remove(id);
    }
    let mut army = Unit::new(UnitKind::Army, player, stand.0, stand.1);
    army.men = 300;
    army.troops[0] = 300;
    army.county = map.county_at(stand.0, stand.1);
    army.owner_is_human = true;
    let army = game.kingdom.campaign.units.spawn(army).expect("a free unit slot");

    let mut m = Machine::new(ScreenId::Campaign);
    let mut ruler = MapScreen::new();
    draw_stack(&mut m, &mut game, &assets);
    draw(&mut ruler, &mut game, &assets);
    let want = campaign::Viewport::centred_on_tile(town.0 as usize, town.1 as usize, ruler.zoom());
    for _ in 0..200 {
        let v = ruler.viewport();
        let key = if v.col < want.col {
            Key::Right
        } else if v.col > want.col {
            Key::Left
        } else if v.row + 1 < want.row {
            Key::Down
        } else if v.row > want.row + 1 {
            Key::Up
        } else {
            break;
        };
        send_stack(&mut m, &mut game, &assets, Event::KeyDown(key));
        send(&mut ruler, &mut game, &assets, Event::KeyDown(key));
        if ruler.viewport() == v {
            break;
        }
    }
    let centre = |t: (u8, u8)| {
        campaign::tile_centre(ruler.viewport(), ruler.zoom(), t.0 as usize, t.1 as usize)
            .unwrap_or_else(|| panic!("{t:?} is in view"))
    };
    let (ax, ay) = centre(stand);
    let (hx, hy) = centre(town);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: ax, y: ay });
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: hx, y: hy });

    let sheet = assets.map.flag_sheet(&campaign::NEAR).expect("Flags1a.pl8");
    let ball = |n: usize| sheet.frame(n).unwrap_or_else(|| panic!("Flags1a.pl8 frame {n:#x}"));
    let at = |t: (u8, u8)| {
        let (row, col) = campaign::tile_to_cell(t.0 as usize, t.1 as usize);
        let (x, y) = campaign::cell_to_screen(ruler.viewport(), ruler.zoom(), row, col);
        (x + 20, y + 6)
    };
    const GREY: usize = 0x38;
    const GOLD: usize = 0x4E;

    let figure = assets.map.sprite_sheet(&campaign::NEAR, 0).and_then(|s| s.frame(0x48)).expect("an army frame");
    let (fx0, fy0) = at_rest(&ruler, stand, (0, -4), &figure);
    let clear = |t: (u8, u8)| {
        let ((bx, by), b) = (at(t), ball(GREY));
        bx + b.width as i32 <= fx0
            || bx >= fx0 + figure.width as i32
            || by + b.height as i32 <= fy0
            || by >= fy0 + figure.height as i32
    };
    let visible: Vec<((u8, u8), i32)> = steps.iter().copied().filter(|&(t, _)| clear(t)).collect();
    assert!(!visible.is_empty(), "at least one step's ball is clear of the army's figure: {steps:?}");

    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(ink_at(&canvas, &ball(GOLD), at(town)), "the enemy town {town:?} ends the route in frame 0x4E, at (+0x14, +6)");
    assert!(!ink_at(&canvas, &ball(GREY), at(town)), "and not in the grey ball");
    for &(t, c) in &visible {
        assert!(ink_at(&canvas, &ball(GREY + c as usize), at(t)), "the step at {t:?}, cost {c} of 15, is frame 0x38 + {c}");
        assert!(!ink_at(&canvas, &ball(GREY), at(t)), "and not grey: {t:?} is in reach");
    }

    game.kingdom.campaign.units.get_mut(army).expect("the army").moves_used = 15;
    let canvas = draw_stack(&mut m, &mut game, &assets);
    for &(t, c) in &visible {
        assert!(ink_at(&canvas, &ball(GREY), at(t)), "with no moves left the step at {t:?}, cost {c}, is grey");
    }
    assert!(
        ink_at(&canvas, &ball(GOLD), at(town)),
        "and the town is still gold — `local_14` is tested before the cost is looked at"
    );
}

/// **`FUN_0043CAF4`'s extra step: the right button also selects a county.**
///
/// ```c
/// if ((g_pickedTileCounty != 0) && (g_pickedTileCounty != g_selectedCounty) &&
///     (g_pickedTileUnit == 0)) {
///   DAT_0053f0dc = g_counties[g_pickedTileCounty].townTile;
///   if (DAT_0053f0dc != 0) {
///     g_selectedCounty = g_pickedTileCounty;
///     Map_CentreOnTile(DAT_0053f0dc);
///   }
///   DAT_004eb260 = 1; FUN_004050c0();
/// }
/// FUN_0041b032();
/// ```
#[test]
fn right_clicking_another_countys_ground_selects_it_and_a_unit_on_it_does_not() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    let here = game.selected;
    assert!(here != 0, "setup: the game opens with a county selected");

    let empty_tile = |game: &Game, id: u8| {
        let map = &game.kingdom.campaign.map;
        (0..map.county.len()).find(|&t| {
            let (x, y) = l2_kingdom::map::coords(t);
            map.county[t] == id && game.kingdom.campaign.units.at(x, y).is_none()
        })
    };
    let (there, tile, town) = (1..=game.kingdom.county_count as u8)
        .filter(|&id| id != here)
        .find_map(|id| {
            let ctx = Ctx { game: &mut game, assets: &assets };
            let town = *MapScreen::town(&ctx, id).first()?;
            Some((id, empty_tile(&game, id)?, town))
        })
        .expect("some other county has a town and a tile with nobody on it");

    let (x, y) = on_screen(&mut screen, tile);
    let opened = send(&mut screen, &mut game, &assets, Event::RightClick { x, y });
    assert_eq!(
        opened,
        Transition::Push(ScreenId::Info(l2_game::screens::info::Target::Tile(tile))),
        "the right click still opens the information panel on the tile"
    );
    assert_eq!(game.selected, there, "and it selected the county the tile belongs to");
    let (tx, ty) = l2_kingdom::map::coords(town);
    assert!(
        campaign::tile_centre(screen.viewport(), screen.zoom(), tx as usize, ty as usize).is_some(),
        "the view moved to county {there}'s town square"
    );

    let (elsewhere, ground) = (1..=game.kingdom.county_count as u8)
        .filter(|&id| id != there)
        .find_map(|id| Some((id, empty_tile(&game, id)?)))
        .expect("a third county with an empty tile");
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");
    let (gx, gy) = l2_kingdom::map::coords(ground);
    {
        let u = game.kingdom.campaign.units.get_mut(unit).expect("the merchant");
        u.x = gx;
        u.y = gy;
        u.county = elsewhere;
    }
    let (x, y) = on_screen(&mut screen, ground);
    let opened = send(&mut screen, &mut game, &assets, Event::RightClick { x, y });
    assert_eq!(
        opened,
        Transition::Push(ScreenId::Info(l2_game::screens::info::Target::Unit(unit))),
        "the right click opens the unit half"
    );
    assert_eq!(game.selected, there, "and a unit under the cursor selects nothing");
}


