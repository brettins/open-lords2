#![allow(unused_imports)]
use super::*;
use super::geometry::*;
use super::overlays_part::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

#[test]
fn end_turn_works_through_an_open_county_panel() {
    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.handle(Event::Click { x: 0, y: 0 }, &mut ctx); // build the map's planes
    }
    m.push(ScreenId::County(1, county::Panel::Tax));
    assert_eq!(m.top_id(), Some(ScreenId::County(1, county::Panel::Tax)));

    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.handle(
            Event::Click {
                x: map::END_TURN_BUTTON.centre_x(),
                y: map::END_TURN_BUTTON.y + map::END_TURN_BUTTON.h / 2,
            },
            &mut ctx,
        );
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the panel did not come down");
    assert!(
        l2_game::turn::turn_in_flight(&g),
        "the press reached the map and started nothing; End Turn was not run",
    );
    let mut dismissed = 0;
    for _ in 0..400 {
        if !l2_game::turn::turn_in_flight(&g) {
            break;
        }
        let mut ctx = Ctx { game: &mut g, assets: &a };
        if m.top_id() == Some(ScreenId::Message) {
            m.handle(Event::RightClick { x: 320, y: 240 }, &mut ctx);
            dismissed += 1;
            continue;
        }
        m.update(&mut ctx);
    }
    assert_eq!(
        g.turns_played, 1,
        "and the turn did not finish after dismissing {dismissed} message(s); top is {:?}",
        m.top_id(),
    );
}

