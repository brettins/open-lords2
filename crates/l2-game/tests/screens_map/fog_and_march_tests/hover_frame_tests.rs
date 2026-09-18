#![allow(unused_imports)]
use super::*;
use super::fog_tests::*;
use super::march_tests::*;
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
use l2_game::screens::map::MapScreen;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

fn ball_is_drawn(canvas: &Canvas, art: &l2_formats::pl8::DecodedFrame, at: (i32, i32)) -> bool {
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

/// **A pointer that never moves after the army is picked still ends in the
/// gold ball.** `Screen_DrawWidgets` (`0x004BA26E`)'s `0x10` arm runs
/// `Map_HoverUnitTarget` (`0x004A8E0B`) once a frame, and its
/// `Path_MarkPreviewTiles()` (`0x004A91BA`) call at `004a0000.c:3726` is
/// *above* the `if (DAT_005691E0 != g_hoverTileOffset)` guard at 3727 — the
/// trail is re-marked with the pointer standing still, and `DAT_005691E0` is
/// written nowhere else in the binary, so nothing resets it when the selection
/// begins. Ours ran the hover only on `Event::Pointer`, so a pointer already
/// resting on the destination drew nothing: *"no red X when I click to tell an
/// army to move"*.
///
/// **Ablation.** Take the `update_hover_path` call out of `MapScreen::draw`
/// and the first assertion fails — no ball is painted on the town, because no
/// `Event::Pointer` ever follows the click. The pre-click assertion is the
/// negative control: the same tile is bare while no army is selected.
#[test]
fn a_still_pointer_over_an_enemy_town_gets_its_gold_ball_on_the_frame_after_the_army_is_picked() {
    use l2_kingdom::map::{coords, flags, index, MAP_TILES};
    use l2_kingdom::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let player = game.player;
    let map = game.kingdom.campaign.map.clone();
    let cost = map.cost_map();
    let plain = |t: usize| {
        map.county[t] != 0
            && map.flags[t] & (flags::IMPASSABLE | flags::CASTLE | flags::PLOT | flags::SETTLEMENT)
                == 0
    };
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
        for (dx, dy) in [(-2i32, 0i32), (2, 0), (0, -2), (0, 2), (-3, 0), (3, 0)] {
            let (sx, sy) = ((x as i32 + dx) as u8, (y as i32 + dy) as u8);
            if !plain(index(sx, sy)) {
                continue;
            }
            let fill =
                l2_kingdom::movement::flood_fill(&cost, (sx, sy), l2_kingdom::movement::Routing::Direct);
            let Some(path) = l2_kingdom::movement::extract_path(&cost, &fill, (x, y)) else {
                continue;
            };
            if path.last() == Some(&(x, y)) && path.len() > 1 {
                chosen = Some(((x, y), (sx, sy)));
                break 'search;
            }
        }
    }
    let (town, stand) = chosen.expect("an enemy county's town a short plain march from somewhere");

    let crowd: Vec<usize> = game
        .kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| {
            (u.x as i32 - town.0 as i32).abs() <= 5 && (u.y as i32 - town.1 as i32).abs() <= 5
        })
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
    game.kingdom.campaign.units.spawn(army).expect("a free unit slot");

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
    let sheet = assets.map.flag_sheet(&campaign::NEAR).expect("Flags1a.pl8");
    let gold = sheet.frame(0x4E).expect("Flags1a.pl8 frame 0x4e");
    let at = |t: (u8, u8)| {
        let (row, col) = campaign::tile_to_cell(t.0 as usize, t.1 as usize);
        let (x, y) = campaign::cell_to_screen(ruler.viewport(), ruler.zoom(), row, col);
        (x + 20, y + 6)
    };

    // The pointer comes to rest on the town first, and never moves again.
    let (hx, hy) = centre(town);
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: hx, y: hy });
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(
        !ball_is_drawn(&canvas, &gold, at(town)),
        "with no army selected the town {town:?} carries no marker"
    );

    // `Map_BeginMoveSelection` (`0x0043723A`), and not one `Event::Pointer`
    // after it.
    let (ax, ay) = centre(stand);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: ax, y: ay });
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(
        ball_is_drawn(&canvas, &gold, at(town)),
        "the next frame paints frame 0x4E on {town:?} at (+0x14, +6) with the pointer never moving"
    );

    // And it stays there: `Path_MarkPreviewTiles` re-marks every frame.
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(ball_is_drawn(&canvas, &gold, at(town)), "and on every frame after it");
}
