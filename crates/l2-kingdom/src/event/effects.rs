#![allow(unused_imports)]
use super::*;
use super::deck::*;
use super::tests::*;
use crate::county::County;
use crate::report::Message;
use crate::tables::{Season, Tables};
use l2_net::{Pcg32, Quirk, Quirks};

/// The weapon type *"Weapons found"* and *"Corruption"* move.
///
/// **This is a bug and it is reproduced.** `FUN_0044938C` indexes the realm's
/// weapon array with `(countyId & 3) + 1`
/// weapon type at `+0x290`, so which weapon a county finds depends on its
/// *identity* and never on what its blacksmith makes — and type 0, the
/// crossbow, can never be found or embezzled at all. `FUN_00449688`, the
/// *"Corruption"* handler, computes the same index the same way, which is what
/// makes it a shared idiom.
///
/// **Switchable** — [`Quirk::FoundWeaponFollowsCountyId`], `docs/bugs.md` B3.
/// With the quirk fixed the county's own `weapon_type` (`+0x290`) is used, which
/// is the field both handlers were plainly reaching for, and the crossbow —
/// weapon slot 0, which `(id & 3) + 1` can never produce — becomes findable.
pub fn weapon_slot(county_id: usize, county_weapon_type: usize, quirks: Quirks) -> usize {
    if quirks.reproduces(Quirk::FoundWeaponFollowsCountyId) {
        (county_id & 3) + 1
    } else {
        county_weapon_type.min(crate::tables::WEAPON_TYPE_COUNT - 1)
    }
}

impl EventKind {
    /// The handler's effect, given the season now beginning.
    ///
    /// Four of the twenty-four are **seasonal**: *Rats*, *Grain found*,
    /// *Plague* and *Wedding fever* each read `g_season` and pick one of four
    /// percentages. Nothing in `docs/kingdom.md` mentioned that, and it is the
    /// difference between a plague adding 20% to a county's deaths in Summer and
    /// 40% in Winter — **of its deaths, not of its people**: `Population_UpdateAll`
    /// takes the population percentage of the season's natural deaths or births
    /// (C169).
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
    /// season and the clamp, does most of the work.
    pub fn health_side_effect(self) -> Option<Effect> {
        match self {
            EventKind::Plague => Some(Effect::Health { delta: -20, cap: 25 }),
            _ => None,
        }
    }
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

/// Whether a county and its realm satisfy an event's guard.
pub fn guard_passes(
    guard: &Guard,
    county: &County,
    id: usize,
    purse: &RealmPurse,
    quirks: Quirks,
) -> bool {
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
        Guard::Weapons(n) => purse.weapons[weapon_slot(id, county.weapon_type, quirks)] >= *n,
        Guard::TaxShown(n) => county.tax_shown >= *n,
        Guard::Both(a, b) => {
            guard_passes(a, county, id, purse, quirks)
                && guard_passes(b, county, id, purse, quirks)
        }
        // `Mother nature` needs a field slot free to turn fallow; `Locusts`
        // needs a grain field to ruin. **`[I]` on the mapping**: the original
        // walks the county's twenty map tiles and tests the tile's terrain
        // byte, and this crate models fields as the three counts at `+0x1FF`,
// `+0x200` and `+0x201`. "A barren field" is taken
        // to be a free slot below `MAX_FIELDS`, and "a grain field" to be
        // `fields_grain > 0`. [`apply`] does the testing, so the guard passes
        // here and the effect reports the failure.
        Guard::FieldAvailable => true,
    }
}

/// Apply an event to a county and its realm's purse. Returns `false` — changing
/// nothing — if the guard failed, which is the original's "clear the flag and
/// show nothing" case.
/// **The flags are set before the handler runs, and a failing guard clears
/// them.** `[V]` — `Event_RollAll` does `eventFired = 1; eventId = <slot>;`
/// *then* dispatches, and every guarded handler's failing branch is the same two
/// writes: `FUN_00448F1F` (*Wolves*) is
/// `if (herd < 0x28) { eventId = 0; eventFired = 0; } else { … }`, and
/// `FUN_0044934A` (*Mother nature*) clears the same pair when its field change
/// could not happen.
///
/// The order matters because **nothing else clears them** (see [`roll_all`]): a
/// county carrying a letter nobody has read yet has that letter *destroyed* by a
/// new event whose guard fails, and left alone by a season that deals it
/// nothing.
pub fn fire(
    county: &mut County,
    id: usize,
    purse: &mut RealmPurse,
    kind: EventKind,
    season: Season,
    quirks: Quirks,
) -> bool {
    county.event_fired = true;
    county.event_id = kind.id();
    let mut failed = !guard_passes(&kind.guard(), county, id, purse, quirks);
    if !failed {
        let effect = kind.effect(season);
        failed = !apply(county, id, purse, effect, quirks);
    }
    if failed {
        county.event_id = 0;
        county.event_fired = false;
        return false;
    }
    if let Some(extra) = kind.health_side_effect() {
        apply(county, id, purse, extra, quirks);
    }
    true
}

/// One [`Effect`]. Returns `false` when the effect could not happen at all,
/// which only the two field events can report.
fn apply(
    county: &mut County,
    id: usize,
    purse: &mut RealmPurse,
    effect: Effect,
    quirks: Quirks,
) -> bool {
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
        Effect::Weapons(d) => purse.weapons[weapon_slot(id, county.weapon_type, quirks)] += d,
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

