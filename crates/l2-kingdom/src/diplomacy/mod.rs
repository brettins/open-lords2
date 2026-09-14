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
//! bug — no floating point.
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
//! instead of calling `Diplo_Post` when `g_multiplayer` is set, and
//!   `FUN_00448308` calls `Diplo_Post` on every peer. Replication is paused by
//!   instruction; the seam is [`post`] and nothing else.
//! * **Draw anything.** A [`Letter`] is a `Msg_Enqueue` record as a value; what
//!   shows it is `l2-game`'s business.

mod standing;
pub use standing::*;
mod inbox;
pub use inbox::*;
mod replies;
pub use replies::*;
mod ai;
pub use ai::*;
mod tests;
pub use tests::*;

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

/// The offence amounts, so the four call sites read as the rule.
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
/// A `Pcg32` of its own, for the reason in the module
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

