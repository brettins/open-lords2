#![allow(unused_imports)]
use super::*;
use super::geometry::*;
use super::overlays_part::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

/// **A press on the campaign map's own column reaches the map through the
/// stack**, which is the other end of the same claim: passing is only correct if
/// something underneath answers.
///
/// End Turn under an open county panel is the case that matters, because it is
/// the only way a turn can be ended without closing the panel first — and it is
/// where our own BACK TO MAP button used to sit.
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
    // A turn takes many ticks now — `begin_turn` starts it and `tick_turn`
    // winds it on — so *in flight* is the observable effect of the press, and
    // it is the one that says the press arrived.
    assert!(
        l2_game::turn::turn_in_flight(&g),
        "the press reached the map and started nothing; End Turn was not run",
    );
    // **A message suspends the turn, and that is faithful.** The turn is wound
    // by the map screen's own `update`, so once `Msg_Pump` pulls a record and
    // pushes `ScreenId::Message` the map stops ticking until the scroll is
    // dismissed — which is how the original shows obituaries one click at a
    // time. Before the message window existed there was nothing to dismiss and
    // this loop needed no input; now it does, so it dismisses like a player.
    // The right button is the arm that closes any scroll with no rectangle and
    // no category test in front of it.
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

