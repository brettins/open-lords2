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

#[test]
fn the_right_button_closes_it_from_anywhere() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert!(!g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

#[test]
fn a_left_click_on_the_map_closes_a_notice_and_selects_nothing() {
    let (mut g, a, mut m) = world();
    g.kingdom.counties[3].owner = 1;
    g.selected = 0;
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::Click { x: 200, y: 200 });
    assert!(!g.messages.is_open(), "the click closed the scroll");
    assert_eq!(g.selected, 0, "and the map did not act on it");
}

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
    assert_eq!(ticks, 400, "2000 down past 0x641, and it is gone");
    assert!(!g.messages.is_open());
}

#[test]
fn walking_into_another_screen_loses_the_message() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);
    assert!(g.messages.is_open());

    m.push(ScreenId::Village(3));
    tick(&mut m, &mut g, &a);
    assert!(!g.messages.is_open(), "the village dismissed it unread");
}



/// `Map_Click` (`0x0043CE1A`) with a message up is
/// `Msg_DismissUnlessQuestion` (`0x00476710`), and that calls **`Msg_Dismiss`**
/// (`0x00476768`) — not the two lines that clear the window. So the click
/// carries `Msg_Dismiss`'s whole tail: `FUN_00476E21`'s restore of
/// `_DAT_004F0350` and `DAT_004F0358 = 0x14`, and `Campaign_EnterConquest`
/// (`0x00497879`) with `g_screenId = 0x1C`. Ours closed the ring alone: the
/// player who clicked his Victory! away on the map was never told he had won
/// and the tip host he was clicking through stayed seated.
#[test]
fn the_win_letter_clicked_away_on_the_map_still_ends_the_game() {
    let (mut g, a, mut m) = world();
    ai_turns_over(&mut g);
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 1;
    }
    for realm in 2..=5u8 {
        dispossess(&mut g, realm, 1);
        g.recount_realm(realm);
    }

    let mut seen: Vec<u16> = Vec::new();
    for _ in 0..24 {
        let Some(group) = g.messages.open().map(|r| r.group) else {
            tick(&mut m, &mut g, &a);
            continue;
        };
        seen.push(group);
        send(&mut m, &mut g, &a, Event::Click { x: 8, y: 470 });
        if g.outcome().is_over() {
            break;
        }
    }

    assert_eq!(seen.last(), Some(&225), "the last letter shown is Victory!: {seen:?}");
    assert_eq!(g.outcome(), Outcome::Won);
    assert_eq!(g.campaign.map, 1, "Campaign_EnterConquest ran on the map's own dismissal");
}
