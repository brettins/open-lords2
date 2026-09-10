//! **A game that can be won and a game that can be lost.**
//!
//! Until this file existed a game of Lords of the Realm II in this workspace ran
//! for ever: `Realm::is_eliminated` and `Realm::in_play` were both implemented
//! and tested, and nothing read either one as an ending.
//!
//! Each test here drives a whole turn through the real phase machine — no
//! reaching into the ending chain and calling it directly — and asserts on the
//! two things the original ends a game with: the outcome byte `DAT_0053F0C4`,
//! and which of screen `0x1C`'s three sentences the campaign asks for.
//!
//! The positions are constructed. Forty turns of a real game is not a test.

use l2_game::screen::{Ctx, Screen, ScreenId};
use l2_game::screens::conquest::{ConquestScreen, Outcome as Branch};
use l2_game::victory::{ConquestBranch, Track};
use l2_game::{turn, Game};
use l2_kingdom::victory::Outcome;

/// Five realms on a fourteen-county map, realm 1 human. Nobody owns anything
/// yet; each test hands out the counties itself, because *who holds what* is the
/// whole input to the ending chain.
fn five_realms() -> Game {
    let mut g = Game::new(0x51EED);
    g.player = 1;
    g.kingdom.set_county_count(14);
    for id in 1..=14 {
        let c = &mut g.kingdom.counties[id];
        c.population = 417;
        c.pop_last = 417;
        c.happiness = 65;
        c.happiness_last = 65;
        c.health_meter = 65;
        c.health_band = l2_kingdom::tables::health_band(65);
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;
    }
    for realm in 1..=5usize {
        g.kingdom.realms[realm].in_play = true;
        // Non-zero, so that the recount's `strength != 0` guard lets each realm
        // be looked at once. This is the state a realm is in the moment before
        // it loses its last county.
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].gold = 1000;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 0;
    g
}

fn give(g: &mut Game, counties: std::ops::RangeInclusive<usize>, realm: u8) {
    for id in counties {
        g.kingdom.counties[id].owner = realm;
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
}

// ---------------------------------------------------------------------------
// Won
// ---------------------------------------------------------------------------

/// **The whole map is the player's, and one turn ends the game.**
///
/// Four AI realms are recounted at zero and raise group 194 apiece; the last of
/// those leaves one realm standing, which is the human, so `Score_RankRealms`
/// crowns it and enqueues group 225. Showing 225 is `DAT_0053F0C4 = 10`.
#[test]
fn holding_every_county_wins_the_game() {
    let mut g = five_realms();
    give(&mut g, 1..=14, 1);

    let outcome = turn::end_turn(&mut g).expect("the machine comes round");

    assert_eq!(outcome.outcome, Outcome::Won);
    assert_eq!(outcome.outcome.value(), 10, "DAT_0053F0C4");
    assert_eq!(g.outcome(), Outcome::Won);
    for realm in 2..=5 {
        assert!(!g.kingdom.realms[realm].in_play, "realm {realm} is out");
        assert_eq!(g.kingdom.realms[realm].strength, 0);
    }
    assert!(g.kingdom.realms[1].in_play, "and the player is not");
    assert_eq!(g.campaign.ranking.opponents_remaining, 0);
    assert!(g.kingdom.realms[1].crowned_once, "realm +0xED, the one-shot guard");
}

/// The win advances the campaign and names the next country.
#[test]
fn a_win_steps_the_campaign_on_to_the_next_map() {
    let mut g = five_realms();
    give(&mut g, 1..=14, 1);
    assert_eq!(g.campaign.map, 0);
    assert_eq!(g.campaign.current().map(|m| m.scenario), Some(17), "Quaintville");

    turn::end_turn(&mut g).expect("the machine comes round");
    // **The step into 0x1C is `Msg_Dismiss`'s, not the turn's.** `crate::message::drain`
    // is the headless stand-in for the frame loop and it dismisses the victory
    // message, which is where `Campaign_EnterConquest` runs -- so the counter has
    // already moved by the time `end_turn` returns and this asks for the BRANCH
    // rather than stepping again. Calling `enter_conquest_screen` here a second
    // time is what turned this test red, correctly.
    assert_eq!(g.campaign.branch(), ConquestBranch::Won);
    assert_eq!(g.campaign.map, 1);
    assert_eq!(g.campaign.current().map(|m| m.scenario), Some(12), "Rose");
    // `FUN_00499E5D` takes the difficulty and the purse from the same row.
    assert_eq!(g.campaign.current().map(|m| (m.difficulty, m.gold)), Some((0, 2500)));
}

/// The eighth win is the end of the campaign, not the end of a map.
#[test]
fn the_last_win_of_a_campaign_asks_for_the_long_branch() {
    let mut g = five_realms();
    give(&mut g, 1..=14, 1);
    g.campaign.map = 7;

    turn::end_turn(&mut g).expect("the machine comes round");
    assert_eq!(g.outcome(), Outcome::Won);
    assert_eq!(g.campaign.branch(), ConquestBranch::Finished);
    assert!(g.campaign.is_complete());
}

// ---------------------------------------------------------------------------
// Lost
// ---------------------------------------------------------------------------

/// **The player holds nothing, and one turn ends the game.**
///
/// The human is recounted at zero at the top of phase 4 — `AI_RunTurnStep` runs
/// step 0 for every realm, the human included — and raises group 224 with
/// `from == g_localPlayer`, which is `DAT_0053F0C4 = 11`.
#[test]
fn holding_nothing_loses_the_game() {
    let mut g = five_realms();
    give(&mut g, 1..=14, 2);
    // Realm 2 has everything; 3, 4 and 5 die alongside the player, so the loss
    // has to survive three other eliminations in the same queue.
    let outcome = turn::end_turn(&mut g).expect("the machine comes round");

    assert_eq!(outcome.outcome, Outcome::Lost);
    assert_eq!(outcome.outcome.value(), 11, "DAT_0053F0C4");
    assert!(!g.kingdom.realms[1].in_play);
    assert!(g.kingdom.realms[2].in_play, "somebody is still standing");
    assert!(g.campaign.ranking.opponents_remaining > 0, "or it would be scored a win");
}

/// A loss does not advance the campaign: `FUN_00497879` increments only on 10,
/// and `FUN_00499E5D` then reloads the same row.
#[test]
fn a_loss_replays_the_same_map() {
    let mut g = five_realms();
    give(&mut g, 1..=14, 2);
    g.campaign.map = 3;
    let before = g.campaign.current();

    turn::end_turn(&mut g).expect("the machine comes round");
    assert_eq!(g.campaign.branch(), ConquestBranch::Lost);
    assert_eq!(g.campaign.map, 3);
    assert_eq!(g.campaign.current(), before, "the same country, to be fought again");
}

// ---------------------------------------------------------------------------
// Neither
// ---------------------------------------------------------------------------

/// The ordinary turn: nobody dies, nothing ends, and the queue is empty
/// afterwards. Without this the two tests above would pass on a chain that
/// declared a winner every turn.
#[test]
fn a_turn_in_which_nobody_dies_ends_nothing() {
    let mut g = five_realms();
    for realm in 1..=5usize {
        g.kingdom.counties[realm].owner = realm as u8;
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);

    let outcome = turn::end_turn(&mut g).expect("the machine comes round");
    assert_eq!(outcome.outcome, Outcome::InPlay);
    assert_eq!(outcome.outcome.value(), 0);
    assert!(!outcome.outcome.is_over());
    assert!(g.messages.is_empty(), "the ring was drained by the headless turn");
    assert_eq!(g.campaign.ranking.realms_in_play, 5);
    assert_eq!(g.campaign.ranking.opponents_remaining, 4);
    for realm in 1..=5 {
        assert!(g.kingdom.realms[realm].in_play, "realm {realm} survived");
    }
}

/// Five turns of an unfinished game stay unfinished, and the counter does not
/// creep. `docs/plan.md` §0: the defects that matter appear under repetition.
#[test]
fn an_unfinished_game_stays_unfinished_over_several_turns() {
    let mut g = five_realms();
    for realm in 1..=5usize {
        g.kingdom.counties[realm].owner = realm as u8;
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);

    for turn_no in 1..=5 {
        let outcome = turn::end_turn(&mut g).expect("the machine comes round");
        assert_eq!(outcome.outcome, Outcome::InPlay, "turn {turn_no}");
        assert_eq!(g.campaign.map, 0, "turn {turn_no}");
        assert!(g.messages.is_empty(), "turn {turn_no}");
    }
}

// ---------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------

/// The interstitial draws the branch the game actually reached rather than the
/// shell's cycling default.
#[test]
fn the_conquest_screen_takes_its_branch_from_the_game() {
    for (owner, expected) in [(1u8, Branch::Won), (2u8, Branch::Lost)] {
        let mut g = five_realms();
        give(&mut g, 1..=14, owner);
        turn::end_turn(&mut g).expect("the machine comes round");

        let assets = l2_game::game::Assets::placeholder();
        let mut ctx = Ctx { game: &mut g, assets: &assets };
        let mut screen = ConquestScreen::new();
        assert_eq!(screen.id(), ScreenId::Conquest);
        screen.update(&mut ctx);
        assert_eq!(screen.outcome(), expected, "owner {owner}");
    }
}

/// A game still in play leaves the screen on its own default, so walking the
/// shell still shows all three sentences.
#[test]
fn a_game_still_in_play_leaves_the_shell_cycling() {
    let mut g = five_realms();
    let assets = l2_game::game::Assets::placeholder();
    let mut ctx = Ctx { game: &mut g, assets: &assets };
    let mut screen = ConquestScreen::new();
    screen.update(&mut ctx);
    assert_eq!(screen.outcome(), Branch::Won, "the default the index screen walks");
}

// ---------------------------------------------------------------------------
// The campaign table
// ---------------------------------------------------------------------------

#[test]
fn the_second_campaign_starts_at_australia() {
    let g = {
        let mut g = five_realms();
        g.campaign = l2_game::victory::Campaign::new(Track::Second);
        g
    };
    assert_eq!(g.campaign.map, 2, "DAT_0053F258 = 2, from FUN_00433461");
    assert_eq!(g.campaign.current().map(|m| m.scenario), Some(52));
}
