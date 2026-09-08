//! Industry, weapons, wages and castles — `docs/kingdom.md` §7.4 and §7.5.
//!
//! # Industry
//!
//! `Industry_Produce` (`0x0044EA92`) runs four times per county per season:
//!
//! | commodity | job (`L2.eng` group 74) | divisor | base efficiency | credited to |
//! |---|---|---:|---:|---|
//! | wood | 7 Wood cutting | 1 | **20%** | realm `+0x130` |
//! | iron | 5 Iron mining | 1 | 15% | realm `+0x120` |
//! | weapons | 8 Blacksmith | **4** | 15% | realm `+0x140 + type*4` |
//! | stone | 6 Stone quarrying | **2** | 15% | realm `+0x128` |
//!
//! `output = min(resourceLimit, Pct(workers / divisor, efficiency))`. The 15%
//! base appears verbatim in a published FAQ (*"30 serfs working at 15%
//! efficiency"*), and *"iron and wood harvest at twice the quantity of stone"*
//! is exactly the divisor column.
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
    Commodity, BANKRUPT_STAGE_MAX, CASTLE_COST, CASTLE_FREE_ARCHERS, CASTLE_GARRISON_CAP,
    CASTLE_WORKFORCE, JOB_CASTLE_BUILDING, WEAPON_COST, WEAPON_TYPE_COUNT,
};

/// What one commodity's pass would produce, before it is credited anywhere.
pub fn output(county: &County, c: Commodity) -> i32 {
    let record = &county.industry[c.index()];
    let workers = county.labour[c.job()].max(0);
    let raw = pct(workers / c.divisor(), record.efficiency);
    raw.min(record.limit).max(0)
}

/// One `Industry_Produce` pass: produce, credit the realm, and record the
/// output on the county.
///
/// Weapons are the one commodity that *spends*: the blacksmith's output is
/// capped by what [`WEAPON_COST`] can be paid for out of the realm's wood and
/// iron. Nothing in `docs/kingdom.md` states that cap explicitly — §7.4 says
/// only *"weapons debit `g_weaponCost`"* — but a realm cannot debit stock it
/// does not have, and the alternative is a negative stockpile. **`[I]`**
pub fn produce(county: &mut County, realm: &mut Realm, c: Commodity) {
    let mut made = output(county, c);
    match c {
        Commodity::Wood => realm.wood += made,
        Commodity::Iron => realm.iron += made,
        Commodity::Stone => realm.stone += made,
        Commodity::Weapons => {
            let weapon = county.weapon_type.min(WEAPON_TYPE_COUNT - 1);
            let (wood, iron) = WEAPON_COST[weapon];
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
    county.industry[c.index()].output = made;
}

/// `Wages_PayAll` (`0x004ACBD4`) sums [`Realm::wage_for_unit`] over a realm's
/// units.
///
/// `men` is campaign unit `+0x168`, the field `docs/battle.md` §4.1 identified
/// as the total over troop types 0..=6. Units live in `g_units`, which is not
/// this crate's, so the caller supplies the per-unit totals in a stable order.
pub fn compute_wages(realm: &Realm, unit_men: &[i32], difficulty: u8) -> i32 {
    let mut total: i64 = 0;
    for &men in unit_men {
        total += realm.wage_for_unit(men, difficulty) as i64;
    }
    total as i32
}

/// Pay the bill in [`Realm::wages`], advancing the bankruptcy escalation if the
/// treasury cannot cover it.
///
/// **The escalation itself is not implemented, because it is not documented.**
/// `docs/kingdom.md` §2 records `+0x158` as a 0..=5 counter and marks it
/// **`[D]`**; what each stage *does* — disband units? seize goods? — was never
/// traced. This advances the counter, empties the treasury, and says so.
pub fn pay(realm: &mut Realm, id: u8, out: &mut Vec<Message>) {
    if realm.gold >= realm.wages {
        realm.gold -= realm.wages;
        realm.bankrupt_stage = 0;
        return;
    }
    realm.gold = 0;
    realm.bankrupt_stage = (realm.bankrupt_stage + 1).min(BANKRUPT_STAGE_MAX);
    out.push(Message::Bankrupt { realm: id, stage: realm.bankrupt_stage });
}

// ---------------------------------------------------------------------------
// Castles
// ---------------------------------------------------------------------------

/// The garrison a completed castle can hold.
pub fn garrison_cap(castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        CASTLE_GARRISON_CAP[(castle_type as usize - 1).min(CASTLE_GARRISON_CAP.len() - 1)]
    }
}

/// The archers a new castle comes with. The manual: *"A new castle will
/// automatically include a garrison. Its size will vary according to the size
/// of the castle."*
pub fn free_archers(castle_type: u8) -> i32 {
    if castle_type == 0 {
        0
    } else {
        CASTLE_FREE_ARCHERS[(castle_type as usize - 1).min(CASTLE_FREE_ARCHERS.len() - 1)]
    }
}

/// `(wood, stone)` to build a castle type, and the workforce it consumes.
pub fn castle_cost(castle_type: u8) -> (i32, i32) {
    CASTLE_COST[(castle_type.max(1) as usize - 1).min(CASTLE_COST.len() - 1)]
}

/// The workforce a castle type consumes.
///
/// The table holds two ints per level and both carry the same number. What the
/// second column is for is not established, so this reads the first and the
/// table keeps the pair rather than pretending it is a flat array.
pub fn castle_workforce(castle_type: u8) -> i32 {
    CASTLE_WORKFORCE[(castle_type.max(1) as usize - 1).min(CASTLE_WORKFORCE.len() - 1)].0
}

/// Order a castle: debit the realm's wood and stone and mark the county as
/// building. Returns `false` — changing nothing — if the realm cannot pay.
///
/// **Where the resources are debited is a choice, not a finding.**
/// `docs/kingdom.md` §7.5 gives the five cost tables and §3.4 names
/// `Castle_BuildTick` as a pass, and nothing says whether the cost is taken up
/// front or drawn down each season. Up front is the reading that cannot leave a
/// half-built castle owing resources a realm has since spent.
pub fn order_castle(county: &mut County, realm: &mut Realm, castle_type: u8) -> bool {
    if castle_type == 0 || castle_type as usize > CASTLE_COST.len() {
        return false;
    }
    let (wood, stone) = castle_cost(castle_type);
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
pub fn build_tick(county: &mut County, id: u8, out: &mut Vec<Message>) -> bool {
    if county.castle_building == 0 {
        return false;
    }
    county.castle_progress += county.labour[JOB_CASTLE_BUILDING].max(0);
    if county.castle_progress < castle_workforce(county.castle_building) {
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
    use crate::tables::JOB_BLACKSMITH;

    fn worker_county(job: usize, workers: i32) -> County {
        let mut c = County::new();
        c.labour[job] = workers;
        c
    }

    /// The FAQ's own example: *"30 serfs working at 15% efficiency"*.
    #[test]
    fn thirty_serfs_at_fifteen_percent_produce_four() {
        let c = worker_county(Commodity::Iron.job(), 30);
        assert_eq!(output(&c, Commodity::Iron), 4, "Pct(30 / 1, 15)");
    }

    /// *"Iron and wood harvest at twice the quantity of stone"* — the divisor
    /// column, stated as a ratio.
    #[test]
    fn iron_and_wood_harvest_at_twice_the_quantity_of_stone() {
        let workers = 200;
        let mut c = County::new();
        for cm in Commodity::ALL {
            c.labour[cm.job()] = workers;
        }
        let iron = output(&c, Commodity::Iron);
        let stone = output(&c, Commodity::Stone);
        assert_eq!(iron, 30);
        assert_eq!(stone, 15);
        assert_eq!(iron, stone * 2);
    }

    #[test]
    fn wood_is_the_only_commodity_with_a_twenty_percent_base() {
        assert_eq!(Commodity::Wood.base_efficiency(), 20);
        for c in Commodity::ALL {
            if c != Commodity::Wood {
                assert_eq!(c.base_efficiency(), 15, "{c:?}");
            }
        }
        let c = worker_county(Commodity::Wood.job(), 100);
        assert_eq!(output(&c, Commodity::Wood), 20);
    }

    #[test]
    fn the_resource_limit_caps_the_output() {
        let mut c = worker_county(Commodity::Wood.job(), 1000);
        assert_eq!(output(&c, Commodity::Wood), 200);
        c.industry[Commodity::Wood.index()].limit = 40;
        assert_eq!(output(&c, Commodity::Wood), 40);
    }

    #[test]
    fn each_commodity_is_credited_to_its_own_stockpile() {
        let mut c = County::new();
        for cm in Commodity::ALL {
            c.labour[cm.job()] = 400;
        }
        let mut r = Realm::new();
        produce(&mut c, &mut r, Commodity::Wood);
        produce(&mut c, &mut r, Commodity::Iron);
        produce(&mut c, &mut r, Commodity::Stone);
        assert_eq!(r.wood, 80);
        assert_eq!(r.iron, 60);
        assert_eq!(r.stone, 30);
        assert_eq!(r.weapons, [0; WEAPON_TYPE_COUNT]);
    }

    /// A crossbow costs 6 wood and 10 iron, so a blacksmith with the workers
    /// for 15 of them but the iron for 4 makes 4.
    #[test]
    fn the_blacksmith_is_capped_by_the_stock_the_weapon_costs() {
        let mut c = worker_county(JOB_BLACKSMITH, 400);
        c.weapon_type = 0; // crossbow: 6 wood, 10 iron
        assert_eq!(output(&c, Commodity::Weapons), 15, "Pct(400 / 4, 15)");

        let mut r = Realm::new();
        r.wood = 1000;
        r.iron = 40;
        produce(&mut c, &mut r, Commodity::Weapons);
        assert_eq!(r.weapons[0], 4, "the iron ran out first");
        assert_eq!(r.iron, 0);
        assert_eq!(r.wood, 1000 - 4 * 6);
    }

    /// A bow costs no iron at all, so an ironless realm can still make bows.
    #[test]
    fn bows_need_no_iron() {
        let mut c = worker_county(JOB_BLACKSMITH, 400);
        c.weapon_type = 4; // bow: 13 wood, 0 iron
        let mut r = Realm::new();
        r.wood = 1000;
        r.iron = 0;
        produce(&mut c, &mut r, Commodity::Weapons);
        assert_eq!(r.weapons[4], 15);
        assert_eq!(r.wood, 1000 - 15 * 13);
    }

    #[test]
    fn a_realm_with_nothing_in_stock_makes_no_weapons_and_owes_nothing() {
        let mut c = worker_county(JOB_BLACKSMITH, 4000);
        let mut r = Realm::new();
        produce(&mut c, &mut r, Commodity::Weapons);
        assert_eq!(r.weapons[0], 0);
        assert_eq!(r.wood, 0);
        assert_eq!(r.iron, 0, "and no negative stockpile");
    }

    // --- wages -------------------------------------------------------------

    #[test]
    fn the_wage_bill_is_the_sum_over_units_and_ignores_troop_type() {
        let mut r = Realm::new();
        r.is_human = true;
        assert_eq!(compute_wages(&r, &[250, 252, 254], 0), 62 + 63 + 63);
        assert_eq!(compute_wages(&r, &[], 0), 0);
    }

    #[test]
    fn a_realm_that_can_pay_clears_its_bankruptcy_counter() {
        let mut r = Realm::new();
        r.gold = 1000;
        r.wages = 400;
        r.bankrupt_stage = 3;
        let mut out = Vec::new();
        pay(&mut r, 1, &mut out);
        assert_eq!(r.gold, 600);
        assert_eq!(r.bankrupt_stage, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn a_realm_that_cannot_pay_escalates_and_stops_at_five() {
        let mut r = Realm::new();
        r.gold = 10;
        r.wages = 400;
        let mut out = Vec::new();
        for expected in [1u8, 2, 3, 4, 5, 5, 5] {
            pay(&mut r, 2, &mut out);
            assert_eq!(r.bankrupt_stage, expected);
            assert_eq!(r.gold, 0, "the treasury is emptied, never negative");
        }
        assert_eq!(out.len(), 7);
        assert_eq!(out[0], Message::Bankrupt { realm: 2, stage: 1 });
    }

    // --- castles -----------------------------------------------------------

    #[test]
    fn a_castle_cannot_be_ordered_without_the_wood_and_the_stone() {
        let mut c = County::new();
        let mut r = Realm::new();
        r.wood = 199;
        r.stone = 1000;
        assert!(!order_castle(&mut c, &mut r, 3), "a keep needs 200 wood");
        assert_eq!(c.castle_building, 0);
        assert_eq!(r.wood, 199, "and nothing is spent on a refused order");

        r.wood = 200;
        assert!(order_castle(&mut c, &mut r, 3));
        assert_eq!(c.castle_building, 3);
        assert_eq!((r.wood, r.stone), (0, 0));
    }

    #[test]
    fn a_royal_castle_takes_several_seasons_of_workers() {
        let mut c = County::new();
        let mut r = Realm::new();
        r.wood = 10_000;
        r.stone = 10_000;
        assert!(order_castle(&mut c, &mut r, 5));
        c.labour[JOB_CASTLE_BUILDING] = 500;

        let mut out = Vec::new();
        for season in 1..5 {
            assert!(!build_tick(&mut c, 1, &mut out), "season {season} is too early");
        }
        assert!(build_tick(&mut c, 1, &mut out), "2500 workforce, 500 a season");
        assert_eq!(c.castle_type, 5);
        assert_eq!(c.castle_building, 0);
        assert_eq!(out, vec![Message::CastleBuilt { county: 1, castle_type: 5 }]);
    }

    #[test]
    fn a_county_not_building_anything_does_nothing() {
        let mut c = County::new();
        let mut out = Vec::new();
        for _ in 0..10 {
            assert!(!build_tick(&mut c, 1, &mut out));
        }
        assert!(out.is_empty());
        assert_eq!(c.castle_progress, 0);
    }

    #[test]
    fn a_bigger_castle_holds_more_men_and_comes_with_more_archers() {
        assert_eq!(garrison_cap(0), 0);
        assert_eq!(free_archers(0), 0);
        for t in 1..5u8 {
            assert!(garrison_cap(t + 1) >= garrison_cap(t), "castle {t}");
            assert!(free_archers(t + 1) >= free_archers(t), "castle {t}");
        }
        assert_eq!(garrison_cap(5), 600);
        assert_eq!(free_archers(5), 300);
    }

    /// The default starting castle is the Norman keep, and it is the cheapest
    /// castle in wood — which is why it is the one every player-owned county in
    /// the shipped save has.
    #[test]
    fn the_norman_keep_is_the_cheapest_castle_in_wood() {
        assert_eq!(crate::tables::CASTLE_STARTING_TYPE, 3);
        let (wood, _) = castle_cost(3);
        for t in 1..=5u8 {
            assert!(castle_cost(t).0 >= wood, "castle {t}");
        }
    }
}
