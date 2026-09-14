#![allow(unused_imports)]
use super::*;
use super::dismissal_and_timing::*;
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

pub(crate) fn alliance_offer(from: u8) -> Record {
    Record {
        to: 1,
        from,
        group: 180,
        variant: 0,
        category: category::ALLIANCE_PROMPT,
        ..Record::default()
    }
}

/// **A player accepts an alliance.** `L2.eng` group 180, *"Accept alliance ?"*,
/// message category `0x0B`, widget table `0x004DDAC0`, handler `FUN_00436872` —
/// and the thumb-up calls `Diplo_FormAlliance(g_localPlayer, offerer)`.
#[test]
fn a_player_can_accept_an_alliance() {
    let (mut g, a, mut m) = world();
    g.kingdom.realms[3].offer_pending = true;
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let [yes, _no] = Prompt::AcceptAlliance.widgets();
    send(&mut m, &mut g, &a, on(yes));

    assert_eq!(g.kingdom.realms[1].ally, 3, "realm 1 is allied to realm 3");
    assert_eq!(g.kingdom.realms[3].ally, 1, "and realm 3 to realm 1");
    assert!(!g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

/// **Declining an AI's offer in single player does nothing at all** — not a
/// refusal, not a grudge, not a letter. The guard is
/// `(offerer is human) || (hotspot != 0)`, and an AI offer declined fails both.
///
/// Asserted,
/// indistinguishable from a rule nobody wired up unless something checks the
/// nothing.
#[test]
fn declining_an_ai_offer_runs_nothing_at_all() {
    let (mut g, a, mut m) = world();
    let before = g.kingdom.realms.clone();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let [_yes, no] = Prompt::AcceptAlliance.widgets();
    send(&mut m, &mut g, &a, on(no));

    assert!(!g.messages.is_open(), "the window closed");
    assert_eq!(g.kingdom.realms[1].ally, 0, "and nobody is allied");
    assert_eq!(g.kingdom.realms[3].ally, 0);
    assert_eq!(
        g.kingdom.realms[1].pair(3),
        before[1].pair(3),
        "no grudge, no standing change -- the offer simply lapses",
    );
}

/// **A right click on an alliance offer is neither yes nor no.** The right
/// branch is tested before the widgets and has no category test in front of it.
#[test]
fn a_right_click_closes_an_offer_without_answering_it() {
    let (mut g, a, mut m) = world();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::RightClick { x: 300, y: 300 });
    assert!(!g.messages.is_open());
    assert_eq!(g.kingdom.realms[1].ally, 0, "closing is not accepting");
}

/// **An offer that arrives when you already have an ally closes itself.**
/// `Msg_DrawWindow`'s category-`0x0B` arm opens with the guard, so the window is
/// and the player never sees it.
#[test]
fn an_offer_lapses_unseen_when_you_already_have_an_ally() {
    let (mut g, a, mut m) = world();
    l2_kingdom::diplomacy::form_alliance(&mut g.kingdom.realms, 1, 2);
    post(&mut g, alliance_offer(3));

    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    assert!(!g.messages.is_open(), "the draw dismissed it");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and it was never on screen to click");
    assert_eq!(g.kingdom.realms[1].ally, 2, "the alliance we had is untouched");
}

/// **A player pays his ally for help.** `L2.eng` group 185, *"Pay -"*, message
/// category `0x0A`, widget table `0x004DDA90`, handler `Diplo_PayHelpClicked`
/// (`0x004367FF`) — the accept button calls `Diplo_PayForHelp(myAlly, me,
/// g_diploHelpCounty, g_diploHelpPrice)`.
#[test]
fn a_player_can_pay_his_ally_for_help() {
    let (mut g, a, mut m) = world();
    l2_kingdom::diplomacy::form_alliance(&mut g.kingdom.realms, 1, 2);
    g.kingdom.counties[4].owner = 3;
    g.kingdom.diplomacy.help_county = 4;
    g.kingdom.diplomacy.help_price = 250;
    let purse = g.kingdom.realms[1].gold;
    let multiple = g.kingdom.realms[2].pair(1).help_price_multiple;

    post(
        &mut g,
        Record { to: 1, from: 2, group: 185, category: category::PAY_PROMPT, county: 4, ..Record::default() },
    );
    open_the_scroll(&mut m, &mut g, &a);

    let [yes, _no] = Prompt::PayForHelp.widgets();
    send(&mut m, &mut g, &a, on(yes));

    assert_eq!(g.kingdom.realms[1].gold, purse - 250, "the gold moved");
    assert_eq!(g.kingdom.realms[2].gold, 2_000 + 250, "into the ally's treasury");
    assert_eq!(
        g.kingdom.realms[2].pair(1).help_price_multiple,
        multiple + 1,
        "and the next favour costs more",
    );
    assert!(!g.messages.is_open());
}

/// Declining the price leaves the treasury alone.
#[test]
fn declining_the_price_costs_nothing() {
    let (mut g, a, mut m) = world();
    l2_kingdom::diplomacy::form_alliance(&mut g.kingdom.realms, 1, 2);
    g.kingdom.diplomacy.help_county = 4;
    g.kingdom.diplomacy.help_price = 250;
    let purse = g.kingdom.realms[1].gold;

    post(
        &mut g,
        Record { to: 1, from: 2, group: 185, category: category::PAY_PROMPT, county: 4, ..Record::default() },
    );
    open_the_scroll(&mut m, &mut g, &a);

    let [_yes, no] = Prompt::PayForHelp.widgets();
    send(&mut m, &mut g, &a, on(no));

    assert_eq!(g.kingdom.realms[1].gold, purse, "not a crown");
    assert!(!g.messages.is_open());
}

/// **A click inside a prompt that is on neither button does not fall through.**
/// `Widget_Test` is asked first and the corner button after it,
/// on the corner box or on nothing — but it never reaches the map underneath
/// while a question is up, because `Msg_DismissUnlessQuestion` refuses it.
#[test]
fn a_miss_inside_a_prompt_does_not_answer_it() {
    let (mut g, a, mut m) = world();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let [yes, _no] = Prompt::AcceptAlliance.widgets();
    // One pixel above the thumb-up: inside the window, on neither button.
    send(&mut m, &mut g, &a, Event::Click { x: yes.0, y: yes.1 - 1 });
    assert!(g.messages.is_open(), "still waiting for an answer");
    assert_eq!(g.kingdom.realms[1].ally, 0);
}

// ------------------------------------------------------------ 3. win a game

