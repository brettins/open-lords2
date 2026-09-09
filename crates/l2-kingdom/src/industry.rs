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
//!   which means the England turn-one fixture's settings put every industry at 80% and the
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
use crate::tables::{Commodity, Tables, RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT};

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
    t: &Tables,
    last_efficiency: i32,
    workers: i32,
    capacity: i32,
    base: i32,
    advanced_farming: bool,
) -> i32 {
    if !advanced_farming {
        return t.efficiency.without_advanced_farming;
    }
    if workers == 0 {
        return 0;
    }
    let mut increment = base;
    if capacity < workers {
        increment = pct(base, pct_of(capacity, workers));
    }
    let mut efficiency = last_efficiency + increment;
    if efficiency > t.efficiency.max {
        efficiency = t.efficiency.max;
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
/// first county emptying it. Those two denominators are [`WeaponShare`], and
/// they are no longer `[D]`.
pub fn resource_limit(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
) -> i32 {
    let record = &county.industry[c.index()];
    if !record.enabled {
        return 0;
    }
    if c == Commodity::Weapons {
        let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
        let (wood, iron) = (t.weapon[weapon].wood, t.weapon[weapon].iron);
        // Written the way the original writes it — `(stock * cost / share) /
        // cost`, multiplying by the cost and dividing by it again. That is not
        // a no-op once `share` exceeds 1: it rounds the share down to a whole
        // weapon's worth of stock. Kept rather than cancelled.
        let quota = |stock: i32, cost: i32, share: i32| {
            ((stock as i64 * cost as i64 / share.max(1) as i64) / cost as i64) as i32
        };
        let mut limit = RESOURCE_LIMIT_UNLIMITED;
        if wood != 0 {
            limit = limit.min(quota(realm.wood, wood, weapon_share.wood));
        }
        if iron != 0 {
            limit = limit.min(quota(realm.iron, iron, weapon_share.iron));
        }
        return limit.max(0);
    }
    if !record.has_resource || record.disabled_seasons != 0 {
        return 0;
    }
    RESOURCE_LIMIT_UNLIMITED
}

/// The two denominators `FUN_0044F15B` (`0x0044F15B`) computes before every
/// weapons `resourceLimit`, and the answer to a `[D]` this module carried.
///
/// ```c
/// woodShare = ironShare = 0;
/// for (c = 1; c <= countyCount; c++)
///     if (counties[c].owner == realm && counties[c].industry[2].enabled
///         && counties[c].labour[7].workers > 0) {
///         woodShare += weaponCost[counties[c].weaponType].wood;
///         ironShare += weaponCost[counties[c].weaponType].iron;
///     }
/// if (woodShare < 1) woodShare = 1;
/// if (ironShare < 1) ironShare = 1;
/// ```
///
/// **They are summed *costs*, not a county count** — which is why
/// [`WeaponShare::SINGLE_SMITH`] is not the right default and this crate's old
/// `weapon_share: 1` was not either. A lone smithy forging crossbows at 6 wood
/// and 10 iron divides by 6 and by 10, so its limit is `realm.wood / 6` and
/// `realm.iron / 10` — the number of crossbows the stockpile can actually pay
/// for. The old default gave it the whole stockpile and relied on
/// [`produce_with_share`]'s affordability clamp to bring it back down to the
/// same number.
///
/// Two counties forging *different* weapons therefore split the stockpile in
/// proportion to what each weapon costs, not evenly. **`[V]`**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponShare {
    pub wood: i32,
    pub iron: i32,
}

impl WeaponShare {
    /// `1` and `1` — the value that makes [`resource_limit`] hand a smithy the
    /// **whole** stockpile. It is what this crate used to pass everywhere; it
    /// is the identity, not the single-county case, and it is kept only for
    /// tests that want the unbounded limit.
    pub const UNSHARED: WeaponShare = WeaponShare { wood: 1, iron: 1 };

    /// The share of a realm whose only smithy is this county's — its own
    /// weapon's costs, which is what the sum comes to.
    pub fn single_smith(t: &Tables, weapon_type: usize) -> WeaponShare {
        let weapon = weapon_type.min(WEAPON_TYPE_COUNT - 1);
        WeaponShare {
            wood: t.weapon[weapon].wood.max(1),
            iron: t.weapon[weapon].iron.max(1),
        }
    }

    /// Alias for [`WeaponShare::UNSHARED`], named for what it is not.
    pub const SINGLE_SMITH: WeaponShare = WeaponShare::UNSHARED;
}

/// `FUN_0044F15B` — [`WeaponShare`] for one realm, summed over the counties
/// whose blacksmith is switched on **and staffed**. An idle smithy takes no
/// share, so switching one off gives the rest of the realm more iron.
pub fn weapon_shares(
    t: &Tables,
    counties: &[County],
    county_count: usize,
    realm: u8,
) -> WeaponShare {
    let mut share = WeaponShare { wood: 0, iron: 0 };
    for c in counties.iter().take(county_count + 1).skip(1) {
        if c.owner != realm
            || !c.industry[Commodity::Weapons.index()].enabled
            || c.labour[t.commodity[Commodity::Weapons.index()].job] <= 0
        {
            continue;
        }
        let weapon = c.weapon_type.min(WEAPON_TYPE_COUNT - 1);
        share.wood += t.weapon[weapon].wood;
        share.iron += t.weapon[weapon].iron;
    }
    WeaponShare { wood: share.wood.max(1), iron: share.iron.max(1) }
}

/// What one commodity's pass would produce, before it is credited anywhere.
pub fn output(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
) -> i32 {
    let record = &county.industry[c.index()];
    let workers = county.labour[t.commodity[c.index()].job].max(0);
    let raw = pct(workers / t.commodity[c.index()].divisor, record.efficiency);
    raw.min(resource_limit(t, county, c, realm, weapon_share)).max(0)
}

/// `Industry_LabourEstimate` (`0x0044F318`) — **one industry's labour
/// ceiling.**
///
/// Returns `(wanted, useful)`, and the wanted floor is always `-1`: no industry
/// has one.
///
/// ```c
/// wanted[slot] = -1; useful[slot] = 0;
/// if (owner == 0) return;                      /* a neutral county mines nothing */
/// limit = resourceLimit(county, industry, owner, 0);
/// if (limit <= 0 || popBand == 0) return;
/// if (industry != weapons) { useful[slot] = 100000; return; }
/// best = -1;
/// for (w = 0; w < population + popBand; w += popBand) {     /* one icon at a time */
///     n = min(w, population);
///     made = min(Pct(n / divisor, efficiencyRamp(county, industry, n, base)), limit);
///     if (best < made) { best = made; useful[slot] = n; }
/// }
/// ```
///
/// **Three things this makes concrete.** The owner test is why an unowned
/// county's wood ceiling is 0 and an owned one's is 100,000, which is the whole
/// difference between the shipped save's county of foresters and its county of
/// idlers. The search steps by **`popBand`**, one peasant icon, not by one
/// person — so the blacksmith's ceiling is always a multiple of the icon size.
/// And the blacksmith is the only industry with a real ceiling at all: wood,
/// iron and stone are bounded by [`RESOURCE_LIMIT_UNLIMITED`]'s 999 units of
/// output rather than by any worker count.
///
/// `weapon_share` is [`weapon_shares`] for the owning realm; the original calls
/// `FUN_0044F15B` afresh inside every `resourceLimit`, so it is the *current*
/// state of the realm's smithies each time.
pub fn labour_estimate(
    t: &Tables,
    county: &County,
    c: Commodity,
    realm: &Realm,
    weapon_share: WeaponShare,
    advanced_farming: bool,
) -> (i32, i32) {
    const NONE: (i32, i32) = (crate::county::LABOUR_NO_FLOOR, 0);
    if county.owner == 0 || county.pop_band == 0 {
        return NONE;
    }
    let limit = resource_limit(t, county, c, realm, weapon_share);
    if limit <= 0 {
        return NONE;
    }
    if c != Commodity::Weapons {
        return (crate::county::LABOUR_NO_FLOOR, crate::county::LABOUR_UNBOUNDED);
    }

    let row = t.commodity[c.index()];
    let record = &county.industry[c.index()];
    let band = county.pop_band.max(1);
    let mut best = -1;
    let mut ceiling = 0;
    let mut trial = 0;
    while trial < county.population + band {
        let workers = trial.min(county.population);
        let efficiency = efficiency_ramp(
            t,
            record.efficiency,
            workers,
            record.capacity,
            row.base_efficiency,
            advanced_farming,
        );
        let made = pct(workers / row.divisor, efficiency).min(limit);
        if best < made {
            best = made;
            ceiling = workers;
        }
        trial += band;
    }
    (crate::county::LABOUR_NO_FLOOR, ceiling)
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
/// iron. The original debits `made * cost` from each stockpile with **no clamp
/// at all** — it relies on [`resource_limit`]'s realm-wide share to keep the
/// total demand inside the stock, and now that [`weapon_shares`] is the real
/// denominator that reliance holds: `(stock * cost / Σcost) / cost` is at most
/// `stock / Σcost`, so the whole realm's smithies together can never ask for
/// more wood or iron than there is. The clamp below is therefore **provably
/// redundant** against a correct share, and it is kept only as a floor against
/// a caller that passes [`WeaponShare::UNSHARED`].
pub fn produce(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    c: Commodity,
    advanced_farming: bool,
) {
    let share = WeaponShare::single_smith(t, county.weapon_type);
    produce_with_share(t, county, realm, c, advanced_farming, share)
}

/// [`produce`], with the realm-wide weapon share [`resource_limit`] describes.
pub fn produce_with_share(
    t: &Tables,
    county: &mut County,
    realm: &mut Realm,
    c: Commodity,
    advanced_farming: bool,
    weapon_share: WeaponShare,
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
        t,
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
    // `+0x1C3` — **a castle is under construction.** Four independent readers
    // all mean that: `Labour_Allocate` will not staff castle building without
    // it, `Castle_BuildEstimate` computes nothing without it,
    // `Industry_LabourEstimate` shows the outstanding wood and stone only with
    // it, and `Tax_CollectAll` charges the *lower* of the standing and the
    // building castle while it is set. Nothing in this crate used to write it,
    // so the castle-building job had a ceiling of zero for ever and no county
    // could build anything.
    //
    // **It is a byte with three values and not a flag**, which the siege work
    // settled: 1 is *a castle is being built or upgraded* and 2 is *a castle is
    // being repaired after a siege*. `Siege_LaunchAssault` fights a different
    // castle for each, and the castle-build season pass sends a different
    // message. Every reader above tests it against zero, so widening it changes
    // none of them. See [`crate::siege::CASTLE_DEGRADED_BUILDING`].
    county.castle_degraded = crate::siege::CASTLE_DEGRADED_BUILDING;
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
    county.castle_degraded = 0;
    out.push(Message::CastleBuilt { county: id, castle_type: county.castle_type });
    true
}

/// `Castle_BuildEstimate` (`0x00450E46`) — the **castle** labour ceiling.
///
/// ```c
/// wanted[3] = -1; useful[3] = 0;
/// if (!castleUnderConstruction) return;
/// delivered = min(100 - Pct(woodDelivered, woodNeeded),
///                 100 - Pct(stoneDelivered, stoneNeeded));
/// useful[3] = (delivered < 100) ? 0 : workRemaining;
/// seasonsLeft = (delivered < 100 || workers < 1) ? 100
///             : DivCeil(workRemaining, workers);
/// ```
///
/// **The materials clause cannot be reproduced and does not need to be.** The
/// original tracks a delivery against a requirement in six words this crate
/// does not have (`+0x1CC` … `+0x1E0`); [`order_castle`] takes the whole cost
/// out of the realm the moment the castle is ordered, which is the reading
/// `docs/kingdom.md` §7.5 records and which makes the delivery permanently
/// complete. So the gate is open whenever a build is under way, and the
/// ceiling is the work outstanding. **`[I]`**, and it is the *model* that is
/// inferred, not the arithmetic: given up-front delivery this is what
/// `Castle_BuildEstimate` computes.
///
/// The ceiling is a *cumulative* figure — the whole remaining workforce, not a
/// per-season share — so a county that can staff it finishes the castle in one
/// season and one that cannot puts everybody it has on the walls.
pub fn castle_labour_estimate(t: &Tables, county: &County) -> (i32, i32) {
    if county.castle_degraded == 0 || county.castle_building == 0 {
        return (crate::county::LABOUR_NO_FLOOR, 0);
    }
    let remaining =
        (castle_workforce(t, county.castle_building) - county.castle_progress).max(0);
    (crate::county::LABOUR_NO_FLOOR, remaining)
}

// ---------------------------------------------------------------------------
// Switching an industry on and off
// ---------------------------------------------------------------------------

/// What a click on a building on the campaign map toggles.
///
/// `Map_Click` picks this from a ladder on the clicked tile's graphic index,
/// and the whole ladder is: **0…3 iron, 4…6 stone, 7…9 weapons, 10…12 wood,
/// 13…20 nothing at all, 21 and above the castle.** `[D]` — `Map_Click`'s own
/// `if`/`else if` chain, and it is the only way to reach
/// [`toggle_from_map`]: **nothing on any county panel switches an industry**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapToggle {
    /// One of the four commodities: its enable byte, county `+0x297 + c*0x18`.
    Industry(Commodity),
    /// County `+0x1B0` — the switch a player throws by dragging builders onto
    /// the castle, and the second gate `Labour_Allocate` puts on castle
    /// building. Until now this crate had no such field and treated it as
    /// permanently thrown ([`crate::labour::ceilings`]).
    Castle,
}

/// The ladder itself, so a screen can ask "is this building anything?".
pub fn map_toggle_for_graphic(graphic: u8) -> Option<MapToggle> {
    match graphic {
        0..=3 => Some(MapToggle::Industry(Commodity::Iron)),
        4..=6 => Some(MapToggle::Industry(Commodity::Stone)),
        7..=9 => Some(MapToggle::Industry(Commodity::Weapons)),
        10..=12 => Some(MapToggle::Industry(Commodity::Wood)),
        13..=20 => None,
        _ => Some(MapToggle::Castle),
    }
}

/// `Industry_ToggleFromMap` (`0x0043D309`) — switch one industry, or castle
/// building, on or off.
///
/// ```c
/// enabled ^= 1;                                   /* +0x297 + c*0x18, or +0x1B0 */
/// County_RefreshEstimates(county, seasonNext);
/// Labour_ToggleIndustryShare(county, jobFor(industry), enabled);
/// County_RefreshEstimates(county, seasonNext);
/// Labour_Allocate(county); Ration_Apply(county, season); Labour_Allocate(county);
/// County_RefreshEstimates(county, seasonNext);
/// ```
///
/// **`[D]`.** The enable byte is what [`crate::labour::ceilings`] already gates
/// each mining job on, so switching one off empties that job on the next
/// allocation — which is the point of the button. The castle arm reads its
/// switch *before* flipping it, so the share is toggled to the state the switch
/// was **leaving**, not the one it lands in; that is the original's order and
/// it is kept.
///
/// The caller supplies the allocation and the estimates it can run, as
/// [`crate::field::set_type`] does and for the same reasons —
/// [`crate::Kingdom::toggle_industry`] is the whole thing assembled.
pub fn toggle_from_map(county: &mut County, what: MapToggle) -> bool {
    match what {
        MapToggle::Industry(c) => {
            let slot = c.index();
            county.industry[slot].enabled = !county.industry[slot].enabled;
            crate::labour::toggle_industry_share(
                county,
                c.job(),
                county.industry[slot].enabled,
            );
            county.industry[slot].enabled
        }
        MapToggle::Castle => {
            // `local_c = (castleSwitch != 0)` is taken **before** the flip, and
            // that stale value is what reaches the share toggle. Reproduced.
            let was = county.castle_switch;
            crate::labour::toggle_industry_share(county, crate::tables::JOB_CASTLE_BUILDING, was);
            county.castle_switch = !was;
            county.castle_switch
        }
    }
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
    /// efficiencies never come into it. The England turn-one fixture has the option off.
    #[test]
    fn a_basic_game_runs_every_industry_at_eighty_percent() {
        for base in [15, 20] {
            for workers in [0, 1, 30, 10_000] {
                assert_eq!(efficiency_ramp(T, 0, workers, 50, base, false), 80);
                assert_eq!(efficiency_ramp(T, 100, workers, 50, base, false), 80);
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
            e = efficiency_ramp(T, e, 30, 1_000, 15, true);
            seen.push(e);
        }
        assert_eq!(seen, vec![15, 30, 45, 60, 75, 90, 100, 100, 100]);
    }

    /// The floor is the base, not zero: an industry can never be worth less
    /// than a fresh one.
    #[test]
    fn the_ramp_never_falls_below_the_base() {
        assert_eq!(efficiency_ramp(T, -50, 10, 1_000, 20, true), 20);
        assert_eq!(efficiency_ramp(T, 0, 10, 1_000, 20, true), 20);
    }

    /// A workless industry ramps to nothing at all, which is the one case that
    /// returns below the base.
    #[test]
    fn an_industry_with_no_workers_has_no_efficiency() {
        assert_eq!(efficiency_ramp(T, 90, 0, 100, 15, true), 0);
    }

    /// **Overstaffing slows the ramp.** Past the capacity the increment is
    /// scaled by `capacity / workers`, so twice the workers improve at half the
    /// rate — the raw output still rises, the *improvement* does not.
    #[test]
    fn working_more_serfs_than_the_capacity_slows_the_improvement() {
        // 100 workers against a capacity of 100: the full 15 points.
        assert_eq!(efficiency_ramp(T, 0, 100, 100, 15, true), 15);
        // 200 workers against the same capacity: PctOf(100,200) = 50, so
        // Pct(15, 50) = 7.
        assert_eq!(efficiency_ramp(T, 0, 200, 100, 15, true), 15, "but never below the base");
        // Above the base the scaling shows.
        assert_eq!(efficiency_ramp(T, 50, 100, 100, 15, true), 65);
        assert_eq!(efficiency_ramp(T, 50, 200, 100, 15, true), 57, "50 + Pct(15, 50)");
        assert_eq!(efficiency_ramp(T, 50, 400, 100, 15, true), 53, "50 + Pct(15, 25)");
    }

    /// A capacity of zero scales the increment to nothing, so the efficiency
    /// sticks at the base for ever.
    #[test]
    fn an_industry_with_no_capacity_is_pinned_at_its_base() {
        let mut e = 15;
        for _ in 0..20 {
            e = efficiency_ramp(T, e, 30, 0, 15, true);
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
            assert_eq!(resource_limit(T, &c, cm, &realm, WeaponShare::UNSHARED), 999);
        }

        c.industry[Commodity::Iron.index()].has_resource = false;
        assert_eq!(resource_limit(T, &c, Commodity::Iron, &realm, WeaponShare::UNSHARED), 0, "no ore in the ground");

        c.industry[Commodity::Wood.index()].enabled = false;
        assert_eq!(resource_limit(T, &c, Commodity::Wood, &realm, WeaponShare::UNSHARED), 0, "switched off");

        c.industry[Commodity::Stone.index()].disabled_seasons = 2;
        assert_eq!(resource_limit(T, &c, Commodity::Stone, &realm, WeaponShare::UNSHARED), 0, "counting down");
    }

    /// 999 is a literal, not a saturating value: a county with enough workers
    /// really is capped there.
    #[test]
    fn nine_hundred_and_ninety_nine_is_a_real_cap() {
        let realm = Realm::new();
        let mut c = advanced_county(0);
        c.labour[Commodity::Wood.job()] = 100_000;
        assert_eq!(output(T, &c, Commodity::Wood, &realm, WeaponShare::UNSHARED), 999);
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
        let unshared = WeaponShare::UNSHARED;
        assert_eq!(
            resource_limit(T, &c, Commodity::Weapons, &realm, unshared),
            300,
            "the iron is scarcer"
        );
        assert_eq!(
            resource_limit(T, &c, Commodity::Weapons, &realm, WeaponShare { wood: 18, iron: 30 }),
            10,
            "three smithies forging crossbows split the 300 iron three ways, at 10 a crossbow"
        );

        // A bow costs no iron, so an ironless realm is not limited by it.
        c.weapon_type = 4;
        realm.iron = 0;
        assert_eq!(resource_limit(T, &c, Commodity::Weapons, &realm, unshared), 600);
    }

    /// **The real denominator is a sum of costs, not a count of counties.**
    /// `FUN_0044F15B`, and the answer to the `[D]` this module carried.
    #[test]
    fn the_weapon_share_sums_the_costs_of_every_staffed_smithy() {
        let mut counties = vec![County::new(); 4];
        for (id, weapon) in [(1usize, 0usize), (2, 1), (3, 0)] {
            counties[id].owner = 1;
            counties[id].weapon_type = weapon;
            counties[id].labour[T.commodity[Commodity::Weapons.index()].job] = 40;
        }
        // Crossbow (6, 10), mace (4, 4), crossbow (6, 10).
        assert_eq!(weapon_shares(T, &counties, 3, 1), WeaponShare { wood: 16, iron: 24 });

        // An unstaffed smithy takes no share at all.
        counties[2].labour[T.commodity[Commodity::Weapons.index()].job] = 0;
        assert_eq!(weapon_shares(T, &counties, 3, 1), WeaponShare { wood: 12, iron: 20 });

        // Neither does a switched-off one, nor another realm's.
        counties[3].industry[Commodity::Weapons.index()].enabled = false;
        counties[1].owner = 2;
        assert_eq!(weapon_shares(T, &counties, 3, 1), WeaponShare { wood: 1, iron: 1 });
    }

    /// A realm with one smithy divides by that weapon's own costs, so its
    /// limit is exactly what the stockpile can pay for — the number the
    /// affordability clamp in [`produce`] used to be doing on its own.
    #[test]
    fn a_lone_smithy_is_limited_to_what_the_stockpile_can_buy() {
        let mut c = County::new();
        c.owner = 1;
        c.weapon_type = 0; // 6 wood, 10 iron
        c.labour[T.commodity[Commodity::Weapons.index()].job] = 10;
        let mut realm = Realm::new();
        realm.wood = 600;
        realm.iron = 300;
        let counties = vec![County::new(), c.clone()];
        let share = weapon_shares(T, &counties, 1, 1);
        assert_eq!(share, WeaponShare { wood: 6, iron: 10 });
        assert_eq!(
            resource_limit(T, &c, Commodity::Weapons, &realm, share),
            30,
            "300 iron at 10 a crossbow"
        );
    }

    // --- production --------------------------------------------------------

    /// The FAQ's own example: *"30 serfs working at 15% efficiency"*, which is
    /// the Advanced Farming game.
    #[test]
    fn thirty_serfs_at_fifteen_percent_produce_four() {
        let mut c = worker_county(Commodity::Iron.job(), 30);
        c.industry[Commodity::Iron.index()].efficiency = 15;
        let realm = Realm::new();
        assert_eq!(output(T, &c, Commodity::Iron, &realm, WeaponShare::UNSHARED), 4, "Pct(30 / 1, 15)");
    }

    /// *"Iron and wood harvest at twice the quantity of stone"* — the divisor
    /// column, stated as a ratio, at a shared efficiency.
    #[test]
    fn iron_and_wood_harvest_at_twice_the_quantity_of_stone() {
        let mut c = advanced_county(200);
        c.industry[Commodity::Wood.index()].efficiency = 15; // level the bases
        let realm = Realm::new();
        let iron = output(T, &c, Commodity::Iron, &realm, WeaponShare::UNSHARED);
        let stone = output(T, &c, Commodity::Stone, &realm, WeaponShare::UNSHARED);
        let wood = output(T, &c, Commodity::Wood, &realm, WeaponShare::UNSHARED);
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
    /// that is the England turn-one fixture's setting.
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
        assert_eq!(
            output(T, &c, Commodity::Weapons, &r, WeaponShare::UNSHARED),
            15,
            "the workers allow 15"
        );
        // …and against the realm's real share it is already 4, because the
        // share is the weapon's own cost: 40 iron at 10 a crossbow.
        assert_eq!(output(T, &c, Commodity::Weapons, &r, WeaponShare::single_smith(T, 0)), 4);

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
    /// the England turn-one fixture has.
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
