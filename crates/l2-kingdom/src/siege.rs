//! **Sieges on the campaign map** — laying one, building the engines, and
//! deciding whether an assault is possible at all.
//!
//! `docs/armies.md` §4 traces the chain; this is it. The gate it exists to open
//! is [`crate::conquest::can_be_entered`]: **a county holding both a castle and
//! a garrison cannot be walked into**, so without a siege there is no way to
//! take it and no way to win a game.
//!
//! # The chain, and where each piece is
//!
//! ```text
//! Army_BeginSiege     0x004A7CA2  begin_siege     the guard
//! Siege_Link          0x004A7E0A  link            the two back-pointers
//! Siege_Prepare       0x004A7EB5  prepare         clear the records; the AI's order
//! Siege_RecomputeBuildTime 0x004A80DB recompute_build_time  percent and seasons
//! Siege_StartPhase    0x004A82B9  start_phase     break stale links, seed the cursor
//! Siege_ValidateLink  0x004A8426  validate_link
//! Siege_TickPhase     0x004A84BA  tick_phase      the resumable pump
//! Siege_BuildTick     0x004A8507  build_tick      one season of construction
//! Siege_LaunchAssault 0x004A8AAB  assault         the level, the gate, the battle
//! Siege_Break         0x0043B917  break_siege     three callers, all of them lifts
//! Army_PrepareForBattle 0x004AA6CA prepare_besieger / garrison_oil
//! ```
//!
//! # Engines are built on the spot, and the army is pinned while they are
//!
//! Nothing is carried to a siege. [`ENGINE_WORK`] is three man-season costs and
//! [`build_tick`] spends the besieging army's whole strength on them once a
//! season, so **an army of 400 building two towers is ready next season and the
//! same army ordering three rams waits three**. The army does not move while it
//! builds — not because anything pins it, but because *any* successful move
//! order calls [`break_siege`]. `L2.eng` 10/13 *"Lift the siege?"* is a warning,
//! not a veto. `[V]`
//!
//! # What the original's own Readme says, and it agrees
//!
//! `Readme.txt` in the shipped install — *"LORDS OF THE REALM ROYAL EDITION
//! Additional Notes"*, the errata the printed manual could not carry — states
//! two of this module's rules in English:
//!
//! * **Capturing Counties (pg76)**: *"If a garrisoned castle is present in the
//!   county, it must be attacked instead of the county town to gain control of
//!   the county."* That is [`crate::conquest::can_be_entered`] verbatim, and it
//!   promotes the gate from a reading of one `if` to **[V]**.
//! * **Besieged Castles (p.87)**: *"When one of your castles is under siege,
//!   you may only leave the castle to engage the sieging force, and you may not
//!   enter the castle or strengthen the garrison until the siege is lifted."*
//!   [`garrison_is_besieged`] is that rule, and `L2.eng` 289 —
//!   *"…As it is currently under siege !!"* — is the refusal the original
//!   prints.
//!
//! # Scope
//!
//! Everything here is campaign arithmetic. The battle a siege produces is
//! `l2-sim`'s, and the two are joined in `l2-game`'s seam exactly as an
//! ordinary battle is; the only difference this module makes to that seam is
//! [`assault_castle_level`], which is the argument
//! [`crate::battle::auto_resolve`] has always taken and nothing has ever
//! passed.

use crate::county::{County, MAX_COUNTIES};
use crate::math::pct_of;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{Unit, UnitKind, Units, MAX_UNITS};

/// The three engine types, in the order their records sit in the unit record
/// (`+0x182`, `+0x188`, `+0x18E` — stride 6) and in `g_siegeEngineWork`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Engine {
    Catapult = 0,
    SiegeTower = 1,
    BatteringRam = 2,
}

/// The three, in record order. A caller drawing the siege-preparation screen
/// walks this: `L2.eng` group 83 lists *"Catapults / Siege towers / Battering
/// rams"* in exactly this order.
pub const ENGINES: [Engine; 3] = [Engine::Catapult, Engine::SiegeTower, Engine::BatteringRam];

impl Engine {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Engine> {
        ENGINES.get(i).copied()
    }

    /// The battle troop type this engine becomes when the battle starts —
    /// 7 catapult, 8 siege tower, 9 battering ram. `Army_PrepareForBattle`
    /// copies the counts into `+0x17A`, `+0x17C` and `+0x17E`, which are
    /// `troops[7..=9]`.
    pub fn troop_type(self) -> usize {
        self.index() + 7
    }

    pub fn name(self) -> &'static str {
        match self {
            Engine::Catapult => "Catapults",
            Engine::SiegeTower => "Siege towers",
            Engine::BatteringRam => "Battering rams",
        }
    }
}

/// `g_siegeEngineWork` (`0x004DE440`) — man-seasons one engine of each type
/// costs. Read out of `Lords2.exe`, three `i32` followed by a zero and then
/// string data, which is what fixes the count at three. `[V]`
///
/// **A ram is worth two of anything else**, and that is the whole of the
/// trade-off the siege-preparation screen offers.
pub const ENGINE_WORK: [i32; 3] = [200, 200, 400];

/// How many of each engine the siege-preparation screen will let a player
/// order — **four catapults, four towers, two rams**.
///
/// `[V]`, and new here. The screen's increment handler (`0x0043B681`) picks its
/// ceiling from the hotspot id — `4` for hotspots 0 and 1, `2` for anything
/// else — and refuses to increment at it; the decrement handler
/// (`0x0043B741`) refuses at zero. Nobody had read those two functions, so
/// `docs/armies.md` §4 knew the button existed and not what it was bounded by.
///
/// The cap is on the *screen*, not on the record: [`prepare`]'s AI branch
/// writes 4 towers for the Knight without consulting it, and 3 catapults *plus*
/// 2 towers for the Countess, which no sequence of clicks could produce at
/// once. So this is a rule about the player, and the asymmetry is the
/// original's.
pub const ENGINE_ORDER_CAP: [i16; 3] = [4, 4, 2];

/// The defender's boiling-oil count by **castle level** 0…4 — that is,
/// `castleType - 1`. `Army_PrepareForBattle`'s mode-0 branch, a plain switch on
/// `g_castleLevel`. `[V]`
///
/// The Readme's *Boiling Oil (pg94)* — *"Oil is designed for use by the
/// besieged for defense of the castle"* — is the same statement in English:
/// only the garrison ever gets any.
pub const OIL_BY_CASTLE_LEVEL: [i32; 5] = [1, 2, 3, 4, 6];

/// The castle level at and above which an assault **requires** siege engines.
///
/// `Siege_LaunchAssault`'s gate is `level < 3 || engines > 0`, and `L2.eng` 281
/// is the same sentence: *"Your captains advise that you must build some siege
/// engines to besiege this castle."* Level 3 is a stone castle (type 4), so a
/// palisade, a motte and bailey and a Norman keep can be stormed bare-handed
/// and nothing above them can. `[V]`
pub const ENGINES_REQUIRED_FROM_LEVEL: u8 = 3;

/// One engine type's build record — `+0x182 + e*6`, three `i16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineBuild {
    /// `+0` — how many were ordered.
    pub ordered: i16,
    /// `+2` — percent complete, 0…100. Derived from the other two every time
    /// either changes, and stored because the screen prints it.
    pub percent: i16,
    /// `+4` — man-seasons of work done against `ordered * ENGINE_WORK[e]`.
    pub work_done: i16,
}

impl EngineBuild {
    /// The work this record still needs, or 0 once it is complete.
    pub fn remaining(&self, engine: Engine) -> i32 {
        if self.ordered < 1 {
            return 0;
        }
        (ENGINE_WORK[engine.index()] * self.ordered as i32 - self.work_done as i32).max(0)
    }

    pub fn is_complete(&self) -> bool {
        self.percent >= 100
    }
}

/// Why [`begin_siege`] refused.
///
/// `Army_BeginSiege` is a single four-clause `if` with no else, so a refusal is
/// silent in the original — the map click simply does nothing. Naming the four
/// clauses is what lets the map layer print `L2.eng` 284 / 285 / 289 / 275,
/// which `docs/armies.md` §9 pairs with exactly these conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeRefusal {
    /// The unit is not an army, or the slot is empty.
    NotAnArmy,
    /// The county has no garrison. `L2.eng` 285 — *"This castle is deserted my
    /// liege. Your enemies await you in the county town."*
    NoGarrison,
    /// County `+0x1C2`, the castle-ruined flag. `L2.eng` 165 names it.
    CastleRuined,
    /// `castleDegraded == 1` with nothing being built. `L2.eng` 284 — *"This
    /// castle is under construction my liege."*
    CastleUnderConstruction,
    /// Somebody else is already besieging it. `L2.eng` 275 — *"…you must join
    /// with or dispose of the existing siegers."*
    AlreadyBesieged,
}

/// `Army_BeginSiege` (`0x004A7CA2`) — an army lays siege to a county's castle.
///
/// ```c
/// if (county.garrisonUnit != 0 && county.castleRuined == 0
///     && (county.castleDegraded != 1 || county.castleBuilding != 0)
///     && units[county.garrisonUnit].besiegedBy == 0)
///     Siege_Link(unit, county);
/// ```
///
/// **The guard does not check whose garrison it is.** Besieging your own
/// castle is not refused here; the map's hover test is what never offers the
/// order, and `Army_Garrison` is the sibling call for a castle that is yours
/// (`docs/armies.md` §9's target table: *your* county → `Army_Garrison`,
/// *theirs* → `Army_BeginSiege`). Reproduced as the original has it, with the
/// ownership left to the caller that decides which of the two to call.
pub fn begin_siege(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
    season: u8,
) -> Result<(), SiegeRefusal> {
    match units.get(army) {
        Some(u) if u.kind == UnitKind::Army => {}
        _ => return Err(SiegeRefusal::NotAnArmy),
    }
    let Some(c) = counties.get(county as usize) else { return Err(SiegeRefusal::NotAnArmy) };
    if c.garrison_unit == 0 {
        return Err(SiegeRefusal::NoGarrison);
    }
    if c.castle_ruined {
        return Err(SiegeRefusal::CastleRuined);
    }
    if c.castle_degraded == CASTLE_DEGRADED_BUILDING && c.castle_building == 0 {
        return Err(SiegeRefusal::CastleUnderConstruction);
    }
    if units.get(c.garrison_unit).is_some_and(|g| g.besieged_by != 0) {
        return Err(SiegeRefusal::AlreadyBesieged);
    }
    link(t, counties, realms, units, army, county, season);
    Ok(())
}

/// `Siege_Link` (`0x004A7E0A`) — the two back-pointers, then [`prepare`].
///
/// The garrison's `+0x19A` and the besieger's `+0x199` are the pair
/// [`crate::unit::destroy`] already knows how to unpick, and
/// [`validate_link`] is what notices when they stop agreeing.
pub fn link(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    county: u8,
    season: u8,
) {
    let garrison = counties[county as usize].garrison_unit;
    if let Some(u) = units.get_mut(army) {
        u.needs_destination = true;
        u.besieging_county = county;
    }
    if let Some(g) = units.get_mut(garrison) {
        g.besieged_by = army as u8;
    }
    prepare(t, counties, realms, units, army, season);
}

/// `Siege_Prepare` (`0x004A7EB5`) — clear the three build records, and for an
/// **AI** owner write the order its lord's doctrine names.
///
/// # The doctrine field is `personality +0xA0`, and it was untraced until now
///
/// `docs/diplomacy.md` §8.4 lists eleven fields of the 240-byte personality
/// record that *"hold plausible per-lord values and were never traced"*, and
/// `+0xA0` is one of them. This function is its only reader. Read out of
/// `Lords2.exe` at `0x004D8A58 + (lord-1)*0xF0 + 0xA0`, the four values are
///
/// | lord | `+0xA0` | orders |
/// |---|---:|---|
/// | 1 the Knight | **8** | 4 siege towers |
/// | 2 the Baron | **9** | 1 battering ram, and the default 2 towers |
/// | 3 the Countess | **7** | 3 catapults, the default 2 towers, and a ram against a stone or royal castle after season 2 |
/// | 4 the Bishop | **7** | the same |
///
/// **[V]** — three of the four values are exactly the three constants the
/// function tests and the fourth repeats one of them, which a field that meant
/// something else would not do. Two consequences fall out and both are worth
/// stating: the *"default: 2 towers"* arm is **unreachable for every shipped
/// lord**, and the Knight is the only lord who never brings artillery.
///
/// # The orders are cumulative, not alternative
///
/// The tower count is written **before** the personality tests and only the
/// `== 8` arm overwrites it, so the Countess builds 3 catapults *and* 2 towers
/// — 600 + 400 = **1,000 man-seasons**, not 600. `docs/armies.md` §4 already
/// said so; this is where it is enforced.
pub fn prepare(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    army: usize,
    season: u8,
) {
    let Some(u) = units.get_mut(army) else { return };
    u.siege_seasons_left = 0;
    u.engines = [EngineBuild::default(); 3];
    if u.owner_is_human {
        return;
    }
    let county = u.besieging_county;
    let owner = u.owner;
    let castle_type = counties.get(county as usize).map_or(0, |c| c.castle_type);
    let doctrine = realms
        .get(owner as usize)
        .and_then(|r| siege_doctrine(t, r.lord))
        .unwrap_or(0);

    u.engines[Engine::SiegeTower.index()].ordered = 2;
    if doctrine == 8 {
        u.engines[Engine::SiegeTower.index()].ordered = 4;
    }
    if doctrine == 9 {
        u.engines[Engine::BatteringRam.index()].ordered = 1;
    }
    if doctrine == 7 {
        u.engines[Engine::Catapult.index()].ordered = 3;
        if castle_type > 3 && season > 2 {
            u.engines[Engine::BatteringRam.index()].ordered = 1;
        }
    }
    recompute_build_time(units, army);
}

/// The lord's `personality +0xA0`, or `None` for the human (lord 0) and for a
/// lord past the four the table holds.
pub fn siege_doctrine(t: &Tables, lord: u8) -> Option<i32> {
    if lord == 0 {
        return None;
    }
    t.ai.personality.get(lord as usize - 1).map(|p| p.siege_doctrine)
}

/// `Siege_RecomputeBuildTime` (`0x004A80DB`) — refresh each record's percentage
/// and write the seasons the screen prints.
///
/// ```c
/// remaining = 0;
/// for e in 0..3 {
///     if (ordered[e] < 1) { percent[e] = 0; work[e] = 0; continue; }
///     total = ENGINE_WORK[e] * ordered[e];
///     percent[e] = work[e] * 100 / total;
///     if (percent[e] < 100) remaining += total - work[e]; else percent[e] = 100;
/// }
/// siegeSeasonsLeft = ceil(remaining / men);
/// ```
///
/// **An army with no men does nothing at all** — the whole body is inside
/// `if (menTotal > 0)`, so `siegeSeasonsLeft` keeps whatever it held. That is
/// reproduced rather than tidied: a besieger starved to nothing must not
/// silently report its engines ready.
///
/// Returns the seasons written, or the field's existing value when there were
/// no men to divide by.
pub fn recompute_build_time(units: &mut Units, army: usize) -> u8 {
    let Some(u) = units.get_mut(army) else { return 0 };
    let men = u.men;
    if men < 1 {
        return u.siege_seasons_left;
    }
    let mut remaining = 0i32;
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 {
            record.percent = 0;
            record.work_done = 0;
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        record.percent = pct_of(done, total) as i16;
        if record.percent < 100 {
            remaining += total - done;
        } else {
            record.percent = 100;
        }
    }
    u.siege_seasons_left = crate::math::div_ceil(remaining, men).clamp(0, 255) as u8;
    u.siege_seasons_left
}

/// `Siege_BuildTick` (`0x004A8507`) — **one season of construction**, and the
/// only thing turn phase 2 does.
///
/// Three sweeps over the three records:
///
/// 1. **an even share.** `men / incomplete` goes to each record that is ordered
///    and unfinished, capped at what that record still needs; whatever a record
///    could not absorb stays in the pot.
/// 2. **the spill.** The pot is walked over the same records in index order,
///    each taking as much as it can — so an army whose towers finish early
///    puts the rest of its season into the rams. This is why a mixed order does
///    not idle.
/// 3. **the recount**, identical to [`recompute_build_time`]'s body.
///
/// Returns true when `siegeSeasonsLeft` reaches zero — which is what
/// [`tick_phase`] yields on. **An army with no engines ordered at all returns
/// true on its first tick**, because zero work needs zero seasons; that is the
/// bare-handed storm of a small castle, and [`assault_castle_level`]'s gate is
/// what decides whether it is allowed.
pub fn build_tick(units: &mut Units, army: usize) -> bool {
    let Some(u) = units.get_mut(army) else { return false };
    if u.owner == 0 || u.kind != UnitKind::Army || u.besieging_county == 0 {
        return false;
    }
    let men = u.men;
    if men < 1 {
        return false;
    }

    let incomplete = |u: &Unit| {
        (0..3).filter(|&e| u.engines[e].ordered > 0 && u.engines[e].percent < 100).count() as i32
    };
    let n = incomplete(u);
    let share = if n != 0 { men / n } else { men };

    // 1 — the even share.
    let mut pot = men;
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 || record.percent >= 100 {
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        let spent = if share < total - done {
            record.work_done += share as i16;
            share
        } else {
            record.work_done = total as i16;
            total - done
        };
        pot -= spent;
    }

    // 2 — the spill, in index order.
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 || record.percent >= 100 {
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        if done >= total {
            continue;
        }
        if pot < total - done {
            record.work_done += pot as i16;
            pot = 0;
        } else {
            record.work_done = total as i16;
            pot -= total - done;
        }
    }

    // 3 — the recount. The original repeats the body rather than calling
    // `Siege_RecomputeBuildTime`, and it differs in one way that matters: the
    // sweep skips records that were already complete when the tick began, so a
    // finished record's percentage is not rewritten. It is already 100.
    let mut remaining = 0i32;
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 || record.percent >= 100 {
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        record.percent = pct_of(done, total) as i16;
        if record.percent < 100 {
            remaining += total - done;
        } else {
            record.percent = 100;
        }
    }
    u.siege_seasons_left = crate::math::div_ceil(remaining, men).clamp(0, 255) as u8;
    u.siege_seasons_left == 0
}

/// `Siege_ValidateLink` (`0x004A8426`) — break a besieger's link when the
/// castle it is sitting outside no longer has the garrison it was pointed at.
///
/// Two clauses, and the second is the one that is easy to miss: the county's
/// garrison slot must still hold a unit *and* that unit must still name this
/// county as the castle it is inside. A garrison that marched out leaves the
/// first test passing and the second failing.
pub fn validate_link(counties: &[County; MAX_COUNTIES], units: &mut Units, army: usize) {
    let Some(county) = units.get(army).map(|u| u.besieging_county) else { return };
    let garrison = counties.get(county as usize).map_or(0, |c| c.garrison_unit);
    let still_inside = units.get(garrison).is_some_and(|g| g.garrison_county == county);
    if garrison == 0 || !still_inside {
        if let Some(u) = units.get_mut(army) {
            u.besieging_county = 0;
        }
    }
}

/// The resumable cursor turn phase 2 walks. `g_siegeCursor`, `g_siegeCount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeCursor {
    /// `g_siegeCursor` — the next unit slot [`tick_phase`] will look at.
    pub at: usize,
    /// `g_siegeCount` — how many live besieging armies [`start_phase`] found.
    /// The original uses it for one thing only: a battle outcome banner is
    /// skipped when more than one siege is in progress and the battle is a
    /// siege (`Battle_CheckOutcome`'s `g_siegeCount < 2` test).
    pub count: u32,
}

/// `Siege_StartPhase` (`0x004A82B9`) — the first call of turn phase 2.
///
/// Breaks every garrison/besieger pair that no longer agrees, runs
/// [`validate_link`] over every besieging army, and seeds the cursor at 1.
///
/// **It does not call [`prepare`].** `docs/armies.md` §4 used to say it
/// *"re-prepares every besieging army"*; it breaks stale links, and re-preparing
/// would wipe the player's engine order every season.
pub fn start_phase(counties: &[County; MAX_COUNTIES], units: &mut Units) -> SiegeCursor {
    // The county sweep runs 1..=16 in the original — `MAX_COUNTY_ID`, not the
    // map's county count.
    for county in 1..=crate::county::MAX_COUNTY_ID {
        let garrison = counties[county as usize].garrison_unit;
        if garrison == 0 {
            continue;
        }
        let besieger = units.get(garrison).map_or(0, |g| g.besieged_by) as usize;
        if besieger == 0 {
            continue;
        }
        let agrees = units.get(besieger).is_some_and(|b| b.besieging_county == county);
        if !agrees {
            if let Some(g) = units.get_mut(garrison) {
                g.besieged_by = 0;
            }
            if let Some(b) = units.get_mut(besieger) {
                b.besieging_county = 0;
            }
        }
    }

    let mut count = 0;
    for id in 1..=MAX_UNITS {
        let besieging = units
            .get(id)
            .is_some_and(|u| u.owner != 0 && u.kind == UnitKind::Army && u.besieging_county != 0);
        if besieging {
            validate_link(counties, units, id);
            // `g_siegeCount` is incremented unconditionally inside
            // `Siege_ValidateLink` — **including for the link it just broke**.
            // Reproduced: the count is "armies that were besieging when the
            // phase began", and the one consumer only asks whether it is
            // above 1.
            count += 1;
        }
    }
    SiegeCursor { at: 1, count }
}

/// `Siege_TickPhase` (`0x004A84BA`) — the pump.
///
/// Walks the cursor from where it stands to [`MAX_UNITS`], calling
/// [`build_tick`], and **stops on the first army whose engines came in**,
/// leaving the cursor on it. Returns that army; `None` means the sweep is
/// exhausted and the phase is over.
///
/// The caller runs [`assault`] on the army returned and calls this again. The
/// cursor is deliberately *not* advanced past it: the assault clears the siege
/// link one way or another, so the next call's [`build_tick`] on the same slot
/// returns false and the cursor moves on by itself.
pub fn tick_phase(cursor: &mut SiegeCursor, units: &mut Units) -> Option<usize> {
    while cursor.at <= MAX_UNITS {
        if build_tick(units, cursor.at) {
            return Some(cursor.at);
        }
        cursor.at += 1;
    }
    None
}

/// `Siege_Break` (`0x0043B917`) — lift a siege.
///
/// Three callers, and together they are the whole answer to *"the lift-siege
/// handler was not traced"*:
///
/// * **any successful move order** on a type-1 unit (`Unit_OrderMove`), so
///   giving a besieging army anywhere to go lifts its siege — `L2.eng` 10/13
///   *"Lift the siege?"* is a confirmation, not a veto;
/// * the siege-preparation screen's *"Lift siege"* button, `L2.eng` 83/6;
/// * [`assault`], when the castle is too strong to storm without engines.
pub fn break_siege(counties: &[County; MAX_COUNTIES], units: &mut Units, army: usize) {
    let Some(county) = units.get(army).map(|u| u.besieging_county) else { return };
    if county == 0 {
        return;
    }
    let garrison = counties.get(county as usize).map_or(0, |c| c.garrison_unit);
    if let Some(u) = units.get_mut(army) {
        u.besieging_county = 0;
    }
    if let Some(g) = units.get_mut(garrison) {
        g.besieged_by = 0;
    }
}

/// County `+0x1C3` when the castle is **under construction**. The byte holds
/// 0, 1 or 2 and [`assault_castle_level`] is what reads all three.
pub const CASTLE_DEGRADED_BUILDING: u8 = 1;
/// County `+0x1C3` when the castle has been **knocked down in a siege**, and
/// `+0x1F9` holds the level that is left standing.
///
/// > **Nothing in this workspace writes it, and that is a known hole rather
/// > than an oversight.** The other two values of the byte have reachable
/// > writers now — [`crate::industry::order_castle`] sets 1 and
/// > [`crate::industry::build_tick`] clears it — but 2 belongs to the
/// > end-of-siege bookkeeper, `FUN_004784CA` (`0x004784CA`), which is called
/// > from `Battle_ReturnToCampaign`'s siege arm and does this:
/// >
/// > ```c
/// > if (!g_battleIsSiege || (breachDamage == 0 && wallDamage == 0)) return;
/// > county[+0x1E4..+0x1F1] = the battle's breach and approach scores;
/// > if (g_castleLevel < 2) { woodOwed  = woodTotal  = wallDamage * 10; }
/// > else                   { stoneOwed = stoneTotal = wallDamage * 15; }
/// > workLeft = workTotal = breachDamage * 5 + wallDamage * 15;
/// > county.castleLevelLeft = g_castleLevel;
/// > county.castleDegraded  = 2;
/// > county.percent         = 0;
/// > ```
/// >
/// > — adding to the totals rather than replacing them when a build was
/// > already under way, so **a wooden castle is repaired in wood and a stone
/// > one in stone**, and a siege on a half-built castle makes the job bigger.
/// >
/// > **This is [`record_castle_damage`] now**, and the note that used to stand
/// > here — *"not reproduced, because every number comes from two battle-side
/// > accumulators `l2-sim` does not have"* — is out of date in the part that
/// > matters and was right about the rest. `l2-sim` keeps both accumulators;
/// > the autocalc really does produce nothing, and that is the rule rather than
/// > a gap: `Battle_AutoResolve` never touches either global, so a siege the
/// > player declines to watch leaves the castle unmarked and the three readers
/// > below are reached only by a siege somebody **fought**.
pub const CASTLE_DEGRADED_DAMAGED: u8 = 2;

/// **What a siege left on a castle** — county `+0x1E4` … `+0x1F1`, the six
/// values `Siege_RecordCastleDamage` (`0x004784CA`) writes and `FUN_004787A4`
/// (`0x004787A4`) reads back into the battle when the *next* assault opens on
/// the same castle.
///
/// That pairing is the whole reason these are stored rather than consumed. A
/// besieger thrown off a half-wrecked castle comes back to a half-wrecked
/// castle: the moat it filled is still filled, the walls it opened are still
/// open, and the gate it broke is still broken. Without the round trip the six
/// numbers would be write-only, which is `docs/decisions.md` C27's shape and
/// the exact hole this type exists to avoid re-opening.
///
/// It is deliberately **not** `l2_sim::CastleDamage`, though the fields are the
/// same six: `l2-kingdom` is below `l2-sim` in nothing and beside it in the
/// dependency graph, and `docs/plan.md`'s one-way rule says neither simulation
/// learns the other exists. `l2-game`'s `engagement` module owns the
/// conversion, because it is the only crate that can see both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SiegeScars {
    /// `+0x1E4` — `DAT_0057A0D8`, **moat cells filled in**. Five man-seasons
    /// of digging each and not a stick of wood; see [`record_castle_damage`].
    pub moat_filled: u16,
    /// `+0x1E6` — `DAT_0056D648`, **rampart cells left hanging by a
    /// collapse**. This is the number the repair is billed in wood or stone.
    pub wall_damage: u16,
    /// `+0x1E8` — `g_siegeBreachScore` as the battle ended.
    pub breach_score: i32,
    /// `+0x1EC` — `g_siegeApproachScore` as the battle ended.
    pub approach_score: i32,
    /// `+0x1F0` — `_DAT_0055307C`, rampart patches down.
    pub ramparts_breached: u8,
    /// `+0x1F1` — `_DAT_00569588`, the gate is open. Set by the
    /// twenty-thousandth hit and by the garrison's own drawbridge.
    pub gate_open: bool,
}

impl SiegeScars {
    /// `Siege_RecordCastleDamage`'s opening `if`: it does nothing at all unless
    /// one of the two accumulators is non-zero.
    pub fn any(&self) -> bool {
        self.moat_filled != 0 || self.wall_damage != 0
    }
}

/// Wood owed per point of [`SiegeScars::wall_damage`], **below castle level
/// 2** — a palisade or a motte and bailey, which are made of wood.
pub const REPAIR_WOOD_PER_WALL: i32 = 10;
/// Stone owed per point, at level 2 and above.
pub const REPAIR_STONE_PER_WALL: i32 = 15;
/// Man-seasons per point of [`SiegeScars::wall_damage`], either way.
pub const REPAIR_WORK_PER_WALL: i32 = 15;
/// Man-seasons per moat cell filled in — the digging, and the only thing the
/// moat costs.
pub const REPAIR_WORK_PER_MOAT: i32 = 5;

/// **Bill the repair** — `Siege_RecordCastleDamage` (`0x004784CA`), called from
/// `Battle_ReturnToCampaign`'s siege arm and the **only** writer of
/// [`CASTLE_DEGRADED_DAMAGED`] in the whole binary.
///
/// ```c
/// if (!g_battleIsSiege || (DAT_0057A0D8 == 0 && DAT_0056D648 == 0)) return;
/// county[+0x1E4 .. +0x1F1] = the six numbers the battle finished with;
/// if (g_castleLevel < 2) { woodOwed  += wallDamage * 10; woodTotal  += same; }
/// else                   { stoneOwed += wallDamage * 15; stoneTotal += same; }
/// workLeft += moatFilled * 5 + wallDamage * 15;  workTotal += same;
/// county.castleLevelLeft = g_castleLevel;
/// county.castleDegraded  = 2;
/// county.castlePercent   = 0;
/// ```
///
/// Four things follow, and three of them are visible to a player.
///
/// * **A castle is repaired in the material it is made of.** Wood below level
///   2, stone at 2 and above, and never both. `docs/bugs.md` B69.
/// * **The `+=` is real.** The original writes plain `=` when
///   `castleDegraded != 1` and `x = x + y` when it is 1, so besieging a castle
///   that is *already being built* makes the job bigger than the castle was —
///   the scaffolding's bill and the siege's are added together and paid once.
/// * **Filling in the moat costs work and no materials.** `moatFilled` is only
///   ever multiplied by 5 into the work total; it never reaches the wood or
///   stone line. So a besieger who shovels the ditch full and is then thrown
///   off has cost the defender labour and nothing else.
/// * **`castlePercent` is reset to 0**, which is what puts the scaffolding
///   back on the map tile: `Castle_StampTile` reads `< 50` as scaffolding.
///
/// > **`docs/symbols.md` calls `DAT_0057A0D8` `breachDamage` and that is a
/// > misnomer** — the whole binary holds three writers of it and the only one
/// > that adds is the moat fill. The parameter is named for what writes it.
/// > `CNEW-moat-damage`.
///
/// Answers whether anything was billed.
pub fn record_castle_damage(county: &mut County, castle_level: u8, scars: SiegeScars) -> bool {
    if !scars.any() {
        return false;
    }
    let already_building = county.castle_degraded == CASTLE_DEGRADED_BUILDING;
    let add = |slot: &mut i32, amount: i32| {
        *slot = if already_building { *slot + amount } else { amount };
    };

    county.siege_scars = scars;

    let wall = scars.wall_damage as i32;
    if castle_level < 2 {
        let bill = wall * REPAIR_WOOD_PER_WALL;
        add(&mut county.castle_wood_owed, bill);
        add(&mut county.castle_wood_total, bill);
    } else {
        let bill = wall * REPAIR_STONE_PER_WALL;
        add(&mut county.castle_stone_owed, bill);
        add(&mut county.castle_stone_total, bill);
    }
    let work = scars.moat_filled as i32 * REPAIR_WORK_PER_MOAT + wall * REPAIR_WORK_PER_WALL;
    add(&mut county.castle_work_left, work);
    add(&mut county.castle_work_total, work);

    county.castle_level_left = castle_level;
    county.castle_degraded = CASTLE_DEGRADED_DAMAGED;
    county.castle_percent = 0;
    true
}

/// **The other half of the round trip** — `FUN_004787A4` (`0x004787A4`), which
/// `Battle_Start` runs on the way *into* an assault.
///
/// ```c
/// if (county.castleDegraded == 2) { the six globals = county[+0x1E4 .. +0x1F1]; }
/// else                            { county[+0x1E4 .. +0x1F1] = 0; }
/// ```
///
/// So a castle carries its scars into the next assault, and a castle that is
/// *not* mid-repair has them cleared — which is what stops a rebuilt castle
/// inheriting the last siege's open gate. Both arms matter and only the first
/// one is obvious.
pub fn scars_for_assault(county: &mut County) -> SiegeScars {
    if county.castle_degraded == CASTLE_DEGRADED_DAMAGED {
        county.siege_scars
    } else {
        county.siege_scars = SiegeScars::default();
        SiegeScars::default()
    }
}

/// **Which castle is actually fought** — `Siege_LaunchAssault`'s opening, and
/// not simply `castleType`.
///
/// ```c
/// if (degraded == 1 && castleBuilding != 0) level = castleBuilding - 1;
/// else if (degraded == 2)                   level = county.castleLevelLeft;   /* +0x1F9 */
/// else                                      level = castleType - 1;
/// ```
///
/// The level is **zero-based**: type 1 (palisade) is level 0 and type 5 (royal)
/// is level 4, which is the indexing [`OIL_BY_CASTLE_LEVEL`] and
/// `crate::battle`'s castle-strength bonus both use. That is why this returns a
/// level and not a type, and why the two tables are five long.
///
/// `castleType == 0` — no castle at all — would give `-1`; the assault path
/// cannot reach it because [`begin_siege`] requires a garrison and a garrison
/// requires a castle, and it is clamped here rather than wrapped.
pub fn assault_castle_level(county: &County) -> u8 {
    if county.castle_degraded == CASTLE_DEGRADED_BUILDING && county.castle_building != 0 {
        county.castle_building.saturating_sub(1)
    } else if county.castle_degraded == CASTLE_DEGRADED_DAMAGED {
        county.castle_level_left
    } else {
        county.castle_type.saturating_sub(1)
    }
}

/// The gate: `level < 3 || engines > 0`. `L2.eng` 281 is this sentence in the
/// game's own words. `[V]`
pub fn can_assault(castle_level: u8, engines_built: i32) -> bool {
    castle_level < ENGINES_REQUIRED_FROM_LEVEL || engines_built > 0
}

/// What `Siege_LaunchAssault` decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assault {
    /// The battle is on: these two units, at this castle level. `attacker` is
    /// always the besieger and `defender` always the garrison — the mapping
    /// `crate::battle::outcome`'s four siege banners depend on.
    Battle { attacker: usize, defender: usize, castle_level: u8 },
    /// `level >= 3` with no engines: message `0x119` (`L2.eng` 281) and the
    /// siege is **lifted**. The army does not stall waiting; it gives up.
    NoEngines,
    /// The link had already gone — nothing to assault.
    NoSiege,
}

/// `Siege_LaunchAssault` (`0x004A8AAB`) — turn phase 2's payoff.
///
/// Works out the level, sums the three engine counts, applies the gate, and
/// either names the battle or lifts the siege. **It does not fight the
/// battle**: `Battle_ChooseSettlement` decides how that happens and the caller
/// owns it, exactly as [`crate::conquest::attack_county`] hands back
/// [`crate::conquest::Attack::Battle`] rather than resolving one.
pub fn assault(counties: &[County; MAX_COUNTIES], units: &mut Units, army: usize) -> Assault {
    let Some(county) = units.get(army).map(|u| u.besieging_county) else { return Assault::NoSiege };
    if county == 0 {
        return Assault::NoSiege;
    }
    let Some(c) = counties.get(county as usize) else { return Assault::NoSiege };
    let garrison = c.garrison_unit;
    if garrison == 0 {
        return Assault::NoSiege;
    }
    let level = assault_castle_level(c);
    let engines: i32 =
        units.get(army).map_or(0, |u| u.engines.iter().map(|r| r.ordered as i32).sum());
    if can_assault(level, engines) {
        Assault::Battle { attacker: army, defender: garrison, castle_level: level }
    } else {
        break_siege(counties, units, army);
        Assault::NoEngines
    }
}

/// The four battle-only troop slots, `+0x17A … +0x180`.
///
/// They are **not** part of [`Unit::troops`] here, and deliberately: the
/// original zeroes them again the moment the battle is over
/// (`Army_ClearBattleSlots`), nothing on the campaign map reads them, and a
/// field that exists only between two calls is a field a save has to carry for
/// no reason. They are produced on the way into a battle and discarded on the
/// way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BattleEngines {
    /// Troop type 7.
    pub catapults: i32,
    /// Troop type 8.
    pub siege_towers: i32,
    /// Troop type 9.
    pub battering_rams: i32,
    /// Troop type 10 — the defender's only.
    pub oil: i32,
}

impl BattleEngines {
    /// As four counts indexed by troop type 7…10, which is the order both
    /// `TROOPS*.ENG` and `l2_sim::Troop` use.
    pub fn counts(&self) -> [i32; 4] {
        [self.catapults, self.siege_towers, self.battering_rams, self.oil]
    }

    pub fn total(&self) -> i32 {
        self.counts().iter().sum()
    }
}

/// `Army_PrepareForBattle(unit, 4)` — the besieger's engines become troops.
///
/// **The counts that go into the battle are what was *ordered*, not what was
/// finished.** The original copies `+0x182`, `+0x188` and `+0x18E` — the
/// `ordered` field of each record — with no reference to the percentages, and
/// it can do that because [`tick_phase`] only reaches the assault when
/// `siegeSeasonsLeft` is zero, which means every ordered engine is complete.
/// Reproduced as written; a caller that assaults early gets engines it has not
/// paid for, and so does the original.
pub fn prepare_besieger(unit: &Unit) -> BattleEngines {
    BattleEngines {
        catapults: unit.engines[Engine::Catapult.index()].ordered as i32,
        siege_towers: unit.engines[Engine::SiegeTower.index()].ordered as i32,
        battering_rams: unit.engines[Engine::BatteringRam.index()].ordered as i32,
        oil: 0,
    }
}

/// `Army_PrepareForBattle(unit, 0)` — the garrison's boiling oil, by castle
/// level.
///
/// A level outside 0…4 gets none, which is the original's behaviour: the switch
/// has five arms and no default.
pub fn prepare_garrison(castle_level: u8) -> BattleEngines {
    BattleEngines {
        oil: OIL_BY_CASTLE_LEVEL.get(castle_level as usize).copied().unwrap_or(0),
        ..BattleEngines::default()
    }
}

/// The siege-preparation screen's `+` and `−` buttons — `0x0043B681` and
/// `0x0043B741`, both of which end in `0x0043B7C4`.
///
/// `delta` is `+1` or `−1`; anything else is refused. Returns true if the order
/// changed, in which case [`recompute_build_time`] has already run and the
/// caller should redraw. The caps are [`ENGINE_ORDER_CAP`].
pub fn order_engine(units: &mut Units, army: usize, engine: Engine, delta: i16) -> bool {
    let cap = ENGINE_ORDER_CAP[engine.index()];
    let Some(u) = units.get_mut(army) else { return false };
    let current = u.engines[engine.index()].ordered;
    let ok = match delta {
        1 => current < cap,
        -1 => current > 0,
        _ => false,
    };
    if !ok {
        return false;
    }
    u.engines[engine.index()].ordered = current + delta;
    recompute_build_time(units, army);
    true
}

/// **Is this county's garrison under siege?** — the Readme's *Besieged Castles
/// (p.87)* rule, and the guard `Army_Garrison` needs.
///
/// *"When one of your castles is under siege, you may only leave the castle to
/// engage the sieging force, and you may not enter the castle or strengthen the
/// garrison until the siege is lifted."* `L2.eng` 289 is the refusal, and
/// `docs/armies.md` §9's target table already paired that string with this
/// condition; the Readme is the second source that makes it **[V]**.
pub fn garrison_is_besieged(counties: &[County; MAX_COUNTIES], units: &Units, county: u8) -> bool {
    counties
        .get(county as usize)
        .map(|c| c.garrison_unit)
        .and_then(|g| units.get(g))
        .is_some_and(|g| g.besieged_by != 0)
}

/// **May this garrison move at all?** The other half of *Besieged Castles
/// (p.87)*: a besieged garrison *"may only leave the castle to engage the
/// sieging force"*.
///
/// Returns the besieger's slot when the garrison is pinned — the one
/// destination a sortie may have. `None` means the garrison is not besieged and
/// is free to march.
///
/// **`[I]`, and marked so.** The Readme states the rule and nothing in the
/// decompiled mover was found to enforce it: `Unit_OrderMove` calls
/// [`break_siege`] on the *besieger*, not on the garrison, and no guard on
/// `garrison_county` with a live `besieged_by` was located. So this is the
/// game's own documentation implemented in the absence of the code that does
/// it, and it is a function a caller must choose to call rather than a rule
/// wired into movement.
pub fn sortie_target(units: &Units, garrison: usize) -> Option<usize> {
    let besieger = units.get(garrison)?.besieged_by;
    (besieger != 0).then_some(besieger as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    const T: &Tables = &Tables::DEFAULT;

    fn besieged_county() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units) {
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        let mut units = Units::new();
        realms[1].in_play = true;
        realms[1].is_human = true;
        realms[2].in_play = true;
        realms[2].lord = 1;
        counties[4].owner = 2;
        counties[4].castle_type = 4;
        let mut garrison = Unit::new(UnitKind::Army, 2, 10, 10);
        garrison.men = 200;
        garrison.troops[0] = 200;
        garrison.garrison_county = 4;
        let g = units.spawn(garrison).unwrap();
        counties[4].garrison_unit = g;
        (counties, realms, units)
    }

    fn besieger(units: &mut Units, men: i32) -> usize {
        let mut a = Unit::new(UnitKind::Army, 1, 11, 10);
        a.owner_is_human = true;
        a.men = men;
        a.troops[0] = men;
        units.spawn(a).unwrap()
    }

    /// The guard, all four clauses.
    #[test]
    fn the_four_refusals_are_each_reachable() {
        let (mut counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);

        let saved = counties[4].garrison_unit;
        counties[4].garrison_unit = 0;
        assert_eq!(
            begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
            Err(SiegeRefusal::NoGarrison)
        );
        counties[4].garrison_unit = saved;

        counties[4].castle_ruined = true;
        assert_eq!(
            begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
            Err(SiegeRefusal::CastleRuined)
        );
        counties[4].castle_ruined = false;

        counties[4].castle_degraded = CASTLE_DEGRADED_BUILDING;
        counties[4].castle_building = 0;
        assert_eq!(
            begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
            Err(SiegeRefusal::CastleUnderConstruction)
        );
        counties[4].castle_degraded = 0;

        units.get_mut(counties[4].garrison_unit).unwrap().besieged_by = 99;
        assert_eq!(
            begin_siege(T, &counties, &realms, &mut units, army, 4, 1),
            Err(SiegeRefusal::AlreadyBesieged)
        );
        units.get_mut(counties[4].garrison_unit).unwrap().besieged_by = 0;

        assert_eq!(begin_siege(T, &counties, &realms, &mut units, army, 4, 1), Ok(()));
    }

    /// Laying a siege links both records, and lifting it unlinks both.
    #[test]
    fn the_link_is_a_pair_and_breaking_it_clears_both_halves() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        assert_eq!(units.get(army).unwrap().besieging_county, 4);
        assert_eq!(units.get(counties[4].garrison_unit).unwrap().besieged_by, army as u8);

        break_siege(&counties, &mut units, army);
        assert_eq!(units.get(army).unwrap().besieging_county, 0);
        assert_eq!(units.get(counties[4].garrison_unit).unwrap().besieged_by, 0);
    }

    /// `docs/armies.md` §4's worked example: 400 men, two towers, one season.
    #[test]
    fn four_hundred_men_build_two_towers_in_one_season() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        order_engine(&mut units, army, Engine::SiegeTower, 1);
        order_engine(&mut units, army, Engine::SiegeTower, 1);
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 1, "400 man-seasons over 400 men");
        assert!(build_tick(&mut units, army));
        assert_eq!(units.get(army).unwrap().engines[1].percent, 100);
    }

    /// And the other half of it: three rams is 1,200 man-seasons and takes
    /// three.
    #[test]
    fn the_same_army_ordering_rams_waits_three_seasons() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        // The screen caps rams at two, so the third is written past it — this
        // is the AI's reach, not a player's.
        units.get_mut(army).unwrap().engines[2].ordered = 3;
        recompute_build_time(&mut units, army);
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 3);
        assert!(!build_tick(&mut units, army));
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 2);
        assert!(!build_tick(&mut units, army));
        assert!(build_tick(&mut units, army));
    }

    /// The spill: a season's men that a finished record cannot absorb go to
    /// the one that is still short, which is why a mixed order does not idle.
    #[test]
    fn work_spills_from_a_finished_engine_onto_an_unfinished_one() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        // One tower (200) and one ram (400): 600 man-seasons over 400 men.
        units.get_mut(army).unwrap().engines[1].ordered = 1;
        units.get_mut(army).unwrap().engines[2].ordered = 1;
        recompute_build_time(&mut units, army);
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 2);

        build_tick(&mut units, army);
        let u = units.get(army).unwrap();
        // Even share is 200 each. The tower takes all 200 and is done; the ram
        // takes 200 of the 400 it needs, and there is nothing left to spill.
        assert_eq!(u.engines[1].percent, 100);
        assert_eq!(u.engines[2].work_done, 200);
        // Second season: the tower is complete, so all 400 go to the ram, which
        // needs 200. It finishes and the siege is ready.
        assert!(build_tick(&mut units, army));
    }

    /// The screen's caps, which are a rule about the player and not about the
    /// record.
    #[test]
    fn the_screen_caps_orders_at_four_four_and_two() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        for (engine, cap) in ENGINES.iter().zip(ENGINE_ORDER_CAP.iter()) {
            for _ in 0..10 {
                order_engine(&mut units, army, *engine, 1);
            }
            assert_eq!(units.get(army).unwrap().engines[engine.index()].ordered, *cap);
            for _ in 0..10 {
                order_engine(&mut units, army, *engine, -1);
            }
            assert_eq!(units.get(army).unwrap().engines[engine.index()].ordered, 0);
        }
    }

    /// The gate, in the game's own words: *"you must build some siege engines
    /// to besiege this castle."*
    #[test]
    fn a_big_castle_cannot_be_stormed_bare_handed_and_a_small_one_can() {
        assert!(can_assault(0, 0), "a palisade needs nothing");
        assert!(can_assault(2, 0), "a Norman keep needs nothing");
        assert!(!can_assault(3, 0), "a stone castle does");
        assert!(can_assault(4, 1), "one engine is enough for a royal castle");
    }

    /// And an army that refuses the gate does not stall — it gives up.
    #[test]
    fn a_besieger_with_no_engines_against_a_stone_castle_lifts_its_own_siege() {
        let (mut counties, realms, mut units) = besieged_county();
        counties[4].castle_type = 4; // level 3
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        assert_eq!(assault(&counties, &mut units, army), Assault::NoEngines);
        assert_eq!(units.get(army).unwrap().besieging_county, 0, "the siege was lifted");
        assert_eq!(units.get(counties[4].garrison_unit).unwrap().besieged_by, 0);
    }

    /// The level is not the type, and all three arms are reachable.
    #[test]
    fn the_castle_that_is_fought_is_not_always_the_castle_that_is_owned() {
        let mut c = County::new();
        c.castle_type = 5;
        assert_eq!(assault_castle_level(&c), 4, "a royal castle is level 4");

        c.castle_degraded = CASTLE_DEGRADED_BUILDING;
        c.castle_building = 3;
        assert_eq!(assault_castle_level(&c), 2, "the one being built, not the one owned");

        c.castle_degraded = CASTLE_DEGRADED_DAMAGED;
        c.castle_level_left = 1;
        assert_eq!(assault_castle_level(&c), 1, "what a previous siege left standing");
    }

    /// The four lords' doctrines, and the fact that the default is unreachable.
    #[test]
    fn every_shipped_lord_takes_a_named_branch_and_none_takes_the_default() {
        let doctrines: Vec<i32> = (1..=4).filter_map(|l| siege_doctrine(T, l)).collect();
        assert_eq!(doctrines, vec![8, 9, 7, 7], "Knight, Baron, Countess, Bishop");
        assert_eq!(siege_doctrine(T, 0), None, "the human has no doctrine");
    }

    /// The Knight storms with towers alone; the Countess brings artillery
    /// *and* the default towers, which is the cumulative rule.
    #[test]
    fn the_ai_orders_are_cumulative_and_not_alternative() {
        let (counties, mut realms, mut units) = besieged_county();
        realms[3].in_play = true;
        realms[3].lord = 3; // the Countess

        let mut knight = Unit::new(UnitKind::Army, 2, 11, 10);
        knight.men = 400;
        let k = units.spawn(knight).unwrap();
        units.get_mut(k).unwrap().besieging_county = 4;
        prepare(T, &counties, &realms, &mut units, k, 1);
        let e = units.get(k).unwrap().engines;
        assert_eq!((e[0].ordered, e[1].ordered, e[2].ordered), (0, 4, 0), "the Knight: 4 towers");

        let mut countess = Unit::new(UnitKind::Army, 3, 12, 10);
        countess.men = 400;
        let c = units.spawn(countess).unwrap();
        units.get_mut(c).unwrap().besieging_county = 4;
        prepare(T, &counties, &realms, &mut units, c, 1);
        let e = units.get(c).unwrap().engines;
        assert_eq!(
            (e[0].ordered, e[1].ordered, e[2].ordered),
            (3, 2, 0),
            "the Countess: 3 catapults AND the default 2 towers"
        );
        // 3 * 200 + 2 * 200 = 1000 man-seasons, not 600.
        assert_eq!(units.get(c).unwrap().siege_seasons_left, 3, "1000 over 400 men");
    }

    /// The Countess's late ram, which needs both halves of its condition.
    #[test]
    fn the_late_ram_needs_a_big_castle_and_a_late_season() {
        let (mut counties, mut realms, mut units) = besieged_county();
        realms[3].in_play = true;
        realms[3].lord = 3;
        let mut a = Unit::new(UnitKind::Army, 3, 12, 10);
        a.men = 400;
        let c = units.spawn(a).unwrap();
        units.get_mut(c).unwrap().besieging_county = 4;

        counties[4].castle_type = 3; // not > 3
        prepare(T, &counties, &realms, &mut units, c, 4);
        assert_eq!(units.get(c).unwrap().engines[2].ordered, 0, "a keep gets no ram");

        counties[4].castle_type = 5;
        prepare(T, &counties, &realms, &mut units, c, 2); // not > 2
        assert_eq!(units.get(c).unwrap().engines[2].ordered, 0, "not before season 3");

        prepare(T, &counties, &realms, &mut units, c, 3);
        assert_eq!(units.get(c).unwrap().engines[2].ordered, 1);
    }

    /// The pump stops on the army that is ready and leaves the cursor on it.
    #[test]
    fn the_cursor_stops_on_the_army_whose_engines_came_in() {
        let (counties, realms, mut units) = besieged_county();
        let slow = besieger(&mut units, 100);
        let fast = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, slow, 4, 1).unwrap();
        // Two armies cannot besiege the same castle in the original — the
        // second link overwrites the first — so this is the shape of the sweep
        // rather than a legal position.
        units.get_mut(slow).unwrap().besieging_county = 4;
        units.get_mut(fast).unwrap().besieging_county = 4;
        units.get_mut(slow).unwrap().engines[0].ordered = 4; // 800 over 100 men
        units.get_mut(fast).unwrap().engines[0].ordered = 1; // 200 over 400 men
        recompute_build_time(&mut units, slow);
        recompute_build_time(&mut units, fast);

        let mut cursor = SiegeCursor { at: 1, count: 2 };
        assert_eq!(tick_phase(&mut cursor, &mut units), Some(fast));
        assert_eq!(cursor.at, fast, "the cursor is left on the army that is ready");
        assert_eq!(units.get(slow).unwrap().siege_seasons_left, 7, "800-100 over 100");
    }

    /// A stale link is broken at the top of the phase, in both directions.
    #[test]
    fn the_phase_opens_by_breaking_links_that_no_longer_agree() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();

        // The garrison marched out: the slot still names it, but it no longer
        // names the county.
        let garrison = counties[4].garrison_unit;
        units.get_mut(garrison).unwrap().garrison_county = 0;
        start_phase(&counties, &mut units);
        assert_eq!(units.get(army).unwrap().besieging_county, 0);

        // **And the garrison's back-pointer survives that turn.**
        // `Siege_ValidateLink` clears `+0x199` and touches `+0x19A` not at all;
        // the county sweep that would clear it ran *before* the validation
        // pass in the same call. So the pair takes two turn-phase-2s to come
        // fully apart, and this asserts the lag rather than tidying it away.
        assert_eq!(units.get(garrison).unwrap().besieged_by, army as u8, "stale for one turn");
        start_phase(&counties, &mut units);
        assert_eq!(units.get(garrison).unwrap().besieged_by, 0, "and gone on the next");
    }

    /// The oil table, and that the attacker never gets any.
    #[test]
    fn only_the_garrison_gets_oil_and_the_count_is_the_castle_level() {
        for (level, expected) in OIL_BY_CASTLE_LEVEL.iter().enumerate() {
            assert_eq!(prepare_garrison(level as u8).oil, *expected);
        }
        assert_eq!(prepare_garrison(9).oil, 0, "no default arm");

        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.engines[0].ordered = 2;
        u.engines[2].ordered = 1;
        let e = prepare_besieger(&u);
        assert_eq!((e.catapults, e.siege_towers, e.battering_rams, e.oil), (2, 0, 1, 0));
    }

    /// The Readme's *Besieged Castles (p.87)* rule, both halves.
    #[test]
    fn a_besieged_garrison_cannot_be_reinforced_and_may_only_sortie() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        assert!(!garrison_is_besieged(&counties, &units, 4));
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        assert!(garrison_is_besieged(&counties, &units, 4));
        assert_eq!(sortie_target(&units, counties[4].garrison_unit), Some(army));
        break_siege(&counties, &mut units, army);
        assert_eq!(sortie_target(&units, counties[4].garrison_unit), None);
    }

    /// An army with no men neither builds nor reports itself ready — the
    /// original's `if (menTotal > 0)` wrapping the whole body.
    #[test]
    fn a_besieger_with_no_men_never_reports_its_engines_ready() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        order_engine(&mut units, army, Engine::Catapult, 1);
        units.get_mut(army).unwrap().men = 0;
        assert!(!build_tick(&mut units, army));
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 1, "unchanged, not zeroed");
    }
}
