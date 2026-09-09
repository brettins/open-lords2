//! Diplomacy — the standing, the alliance, the inbox and the seven replies.
//!
//! `docs/diplomacy.md` is the document; this is the subsystem it traces. Every
//! function below names the original it reproduces (`CLAUDE.md` rule 5), and
//! the addresses are the GOG Windows build, `ImageBase 0x400000`.
//!
//! # Why this module existed as a hole for so long, and what it was blocking
//!
//! Four fields `crate::ai_army` reads have **exactly one writer each, and until
//! now it did not exist**:
//!
//! | field | its only writer |
//! |---|---|
//! | [`crate::realm::Pair::standing`] | [`init`], [`offend`], [`offend_all`], the seven replies |
//! | [`crate::realm::Realm::war_target`] | [`offend`] |
//! | [`crate::realm::Realm::ally`] | [`form_alliance`], [`break_alliance`], [`reconcile_alliances`] |
//! | [`crate::realm::Realm::target_county`] | [`pay_for_help`] |
//!
//! So AI step 10 — the raid — was implemented, dispatched, tested and could
//! never fire in a played game, because `pick_raid_victim` wants a standing
//! below −10 and nothing could ever put one there. `docs/decisions.md` C62
//! stated it and `crates/l2-game/tests/ai_war.rs` held a test written to go red
//! the day it stopped being true. This module is that day.
//!
//! # The two AI turn steps
//!
//! * **Step 1**, [`answer_inbox`] — `Diplo_AnswerInbox` (`0x004A277D`).
//! * **Step 2**, [`ai_diplomacy`] — `AI_Diplomacy` (`0x004A0C1D`).
//!
//! They are the last two of the fourteen `AI_RunTurnStep` dispatches; see
//! [`crate::ai::AiStep`].
//!
//! # Determinism
//!
//! `docs/netcode.md`. Every loop here is `for i in 1..MAX_REALMS` in ascending
//! index order — a diplomatic matrix walked in any other order is a lockstep
//! bug — and there is no floating point.
//!
//! **The dice are a separate stream, on purpose.** Three of the seven reply
//! handlers roll `g_rand7B` (`randStateB & 0x7F`, so 0..=127), and the original
//! draws them from a generator every other rule in the game also steps. We
//! cannot reproduce that stream, and folding these draws into
//! [`crate::Kingdom::rng`] would move the weather and the event deck of every
//! existing game. So [`Dice`] is its own `Pcg32`, stepped only here.
//!
//! **Nothing on the AI-to-AI path draws at all.** The three handlers that roll
//! are reached only from a realm's inbox, and in single player only a person
//! ever posts to one — so a game with no human diplomacy consumes no diplomatic
//! randomness whatever, and the forty-turn AI war stays a pure function of its
//! starting position.
//!
//! # What this module does not do
//!
//! * **Multiplayer.** `Diplo_SendClicked` issues net commands `0x48`/`0x49`
//!   instead of calling `Diplo_Post` when `g_multiplayer` is set, and
//!   `FUN_00448308` calls `Diplo_Post` on every peer. Replication is paused by
//!   instruction; the seam is [`post`] and nothing else.
//! * **Draw anything.** A [`Letter`] is a `Msg_Enqueue` record as a value; what
//!   shows it is `l2-game`'s business.

use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use l2_net::Pcg32;

// ---------------------------------------------------------------- §1 constants

/// The floor every standing write clamps to. `docs/diplomacy.md` §1. `[V]`
pub const STANDING_MIN: i8 = -30;
/// The ceiling every standing write clamps to.
pub const STANDING_MAX: i8 = 30;

/// The standing `Diplo_Init` gives an in-play AI realm's view of everyone.
/// A human's row, and a dead realm's, opens at 0.
pub const STANDING_START_AI: i8 = 5;

/// How many letters one realm's inbox holds. `g_diploInbox` is
/// 6 realms × **5** slots × 8 bytes at `0x0053F0F0`.
pub const INBOX_SLOTS: usize = 5;

/// `L2.eng` group ids, named because the group id *is* the message. Index 0 of
/// each is a label the game wrote about itself, quoted here.
pub mod group {
    /// *"Invasion of "* — an army enters its declared target county.
    pub const INVASION: u16 = 170;
    /// *"Reply to gift."* — pleased.
    pub const GIFT_PLEASED: u16 = 171;
    /// *"Reply to gift."* — grudging.
    pub const GIFT_GRUDGING: u16 = 172;
    /// *"Reply to gift."* — contemptuous.
    pub const GIFT_CONTEMPTUOUS: u16 = 173;
    /// *"Reply to compliment."* — the first one, worth +15.
    pub const COMPLIMENT_FIRST: u16 = 174;
    /// *"Reply to compliment."* — the second, worth +8.
    pub const COMPLIMENT_SECOND: u16 = 175;
    /// *"Reply to compliment."* — *"You are boring me now with your groveling
    /// letters. Do not send me any more."*
    pub const COMPLIMENT_ENOUGH: u16 = 176;
    /// *"Reply to Insult."* (the capital I is the game's.)
    pub const INSULT: u16 = 177;
    /// *"Reply to alliance offer."* — the refusal.
    pub const ALLIANCE_REFUSED: u16 = 178;
    /// *"Reply to alliance offer."* — the acceptance.
    pub const ALLIANCE_ACCEPTED: u16 = 179;
    /// *"Accept alliance ?"* — an AI courting a person, category `0x0B`.
    pub const ALLIANCE_OFFER: u16 = 180;
    /// *"End of alliance."* — step 2's grudge passed the lord's tolerance.
    pub const ALLIANCE_ENDED: u16 = 181;
    /// *"Broken alliance."* — an **act** broke one.
    pub const ALLIANCE_BROKEN: u16 = 182;
    /// *"Help in "* — refused.
    pub const HELP_REFUSED: u16 = 183;
    /// *"Help in "* — agreed, for nothing.
    pub const HELP_AGREED: u16 = 184;
    /// *"Pay -"* — agreed, for a price. Category 10, the prompt layout.
    pub const HELP_PRICED: u16 = 185;
    /// *"Attack of "* — refused.
    pub const ATTACK_REFUSED: u16 = 186;
    /// *"Attack of "* — agreed, for nothing.
    pub const ATTACK_AGREED: u16 = 187;
    /// *"Pay -"* — agreed, for a price.
    pub const ATTACK_PRICED: u16 = 188;
    /// *"Warning."* — the first.
    pub const WARNING_FIRST: u16 = 189;
    /// *"Warning."* — the second.
    pub const WARNING_SECOND: u16 = 190;
    /// *"Notice of revenge."* — the third, and war.
    pub const NOTICE_OF_REVENGE: u16 = 191;
    /// *"Retort to alliance offer."* — I am at war with you.
    pub const ALLIANCE_AT_WAR: u16 = 196;
    /// *"Reply to alliance offer."* — I already have an ally.
    pub const ALLIANCE_HAVE_ALLY: u16 = 197;

    // The refusals `Diplo_SendClicked` raises against the person composing.
    // Each is its own group and each has *"Message not sent."* at index 0.
    /// *"Already in alliance."* / *"You cannot ally with this player, my Lord,
    /// until they end their current treaty."*
    pub const ALREADY_IN_ALLIANCE: u16 = 219;
    /// *"You did not select a county, my Lord."*
    pub const NO_COUNTY_SELECTED: u16 = 240;
    /// *"Your requested county does not belong to anyone, my Lord."*
    pub const COUNTY_UNOWNED: u16 = 241;
    /// *"Your requested county does not belong to us, my Lord."*
    pub const COUNTY_NOT_OURS: u16 = 242;
    /// *"Our county does not have an enemy in it, my Lord. We cannot ask for
    /// help unless we are under threat."*
    pub const COUNTY_UNTHREATENED: u16 = 243;
    /// *"This county is part of our alliance, my Lord. We can only ask that
    /// those territories belonging to our enemies be attacked."*
    pub const COUNTY_IS_ALLIED: u16 = 244;
}

/// `Msg_Enqueue`'s `+0x11` category byte, which picks the window layout.
pub mod category {
    /// A plain lord's letter.
    pub const LETTER: u8 = 1;
    /// The pay-for-help prompt, answered by `Diplo_PayHelpClicked`.
    pub const PAY_PROMPT: u8 = 10;
    /// The *"Accept alliance ?"* prompt.
    pub const ALLIANCE_PROMPT: u8 = 0x0B;
    /// A system notice from the game itself — the composer's refusals.
    pub const SYSTEM: u8 = 0;
}

/// The offence amounts, so the four call sites read as the rule rather than as
/// a number. `docs/diplomacy.md` §5.
pub mod offence {
    /// Destroying an enemy supply transport. Gated by [`super::action_allowed`].
    pub const TRANSPORT: i8 = 5;
    /// Trampling a field in another realm's county — `Unit_CrossField`
    /// (`0x0046673C`), and **only when the trampling realm is human**.
    pub const FIELD: i8 = 10;
    /// Burning a dwelling — `Unit_BurnDwelling` (`0x00468AE2`). No human guard.
    pub const DWELLING: i8 = 20;
    /// Winning a battle — `Battle_ReturnToCampaign` (`0x004AB383`).
    pub const BATTLE: i8 = 20;
    /// What a broken alliance adds to whatever else the act cost, and what
    /// every third party's opinion of the betrayer drops by.
    pub const BETRAYAL: i8 = 15;
    /// Step 2's own charge when the grudge tops out.
    pub const GRUDGE_BREAK: i8 = 5;
}

// ------------------------------------------------------------------ §2 values

/// One `Msg_Enqueue` (`0x00472BC5`) record, as a value.
///
/// The original enqueues into a 50-slot ring and only when the recipient is
/// realm 0 or the local player; here the rule layer *returns* its letters and
/// the caller decides. That is the same shape [`crate::ai::Taunt`] already
/// uses, and for the same reason: a realm-to-realm letter is not a season
/// message and `l2-kingdom` has nothing to draw it with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Letter {
    /// `+0x04` — the sender. **0 is the game itself**, which is how the
    /// composer's own refusals are addressed.
    pub from: u8,
    /// `+0x00` — the recipient.
    pub to: u8,
    /// `+0x08` — the `L2.eng` group. A message *is* its group number.
    pub group: u16,
    /// `+0x0C` — `lord * 4 + rotation - 4`, which for lords 1..=4 is 0..=15 in
    /// four contiguous blocks of four. 0 for a system message.
    pub variant: u8,
    /// `+0x11` — see [`category`].
    pub category: u8,
    /// `+0x12` — a county id where the text needs one.
    pub county: u8,
    /// `+0x14` — a numeric payload.
    pub payload: i32,
}

impl Letter {
    /// A notice from the game to one realm: sender 0, variant 0, category 0.
    /// `Msg_Enqueue(0, to, group, 0, 0, 0, 0, 0)`.
    pub fn system(to: u8, group: u16) -> Letter {
        Letter {
            from: 0,
            to,
            group,
            variant: 0,
            category: category::SYSTEM,
            county: 0,
            payload: 0,
        }
    }
}

/// One of a realm's five inbox slots — `g_diploInbox + to * 0x28 + slot * 8`.
///
/// `from == 0` terminates the walk, and [`post`] fills the first free slot, so
/// the array stays dense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InboxSlot {
    /// `+0x00` — the sender realm. 0 means the slot is free.
    pub from: u8,
    /// `+0x01` — the message kind, 0..=6. See [`Kind`].
    pub kind: u8,
    /// `+0x02` — a county id, for kinds 5 and 6.
    pub county: u8,
    /// `+0x04` — gold, for kind 0.
    pub gold: i32,
}

/// The seven things a person can send, `L2.eng` group 72's menu in order: the
/// kind byte is `menuIndex - 2`. `docs/diplomacy.md` §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Kind {
    /// Group 72 index 2, *"Dispatch a gift."*
    Gift = 0,
    /// index 3, *"Send a compliment."*
    Compliment = 1,
    /// index 4, *"Send an insult."*
    Insult = 2,
    /// index 5, *"Offer an alliance."*
    OfferAlliance = 3,
    /// index 6, *"Terminate alliance."*
    EndAlliance = 4,
    /// index 7, *"Ask ally for help."*
    AskHelp = 5,
    /// index 8, *"Ask ally to attack."*
    AskAttack = 6,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Gift,
        Kind::Compliment,
        Kind::Insult,
        Kind::OfferAlliance,
        Kind::EndAlliance,
        Kind::AskHelp,
        Kind::AskAttack,
    ];

    pub fn from_byte(b: u8) -> Option<Kind> {
        Kind::ALL.get(b as usize).copied()
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// The index into `L2.eng` group 72 that names this kind on the diplomacy
    /// screen's menu — `kind + 2`.
    pub fn menu_index(self) -> usize {
        self as usize + 2
    }
}

/// **The diplomatic dice.** `g_rand7B` (`0x0058FD60`) is `randStateB & 0x7F`,
/// so 0..=127, and three reply handlers compare against it.
///
/// A `Pcg32` of its own rather than the kingdom's, for the reason in the module
/// documentation: the original's generator is stepped by every rule in the game
/// and ours is not, so sharing one would make a person's letter change the
/// weather.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dice(Pcg32);

impl Dice {
    /// Stream 0x0D1F is arbitrary and fixed; what matters is that it is not the
    /// stream [`crate::Kingdom::rng`] uses.
    pub fn from_seed(seed: u64) -> Dice {
        Dice(Pcg32::new(seed, 0x0D1F))
    }

    /// One draw of `g_rand7B`: 0..=127.
    pub fn rand7b(&mut self) -> i32 {
        (self.0.next_u32() & 0x7F) as i32
    }

    pub fn parts(&self) -> (u64, u64) {
        self.0.parts()
    }

    pub fn from_parts(state: u64, increment: u64) -> Dice {
        Dice(Pcg32::from_parts(state, increment))
    }
}

/// Everything diplomacy keeps outside the realm records.
///
/// Three globals, and they are three because the original has three: the inbox
/// itself, and the two the pay-for-help prompt is handed by the reply that
/// raised it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diplomacy {
    /// `g_diploInbox` (`0x0053F0F0`) — 6 × 5 slots. Index 0 is a realm slot
    /// nobody plays, like the realm array itself.
    pub inbox: [[InboxSlot; INBOX_SLOTS]; MAX_REALMS],
    /// `g_diploHelpPrice` (`0x00567958`) — what the last *"Pay -"* reply is
    /// asking for. Read by `Diplo_PayHelpClicked` when the person accepts.
    pub help_price: i32,
    /// `g_diploHelpCounty` — the county that price is for.
    pub help_county: u8,
    /// [`Dice`].
    pub dice: Dice,
}

impl Diplomacy {
    pub fn new(seed: u64) -> Diplomacy {
        Diplomacy {
            inbox: [[InboxSlot::default(); INBOX_SLOTS]; MAX_REALMS],
            help_price: 0,
            help_county: 0,
            dice: Dice::from_seed(seed),
        }
    }

    /// `Diplo_ClearInboxes` (`0x004A2584`) — zero all 6 × 5 slots.
    pub fn clear_inboxes(&mut self) {
        self.inbox = [[InboxSlot::default(); INBOX_SLOTS]; MAX_REALMS];
    }

    /// The filled slots of one realm's inbox, in slot order. A convenience for
    /// tests and for a caller that wants to draw the mail; the rule walks the
    /// array itself.
    pub fn pending(&self, realm: u8) -> impl Iterator<Item = &InboxSlot> {
        self.inbox[(realm as usize).min(MAX_REALMS - 1)].iter().take_while(|s| s.from != 0)
    }
}

// ------------------------------------------------------- §3 the small helpers

/// Clamp a standing to `[-30, +30]`, which every write site in the original
/// does inline.
fn clamp_standing(v: i8) -> i8 {
    v.clamp(STANDING_MIN, STANDING_MAX)
}

/// Add to `realms[me].pair(them).standing` and clamp.
fn move_standing(realms: &mut [Realm], me: u8, them: u8, delta: i8) {
    let p = realms[me as usize].pair_mut(them);
    p.standing = clamp_standing(p.standing.wrapping_add(delta));
}

/// `realm.voiceRotation` is advanced after **every** message a realm sends, so
/// the lord's four recorded takes cycle rather than repeat. This is the
/// `Msg_Enqueue` + rotation pair the original repeats at fourteen sites.
fn speak(realms: &mut [Realm], from: u8, to: u8, group: u16, cat: u8, county: u8) -> Letter {
    let realm = &mut realms[from as usize];
    let letter = Letter {
        from,
        to,
        group,
        variant: realm.message_variant(),
        category: cat,
        county,
        payload: 0,
    };
    realm.advance_voice();
    letter
}

// ----------------------------------------------------------------- §4 the init

/// `Diplo_Init` (`0x004A1C53`) — the new game.
///
/// **The one place every field of the pair record is written**, which is what
/// makes `docs/diplomacy.md` §1's field map a reading rather than a guess. An
/// in-play AI realm's opening view of *everyone* is [`STANDING_START_AI`]; a
/// human's and a dead realm's is 0.
///
/// Three details worth having in the model rather than tidied away:
///
/// * the inner loop runs `1..6` with **no `other != me` guard**, so a realm
///   ends up with an opinion of itself, at the same opening value;
/// * the opening standing is decided once per realm, from *that realm's* own
///   `strength` and `is_human` — not per pair — so an AI opens at 5 towards a
///   human as well, and a human opens at 0 towards everybody;
/// * `help_price_multiple` opens at **1**, and it is the one field whose zero
///   would be wrong: a never-initialised record would price military help at
///   nothing.
pub fn init(realms: &mut [Realm], diplomacy: &mut Diplomacy) {
    diplomacy.help_price = 0;
    diplomacy.help_county = 0;
    diplomacy.clear_inboxes();
    for me in 1..MAX_REALMS.min(realms.len()) {
        let opening =
            if realms[me].strength == 0 || realms[me].is_human { 0 } else { STANDING_START_AI };
        realms[me].war_target = 0;
        realms[me].ally = 0;
        for other in 1..MAX_REALMS {
            let p = realms[me].pair_mut(other as u8);
            *p = crate::realm::Pair::new();
            p.standing = opening;
        }
    }
}

/// `Diplo_DefaultTarget` (`0x004A1E6C`) — the first in-play realm that is not
/// the local player, or 0. It is what the diplomacy screen opens on.
pub fn default_target(realms: &[Realm], local_player: u8) -> u8 {
    for id in 1..MAX_REALMS.min(realms.len()) {
        if realms[id].strength != 0 && local_player as usize != id {
            return id as u8;
        }
    }
    0
}

// ------------------------------------------------------------ §5 the alliance

/// `Diplo_FormAlliance` (`0x004A1774`) — **six bytes, and that is the whole of
/// what an alliance is**: `allied` and a cleared `grudge` in both directions,
/// and `ally` on both sides.
///
/// It is not shared vision, not shared victory and not an automatic call to
/// arms. What it buys is the two request kinds, an icon on the map and the lord
/// card, and [`action_allowed`] returning false — which suppresses the offence
/// hook and charges grudge instead.
///
/// **Exclusivity is not a rule written anywhere; it is the width of the
/// field.** `realm +0x81` is one byte.
pub fn form_alliance(realms: &mut [Realm], a: u8, b: u8) {
    realms[a as usize].pair_mut(b).allied = true;
    realms[a as usize].pair_mut(b).grudge = 0;
    realms[b as usize].pair_mut(a).allied = true;
    realms[b as usize].pair_mut(a).grudge = 0;
    realms[a as usize].ally = b;
    realms[b as usize].ally = a;
}

/// `Diplo_BreakAlliance` (`0x004A1A54`) — clear `allied` and `ally` on
/// whichever sides point at the other.
///
/// **It does not touch standing.** Every caller applies its own penalty, and
/// they differ: an insult costs 20, ending it from the screen costs 15, and
/// step 2's grudge break goes through [`offend`] for 5.
///
/// The two halves are guarded separately — `if realms[a].ally == b` and
/// `if realms[b].ally == a` — so a one-sided pairing is cleared on the side
/// that holds it and left alone on the side that does not.
pub fn break_alliance(realms: &mut [Realm], a: u8, b: u8) {
    if realms[a as usize].ally == b {
        realms[a as usize].pair_mut(b).allied = false;
        realms[a as usize].ally = 0;
    }
    if realms[b as usize].ally == a {
        realms[b as usize].pair_mut(a).allied = false;
        realms[b as usize].ally = 0;
    }
}

/// `Diplo_ReconcileAlliances` (`0x004A1847`), called from `Turn_Tick`.
///
/// > **This corrects `docs/diplomacy.md` §4.1.** The document says it *"drops
/// > any pairing that is one-sided or whose partner has been eliminated"*. It
/// > does not: a pairing where the partner points at **nobody** is **repaired**
/// > — the function writes `ally` back on the partner and sets `allied` both
/// > ways. Only a partner pointing at a *third* realm, or a partner already
/// > marked dead, drops it. So the function's job is to make the matrix agree
/// > with the `ally` bytes, and its tie-break favours the lower index.
///
/// The dead-partner test reads a `handled` array the same loop is filling, in
/// ascending order, so it can only ever see realms **below** the one being
/// walked. A realm allied to a *higher*-indexed eliminated realm is therefore
/// not dropped here at all — the partner still points back, so the else branch
/// takes it and the alliance survives. `docs/bugs.md`. Reproduced.
pub fn reconcile_alliances(realms: &mut [Realm]) {
    let n = MAX_REALMS.min(realms.len());
    for me in 1..n {
        for other in 1..MAX_REALMS {
            realms[me].pair_mut(other as u8).allied = false;
        }
    }
    let mut handled = [false; MAX_REALMS];
    let mut ally: [u8; MAX_REALMS] = [0; MAX_REALMS];
    for id in 1..n {
        ally[id] = realms[id].ally;
    }
    for me in 1..n {
        if realms[me].strength == 0 {
            handled[me] = true;
            continue;
        }
        let partner = ally[me] as usize;
        if partner == 0 || partner >= MAX_REALMS {
            continue;
        }
        if handled[partner] || (ally[partner] != 0 && ally[partner] as usize != me) {
            realms[me].ally = 0;
            realms[me].pair_mut(partner as u8).allied = false;
            handled[me] = true;
            ally[me] = 0;
        } else {
            realms[me].ally = partner as u8;
            realms[partner].ally = me as u8;
            realms[me].pair_mut(partner as u8).allied = true;
            realms[partner].pair_mut(me as u8).allied = true;
        }
    }
}

/// `Diplo_ActionAllowed` (`0x004A16F7`) — **not a predicate.**
///
/// It answers *"does this act count against me?"* and, when the answer is no
/// because the target is the actor's own ally, it **charges the actor a point
/// of grudge against that ally on the way out**. A target of 0 is allowed;
/// acting on yourself is not.
///
/// `docs/bugs.md` B74: both AI county choosers call this once per county and
/// two of the mission searches call it once per unit slot, so an AI hemmed in
/// by its ally accumulates grudge purely by looking at the map — and the
/// Knight's tolerance of 5 is reached in a handful of turns. **Reproduced**;
/// whether it is intended is not established.
pub fn action_allowed(realms: &mut [Realm], actor: u8, target: u8) -> bool {
    if target == 0 {
        return true;
    }
    if realms[actor as usize].ally == target {
        let p = realms[actor as usize].pair_mut(target);
        p.grudge = p.grudge.wrapping_add(1);
        return false;
    }
    actor != target
}

// ------------------------------------------------------------- §6 the offence

/// `Diplo_Offend(offended, offender, amount)` (`0x004A1EE1`) — **the single
/// hook every relationship-damaging act goes through.**
///
/// ```text
/// if the offender was my ally:
///       break the alliance
///       set pair.atWar and my warTarget      -- UNLESS I am the Bishop
///       offend_all(offender, 15)             -- everyone else's opinion drops 15
///       send group 182 "Broken alliance."
///       amount += 15
///
/// standing -= amount, clamped to [-30, +30]
///
/// if not already at war, standing has bottomed out at -30, and the offender is
/// HUMAN:  warningsSent 0->1: group 189; 1->2: group 190; 2->3: group 191 and war
/// ```
///
/// Three things the document does not say, all of them from the guard chain:
///
/// 1. **The whole function is a no-op when the offended realm is human.**
///    `g_realms[offended].isHuman == 0` is in the entry guard, beside the range
///    checks. A person's realm keeps no standing towards anybody, which is why
///    `Diplo_Init` opens a human's row at 0 and why nothing ever moves it.
///    Everything that reads a standing reads an AI's.
/// 2. **The Bishop's guard covers `warTarget` as well as `atWar`.**
///    `docs/diplomacy.md` §5 says the test *"guards only the `atWar` write"*;
///    the `atWar` store is a comma-expression inside the same `&&` chain as the
///    `warTarget == 0` test, so a betrayed Bishop sets **neither**. He breaks
///    the alliance, takes the standing hit, sends group 182 and stays
///    technically at peace with a realm he has not even resolved to attack.
/// 3. **Betraying an ally is the only act in the game that costs reputation
///    with third parties** — [`offend_all`] is reached from nowhere else.
///
/// Returns the letters. The offence amounts are in [`offence`].
pub fn offend(realms: &mut [Realm], offended: u8, offender: u8, amount: i8) -> Vec<Letter> {
    let mut out = Vec::new();
    let n = MAX_REALMS.min(realms.len()) as u8;
    if offended == 0
        || offended >= n
        || offender == 0
        || offender >= n
        || offender == offended
        || realms[offended as usize].strength == 0
        || realms[offended as usize].is_human
    {
        return out;
    }
    let mut amount = amount;
    if realms[offender as usize].ally == offended {
        break_alliance(realms, offended, offender);
        if realms[offended as usize].lord != crate::realm::LORD_BISHOP {
            realms[offended as usize].pair_mut(offender).at_war = true;
            if realms[offended as usize].war_target == 0 {
                realms[offended as usize].war_target = offender;
            }
        }
        offend_all(realms, offender, offence::BETRAYAL);
        out.push(speak(
            realms,
            offended,
            offender,
            group::ALLIANCE_BROKEN,
            category::LETTER,
            0,
        ));
        amount = amount.wrapping_add(offence::BETRAYAL);
    }
    move_standing(realms, offended, offender, amount.wrapping_neg());

    let pair = *realms[offended as usize].pair(offender);
    if pair.at_war || pair.standing != STANDING_MIN || !realms[offender as usize].is_human {
        return out;
    }
    let group = match pair.warnings_sent {
        0 => group::WARNING_FIRST,
        1 => group::WARNING_SECOND,
        2 => group::NOTICE_OF_REVENGE,
        _ => return out,
    };
    realms[offended as usize].pair_mut(offender).warnings_sent = pair.warnings_sent + 1;
    if group == group::NOTICE_OF_REVENGE {
        realms[offended as usize].pair_mut(offender).at_war = true;
        if realms[offended as usize].war_target == 0 {
            realms[offended as usize].war_target = offender;
        }
    }
    out.push(speak(realms, offended, offender, group, category::LETTER, 0));
    out
}

/// `Diplo_OffendAll` (`0x004A24D1`) — lower **every other realm's** standing
/// towards this one.
///
/// It clamps at the floor only, walks 1..5 with no in-play test, and is reached
/// from exactly one place: [`offend`]'s betrayal branch.
pub fn offend_all(realms: &mut [Realm], realm: u8, amount: i8) {
    for id in 1..MAX_REALMS.min(realms.len()) {
        if id as u8 == realm {
            continue;
        }
        let p = realms[id].pair_mut(realm);
        p.standing = p.standing.wrapping_sub(amount).max(STANDING_MIN);
    }
}

// --------------------------------------------------------------- §7 the inbox

/// `Diplo_Post` (`0x004A2621`) — **the only writer of the inbox**, and the
/// player's whole outgoing side in single player.
///
/// It fills the first free of five slots, bumps the recipient's
/// `compliments_from` when the kind is a compliment, sets `has_mail`, and — if
/// the gold is non-zero — **moves it immediately**, clamped to what the sender
/// actually holds.
///
/// **A gift is spent when it is posted, not when it is answered.** The reply
/// arrives a turn later and could be an insult; the money has gone either way.
///
/// A full inbox silently drops the letter, which is the original's behaviour
/// and not a softening of it: the loop returns when it runs off the fifth slot.
pub fn post(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    from: u8,
    to: u8,
    kind: Kind,
    gold: i32,
    county: u8,
) {
    let to_i = (to as usize).min(MAX_REALMS - 1);
    let Some(slot) = diplomacy.inbox[to_i].iter_mut().find(|s| s.from == 0) else { return };
    slot.from = from;
    slot.kind = kind.byte();
    slot.county = county;
    slot.gold = gold;
    if kind == Kind::Compliment {
        let p = realms[to_i].pair_mut(from);
        p.compliments_from = p.compliments_from.wrapping_add(1);
    }
    realms[to_i].pair_mut(from).has_mail = true;
    if gold == 0 {
        return;
    }
    let gold = gold.min(realms[from as usize].gold);
    realms[from as usize].gold -= gold;
    realms[to_i].gold += gold;
}

/// **AI turn step 1** — `Diplo_AnswerInbox` (`0x004A277D`).
///
/// Walks the five slots, dispatches on the kind byte, then zeroes all five
/// slots *and* all six `has_mail` bytes. **A realm's inbox is emptied every
/// turn whether or not it was full**, and a letter from the realm itself is
/// skipped without being answered.
///
/// Because a human realm's AI turn is skipped entirely (`realm +0x05`), a
/// letter posted to a person's inbox would never be answered. In single player
/// nothing posts one; in multiplayer the send goes through the netcode instead.
pub fn answer_inbox(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    realm: u8,
) -> Vec<Letter> {
    let mut out = Vec::new();
    let me = (realm as usize).min(MAX_REALMS - 1);
    for slot in 0..INBOX_SLOTS {
        let entry = diplomacy.inbox[me][slot];
        if entry.from == 0 {
            break;
        }
        if entry.from == realm {
            continue;
        }
        let Some(kind) = Kind::from_byte(entry.kind) else { continue };
        out.extend(match kind {
            Kind::Gift => reply_gift(realms, tables, realm, entry.from, entry.gold),
            Kind::Compliment => reply_compliment(realms, realm, entry.from),
            Kind::Insult => reply_insult(realms, realm, entry.from),
            Kind::OfferAlliance => {
                reply_alliance_offer(realms, diplomacy, realm, entry.from, realms_active(realms))
            }
            Kind::EndAlliance => reply_alliance_end(realms, realm, entry.from),
            Kind::AskHelp => {
                reply_help_request(realms, diplomacy, tables, realm, entry.from, entry.county)
            }
            Kind::AskAttack => {
                reply_attack_request(realms, diplomacy, tables, realm, entry.from, entry.county)
            }
        });
    }
    diplomacy.inbox[me] = [InboxSlot::default(); INBOX_SLOTS];
    for other in 0..MAX_REALMS {
        realms[me].pair_mut(other as u8).has_mail = false;
    }
    out
}

/// `g_realmsActive` (`0x00554004`) as *"realms still in play"*.
///
/// **The original has two writers with two meanings** — one counts realms still
/// in play and one counts realms that have not yet finished their turn, and the
/// second runs every frame of phase 4, so whichever wrote last is what the
/// diplomacy code reads. `docs/diplomacy.md` §9 leaves it unresolved and
/// nothing here depends on resolving it: both readings mean *"the game is
/// nearly over"* at `< 3`, and both branches that test it refuse either way.
pub fn realms_active(realms: &[Realm]) -> usize {
    realms.iter().skip(1).take(MAX_REALMS - 1).filter(|r| r.strength != 0).count()
}

// ----------------------------------------------- §8 the seven reply handlers

/// Kind 0 — `Diplo_ReplyGift` (`0x004A29B2`).
///
/// Three tiers against `bestGift + T/2` and `bestGift + T`, where `T` is the
/// lord's [`crate::tables::AI_PERSONALITY_GIFT_INCREMENT`]:
///
/// | gift | reply | standing |
/// |---|---|---:|
/// | `< best + T/2` | group 173 | **−8** |
/// | `< best + T` | group 172 | **+5** |
/// | `>= best + T` | group 171 | **+10** |
///
/// **The bar ratchets.** `best = max(best, gift)` runs *after* the tier is
/// chosen and never falls, so every gift is judged against the largest you have
/// ever sent and the second must beat the first. A gift under half the
/// increment **costs** 8, which is more than the best gift gains.
///
/// The Bishop's `T` is 50 against the Countess's 200, so he is the cheapest
/// lord to buy — and he is also the one with the fattest cheat income.
pub fn reply_gift(
    realms: &mut [Realm],
    tables: &Tables,
    me: u8,
    them: u8,
    gold: i32,
) -> Vec<Letter> {
    if !realms[them as usize].is_human {
        return Vec::new();
    }
    let Some(p) = tables.ai_personality(realms[me as usize].lord) else { return Vec::new() };
    let best = realms[me as usize].pair(them).best_gift;
    let increment = p.gift_increment;
    let (group, delta) = if gold < best + increment / 2 {
        (group::GIFT_CONTEMPTUOUS, -8)
    } else if gold < best + increment {
        (group::GIFT_GRUDGING, 5)
    } else {
        (group::GIFT_PLEASED, 10)
    };
    if best < gold {
        realms[me as usize].pair_mut(them).best_gift = gold;
    }
    move_standing(realms, me, them, delta);
    vec![speak(realms, me, them, group, category::LETTER, 0)]
}

/// Kind 1 — `Diplo_ReplyCompliment` (`0x004A2CD6`).
///
/// On `compliments_from`, which [`post`] incremented before this ran and which
/// **nothing ever resets**: 1 → group 174 and **+15**, 2 → group 175 and
/// **+8**, 3 or more → group 176 and **−4**.
///
/// So compliments are worth +23 in total, once, and cost 4 apiece for ever
/// after — and group 176 says the same in words: *"You are boring me now with
/// your groveling letters. Do not send me any more."*
pub fn reply_compliment(realms: &mut [Realm], me: u8, them: u8) -> Vec<Letter> {
    if !realms[them as usize].is_human {
        return Vec::new();
    }
    let n = realms[me as usize].pair(them).compliments_from;
    let (group, delta) = match n {
        0 | 1 => (group::COMPLIMENT_FIRST, 15),
        2 => (group::COMPLIMENT_SECOND, 8),
        _ => (group::COMPLIMENT_ENOUGH, -4),
    };
    move_standing(realms, me, them, delta);
    vec![speak(realms, me, them, group, category::LETTER, 0)]
}

/// Kind 2 — `Diplo_ReplyInsult` (`0x004A2F61`). Breaks any alliance between the
/// two, costs **−20**, and replies with group 177.
pub fn reply_insult(realms: &mut [Realm], me: u8, them: u8) -> Vec<Letter> {
    if !realms[them as usize].is_human {
        return Vec::new();
    }
    break_alliance(realms, me, them);
    move_standing(realms, me, them, -20);
    vec![speak(realms, me, them, group::INSULT, category::LETTER, 0)]
}

/// Kind 3 — `Diplo_ReplyAllianceOffer` (`0x004A30F6`).
///
/// ```text
/// if pair.atWar                    reply 196 "Retort";        standing -1
/// else if realmsActive < 3         reply 178;                  standing -2
/// else if I already have an ally   reply 197;                  standing -1
/// else if they already have one    reply 178;                  standing -2
/// else if standing >= 11           reply 179;  ALLY;           standing +4
/// else if standing < -10           reply 178;                  standing -2
/// else if rand7B < standing*3 + 45 reply 179;  ALLY;           standing +4
/// else                             reply 178;                  standing -2
/// ```
///
/// **The two break points are exactly the lord card's three colour bands.**
/// `Diplo_DrawLordCard` fills the thermometer `0xFA` at ≥ +11, `0xFC` between,
/// `0xF9` at ≤ −11 — the UI's bands and the AI's branches are the same numbers
/// written by different code, which is the best corroboration in
/// `docs/diplomacy.md` that the pair block is what it looks like.
///
/// The middle band runs from about **12 %** at standing −10 to about **59 %**
/// at +10, hinging on 45/128 ≈ 35 % at 0.
///
/// Two details not in the document. **It decrements the answering realm's own
/// `offer_timer`** on every offer it fields, whatever the answer — so being
/// courted by a person pushes back the turn that realm next courts somebody
/// itself. And it has **no `is_human` guard**, unlike the three handlers above
/// it, so it would answer an AI too; nothing in single player posts one.
pub fn reply_alliance_offer(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    me: u8,
    them: u8,
    realms_active: usize,
) -> Vec<Letter> {
    let standing = realms[me as usize].pair(them).standing;
    let group = if realms[me as usize].pair(them).at_war {
        group::ALLIANCE_AT_WAR
    } else if realms_active < 3 {
        group::ALLIANCE_REFUSED
    } else if realms[me as usize].ally != 0 {
        group::ALLIANCE_HAVE_ALLY
    } else if realms[them as usize].ally != 0 {
        group::ALLIANCE_REFUSED
    } else if standing >= 11 {
        group::ALLIANCE_ACCEPTED
    } else if standing < -10 {
        group::ALLIANCE_REFUSED
    } else if diplomacy.dice.rand7b() < standing as i32 * 3 + 45 {
        group::ALLIANCE_ACCEPTED
    } else {
        group::ALLIANCE_REFUSED
    };
    if realms[me as usize].offer_timer != 0 {
        realms[me as usize].offer_timer -= 1;
    }
    match group {
        group::ALLIANCE_REFUSED => move_standing(realms, me, them, -2),
        group::ALLIANCE_ACCEPTED => {
            move_standing(realms, me, them, 4);
            form_alliance(realms, me, them);
        }
        _ => move_standing(realms, me, them, -1),
    }
    vec![speak(realms, me, them, group, category::LETTER, 0)]
}

/// Kind 4 — `Diplo_ReplyAllianceEnd` (`0x004A3475`). Breaks the alliance, costs
/// **−15**, and **sends nothing at all**: terminating an alliance from the
/// diplomacy screen is silent.
///
/// The *"Broken alliance."* text, group 182, is not this — it comes from
/// [`offend`] when an *act* rather than a letter breaks one.
///
/// Like kind 3 it has no `is_human` guard, so an AI's termination would also be
/// honoured; and unlike kind 3 there is nothing to send either way.
pub fn reply_alliance_end(realms: &mut [Realm], me: u8, them: u8) -> Vec<Letter> {
    break_alliance(realms, me, them);
    move_standing(realms, me, them, -15);
    Vec::new()
}

/// Kind 5 — `Diplo_ReplyHelpRequest` (`0x004A3551`).
///
/// ```text
/// if they are not my ally                            refuse
/// else if my mean population < personality[+0x30]    refuse
/// else if standing < 10                              refuse
/// else if rand7B < standing * 4                      accept, and march for free
/// else                                               "pay me"
/// ```
///
/// > **`personality +0x30` is a population floor, not a treasury one.**
/// > `docs/diplomacy.md` §3.5 calls it *"a treasury floor … below which an ally
/// > will not move at all"*; the comparison is against realm `+0x14`, the mean
/// > population of the realm's counties, which is
/// > [`crate::realm::Realm::population_mean`]. §8.4's own note that the same
/// > field is *"also a county-population floor at step 9"* is the
/// > corroboration: it is a population floor in both places.
///
/// A refusal adds **+1 grudge** — the lord resents being asked. Accepting calls
/// [`pay_for_help`] with a price of **zero**, so the ally's `target_county`
/// becomes the county and it marches. *Pay me* sets [`Diplomacy::help_price`]
/// to `personality[+0x0C] × helpPriceMultiple` and raises a category-10 prompt;
/// clicking it runs [`pay_for_help`] for real.
pub fn reply_help_request(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    me: u8,
    them: u8,
    county: u8,
) -> Vec<Letter> {
    reply_request(
        realms,
        diplomacy,
        tables,
        me,
        them,
        county,
        RequestShape {
            refused: group::HELP_REFUSED,
            agreed: group::HELP_AGREED,
            priced: group::HELP_PRICED,
            odds: 4,
            grudge: 1,
        },
    )
}

/// Kind 6 — `Diplo_ReplyAttackRequest` (`0x004A38DB`). The same shape as
/// [`reply_help_request`] with three constants moved: groups 186/187/188, a
/// **+2** grudge on refusal, and **half** the acceptance odds (`standing × 2`).
///
/// Asking an ally to attack somebody is twice as annoying and half as likely to
/// work as asking for help at home.
pub fn reply_attack_request(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    me: u8,
    them: u8,
    county: u8,
) -> Vec<Letter> {
    reply_request(
        realms,
        diplomacy,
        tables,
        me,
        them,
        county,
        RequestShape {
            refused: group::ATTACK_REFUSED,
            agreed: group::ATTACK_AGREED,
            priced: group::ATTACK_PRICED,
            odds: 2,
            grudge: 2,
        },
    )
}

/// The three constants that separate kinds 5 and 6. The original is two
/// near-identical 900-byte functions; keeping them as one shape with a
/// parameter block makes the difference between them the *only* thing written
/// down, which is the point of §3.5's table.
struct RequestShape {
    refused: u16,
    agreed: u16,
    priced: u16,
    odds: i32,
    grudge: u8,
}

fn reply_request(
    realms: &mut [Realm],
    diplomacy: &mut Diplomacy,
    tables: &Tables,
    me: u8,
    them: u8,
    county: u8,
    shape: RequestShape,
) -> Vec<Letter> {
    let Some(p) = tables.ai_personality(realms[me as usize].lord) else { return Vec::new() };
    let standing = realms[me as usize].pair(them).standing as i32;
    let group = if realms[me as usize].ally != them {
        shape.refused
    } else if realms[me as usize].population_mean < p.help_population_floor {
        shape.refused
    } else if standing < 10 {
        shape.refused
    } else if diplomacy.dice.rand7b() < standing * shape.odds {
        shape.agreed
    } else {
        shape.priced
    };
    if group == shape.refused {
        let pair = realms[me as usize].pair_mut(them);
        pair.grudge = pair.grudge.wrapping_add(shape.grudge);
        return vec![speak(realms, me, them, group, category::LETTER, county)];
    }
    if group == shape.agreed {
        let letter = speak(realms, me, them, group, category::LETTER, county);
        pay_for_help(realms, me, them, county, 0);
        return vec![letter];
    }
    diplomacy.help_county = county;
    diplomacy.help_price =
        p.help_price * realms[me as usize].pair(them).help_price_multiple as i32;
    vec![speak(realms, me, them, group, category::PAY_PROMPT, county)]
}

/// `Diplo_PayForHelp(ally, payer, county, price)` (`0x004A1B29`).
///
/// Moves the price from the payer to the ally **if the payer can afford it**,
/// increments the pair's `help_price_multiple` so the next purchase costs more,
/// takes the ally's opinion of the payer **down 4** for having made them do it,
/// and points the ally's [`crate::realm::Realm::target_county`] at the county.
///
/// The multiple starts at 1 and never falls, so help **doubles, trebles,
/// quadruples**: the Knight's 500 becomes 1,000 then 1,500, and the Countess's
/// 1,600 becomes 3,200.
///
/// **If the payer cannot afford it, nothing at all happens** — not the march,
/// not the multiple, not the standing. The whole body is inside the test.
pub fn pay_for_help(realms: &mut [Realm], ally: u8, payer: u8, county: u8, price: i32) {
    if price > realms[payer as usize].gold {
        return;
    }
    realms[payer as usize].gold -= price;
    realms[ally as usize].gold += price;
    let p = realms[ally as usize].pair_mut(payer);
    p.help_price_multiple = p.help_price_multiple.wrapping_add(1);
    p.standing = p.standing.wrapping_sub(4).max(STANDING_MIN);
    realms[ally as usize].target_county = county;
}

// ------------------------------------------------------------ §9 AI turn step 2

/// **AI turn step 2** — `AI_Diplomacy` (`0x004A0C1D`), the real diplomacy
/// driver, and the writer of three of the four starved fields.
///
/// It does three things in order.
///
/// **One: standing heals — but not towards you.**
///
/// ```c
/// for (other = 1; other < 6; other++)
///     if (!realms[other].isHuman)
///         pair[me][other].standing = min(standing + 1, +30);
/// ```
///
/// The guard is on the **other** realm being non-human, so **an AI's standing
/// towards a person never drifts back up**. Damage you do is permanent; damage
/// the AIs do to each other heals at a point a turn. There is no `other != me`
/// guard and no in-play test either, so a realm also heals towards itself and
/// towards the dead.
///
/// **Two: an alliance decays on its own.** An eliminated ally is dropped and
/// the function **returns immediately**, skipping the courtship below for that
/// turn. Otherwise the grudge accumulates:
///
/// | condition | grudge |
/// |---|---:|
/// | my standing towards the ally < −10 | **+10** |
/// | `realmsActive < 3` | **+25** |
/// | the ally is the ranked leader and holds ≥ 21 % of the map | **+1** |
///
/// and past the lord's [`crate::tables::AI_PERSONALITY_GRUDGE_TOLERANCE`] —
/// Knight **5**, Baron 10, Countess 15, Bishop 20 — the alliance breaks,
/// [`offend`] fires for 5 and group 181 goes out. Note the order: the alliance
/// is broken *before* `offend` runs, so `offend`'s betrayal branch cannot see
/// it and the extra 15, the `atWar` flag and [`offend_all`] never happen. A
/// grudge break is much cheaper than a betrayal.
///
/// > **Two of the three envy tiers are unreachable.** The ally-is-winning test
/// > is `if (v < 0x15) { if (v < 0x22) { if (0x32 < v) +4 } else +2 } else +1`.
/// > `v < 0x15` implies `v < 0x22`, which makes `+2` dead, and implies
/// > `!(0x32 < v)`, which makes `+4` dead. Only `v >= 21 → +1` can fire. The
/// > ladder was written with its comparisons the wrong way round.
/// > `docs/bugs.md`; reproduced by writing only the rung that can fire.
///
/// **Three: if not allied, the AI goes courting.** From year **1269**, and only
/// if the realm is not ranked first, [`pick_ally_candidate`] chooses; a counter
/// must then reach the lord's [`crate::tables::AI_PERSONALITY_OFFER_INTERVAL`]
/// — Knight 12, Baron 10, Countess 8, Bishop **4** turns — before it acts.
/// Against another AI it simply forms the alliance with no message; against a
/// person it sets `offer_pending` and sends group 180 with the prompt layout.
///
/// The timer only advances on a turn there **is** a candidate, and it is reset
/// to 0 when it fires whether or not the offer went anywhere — a human who
/// already has an ally is passed over silently and still costs the courtier its
/// full interval.
pub fn ai_diplomacy(
    realms: &mut [Realm],
    tables: &Tables,
    me: u8,
    year: i32,
    rank_leader: u8,
) -> Vec<Letter> {
    let mut out = Vec::new();
    let n = MAX_REALMS.min(realms.len());
    for other in 1..n {
        if realms[other].is_human {
            continue;
        }
        let p = realms[me as usize].pair_mut(other as u8);
        p.standing = p.standing.wrapping_add(1).min(STANDING_MAX);
    }

    let active = realms_active(realms);
    let ally = realms[me as usize].ally;
    if ally != 0 {
        let tolerance =
            tables.ai_personality(realms[me as usize].lord).map(|p| p.grudge_tolerance);
        if realms[ally as usize].strength == 0 {
            break_alliance(realms, me, ally);
            return out;
        }
        if realms[me as usize].pair(ally).standing < -10 {
            bump_grudge(realms, me, ally, 10);
        }
        if active < 3 {
            bump_grudge(realms, me, ally, 25);
        }
        // The one reachable rung of the envy ladder.
        if rank_leader == ally && realms[ally as usize].share_of_map_pct >= 0x15 {
            bump_grudge(realms, me, ally, 1);
        }
        if let Some(tolerance) = tolerance {
            if (realms[me as usize].pair(ally).grudge as i32) > tolerance {
                break_alliance(realms, me, ally);
                out.extend(offend(realms, me, ally, offence::GRUDGE_BREAK));
                out.push(speak(
                    realms,
                    me,
                    ally,
                    group::ALLIANCE_ENDED,
                    category::LETTER,
                    0,
                ));
            }
        }
    }

    if realms[me as usize].ally != 0 || year <= 1268 || realms[me as usize].rank == 1 {
        return out;
    }
    let candidate = pick_ally_candidate(realms, me);
    realms[me as usize].ally_candidate = candidate;
    if candidate == 0 {
        return out;
    }
    realms[me as usize].offer_timer = realms[me as usize].offer_timer.saturating_add(1);
    let Some(interval) = tables.ai_personality(realms[me as usize].lord).map(|p| p.offer_interval)
    else {
        return out;
    };
    if (realms[me as usize].offer_timer as i32) < interval {
        return out;
    }
    realms[me as usize].offer_timer = 0;
    if !realms[candidate as usize].is_human {
        form_alliance(realms, me, candidate);
    } else if realms[candidate as usize].ally == 0 {
        realms[me as usize].offer_pending = true;
        out.push(speak(
            realms,
            me,
            candidate,
            group::ALLIANCE_OFFER,
            category::ALLIANCE_PROMPT,
            0,
        ));
    }
    out
}

fn bump_grudge(realms: &mut [Realm], me: u8, them: u8, by: u8) {
    let p = realms[me as usize].pair_mut(them);
    p.grudge = p.grudge.wrapping_add(by);
}

/// `Diplo_PickAllyCandidate` (`0x004A1241`) — the **best-ranked** realm that is
/// in play, not ranked first, unallied, not already being courted by somebody
/// else, not at war with me, and whose standing with me is above −10. 0 for
/// none.
///
/// *"Not already being courted"* is `offer_pending` on the candidate, which is
/// what stops two AIs writing to the same person in the same turn. *"Not ranked
/// first"* on both sides is the game's whole diplomatic logic in one line: the
/// leader is nobody's friend and makes no friends.
///
/// Ties go to the lower index, because the comparison is strict `<` and the
/// walk is ascending.
pub fn pick_ally_candidate(realms: &[Realm], me: u8) -> u8 {
    let mut best = 0u8;
    let mut best_rank = 6u8;
    for id in 1..MAX_REALMS.min(realms.len()) {
        let r = &realms[id];
        if r.strength == 0
            || id as u8 == me
            || r.rank == 1
            || r.ally != 0
            || r.offer_pending
            || realms[me as usize].pair(id as u8).standing <= -11
            || realms[me as usize].pair(id as u8).at_war
            || r.rank >= best_rank
        {
            continue;
        }
        best = id as u8;
        best_rank = r.rank;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realm::{LORD_BISHOP, LORD_ELIMINATED};

    fn world() -> ([Realm; MAX_REALMS], Diplomacy) {
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for (id, r) in realms.iter_mut().enumerate().take(MAX_REALMS).skip(1) {
            r.in_play = true;
            r.strength = 3;
            r.lord = ((id - 1) % 4 + 1) as u8;
            r.rank = id as u8;
            r.gold = 5000;
            r.population_mean = 4000;
        }
        realms[1].is_human = true;
        realms[1].lord = crate::realm::LORD_HUMAN;
        let mut d = Diplomacy::new(7);
        init(&mut realms, &mut d);
        (realms, d)
    }

    /// `Diplo_Init` gives an in-play AI realm 5 towards **everyone**, a human 0
    /// towards everyone, and opens the help multiple at 1.
    #[test]
    fn the_opening_standing_is_the_realms_own_and_not_the_pairs() {
        let (realms, _) = world();
        for other in 1..MAX_REALMS {
            assert_eq!(realms[1].pair(other as u8).standing, 0, "the human's row is all zeros");
            assert_eq!(realms[2].pair(other as u8).standing, 5, "an AI opens at 5 on everyone");
            assert_eq!(realms[2].pair(other as u8).help_price_multiple, 1);
        }
        assert_eq!(realms[2].pair(2).standing, 5, "including itself: no other != me guard");
    }

    /// The whole hook is skipped when the offended realm is a person — the
    /// `isHuman == 0` clause is in `Diplo_Offend`'s entry guard.
    #[test]
    fn a_person_takes_no_offence_because_the_guard_refuses_to_let_them() {
        let (mut realms, _) = world();
        let letters = offend(&mut realms, 1, 2, offence::BATTLE);
        assert!(letters.is_empty());
        assert_eq!(realms[1].pair(2).standing, 0, "unchanged, and it was 0 to begin with");

        let letters = offend(&mut realms, 2, 1, offence::BATTLE);
        assert!(letters.is_empty(), "no warning yet: standing has not bottomed out");
        assert_eq!(realms[2].pair(1).standing, 5 - 20);
    }

    /// Two warnings, then war — and only against a person.
    #[test]
    fn the_warning_ladder_runs_out_at_the_third_and_declares_war() {
        let (mut realms, _) = world();
        realms[2].pair_mut(1).standing = STANDING_MIN;
        let groups = |ls: Vec<Letter>| ls.iter().map(|l| l.group).collect::<Vec<_>>();
        assert_eq!(groups(offend(&mut realms, 2, 1, 1)), vec![group::WARNING_FIRST]);
        assert_eq!(groups(offend(&mut realms, 2, 1, 1)), vec![group::WARNING_SECOND]);
        assert!(!realms[2].pair(1).at_war);
        assert_eq!(groups(offend(&mut realms, 2, 1, 1)), vec![group::NOTICE_OF_REVENGE]);
        assert!(realms[2].pair(1).at_war, "the third is war");
        assert_eq!(realms[2].war_target, 1, "and it names the target");
        assert!(offend(&mut realms, 2, 1, 1).is_empty(), "and there is no fourth");
    }

    /// The betrayal branch, and the Bishop's exemption from **both** writes.
    #[test]
    fn a_betrayed_bishop_declares_neither_war_nor_a_target() {
        let (mut realms, _) = world();
        realms[2].lord = LORD_BISHOP;
        form_alliance(&mut realms, 2, 3);
        let letters = offend(&mut realms, 2, 3, offence::DWELLING);
        assert_eq!(letters[0].group, group::ALLIANCE_BROKEN);
        assert!(!realms[2].pair(3).at_war, "the Bishop stays technically at peace");
        assert_eq!(realms[2].war_target, 0, "and does not even name a target");
        assert_eq!(realms[2].ally, 0, "but the alliance is gone");

        let (mut realms, _) = world();
        realms[2].lord = 1;
        form_alliance(&mut realms, 2, 3);
        offend(&mut realms, 2, 3, offence::DWELLING);
        assert!(realms[2].pair(3).at_war, "every other lord flags a war");
        assert_eq!(realms[2].war_target, 3);
    }

    /// Betrayal is the only act that costs reputation with third parties.
    #[test]
    fn betraying_an_ally_costs_fifteen_with_everybody_else() {
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 3);
        offend(&mut realms, 2, 3, offence::BATTLE);
        assert_eq!(realms[4].pair(3).standing, 5 - 15, "realm 4 heard about it");
        assert_eq!(realms[5].pair(3).standing, 5 - 15);
        assert_eq!(realms[2].pair(3).standing, STANDING_MIN, "20 + 15 from 5, clamped");
    }

    /// The gift ratchet: the second gift is judged against the first.
    #[test]
    fn a_gift_is_judged_against_the_largest_you_have_ever_sent() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        realms[2].lord = 1; // Knight, increment 100
        assert_eq!(reply_gift(&mut realms, &t, 2, 1, 100)[0].group, group::GIFT_PLEASED);
        assert_eq!(realms[2].pair(1).standing, 15);
        assert_eq!(realms[2].pair(1).best_gift, 100);
        // The same gift again is now under best + T/2 and costs eight.
        assert_eq!(reply_gift(&mut realms, &t, 2, 1, 100)[0].group, group::GIFT_CONTEMPTUOUS);
        assert_eq!(realms[2].pair(1).standing, 7);
        assert_eq!(reply_gift(&mut realms, &t, 2, 1, 200)[0].group, group::GIFT_PLEASED);
    }

    /// Three compliments and you have overdone it, permanently.
    #[test]
    fn the_third_compliment_and_every_one_after_it_costs_four() {
        let (mut realms, mut d) = world();
        let t = Tables::DEFAULT;
        for expected in [group::COMPLIMENT_FIRST, group::COMPLIMENT_SECOND] {
            post(&mut realms, &mut d, 1, 2, Kind::Compliment, 0, 0);
            let letters = answer_inbox(&mut realms, &mut d, &t, 2);
            assert_eq!(letters[0].group, expected);
        }
        assert_eq!(realms[2].pair(1).standing, 5 + 15 + 8);
        for _ in 0..3 {
            post(&mut realms, &mut d, 1, 2, Kind::Compliment, 0, 0);
            let letters = answer_inbox(&mut realms, &mut d, &t, 2);
            assert_eq!(letters[0].group, group::COMPLIMENT_ENOUGH);
        }
        assert_eq!(realms[2].pair(1).standing, 5 + 15 + 8 - 12, "and it never resets");
    }

    /// A gift is spent when it is posted, not when it is answered — and it is
    /// clamped to what the sender actually holds.
    #[test]
    fn the_gold_moves_at_the_post_office() {
        let (mut realms, mut d) = world();
        realms[1].gold = 300;
        post(&mut realms, &mut d, 1, 2, Kind::Gift, 1000, 0);
        assert_eq!(realms[1].gold, 0, "clamped to what was there");
        assert_eq!(realms[2].gold, 5000 + 300);
        assert!(realms[2].pair(1).has_mail);
    }

    /// The inbox is five deep, dense, and emptied every turn whether or not it
    /// was full.
    #[test]
    fn the_inbox_holds_five_and_the_sixth_letter_is_lost() {
        let (mut realms, mut d) = world();
        for _ in 0..6 {
            post(&mut realms, &mut d, 1, 2, Kind::Compliment, 0, 0);
        }
        assert_eq!(d.pending(2).count(), 5);
        assert_eq!(
            realms[2].pair(1).compliments_from,
            5,
            "and the sixth is lost outright: `Diplo_Post` returns off the end of \
             the slot walk before it counts the compliment, sets `has_mail` or \
             moves any gold — so a gift into a full inbox is not even spent"
        );
        answer_inbox(&mut realms, &mut d, &Tables::DEFAULT, 2);
        assert_eq!(d.pending(2).count(), 0);
        assert!(!realms[2].pair(1).has_mail);
    }

    /// The alliance offer's ladder, at the two break points the lord card
    /// draws.
    #[test]
    fn eleven_accepts_outright_and_minus_eleven_refuses_outright() {
        let (mut realms, mut d) = world();
        realms[2].pair_mut(1).standing = 11;
        assert_eq!(
            reply_alliance_offer(&mut realms, &mut d, 2, 1, 5)[0].group,
            group::ALLIANCE_ACCEPTED
        );
        assert_eq!(realms[2].ally, 1);
        assert_eq!(realms[2].pair(1).standing, 15);

        let (mut realms, mut d) = world();
        realms[2].pair_mut(1).standing = -11;
        assert_eq!(
            reply_alliance_offer(&mut realms, &mut d, 2, 1, 5)[0].group,
            group::ALLIANCE_REFUSED
        );
        assert_eq!(realms[2].ally, 0);
    }

    /// At war is a permanent bar, and it is answered with a different group
    /// from the ordinary refusal.
    #[test]
    fn war_blocks_an_alliance_at_any_standing_at_all() {
        let (mut realms, mut d) = world();
        realms[2].pair_mut(1).standing = STANDING_MAX;
        realms[2].pair_mut(1).at_war = true;
        let l = reply_alliance_offer(&mut realms, &mut d, 2, 1, 5);
        assert_eq!(l[0].group, group::ALLIANCE_AT_WAR, "\"Retort to alliance offer.\"");
        assert_eq!(realms[2].ally, 0);
        assert_eq!(realms[2].pair(1).standing, STANDING_MAX - 1, "a retort costs one, not two");
    }

    /// **Step 2's heal is asymmetric**, and it is the sharpest single fact in
    /// the subsystem: damage a person does never heals.
    #[test]
    fn an_ai_forgives_another_ai_and_never_forgives_a_person() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        realms[2].pair_mut(1).standing = -20;
        realms[2].pair_mut(3).standing = -20;
        for _ in 0..10 {
            ai_diplomacy(&mut realms, &t, 2, 1268, 0);
        }
        assert_eq!(realms[2].pair(1).standing, -20, "the human is realm 1");
        assert_eq!(realms[2].pair(3).standing, -10, "and realm 3 is not");
    }

    /// Courtship: not before 1269, not while ranked first, and only after the
    /// lord's own interval of turns.
    #[test]
    fn an_ai_courts_the_best_ranked_realm_it_can_reach_after_its_lords_interval() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        realms[2].lord = LORD_BISHOP; // interval 4, the shortest
        realms[2].rank = 3;
        for _ in 0..8 {
            ai_diplomacy(&mut realms, &t, 2, 1268, 0);
        }
        assert_eq!(realms[2].ally, 0, "1268 is too early");
        for turn in 1..=4 {
            ai_diplomacy(&mut realms, &t, 2, 1269, 0);
            if turn < 4 {
                assert_eq!(realms[2].ally, 0, "the interval has not run out");
            }
        }
        assert_ne!(realms[2].ally, 0, "and on the fourth it acts");
        assert_ne!(realms[2].ally, 1, "never the person: that goes through group 180");
    }

    /// An eliminated ally drops the alliance and **returns**, so the courtship
    /// below never runs that turn.
    #[test]
    fn a_dead_ally_ends_the_step_as_well_as_the_alliance() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        realms[2].rank = 3;
        form_alliance(&mut realms, 2, 3);
        realms[3].strength = 0;
        realms[3].lord = LORD_ELIMINATED;
        ai_diplomacy(&mut realms, &t, 2, 1300, 0);
        assert_eq!(realms[2].ally, 0);
        assert_eq!(realms[2].ally_candidate, 0, "the courtship did not run");
    }

    /// The grudge break is much cheaper than a betrayal, because the alliance
    /// is already gone by the time `offend` looks at it.
    #[test]
    fn a_grudge_break_costs_five_and_not_twenty() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        realms[2].lord = 1; // Knight, tolerance 5
        realms[2].rank = 3;
        form_alliance(&mut realms, 2, 3);
        realms[2].pair_mut(3).standing = -20;
        let before = realms[4].pair(3).standing;
        let letters = ai_diplomacy(&mut realms, &t, 2, 1300, 0);
        assert_eq!(realms[2].ally, 0);
        assert_eq!(letters.last().unwrap().group, group::ALLIANCE_ENDED);
        // −20, healed to −19 by the step's own first pass, then −5.
        assert_eq!(realms[2].pair(3).standing, -24, "five, not five plus fifteen");
        assert_eq!(realms[4].pair(3).standing, before, "and nobody else hears about it");
    }

    /// `Diplo_ActionAllowed` charges the asker a point of grudge every time it
    /// says no. `docs/bugs.md` B74, reproduced.
    #[test]
    fn asking_whether_an_act_against_an_ally_counts_corrodes_the_alliance() {
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 3);
        for _ in 0..6 {
            assert!(!action_allowed(&mut realms, 2, 3));
        }
        assert_eq!(realms[2].pair(3).grudge, 6, "six looks at the map, six grudge");
        assert!(action_allowed(&mut realms, 2, 4), "a non-ally is allowed and costs nothing");
        assert!(action_allowed(&mut realms, 2, 0), "and target 0 always is");
        assert!(!action_allowed(&mut realms, 2, 2), "acting on yourself is not");
    }

    /// A one-sided alliance is **repaired**, not dropped — which is the
    /// opposite of what `docs/diplomacy.md` §4.1 said.
    #[test]
    fn reconciling_repairs_a_one_sided_pairing_and_drops_a_contested_one() {
        let (mut realms, _) = world();
        realms[2].ally = 3;
        reconcile_alliances(&mut realms);
        assert_eq!(realms[3].ally, 2, "repaired");
        assert!(realms[2].pair(3).allied && realms[3].pair(2).allied);

        let (mut realms, _) = world();
        realms[2].ally = 3;
        realms[3].ally = 4;
        realms[4].ally = 3;
        reconcile_alliances(&mut realms);
        assert_eq!(realms[2].ally, 0, "realm 3 was already spoken for");
        assert_eq!(realms[3].ally, 4);
    }

    /// Help is a request, and the price of it doubles every time.
    #[test]
    fn the_price_of_help_ratchets_and_the_asking_costs_standing() {
        let (mut realms, _) = world();
        realms[2].lord = 1; // Knight, base price 500
        form_alliance(&mut realms, 2, 1);
        realms[2].pair_mut(1).standing = 20;
        realms[1].gold = 10_000;
        pay_for_help(&mut realms, 2, 1, 4, 500);
        assert_eq!(realms[1].gold, 9500);
        assert_eq!(realms[2].gold, 5500);
        assert_eq!(realms[2].target_county, 4, "and the ally marches on it");
        assert_eq!(realms[2].pair(1).help_price_multiple, 2);
        assert_eq!(realms[2].pair(1).standing, 16, "minus four for having been made to");

        realms[1].gold = 100;
        pay_for_help(&mut realms, 2, 1, 5, 1000);
        assert_eq!(realms[1].gold, 100, "cannot afford it, so nothing happens at all");
        assert_eq!(realms[2].target_county, 4);
        assert_eq!(realms[2].pair(1).help_price_multiple, 2);
    }

    /// An ally with too few people refuses whatever the standing — and the
    /// floor is a population, not a treasury.
    #[test]
    fn a_thinly_peopled_ally_will_not_march_at_any_standing() {
        let (mut realms, mut d) = world();
        let t = Tables::DEFAULT;
        realms[2].lord = 1; // Knight, floor 750
        form_alliance(&mut realms, 2, 1);
        realms[2].pair_mut(1).standing = STANDING_MAX;
        realms[2].population_mean = 749;
        realms[2].gold = 1_000_000;
        let l = reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
        assert_eq!(l[0].group, group::HELP_REFUSED, "gold is not what is asked about");
        assert_eq!(realms[2].pair(1).grudge, 1);
        realms[2].population_mean = 750;
        let l = reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
        assert_ne!(l[0].group, group::HELP_REFUSED);
    }

    /// Asking an ally to attack is twice as annoying as asking for help.
    #[test]
    fn an_attack_request_refused_costs_two_grudge_where_help_costs_one() {
        let (mut realms, mut d) = world();
        let t = Tables::DEFAULT;
        reply_help_request(&mut realms, &mut d, &t, 2, 1, 4);
        assert_eq!(realms[2].pair(1).grudge, 1, "not even my ally");
        reply_attack_request(&mut realms, &mut d, &t, 2, 1, 4);
        assert_eq!(realms[2].pair(1).grudge, 3);
    }

    /// Ending an alliance from the screen is silent and costs fifteen.
    #[test]
    fn terminating_an_alliance_sends_nothing_at_all() {
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 1);
        let letters = reply_alliance_end(&mut realms, 2, 1);
        assert!(letters.is_empty(), "\"Broken alliance.\" is the other path");
        assert_eq!(realms[2].ally, 0);
        assert_eq!(realms[2].pair(1).standing, 5 - 15);
    }

    /// Every message advances the sender's voice rotation, so the lord's four
    /// recorded takes cycle rather than repeat.
    #[test]
    fn the_four_recorded_takes_cycle() {
        let (mut realms, _) = world();
        realms[2].lord = 2; // Baron: variants 4..=7
        let mut seen = Vec::new();
        for _ in 0..5 {
            seen.push(reply_insult(&mut realms, 2, 1)[0].variant);
        }
        assert_eq!(seen, vec![4, 5, 6, 7, 4]);
    }

    /// The whole point of the module: a played turn writes the four fields the
    /// raid handler starves without.
    #[test]
    fn step_two_writes_a_standing_a_war_target_and_an_ally() {
        let (mut realms, _) = world();
        let t = Tables::DEFAULT;
        // A standing, from the heal.
        realms[2].pair_mut(3).standing = 0;
        ai_diplomacy(&mut realms, &t, 2, 1268, 0);
        assert_eq!(realms[2].pair(3).standing, 1);
        // An ally, from the courtship.
        realms[2].rank = 3;
        realms[2].lord = LORD_BISHOP;
        for _ in 0..4 {
            ai_diplomacy(&mut realms, &t, 2, 1269, 0);
        }
        assert_ne!(realms[2].ally, 0);
        // A war target, from an act.
        let (mut realms, _) = world();
        form_alliance(&mut realms, 2, 3);
        offend(&mut realms, 2, 3, offence::BATTLE);
        assert_eq!(realms[2].war_target, 3);
        // And a target county, from a paid request.
        pay_for_help(&mut realms, 2, 3, 9, 0);
        assert_eq!(realms[2].target_county, 9);
    }
}
