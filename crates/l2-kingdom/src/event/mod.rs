//! Random events — `docs/kingdom.md` §8.1, `Event_RollAll` (`0x00448819`).
//!
//! # The table
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
//!   That last one is the strong evidence: id `0x13B` = group 315 is *"Stop
//!   thief!.  Highwaymen waylay your tax collectors. Lose all tax revenues this
//! season."*, and the handler's whole body sets county `+0x1A8`, which is the
//!   *"another untraced gate"* `docs/kingdom.md` §4.1 says zeroes the tax take.
//!
//! # The draw is a ring, not a probability
//!
//! ```c
//! index = g_seasonRandom * 2;                 /* 0 .. 254, drawn once a season */
//! for (county = 1; county <= g_countyCount; county++) {
//! clear the three modifiers and the tax gate;
//!     index++; if (index > 255) index = 0;
//!     if (owner is human && g_year > 1268 && g_eventTable[index] != 0) fire it;
//! }
//! ```
//!
//! So one number is drawn per **season**, and the counties then
//! walk consecutive slots of the deck. Three consequences, and the third is a
//! bug:
//!
//! * adjacent counties can never both draw, because no two dealt slots are
//!   adjacent — the closest pair is eight apart;
//! * the deck's density, 26 slots in 256, is the whole frequency rule: about
//!   one event per ten eligible county-seasons;
//! * **only odd-numbered counties can ever draw an event at all.**
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
//! # What the handlers do
//!
//! Most write an `i8` percentage into county `+0x1FB`, `+0x1FC` or `+0x1FD`,
//! which the population, grain and herd passes then apply — and several of
//! those percentages **depend on the season**, which `docs/kingdom.md` does not
//! mention. The rest move happiness, the health meter, a stockpile, the
//! treasury, a field or the tax gate directly.
//!
//! Almost every handler is **guarded**, and a handler whose guard fails clears
//! the county's `eventFired` flag and event id, so nothing is shown: *Wolves*
//! needs a herd of 40, *Plague* needs 100 people, *Fraud* needs 500 crowns to
//! steal. That is why a county with nothing left is not kicked while it is
//! down.
//!
//! # The whole path, and the half that is not here
//!
//! | | function | |
//! |---|---|---|
//! | roll, store, dispatch | `Event_RollAll` | `0x00448819`, once a season, [`roll_all`] |
//! | the 24 handlers | `FUN_00448E12` … `FUN_00449826` | a failing guard clears both flags, [`fire`] |
//! | **post the letter** | `Event_Post` (`FUN_00448D7E`) | `0x00448D7E`, once a **frame**, for `g_selectedCounty` — `l2_game::message::post_event` |
//! | draw it | `Msg_DrawWindow` category `0x0F` | `0x0047309E`, `l2_game::screens::message::draw_event` |
//! | clear `eventFired` | `Event_Post` again | and **nothing at all clears `eventId`** |
//!
//! **The letter is posted for the selected county only**, so an event in a
//! county the player is not looking at waits — for a frame, a season or the rest
//! of the game — and arrives the moment that county is picked. That is not a
//! rule this crate can carry: `g_selectedCounty` is one peer's cursor, not
//! simulation state, so the poster lives in `l2-game`.

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

/// The `i8` sentinel `Herd_SeasonTick` tests for before it tests the sign:
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
    /// 135 — *"Rats!! Vermin have been discovered in the county's tithe barns.
    /// They have been eating through your stocks of grain."*
    Rats = 0x87,
    /// 136 — *"Mad Cows !! A strange and deadly malady affects your dairy herd.
    /// Many fatalities."*
    MadCows = 0x88,
    /// 137 — *"Wolves. … they have grown desperate and are devastating your
    /// cattle herd."*
    Wolves = 0x89,
    /// 138 — *"Plague. The Black Death is spreading across the county."*
    Plague = 0x8A,
    /// 139 — *"Grain found. … extra stocks of grain were found in the village's
    /// tithe barns."*
    GrainFound = 0x8B,
    /// 140 — *"Bad cattle stock … The weaklings … have had to be culled."*
    BadCattle = 0x8C,
    /// 141 — *"Cow bonanza!! … an unusually high number of calves."*
    CowBonanza = 0x8D,
    /// 142 — *"Wedding fever. … a jump in the number of children born."*
    WeddingFever = 0x8E,
    /// 302 — *"Healthy eating. Due to a nutritious diet the health of this
    /// county has improved."*
    HealthyEating = 0x12E,
    /// 303 — *"Mother nature. … One barren field spontaneously became fallow."*
    MotherNature = 0x12F,
    /// 304 — *"Weapons found. … Some weapons have been added to your armory."*
    WeaponsFound = 0x130,
    /// 305 — *"Donation. … sends a gift of gold."*
    Donation = 0x131,
    /// 306 — *"Treasure. Miners discover ancient buried valuables."*
    Treasure = 0x132,
    /// 307 — *"Holy Relic. … Happiness increases."*
    HolyRelic = 0x133,
    /// 308 — *"Witch !! … Happiness increases as shes burned at the stake."*
    Witch = 0x134,
    /// 309 — *"Stone found. … Some tonnes of stone added to your inventory."*
    StoneFound = 0x135,
    /// 310 — *"Pests. Locusts strip your fields bare! A grain field is made
    /// barren."*
    Locusts = 0x136,
    /// 311 — *"Hags curse. Witches curdle your cows milk … Some people are
    /// ill."*
    HagsCurse = 0x137,
    /// 312 — *"Fraud. Embezzlers loot your treasury."*
    Fraud = 0x138,
    /// 313 — *"Corruption. An unscrupulous armorer inflated your weapons
    /// inventory."*
    Corruption = 0x139,
    /// 314 — *"No bull. Cattle will not reproduce this season."*
    NoBull = 0x13A,
    /// 315 — *"Stop thief!. Highwaymen waylay your tax collectors. Lose all tax
    /// revenues this season."*
    StopThief = 0x13B,
    /// 316 — *"Pests. Termites infest your wood stocks."*
    Termites = 0x13C,
    /// 317 — *"No songs. The local Bard loses his voice, people are sad."*
    NoSongs = 0x13D,
}

/// The 24 ids, ascending. Every one is reachable from [`EVENT_DECK`] and every
/// non-zero deck slot is one of these.
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
///
/// Kept sparse because the sparseness *is* the
/// finding: 230 of 256 slots are empty and every filled one is at
/// `index ≡ 7 (mod 8)`.
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

/// The deck is 256 slots long and the index wraps at 255.
pub const EVENT_DECK_SLOTS: usize = 256;

/// The seasonal draw is `LFSR & 0x7F`, so 0..=127, and the deck index starts at
/// twice it. `[V]` — `FUN_00404A46` publishes six masked values from two
/// 31-bit LFSRs and `Event_RollAll` reads the `& 0x7F` one.
pub const EVENT_SEED_BOUND: u32 = 128;

/// What slot `slot` of the deck holds.
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

/// What one handler does to one county, once its guard has passed.
///
/// Split out from [`fire`] so the effects can be read as a table
/// a 24-arm `match` full of field writes, and so a test can assert the guard
/// and the effect separately.
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
    /// Move the realm's treasury.
    Gold(i32),
    /// Move the realm's stone stockpile.
    Stone(i32),
    /// Move the realm's wood stockpile.
    Wood(i32),
    /// Move the realm's count of one weapon type. **The type is
    /// `(countyId & 3) + 1`** — see [`weapon_slot`].
    Weapons(i32),
    /// Set the tax gate at county `+0x1A8`, so `Tax_CollectAll` takes nothing
    /// from this county this season.
    SuppressTax,
    /// Turn one barren field into a fallow one.
    FieldGained,
    /// Turn one grain field barren.
    FieldLost,
}

/// The units of weapons a find or an embezzlement moves. `[V]` — both handlers
/// use 25, and *"Corruption"* is guarded on the realm already holding 25.
pub const EVENT_WEAPON_UNITS: i32 = 25;

/// Why a county could not draw the event the deck offered it.
///
/// A failed guard is not "nothing happened": the original clears the county's
/// `eventFired` flag and stored id, so the player is told nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// No guard: the handler always fires.
    Always,
    /// The county's grain store must be at least this.
    Grain(i32),
    /// The herd must be at least this.
    Herd(i32),
    /// The population must be at least this.
    Population(i32),
    /// Happiness must be at least this.
    Happiness(i32),
    /// The health meter must be at least this.
    HealthAtLeast(i32),
    /// The health meter must be **below** this — *Healthy eating* refuses to
    /// help a county that is already well.
    HealthBelow(i32),
    /// Happiness must be **below** this.
    HappinessBelow(i32),
    /// The realm's treasury must be at least this.
    Gold(i32),
    /// The realm's wood stockpile must be at least this.
    Wood(i32),
    /// The realm must hold this many of [`weapon_slot`]'s weapon type.
    Weapons(i32),
    /// The county's shown tax must be at least this —
    /// a tax collector who is carrying nothing.
    TaxShown(i32),
    /// Two guards, both of which must pass.
    Both(&'static Guard, &'static Guard),
    /// The effect itself decides: the two field events fail when the county has
    /// no field of the kind they need.
    FieldAvailable,
}

/// The realm-side state an event may read or move. The realm record is not this
/// module's, so the caller passes a copy and gets it back mutated.
///
/// Kept as a small struct so a test can drive an
/// event without building a kingdom, and so `event` stays independent of
/// `realm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RealmPurse {
    pub gold: i32,
    pub wood: i32,
    pub stone: i32,
    /// The realm's six weapon counters.
    pub weapons: [i32; crate::tables::WEAPON_TYPE_COUNT],
}

