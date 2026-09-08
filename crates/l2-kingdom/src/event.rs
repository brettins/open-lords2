//! Random events — `docs/kingdom.md` §8.1, `Event_RollAll` (`0x00448819`).
//!
//! # The table, which is not a 24-entry table
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
//!   groups describes, in prose, precisely what the decompiled handler does.
//!   That last one is the strong evidence: id `0x13B` = group 315 is *"Stop
//!   thief!.  Highwaymen waylay your tax collectors. Lose all tax revenues this
//!   season."*, and the handler's whole body sets county `+0x1A8`, which is the
//!   *"another untraced gate"* `docs/kingdom.md` §4.1 says zeroes the tax take.
//!
//! # The draw is a ring, not a probability
//!
//! ```c
//! index = g_seasonRandom * 2;                 /* 0 .. 254, drawn once a season */
//! for (county = 1; county <= g_countyCount; county++) {
//!     clear the three modifiers and the tax gate;
//!     index++; if (index > 255) index = 0;
//!     if (owner is human && g_year > 1268 && g_eventTable[index] != 0) fire it;
//! }
//! ```
//!
//! So one number is drawn per **season**, not per county, and the counties then
//! walk consecutive slots of the deck. Three consequences, and the third is a
//! bug:
//!
//! * adjacent counties can never both draw, because no two dealt slots are
//!   adjacent — the closest pair is eight apart;
//! * the deck's density, 26 slots in 256, is the whole frequency rule: about
//!   one event per ten eligible county-seasons;
//! * **only odd-numbered counties can ever draw an event at all.**
//!
//! That last one is not a reading, it is arithmetic. The seed is
//! `g_seasonRandom * 2`, which is always **even**; the index the wrap resets to
//! is 0, which is also even, so the parity survives the wrap; county `k`
//! therefore always lands on a slot of parity `k`. And every dealt slot is at
//! `index ≡ 7 (mod 8)`, which is **odd**. So counties 2, 4, 6 … 16 are
//! permanently exempt from the random-event table, and no amount of play will
//! reveal it, because the counties that do draw look exactly as they should.
//! The seed is `00448822 MOV EAX,[0x0058FD60] / ADD EAX,EAX` in the
//! disassembly, so the doubling is not a decompiler artefact
//! (`docs/decisions.md` C13). Reproduced rather than corrected, and asserted
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

use crate::county::County;
use crate::report::Message;
use crate::tables::{Season, Tables};
use l2_net::Pcg32;

/// The `i8` sentinel `Herd_SeasonTick` tests for before it tests the sign:
/// *"No bull"* writes 99 into the herd modifier and the herd pass reads it as
/// **"no growth at all this season"** rather than as +99%. `[V]` —
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
    /// The `L2.eng` group that names and describes this event, which is the
    /// same number as the deck stores.
    pub fn id(self) -> u16 {
        self as u16
    }

    pub fn from_id(id: u16) -> Option<EventKind> {
        EVENT_KINDS.iter().copied().find(|k| k.id() == id)
    }

    /// `L2.eng` group index 0 — the headline the message box shows.
    pub fn name(self) -> &'static str {
        match self {
            EventKind::Rats => "Rats!!",
            EventKind::MadCows => "Mad Cows !!",
            EventKind::Wolves => "Wolves.",
            EventKind::Plague => "Plague.",
            EventKind::GrainFound => "Grain found.",
            EventKind::BadCattle => "Bad cattle stock",
            EventKind::CowBonanza => "Cow bonanza!!",
            EventKind::WeddingFever => "Wedding fever.",
            EventKind::HealthyEating => "Healthy eating.",
            EventKind::MotherNature => "Mother nature.",
            EventKind::WeaponsFound => "Weapons found.",
            EventKind::Donation => "Donation.",
            EventKind::Treasure => "Treasure",
            EventKind::HolyRelic => "Holy Relic.",
            EventKind::Witch => "Witch !!",
            EventKind::StoneFound => "Stone found.",
            EventKind::Locusts => "Pests.",
            EventKind::HagsCurse => "Hags curse.",
            EventKind::Fraud => "Fraud.",
            EventKind::Corruption => "Corruption.",
            EventKind::NoBull => "No bull.",
            EventKind::StopThief => "Stop thief!.",
            EventKind::Termites => "Pests.",
            EventKind::NoSongs => "No songs.",
        }
    }
}

/// `g_eventTable` (`0x004D6108`) — the 256-slot deck, as `(slot, id)` pairs for
/// the 26 non-zero slots. Every other slot is "no event".
///
/// Kept sparse rather than as a 256-entry array because the sparseness *is* the
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
/// Split out from [`fire`] so the effects can be read as a table rather than as
/// a 24-arm `match` full of field writes, and so a test can assert the guard
/// and the effect separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Write an `i8` percentage into county `+0x1FB`. Applied to births and
    /// deaths by `Population_UpdateAll`, capped at 20% of the county.
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

/// The weapon type *"Weapons found"* and *"Corruption"* move.
///
/// **This is a bug and it is reproduced.** `FUN_0044938C` indexes the realm's
/// weapon array with `(countyId & 3) + 1` rather than with the county's chosen
/// weapon type at `+0x290`, so which weapon a county finds depends on its
/// *identity* and never on what its blacksmith makes — and type 0, the
/// crossbow, can never be found or embezzled at all. `FUN_00449688`, the
/// *"Corruption"* handler, computes the same index the same way, which is what
/// makes it a shared idiom rather than a one-off typo.
pub fn weapon_slot(county_id: usize) -> usize {
    (county_id & 3) + 1
}

/// The units of weapons a find or an embezzlement moves. `[V]` — both handlers
/// use 25, and *"Corruption"* is guarded on the realm already holding 25.
pub const EVENT_WEAPON_UNITS: i32 = 25;

impl EventKind {
    /// The handler's effect, given the season now beginning.
    ///
    /// Four of the twenty-four are **seasonal**: *Rats*, *Grain found*,
    /// *Plague* and *Wedding fever* each read `g_season` and pick one of four
    /// percentages. Nothing in `docs/kingdom.md` mentions that, and it is the
    /// difference between a plague costing 20% of a county in Summer and 40% in
    /// Winter.
    pub fn effect(self, season: Season) -> Effect {
        // Indexed Spring, Summer, Autumn, Winter — the order `g_season` is
        // 1..=4 in, so a table read is `[season.index() - 1]`.
        let by_season = |s: [i32; 4]| s[season.index() as usize - 1];
        match self {
            EventKind::Rats => Effect::GrainPct(by_season([-30, -25, -45, -40])),
            EventKind::MadCows => Effect::HerdPct(-20),
            EventKind::Wolves => Effect::HerdPct(-40),
            EventKind::Plague => Effect::PopulationPct(by_season([-30, -20, -30, -40])),
            EventKind::GrainFound => Effect::GrainPct(by_season([40, 35, 25, 15])),
            EventKind::BadCattle => Effect::HerdPct(-10),
            EventKind::CowBonanza => Effect::HerdPct(25),
            EventKind::WeddingFever => Effect::PopulationPct(by_season([60, 50, 40, 30])),
            EventKind::HealthyEating => Effect::Health { delta: 20, cap: 100 },
            EventKind::MotherNature => Effect::FieldGained,
            EventKind::WeaponsFound => Effect::Weapons(EVENT_WEAPON_UNITS),
            EventKind::Donation => Effect::Gold(500),
            EventKind::Treasure => Effect::Gold(1000),
            EventKind::HolyRelic => Effect::Happiness(5),
            EventKind::Witch => Effect::Happiness(10),
            EventKind::StoneFound => Effect::Stone(100),
            EventKind::Locusts => Effect::FieldLost,
            EventKind::HagsCurse => Effect::Health { delta: -20, cap: 100 },
            EventKind::Fraud => Effect::Gold(-500),
            EventKind::Corruption => Effect::Weapons(-EVENT_WEAPON_UNITS),
            EventKind::NoBull => Effect::HerdPct(HERD_NO_GROWTH),
            EventKind::StopThief => Effect::SuppressTax,
            EventKind::Termites => Effect::Wood(-300),
            EventKind::NoSongs => Effect::Happiness(-7),
        }
    }

    /// *Plague* alone moves the health meter as well as the population. `[V]` —
    /// `FUN_00448F6F` does `meter -= 20; if (meter > 25) meter = 25;
    /// if (meter < 0) meter = 0;`, so a Perfect county drops to Sick in one
    /// season and the clamp, not the subtraction, does most of the work.
    pub fn health_side_effect(self) -> Option<Effect> {
        match self {
            EventKind::Plague => Some(Effect::Health { delta: -20, cap: 25 }),
            _ => None,
        }
    }
}

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
    /// The county's shown tax must be at least this — there is no point robbing
    /// a tax collector who is carrying nothing.
    TaxShown(i32),
    /// Two guards, both of which must pass.
    Both(&'static Guard, &'static Guard),
    /// The effect itself decides: the two field events fail when the county has
    /// no field of the kind they need.
    FieldAvailable,
}

impl EventKind {
    /// The handler's guard. `[V]` — every one of these is the `if` at the top
    /// of the handler, and the failing branch is always the same two writes.
    pub fn guard(self) -> Guard {
        match self {
            EventKind::Rats | EventKind::GrainFound => Guard::Grain(50),
            EventKind::MadCows
            | EventKind::Wolves
            | EventKind::BadCattle
            | EventKind::CowBonanza => Guard::Herd(40),
            EventKind::Plague => Guard::Population(100),
            EventKind::WeddingFever => Guard::Both(&Guard::Population(100), &Guard::Happiness(30)),
            EventKind::HealthyEating => Guard::HealthBelow(81),
            EventKind::HagsCurse => Guard::Both(&Guard::HealthAtLeast(20), &Guard::Herd(20)),
            EventKind::NoBull => Guard::Herd(20),
            EventKind::HolyRelic => Guard::HappinessBelow(96),
            EventKind::Witch => Guard::HappinessBelow(91),
            EventKind::NoSongs => Guard::Happiness(7),
            EventKind::Fraud => Guard::Gold(500),
            EventKind::Termites => Guard::Wood(300),
            EventKind::Corruption => Guard::Weapons(EVENT_WEAPON_UNITS),
            EventKind::StopThief => Guard::TaxShown(10),
            EventKind::MotherNature | EventKind::Locusts => Guard::FieldAvailable,
            // `Weapons found`, `Donation`, `Treasure` and `Stone found` have no
            // guard at all: their handlers are one line.
            EventKind::WeaponsFound
            | EventKind::Donation
            | EventKind::Treasure
            | EventKind::StoneFound => Guard::Always,
        }
    }
}

/// The realm-side state an event may read or move. The realm record is not this
/// module's, so the caller passes a copy and gets it back mutated.
///
/// Kept as a small struct rather than a `&mut Realm` so a test can drive an
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

/// Whether a county and its realm satisfy an event's guard.
pub fn guard_passes(guard: &Guard, county: &County, id: usize, purse: &RealmPurse) -> bool {
    match guard {
        Guard::Always => true,
        Guard::Grain(n) => county.grain >= *n,
        Guard::Herd(n) => county.herd >= *n,
        Guard::Population(n) => county.population >= *n,
        Guard::Happiness(n) => county.happiness >= *n,
        Guard::HealthAtLeast(n) => county.health_meter >= *n,
        Guard::HealthBelow(n) => county.health_meter < *n,
        Guard::HappinessBelow(n) => county.happiness < *n,
        Guard::Gold(n) => purse.gold >= *n,
        Guard::Wood(n) => purse.wood >= *n,
        Guard::Weapons(n) => purse.weapons[weapon_slot(id)] >= *n,
        Guard::TaxShown(n) => county.tax_shown >= *n,
        Guard::Both(a, b) => {
            guard_passes(a, county, id, purse) && guard_passes(b, county, id, purse)
        }
        // `Mother nature` needs a field slot free to turn fallow; `Locusts`
        // needs a grain field to ruin. **`[I]` on the mapping**: the original
        // walks the county's twenty map tiles and tests the tile's terrain
        // byte, and this crate models fields as the three counts at `+0x1FF`,
        // `+0x200` and `+0x201` rather than as tiles. "A barren field" is taken
        // to be a free slot below `MAX_FIELDS`, and "a grain field" to be
        // `fields_grain > 0`. [`apply`] does the testing, so the guard passes
        // here and the effect reports the failure.
        Guard::FieldAvailable => true,
    }
}

/// Apply an event to a county and its realm's purse. Returns `false` — changing
/// nothing — if the guard failed, which is the original's "clear the flag and
/// show nothing" case.
pub fn fire(
    county: &mut County,
    id: usize,
    purse: &mut RealmPurse,
    kind: EventKind,
    season: Season,
) -> bool {
    if !guard_passes(&kind.guard(), county, id, purse) {
        return false;
    }
    let effect = kind.effect(season);
    if !apply(county, id, purse, effect) {
        return false;
    }
    if let Some(extra) = kind.health_side_effect() {
        apply(county, id, purse, extra);
    }
    county.event_fired = true;
    county.event_id = kind.id();
    true
}

/// One [`Effect`]. Returns `false` when the effect could not happen at all,
/// which only the two field events can report.
fn apply(county: &mut County, id: usize, purse: &mut RealmPurse, effect: Effect) -> bool {
    match effect {
        Effect::PopulationPct(p) => county.event_population_pct = p,
        Effect::GrainPct(p) => county.event_grain_pct = p,
        Effect::HerdPct(p) => county.event_herd_pct = p,
        Effect::Health { delta, cap } => {
            county.health_meter = (county.health_meter + delta).min(cap).max(0);
            county.health_band = crate::tables::health_band(county.health_meter);
        }
        Effect::Happiness(d) => {
            county.happiness = crate::math::clamp(county.happiness + d, 0, 100)
        }
        Effect::Gold(d) => purse.gold += d,
        Effect::Stone(d) => purse.stone += d,
        Effect::Wood(d) => purse.wood += d,
        Effect::Weapons(d) => purse.weapons[weapon_slot(id)] += d,
        Effect::SuppressTax => county.tax_suppressed = true,
        Effect::FieldGained => {
            if county.field_total() >= crate::county::MAX_FIELDS as i32 {
                return false;
            }
            county.fields_fallow += 1;
        }
        Effect::FieldLost => {
            if county.fields_grain <= 0 {
                return false;
            }
            county.fields_grain -= 1;
        }
    }
    true
}

/// Whether a county is eligible to draw at all. **The AI never draws random
/// events**, and nothing is drawn until the year passes 1268.
pub fn eligible(t: &Tables, county: &County, owner_is_human: bool, year: i32) -> bool {
    year > t.event.first_year && !county.is_unowned() && owner_is_human
}

/// `Event_RollAll` — clear last season's modifiers, then walk the deck.
///
/// `purses` is indexed by realm id and is only read for the seven events that
/// touch a treasury or a stockpile; the caller copies the values back.
///
/// **One draw per season, before the county loop**, which is the determinism
/// property that matters: the number of values taken from the generator does
/// not depend on how many counties there are or who owns them
/// (`docs/netcode.md` §3).
#[allow(clippy::too_many_arguments)]
pub fn roll_all(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    human_owner: &dyn Fn(u8) -> bool,
    purses: &mut [RealmPurse],
    year: i32,
    season: Season,
    rng: &mut Pcg32,
    out: &mut Vec<Message>,
) {
    let mut index = (rng.below(EVENT_SEED_BOUND) * 2) as usize;
    for id in 1..=county_count {
        // The original clears all four every season, whether or not this county
        // draws — which is what stops a modifier being applied twice.
        counties[id].event_fired = false;
        counties[id].event_id = 0;
        counties[id].event_population_pct = 0;
        counties[id].event_grain_pct = 0;
        counties[id].event_herd_pct = 0;
        counties[id].tax_suppressed = false;

        index += 1;
        if index > EVENT_DECK_SLOTS - 1 {
            index = 0;
        }
        let Some(kind) = deck_slot(index) else { continue };
        if !eligible(t, &counties[id], human_owner(counties[id].owner), year) {
            continue;
        }
        let owner = counties[id].owner as usize;
        let mut purse = purses.get(owner).copied().unwrap_or_default();
        if fire(&mut counties[id], id, &mut purse, kind, season) {
            if let Some(slot) = purses.get_mut(owner) {
                *slot = purse;
            }
            out.push(Message::Event { county: id as u8, kind });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;
    use std::collections::BTreeSet;

    fn county(owner: u8) -> County {
        let mut c = County::new();
        c.owner = owner;
        c
    }

    /// **The invariant that closes the table.** The deck holds exactly the 24
    /// ids the dispatch handles: no id the dispatch cannot reach, and no
    /// handler the deck cannot deal.
    #[test]
    fn the_deck_holds_exactly_the_twenty_four_dispatched_ids() {
        let dealt: BTreeSet<u16> = EVENT_DECK.iter().map(|(_, k)| k.id()).collect();
        let handled: BTreeSet<u16> = EVENT_KINDS.iter().map(|k| k.id()).collect();
        assert_eq!(dealt, handled);
        assert_eq!(handled.len(), 24);
    }

    /// The two id runs `docs/kingdom.md` §8.1 names: `0x87 … 0x8E` and
    /// `0x12E … 0x13D`, contiguous and with nothing between them.
    #[test]
    fn the_ids_are_the_two_contiguous_runs_the_document_names() {
        let ids: Vec<u16> = EVENT_KINDS.iter().map(|k| k.id()).collect();
        assert_eq!(ids[..8], (0x87..=0x8E).collect::<Vec<u16>>()[..]);
        assert_eq!(ids[8..], (0x12E..=0x13D).collect::<Vec<u16>>()[..]);
        for pair in ids.windows(2) {
            assert!(pair[1] > pair[0], "the kinds are in ascending id order");
        }
    }

    /// Every filled slot is at `index ≡ 7 (mod 8)` — the deck is 32 groups of
    /// eight with at most one event in each group's last slot.
    #[test]
    fn every_dealt_slot_sits_at_the_end_of_a_group_of_eight() {
        for (slot, kind) in EVENT_DECK {
            assert_eq!(slot % 8, 7, "{} is at slot {slot}", kind.name());
            assert!(slot < EVENT_DECK_SLOTS);
        }
        for pair in EVENT_DECK.windows(2) {
            assert!(pair[1].0 > pair[0].0, "the deck is in slot order");
        }
    }

    /// 26 filled slots for 24 ids: two events are dealt twice and so come up
    /// about twice as often as the rest.
    #[test]
    fn two_events_are_dealt_twice_and_the_rest_once() {
        let mut doubled: Vec<&str> = Vec::new();
        for kind in EVENT_KINDS {
            let n = EVENT_DECK.iter().filter(|(_, k)| *k == kind).count();
            assert!(n == 1 || n == 2, "{} appears {n} times", kind.name());
            if n == 2 {
                doubled.push(kind.name());
            }
        }
        doubled.sort_unstable();
        assert_eq!(doubled, vec!["Grain found.", "Plague."]);
        assert_eq!(EVENT_DECK.len(), 26);
    }

    /// The frequency rule, stated as the density it is: 26 slots in 256.
    #[test]
    fn about_one_county_season_in_ten_draws_anything() {
        let filled = (0..EVENT_DECK_SLOTS).filter(|s| deck_slot(*s).is_some()).count();
        assert_eq!(filled, EVENT_DECK.len());
        assert_eq!(EVENT_DECK_SLOTS - filled, 230, "230 empty slots");
    }

    /// **The AI never draws random events**, and nothing happens before 1268 is
    /// past.
    #[test]
    fn only_a_human_county_after_1268_is_eligible() {
        let c = county(1);
        assert!(!eligible(T, &c, true, 1268), "the first year is excluded");
        assert!(eligible(T, &c, true, 1269));
        assert!(!eligible(T, &c, false, 1269), "an AI county never draws");
        assert!(!eligible(T, &county(0), true, 1269), "nor an unowned one");
    }

    /// Four handlers read the season. A plague in Winter costs twice what a
    /// plague in Summer does.
    #[test]
    fn four_events_hit_harder_in_winter_than_in_summer() {
        let hit = |k: EventKind, s: Season| match k.effect(s) {
            Effect::PopulationPct(p) | Effect::GrainPct(p) => p,
            other => panic!("{other:?} is not a percentage"),
        };
        assert_eq!(hit(EventKind::Plague, Season::Summer), -20);
        assert_eq!(hit(EventKind::Plague, Season::Winter), -40);
        assert_eq!(hit(EventKind::Rats, Season::Autumn), -45, "the harvest is in the barn");
        assert_eq!(hit(EventKind::Rats, Season::Summer), -25);
        assert_eq!(hit(EventKind::GrainFound, Season::Spring), 40);
        assert_eq!(hit(EventKind::GrainFound, Season::Winter), 15);
        assert_eq!(hit(EventKind::WeddingFever, Season::Spring), 60);
        assert_eq!(hit(EventKind::WeddingFever, Season::Winter), 30);
    }

    /// Every one of the twenty-four has an effect in every season, and no
    /// percentage is outside what an `i8` can hold — the original stores them
    /// in a signed byte.
    #[test]
    fn every_effect_fits_the_signed_byte_the_original_stores_it_in() {
        for kind in EVENT_KINDS {
            for season in Season::ALL {
                if let Effect::PopulationPct(p) | Effect::GrainPct(p) | Effect::HerdPct(p) =
                    kind.effect(season)
                {
                    assert!((-128..=127).contains(&p), "{} gives {p}", kind.name());
                }
            }
        }
    }

    /// A guard that fails leaves the county untouched and reports nothing —
    /// the original's "clear the flag" branch.
    #[test]
    fn a_failed_guard_changes_nothing_at_all() {
        let mut c = county(1);
        c.herd = 39; // one short of Wolves' guard
        let before = c.clone();
        let mut purse = RealmPurse::default();
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring));
        assert_eq!(c, before);
        assert!(!c.event_fired);

        c.herd = 40;
        assert!(fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring));
        assert_eq!(c.event_herd_pct, -40);
        assert!(c.event_fired);
        assert_eq!(c.event_id, 0x89);
    }

    /// *"Stop thief!. … Lose all tax revenues this season."* — the handler's
    /// whole body is the gate `docs/kingdom.md` §4.1 could not explain.
    #[test]
    fn stop_thief_sets_the_gate_that_zeroes_the_tax_take() {
        let mut c = county(1);
        c.tax_shown = 9;
        let mut purse = RealmPurse::default();
        assert!(!fire(&mut c, 1, &mut purse, EventKind::StopThief, Season::Spring));
        assert!(!c.tax_suppressed, "not worth robbing");

        c.tax_shown = 10;
        assert!(fire(&mut c, 1, &mut purse, EventKind::StopThief, Season::Spring));
        assert!(c.tax_suppressed);

        // And the gate is what Tax_CollectAll reads.
        c.population = 1000;
        c.tax_rate = 10;
        c.castle_type = 0;
        assert_eq!(crate::tax::collect(T, &mut c, 0), 0, "the collectors were waylaid");
    }

    /// *"No bull"* writes 99, and the herd pass reads 99 as a sentinel rather
    /// than as +99%.
    #[test]
    fn no_bull_stops_the_herd_growing_rather_than_doubling_it() {
        let mut c = county(1);
        c.herd = 200;
        c.weather = crate::tables::Weather::Sunny; // +5% if anything grew
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::NoBull, Season::Spring));
        assert_eq!(c.event_herd_pct, HERD_NO_GROWTH);

        crate::land::herd_season_tick(T, &mut c);
        assert_eq!(c.herd, 200, "no growth, and certainly not +99%");
    }

    /// The plague hits the population *and* pins the health meter to 25.
    #[test]
    fn the_plague_knocks_a_perfect_county_into_the_sick_band() {
        let mut c = county(1);
        c.population = 500;
        c.health_meter = 100;
        c.health_band = 4;
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::Plague, Season::Winter));
        assert_eq!(c.event_population_pct, -40);
        assert_eq!(c.health_meter, 25, "clamped down to 25, not 100 - 20");
        assert_eq!(c.health_band, 1, "Sick");
    }

    /// **The weapon-slot bug, reproduced.** Which weapon a county finds depends
    /// on its id and never on what its blacksmith makes, and the crossbow can
    /// never be found at all.
    #[test]
    fn a_found_weapon_is_chosen_by_the_county_id_not_by_the_blacksmith() {
        for id in 1..=16usize {
            let mut c = county(1);
            c.weapon_type = 5; // armour: what the county actually makes
            let mut purse = RealmPurse::default();
            assert!(fire(&mut c, id, &mut purse, EventKind::WeaponsFound, Season::Spring));
            let slot = weapon_slot(id);
            assert_eq!(purse.weapons[slot], 25, "county {id}");
            assert_ne!(slot, 0, "slot 0, the crossbow, is unreachable");
            assert_ne!(slot, 5, "and it is never what the county makes here");
        }
    }

    /// *Fraud*, *Termites* and *Corruption* all take, and all refuse to take
    /// what is not there.
    #[test]
    fn the_three_thefts_are_guarded_on_there_being_something_to_take() {
        let mut c = county(1);
        let mut purse = RealmPurse { gold: 499, wood: 299, ..RealmPurse::default() };
        purse.weapons[weapon_slot(1)] = 24;
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Fraud, Season::Spring));
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Termites, Season::Spring));
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Corruption, Season::Spring));
        assert_eq!((purse.gold, purse.wood), (499, 299));

        purse = RealmPurse { gold: 500, wood: 300, ..RealmPurse::default() };
        purse.weapons[weapon_slot(1)] = 25;
        assert!(fire(&mut c, 1, &mut purse, EventKind::Fraud, Season::Spring));
        assert!(fire(&mut c, 1, &mut purse, EventKind::Termites, Season::Spring));
        assert!(fire(&mut c, 1, &mut purse, EventKind::Corruption, Season::Spring));
        assert_eq!((purse.gold, purse.wood), (0, 0));
        assert_eq!(purse.weapons[weapon_slot(1)], 0);
    }

    /// The two field events, on the crate's county-count model of fields.
    #[test]
    fn the_two_field_events_need_a_field_to_work_on() {
        let mut full = county(1);
        full.fields_fallow = crate::county::MAX_FIELDS as i32;
        let mut purse = RealmPurse::default();
        assert!(!fire(&mut full, 1, &mut purse, EventKind::MotherNature, Season::Spring));

        let mut room = county(1);
        room.fields_grain = 4;
        assert!(fire(&mut room, 1, &mut purse, EventKind::MotherNature, Season::Spring));
        assert_eq!(room.fields_fallow, 1);
        assert!(fire(&mut room, 1, &mut purse, EventKind::Locusts, Season::Spring));
        assert_eq!(room.fields_grain, 3);

        let mut none = county(1);
        assert!(!fire(&mut none, 1, &mut purse, EventKind::Locusts, Season::Spring));
    }

    /// The draw count must not depend on ownership, or two peers that disagree
    /// about one county's owner would diverge in every later draw.
    #[test]
    fn the_stream_advances_identically_whoever_owns_the_counties() {
        let run = |human: bool| {
            let mut rng = Pcg32::from_seed(1234);
            let mut c = vec![County::new(); 17];
            let mut purses = vec![RealmPurse::default(); 6];
            for id in 1..=14 {
                // Odd counties owned: see
                // `only_odd_numbered_counties_can_ever_draw_an_event`.
                c[id].owner = if id % 2 == 1 { 1 } else { 0 };
                c[id].grain = 5_000;
                c[id].herd = 500;
                c[id].population = 500;
                c[id].happiness = 60;
                c[id].health_meter = 60;
                c[id].tax_shown = 100;
                c[id].fields_grain = 6;
            }
            let mut out = Vec::new();
            for _ in 0..40 {
                roll_all(T, 
                    &mut c,
                    14,
                    &|_| human,
                    &mut purses,
                    1300,
                    Season::Spring,
                    &mut rng,
                    &mut out,
                );
            }
            (rng, out.len())
        };
        let (a, events_a) = run(true);
        let (b, events_b) = run(false);
        assert_eq!(a, b, "the generator must be in the same place either way");
        assert!(events_a > 0, "somebody should have drawn something");
        assert_eq!(events_b, 0, "and an all-AI kingdom draws nothing");
    }

    /// One draw per season, not one per county — so a kingdom of 4 and a
    /// kingdom of 14 leave the generator in the same place.
    #[test]
    fn the_season_draws_exactly_one_number_however_many_counties_there_are() {
        for n in [1usize, 4, 14, 16] {
            let mut rng = Pcg32::from_seed(9);
            let mut c = vec![County::new(); 17];
            let mut purses = vec![RealmPurse::default(); 6];
            let mut out = Vec::new();
            roll_all(T, &mut c, n, &|_| true, &mut purses, 1300, Season::Spring, &mut rng, &mut out);
            let mut reference = Pcg32::from_seed(9);
            reference.below(EVENT_SEED_BOUND);
            assert_eq!(rng, reference, "kingdom of {n}");
        }
    }

    #[test]
    fn the_same_seed_rolls_the_same_events() {
        let run = || {
            let mut rng = Pcg32::from_seed(7);
            let mut c = vec![County::new(); 17];
            let mut purses = vec![RealmPurse::default(); 6];
            for id in 1..=14 {
                c[id].owner = 1;
                c[id].grain = 900;
                c[id].herd = 200;
                c[id].population = 500;
            }
            let mut out = Vec::new();
            for _ in 0..10 {
                roll_all(T, 
                    &mut c,
                    14,
                    &|_| true,
                    &mut purses,
                    1300,
                    Season::Autumn,
                    &mut rng,
                    &mut out,
                );
            }
            out
        };
        assert_eq!(run(), run());
    }

    /// Adjacent counties can never both draw, because no two dealt slots are
    /// adjacent. A property of the deck rather than of the code.
    #[test]
    fn no_two_neighbouring_counties_can_draw_in_the_same_season() {
        for pair in EVENT_DECK.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= 8, "slots {} and {}", pair[0].0, pair[1].0);
        }
    }

    /// **The reproduced bug: half the counties never draw an event.**
    ///
    /// The seed is `random * 2` and so always even, the wrap resets to 0 which
    /// is also even, and every dealt slot is odd. County `k` lands on a slot of
    /// parity `k`, so an even-numbered county cannot reach a dealt slot at any
    /// seed, in any season, for the whole game.
    ///
    /// Checked exhaustively over every one of the 128 seeds rather than by
    /// sampling, because "we never saw it happen" is not the same claim.
    #[test]
    fn only_odd_numbered_counties_can_ever_draw_an_event() {
        for slot in EVENT_DECK.iter().map(|(s, _)| *s) {
            assert_eq!(slot % 2, 1, "slot {slot} is even");
        }
        let mut drew: Vec<usize> = Vec::new();
        for seed in 0..EVENT_SEED_BOUND as usize {
            let mut index = seed * 2;
            for county in 1..=crate::county::MAX_COUNTY_ID as usize {
                index += 1;
                if index > EVENT_DECK_SLOTS - 1 {
                    index = 0;
                }
                if deck_slot(index).is_some() && !drew.contains(&county) {
                    drew.push(county);
                }
            }
        }
        drew.sort_unstable();
        assert_eq!(drew, vec![1, 3, 5, 7, 9, 11, 13, 15], "the even counties never come up");
    }

    /// The same thing through the real entry point: an all-even kingdom draws
    /// nothing at all however long it runs.
    #[test]
    fn a_kingdom_of_even_numbered_counties_never_sees_an_event() {
        let mut rng = Pcg32::from_seed(4242);
        let mut c = vec![County::new(); 17];
        let mut purses = vec![RealmPurse::default(); 6];
        for id in 1..=16 {
            c[id].owner = if id % 2 == 0 { 1 } else { 0 };
            c[id].grain = 5_000;
            c[id].herd = 500;
            c[id].population = 500;
            c[id].happiness = 60;
            c[id].health_meter = 60;
            c[id].tax_shown = 100;
        }
        let mut out = Vec::new();
        for _ in 0..500 {
            roll_all(T, &mut c, 16, &|_| true, &mut purses, 1300, Season::Spring, &mut rng, &mut out);
        }
        assert!(out.is_empty(), "five hundred seasons and not one event");
    }

    #[test]
    fn an_event_is_consumed_by_the_pass_that_reads_it() {
        let mut c = county(1);
        c.grain = 1000;
        c.weather = crate::tables::Weather::Cloudy;
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::Rats, Season::Winter));
        crate::land::grain_season_tick(T, &mut c, Season::Winter, true);
        assert_eq!(c.grain, 600, "a 40% winter loss");
        assert_eq!(c.event_grain_pct, 0);
    }

    #[test]
    fn every_id_round_trips_through_from_id() {
        for kind in EVENT_KINDS {
            assert_eq!(EventKind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(EventKind::from_id(0), None);
        assert_eq!(EventKind::from_id(0x8F), None, "the gap between the two runs");
    }
}
