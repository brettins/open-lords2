//! **How a game of Lords of the Realm II ends.**
//!
//! Four functions in the original, and the chain is short enough to state whole:
//!
//! 1. `FUN_0049B42B` ([`recount_strength`]) rebuilds realm `+0x04` as
//!    `3 * counties + 1 * armies`. A realm that comes out zero has just been
//!    eliminated and a message says so.
//! 2. `Score_RankRealms` (`0x0049AA0E`, [`rank_and_crown`]) scores and ranks the
//!    survivors, counts how many of them are **not** the local player, and — when
//!    the ranking table's first and last live entries are the same realm — crowns
//!    whoever is left.
//! 3. `Msg_DrawWindow`'s category-`0x0E` arm ([`outcome_of`]) turns the message
//!    being displayed into `DAT_0053F0C4`: **10 won, 11 lost**.
//! 4. `FUN_00497879` ([`crate::victory::Outcome`]'s consumer, which lives in the
//!    game crate) advances the campaign counter on a win and enters screen
//!    `0x1C`.
//!
//! # The victory condition, and it is not what it reads like
//!
//! `docs/plan.md` revision 4 records it as *"`Score_RankRealms` fires group 225
//! when the trailing realm equals the leader"* and warns that this "reads
//! strange". Read against the code it is not strange at all — but it is also not
//! what the sentence suggests. The comparison is
//!
//! ```c
//! if (g_rankTrailer == g_rankLeader) { ... }
//! ```
//!
//! and `g_rankLeader` / `g_rankTrailer` are **realm indices**, not scores: the
//! first and the last non-zero entry of the sorted ranking table, from which
//! every eliminated realm has just been struck out. Two realm *indices* are equal
//! exactly when the table has one live entry. So the condition is the ordinary
//! one — **one realm left standing** — written as a scan of a table rather than a
//! count. It is not "the bottom-ranked realm caught the leader on points".
//!
//! What *is* strange is everything around it, and all three are reproduced here:
//!
//! * **The empty case passes too.** With nobody in play, leader and trailer are
//!   both `0`, `0 == 0`, and the original crowns `g_realms[0]` — the array slot.
//!   See [`Ranking::sole_survivor`].
//! * **The last realm standing being an AI does not end the game the first
//!   time.** It sends group 195, *"Just call me king."*, and sets the one-shot
//!   guard `+0xED`. `Score_RankRealms` runs many times a turn, so the *next* call
//!   finds the guard set, takes the other branch, and sends the human group 225
//!   *"Victory!"* — the human wins a game in which the human is dead. In practice
//!   the human's own group 224 has already set the outcome to 11 by then.
//! * **The mainline human victory is not this function at all.** It is
//!   [`outcome_of`]'s last branch: whenever a category-`0x0E` message is
//!   displayed and [`Ranking::opponents_remaining`] is zero, `Msg_DrawWindow`
//!   enqueues group 225 at the human. Killing the last AI raises group 194 for
//!   *that AI*; displaying 194 with no opponents left is what wins the game.
//!
//! # The human is treated differently, three times over
//!
//! * `AI_RunTurnStep` (`0x0049A581`) runs **step 0 for every realm**, including
//!   the human: the `isHuman` test guards the fourteen *handlers*, not the
//!   initialisation above them. So the human's strength is recounted, and the
//!   human's elimination detected, on the human's own turn. See
//!   [`crate::ai::begin_realm_turn`], whose caller must therefore not skip
//!   humans.
//! * The message [`recount_strength`] sends is chosen on **"is this the local
//!   player"**, not on "is this a human": the local player gets group 224
//!   *"Defeat!"*, an AI gets group 194 *"Foiled again."*, and a **second human in
//!   a network game gets neither**.
//! * `voice_rotation` is advanced for all three of those cases, the local
//!   player's included, even though the human has no recorded voice lines.
//!
//! # What does *not* end a game
//!
//! Nothing here reads a turn limit, a score threshold or a date. The only ending
//! is elimination — every other realm's, or your own.

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{UnitKind, Units};
use l2_net::{Quirk, Quirks};

/// `L2.eng` group 194 — *"Foiled again."*, an **AI** realm has been eliminated.
pub const MSG_AI_ELIMINATED: u16 = 194;
/// `L2.eng` group 195 — *"Just call me king."*, an AI is the last realm standing.
pub const MSG_AI_CROWNED: u16 = 195;
/// `L2.eng` group 224 — *"Defeat!"*, **you** have been eliminated.
pub const MSG_DEFEAT: u16 = 224;
/// `L2.eng` group 225 — *"Victory!"*.
pub const MSG_VICTORY: u16 = 225;

/// The message category the ending messages travel in. `Msg_DrawWindow`'s
/// `g_messageCategory == 0x0E` arm is the only place `DAT_0053F0C4` is written
/// during play.
pub const CATEGORY_ENDING: u8 = 0x0E;

/// `DAT_0053F0C4` — how the game ended, or that it has not.
///
/// The two live values are the original's own: **10 won, 11 lost**. Everything
/// that reads it (`FUN_00497879`, `FUN_0042E060`, `FUN_00476768`, the screen
/// `0x1C` painter) tests exactly those two numbers, so they are the
/// discriminants rather than a table on the side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Outcome {
    /// `0`. `Game_NewGame` and `FUN_004975D3` both clear it to this.
    #[default]
    InPlay = 0,
    Won = 10,
    Lost = 11,
}

impl Outcome {
    /// The byte the original stores.
    pub fn value(self) -> u8 {
        self as u8
    }

    pub fn from_value(v: u8) -> Option<Outcome> {
        match v {
            0 => Some(Outcome::InPlay),
            10 => Some(Outcome::Won),
            11 => Some(Outcome::Lost),
            _ => None,
        }
    }

    /// Whether the game is over. The two consumers that gate on it —
    /// `FUN_00476768` and `FUN_0042E060` — both write `(x == 0xb) || (x == 10)`.
    pub fn is_over(self) -> bool {
        self != Outcome::InPlay
    }
}

/// One thing the ending chain did, in the order it did it.
///
/// These are the `Msg_Enqueue` calls of `FUN_0049B42B` and `Score_RankRealms`,
/// kept as data because the message *queue* is not this crate's — and because
/// [`outcome_of`] reads two fields of a queued message (`group` and `from`) and
/// nothing else, so a caller with a real queue and a caller with a `Vec` reach
/// the same outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ending {
    /// `L2.eng` group: one of the four constants above.
    pub group: u16,
    /// `Msg_Enqueue`'s `from`, which `Msg_DrawWindow` reads back as
    /// `DAT_00553EE0`. **This, compared against the local player, is what tells
    /// a loss from someone else's loss** — see [`outcome_of`].
    pub from: u8,
    /// `Msg_Enqueue`'s `to`. Carried for faithfulness; nothing here branches on
    /// it.
    pub to: u8,
    /// The message category. Group 195 travels as category 1 and every other
    /// ending message as [`CATEGORY_ENDING`], which is why the crowning of an AI
    /// cannot itself set an outcome.
    pub category: u8,
    /// `Msg_Enqueue`'s `+0x0C` — **which of the group's strings the window
    /// draws**, as `variant + 1` past the label at index 0.
    ///
    /// It matters here and it was missing: groups 194 and 195 hold **seventeen**
    /// strings apiece — a label and sixteen lord-flavoured laments — and the
    /// original picks one with [`voice_variant`], `lord * 4 + rotation - 4`. An
    /// `Ending` with no variant draws the Knight's first line for every lord in
    /// the game, for ever. Groups 224 and 225 pass 0 and have three and two
    /// strings.
    pub variant: u8,
}

/// `(voiceRotation - 4) + lord * 4` — the variant every lord-flavoured message
/// in the game is enqueued with, and the reason `Realm` carries a rotation at
/// `+0x159` at all.
///
/// Lords are 1..=4 and the rotation 0..=3, so this is 0..=15 in four contiguous
/// blocks of four: **each lord has four things to say and says them in turn.**
/// It is not clamped here for the same reason nothing clamps it there — a lord
/// byte outside 1..=4 is a realm that was never set up.
pub fn voice_variant(realm: &Realm) -> u8 {
    (realm.lord as i32 * 4 + realm.voice_rotation as i32 - 4).clamp(0, 15) as u8
}

impl Ending {
    /// Whether `Msg_DrawWindow` would take its `DAT_0053F0C4` arm for this
    /// message.
    pub fn sets_outcome(self) -> bool {
        self.category == CATEGORY_ENDING
    }
}

/// `3 * counties + 1 * armies` — realm `+0x04`, rebuilt from scratch.
///
/// **Armies only.** `FUN_0049B42B` counts `g_units` slots 1..=150 whose type byte
/// is 1; merchants, transports and peasant mobs are in the same array and do not
/// count (`docs/armies.md`). The result cannot overflow the byte it lands in:
/// sixteen counties and a hundred and fifty armies is 198.
pub fn strength(
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    units: &Units,
    realm: u8,
) -> u8 {
    let mut s: u8 = 0;
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        if counties[id].owner == realm {
            s = s.wrapping_add(3);
        }
    }
    for (_, u) in units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            s = s.wrapping_add(1);
        }
    }
    s
}

/// `FUN_0049B42B` — recount one realm's strength and notice if it has just died.
///
/// Called from four places in the original, and the four are worth having in one
/// list because they are the whole of "when can a realm be eliminated":
///
/// | caller | when |
/// |---|---|
/// | `AI_RunTurnStep` step 0 | the top of **every** realm's turn, human included |
/// | `County_ChangeOwner` (`0x004A72FE`) | a county changes hands — **for the realm that is losing it** |
/// | `Battle_Resolve` (`0x004A4E..`), twice | after the loser's army is destroyed |
///
/// **The `County_ChangeOwner` call cannot eliminate anybody**, and that is a
/// quirk of the original rather than a reading of it: it recounts the old owner
/// *before* `g_counties[c].owner = newOwner` runs, so the county being lost is
/// still counted. A realm losing its last county therefore survives until its own
/// step 0 comes round. Reproduced by [`crate::conquest::change_owner`]'s caller
/// doing the same thing in the same order.
///
/// # The guard
///
/// The whole body is skipped for a realm whose strength is **already** zero, so
/// an eliminated realm is never re-eliminated and never sends its message twice.
///
/// Returns the message the original enqueues, if any. The caller must run
/// [`rank_and_crown`] afterwards — the original calls `Score_RankRealms` at the
/// bottom of this function, and it is split out only so the two halves can be
/// tested apart.
pub fn recount_strength(
    realms: &mut [Realm; MAX_REALMS],
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    units: &Units,
    realm: u8,
    local_player: u8,
) -> Option<Ending> {
    let id = realm as usize;
    if id == 0 || id >= MAX_REALMS || realms[id].strength == 0 {
        return None;
    }
    let s = strength(counties, county_count, units, realm);
    realms[id].strength = s;
    realms[id].in_play = s != 0;
    if s != 0 {
        return None;
    }

    // The three-way split is on **the local player**, not on `is_human`. A
    // second human in a network game falls through both arms and is told
    // nothing, which is a real difference and not an oversight here.
    let msg = if local_player == realm {
        // `Msg_Enqueue(g_localPlayer, g_localPlayer, 0xE0, 0, 0x0E, …)` —
        // **variant 0**, so your own defeat is always group 224's first string.
        Some(Ending {
            group: MSG_DEFEAT,
            from: local_player,
            to: local_player,
            category: CATEGORY_ENDING,
            variant: 0,
        })
    } else if !realms[id].is_human {
        // `Msg_Enqueue(realm, 0, 0xC2, (rotation - 4) + lord * 4, 0x0E, …)`.
        Some(Ending {
            group: MSG_AI_ELIMINATED,
            from: realm,
            to: 0,
            category: CATEGORY_ENDING,
            variant: voice_variant(&realms[id]),
        })
    } else {
        None
    };
    // Advanced in all three cases, the silent one included.
    advance_voice(&mut realms[id]);
    msg
}

/// Realm `+0x159`, wrapping at 4. `docs/diplomacy.md` §0.
fn advance_voice(realm: &mut Realm) {
    realm.voice_rotation += 1;
    if realm.voice_rotation > 3 {
        realm.voice_rotation = 0;
    }
}

/// What `Score_RankRealms` leaves behind besides the ranks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ranking {
    /// `g_rankLeader` (`0x00553D24`) — the first live entry of the sorted table,
    /// 0 when nobody is in play.
    pub leader: u8,
    /// `g_rankTrailer` — the last live entry.
    pub trailer: u8,
    /// `DAT_0056D5D8` — how many realms are in play that are **not** the local
    /// player. [`outcome_of`]'s last branch is the whole reason this is counted.
    pub opponents_remaining: u8,
    /// How many realms are in play at all, local player included. Not a global
    /// of the original's; it is here so a test can tell "one left" from "none
    /// left" without re-walking the array.
    pub realms_in_play: u8,
}

impl Ranking {
    /// The original's test, verbatim: `g_rankTrailer == g_rankLeader`.
    ///
    /// **True when nobody is in play**, because both are then 0 — and the
    /// original goes on to crown `g_realms[0]`, the slot that is never a realm.
    /// Keep the test and let [`rank_and_crown`] deal with the slot.
    pub fn sole_survivor(self) -> bool {
        self.trailer == self.leader
    }
}

/// `Score_RankRealms` (`0x0049AA0E`) — score, rank, and crown the last realm
/// standing.
///
/// The ranking itself is [`crate::ai::rank_realms`], which was already here. This
/// adds the three globals that function drops on the floor and the crowning at
/// the bottom of it.
///
/// Five callers in the original: `Turn_Tick`'s phase 7, `Game_NewGame`,
/// `Turn_AdvancePhase`, [`recount_strength`], and one UI path. It is **not** in
/// `Season_Advance`'s call list — `docs/kingdom.md` §3.4 said it was and
/// `crates/l2-kingdom/src/phase.rs` already records the correction.
pub fn rank_and_crown(
    t: &Tables,
    realms: &mut [Realm; MAX_REALMS],
    local_player: u8,
    quirks: Quirks,
    out: &mut Vec<Ending>,
) -> Ranking {
    crate::ai::rank_realms(t, realms);

    let mut r = Ranking::default();
    for id in 1..MAX_REALMS {
        if !realms[id].in_play {
            continue;
        }
        r.realms_in_play += 1;
        if id as u8 != local_player {
            r.opponents_remaining += 1;
        }
    }
    // `g_rankLeader` / `g_rankTrailer` are read off the *sorted* table, but the
    // table's live entries are exactly the in-play realms and the sort is by
    // rank, so the first and last live entries are the rank-1 and rank-n realms.
    // Walking ranks avoids materialising a sorted array whose order would have
    // to be argued about (`docs/netcode.md` §3).
    for id in 1..MAX_REALMS {
        if !realms[id].in_play {
            continue;
        }
        if r.leader == 0 || realms[id].rank < realms[r.leader as usize].rank {
            r.leader = id as u8;
        }
        if r.trailer == 0 || realms[id].rank > realms[r.trailer as usize].rank {
            r.trailer = id as u8;
        }
    }

    // **Switchable** — [`Quirk::EmptyGameIsWonBySlotZero`], `docs/bugs.md` B51.
    // With nobody in play leader and trailer are both 0, `0 == 0` passes, and
    // the original crowns `g_realms[0]`, which is not a realm. The fixed path
    // requires somebody to be standing before anyone is crowned.
    let crowning = r.sole_survivor()
        && (quirks.reproduces(Quirk::EmptyGameIsWonBySlotZero) || r.realms_in_play > 0);
    if crowning {
        let winner = r.leader as usize;
        // **Switchable** — [`Quirk::DeadHumanCanStillWin`], `docs/bugs.md` B52.
        // The `else` limb is reached twice for an AI winner: `Score_RankRealms`
        // runs many times a turn, and the second call finds `crowned_once` set,
        // falls through, and sends the *human* group 225 *"Victory!"* — in a
        // game the human is not in. The fixed path sends the victory only to a
        // local player who is actually the realm left standing.
        let victory_is_the_local_players =
            quirks.reproduces(Quirk::DeadHumanCanStillWin) || winner == local_player as usize;
        if !realms[winner].crowned_once && !realms[winner].is_human {
            realms[winner].crowned_once = true;
            out.push(Ending {
                group: MSG_AI_CROWNED,
                from: r.leader,
                to: 0,
                // Category 1, not 0x0E: an AI's coronation is a taunt and
                // cannot set an outcome.
                category: 1,
                variant: voice_variant(&realms[winner]),
            });
            advance_voice(&mut realms[winner]);
        } else {
            realms[winner].crowned_once = true;
            if victory_is_the_local_players {
                out.push(Ending {
                    group: MSG_VICTORY,
                    from: 0,
                    to: local_player,
                    category: CATEGORY_ENDING,
                    variant: 0,
                });
            }
        }
    }
    r
}

/// What displaying one message does to `DAT_0053F0C4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeStep {
    /// The outcome is set to this. `Msg_DrawWindow` writes `DAT_0053F0C4 = 0`
    /// first and then may overwrite it, so [`Outcome::InPlay`] is a real answer
    /// and not "nothing happened".
    Set(Outcome),
    /// Nobody is left to fight. The original **enqueues group 225 at the local
    /// player** and leaves the outcome at zero for this message; the victory
    /// arrives when that message is displayed in its turn.
    EnqueueVictory,
}

/// `Msg_DrawWindow`'s category-`0x0E` arm, which is the only writer of
/// `DAT_0053F0C4` during play.
///
/// The original writes the same four-way ladder twice, once in each of the
/// animated and unanimated paths, and they are identical:
///
/// ```c
/// DAT_0053f0c4 = 0;
/// if ((group == 0xe1) || (0 < DAT_0056d5d8)) {
///   if ((group == 0xe1) || (g_localPlayer != DAT_00553ee0)) {
///     if (group == 0xe1) { DAT_0053f0c4 = 10; }
///   } else { DAT_0053f0c4 = 0xb; }
/// } else { Msg_Enqueue(0, g_localPlayer, 0xe1, 0, '\x0e', 0, 0, 0); }
/// ```
///
/// Flattened, and this is the whole of "who won":
///
/// | message | opponents left | result |
/// |---|---|---|
/// | 225 *Victory!* | any | **won** |
/// | anything else | some | **lost** if it is about me, otherwise nothing |
/// | anything else | none | enqueue 225 — and the game is won when it shows |
///
/// The third row is the ordinary way a person wins: the last AI's elimination
/// raises group 194 *about that AI*, and displaying 194 with nobody left is what
/// produces the victory. The second row is the ordinary way a person loses:
/// group 224 with `from == me`.
pub fn outcome_of(
    msg: Ending,
    local_player: u8,
    ranking: Ranking,
    quirks: Quirks,
) -> OutcomeStep {
    if msg.group == MSG_VICTORY {
        return OutcomeStep::Set(Outcome::Won);
    }
    // **Switchable** — [`Quirk::MutualDestructionIsAWin`], `docs/bugs.md` B53.
    // The original tests "no opponents left" *before* "is this message about
    // me", so a local player eliminated on the same pass as the last opponent
    // is handed a victory. The fixed path asks whose defeat this is first; the
    // two tests are otherwise unchanged and in the same function.
    let mine_first = !quirks.reproduces(Quirk::MutualDestructionIsAWin);
    if mine_first && msg.from == local_player {
        return OutcomeStep::Set(Outcome::Lost);
    }
    if ranking.opponents_remaining == 0 {
        return OutcomeStep::EnqueueVictory;
    }
    if msg.from == local_player {
        OutcomeStep::Set(Outcome::Lost)
    } else {
        OutcomeStep::Set(Outcome::InPlay)
    }
}

/// The message `Msg_DrawWindow` enqueues for [`OutcomeStep::EnqueueVictory`].
pub fn victory_message(local_player: u8) -> Ending {
    Ending { group: MSG_VICTORY, from: 0, to: local_player, category: CATEGORY_ENDING, variant: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;
    use crate::unit::Unit;

    const T: &Tables = &Tables::DEFAULT;

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units) {
        let counties = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for r in realms.iter_mut().skip(1) {
            r.in_play = true;
            r.strength = 3;
        }
        (counties, realms, Units::default())
    }

    fn own(counties: &mut [County; MAX_COUNTIES], ids: &[usize], realm: u8) {
        for &id in ids {
            counties[id].owner = realm;
        }
    }

    fn army(units: &mut Units, owner: u8) {
        units.spawn(Unit::new(UnitKind::Army, owner, 0, 0));
    }

    // --- strength ----------------------------------------------------------

    #[test]
    fn strength_is_three_a_county_and_one_an_army() {
        let (mut counties, _, mut units) = world();
        own(&mut counties, &[1, 2, 3], 1);
        army(&mut units, 1);
        army(&mut units, 1);
        assert_eq!(strength(&counties, 14, &units, 1), 11);
    }

    #[test]
    fn only_armies_count_not_merchants_or_mobs() {
        let (counties, _, mut units) = world();
        for kind in [UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
            units.spawn(Unit::new(kind, 1, 0, 0));
        }
        assert_eq!(strength(&counties, 14, &units, 1), 0, "three units, none of them an army");
        army(&mut units, 1);
        assert_eq!(strength(&counties, 14, &units, 1), 1);
    }

    #[test]
    fn the_count_cannot_overflow_its_byte() {
        // Sixteen counties and a hundred and fifty armies is 198.
        let (mut counties, _, mut units) = world();
        own(&mut counties, &(1..=16).collect::<Vec<_>>(), 1);
        for _ in 1..=150 {
            army(&mut units, 1);
        }
        assert_eq!(strength(&counties, 16, &units, 1), 198);
    }

    // --- elimination -------------------------------------------------------

    #[test]
    fn a_realm_with_nothing_left_is_eliminated_and_an_ai_says_so() {
        let (counties, mut realms, units) = world();
        realms[2].is_human = false;
        let msg = recount_strength(&mut realms, &counties, 14, &units, 2, 1);
        assert!(!realms[2].in_play);
        assert_eq!(realms[2].strength, 0);
        assert_eq!(msg.map(|m| m.group), Some(MSG_AI_ELIMINATED));
        assert_eq!(msg.map(|m| m.from), Some(2));
    }

    #[test]
    fn the_local_player_gets_group_224_instead() {
        let (counties, mut realms, units) = world();
        realms[1].is_human = true;
        let msg = recount_strength(&mut realms, &counties, 14, &units, 1, 1);
        assert_eq!(msg.map(|m| m.group), Some(MSG_DEFEAT));
        // `from == to == me` is what makes `outcome_of` call it a loss.
        assert_eq!(msg.map(|m| (m.from, m.to)), Some((1, 1)));
    }

    /// The third arm of `FUN_0049B42B`: a human who is not the local player.
    #[test]
    fn a_second_human_in_a_network_game_is_told_nothing() {
        let (counties, mut realms, units) = world();
        realms[3].is_human = true;
        let msg = recount_strength(&mut realms, &counties, 14, &units, 3, 1);
        assert_eq!(msg, None, "eliminated in silence");
        assert!(!realms[3].in_play, "but eliminated all the same");
        assert_eq!(realms[3].voice_rotation, 1, "and the voice still rotates");
    }

    #[test]
    fn a_realm_that_still_holds_something_is_not_eliminated() {
        let (mut counties, mut realms, mut units) = world();
        own(&mut counties, &[4], 2);
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 2, 1), None);
        assert!(realms[2].in_play);
        assert_eq!(realms[2].strength, 3);

        // A realm with no counties but one army is still alive.
        realms[3].strength = 3;
        army(&mut units, 3);
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 3, 1), None);
        assert_eq!(realms[3].strength, 1);
    }

    #[test]
    fn an_already_eliminated_realm_is_skipped_entirely() {
        let (counties, mut realms, units) = world();
        assert!(recount_strength(&mut realms, &counties, 14, &units, 2, 1).is_some());
        let rotation = realms[2].voice_rotation;
        // Second time round the guard `strength != 0` fails and nothing happens.
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 2, 1), None);
        assert_eq!(realms[2].voice_rotation, rotation, "no second message, no second rotation");
    }

    #[test]
    fn realm_zero_and_out_of_range_are_refused() {
        let (counties, mut realms, units) = world();
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 0, 1), None);
        assert_eq!(recount_strength(&mut realms, &counties, 14, &units, 99, 1), None);
    }

    // --- ranking and crowning ----------------------------------------------

    #[test]
    fn opponents_remaining_excludes_the_local_player() {
        let (mut counties, mut realms, units) = world();
        own(&mut counties, &[1], 1);
        own(&mut counties, &[2], 2);
        own(&mut counties, &[3], 3);
        for id in 4..MAX_REALMS {
            realms[id].in_play = false;
            realms[id].strength = 0;
        }
        let _ = &units;
        let mut out = Vec::new();
        let r = rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!(r.realms_in_play, 3);
        assert_eq!(r.opponents_remaining, 2);
        assert!(!r.sole_survivor());
        assert!(out.is_empty());
    }

    #[test]
    fn one_realm_standing_and_it_is_the_human_is_a_victory() {
        let (_, mut realms, _) = world();
        for id in 2..MAX_REALMS {
            realms[id].in_play = false;
            realms[id].strength = 0;
        }
        realms[1].is_human = true;
        let mut out = Vec::new();
        let r = rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert!(r.sole_survivor());
        assert_eq!(r.leader, 1);
        assert_eq!(r.opponents_remaining, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_VICTORY);
        assert_eq!(out[0].to, 1);
        assert!(realms[1].crowned_once);
    }

    /// The one-shot guard, and what it does on the *second* call.
    #[test]
    fn a_lone_ai_taunts_once_and_then_hands_the_human_a_victory() {
        let (_, mut realms, _) = world();
        for id in 1..MAX_REALMS {
            realms[id].in_play = id == 3;
            realms[id].strength = if id == 3 { 3 } else { 0 };
        }
        realms[3].is_human = false;
        let mut out = Vec::new();
        rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_AI_CROWNED, "\"Just call me king.\"");
        assert_eq!(out[0].category, 1, "a taunt, so it cannot set an outcome");
        assert!(!out[0].sets_outcome());
        assert!(realms[3].crowned_once);

        // `Score_RankRealms` runs several times a turn. The guard is now set, so
        // the other branch fires and the *human* is sent group 225 — in a game
        // the human has already lost. Reproduced deliberately.
        out.clear();
        rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_VICTORY);
        assert!(out[0].sets_outcome());
    }

    /// The degenerate case the comparison lets through.
    #[test]
    fn with_nobody_in_play_the_leader_and_the_trailer_are_both_the_array_slot() {
        let (_, mut realms, _) = world();
        for id in 1..MAX_REALMS {
            realms[id].in_play = false;
            realms[id].strength = 0;
        }
        let mut out = Vec::new();
        let r = rank_and_crown(T, &mut realms, 1, Q, &mut out);
        assert_eq!((r.leader, r.trailer), (0, 0));
        assert!(r.sole_survivor(), "0 == 0, and the original crowns g_realms[0]");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].group, MSG_AI_CROWNED, "realm 0 is not human and not yet crowned");
        assert!(realms[0].crowned_once, "the array slot is written, exactly as it is there");
    }

    // --- the outcome -------------------------------------------------------

    #[test]
    fn group_225_is_a_win_whatever_else_is_true() {
        let msg = victory_message(1);
        for opponents in 0..=4 {
            let r = Ranking { opponents_remaining: opponents, ..Ranking::default() };
            assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::Set(Outcome::Won));
        }
    }

    #[test]
    fn my_own_elimination_with_opponents_left_is_a_loss() {
        let msg = Ending { group: MSG_DEFEAT, from: 1, to: 1, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 2, ..Ranking::default() };
        assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::Set(Outcome::Lost));
    }

    #[test]
    fn somebody_elses_elimination_ends_nothing() {
        let msg = Ending { group: MSG_AI_ELIMINATED, from: 3, to: 0, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 2, ..Ranking::default() };
        assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::Set(Outcome::InPlay));
    }

    /// The mainline human victory: it is this branch, not `Score_RankRealms`.
    #[test]
    fn the_last_ais_death_notice_with_no_opponents_left_enqueues_the_victory() {
        let msg = Ending { group: MSG_AI_ELIMINATED, from: 3, to: 0, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 0, ..Ranking::default() };
        assert_eq!(outcome_of(msg, 1, r, Q), OutcomeStep::EnqueueVictory);
        // …and that message is then a win.
        assert_eq!(
            outcome_of(victory_message(1), 1, r, Q),
            OutcomeStep::Set(Outcome::Won),
            "the enqueued 225 is what actually sets the outcome"
        );
    }

    /// Both realms die at once and the original calls it a win. Strange, and
    /// reproduced.
    #[test]
    fn dying_at_the_same_moment_as_the_last_opponent_is_scored_a_win() {
        let msg = Ending { group: MSG_DEFEAT, from: 1, to: 1, category: CATEGORY_ENDING, variant: 0 };
        let r = Ranking { opponents_remaining: 0, ..Ranking::default() };
        assert_eq!(
            outcome_of(msg, 1, r, Q),
            OutcomeStep::EnqueueVictory,
            "the opponents test is checked before the is-it-me test"
        );
    }

    #[test]
    fn the_outcome_bytes_are_ten_and_eleven() {
        assert_eq!(Outcome::InPlay.value(), 0);
        assert_eq!(Outcome::Won.value(), 10);
        assert_eq!(Outcome::Lost.value(), 11);
        for v in [0u8, 10, 11] {
            assert_eq!(Outcome::from_value(v).map(Outcome::value), Some(v));
        }
        assert_eq!(Outcome::from_value(1), None);
        assert!(!Outcome::InPlay.is_over());
        assert!(Outcome::Won.is_over() && Outcome::Lost.is_over());
    }
}
