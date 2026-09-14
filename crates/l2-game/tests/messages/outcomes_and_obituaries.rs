#![allow(unused_imports)]
use super::*;
use super::dismissal_and_timing::*;
use super::prompts_and_alliances::*;
use super::posting_and_sync::*;
use super::painting_and_layout::*;
use super::battle_drain::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

/// **A player is told he has won,
///
/// The chain, end to end and with nothing skipped: realm 3 is recounted at zero
/// strength, `Realm_RecountStrength` enqueues group 194; `Score_RankRealms`
/// leaves `opponents_remaining` at 0; `Msg_Pump` shows 194; `Msg_DrawWindow`'s
/// category-`0x0E` arm sees no opponents left and **enqueues group 225 onto the
/// ring it is being drawn from**; the player dismisses 194; `Msg_Pump` shows
/// 225; `Msg_DrawWindow` sets `DAT_0053F0C4 = 10`; the player dismisses 225 and
/// `Msg_Dismiss` enters screen `0x1C`.
///
/// Every dismissal below is a click.
#[test]
fn a_player_is_told_he_has_won_and_dismissing_it_wins_the_game() {
    let (mut g, a, mut m) = world();
    // The human holds everything; the other four hold nothing and are about to
    // be counted out.
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 1;
    }
    for realm in 2..=5u8 {
        dispossess(&mut g, realm, 1);
        g.recount_realm(realm);
    }
    assert_eq!(g.campaign.ranking.opponents_remaining, 0, "nobody is left to fight");
    assert_eq!(g.outcome(), Outcome::InPlay, "and nothing has been decided yet");

    // Four obituaries and, behind them, a victory that does not exist yet.
    let mut seen: Vec<u16> = Vec::new();
    for _ in 0..12 {
        if m.top_id() != Some(ScreenId::Message) {
            tick(&mut m, &mut g, &a);
            continue;
        }
        let group = g.messages.open().expect("a record").group;
        seen.push(group);
        let frame = message::frame_of(g.messages.open().unwrap()).expect("a frame");
        let (x, y) = frame.ok_button();
        send(&mut m, &mut g, &a, Event::Click { x, y });
        if m.top_id() == Some(ScreenId::Conquest) {
            break;
        }
    }

    assert!(seen.contains(&194), "an AI's obituary was shown: {seen:?}");
    assert_eq!(seen.last(), Some(&225), "and the last thing shown is Victory!: {seen:?}");
    assert_eq!(g.outcome(), Outcome::Won);
    assert_eq!(g.outcome().value(), 10, "DAT_0053F0C4");
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "screen 0x1C");
    assert_eq!(g.campaign.map, 1, "and the campaign stepped on");
}

/// **`docs/plan.md`'s mainline win, on its own**, with `Score_RankRealms` kept
/// out of it.
///
/// The test above wins a real position, and it turns out **two** paths put group
/// 225 in the ring there: this one, and `rank_and_crown` crowning the sole
/// survivor. Ablating the enqueue inside the draw left that test green — which
/// is exactly `docs/agents.md`'s *"a check that passes for an accidental reason
/// looks exactly like one that passes"*, and it is why this test exists.
///
/// So: no recount, no ranking pass, nothing but a ranking that says there is
/// nobody left and one obituary in the ring. Displaying it has to produce the
/// victory, or the arm is not built.
#[test]
fn displaying_an_ending_with_nobody_left_is_what_enqueues_the_victory() {
    let (mut g, a, mut m) = world();
    // Late in the player's turn: phase 4 is open, every AI realm has finished
    // its steps and the map's frames run no `Realm_RecountStrength` — so the
    // ranking below is the one the draw reads. Without this the AI frames'
    // step-0 prologue ranks the realms again and hands back the four opponents
    // this test is keeping out of it.
    ai_turns_over(&mut g);
    g.campaign.ranking =
        l2_kingdom::victory::Ranking { opponents_remaining: 0, ..Default::default() };
    post(
        &mut g,
        Record { to: 0, from: 3, group: 194, category: category::ENDING, ..Record::default() },
    );

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(194));
    assert_eq!(
        g.messages.waiting().iter().map(|r| r.group).collect::<Vec<_>>(),
        vec![225],
        "displaying 194 pushed 225 onto the ring it was drawn from",
    );
    assert_eq!(g.outcome(), Outcome::InPlay, "and 194 itself decided nothing");

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(225), "Victory!");
    assert_eq!(g.outcome(), Outcome::Won, "displayed is what writes 10");

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "and dismissed is what enters 0x1C");
}

/// **The mirror: the player's own defeat.** Group 224 with `from ==
/// g_localPlayer` is `DAT_0053F0C4 = 11`, and dismissing it enters the same
/// screen with the other branch.
#[test]
fn a_player_is_told_he_has_lost_and_dismissing_it_loses_the_game() {
    let (mut g, a, mut m) = world();
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 2;
    }
    dispossess(&mut g, 1, 2);
    g.recount_realm(1);
    assert!(g.campaign.ranking.opponents_remaining > 0, "or it would be scored a win");

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(224), "Defeat!");
    assert_eq!(g.outcome(), Outcome::Lost, "displayed is what writes the outcome");
    assert_eq!(m.top_id(), Some(ScreenId::Message), "and it is still on screen");

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "dismissed is what acts on it");
    assert_eq!(g.campaign.map, 0, "a loss replays the map");
}

/// **An AI dying while others live ends nothing**
/// told. Without this the two tests above would pass on a chain that declared a
/// winner every time anybody died.
#[test]
fn an_ai_dying_while_others_live_is_a_message_and_not_an_ending() {
    let (mut g, a, mut m) = world();
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = if id <= 3 { 1 } else { 2 };
    }
    dispossess(&mut g, 3, 2);
    g.recount_realm(3);

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(194), "Foiled again.");
    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });

    assert_eq!(g.outcome(), Outcome::InPlay);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the game carries on");
}

/// **The lord-flavoured variant is carried.** `Realm_RecountStrength` enqueues
/// group 194 with `variant = lord * 4 + rotation - 4`, and group 194 holds
/// seventeen strings — a label and sixteen laments, four per lord. A message
/// with no variant would draw the Knight's first line for every lord in the
/// game.
#[test]
fn an_ai_obituary_carries_the_lords_own_line() {
    let (mut g, _a, _m) = world();
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 1;
    }
    dispossess(&mut g, 4, 1);
    g.kingdom.realms[4].lord = 3;
    g.kingdom.realms[4].voice_rotation = 2;
    g.recount_realm(4);

    let r = g.messages.waiting().into_iter().find(|r| r.group == 194).expect("an obituary");
    assert_eq!(r.variant, 3 * 4 + 2 - 4, "lord * 4 + rotation - 4");
    assert_eq!(r.body_index(), 11, "and the body is variant + 1, past the label");
}

// --------------------------------------------- 3b. meet a county's own event

