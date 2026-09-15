//! `docs/diplomacy.md` is the document; this is the subsystem it traces. Every
//! function below names the original it reproduces (`CLAUDE.md` rule 5), and
//! the addresses are the GOG Windows build, `ImageBase 0x400000`.
//!
//! So AI step 10 — the raid — was implemented, dispatched, tested and could
//! never fire in a played game, because `pick_raid_victim` wants a standing
//! below −10 and nothing could ever put one there. `docs/decisions.md` C62
//! stated it and `crates/l2-game/tests/ai_war/main.rs` held a test written to go red
//! the day it stopped being true. This module is that day.
//!
//! * **Step 1**, [`answer_inbox`] — `Diplo_AnswerInbox` (`0x004A277D`).
//!
//! * **Step 2**, [`ai_diplomacy`] — `AI_Diplomacy` (`0x004A0C1D`).
//!
//! * **Multiplayer.** `Diplo_SendClicked` issues net commands `0x48`/`0x49`
//! instead of calling `Diplo_Post` when `g_multiplayer` is set, and
//!   `FUN_00448308` calls `Diplo_Post` on every peer. Replication is paused by
//!   instruction; the seam is [`post`] and nothing else.

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


/// The floor every standing write clamps to. `docs/diplomacy.md` §1. `[V]`
pub const STANDING_MIN: i8 = -30;
pub const STANDING_MAX: i8 = 30;

pub const STANDING_START_AI: i8 = 5;

/// How many letters one realm's inbox holds. `g_diploInbox` is
/// 6 realms × **5** slots × 8 bytes at `0x0053F0F0`.
pub const INBOX_SLOTS: usize = 5;

/// `L2.eng` group ids, named because the group id *is* the message. Index 0 of
/// each is a label the game wrote about itself, quoted here.
pub mod group {
    pub const INVASION: u16 = 170;
    pub const GIFT_PLEASED: u16 = 171;
    pub const GIFT_GRUDGING: u16 = 172;
    pub const GIFT_CONTEMPTUOUS: u16 = 173;
    pub const COMPLIMENT_FIRST: u16 = 174;
    pub const COMPLIMENT_SECOND: u16 = 175;
    pub const COMPLIMENT_ENOUGH: u16 = 176;
    pub const INSULT: u16 = 177;
    pub const ALLIANCE_REFUSED: u16 = 178;
    pub const ALLIANCE_ACCEPTED: u16 = 179;
    pub const ALLIANCE_OFFER: u16 = 180;
    pub const ALLIANCE_ENDED: u16 = 181;
    pub const ALLIANCE_BROKEN: u16 = 182;
    pub const HELP_REFUSED: u16 = 183;
    pub const HELP_AGREED: u16 = 184;
    pub const HELP_PRICED: u16 = 185;
    pub const ATTACK_REFUSED: u16 = 186;
    pub const ATTACK_AGREED: u16 = 187;
    pub const ATTACK_PRICED: u16 = 188;
    pub const WARNING_FIRST: u16 = 189;
    pub const WARNING_SECOND: u16 = 190;
    pub const NOTICE_OF_REVENGE: u16 = 191;
    pub const ALLIANCE_AT_WAR: u16 = 196;
    pub const ALLIANCE_HAVE_ALLY: u16 = 197;

    pub const ALREADY_IN_ALLIANCE: u16 = 219;
    pub const NO_COUNTY_SELECTED: u16 = 240;
    pub const COUNTY_UNOWNED: u16 = 241;
    pub const COUNTY_NOT_OURS: u16 = 242;
    pub const COUNTY_UNTHREATENED: u16 = 243;
    pub const COUNTY_IS_ALLIED: u16 = 244;
}

/// `Msg_Enqueue`'s `+0x11` category byte, which picks the window layout.
pub mod category {
    pub const LETTER: u8 = 1;
    pub const PAY_PROMPT: u8 = 10;
    pub const ALLIANCE_PROMPT: u8 = 0x0B;
    pub const SYSTEM: u8 = 0;
}

pub mod offence {
    pub const TRANSPORT: i8 = 5;
    /// Trampling a field in another realm's county — `Unit_CrossField`
    /// (`0x0046673C`), and **only when the trampling realm is human**.
    pub const FIELD: i8 = 10;
    /// Burning a dwelling — `Unit_BurnDwelling` (`0x00468AE2`). No human guard.
    pub const DWELLING: i8 = 20;
    /// Winning a battle — `Battle_ReturnToCampaign` (`0x004AB383`).
    pub const BATTLE: i8 = 20;
    pub const BETRAYAL: i8 = 15;
    pub const GRUDGE_BREAK: i8 = 5;
}


/// One `Msg_Enqueue` (`0x00472BC5`) record, as a value.
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
    Gift = 0,
    Compliment = 1,
    Insult = 2,
    OfferAlliance = 3,
    EndAlliance = 4,
    AskHelp = 5,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dice(Pcg32);

impl Dice {
    /// Stream 0x0D1F is arbitrary and fixed; what matters is that it is not the
    /// stream [`crate::Kingdom::rng`] uses.
    pub fn from_seed(seed: u64) -> Dice {
        Dice(Pcg32::new(seed, 0x0D1F))
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diplomacy {
    /// `g_diploInbox` (`0x0053F0F0`) — 6 × 5 slots. Index 0 is a realm slot
    /// nobody plays, like the realm array itself.
    pub inbox: [[InboxSlot; INBOX_SLOTS]; MAX_REALMS],
    /// `g_diploHelpPrice` (`0x00567958`) — what the last *"Pay -"* reply is
    /// asking for. Read by `Diplo_PayHelpClicked` when the person accepts.
    pub help_price: i32,
    pub help_county: u8,
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

    pub fn pending(&self, realm: u8) -> impl Iterator<Item = &InboxSlot> {
        self.inbox[(realm as usize).min(MAX_REALMS - 1)].iter().take_while(|s| s.from != 0)
    }
}

