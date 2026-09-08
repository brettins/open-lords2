//! Industry, weapons, wages and castles — `docs/kingdom.md` §7.4 and §7.5.
//!
//! # Industry
//!
//! `Industry_Produce` (`0x0044EA92`) runs four times per county per season:
//!
//! | commodity | job (`L2.eng` group 74) | divisor | base | credited to |
//! |---|---|---:|---:|---|
//! | wood | 6 Wood cutting | 1 | **20%** | realm `+0x130` |
//! | iron | 4 Iron mining | 1 | 15% | realm `+0x120` |
//! | weapons | 7 Blacksmith | **4** | 15% | realm `+0x140 + type*4` |
//! | stone | 5 Stone quarrying | **2** | 15% | realm `+0x128` |
//!
//! (The job column is one lower than `docs/kingdom.md` §7.4's — see
//! [`crate::tables::JOB_COUNT`] — and the order the driver runs them in is
//! [`crate::tables::INDUSTRY_ORDER`], neither of the two orders the document
//! gives.)
//!
//! `output = min(resourceLimit, Pct(workers / divisor, efficiency))`. Both of
//! the terms `docs/kingdom.md` §7.4 leaves unexplained are now traced:
//!
//! * **`efficiency` is not the base, it ramps.** [`efficiency_ramp`] adds the
//!   base to *last season's* efficiency every season, capped at 100 — so a new
//!   mine starts at 15% and reaches full output in seven seasons. And with
//!   *Advanced Farming* off the whole mechanism is bypassed for a flat 80%,
//!   which means the shipped save's settings put every industry at 80% and the
//!   published *"30 serfs working at 15% efficiency"* describes the advanced
//!   game only.
//! * **`resourceLimit` is a literal 999 for wood, iron and stone** once the
//!   industry is enabled and has a resource, and 0 otherwise; for weapons it is
//!   the realm's share of the wood and iron the weapon costs. See
//!   [`resource_limit`].
//!
//! # Wages
//!
//! `Wages_ForUnit` (`0x004AD52B`) is the whole army upkeep rule, and troop type
//! does not enter it: a knight and a peasant cost the same. See
//! [`crate::realm::Realm::wage_for_unit`].

use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::report::Message;
use crate::tables::{
    Commodity, Tables, EFFICIENCY_MAX, EFFICIENCY_WITHOUT_ADVANCED_FARMING,
    RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT,
};

/// `PctOf(a, b) = a * 100 / b` — `FUN_00404DC1`, the companion to
/// [`crate::math::pct`]. Zero denominator gives zero, exactly as the original
/// does rather than dividing.
#[inline]
pub fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        ((a as i64 * 100) / b as i64) as i32
    }
}

/// `FUN_0044F248` — the efficiency ramp `docs/kingdom.md` §12 lists as unknown.
///
/// ```c
/// if (g_optAdvancedFarming == 0) return 80;
/// if (workers == 0)              return 0;
/// increment = base;
/// if (capacity < workers) increment = Pct(base, PctOf(capacity, workers));
/// eff = lastEfficiency + increment;
/// if (eff > 100)  eff = 100;
/// if (eff < base) eff = base;      /* the floor is the base, not zero */
/// return eff;
/// ```
///
/// Three things fall out of it that no summary of §7.4 would suggest:
///
/// * the efficiency **compounds season on season**, so an industry is worth
///   more the longer it has been running and a county that is conquered and
///   restarted is not;
/// * **overstaffing is self-defeating** — past `capacity` the increment is
///   scaled by `capacity / workers`, so twice the workers ramp at half the
///   rate, and the season's raw output rises while the improvement slows;
/// * with *Advanced Farming* off none of it happens and every industry sits at
///   a flat 80%, which is more than five times the base.
///
/// `[V]` on all of it; the `advanced_farming` global is `0x0053F25C`, the same
/// one `Grain_Sow` and `Fertility_Update` branch on.
pub fn efficiency_ramp(
    last_efficiency: i32,
    workers: i32,
    capacity: i32,
    base: i32,
    advanced_farming: bool,
) -> i32 {
    if !advanced_farming {
        return EFFICIENCY_WITHOUT_ADVANCED_FARMING;
    }
    if workers == 0 {
        return 0;
    }
    let mut increment = base;
    if capacity < workers {
        increment = pct(base, pct_of(capacity, workers));
    }
    let mut efficiency = last_efficiency + increment;
    if efficiency > EFFICIENCY_MAX {
        efficiency = EFFICIENCY_MAX;
    }
    if efficiency < base {
        efficiency = base;
    }
    efficiency
}

/// `FUN_0044EF4E` — the `resourceLimit` term.
///
/// For wood, iron and stone it is a **flag test, not a quantity**: the industry
/// must be enabled (`+0x297`), must have its resource (`+0x295`) and must not
/// be counting down a disablement (`+0x296`); if all three hold the limit is a
/// literal [`RESOURCE_LIMIT_UNLIMITED`] and otherwise it is zero. So the only
/// thing bounding a mine is the county's workers — up to 999 units a season,
/// which no plausible workforce reaches.
///
/// For **weapons** it is a real quantity: the realm's wood and iron each
/// divided by a denominator the driver computes across the whole realm, so
/// every county's blacksmith gets a share of one stockpile rather than the
/// first county emptying it. `[D]` on the denominators (`0x0057C904` and
/// `0x0056D628`, written by `FUN_0044F15B`, which was not traced); this crate
/// takes the caller's `weapon_share` for that and defaults it to 1, which is
/// the single-county case.
pub fn resource_limit(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: i32,
) -> i32 {
    let record = &county.industry[c.index()];
    if !record.enabled {
        return 0;
    }
    if c == Commodity::Weapons {
        let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
        let (wood, iron) = (t.weapon[weapon].wood, t.weapon[weapon].iron);
        let share = weapon_share.max(1);
        // Written the way the original writes it — `(stock * cost / share) /
        // cost`, multiplying by the cost and dividing by it again. That is not
        // a no-op once `share` exceeds 1: it rounds the share down to a whole
        // weapon's worth of stock. Kept rather than cancelled.
        let quota = |stock: i32, cost: i32| {
            ((stock as i64 * cost as i64 / share as i64) / cost as i64) as i32
        };
        let mut limit = RESOURCE_LIMIT_UNLIMITED;
        if wood != 0 {
            limit = limit.min(quota(realm.wood, wood));
        }
        if iron != 0 {
            limit = limit.min(quota(realm.iron, iron));
        }
        return limit.max(0);
    }
    if !record.has_resource || record.disabled_seasons != 0 {
        return 0;
    }
    RESOURCE_LIMIT_UNLIMITED
}

/// What one commodity's pass would produce, before it is credited anywhere.
pub fn output(t: &Tables, county: &County, c: Commodity, realm: &Realm, weapon_share: i32) -> i32 {
    let record = &county.industry[c.index()];
    let workers = county.labour[t.commodity[c.index()].job].max(0);
    let raw = pct(workers / t.commodity[c.index()].divisor, record.efficiency);
    raw.min(resource_limit(t, county, c, realm, weapon_share)).max(0)
}

/// One `Industry_Produce` pass: ramp the efficiency, produce, credit the realm,
/// and add to the county's running total.
///
/// A pass on an industry that is counting down a disablement produces nothing,
/// zeroes the running total and steps the counter — and reinstates the industry
/// when the counter reaches zero. That branch is the whole `else` of
/// `Industry_Produce`.
///
/// Weapons are the one commodity that *spends*: the blacksmith's output is
/// capped by what [`WEAPON_COST`] can be paid for out of the realm's wood and
/// iron. **`[I]`, and unchanged from before this module knew what
/// `resourceLimit` was.** The original debits `made * cost` from each
/// stockpile with **no clamp at all** — it relies on [`resource_limit`]'s
/// realm-wide share to keep the total demand inside the stock, and that share's
/// denominator (`FUN_0044F15B`) was not traced. So with a share of 1 the limit
/// is effectively the whole stockpile and this affordability clamp is what
/// actually binds. It is kept because a negative stockpile is worse than a
/// small divergence, and it is flagged here rather than presented as the
/// original's rule.
pub fn produce(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    c: Commodity,
    advanced_farming: bool,
) {
    produce_with_share(t, county, realm, c, advanced_farming, 1)
}

/// [`produce`], with the realm-wide weapon share [`resource_limit`] describes.
pub fn produce_with_share(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    c: Commodity,
    advanced_farming: bool,
    weapon_share: i32,
) {
    let index = c.index();
    // The snapshot the panel's "produced this season" line is the difference
    // against, taken before anything else happens.
    let previous_total = county.industry[index].total;

    if county.industry[index].disabled_seasons != 0 {
        county.industry[index].total = 0;
        county.industry[index].output = 0;
        county.industry[index].disabled_seasons -= 1;
        if county.industry[index].disabled_seasons < 1 {
            county.industry[index].enabled = true;
        }
        return;
    }

    let workers = county.labour[t.commodity[c.index()].job].max(0);
    county.industry[index].efficiency = efficiency_ramp(
        county.industry[index].efficiency,
        workers,
        county.industry[index].capacity,
        t.commodity[c.index()].base_efficiency,
        advanced_farming,
    );

    let mut made = output(t, county, c, realm, weapon_share);
    match c {
        Commodity::Wood => realm.wood += made,
        Commodity::Iron => realm.iron += made,
        Commodity::Stone => realm.stone += made,
        Commodity::Weapons => {
            let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
            let (wood, iron) = (t.weapon[weapon].wood, t.weapon[weapon].iron);
            if wood > 0 {
                made = made.min(realm.wood / wood);
            }
            if iron > 0 {
                made = made.min(realm.iron / iron);
            }
            made = made.max(0);
            realm.wood -= made * wood;
            realm.iron -= made * iron;
            realm.weapons[weapon] += made;
        }
    }
    county.industry[index].total = previous_total + made;
    county.industry[index].output = made;
}

/// `Wages_PayAll` (`0x004ACBD4`) sums [`Realm::wage_for_unit`] over a realm's
/// units.
///
/// `men` is campaign unit `+0x168`, the field `docs/battle.md` §4.1 identified
/// as the total over troop types 0..=6. Units live in `g_units`, which is not
/// this crate's, so the caller supplies the per-unit totals in a stable order.
pub fn compute_wages(t: &Tables, realm: &Realm, unit_men: &[i32], difficulty: u8) -> i32 {
    let mut total: i64 = 0;
    for &men in unit_men {
        total += realm.wage_for_unit(t, men, difficulty) as i64;
    }
    total as i32
}

/// What an unpaid season does to a realm's armies — the escalation
/// `docs/kingdom.md` §2 records as `+0x158`, *"0 … 5"*, with no idea what the
/// stages do.
///
/// They are traced now, and each one is confirmed by the `L2.eng` group its
/// handler raises. That is the same cross-check the event table gets: the
/// message the player is shown says, in prose, what the code does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BankruptcyAction {
    /// Nothing: the bill was paid.
    None,
    /// **Stage 0 → 1.** Every mercenary in every army walks off, and any army
    /// left with no men at all is destroyed. Message `0xA0`, `L2.eng` group 160
    /// — *"Mercenaries desert! You have insufficient crowns in your treasury to
    /// pay for the services of all of your troops. The mercenaries promptly
    /// deserted, in search of a more reliable employer."*
    MercenariesDesert,
    /// **Stage 0 → 1 when the realm had no mercenaries to lose.** A warning and
    /// nothing more. Message `0x10E`, group 270 — *"Unpaid troops. … Be warned,
    /// your men will not tolerate this situation for long!"*
    Warned,
    /// **Stages 1, 2 and 3 → 2, 3, 4.** Every army loses men. Message `0x11F`,
    /// group 287 — *"Angry troops. You still have not payed your soldiers. Your
    /// captains report that your armies are losing men."*
    Desertion,
    /// **Stage 4 → 5.** The same desertion, with a final warning. Message
    /// `0x10F`, group 271 — *"Mutinous troops. … it has been over a year since
    /// your men received any wages. They say that the men will revolt next
    /// season."* **"Over a year" is a check on the stage count**: stage 4 is
    /// reached on the fourth unpaid season, which is four seasons, which is a
    /// year.
    LastWarning,
    /// **Stage 5 → 0.** Every army is destroyed and the counter resets.
    /// Messages `0x110` / `0x111`, groups 272 and 273 — *"Mutiny!!!. Furious at
    /// their ill treatment, all your troops have deserted. All your armies are
    /// disbanded. You are wide open to attack."*, and to everybody else
    /// *"FREE. Our spies report great news, my Lord."*
    Mutiny,
}

impl BankruptcyAction {
    /// The original's message id, which is also its `L2.eng` group.
    ///
    /// [`BankruptcyAction::Mutiny`] has two: `0x110` to the realm itself and
    /// `0x111` to everybody else. The first is given here.
    pub fn message_id(self) -> Option<u16> {
        match self {
            BankruptcyAction::None => None,
            BankruptcyAction::MercenariesDesert => Some(0xA0),
            BankruptcyAction::Warned => Some(0x10E),
            BankruptcyAction::Desertion => Some(0x11F),
            BankruptcyAction::LastWarning => Some(0x10F),
            BankruptcyAction::Mutiny => Some(0x110),
        }
    }

    /// The message the *other* realms are shown, where there is one.
    pub fn rival_message_id(self) -> Option<u16> {
        match self {
            BankruptcyAction::Mutiny => Some(0x111),
            _ => None,
        }
    }
}

/// Pay the bill in [`Realm::wages`], advancing the bankruptcy escalation if the
/// treasury cannot cover it.
///
/// ```c
/// realm.wages = sum of Wages_ForUnit over the realm's armies;
/// if (gold < wages) { ...escalate...; }
/// else              { gold -= wages; stage = 0; }
/// ```
///
/// **Two corrections to what this crate did before.**
///
/// 1. **The treasury is not emptied.** A realm that cannot pay pays *nothing*
///    and keeps every crown it has. The previous implementation zeroed the
///    gold, which would have made a realm one crown short lose its whole
///    treasury; the original takes the whole bill or none of it.
/// 2. **The counter is not clamped at 5, it wraps to 0.** Stage 5 is the
///    mutiny, and after it the escalation starts again from the top — so a
///    realm that never pays loses its armies every six seasons rather than
///    settling at a permanent stage 5.
///
/// `had_mercenaries` is `FUN_004AD230`'s return: it dismisses the realm's
/// mercenaries as a side effect and reports whether there were any. Armies are
/// not this crate's, so the caller answers, and the returned
/// [`BankruptcyAction`] tells the caller what to do to them.
pub fn pay(
    realm: &mut Realm,
    id: u8,
    had_mercenaries: bool,
    out: &mut Vec<Message>,
) -> BankruptcyAction {
    if realm.gold >= realm.wages {
        realm.gold -= realm.wages;
        realm.bankrupt_stage = 0;
        return BankruptcyAction::None;
    }
    let action = match realm.bankrupt_stage {
        0 => {
            realm.bankrupt_stage = 1;
            if had_mercenaries {
                BankruptcyAction::MercenariesDesert
            } else {
                BankruptcyAction::Warned
            }
        }
        1..=3 => {
            realm.bankrupt_stage += 1;
            BankruptcyAction::Desertion
        }
        4 => {
            realm.bankrupt_stage = 5;
            BankruptcyAction::LastWarning
        }
        // 5, and anything above it: the mutiny, then back to the beginning.
        _ => {
            realm.bankrupt_stage = 0;
            BankruptcyAction::Mutiny
        }
    };
    out.push(Message::Bankrupt { realm: id, stage: realm.bankrupt_stage, action });
    action
}

// ---------------------------------------------------------------------------
// Castles
// ---------------------------------------------------------------------------

/// The garrison a completed castle can hold.
pub fn garrison_cap(t: &Tables, castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        t.castle.garrison_cap[(castle_type as usize - 1).min(t.castle.garrison_cap.len() - 1)]
    }
}

/// The archers a new castle comes with. The manual: *"A new castle will
/// automatically include a garrison. Its size will vary according to the size
/// of the castle."*
pub fn free_archers(t: &Tables, castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        t.castle.free_archers[(castle_type as usize - 1).min(t.castle.free_archers.len() - 1)]
    }
}

/// `(wood, stone)` to build a castle type, and the workforce it consumes.
pub fn castle_cost(t: &Tables, castle_type: u8) -> (i32, i32) {
    t.castle.cost[(castle_type.max(1) as usize - 1).min(t.castle.cost.len() - 1)]
}

/// The workforce a castle type consumes.
///
/// The table holds two ints per level and both carry the same number. What the
/// second column is for is not established, so this reads the first and the
/// table keeps the pair rather than pretending it is a flat array.
pub fn castle_workforce(t: &Tables, castle_type: u8) -> i32 {
    t.castle.workforce[(castle_type.max(1) as usize - 1).min(t.castle.workforce.len() - 1)].0
}

/// Order a castle: debit the realm's wood and stone and mark the county as
/// building. Returns `false` — changing nothing — if the realm cannot pay.
///
/// **Where the resources are debited is a choice, not a finding.**
/// `docs/kingdom.md` §7.5 gives the five cost tables and §3.4 names
/// `Castle_BuildTick` as a pass, and nothing says whether the cost is taken up
/// front or drawn down each season. Up front is the reading that cannot leave a
/// half-built castle owing resources a realm has since spent.
pub fn order_castle(t: &Tables, county: &mut County, realm: &mut Realm, castle_type: u8) -> bool {
    if castle_type == 0 || castle_type as usize > t.castle.cost.len() {
        return false;
    }
    let (wood, stone) = castle_cost(t, castle_type);
    if realm.wood < wood || realm.stone < stone {
        return false;
    }
    realm.wood -= wood;
    realm.stone -= stone;
    county.castle_building = castle_type;
    county.castle_progress = 0;
    true
}

/// `Castle_BuildTick` — accumulate this season's workforce, and complete the
/// castle when it reaches [`CASTLE_WORKFORCE`].
///
/// **`[I]` throughout.** §3.4 names the pass and §7.5 gives the workforce table;
/// the *rate* — which job slot supplies the workers, and whether the workforce
/// is a per-season requirement or a cumulative one — is not in the document.
/// Cumulative is the reading that makes a 2,500-workforce royal castle a
/// multi-season project rather than an impossible one.
pub fn build_tick(t: &Tables, county: &mut County, id: u8, out: &mut Vec<Message>) -> bool {
    if county.castle_building == 0 {
        return false;
    }
    county.castle_progress += county.labour[t.job.castle_building].max(0);
    if county.castle_progress < castle_workforce(t, county.castle_building) {
        return false;
    }
    county.castle_type = county.castle_building;
    county.castle_building = 0;
    county.castle_progress = 0;
    out.push(Message::CastleBuilt { county: id, castle_type: county.castle_type });
    true
}


#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;
    use crate::tables::{INDUSTRY_ORDER, JOB_BLACKSMITH, JOB_NAMES};

    fn worker_county(job: usize, workers: i32) -> County {
        let mut c = County::new();
        c.labour[job] = workers;
        c
    }

    /// A county with every industry running at its base efficiency, which is
    /// the state the ramp reaches after one Advanced Farming season.
    fn advanced_county(workers: i32) -> County {
        let mut c = County::new();
        for cm in Commodity::ALL {
            c.labour[cm.job()] = workers;
            c.industry[cm.index()].efficiency = cm.base_efficiency();
            c.industry[cm.index()].capacity = workers;
        }
        c
    }

    // --- the job slots -----------------------------------------------------

    /// **`docs/kingdom.md` §7.4's job column is one too high.** Each commodity
    /// draws on the group-74 job its name says, at the record index the driver
    /// and the labour allocator both use.
    #[test]
    fn each_commodity_draws_on_the_job_its_name_says() {
        assert_eq!(JOB_NAMES[Commodity::Wood.job()], "Wood cutting");
        assert_eq!(JOB_NAMES[Commodity::Iron.job()], "Iron mining");
        assert_eq!(JOB_NAMES[Commodity::Stone.job()], "Stone quarrying");
        assert_eq!(JOB_NAMES[Commodity::Weapons.job()], "Blacksmith");
        assert_eq!(JOB_NAMES[crate::tables::JOB_GRAIN_FARMING], "Grain farming");
        assert_eq!(JOB_NAMES.len(), crate::tables::JOB_COUNT);
        assert_eq!(crate::tables::JOB_COUNT, 9, "nine records, not ten");
    }

    /// The four commodities draw on four different jobs, which is what makes
    /// the mapping falsifiable at all.
    #[test]
    fn no_two_commodities_share_a_job() {
        let mut jobs: Vec<usize> = Commodity::ALL.iter().map(|c| c.job()).collect();
        jobs.sort_unstable();
        jobs.dedup();
        assert_eq!(jobs.len(), 4);
        assert_eq!(jobs, vec![4, 5, 6, 7], "iron, stone, wood, blacksmith");
    }

    /// **The driver runs weapons first**, over every county, before any mining
    /// happens. Neither §3.4's list nor §7.4's index order says so, and it is
    /// what makes the blacksmith spend last season's ore.
    #[test]
    fn the_blacksmith_runs_before_the_mines() {
        assert_eq!(INDUSTRY_ORDER[0], Commodity::Weapons);
        assert_eq!(
            INDUSTRY_ORDER,
            [Commodity::Weapons, Commodity::Iron, Commodity::Stone, Commodity::Wood]
        );
        let mut seen: Vec<Commodity> = INDUSTRY_ORDER.to_vec();
        seen.sort_unstable();
        assert_eq!(seen, Commodity::ALL.to_vec(), "all four, exactly once");
    }

    // --- the efficiency ramp -----------------------------------------------

    /// **With Advanced Farming off every industry is a flat 80%**, and the base
    /// efficiencies never come into it. The shipped save has the option off.
    #[test]
    fn a_basic_game_runs_every_industry_at_eighty_percent() {
        for base in [15, 20] {
            for workers in [0, 1, 30, 10_000] {
                assert_eq!(efficiency_ramp(0, workers, 50, base, false), 80);
                assert_eq!(efficiency_ramp(100, workers, 50, base, false), 80);
            }
        }
    }

    /// **The efficiency compounds.** A new iron mine climbs 15 points a season
    /// and reaches its ceiling in seven.
    #[test]
    fn an_iron_mine_ramps_fifteen_points_a_season_up_to_a_hundred() {
        let mut e = 0;
        let mut seen = Vec::new();
        for _ in 0..9 {
            e = efficiency_ramp(e, 30, 1_000, 15, true);
            seen.push(e);
        }
        assert_eq!(seen, vec![15, 30, 45, 60, 75, 90, 100, 100, 100]);
    }

    /// The floor is the base, not zero: an industry can never be worth less
    /// than a fresh one.
    #[test]
    fn the_ramp_never_falls_below_the_base() {
        assert_eq!(efficiency_ramp(-50, 10, 1_000, 20, true), 20);
        assert_eq!(efficiency_ramp(0, 10, 1_000, 20, true), 20);
    }

    /// A workless industry ramps to nothing at all, which is the one case that
    /// returns below the base.
    #[test]
    fn an_industry_with_no_workers_has_no_efficiency() {
        assert_eq!(efficiency_ramp(90, 0, 100, 15, true), 0);
    }

    /// **Overstaffing slows the ramp.** Past the capacity the increment is
    /// scaled by `capacity / workers`, so twice the workers improve at half the
    /// rate — the raw output still rises, the *improvement* does not.
    #[test]
    fn working_more_serfs_than_the_capacity_slows_the_improvement() {
        // 100 workers against a capacity of 100: the full 15 points.
        assert_eq!(efficiency_ramp(0, 100, 100, 15, true), 15);
        // 200 workers against the same capacity: PctOf(100,200) = 50, so
        // Pct(15, 50) = 7.
        assert_eq!(efficiency_ramp(0, 200, 100, 15, true), 15, "but never below the base");
        // Above the base the scaling shows.
        assert_eq!(efficiency_ramp(50, 100, 100, 15, true), 65);
        assert_eq!(efficiency_ramp(50, 200, 100, 15, true), 57, "50 + Pct(15, 50)");
        assert_eq!(efficiency_ramp(50, 400, 100, 15, true), 53, "50 + Pct(15, 25)");
    }

    /// A capacity of zero scales the increment to nothing, so the efficiency
    /// sticks at the base for ever.
    #[test]
    fn an_industry_with_no_capacity_is_pinned_at_its_base() {
        let mut e = 15;
        for _ in 0..20 {
            e = efficiency_ramp(e, 30, 0, 15, true);
        }
        assert_eq!(e, 15);
    }

    // --- the resource limit ------------------------------------------------

    /// For the three raw commodities the limit is a **flag test**: 999 when the
    /// industry is on and has its resource, 0 otherwise.
    #[test]
    fn a_mine_is_limited_only_by_being_switched_on() {
        let realm = Realm::new();
        let mut c = County::new();
        for cm in [Commodity::Wood, Commodity::Iron, Commodity::Stone] {
            assert_eq!(resource_limit(T, &c, cm, &realm, 1), 999);
        }

        c.industry[Commodity::Iron.index()].has_resource = false;
        assert_eq!(resource_limit(T, &c, Commodity::Iron, &realm, 1), 0, "no ore in the ground");

        c.industry[Commodity::Wood.index()].enabled = false;
        assert_eq!(resource_limit(T, &c, Commodity::Wood, &realm, 1), 0, "switched off");

        c.industry[Commodity::Stone.index()].disabled_seasons = 2;
        assert_eq!(resource_limit(T, &c, Commodity::Stone, &realm, 1), 0, "counting down");
    }

    /// 999 is a literal, not a saturating value: a county with enough workers
    /// really is capped there.
    #[test]
    fn nine_hundred_and_ninety_nine_is_a_real_cap() {
        let realm = Realm::new();
        let mut c = advanced_county(0);
        c.labour[Commodity::Wood.job()] = 100_000;
        assert_eq!(output(T, &c, Commodity::Wood, &realm, 1), 999);
    }

    /// The blacksmith's limit is the realm's stock of what the weapon costs,
    /// divided by the realm-wide share.
    #[test]
    fn the_blacksmith_is_limited_by_the_realms_share_of_the_stockpile() {
        let mut c = County::new();
        c.weapon_type = 0; // crossbow: 6 wood, 10 iron
        let mut realm = Realm::new();
        realm.wood = 600;
        realm.iron = 300;
        assert_eq!(resource_limit(T, &c, Commodity::Weapons, &realm, 1), 300, "the iron is scarcer");
        assert_eq!(resource_limit(T, &c, Commodity::Weapons, &realm, 3), 100, "three counties share");

        // A bow costs no iron, so an ironless realm is not limited by it.
        c.weapon_type = 4;
        realm.iron = 0;
        assert_eq!(resource_limit(T, &c, Commodity::Weapons, &realm, 1), 600);
    }

    // --- production --------------------------------------------------------

    /// The FAQ's own example: *"30 serfs working at 15% efficiency"*, which is
    /// the Advanced Farming game.
    #[test]
    fn thirty_serfs_at_fifteen_percent_produce_four() {
        let mut c = worker_county(Commodity::Iron.job(), 30);
        c.industry[Commodity::Iron.index()].efficiency = 15;
        let realm = Realm::new();
        assert_eq!(output(T, &c, Commodity::Iron, &realm, 1), 4, "Pct(30 / 1, 15)");
    }

    /// *"Iron and wood harvest at twice the quantity of stone"* — the divisor
    /// column, stated as a ratio, at a shared efficiency.
    #[test]
    fn iron_and_wood_harvest_at_twice_the_quantity_of_stone() {
        let mut c = advanced_county(200);
        c.industry[Commodity::Wood.index()].efficiency = 15; // level the bases
        let realm = Realm::new();
        let iron = output(T, &c, Commodity::Iron, &realm, 1);
        let stone = output(T, &c, Commodity::Stone, &realm, 1);
        let wood = output(T, &c, Commodity::Wood, &realm, 1);
        assert_eq!(iron, 30);
        assert_eq!(stone, 15);
        assert_eq!(iron, stone * 2);
        assert_eq!(wood, iron);
    }

    #[test]
    fn wood_is_the_only_commodity_with_a_twenty_percent_base() {
        assert_eq!(Commodity::Wood.base_efficiency(), 20);
        for c in Commodity::ALL {
            if c != Commodity::Wood {
                assert_eq!(c.base_efficiency(), 15, "{c:?}");
            }
        }
    }

    #[test]
    fn each_commodity_is_credited_to_its_own_stockpile() {
        let mut c = advanced_county(400);
        let mut r = Realm::new();
        for cm in [Commodity::Wood, Commodity::Iron, Commodity::Stone] {
            produce(T, &mut c, &mut r, cm, true);
        }
        assert_eq!(r.wood, 160, "Pct(400, 20 + 20) after one ramp step");
        assert_eq!(r.iron, 120, "Pct(400, 15 + 15)");
        assert_eq!(r.stone, 60, "Pct(400 / 2, 30)");
        assert_eq!(r.weapons, [0; WEAPON_TYPE_COUNT]);
    }

    /// A basic game's flat 80% dwarfs the advanced game's early seasons — and
    /// that is the shipped save's setting.
    #[test]
    fn a_basic_game_out_produces_a_new_advanced_one() {
        let mut basic = advanced_county(100);
        let mut advanced = advanced_county(100);
        let mut r1 = Realm::new();
        let mut r2 = Realm::new();
        produce(T, &mut basic, &mut r1, Commodity::Iron, false);
        produce(T, &mut advanced, &mut r2, Commodity::Iron, true);
        assert_eq!(r1.iron, 80, "flat 80%");
        assert_eq!(r2.iron, 30, "15% base, ramped once");
    }

    /// A crossbow costs 6 wood and 10 iron, so a blacksmith with the workers
    /// for 15 of them but the iron for 4 makes 4.
    #[test]
    fn the_blacksmith_is_capped_by_the_stock_the_weapon_costs() {
        let mut c = worker_county(JOB_BLACKSMITH, 400);
        c.industry[Commodity::Weapons.index()].efficiency = 15;
        c.weapon_type = 0; // crossbow: 6 wood, 10 iron
        let mut r = Realm::new();
        r.wood = 1000;
        r.iron = 40;
        assert_eq!(output(T, &c, Commodity::Weapons, &r, 1), 15, "the workers allow 15");

        produce(T, &mut c, &mut r, Commodity::Weapons, true);
        assert_eq!(r.weapons[0], 4);
        assert_eq!(r.iron, 0);
        assert_eq!(r.wood, 1000 - 4 * 6);
    }

    /// A bow costs no iron at all, so an ironless realm can still make bows.
    #[test]
    fn bows_need_no_iron() {
        let mut c = worker_county(JOB_BLACKSMITH, 400);
        c.industry[Commodity::Weapons.index()].efficiency = 15;
        c.weapon_type = 4; // bow: 13 wood, 0 iron
        let mut r = Realm::new();
        r.wood = 1000;
        r.iron = 0;
        produce(T, &mut c, &mut r, Commodity::Weapons, true);
        assert_eq!(r.weapons[4], 15);
        assert_eq!(r.wood, 1000 - 15 * 13);
    }

    #[test]
    fn a_realm_with_nothing_in_stock_makes_no_weapons_and_owes_nothing() {
        let mut c = worker_county(JOB_BLACKSMITH, 4000);
        let mut r = Realm::new();
        produce(T, &mut c, &mut r, Commodity::Weapons, true);
        assert_eq!(r.weapons[0], 0);
        assert_eq!(r.wood, 0);
        assert_eq!(r.iron, 0, "and no negative stockpile");
    }

    /// A disabled industry produces nothing, forgets its running total and
    /// counts down; at zero it comes back.
    #[test]
    fn a_disabled_industry_counts_down_and_then_returns() {
        let mut c = advanced_county(400);
        c.industry[Commodity::Iron.index()].disabled_seasons = 2;
        c.industry[Commodity::Iron.index()].total = 500;
        c.industry[Commodity::Iron.index()].enabled = false;
        let mut r = Realm::new();

        produce(T, &mut c, &mut r, Commodity::Iron, true);
        assert_eq!(r.iron, 0);
        assert_eq!(c.industry[Commodity::Iron.index()].total, 0, "the total is forgotten");
        assert_eq!(c.industry[Commodity::Iron.index()].disabled_seasons, 1);
        assert!(!c.industry[Commodity::Iron.index()].enabled);

        produce(T, &mut c, &mut r, Commodity::Iron, true);
        assert_eq!(c.industry[Commodity::Iron.index()].disabled_seasons, 0);
        assert!(c.industry[Commodity::Iron.index()].enabled, "reinstated");

        produce(T, &mut c, &mut r, Commodity::Iron, true);
        assert!(r.iron > 0, "and producing again");
    }

    /// The running total accumulates; the season's output is the step.
    #[test]
    fn the_running_total_accumulates_across_seasons() {
        let mut c = advanced_county(100);
        c.industry[Commodity::Stone.index()].efficiency = 15;
        let mut r = Realm::new();
        let mut totals = Vec::new();
        for _ in 0..3 {
            produce(T, &mut c, &mut r, Commodity::Stone, true);
            totals.push(c.industry[Commodity::Stone.index()].total);
        }
        assert_eq!(totals, vec![15, 37, 67]);
        assert_eq!(r.stone, 67, "the realm has what the county totalled");
    }

    // --- wages -------------------------------------------------------------

    #[test]
    fn the_wage_bill_is_the_sum_over_units_and_ignores_troop_type() {
        let mut r = Realm::new();
        r.is_human = true;
        assert_eq!(compute_wages(T, &r, &[250, 252, 254], 0), 62 + 63 + 63);
        assert_eq!(compute_wages(T, &r, &[], 0), 0);
    }

    #[test]
    fn a_realm_that_can_pay_clears_its_bankruptcy_counter() {
        let mut r = Realm::new();
        r.gold = 1000;
        r.wages = 400;
        r.bankrupt_stage = 3;
        let mut out = Vec::new();
        assert_eq!(pay(&mut r, 1, false, &mut out), BankruptcyAction::None);
        assert_eq!(r.gold, 600);
        assert_eq!(r.bankrupt_stage, 0);
        assert!(out.is_empty());
    }

    /// **The five stages, in order, each with the `L2.eng` group that says in
    /// prose what its handler does.**
    #[test]
    fn the_escalation_runs_warning_desertion_desertion_desertion_mutiny() {
        let mut r = Realm::new();
        r.gold = 10;
        r.wages = 400;
        let mut out = Vec::new();
        let mut actions = Vec::new();
        for _ in 0..6 {
            actions.push(pay(&mut r, 2, false, &mut out));
        }
        assert_eq!(
            actions,
            vec![
                BankruptcyAction::Warned,      // stage 1 - group 270
                BankruptcyAction::Desertion,   // stage 2 - group 287
                BankruptcyAction::Desertion,   // stage 3
                BankruptcyAction::Desertion,   // stage 4
                BankruptcyAction::LastWarning, // stage 5 - group 271
                BankruptcyAction::Mutiny,      // and back to 0 - group 272
            ]
        );
        // Group 271 says "over a year since your men received any wages", and
        // it fires on the fifth unpaid season.
        assert_eq!(actions[4], BankruptcyAction::LastWarning);
        assert_eq!(r.bankrupt_stage, 0, "the counter wraps rather than sticking");
        assert_eq!(out.len(), 6);
    }

    /// **A realm that cannot pay keeps its gold.** It pays the whole bill or
    /// none of it; the previous implementation emptied the treasury.
    #[test]
    fn an_unpayable_bill_costs_the_treasury_nothing_at_all() {
        let mut r = Realm::new();
        r.gold = 399;
        r.wages = 400;
        let mut out = Vec::new();
        pay(&mut r, 1, false, &mut out);
        assert_eq!(r.gold, 399, "one crown short, and it keeps all 399");
    }

    /// The first unpaid season depends on whether there were mercenaries to
    /// lose — the same stage, two different messages.
    #[test]
    fn the_first_unpaid_season_dismisses_mercenaries_if_there_are_any() {
        let mut with = Realm::new();
        with.wages = 1;
        let mut out = Vec::new();
        assert_eq!(pay(&mut with, 1, true, &mut out), BankruptcyAction::MercenariesDesert);
        assert_eq!(with.bankrupt_stage, 1);

        let mut without = Realm::new();
        without.wages = 1;
        assert_eq!(pay(&mut without, 1, false, &mut out), BankruptcyAction::Warned);
        assert_eq!(without.bankrupt_stage, 1);
    }

    /// Every stage carries the message id it raises, and the mutiny carries a
    /// second one for everybody else.
    #[test]
    fn every_stage_names_the_l2_eng_group_it_raises() {
        assert_eq!(BankruptcyAction::None.message_id(), None);
        assert_eq!(BankruptcyAction::MercenariesDesert.message_id(), Some(0xA0));
        assert_eq!(BankruptcyAction::Warned.message_id(), Some(0x10E));
        assert_eq!(BankruptcyAction::Desertion.message_id(), Some(0x11F));
        assert_eq!(BankruptcyAction::LastWarning.message_id(), Some(0x10F));
        assert_eq!(BankruptcyAction::Mutiny.message_id(), Some(0x110));
        assert_eq!(BankruptcyAction::Mutiny.rival_message_id(), Some(0x111));
        assert_eq!(BankruptcyAction::Desertion.rival_message_id(), None);
    }

    /// Paying at any point resets the escalation, so a realm that scrapes
    /// together one season's wages starts again from the top.
    #[test]
    fn paying_once_clears_the_whole_escalation() {
        let mut r = Realm::new();
        r.wages = 100;
        let mut out = Vec::new();
        pay(&mut r, 1, false, &mut out);
        pay(&mut r, 1, false, &mut out);
        assert_eq!(r.bankrupt_stage, 2);
        r.gold = 100;
        assert_eq!(pay(&mut r, 1, false, &mut out), BankruptcyAction::None);
        assert_eq!(r.bankrupt_stage, 0);
    }

    // --- castles -----------------------------------------------------------

    #[test]
    fn a_castle_cannot_be_ordered_without_the_wood_and_the_stone() {
        let mut c = County::new();
        let mut r = Realm::new();
        r.wood = 199;
        r.stone = 1000;
        assert!(!order_castle(T, &mut c, &mut r, 3), "a keep needs 200 wood");
        assert_eq!(c.castle_building, 0);
        assert_eq!(r.wood, 199, "and nothing is spent on a refused order");

        r.wood = 200;
        assert!(order_castle(T, &mut c, &mut r, 3));
        assert_eq!(c.castle_building, 3);
        assert_eq!((r.wood, r.stone), (0, 0));
    }

    #[test]
    fn a_royal_castle_takes_several_seasons_of_workers() {
        let mut c = County::new();
        let mut r = Realm::new();
        r.wood = 10_000;
        r.stone = 10_000;
        assert!(order_castle(T, &mut c, &mut r, 5));
        c.labour[T.job.castle_building] = 500;

        let mut out = Vec::new();
        for season in 1..5 {
            assert!(!build_tick(T, &mut c, 1, &mut out), "season {season} is too early");
        }
        assert!(build_tick(T, &mut c, 1, &mut out), "2500 workforce, 500 a season");
        assert_eq!(c.castle_type, 5);
        assert_eq!(c.castle_building, 0);
        assert_eq!(out, vec![Message::CastleBuilt { county: 1, castle_type: 5 }]);
    }

    #[test]
    fn a_county_not_building_anything_does_nothing() {
        let mut c = County::new();
        let mut out = Vec::new();
        for _ in 0..10 {
            assert!(!build_tick(T, &mut c, 1, &mut out));
        }
        assert!(out.is_empty());
        assert_eq!(c.castle_progress, 0);
    }

    #[test]
    fn a_bigger_castle_holds_more_men_and_comes_with_more_archers() {
        assert_eq!(garrison_cap(T, 0), 0);
        assert_eq!(free_archers(T, 0), 0);
        for t in 1..5u8 {
            assert!(garrison_cap(T, t + 1) >= garrison_cap(T, t), "castle {t}");
            assert!(free_archers(T, t + 1) >= free_archers(T, t), "castle {t}");
        }
        assert_eq!(garrison_cap(T, 5), 600);
        assert_eq!(free_archers(T, 5), 300);
    }

    /// The default starting castle is the Norman keep, and it is the cheapest
    /// castle in wood — which is why it is the one every player-owned county in
    /// the shipped save has.
    #[test]
    fn the_norman_keep_is_the_cheapest_castle_in_wood() {
        assert_eq!(crate::tables::CASTLE_STARTING_TYPE, 3);
        let (wood, _) = castle_cost(T, 3);
        for t in 1..=5u8 {
            assert!(castle_cost(T, t).0 >= wood, "castle {t}");
        }
    }

    /// `PctOf` is the original's `FUN_00404DC1`, including its zero case.
    #[test]
    fn pct_of_returns_zero_rather_than_dividing_by_zero() {
        assert_eq!(pct_of(50, 500), 10);
        assert_eq!(pct_of(1, 3), 33, "truncating");
        assert_eq!(pct_of(50, 0), 0);
        assert_eq!(pct_of(0, 100), 0);
        assert_eq!(pct_of(50, 49), 102, "and it can exceed 100");
    }
}
