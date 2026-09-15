#![allow(unused_imports)]
use super::*;

use super::*;
use super::layout::*;
use super::tests::*;
use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};
use crate::game::Game;

/// **`Msg_Dismiss` (`0x00476768`).**
///
/// Free function because the ending arm reads
/// [`crate::victory::Campaign`] and can enter the conquest screen, which is the
/// whole `Game`'s business and not the ring's.
///
/// What is deliberately **not** here: the arm for groups `0x9E`, `0x9F`, `0xEE`
/// and `0xEF`, which drops into screen `0x26` and sets `g_quitRequest = 3`.
/// Those four groups are the demo's *"Congratulations"* / *"Defeat"* posters and
/// **nothing in the binary enqueues any of them** — `docs/formats/eng.md` marks
/// all four dead. Building it would be a `dead-reproduced` arm, which
/// `crates/l2-game/tests/arms.rs` asserts stays empty. `docs/arms.json` records
/// it as `dead`.
pub fn dismiss(game: &mut Game) -> Dismissal {
    if !game.messages.is_open() {
        return Dismissal::Nothing;
    }
    game.messages.close();
    // `FUN_00476E21()`, between the redraw and the timer: if this closed while
    // `g_screenId` was the tip's `0x27`, the screen comes back and the tips wait
    // twenty frames. It acts on *any* dismissal on `0x27`, not only a tip's.
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
/// ```c
/// if (category != 0x11 && category != 10 && category != 0x0B && category != 0x0C)
///     Msg_Dismiss();
/// ```
///
/// One caller, and it is the arm nobody had looked for: **`Map_Click`'s entire
/// body is `if (g_messageGroup == 0) { … } else { this }`**. So with a message
/// up, a left click on the campaign map closes it and the map does nothing else
/// at all — no pick, no county selection, no village. A *question* survives the
/// click, which is what stops a stray click on the map from silently declining
/// an alliance.
///
/// **It calls `Msg_Dismiss`, not the two lines that clear the window**, so this
/// is a [`dismiss`] like any other: `FUN_00476E21` runs, `g_screenId` goes back
/// to `_DAT_004F0350` and `DAT_004F0358` is re-armed to `0x14`. Closing the
/// ring here instead left [`crate::tip::Tips::hosting`] set, the `0x27` host
/// seated over the map for ever and the delay at zero — and a save box pushed
/// after it starved.
///
/// Returns whether it closed. The marker for this arm is on its CALLER, in
/// `screens/map/mod.rs`: the gesture is a click on the campaign map and this is
/// only the four-line helper it reaches.
pub fn dismiss_unless_question(game: &mut Game) -> bool {
    if !game.messages.dismissed_by_map_click() {
        return false;
    }
    dismiss(game);
    true
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
/// The letter for the selected county pops over
/// the castle screen, the market or the map alike.
///
/// # Three consequences, and none of them is guessable from `Event_RollAll`
///
/// * **Only the selected county's letter is ever posted.** An event in a county
///   the player is not looking at *waits*, because `Event_RollAll` clears the
///   three modifiers and not this latch (`l2_kingdom::event::roll_all`). It
///   arrives the instant that county is picked, which may be seasons later and
///   may be the same frame the player clicks it.
/// * **The clear is outside the owner test.** A county that changed hands
/// between the roll and the click has its flag thrown away without a letter.
/// * **`eventId` is never cleared by anything.** It is overwritten by the next
///   event the county draws and otherwise stands for the rest of the game, which
///   is why four siege fixtures still read `0x8E` on a county whose Wedding
/// fever is long over. The county panels read it too
///   showing its last event's line.
///
/// # What is not reproduced, and why
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
/// # It moves no byte of the lockstep digest
///
/// `Event_Post` writes `g_counties[county].eventFired = 0`, and it may, because
/// the original is one machine with one `g_selectedCounty`. We may not:
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
/// Returns whether a letter was enqueued.
// arm: 0x00448D7E/event-letter-post frame
pub fn post_event(game: &mut Game) -> bool {
    let county = game.selected as usize;
    let player = game.player;
    let Some(&posted) = game.event_posted.get(county) else { return false };
    let Some(c) = game.kingdom.counties.get_mut(county) else { return false };
    if !c.event_fired || posted {
        return false;
    }
    // `g_counties[county].eventFired = 0` — inside the condition, *before* the
    // owner test, so it happens whoever holds the county. Ours marks instead of
    // clearing, in the same place, so the swallowing below is unchanged.
    game.event_posted[county] = true;
    let c = &game.kingdom.counties[county];
    if c.owner != player {
        return false;
    }
    // `Msg_Enqueue(0, g_localPlayer, eventId, 0, 0x0F, county, 0, 0)` —
    // `Msg_Enqueue(from, to, …)`, so **from nobody, to this player**. The group
    // is the county's stored id and the painter re-reads the county's id for the
// number line, so the two are the same number.
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

/// **The other half of [`post_event`]'s latch: `Event_RollAll` raising it.**
///
/// `Event_RollAll` (`0x00448819`) does `eventFired = 1; eventId = <slot>;` on
/// every county it deals to,
/// `Event_Post` had cleared that same byte. Ours clears
/// [`Game::event_posted`] instead — the kingdom's latch is never lowered — so
/// the raising has to be mirrored here, once per season, from the report
/// `l2_kingdom::event::roll_all` already writes.
///
/// **The report is exactly the right set.** `roll_all` pushes
/// `Message::Event` only where `fire` returned true; a handler whose guard
/// fails clears `eventFired` itself and pushes nothing, which is the original's
/// "a new event destroys an unread letter" (`l2_kingdom::event::fire`) and
/// wants no mark cleared.
pub fn rearm_events(game: &mut Game, report: &l2_kingdom::report::SeasonReport) {
    for message in &report.messages {
        if let l2_kingdom::report::Message::Event { county, .. } = message {
            if let Some(slot) = game.event_posted.get_mut(*county as usize) {
                *slot = false;
            }
        }
    }
}

/// **`Msg_DrawWindow`'s side effects on the frame the window opens** — the arms
/// that are not drawing at all, and that a reading for text and voice lookups
/// walks straight past.
///
/// Three of them, and each is in a different category's branch:
///
/// * **`0x0B`** opens `if (g_realms[g_localPlayer].ally != 0) { Msg_Dismiss();
///   return; }` — an alliance offer that arrives when you already have an ally
///   closes itself unseen. `Msg_DrawDiplomacy` has the same guard for its own
///   alliance arm (group `0xF8`).
/// * **`0x0E`** runs the outcome ladder — `l2_kingdom::victory::outcome_of` —
///   and, with no opponents left, **enqueues group 225 onto the ring it is being
///   drawn from**. That is `docs/plan.md`'s mainline win.
/// * **`0x04`** clamps the timer; see [`MessageQueue::clamp_tip_timer`].
///
/// Returns whether the window is still up. It is called from the message
/// screen's `update`, because [`crate::screen::Screen`]
/// hands `draw` a `&Ctx` on purpose and this changes the world — see
/// `crates/l2-game/src/screen/mod.rs`, *Draw cannot mutate*. The original runs it in
/// the draw; the effect is identical because the original's draw and input both
/// run once per frame, and the difference is recorded here.
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
        // The diplomatic letter carries the same alliance guard for group 0xF8
        // only — `Msg_DrawDiplomacy`'s `iVar2 == 3` arm.
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

/// **`Msg_DrawWindow`'s two animated branches** — categories `0x0D` (a county
/// taken) and `0x0E` (a lord fallen) when `g_optAnimations == 1` and
/// `g_messageTimer > 0x7C6`, which on the frame the window opens it always is.
///
/// Neither shows the window a player would recognise. Each draws a taller one
/// **once**, dismisses the message **from inside the draw**, stops the music
/// and plays a film in the well it drew:
///
/// ```c
/// /* 0x0D */ …; Msg_Dismiss(); Music_Stop(0);
///            if (2 < ++DAT_00553ED4) DAT_00553ED4 = 0;
///            if (!Smk_Play(cap_cty1.smk + DAT_00553ED4 * 0x10, 0x28, 0x69, 0, g_screenId))
///                { g_redrawRequest = 1; Music_StartCampaign(); }
///            Msg_PlayVoice(DAT_004F0374, DAT_004F0354);
/// /* 0x0E */ …the outcome ladder…; FUN_00475B41(g_messageFrom, g_messageGroup);
///            Msg_Dismiss(); Music_Stop(0); Smk_Play(&DAT_004F0340, 0x59, 0x69, …); …
/// ```
///
/// The outcome ladder is [`show`]'s, which has already run on this frame —
/// the animated branch carries its own copy of the same eleven lines. So what
/// is left here is the film: which one, the dismissal, and — because
/// `Msg_Dismiss` can enter the conquest screen, and `Smk_Play` is told to
/// return to whatever `g_screenId` is by then — whether the game is over.
///
/// Returns the film to play, or `None` when this message is not one of the two
/// or animations are off; the unanimated branches are [`show`] and the
/// ordinary window, unchanged.
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
            // `FUN_00475B41` reads the year and the lord before `Msg_Dismiss`
            // runs, though nothing it reads is something the dismissal writes.
            let file = crate::movie::ending_film(game, record.from, record.group);
            let game_over = matches!(dismiss(game), Dismissal::GameOver(_));
            Some(Film::Ending { file, record, game_over })
        }
        _ => None,
    }
}

/// **Show and dismiss every queued message at once**, and return the outcome.
///
/// This is what a *headless* turn does in place of the frame loop: it is
/// `Msg_Pump` + [`show`] + [`dismiss`] run to exhaustion, with no window and no
/// click. [`crate::turn::end_turn`] is the headless door — `docs/agents.md`,
/// *name the branch* — and it cannot raise a screen
/// still ends, and ends by the same ladder an interactive game does.
///
/// Every step goes through
/// the same [`show`] and [`dismiss`] the message screen calls, which is the
/// point: an ending settled headlessly and an ending settled by a person
/// pressing the corner button cannot disagree, because there is one ladder.
///
/// It stops at the first message that ends the game, because in the original
/// dismissing that message enters screen `0x1C` and nothing behind it in the
/// ring is ever shown.
pub fn drain(game: &mut Game) -> Outcome {
    // Fifty slots and one enqueue-while-draining (group 225)
    // iterations is a hard bound that a full ring cannot reach. A `while true`
    // here would be one ring-corruption away from hanging the turn.
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


