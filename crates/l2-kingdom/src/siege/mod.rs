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
//! Nothing is carried to a siege. [`ENGINE_WORK`] is three man-season costs and
//! [`build_tick`] spends the besieging army's whole strength on them once a
//! season, so **an army of 400 building two towers is ready next season and the
//! same army ordering three rams waits three**. The army does not move while it
//! builds — because *any* successful move
//! order calls [`break_siege`]. `L2.eng` 10/13 *"Lift the siege?"* is a warning,
//! not a veto. `[V]`
//!
//! * **Capturing Counties (pg76)**: *"If a garrisoned castle is present in the
//! county, it must be attacked instead of the county town to gain control of
//!   the county."* That is [`crate::conquest::can_be_entered`] verbatim, and it
//!   promotes the gate from a reading of one `if` to **[V]**.
//!
//! * **Besieged Castles (p.87)**: *"When one of your castles is under siege,
//!   you may only leave the castle to engage the sieging force, and you may not
//!   enter the castle or strengthen the garrison until the siege is lifted."*
//!   [`garrison_is_besieged`] is that rule, and `L2.eng` 289 —
//!   *"…As it is currently under siege !!"* — is the refusal the original
//!   prints.

mod lifecycle;
pub use lifecycle::*;
mod engine;
pub use engine::*;
mod assault;
pub use assault::*;

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
pub const ENGINE_WORK: [i32; 3] = [200, 200, 400];

/// `[V]`, and new here. The screen's increment handler (`0x0043B681`) picks its
/// ceiling from the hotspot id — `4` for hotspots 0 and 1, `2` for anything
/// else — and refuses to increment at it; the decrement handler
/// (`0x0043B741`) refuses at zero. Nobody had read those two functions, so
/// `docs/armies.md` §4 knew the button existed and not what it was bounded by.
pub const ENGINE_ORDER_CAP: [i16; 3] = [4, 4, 2];

/// The defender's boiling-oil count by **castle level** 0…4 — that is,
/// `castleType - 1`. `Army_PrepareForBattle`'s mode-0 branch, a plain switch on
/// `g_castleLevel`. `[V]`
pub const OIL_BY_CASTLE_LEVEL: [i32; 5] = [1, 2, 3, 4, 6];

/// `Siege_LaunchAssault`'s gate is `level < 3 || engines > 0`, and `L2.eng` 281
/// is the same sentence: *"Your captains advise that you must build some siege
/// engines to besiege this castle."* Level 3 is a stone castle (type 4)
/// palisade, a motte and bailey and a Norman keep can be stormed bare-handed
/// and nothing above them can. `[V]`
pub const ENGINES_REQUIRED_FROM_LEVEL: u8 = 3;

/// One engine type's build record — `+0x182 + e*6`, three `i16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineBuild {
    pub ordered: i16,
    pub percent: i16,
    pub work_done: i16,
}

impl EngineBuild {
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

/// `Army_BeginSiege` is a single four-clause `if` with no else
/// silent in the original — the map click does nothing. Naming the four
/// clauses is what lets the map layer print `L2.eng` 284 / 285 / 289 / 275,
/// which `docs/armies.md` §9 pairs with exactly these conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiegeRefusal {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeCursor {
    pub at: usize,
    pub count: u32,
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
///
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
/// > — adding to the totals when a build was
/// > already under way, so **a wooden castle is repaired in wood and a stone
/// > one in stone**, and a siege on a half-built castle makes the job bigger.
pub const CASTLE_DEGRADED_DAMAGED: u8 = 2;

/// **What a siege left on a castle** — county `+0x1E4` … `+0x1F1`, the six
/// values `Siege_RecordCastleDamage` (`0x004784CA`) writes and `FUN_004787A4`
/// (`0x004787A4`) reads back into the battle when the *next* assault opens on
/// the same castle.
///
/// That pairing is the whole reason these are stored. A
/// besieger thrown off a half-wrecked castle comes back to a half-wrecked
/// castle: the moat it filled is still filled, the walls it opened are still
/// open, and the gate it broke is still broken. Without the round trip the six
/// numbers would be write-only, which is `docs/decisions.md` C27's shape and
/// the exact hole this type exists to avoid re-opening.
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

    #[test]
    fn the_same_army_ordering_rams_waits_three_seasons() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        units.get_mut(army).unwrap().engines[2].ordered = 3;
        recompute_build_time(&mut units, army);
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 3);
        assert!(!build_tick(&mut units, army));
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 2);
        assert!(!build_tick(&mut units, army));
        assert!(build_tick(&mut units, army));
    }

    #[test]
    fn work_spills_from_a_finished_engine_onto_an_unfinished_one() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();
        units.get_mut(army).unwrap().engines[1].ordered = 1;
        units.get_mut(army).unwrap().engines[2].ordered = 1;
        recompute_build_time(&mut units, army);
        assert_eq!(units.get(army).unwrap().siege_seasons_left, 2);

        build_tick(&mut units, army);
        let u = units.get(army).unwrap();
        assert_eq!(u.engines[1].percent, 100);
        assert_eq!(u.engines[2].work_done, 200);
        assert!(build_tick(&mut units, army));
    }

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

    #[test]
    fn a_big_castle_cannot_be_stormed_bare_handed_and_a_small_one_can() {
        assert!(can_assault(0, 0), "a palisade needs nothing");
        assert!(can_assault(2, 0), "a Norman keep needs nothing");
        assert!(!can_assault(3, 0), "a stone castle does");
        assert!(can_assault(4, 1), "one engine is enough for a royal castle");
    }

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

    #[test]
    fn every_shipped_lord_takes_a_named_branch_and_none_takes_the_default() {
        let doctrines: Vec<i32> = (1..=4).filter_map(|l| siege_doctrine(T, l)).collect();
        assert_eq!(doctrines, vec![8, 9, 7, 7], "Knight, Baron, Countess, Bishop");
        assert_eq!(siege_doctrine(T, 0), None, "the human has no doctrine");
    }

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
        assert_eq!(units.get(c).unwrap().siege_seasons_left, 3, "1000 over 400 men");
    }

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

    #[test]
    fn the_cursor_stops_on_the_army_whose_engines_came_in() {
        let (counties, realms, mut units) = besieged_county();
        let slow = besieger(&mut units, 100);
        let fast = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, slow, 4, 1).unwrap();
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

    #[test]
    fn the_phase_opens_by_breaking_links_that_no_longer_agree() {
        let (counties, realms, mut units) = besieged_county();
        let army = besieger(&mut units, 400);
        begin_siege(T, &counties, &realms, &mut units, army, 4, 1).unwrap();

        let garrison = counties[4].garrison_unit;
        units.get_mut(garrison).unwrap().garrison_county = 0;
        start_phase(&counties, &mut units);
        assert_eq!(units.get(army).unwrap().besieging_county, 0);

        // `Siege_ValidateLink` clears `+0x199` and touches `+0x19A` not at all;
        // the county sweep that would clear it ran *before* the validation
        // pass in the same call. So the pair takes two turn-phase-2s to come
// fully apart, and this asserts the lag.
        assert_eq!(units.get(garrison).unwrap().besieged_by, army as u8, "stale for one turn");
        start_phase(&counties, &mut units);
        assert_eq!(units.get(garrison).unwrap().besieged_by, 0, "and gone on the next");
    }

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

