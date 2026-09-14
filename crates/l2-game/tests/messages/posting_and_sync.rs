#![allow(unused_imports)]
use super::*;
use super::dismissal_and_timing::*;
use super::prompts_and_alliances::*;
use super::outcomes_and_obituaries::*;
use super::painting_and_layout::*;
use super::battle_drain::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

/// Give a county a waiting event letter the way `Event_RollAll` does: the latch
/// and the id, set together, cleared by nothing but the posting.
fn waiting(g: &mut Game, county: usize, kind: l2_kingdom::event::EventKind) {
    // Tip screens off. The ladder posts its own paragraph windows on the
    // campaign map and one of them would be the record these tests read — a
    // real behaviour (`tests/tips.rs`) and not this one.
    g.prefs.tip_screens = false;
    let c = &mut g.kingdom.counties[county];
    c.event_fired = true;
    c.event_id = kind.id();
}

/// **A random event reaches the player, and it reaches him by being looked at.**
///
/// `FUN_00448D7E`, `Battle_Frame`'s `FUN_00448d7e(g_selectedCounty)` at
/// `0x004BA187`. Nothing here enqueues: the county is selected and the frame
/// driver runs.
///
/// Ablation, run: the `crate::message::post_event` call removed from
/// `Machine::update` → the scroll never opens, which is the state this branch
/// found the game in.
#[test]
fn selecting_a_county_with_a_waiting_event_opens_its_letter() {
    let (mut g, a, mut m) = world();
    g.kingdom.counties[3].owner = 1;
    waiting(&mut g, 3, l2_kingdom::event::EventKind::Witch);

    // Not selected: the frame runs and nothing happens, however long.
    for _ in 0..20 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "an unlooked-at county says nothing");
    assert!(g.kingdom.counties[3].event_fired, "and its latch is untouched");

    assert!(g.select(3));
    open_the_scroll(&mut m, &mut g, &a);
    let record = *g.messages.open().expect("the letter");
    assert_eq!(record.category, category::EVENT);
    assert_eq!(record.group, l2_kingdom::event::EventKind::Witch.id());
    assert_eq!(record.county, 3);
    assert_eq!(record.to, 1, "Msg_Enqueue(0, g_localPlayer, …) — from nobody, to this player");
    assert_eq!(record.from, 0);
    assert!(g.event_posted[3], "the poster took the letter");
    assert!(g.kingdom.counties[3].event_fired, "and left the kingdom's latch alone");
    assert_eq!(g.kingdom.counties[3].event_id, 0x134, "and cleared nothing else");
}

/// **The latch is cleared once**, so the same letter is not posted every frame
/// the county stays selected.
#[test]
fn a_letter_is_posted_once_however_long_the_county_stays_picked() {
    let (mut g, a, mut m) = world();
    g.kingdom.counties[2].owner = 1;
    waiting(&mut g, 2, l2_kingdom::event::EventKind::Treasure);
    assert!(g.select(2));
    open_the_scroll(&mut m, &mut g, &a);
    send(&mut m, &mut g, &a, Event::RightClick { x: 300, y: 300 });
    for _ in 0..30 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "no second letter");
    assert!(g.messages.waiting().is_empty());
}

/// **The clear is outside the owner test**,
/// between the roll and the click loses its letter without anyone reading it.
/// `[V]` — `(eventFired = 0, owner == g_localPlayer)` is one comma expression:
/// the assignment runs, then the test.
#[test]
fn a_rivals_county_swallows_its_letter_when_you_look_at_it() {
    let (mut g, a, mut m) = world();
    g.kingdom.counties[4].owner = 3;
    waiting(&mut g, 4, l2_kingdom::event::EventKind::Rats);
    assert!(g.select(4));
    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "not your county, not your letter");
    assert!(g.event_posted[4], "and the letter is swallowed all the same");
}

/// **Posting a letter moves no byte of the kingdom at all.**
///
/// `County::event_fired` is `County+0x000` and sits in `Encode for County`, so
/// a per-peer write there is inside `l2_kingdom::save::checksum`, the lockstep
/// digest. `Event_Post` (`0x00448D7E`) does exactly that write and may, being
/// one machine; ours marks [`Game::event_posted`] instead. `docs/netcode.md`
/// §6, `docs/decisions.md` C210.
#[test]
fn the_poster_touches_nothing_in_the_kingdom() {
    let (mut g, _a, _m) = world();
    g.kingdom.counties[5].owner = 1;
    waiting(&mut g, 5, l2_kingdom::event::EventKind::NoSongs);
    assert!(g.select(5));
    let before = g.kingdom.clone();

    assert!(message::post_event(&mut g), "the letter went out");
    assert!(g.event_posted[5], "on the Game");
    assert!(g.kingdom.counties[5].event_fired, "and not in the kingdom");

    assert_eq!(
        l2_net::Canonical::hash_of(&before),
        l2_net::Canonical::hash_of(&g.kingdom),
        "the poster wrote into the hashed kingdom"
    );
}

/// **Two peers looking at different counties still agree on the tick.**
///
/// The defect this is written against: `post_event` wrote `event_fired` from
/// [`Game::selected`], a per-peer cursor, into the byte at `County+0x000` that
/// `l2_kingdom::save::checksum` — `Canonical::hash_of(kingdom)` — covers. One
/// frame of two `Game`s over the same `Kingdom`, each with its own selection
/// and each with a letter waiting, and the two digests part. Observed red
/// before the fix: the checksums differed after the single frame.
///
/// The original cannot answer this. `Battle_Frame` posts for
/// `g_selectedCounty` (`0x004BA187`) on **one** machine with one selection; the
/// question only exists here. `docs/netcode.md` §6, *What the original
/// did* — networking is the one place the binary is not the authority.
#[test]
fn two_peers_with_different_selections_hash_the_same_kingdom() {
    let (mut red, a, mut m_red) = world();
    red.kingdom.realms[2].is_human = true;
    red.kingdom.counties[3].owner = 1;
    red.kingdom.counties[6].owner = 2;
    waiting(&mut red, 3, l2_kingdom::event::EventKind::Witch);
    waiting(&mut red, 6, l2_kingdom::event::EventKind::Rats);

    // The same world on both peers, which is what tick 0 of a lockstep session
    // is, and then two players with two different cursors.
    let mut blue = red.clone();
    blue.player = 2;
    let mut m_blue = Machine::new(ScreenId::Campaign);
    assert_eq!(
        l2_kingdom::save::checksum(&red.kingdom),
        l2_kingdom::save::checksum(&blue.kingdom),
        "setup: the peers start agreed"
    );
    assert!(red.select(3));
    assert!(blue.select(6));

    tick(&mut m_red, &mut red, &a);
    tick(&mut m_blue, &mut blue, &a);

    assert_eq!(
        l2_kingdom::save::checksum(&red.kingdom),
        l2_kingdom::save::checksum(&blue.kingdom),
        "the selected county reached the lockstep digest"
    );
    // And each peer really did post its own letter, so the test is not passing
    // by doing nothing.
    assert!(red.event_posted[3] && !red.event_posted[6]);
    assert!(blue.event_posted[6] && !blue.event_posted[3]);
}

/// **A second event on a county whose first letter was read still arrives.**
///
/// `Event_RollAll` (`0x00448819`) does `eventFired = 1` and the original needs
/// nothing more, because `Event_Post` had cleared the same byte. Ours keeps the
/// clearing on the `Game`, so the raising has to reach it — `crate::turn`
/// lowers the mark for every county the season report names.
#[test]
fn a_fresh_event_rearms_a_county_whose_letter_was_already_read() {
    let (mut g, _a, _m) = world();
    g.kingdom.counties[1].owner = 1;
    waiting(&mut g, 1, l2_kingdom::event::EventKind::Witch);
    assert!(g.select(1));
    assert!(message::post_event(&mut g), "the first letter");
    assert!(!message::post_event(&mut g), "and only once");

    // What a season in which `Event_RollAll` dealt the county an event looks like
    // on the way out: the latch is up and the report names
    // the county.
    let mut report = l2_kingdom::report::SeasonReport::default();
    report.message(l2_kingdom::report::Message::Event {
        county: 1,
        kind: l2_kingdom::event::EventKind::Rats,
    });
    g.kingdom.counties[1].event_id = l2_kingdom::event::EventKind::Rats.id();
    message::rearm_events(&mut g, &report);

    assert!(message::post_event(&mut g), "the new letter goes out too");
    assert_eq!(
        g.messages.waiting().last().expect("the second letter").group,
        l2_kingdom::event::EventKind::Rats.id()
    );
}

// -------------------------------------------------------------- 4. the save

/// **A save taken with messages queued keeps them.**
///
/// This is not tidiness. The endings are shown one at a time now,
/// really can save on the campaign map with three obituaries behind the one on
/// screen — and a save that dropped them would be a save that can never be won.
#[test]
fn a_save_carries_the_ring_and_the_window() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    post(&mut g, notice(0x93));
    post(&mut g, Record { to: 1, from: 3, group: 180, category: category::ALLIANCE_PROMPT, ..Record::default() });
    open_the_scroll(&mut m, &mut g, &a);

    let bytes = l2_game::save::encode(&g);
    let back = l2_game::save::decode(&bytes, l2_kingdom::tables::Tables::DEFAULT)
        .expect("it reads back");

    assert_eq!(back.messages.open(), g.messages.open(), "the record on screen");
    assert_eq!(back.messages.timer(), g.messages.timer(), "with its timer");
    assert_eq!(back.messages.waiting(), g.messages.waiting(), "and the two behind it");
    assert_eq!(back.messages.waiting().len(), 2);
}

// ------------------------------------------------- 5. against real artwork
//
// Five defects in one week existed only against the real files — a caret that
// drew nothing because it used `_` and `Ui_DrawText` maps that byte to a space,
// and text landing on top of itself because a helper returned an advance with
// the real font and an absolute position without one. Nothing below has a
// threshold in it: each is an equality or an inequality between two canvases,
// which is what `docs/agents.md` asks for instead.

