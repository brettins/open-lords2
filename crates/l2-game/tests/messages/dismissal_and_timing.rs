#![allow(unused_imports)]
use super::*;
use super::prompts_and_alliances::*;
use super::outcomes_and_obituaries::*;
use super::posting_and_sync::*;
use super::painting_and_layout::*;
use super::battle_drain::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

/// **The scroll appears on the campaign map and the corner button closes it.**
///
/// `Msg_Pump` pulls, `Msg_DrawWindow` lays the window out, `Ui_OkButton` puts
/// its picture in the bottom-right corner, and `Msg_HandleInput` hit-tests a
/// 48 × 48 box round it.
#[test]
fn a_message_appears_on_the_map_and_the_corner_button_closes_it() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(0x92));

    // **Pinned, not computed.** `FUN_004093E0(0x20, 0xA0, 0x1A, 0x0E)` and
    // `Ui_OkButton(x + w - 0x30, y + h - 0x30)` put the corner button at
    // (0x190, 0x150) = (400, 336). Deriving this from `frame_of` would make the
    // test agree with whatever the geometry says, which is the trap
    // `docs/agents.md` records: ablating a constant while computing the probe
    // from that same constant tests nothing at all.
    send(&mut m, &mut g, &a, Event::Click { x: 400, y: 336 });

    assert!(!g.messages.is_open(), "the scroll is gone");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the map is back");
}

/// The **right** button closes it too, from anywhere — the arm the original
/// tests before it tests anything else.
#[test]
fn the_right_button_closes_it_from_anywhere() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert!(!g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

/// **A left click on the map closes a notice and the map does nothing else.**
/// `Map_Click`'s whole body is inside `if (g_messageGroup == 0)`.
#[test]
fn a_left_click_on_the_map_closes_a_notice_and_selects_nothing() {
    let (mut g, a, mut m) = world();
    g.kingdom.counties[3].owner = 1;
    g.selected = 0;
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    // The middle of the viewport: somewhere the map would ordinarily act on.
    send(&mut m, &mut g, &a, Event::Click { x: 200, y: 200 });
    assert!(!g.messages.is_open(), "the click closed the scroll");
    assert_eq!(g.selected, 0, "and the map did not act on it");
}

/// …and **a question survives that click**, which is the half that keeps a
/// stray click from answering a lord.
#[test]
fn a_left_click_on_the_map_leaves_a_question_standing() {
    let (mut g, a, mut m) = world();
    post(
        &mut g,
        Record { to: 1, from: 3, group: 180, category: category::ALLIANCE_PROMPT, ..Record::default() },
    );
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::Click { x: 200, y: 200 });
    assert!(g.messages.is_open(), "an alliance offer is not closed by a click on the map");
    assert_eq!(m.top_id(), Some(ScreenId::Message));
}

/// **The message never times out in single player.** `Msg_Pump` clamps the
/// timer to 1, so the scroll waits — two thousand ticks
/// is more than the whole timer and it is still up.
#[test]
fn in_single_player_the_scroll_waits_for_ever() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    for _ in 0..(message::TIMER_START + 500) {
        tick(&mut m, &mut g, &a);
    }
    assert!(g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Message));
}

/// **…and in a network game it goes on its own after 399 ticks**, which is the
/// same code with `g_multiplayer` set. Without this the timeout branch is a rule
/// with no way in.
#[test]
fn in_a_network_game_the_scroll_closes_itself() {
    let (mut g, a, mut m) = world();
    g.multiplayer = true;
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    let mut ticks = 0;
    while m.top_id() == Some(ScreenId::Message) && ticks < 2_000 {
        tick(&mut m, &mut g, &a);
        ticks += 1;
    }
    // Pinned: 2000 - 0x641 = 399 decrements leave the timer at 0x641, and the
    // 400th puts it below. Written out
    // constants the code under test reads.
    assert_eq!(ticks, 400, "2000 down past 0x641, and it is gone");
    assert!(!g.messages.is_open());
}

/// **Opening another screen throws the message away.** `Msg_Pump`'s else
/// branch,
/// part of the interface.
#[test]
fn walking_into_another_screen_loses_the_message() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);
    assert!(g.messages.is_open());

    // Pushed from outside,
    // screen the interface can only reach through three clicks.
    m.push(ScreenId::Village(3));
    tick(&mut m, &mut g, &a);
    assert!(!g.messages.is_open(), "the village dismissed it unread");
}

// -------------------------------------------------------- 2. answer a lord

