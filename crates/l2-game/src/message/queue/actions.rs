#![allow(unused_imports)]
use super::*;

use super::*;
use super::layout::*;
use super::tests::*;
use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};
use crate::game::Game;

/// `Msg_Dismiss` (`0x00476768`) reads `crate::victory::Campaign` and can enter
/// the conquest screen, which belongs to `Game`.
pub fn dismiss(game: &mut Game) -> Dismissal {
    if !game.messages.is_open() {
        return Dismissal::Nothing;
    }
    game.messages.close();
    // `FUN_00476E21()` between the redraw and the timer: if this closed while
    // `g_screenId` was `0x27`, the screen comes back and the tips wait twenty
    // frames. It runs on any dismissal on `0x27`, a tip's or not.
    game.tips.restore();
    let outcome = game.campaign.outcome;
    if outcome.is_over() {
        // `Campaign_EnterConquest` (`0x00497879`), then `g_screenId = 0x1C`.
        game.campaign.enter_conquest_screen();
        return Dismissal::GameOver(outcome);
    }
    Dismissal::Closed
}

/// **`Msg_DismissUnlessQuestion` (`FUN_00476710`, `0x00476710`)** — the whole
/// function:
///
/// `FUN_00476710` (`00470000.c:2409`) calls `Msg_Dismiss` (`2438`), so this is a
/// [`dismiss`]: `FUN_00476E21` runs, `g_screenId` goes back to `_DAT_004F0350`
/// and `DAT_004F0358` is re-armed to `0x14`. Until 2026-09-14 ours closed the
/// ring only, which left [`crate::tip::Tips::hosting`] set and the `0x27` host
/// seated over the map, and a save box pushed after it got no update.
pub fn dismiss_unless_question(game: &mut Game) -> Option<Dismissal> {
    if !game.messages.dismissed_by_map_click() {
        return None;
    }
    Some(dismiss(game))
}

/// **`Event_Post` (`FUN_00448D7E`, `0x00448D7E`) — the only thing in the binary
/// that posts a county's random-event letter.** `[V]`, whole body:
///
/// ```c
/// void FUN_00448d7e(int county) {
///     if ((g_counties[county].eventFired != 0) && (g_mouseRightDown == '\0') &&
///        (g_counties[county].eventFired = 0, g_counties[county].owner == g_localPlayer)) {
///         Msg_Enqueue(0, g_localPlayer, (short)g_counties[county].eventId, 0, '\x0f',
///                     (uchar)county, '\0', 0);
///     }
/// }
/// ```
///
/// **`Battle_Frame` is its only caller**, once a frame, as
/// `FUN_00448d7e(g_selectedCounty)` at `0x004BA187` — between `Msg_Pump` and
/// `Turn_Tick`, which is where [`crate::screen::Machine::update`] calls this.
///
/// **`g_mouseRightDown`.** Our [`crate::input::Event`] has no *held* right
/// button — `RightClick` is the release and [`crate::input::Event::RightPress`] the
/// down edge, both edges, because the original's fifty-odd right-button arms
/// all read the released-this-frame flag (`DAT_004E6900`) and never the held
/// one. So
/// right button is down, the guard would be false on every one of them, and a
/// flag invented to satisfy it would be a flag nothing could ever set. The
/// original's effect is to hold a letter back while the button is held; ours
/// posts on the next frame either way.
///
/// `County::event_fired` is `County+0x000` and is in `Encode for County`, so it
/// is inside `l2_kingdom::save::checksum` — `Canonical::hash_of(kingdom)`, the
/// per-tick lockstep digest — and [`Game::selected`] is a per-peer cursor. Two
/// peers looking at different counties would hash differently on the next tick
/// over a byte **nothing in the simulation reads**: `event_fired` is written by
/// `Event_RollAll` and the 24 handlers and read here alone, in the original and
/// here.
///
/// So the clearing lives on [`Game::event_posted`], which is presentation state
/// and never reaches the kingdom — the same split as [`Game::player_names`],
/// and the alternative (a field in the save but out of the digest) would need a
/// second encoder that `l2_net::Canonical` does not have. The latch itself
/// stays
/// `docs/netcode.md` §6, `docs/decisions.md` C210.
///
// arm: 0x00448D7E/event-letter-post frame
pub fn post_event(game: &mut Game) -> bool {
    let county = game.selected as usize;
    let player = game.player;
    let Some(&posted) = game.event_posted.get(county) else { return false };
    let Some(c) = game.kingdom.counties.get_mut(county) else { return false };
    if !c.event_fired || posted {
        return false;
    }
    game.event_posted[county] = true;
    let c = &game.kingdom.counties[county];
    if c.owner != player {
        return false;
    }
    let group = c.event_id;
    let record = Record {
        to: player,
        from: 0,
        group,
        variant: 0,
        category: category::EVENT,
        county: county as u8,
        spare: 0,
        payload: 0,
    };
    game.messages.enqueue(record, player)
}

/// `Event_RollAll` (`0x00448819`) does `eventFired = 1; eventId = <slot>;` on
/// every county it deals to,
/// `Event_Post` had cleared that same byte. Ours clears
/// [`Game::event_posted`] instead — the kingdom's latch is never lowered — so
/// the raising has to be mirrored here, once per season, from the report
/// `l2_kingdom::event::roll_all` already writes.
pub fn rearm_events(game: &mut Game, report: &l2_kingdom::report::SeasonReport) {
    for message in &report.messages {
        if let l2_kingdom::report::Message::Event { county, .. } = message {
            if let Some(slot) = game.event_posted.get_mut(*county as usize) {
                *slot = false;
            }
        }
    }
}

/// * **`0x0E`** runs the outcome ladder — `l2_kingdom::victory::outcome_of` —
///   and, with no opponents left, **enqueues group 225 onto the ring it is being
///   drawn from**. That is `docs/plan.md`'s mainline win.
pub fn show(game: &mut Game) -> bool {
    let Some(record) = game.messages.open().copied() else { return false };
    // arm: 0x0047309E/tip-timer-clamp draw
    game.messages.clamp_tip_timer();
    if !game.messages.just_opened() {
        return true;
    }
    match record.category {
        // arm: 0x0047309E/alliance-offer-lapses draw
        category::ALLIANCE_PROMPT => {
            let ally = game.kingdom.realms.get(game.player as usize).map_or(0, |r| r.ally);
            if ally != 0 {
                dismiss(game);
                return false;
            }
        }
        // arm: 0x0047309E/ending-sets-outcome draw
        category::ENDING => {
            let step = victory::outcome_of(
                record.as_ending(),
                game.player,
                game.campaign.ranking,
                game.kingdom.options.quirks,
            );
            match step {
                OutcomeStep::Set(o) => game.campaign.outcome = o,
                OutcomeStep::EnqueueVictory => {
                    game.campaign.outcome = Outcome::InPlay;
                    let victory = victory::victory_message(game.player);
                    let player = game.player;
                    game.messages.enqueue(Record::from(victory), player);
                }
            }
        }
        // arm: 0x00475E07/letter-alliance-lapses draw
        category::DIPLOMACY if record.group == 0xF8 => {
            let ally = game.kingdom.realms.get(game.player as usize).map_or(0, |r| r.ally);
            if ally != 0 {
                dismiss(game);
                return false;
            }
        }
        _ => {}
    }
    true
}

/// ```c
/// /* 0x0D */ …; Msg_Dismiss(); Music_Stop(0);
///            if (2 < ++DAT_00553ED4) DAT_00553ED4 = 0;
///            if (!Smk_Play(cap_cty1.smk + DAT_00553ED4 * 0x10, 0x28, 0x69, 0, g_screenId))
///                { g_redrawRequest = 1; Music_StartCampaign(); }
///            Msg_PlayVoice(DAT_004F0374, DAT_004F0354);
/// /* 0x0E */ …the outcome ladder…; FUN_00475B41(g_messageFrom, g_messageGroup);
///            Msg_Dismiss(); Music_Stop(0); Smk_Play(&DAT_004F0340, 0x59, 0x69, …); …
/// ```
pub fn animate(game: &mut Game) -> Option<crate::movie::Film> {
    use crate::movie::Film;
    let record = game.messages.open().copied()?;
    if !game.prefs.animations || game.messages.timer() <= 0x7C6 {
        return None;
    }
    match record.category {
        category::CAPTURE => {
            dismiss(game);
            let take = game.films.next_capture();
            Some(Film::Capture { take, record })
        }
        category::ENDING => {
            // `FUN_00475B41` reads year and lord before `Msg_Dismiss` runs.
            let file = crate::movie::ending_film(game, record.from, record.group);
            let game_over = matches!(dismiss(game), Dismissal::GameOver(_));
            Some(Film::Ending { file, record, game_over })
        }
        _ => None,
    }
}

pub fn drain(game: &mut Game) -> Outcome {
    // Fifty slots plus group 225 draining iteration bounds ring processing.
    for _ in 0..(RING * 2) {
        if !game.messages.is_open() && !game.messages.pull() {
            if game.messages.is_empty() {
                break;
            }
            continue;
        }
        if !show(game) {
            continue;
        }
        if let Dismissal::GameOver(o) = dismiss(game) {
            return o;
        }
    }
    game.campaign.outcome
}


