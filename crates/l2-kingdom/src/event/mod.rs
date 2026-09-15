//! Random events — `docs/kingdom.md` §8.1, `Event_RollAll` (`0x00448819`).
//!
//! `docs/kingdom.md` §8.1 describes `g_eventTable` (`0x004D6108`) as the source
//! of *"one of 24 handlers (ids `0x87` … `0x8E` and `0x12E` … `0x13D`)"*, and
//! this crate previously carried a placeholder catalogue of five because the
//! table's contents were unknown. They are known now, and the table's *shape*
//! is not what the phrase "24-entry event table" suggests.
//!
//! **It is a 256-slot deck of `i16` event ids, 230 of them zero.** A zero slot
//! means "no event", and the 26 non-zero slots hold the 24 distinct ids — two
//! of them appear twice. Every non-zero slot sits at an index `≡ 7 (mod 8)`, so
//! the deck is really 32 groups of eight with at most one event in each group's
//! last slot. **`[V]`**, and three independent things close it:
//!
//! * 256 `i16` is 512 bytes, and `0x004D6108 + 512` is exactly `0x004D6308`,
//!   where `g_birthRateLadder` begins;
//! * the 24 distinct ids are exactly the 24 the dispatch tests, with no id in
//!   the table the dispatch cannot handle and no handler the table cannot
//!   reach;
//! * **each id is its own `L2.eng` group number**, and every one of the 24
//!   groups describes, in prose, what the decompiled handler does.
//!
//!   That last one is the strong evidence: id `0x13B` = group 315 is *"Stop
//!   thief!.  Highwaymen waylay your tax collectors. Lose all tax revenues this
//! season."*, and the handler's whole body sets county `+0x1A8`, which is the
//!   *"another untraced gate"* `docs/kingdom.md` §4.1 says zeroes the tax take.
//!
//! That last one is arithmetic. The seed is
//! `g_seasonRandom * 2`, which is always **even**; the index the wrap resets to
//! is 0, which is also even, so the parity survives the wrap; county `k`
//! therefore always lands on a slot of parity `k`. And every dealt slot is at
//! `index ≡ 7 (mod 8)`, which is **odd**. So counties 2, 4, 6 … 16 are
//! permanently exempt from the random-event table, and no amount of play will
//! reveal it, because the counties that do draw look
//! The seed is `00448822 MOV EAX,[0x0058FD60] / ADD EAX,EAX` in the
//! disassembly, so the doubling is not a decompiler artefact
//! (`docs/decisions.md` C13). Reproduced, and asserted
//! exhaustively over all 128 seeds by
//! `only_odd_numbered_counties_can_ever_draw_an_event`.
//!
//! Most write an `i8` percentage into county `+0x1FB`, `+0x1FC` or `+0x1FD`,
//! which the population, grain and herd passes then apply — and several of
//! those percentages **depend on the season**, which `docs/kingdom.md` does not
//! mention. The rest move happiness, the health meter, a stockpile, the
//! treasury, a field or the tax gate directly.
//!
//! | | function | |
//! |---|---|---|
//! | roll, store, dispatch | `Event_RollAll` | `0x00448819`, once a season, [`roll_all`] |
//! | the 24 handlers | `FUN_00448E12` … `FUN_00449826` | a failing guard clears both flags, [`fire`] |
//! | **post the letter** | `Event_Post` (`FUN_00448D7E`) | `0x00448D7E`, once a **frame**, for `g_selectedCounty` — `l2_game::message::post_event` |
//! | draw it | `Msg_DrawWindow` category `0x0F` | `0x0047309E`, `l2_game::screens::message::draw_event` |
//! | clear `eventFired` | `Event_Post` again | and **nothing at all clears `eventId`** |

mod deck;
pub use deck::*;
mod effects;
pub use effects::*;
mod tests;
pub use tests::*;

use crate::county::County;
use crate::report::Message;
use crate::tables::{Season, Tables};
use l2_net::{Pcg32, Quirk, Quirks};

/// *"No bull"* writes 99 into the herd modifier and the herd pass reads it as
/// **"no growth at all this season"**. `[V]` —
/// `FUN_0044D60D`'s first branch is `if (mod == 'c') { change = 0; births = 0; }`
/// and `L2.eng` group 314 is *"Cattle will not reproduce this season due to the
/// death of your prize bull. Deaths, however, occur normally."*
pub const HERD_NO_GROWTH: i32 = 99;

/// A random event: the `L2.eng` group that names it, which is also the id the
/// deck stores and the dispatch switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum EventKind {
    Rats = 0x87,
    MadCows = 0x88,
    Wolves = 0x89,
    Plague = 0x8A,
    GrainFound = 0x8B,
    BadCattle = 0x8C,
    CowBonanza = 0x8D,
    WeddingFever = 0x8E,
    HealthyEating = 0x12E,
    MotherNature = 0x12F,
    WeaponsFound = 0x130,
    Donation = 0x131,
    Treasure = 0x132,
    HolyRelic = 0x133,
    Witch = 0x134,
    StoneFound = 0x135,
    Locusts = 0x136,
    HagsCurse = 0x137,
    Fraud = 0x138,
    Corruption = 0x139,
    NoBull = 0x13A,
    StopThief = 0x13B,
    Termites = 0x13C,
    NoSongs = 0x13D,
}

pub const EVENT_KINDS: [EventKind; 24] = [
    EventKind::Rats,
    EventKind::MadCows,
    EventKind::Wolves,
    EventKind::Plague,
    EventKind::GrainFound,
    EventKind::BadCattle,
    EventKind::CowBonanza,
    EventKind::WeddingFever,
    EventKind::HealthyEating,
    EventKind::MotherNature,
    EventKind::WeaponsFound,
    EventKind::Donation,
    EventKind::Treasure,
    EventKind::HolyRelic,
    EventKind::Witch,
    EventKind::StoneFound,
    EventKind::Locusts,
    EventKind::HagsCurse,
    EventKind::Fraud,
    EventKind::Corruption,
    EventKind::NoBull,
    EventKind::StopThief,
    EventKind::Termites,
    EventKind::NoSongs,
];

impl EventKind {
}
/// `g_eventTable` (`0x004D6108`) — the 256-slot deck, as `(slot, id)` pairs for
/// the 26 non-zero slots. Every other slot is "no event".
pub const EVENT_DECK: [(usize, EventKind); 26] = [
    (7, EventKind::Rats),
    (23, EventKind::BadCattle),
    (31, EventKind::CowBonanza),
    (39, EventKind::Plague),
    (55, EventKind::GrainFound),
    (63, EventKind::Wolves),
    (79, EventKind::WeddingFever),
    (87, EventKind::GrainFound),
    (103, EventKind::HealthyEating),
    (111, EventKind::Fraud),
    (119, EventKind::MotherNature),
    (127, EventKind::NoBull),
    (135, EventKind::WeaponsFound),
    (143, EventKind::MadCows),
    (151, EventKind::Donation),
    (159, EventKind::Plague),
    (167, EventKind::HolyRelic),
    (183, EventKind::Witch),
    (191, EventKind::StoneFound),
    (199, EventKind::Locusts),
    (207, EventKind::HagsCurse),
    (215, EventKind::Corruption),
    (231, EventKind::StopThief),
    (239, EventKind::Termites),
    (247, EventKind::Treasure),
    (255, EventKind::NoSongs),
];

pub const EVENT_DECK_SLOTS: usize = 256;

/// The seasonal draw is `LFSR & 0x7F`, so 0..=127, and the deck index starts at
/// twice it. `[V]` — `FUN_00404A46` publishes six masked values from two
/// 31-bit LFSRs and `Event_RollAll` reads the `& 0x7F` one.
pub const EVENT_SEED_BOUND: u32 = 128;

pub fn deck_slot(slot: usize) -> Option<EventKind> {
    let slot = slot % EVENT_DECK_SLOTS;
    let mut i = 0;
    while i < EVENT_DECK.len() {
        if EVENT_DECK[i].0 == slot {
            return Some(EVENT_DECK[i].1);
        }
        i += 1;
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Write an `i8` percentage into county `+0x1FB`. `Population_UpdateAll`
    /// takes it **of the season's deaths** (negative) **or births** (positive),
    /// adds ten, and caps the result at 20% of the county —
    /// [`crate::county::County::event_population_swing`].
    PopulationPct(i32),
    /// County `+0x1FC`, applied to the grain store by `Grain_SeasonTick`.
    GrainPct(i32),
    /// County `+0x1FD`, applied to the herd by `Herd_SeasonTick`.
    HerdPct(i32),
    /// Move the health meter (`+0x0B`), then clamp it into `0 ..= cap`.
    Health { delta: i32, cap: i32 },
    /// Move happiness (`+0x0C`).
    Happiness(i32),
    Gold(i32),
    Stone(i32),
    Wood(i32),
    Weapons(i32),
    /// Set the tax gate at county `+0x1A8`, so `Tax_CollectAll` takes nothing
    /// from this county this season.
    SuppressTax,
    FieldGained,
    FieldLost,
}

/// The units of weapons a find or an embezzlement moves. `[V]` — both handlers
/// use 25, and *"Corruption"* is guarded on the realm already holding 25.
pub const EVENT_WEAPON_UNITS: i32 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    Always,
    Grain(i32),
    Herd(i32),
    Population(i32),
    Happiness(i32),
    HealthAtLeast(i32),
    HealthBelow(i32),
    HappinessBelow(i32),
    Gold(i32),
    Wood(i32),
    Weapons(i32),
    TaxShown(i32),
    Both(&'static Guard, &'static Guard),
    FieldAvailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RealmPurse {
    pub gold: i32,
    pub wood: i32,
    pub stone: i32,
    pub weapons: [i32; crate::tables::WEAPON_TYPE_COUNT],
}

