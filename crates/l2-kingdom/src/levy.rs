//! Raising an army — the levy, the armoury basket, and `Army_Create`.
//! `docs/armies.md` §6.
//!
//! **Recruitment is a headcount.** You take a percentage of a county's people
//! and the county's happiness pays for it; there is no per-troop-type
//! recruitment price anywhere on this path. What the men are *carrying* is
//! decided separately, out of the realm's weapon stockpiles, and a man with no
//! weapon is a peasant.
//!
//! # The three numbers, and which one is charged
//!
//! Worth saying once, because the original says it three different ways: the
//! raise-army screen prints `men / 2` as the seasonal wage, `g_mercWage` holds
//! `price / 10` and is never read by anything, and
//! [`crate::realm::Realm::wage_for_unit`] charges `men / 4`. **Only the last is
//! spent.**
//!
//! # No oracle exists for any of this
//!
//! The shipped `lastturn.sav` holds six units and **all six are merchants**.
//! There is not one army in it, so every army-only field — the troop counts,
//! the wage, the morale, the mercenary triple, the move allowance — has no
//! data-side confirmation available at all. Everything here is `[D]` from the
//! instruction stream or `[V]` where a *static table* in the executable or an
//! `L2.eng` string pins it, and nothing in this module is `[V]` on the strength
//! of a save.

use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use crate::unit::{ArmyNames, TroopType, Unit, UnitKind, Units, TROOP_TYPES};

/// `g_levyBasket` (`0x0053F6A0`) has **eight slots of `0x10` bytes** per realm:
/// slot 0 is the unequipped men, slots 1…6 are the six weapon types, and slot 7
/// is the levy total.
pub const BASKET_SLOTS: usize = TROOP_TYPES + 1;

/// The slot holding the total, which the `+`/`−` buttons never touch and which
/// is what `Army_Create` writes into the army's `men`.
pub const BASKET_TOTAL: usize = TROOP_TYPES;

/// One slot of the armoury basket.
///
/// Three fields, and the third is not redundant: `available` is the number the
/// screen prints, `remaining` is what the `+` button compares against. Both are
/// seeded from the same stockpile and they only diverge on the AI's auto-equip
/// path, which decrements `remaining` and leaves `available` alone.
///
/// The original has a fourth field at `+0x0C` that neither seeding function
/// clears — a "chosen count when this slot was last selected" latch that fires
/// the troop-portrait animation. It is presentation and it is not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BasketSlot {
    /// `+0x00` — how many men are in this slot.
    pub chosen: i32,
    /// `+0x04` — the stockpile, as drawn.
    pub available: i32,
    /// `+0x08` — the stockpile, as spent.
    pub remaining: i32,
}

/// The armoury scratch buffer for one realm: who is being equipped with what.
///
/// > **`docs/armies.md` §6.2 attributes this to `Levy_Init` (`0x004AAA80`).
/// > That is the AI's function.** Its only callers are the three AI
/// > army-raising helpers. The **player's** raise-army screen goes through
/// > `FUN_004AA90A`, which is the same seeding plus three UI resets. The
/// > distinction matters for modding — a rule attached to the wrong one of the
/// > two would apply to only half the armies in the game — and the seeding
/// > itself is identical, so [`LevyBasket::seed`] is both. Corrected in the
/// > document. `[V]`
///
/// The realm index is **the county's owner**, not a realm handed in: both
/// seeding functions read `county[+0x05]`. They agree in play and would not for
/// an ownerless county, which is exactly the case a neutral county's militia
/// hits — see [`raise_defence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevyBasket {
    pub slots: [BasketSlot; BASKET_SLOTS],
}

impl LevyBasket {
    /// `Levy_Init` / `FUN_004AA90A` — clear the basket, seed slots 1…6 from the
    /// realm's weapon stockpiles, and put every man in slot 0 and slot 7.
    ///
    /// ```c
    /// for t in 0..8: slot[t].chosen = slot[t].available = 0;
    /// for t in 1..7: slot[t].available = slot[t].remaining = realm.weapons[t - 1];
    /// slot[0].chosen = slot[0].remaining = men;
    /// slot[7].chosen = slot[7].remaining = men;
    /// ```
    pub fn seed(realm: &Realm, men: i32) -> LevyBasket {
        let mut basket = LevyBasket::default();
        for t in 1..=WEAPON_TYPE_COUNT {
            let stock = realm.weapons[t - 1];
            basket.slots[t].available = stock;
            basket.slots[t].remaining = stock;
        }
        basket.slots[0].chosen = men;
        basket.slots[0].remaining = men;
        basket.slots[BASKET_TOTAL].chosen = men;
        basket.slots[BASKET_TOTAL].remaining = men;
        basket
    }

    /// The levy total — slot 7, which the `+`/`−` buttons never move.
    pub fn total(&self) -> i32 {
        self.slots[BASKET_TOTAL].chosen
    }

    /// The men still carrying nothing.
    pub fn unequipped(&self) -> i32 {
        self.slots[0].chosen
    }

    /// The seven troop counts an army raised from this basket would have.
    pub fn troops(&self) -> [i32; TROOP_TYPES] {
        core::array::from_fn(|t| self.slots[t].chosen)
    }

    /// Move `n` men from the unequipped pool into a weapon slot, as the `+`
    /// button does one at a time. Returns how many actually moved: the move is
    /// bounded by the men available and by the stock left.
    pub fn equip(&mut self, troop: TroopType, n: i32) -> i32 {
        let Some(slot) = troop.weapon_slot().map(|w| w + 1) else { return 0 };
        let moved = n.min(self.slots[0].chosen).min(self.slots[slot].remaining).max(0);
        self.slots[0].chosen -= moved;
        self.slots[slot].chosen += moved;
        self.slots[slot].remaining -= moved;
        moved
    }

    /// Put a weapon slot's men back in the unequipped pool, as the `−` button
    /// does.
    pub fn unequip(&mut self, troop: TroopType, n: i32) -> i32 {
        let Some(slot) = troop.weapon_slot().map(|w| w + 1) else { return 0 };
        let moved = n.min(self.slots[slot].chosen).max(0);
        self.slots[slot].chosen -= moved;
        self.slots[slot].remaining += moved;
        self.slots[0].chosen += moved;
        moved
    }

    /// The AI's auto-equip: ten men at a time, round-robin over the six weapon
    /// types.
    ///
    /// ```c
    /// men = slot[7].chosen;  pass = 0;  changed = true;
    /// while (pass < 50 && changed) {
    ///     changed = false;
    ///     for (t = 1; t < 7; t++) {
    ///         if (men < 10) goto done;
    ///         if (slot[t].remaining >= 10) {
    ///             slot[t].chosen += 10;  slot[t].remaining -= 10;
    ///             slot[0].chosen -= 10;  men -= 10;  changed = true;
    ///         }
    ///     }
    ///     pass++;
    /// }
    /// ```
    ///
    /// > **`docs/armies.md` §6.2 says it runs *"until either the men or a
    /// > weapon type runs out"*. Wrong on the second half:** a weapon type that
    /// > runs out is *skipped* on every later pass and the round-robin
    /// > continues with the others, so an army is equipped from whatever the
    /// > armoury still has rather than stopping at the first empty rack. There
    /// > is also a **50-pass ceiling** — at six types and ten men a pass that is
    /// > 3,000 men, twice [`crate::tables::ARMY_MAX_MEN`], so it never bites in
    /// > play — and the floor is `men < 10`, not `men == 0`, so **up to nine men
    /// > are always left as peasants.** Corrected in the document. `[V]`
    pub fn auto_equip(&mut self) {
        let mut men = self.slots[BASKET_TOTAL].chosen;
        let mut pass = 0;
        let mut changed = true;
        while pass < crate::tables::LEVY_AUTO_EQUIP_ROUNDS && changed {
            changed = false;
            for t in 1..=WEAPON_TYPE_COUNT {
                if men < crate::tables::LEVY_AUTO_EQUIP_BATCH {
                    return;
                }
                if self.slots[t].remaining >= crate::tables::LEVY_AUTO_EQUIP_BATCH {
                    let n = crate::tables::LEVY_AUTO_EQUIP_BATCH;
                    self.slots[t].chosen += n;
                    self.slots[t].remaining -= n;
                    self.slots[0].chosen -= n;
                    men -= n;
                    changed = true;
                }
            }
            pass += 1;
        }
    }

    /// What the realm's stockpiles look like after this basket is spent —
    /// `Levy_ConsumeWeapons` (`0x004A9EB1`).
    pub fn consume_weapons(&self, realm: &mut Realm) {
        for t in 1..=WEAPON_TYPE_COUNT {
            realm.weapons[t - 1] = (realm.weapons[t - 1] - self.slots[t].chosen).max(0);
        }
    }
}

/// What a levy percentage would cost and yield — the two globals
/// `Levy_SetPercent` writes, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levy {
    /// `g_levyMen` (`0x00543FD8`).
    pub men: i32,
    /// `g_levyHappinessCost` (`0x00565400`).
    pub happiness_cost: i32,
    /// The percentage the walk-back settled on.
    ///
    /// **The original does not write this back.** `pct` is a by-value parameter
    /// and `g_levyPercent` stays where the player put the slider while the two
    /// globals hold the reduced figures — so the slider can read 80% while the
    /// county is only giving up 59%. Carried here because a caller needs to know
    /// what actually happened, and named `settled` rather than `percent` so it
    /// cannot be mistaken for the slider.
    pub settled: i32,
}

/// `Levy_SetPercent` (`0x00435EBC`) — what taking `percent` of a county costs.
///
/// ```c
/// men  = Pct(county.population, pct);
/// cost = g_armyHappinessCost[pct];
/// if (pct != 0) cost += county.levySurcharge;
/// if (cost > 100) cost = 100;
/// if (cost < 0)   cost = 0;
/// if (county.happiness < 1) { men = 0; cost = 0; }
/// else while (county.happiness - cost < 1) { pct--; ...recompute... }
/// ```
///
/// > **Three corrections to `docs/armies.md` §6.1.**
/// >
/// > 1. Its bare `clamp 0 .. 100;` is ambiguous and reads as if the
/// >    *percentage* is clamped. **The clamp is on the happiness cost, and it
/// >    is applied after the surcharge is added.** The percentage is clamped by
/// >    the slider handler, not here.
/// > 2. The pseudocode **omits the `happiness < 1` early-out**, which is the
/// >    walk-back's only termination guard. Without it a county at zero
/// >    happiness would walk `pct` negative and index the table at −1 forever.
/// > 3. It says *"in a county at happiness 100 with no surcharge the largest
/// >    levy is 59 % (cost 98)"*. The percentage is right and **the cost is
/// >    99** — the table's index 58 is 98 and index 59 is 99, and the loop needs
/// >    `happiness - cost >= 1`, so 99 is exactly affordable at happiness 100.
/// >
/// > Its reasoning about the saturation is also wrong even though the
/// > conclusion survives: *"101 … exceeds any possible happiness, so the
/// > walk-back loop always fires"*. 101 is clamped to 100 before the
/// > comparison, and `100 - 100 = 0 < 1` is what fires the loop. `[V]`
///
/// County `+0x2F4`, the surcharge, is set to 15 by [`create_army`] and **decays
/// by 5 a season** in `Happiness_UpdateAll` — so it is gone after three
/// seasons. §6.1 records that as untraced; see
/// [`crate::happiness::decay_levy_surcharge`].
pub fn set_percent(t: &Tables, county: &County, percent: i32) -> Levy {
    let cost_of = |p: i32| {
        let mut cost = t.army_happiness_cost(p);
        if p != 0 {
            cost += county.levy_surcharge;
        }
        cost.clamp(0, crate::tables::LEVY_COST_MAX)
    };

    let mut pct_taken = percent;
    let mut men = pct(county.population, pct_taken);
    let mut cost = cost_of(pct_taken);

    if county.happiness < 1 {
        return Levy { men: 0, happiness_cost: 0, settled: pct_taken };
    }
    while county.happiness - cost < 1 {
        pct_taken -= 1;
        men = pct(county.population, pct_taken);
        cost = cost_of(pct_taken);
    }
    Levy { men, happiness_cost: cost, settled: pct_taken }
}

/// Why a raise-army order was refused.
///
/// **All three live in the confirm handler `FUN_00435B4D`, not in
/// `Army_Create`.** `docs/armies.md` §6.3 attributes the first two to
/// `Army_Create`; the third it does not mention at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevyRefusal {
    /// Message `0xA8` = `L2.eng` group 168, *"Your proposed army of zero men
    /// fails to fulfill certain principles of medieval troop management"*.
    NoMen,
    /// Message `0x94` = group 148, *"impractical to create an army of less than
    /// 50 men"*.
    TooFew,
    /// Message `0xDD` = group 221. `Army_Create` returned 0: there is no free
    /// road tile and no free open tile in the county to put the army on.
    NowhereToStand,
}

/// The two size refusals, which a **mercenary hire bypasses entirely** because
/// the band supplies the men.
///
/// `[V]` — `hireMercs` is `&&`-ed into both guards.
pub fn refuse_levy(men: i32, hiring_mercenaries: bool) -> Option<LevyRefusal> {
    if hiring_mercenaries {
        return None;
    }
    if men == 0 {
        Some(LevyRefusal::NoMen)
    } else if men < crate::tables::ARMY_MIN_MEN {
        Some(LevyRefusal::TooFew)
    } else {
        None
    }
}

/// `County_FindFreeRoadTile`, then `County_FindFreeOpenTile` — where a new army
/// is put.
///
/// A road tile is preferred, and any passable unoccupied tile of the county
/// will do otherwise. Walked in ascending tile index so the choice is
/// deterministic and reproducible, which the original's own scan order gives
/// for free.
pub fn muster_tile(map: &CampaignMap, units: &Units, county: u8) -> Option<(u8, u8)> {
    let cost = map.cost_map();
    let free = |x: u8, y: u8| {
        map.county_at(x, y) == county && cost.at(x, y) != 0 && units.at(x, y).is_none()
    };
    let mut fallback = None;
    for y in 0..crate::map::MAP_DIM as u8 {
        for x in 0..crate::map::MAP_DIM as u8 {
            if !free(x, y) {
                continue;
            }
            if map.has(x, y, flags::ROAD) {
                return Some((x, y));
            }
            if fallback.is_none() {
                fallback = Some((x, y));
            }
        }
    }
    fallback
}

/// Everything `create_army` needs that is not a county, a realm or a basket.
#[derive(Debug, Clone, Copy)]
pub struct Muster {
    /// The realm raising it. **0 raises an ownerless unit** — the unit's owner
    /// byte becomes 6 and its shield 0 — which is how a neutral county's
    /// militia is created. `docs/armies.md` §1.1 records the 6 and §6.3 does not
    /// connect it to this branch.
    pub realm: u8,
    pub county: u8,
    /// The happiness the county is charged. **A parameter, not a global**: the
    /// AI paths pass an unclamped, surcharge-free figure straight out of the
    /// table, which is what makes the clamp in [`create_army`] reachable.
    pub happiness_cost: i32,
    pub year: i32,
}

/// `Army_Create` (`0x004A9A9A`) — put a levy on the map.
///
/// The order of operations, which is the rule:
///
/// ```c
/// tile = County_FindFreeRoadTile(county) || County_FindFreeOpenTile(county);
/// if (!tile) return 0;                                  /* message 0xDD */
/// Unit_Spawn(1, tile.x, tile.y, realm);                 /* zeroes the record */
/// u.isPlayerDriven = u.needsDestination = 1;
/// u.ownerIsHuman   = realm.isHuman;
/// u.county = u.homeCounty = county;
/// u.yearFormed = g_year;
/// u.morale     = county.happiness;                      /* BEFORE the debit */
/// if (realm == 0) { u.shield = 0; u.owner = 6; } else u.shield = realm.shield;
/// Levy_DebitPopulation(county);                         /* pop -= total; army -= total */
/// u.men = basket[7].chosen;  u.troops[t] = basket[t].chosen;
/// u.nameIndex = Army_PickName(realm);
/// Levy_ConsumeWeapons(realm);
/// if (hireMercs) Mercenary_Hire(unit, county.mercOffer);
/// Army_RecountCountyTroops();  Wages_ForUnit(unit);
/// ...the county's food passes, twice...
/// if (county.happiness < cost) { shownArmy -= happiness; happiness = 0; }
/// else                        { happiness -= cost; shownArmy -= cost; }
/// county.levySurcharge = 15;
/// realm.wages = Wages_ForRealm(realm);
/// ```
///
/// > **Three corrections to `docs/armies.md` §6.3, all `[V]`.**
/// >
/// > 1. **`morale` is the county's happiness *before* the levy cost is
/// >    deducted.** It is the fifth write after the spawn and the debit is
/// >    nearly last, so a county at 80 that pays 30 for its army still gives it
/// >    morale 80. The document's pseudocode lists them in the other order.
/// > 2. **The happiness debit is clamped.** §6.3 renders it as a flat
/// >    `happiness -= cost`. When the county cannot afford the full cost its
/// >    happiness goes to 0 and the panel is debited only what was actually
/// >    taken, so the two always agree. This is reachable: the AI paths pass an
/// >    unclamped cost.
/// > 3. **`Army_Create` sets neither `moveAllowance` nor `movesUsed`.**
/// >    `Unit_Spawn` memsets the whole `0x1A4`-byte record, so a fresh army has
/// >    an allowance of **0** until the next tick, when `Army_Tick` writes 15.
/// >    Reproduced by [`crate::unit::Unit::new`] setting the allowance from the
/// >    kind — the one-frame stale 0 is a rendering artefact of the original's
/// >    frame loop and not a rule.
///
/// The **two food passes really do run twice**, which is not a transcription
/// slip: `Food_Available` is recomputed from the ration pass's per-season caps,
/// so the army's own foraging has to be settled before the happiness is
/// charged.
///
/// Returns the new unit's slot.
#[allow(clippy::too_many_arguments)]
pub fn create_army(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    basket: &LevyBasket,
    muster: Muster,
) -> Result<usize, LevyRefusal> {
    let (x, y) = muster_tile(map, units, muster.county).ok_or(LevyRefusal::NowhereToStand)?;

    let (is_human, shield) = realms
        .get(muster.realm as usize)
        .map_or((false, 0), |r| (r.is_human, r.shield_index));
    let county_happiness = counties
        .get(muster.county as usize)
        .map_or(0, |c| c.happiness);

    let mut unit = Unit::new(UnitKind::Army, muster.realm, x, y);
    unit.player_driven = true;
    unit.needs_destination = true;
    unit.owner_is_human = is_human;
    unit.county = muster.county;
    unit.home_county = muster.county;
    unit.year_formed = muster.year;
    unit.morale = county_happiness;
    if muster.realm == 0 {
        unit.owner = OWNERLESS;
        unit.shield = 0;
    } else {
        unit.shield = shield;
    }
    unit.men = basket.total();
    unit.troops = basket.troops();
    unit.name_index = names.pick(muster.realm);

    let id = units.spawn(unit).ok_or(LevyRefusal::NowhereToStand)?;

    if let Some(county) = counties.get_mut(muster.county as usize) {
        // `Levy_DebitPopulation` (`0x004A9F18`). County `+0x38` is the
        // population panel's *"Army"* line, a display accumulator that
        // `Population_UpdateAll` zeroes every season, so this half is
        // transient; `+0x24` is the real population.
        county.population -= basket.total();
        county.army -= basket.total();

        // The clamp §6.3 leaves out.
        if county.happiness < muster.happiness_cost {
            county.shown_army -= county.happiness;
            county.happiness = 0;
        } else {
            county.happiness -= muster.happiness_cost;
            county.shown_army -= muster.happiness_cost;
        }
        county.levy_surcharge = crate::tables::LEVY_SURCHARGE;
    }
    if let Some(realm) = realms.get_mut(muster.realm as usize) {
        basket.consume_weapons(realm);
    }
    let realms_snapshot: [Realm; MAX_REALMS] = realms.clone();
    units.recount_county_troops(counties, &realms_snapshot);
    crate::unit::refresh_wages(t, units, realms, muster.realm, 0);
    Ok(id)
}

/// The owner byte `Army_Create` writes for `realm == 0`: an **ownerless** unit,
/// which is what a neutral county's militia is. `docs/armies.md` §1.1 records
/// the value without naming the branch that produces it.
pub const OWNERLESS: u8 = 6;

/// How a county's defence is equipped when it is raised to meet an invader —
/// `FUN_004A50AE`'s three modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Defence {
    /// Mode 0 — a **human**-owned county. Nothing is equipped at all: the
    /// defence is entirely peasants. `[D]`, and it is the harshest of the three.
    HumanCounty,
    /// Mode 1 — an **AI**-owned county. The auto-equip round-robin runs over
    /// the realm's real stockpiles.
    AiCounty,
    /// Mode 2 — a **neutral** county. It has no realm and no stockpile, so the
    /// function *grants* 500 each of pikes, bows and maces to realm 0 and then
    /// equips by a size ladder. The happiness cost is forced to zero: nobody
    /// owns the county to be angry at.
    Militia,
}

/// The size ladder a neutral county's militia is equipped by, as
/// `(men at least, archers, pikemen, macemen)`.
///
/// Read straight out of `FUN_004A50AE`'s nested `if`, which tests 480, 360, 240
/// and 120 in that order and equips nobody below 120. The three slots are
/// basket slots 5, 4 and 2 — archers, pikemen, macemen — and the peasant slot
/// is decremented by the sum. `[D]`
pub const MILITIA_LADDER: [(i32, i32, i32, i32); 4] =
    [(480, 150, 100, 50), (360, 100, 70, 0), (240, 80, 40, 0), (120, 60, 0, 0)];

/// The population a county must have before it will raise a defence at all —
/// `FUN_004A50AE`'s `if (county.population < 40) return 0`.
pub const DEFENCE_MIN_POPULATION: i32 = 40;

/// The percentage of its population a **neutral** county levies to defend
/// itself, by difficulty 0…3.
pub const MILITIA_PERCENT_BY_DIFFICULTY: [i32; 4] = [25, 40, 50, 60];

/// The percentage an **owned** county levies. Flat, regardless of difficulty.
pub const DEFENCE_PERCENT: i32 = 40;

/// `FUN_004A50AE` — raise a county's defence force in the face of an invader.
///
/// This is the function `docs/armies.md` never names, and it is what turns
/// walking onto a county into a fight rather than a capture. It levies a
/// percentage of the county's population, equips it according to who owns the
/// county, and calls `Army_Create`.
///
/// Returns the new unit, or `None` when the county has fewer than
/// [`DEFENCE_MIN_POPULATION`] people — in which case there is no defence and
/// [`crate::conquest::attack_county`] captures outright.
#[allow(clippy::too_many_arguments)]
pub fn raise_defence(
    t: &Tables,
    map: &CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    county: u8,
    percent: i32,
    mode: Defence,
    year: i32,
) -> Option<usize> {
    let c = counties.get(county as usize)?;
    if c.population < DEFENCE_MIN_POPULATION {
        return None;
    }
    let realm = c.owner;
    let men = pct(c.population, percent);
    let mut happiness_cost = t.army_happiness_cost(percent);

    let mut basket = LevyBasket::seed(realms.get(realm as usize)?, men);
    match mode {
        Defence::HumanCounty => {}
        Defence::AiCounty => basket.auto_equip(),
        Defence::Militia => {
            // The grant. Realm 0 is nobody's, so this is bookkeeping rather
            // than a gift to a player, but it is what the original writes and
            // it is what the ladder spends.
            for troop in [TroopType::Pikeman, TroopType::Archer, TroopType::Maceman] {
                let slot = troop.weapon_slot().expect("all three are equipped types") + 1;
                basket.slots[slot].available = MILITIA_GRANT;
                basket.slots[slot].remaining = MILITIA_GRANT;
            }
            happiness_cost = 0;
            if let Some(&(_, archers, pikes, maces)) =
                MILITIA_LADDER.iter().find(|&&(floor, ..)| men >= floor)
            {
                basket.slots[TroopType::Archer.index()].chosen = archers;
                basket.slots[TroopType::Pikeman.index()].chosen = pikes;
                basket.slots[TroopType::Maceman.index()].chosen = maces;
                basket.slots[0].chosen -= archers + pikes + maces;
            }
        }
    }

    create_army(
        t,
        map,
        counties,
        realms,
        units,
        names,
        &basket,
        Muster { realm, county, happiness_cost, year },
    )
    .ok()
}

/// What `FUN_004A50AE` grants realm 0 of each of pikes, bows and maces before
/// equipping a neutral county's militia.
pub const MILITIA_GRANT: i32 = 500;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MAP_TILES;

    const T: &Tables = &Tables::DEFAULT;

    fn open_map() -> CampaignMap {
        let mut m = CampaignMap::empty();
        for i in 0..MAP_TILES {
            m.county[i] = 1;
        }
        m
    }

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        counties[1].owner = 1;
        counties[1].population = 500;
        counties[1].happiness = 100;
        realms[1].in_play = true;
        realms[1].is_human = true;
        realms[1].shield_index = 3;
        (counties, realms)
    }

    // --- the levy ----------------------------------------------------------

    #[test]
    fn the_levy_takes_a_percentage_of_the_people_and_the_table_prices_it() {
        let (counties, _) = world();
        let l = set_percent(T, &counties[1], 10);
        assert_eq!(l.men, 50, "a tenth of five hundred");
        assert_eq!(l.happiness_cost, 5, "the table's index 10");
        assert_eq!(l.settled, 10);
    }

    /// The correction: at happiness 100 with no surcharge the largest levy is
    /// 59 %, and **it costs 99, not the 98 `docs/armies.md` §6.1 quotes.**
    #[test]
    fn the_largest_levy_a_hundred_happiness_affords_is_fifty_nine_percent_at_ninety_nine() {
        let (counties, _) = world();
        assert_eq!(T.army_happiness_cost(58), 98);
        assert_eq!(T.army_happiness_cost(59), 99, "not 98");
        assert_eq!(T.army_happiness_cost(60), 100);

        let l = set_percent(T, &counties[1], 100);
        assert_eq!(l.settled, 59, "walked back from 100");
        assert_eq!(l.happiness_cost, 99);
        assert_eq!(l.men, pct(500, 59));
        // …and 60% would cost exactly the county's whole happiness, which the
        // `>= 1` test refuses.
        assert!(counties[1].happiness - T.army_happiness_cost(60) < 1);
    }

    /// The `happiness < 1` early-out, which §6.1's pseudocode omits and which
    /// is the walk-back's only termination guard.
    #[test]
    fn a_county_at_zero_happiness_raises_nobody_rather_than_looping_forever() {
        let (mut counties, _) = world();
        counties[1].happiness = 0;
        let l = set_percent(T, &counties[1], 50);
        assert_eq!((l.men, l.happiness_cost), (0, 0));

        counties[1].happiness = 1;
        let l = set_percent(T, &counties[1], 50);
        assert_eq!(l.happiness_cost, 0, "one happiness buys the zero-cost levy only");
        assert_eq!(l.men, 0);
    }

    /// The surcharge is added **before** the clamp, and the clamp is on the
    /// cost.
    #[test]
    fn the_surcharge_is_added_before_the_cost_is_clamped_at_a_hundred() {
        let (mut counties, _) = world();
        counties[1].levy_surcharge = crate::tables::LEVY_SURCHARGE;
        // Index 20 is 10; plus the surcharge that is 25.
        let l = set_percent(T, &counties[1], 20);
        assert_eq!(l.happiness_cost, 25);
        assert_eq!(l.settled, 20, "still affordable at happiness 100");

        // A surcharge makes the same county give up fewer men than before.
        let plain = {
            let mut c = counties[1].clone();
            c.levy_surcharge = 0;
            set_percent(T, &c, 100)
        };
        let surcharged = set_percent(T, &counties[1], 100);
        assert!(surcharged.settled < plain.settled, "the second army is dearer");
        assert!(surcharged.happiness_cost <= crate::tables::LEVY_COST_MAX);
    }

    #[test]
    fn a_levy_of_nothing_costs_nothing_even_with_a_surcharge_standing() {
        let (mut counties, _) = world();
        counties[1].levy_surcharge = 15;
        let l = set_percent(T, &counties[1], 0);
        assert_eq!((l.men, l.happiness_cost), (0, 0), "the surcharge is skipped at pct 0");
    }

    // --- the basket --------------------------------------------------------

    #[test]
    fn a_seeded_basket_puts_every_man_in_the_peasant_slot_and_the_total() {
        let (_, mut realms) = world();
        realms[1].weapons = [40, 0, 25, 0, 60, 5];
        let b = LevyBasket::seed(&realms[1], 300);
        assert_eq!(b.total(), 300);
        assert_eq!(b.unequipped(), 300);
        assert_eq!(b.troops(), [300, 0, 0, 0, 0, 0, 0]);
        assert_eq!(b.slots[1].available, 40, "crossbows");
        assert_eq!(b.slots[6].available, 5, "armour");
        assert_eq!(b.slots[1].remaining, b.slots[1].available);
    }

    #[test]
    fn equipping_moves_men_out_of_the_peasant_slot_and_spends_the_stock() {
        let (_, mut realms) = world();
        realms[1].weapons = [40, 0, 0, 0, 0, 5];
        let mut b = LevyBasket::seed(&realms[1], 100);
        assert_eq!(b.equip(TroopType::Crossbowman, 30), 30);
        assert_eq!(b.troops(), [70, 30, 0, 0, 0, 0, 0]);
        assert_eq!(b.slots[1].remaining, 10);

        assert_eq!(b.equip(TroopType::Crossbowman, 50), 10, "bounded by the stock");
        assert_eq!(b.equip(TroopType::Knight, 500), 5, "five suits of armour");
        assert_eq!(b.equip(TroopType::Peasant, 10), 0, "a peasant carries nothing");
        assert_eq!(b.unequipped(), 100 - 40 - 5);
        assert_eq!(b.total(), 100, "the total never moves");

        assert_eq!(b.unequip(TroopType::Crossbowman, 15), 15);
        assert_eq!(b.slots[1].chosen, 25);
        assert_eq!(b.slots[1].remaining, 15);
    }

    /// The correction: a spent weapon type is skipped, not terminal.
    #[test]
    fn auto_equip_skips_an_empty_rack_and_keeps_filling_the_others() {
        let (_, mut realms) = world();
        // Only crossbows and armour in the armoury: 25 and 100.
        realms[1].weapons = [25, 0, 0, 0, 0, 100];
        let mut b = LevyBasket::seed(&realms[1], 200);
        b.auto_equip();
        let troops = b.troops();
        assert_eq!(troops[TroopType::Crossbowman.index()], 20, "two batches, then 5 left");
        assert!(troops[TroopType::Knight.index()] >= 100, "the armour kept being handed out");
        assert_eq!(troops.iter().sum::<i32>(), 200, "nobody is lost");
    }

    /// …and the floor is `men < 10`, so up to nine always stay peasants.
    #[test]
    fn auto_equip_always_leaves_the_last_nine_men_as_peasants() {
        let (_, mut realms) = world();
        realms[1].weapons = [1000; WEAPON_TYPE_COUNT];
        for men in [100, 107, 109, 250] {
            let mut b = LevyBasket::seed(&realms[1], men);
            b.auto_equip();
            assert_eq!(b.unequipped(), men % 10, "{men} men");
            assert!(b.unequipped() < 10);
        }
    }

    /// The 50-pass ceiling: at six types and ten men a pass it can equip 3,000,
    /// twice the maximum army, so it never bites in play.
    #[test]
    fn the_auto_equip_ceiling_is_beyond_any_army_that_can_exist() {
        let (_, mut realms) = world();
        realms[1].weapons = [100_000; WEAPON_TYPE_COUNT];
        let mut b = LevyBasket::seed(&realms[1], 10_000);
        b.auto_equip();
        let equipped: i32 = b.troops()[1..].iter().sum();
        assert_eq!(equipped, 3000, "fifty passes of six tens");
        assert!(equipped > crate::tables::ARMY_MAX_MEN);
    }

    #[test]
    fn spending_a_basket_takes_the_weapons_out_of_the_realms_stockpiles() {
        let (_, mut realms) = world();
        realms[1].weapons = [40, 0, 0, 0, 0, 10];
        let mut b = LevyBasket::seed(&realms[1], 100);
        b.equip(TroopType::Crossbowman, 30);
        b.equip(TroopType::Knight, 10);
        b.consume_weapons(&mut realms[1]);
        assert_eq!(realms[1].weapons, [10, 0, 0, 0, 0, 0]);
    }

    // --- the refusals ------------------------------------------------------

    #[test]
    fn an_army_of_fewer_than_fifty_is_refused_unless_mercenaries_supply_the_men() {
        assert_eq!(refuse_levy(0, false), Some(LevyRefusal::NoMen));
        assert_eq!(refuse_levy(49, false), Some(LevyRefusal::TooFew));
        assert_eq!(refuse_levy(50, false), None);
        assert_eq!(refuse_levy(0, true), None, "a band supplies the men");
        assert_eq!(refuse_levy(49, true), None);
    }

    // --- creating ----------------------------------------------------------

    #[test]
    fn raising_an_army_takes_the_men_out_of_the_county_and_charges_its_happiness() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        realms[1].weapons = [50, 0, 0, 0, 0, 0];

        let levy = set_percent(T, &counties[1], 20);
        assert_eq!((levy.men, levy.happiness_cost), (100, 10));
        let mut basket = LevyBasket::seed(&realms[1], levy.men);
        basket.equip(TroopType::Crossbowman, 50);

        let id = create_army(
            T,
            &m,
            &mut counties,
            &mut realms,
            &mut units,
            &mut names,
            &basket,
            Muster { realm: 1, county: 1, happiness_cost: levy.happiness_cost, year: 1268 },
        )
        .expect("there is room in the county");

        let u = units.get(id).unwrap();
        assert_eq!(u.men, 100);
        assert_eq!(u.troops, [50, 50, 0, 0, 0, 0, 0]);
        assert_eq!(u.owner, 1);
        assert_eq!(u.shield, 3);
        assert!(u.owner_is_human);
        assert_eq!((u.county, u.home_county), (1, 1));
        assert_eq!(u.year_formed, 1268);
        assert_eq!(u.kind, UnitKind::Army);
        assert!(u.needs_destination);

        assert_eq!(counties[1].population, 400);
        assert_eq!(counties[1].happiness, 90);
        assert_eq!(counties[1].shown_army, -10);
        assert_eq!(counties[1].levy_surcharge, crate::tables::LEVY_SURCHARGE);
        assert_eq!(counties[1].friendly_troops, 100, "the recount ran");
        assert_eq!(realms[1].weapons[0], 0, "the crossbows were issued");
        assert_eq!(realms[1].wages, 25, "a hundred men at a quarter each");
        assert_eq!(units.get(id).unwrap().wages, 25);
    }

    /// The correction: morale is the happiness the county had **before** the
    /// levy cost was taken off it.
    #[test]
    fn morale_is_the_countys_happiness_before_the_levy_is_charged() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let basket = LevyBasket::seed(&realms[1], 100);

        let id = create_army(
            T,
            &m,
            &mut counties,
            &mut realms,
            &mut units,
            &mut names,
            &basket,
            Muster { realm: 1, county: 1, happiness_cost: 40, year: 1268 },
        )
        .unwrap();
        assert_eq!(units.get(id).unwrap().morale, 100, "not the 60 it is left at");
        assert_eq!(counties[1].happiness, 60);
    }

    /// The clamp §6.3 leaves out: the panel is debited what was taken, not what
    /// was asked for, so the two always agree.
    #[test]
    fn a_county_that_cannot_afford_the_cost_goes_to_zero_and_the_panel_agrees() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        counties[1].happiness = 12;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let basket = LevyBasket::seed(&realms[1], 100);

        create_army(
            T,
            &m,
            &mut counties,
            &mut realms,
            &mut units,
            &mut names,
            &basket,
            Muster { realm: 1, county: 1, happiness_cost: 50, year: 1268 },
        )
        .unwrap();
        assert_eq!(counties[1].happiness, 0);
        assert_eq!(counties[1].shown_army, -12, "what was taken, not the fifty asked for");
    }

    #[test]
    fn an_army_is_mustered_on_a_road_tile_where_there_is_one() {
        let mut m = open_map();
        m.set_flags(20, 20, flags::ROAD);
        let units = Units::new();
        assert_eq!(muster_tile(&m, &units, 1), Some((20, 20)));

        // With the road occupied it falls back to open ground, lowest index.
        let mut units = Units::new();
        units.spawn(Unit::new(UnitKind::Army, 1, 20, 20));
        assert_eq!(muster_tile(&m, &units, 1), Some((0, 0)));
    }

    #[test]
    fn a_county_with_nowhere_to_stand_refuses_the_army() {
        // Every tile impassable.
        let mut m = CampaignMap::empty();
        for i in 0..MAP_TILES {
            m.county[i] = 1;
            m.flags[i] = flags::NO_COUNTY;
        }
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let basket = LevyBasket::seed(&realms[1], 100);
        assert_eq!(
            create_army(
                T,
                &m,
                &mut counties,
                &mut realms,
                &mut units,
                &mut names,
                &basket,
                Muster { realm: 1, county: 1, happiness_cost: 0, year: 1268 },
            ),
            Err(LevyRefusal::NowhereToStand)
        );
    }

    /// `realm == 0` raises an **ownerless** unit — owner byte 6 — which is what
    /// a neutral county's militia is.
    #[test]
    fn a_neutral_countys_defence_is_ownerless_rather_than_owned_by_realm_zero() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        counties[2].owner = 0;
        counties[2].population = 500;
        counties[2].happiness = 77;
        for i in 0..MAP_TILES {
            // Give county 2 some ground of its own.
            if i % 64 > 40 {
                // leave county 1 alone elsewhere
            }
        }
        let mut m2 = m.clone();
        for y in 0..crate::map::MAP_DIM as u8 {
            for x in 40..crate::map::MAP_DIM as u8 {
                m2.set_county(x, y, 2);
            }
        }
        let mut units = Units::new();
        let mut names = ArmyNames::new();

        let id = raise_defence(
            T, &m2, &mut counties, &mut realms, &mut units, &mut names, 2, 25, Defence::Militia, 1268,
        )
        .expect("five hundred people can defend themselves");
        let u = units.get(id).unwrap();
        assert_eq!(u.owner, OWNERLESS);
        assert_eq!(u.shield, 0);
        assert_eq!(u.men, 125, "a quarter of five hundred");
        // 125 men is above the ladder's 120 floor, so 60 of them get bows.
        assert_eq!(u.troops[TroopType::Archer.index()], 60);
        assert_eq!(u.troops[TroopType::Peasant.index()], 65);
        assert_eq!(counties[2].happiness, 77, "a militia costs the county nothing");
    }

    /// The whole ladder, and the floor below which a militia is all peasants.
    #[test]
    fn the_militia_ladder_equips_by_size_and_arms_nobody_below_a_hundred_and_twenty() {
        for (men, want) in [
            (500, (150, 100, 50)),
            (480, (150, 100, 50)),
            (479, (100, 70, 0)),
            (360, (100, 70, 0)),
            (240, (80, 40, 0)),
            (120, (60, 0, 0)),
            (119, (0, 0, 0)),
            (50, (0, 0, 0)),
        ] {
            let found = MILITIA_LADDER
                .iter()
                .find(|&&(floor, ..)| men >= floor)
                .map(|&(_, a, p, m)| (a, p, m))
                .unwrap_or((0, 0, 0));
            assert_eq!(found, want, "{men} men");
        }
    }

    #[test]
    fn a_county_of_fewer_than_forty_people_raises_no_defence_at_all() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        counties[1].population = 39;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        assert!(raise_defence(
            T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, 40, Defence::AiCounty, 1268
        )
        .is_none());
        assert_eq!(units.len(), 0);
    }

    /// A human-owned county's defence is entirely peasants — mode 0 equips
    /// nothing, even out of a full armoury.
    #[test]
    fn a_human_countys_defence_is_all_peasants_however_full_the_armoury_is() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        realms[1].weapons = [500; WEAPON_TYPE_COUNT];
        let mut units = Units::new();
        let mut names = ArmyNames::new();

        let id = raise_defence(
            T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, DEFENCE_PERCENT,
            Defence::HumanCounty, 1268,
        )
        .unwrap();
        let u = units.get(id).unwrap();
        assert_eq!(u.men, 200);
        assert_eq!(u.troops, [200, 0, 0, 0, 0, 0, 0]);
        assert_eq!(realms[1].weapons, [500; WEAPON_TYPE_COUNT], "nothing was issued");
    }

    /// …where an AI county's is armed out of its stockpiles.
    #[test]
    fn an_ai_countys_defence_is_equipped_from_its_own_armoury() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        realms[1].is_human = false;
        realms[1].weapons = [500; WEAPON_TYPE_COUNT];
        let mut units = Units::new();
        let mut names = ArmyNames::new();

        let id = raise_defence(
            T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, DEFENCE_PERCENT,
            Defence::AiCounty, 1268,
        )
        .unwrap();
        let u = units.get(id).unwrap();
        assert_eq!(u.men, 200);
        assert!(u.troops[TroopType::Peasant.index()] < 10, "all but the last few are armed");
        assert_eq!(u.troops.iter().sum::<i32>(), 200);
    }

    #[test]
    fn the_militia_difficulty_ladder_only_ever_takes_more_people() {
        let mut last = 0;
        for pct in MILITIA_PERCENT_BY_DIFFICULTY {
            assert!(pct > last, "the harder game must not levy fewer");
            last = pct;
        }
        assert_eq!(MILITIA_PERCENT_BY_DIFFICULTY[0], 25);
    }
}
