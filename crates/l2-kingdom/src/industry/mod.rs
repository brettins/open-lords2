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
//! [`crate::tables::JOB_COUNT`] —
//! [`crate::tables::INDUSTRY_ORDER`], neither of the two orders the document
//! gives.)
//!
//! `output = min(resourceLimit, Pct(workers / divisor, efficiency))`. Both of
//! the terms `docs/kingdom.md` §7.4 leaves unexplained are now traced:
//!
//! * **`efficiency` is not the base, it ramps.** [`efficiency_ramp`] adds the
//! base to *last season's* efficiency every season, capped at 100 —
//!   mine starts at 15% and reaches full output in seven seasons. And with
//!   *Advanced Farming* off the whole mechanism is bypassed for a flat 80%,
//! which means the England turn-one fixture's settings put every industry at 80% and the
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

mod production;
pub use production::*;
mod wages;
pub use wages::*;
mod castles;
pub use castles::*;
mod ui;
pub use ui::*;

use crate::county::County;
use crate::math::pct;
use crate::realm::Realm;
use crate::report::Message;
use crate::tables::{Commodity, Tables, RESOURCE_LIMIT_UNLIMITED, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

/// The two denominators `FUN_0044F15B` (`0x0044F15B`) computes before every
/// weapons `resourceLimit`,
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
/// **They are summed *costs*** — so
/// [`WeaponShare::SINGLE_SMITH`] is not the right default and this crate's old
/// `weapon_share: 1` was not either. A lone smithy forging crossbows at 6 wood
/// and 10 iron divides by 6 and by 10, so its limit is `realm.wood / 6` and
/// `realm.iron / 10` — the number of crossbows the stockpile can pay
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

/// Why the castle chooser's OK button refused — `FUN_00436B59`'s two guards,
/// and they are the **only** two.
///
/// There is deliberately no *"you cannot afford it"*: see [`order_castle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleRefusal {
    /// Not a castle type at all.
    NoSuchType,
    /// The type picked is the one already standing. Message `0x93` = `L2.eng`
    /// **147**.
    AlreadyBuilt,
    /// The type picked is **smaller** than the one standing. Message `0x122` =
    /// `L2.eng` **290**. You may never knock a castle down to build a lesser
    /// one.
    Downgrade,
}

/// What finishing a castle asks the caller to do — the two things
/// `Castle_BuildTick` does that need the unit array, which this module cannot
/// reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastleComplete {
    /// Archers the castle comes with, already netted against the castle it
    /// replaced and already refused if the county is too small to spare them.
    /// Zero for a repair and for an upgrade that gains nothing.
    pub free_archers: i32,
/// True when this was a **repair** after a siege —
/// message `0xA3` variant 1.
    pub repaired: bool,
}

/// What a click on a building on the campaign map toggles.
///
/// `Map_Click` picks this from a ladder on the clicked tile's graphic index,
///
/// 13…20 nothing at all, 21 and above the castle.** `[D]` — `Map_Click`'s own
/// `if`/`else if` chain, and it is the only way to reach
/// [`toggle_from_map`]: **nothing on any county panel switches an industry**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MapToggle {
    /// One of the four commodities: its enable byte, county `+0x297 + c*0x18`.
    Industry(Commodity),
    /// County `+0x1B0` — the switch a player throws by dragging builders onto
    /// the castle,
    /// building. Until now this crate had no such field and treated it as
    /// permanently thrown ([`crate::labour::ceilings`]).
    Castle,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful. The switched-off answers live in `tests/quirks.rs`.
    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

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

    /// **With Advanced Farming off every industry is a flat 80%**,
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

/// **The real denominator is a sum of costs.**
    /// `FUN_0044F15B`,
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

    /// A crossbow costs 6 wood and 10 iron,
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

    /// Every stage carries the message id it raises,
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

    /// Paying at any point resets the escalation,
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

    fn owned(pop: i32) -> County {
        let mut c = County::new();
        c.owner = 1;
        c.population = pop;
        c
    }

    /// **Ordering a castle staffs it** — `Castle_Order` (`0x00436D02`) calls
    /// `Labour_ToggleIndustryShare(county, 3, 1)`, so the job leaves the order
    /// with a share and the five-member group still sums to 100. It had a
    /// share of 0 at any industry split before.
    #[test]
    fn ordering_a_castle_puts_builders_on_it() {
        use crate::tables::JOB_CASTLE_BUILDING;
        let industry = |c: &County| c.labour_share[JOB_CASTLE_BUILDING..].iter().sum::<i32>();

        let mut c = owned(2_000);
        c.industry_share = 100;
        let mut r = Realm::new();
        r.wood = 10_000;
        r.stone = 10_000;
        assert_eq!(c.labour_share[JOB_CASTLE_BUILDING], 0, "nobody builds before the order");

        assert!(order_castle(T, &mut c, &mut r, 3));
        assert!(c.labour_share[JOB_CASTLE_BUILDING] > 0, "the order staffs the castle job");
        assert_eq!(industry(&c), 100, "and the industry group still sums to 100");

        // `Castle_BuildTick`'s completion branch takes them off again.
        let mut out = Vec::new();
        c.castle_work_left = 0;
        assert!(build_tick(T, &mut c, &mut r, 0, &mut out).is_some());
        assert_eq!(c.labour_share[JOB_CASTLE_BUILDING], 0, "and the topped-out castle frees them");
        assert_eq!(industry(&c), 100);
    }

    /// **A castle you cannot afford is ordered anyway** — `Castle_Order` has no
    /// affordability guard at all. What an empty store buys you is a debt and a
    /// labour ceiling of zero.
    #[test]
    fn a_castle_can_be_ordered_with_nothing_in_the_store() {
        let mut c = owned(2_000);
        let mut r = Realm::new();
        r.wood = 199;
        r.stone = 0;
        assert!(order_castle(T, &mut c, &mut r, 3), "a keep is ordered on credit");
        assert_eq!(c.castle_type, 3, "and castleType moves at once");
        assert_eq!(c.castle_degraded, crate::siege::CASTLE_DEGRADED_BUILDING);
        assert_eq!((r.wood, r.stone), (0, 0), "everything there was is taken");
        assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (1, 1_000), "the rest is owed");
        assert_eq!(castle_materials_percent(&c) < 100, true);
        assert_eq!(castle_labour_estimate(T, &c).1, 0, "and nobody may work on it");

        // The stone arrives over two seasons; the ceiling opens the moment the
        // last stick of it does.
        r.stone = 400;
        r.wood = 50;
        deliver_castle_materials(&mut c, &mut r);
        assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 600));
        assert_eq!(castle_labour_estimate(T, &c).1, 0, "still short of stone");
        r.stone = 600;
        deliver_castle_materials(&mut c, &mut r);
        assert_eq!(castle_labour_estimate(T, &c).1, 800, "the keep's whole workforce");
    }

    /// The two refusals the chooser's OK button has, and they are the only two.
    #[test]
    fn a_castle_may_never_be_replaced_by_a_smaller_one() {
        let mut c = owned(2_000);
        c.castle_type = 3;
        assert_eq!(castle_refusal(T, &c, 3), Some(CastleRefusal::AlreadyBuilt));
        assert_eq!(castle_refusal(T, &c, 2), Some(CastleRefusal::Downgrade));
        assert_eq!(castle_refusal(T, &c, 1), Some(CastleRefusal::Downgrade));
        assert_eq!(castle_refusal(T, &c, 4), None);
        assert_eq!(castle_refusal(T, &c, 6), Some(CastleRefusal::NoSuchType));
        let mut r = Realm::new();
        assert!(!order_castle(T, &mut c, &mut r, 2));
        assert_eq!(c.castle_degraded, 0, "a refusal changes nothing");
    }

    /// **An upgrade pays the difference, and a cheaper column pays you back.**
    /// A motte and bailey is 800 wood and 80 stone; a Norman keep is 200 and
    /// 1,000. So the upgrade costs 920 stone and *refunds* 600 wood.
    #[test]
    fn upgrading_pays_the_difference_and_refunds_what_the_new_castle_needs_less_of() {
        let mut c = owned(2_000);
        c.castle_type = 2;
        let mut r = Realm::new();
        r.wood = 0;
        r.stone = 10_000;
        assert!(order_castle(T, &mut c, &mut r, 3));
        assert_eq!(c.castle_building, 2, "the motte and bailey is what still stands");
        assert_eq!(c.castle_type, 3);
        assert_eq!(r.wood, 600, "600 wood came back out of the old castle");
        assert_eq!(r.stone, 10_000 - 920);
        assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 0));
    }

    #[test]
    fn a_royal_castle_takes_several_seasons_of_workers() {
        let mut c = owned(2_000);
        let mut r = Realm::new();
        r.wood = 10_000;
        r.stone = 10_000;
        assert!(order_castle(T, &mut c, &mut r, 5));
        c.labour[T.job.castle_building] = 500;

        let mut out = Vec::new();
        for season in 1..5 {
            assert!(build_tick(T, &mut c, &mut r, 1, &mut out).is_none(), "season {season}");
            assert!(c.castle_percent < 100, "and it says so: {}", c.castle_percent);
        }
        let done = build_tick(T, &mut c, &mut r, 1, &mut out).expect("2500 at 500 a season");
        assert_eq!(c.castle_type, 5);
        assert_eq!(c.castle_degraded, 0);
        assert_eq!(c.castle_percent, 100);
        assert_eq!(done.free_archers, 300, "a royal castle's own garrison");
        assert!(!done.repaired);
        assert_eq!(out, vec![Message::CastleBuilt { county: 1, castle_type: 5 }]);
    }

    /// **The rounding may never finish a castle.** 2,500 of work at 2,499 done
    /// is 99.96%, and `PctOf` would call that 100.
    #[test]
    fn a_castle_one_man_short_is_ninety_nine_percent_and_not_a_hundred() {
        let mut c = owned(2_000);
        let mut r = Realm::new();
        r.wood = 10_000;
        r.stone = 10_000;
        assert!(order_castle(T, &mut c, &mut r, 5));
        c.labour[T.job.castle_building] = 2_499;
        let mut out = Vec::new();
        assert!(build_tick(T, &mut c, &mut r, 1, &mut out).is_none());
        assert_eq!(c.castle_work_left, 1);
        assert_eq!(c.castle_percent, 99, "clamped, not rounded up");
        assert_eq!(c.castle_degraded, crate::siege::CASTLE_DEGRADED_BUILDING);
    }

    /// An upgrade's free garrison is the **difference**, and a county too small
    /// to spare the men gets none.
    #[test]
    fn the_free_garrison_is_what_the_upgrade_adds_and_a_small_county_gets_none() {
        let mut c = owned(2_000);
        c.castle_type = 3; // a keep, 150 archers
        c.castle_building = 2; // over a motte and bailey, also 150
        assert_eq!(free_garrison_archers(T, &c), 0, "an upgrade that adds nothing");
        c.castle_type = 4; // a stone castle, 200
        assert_eq!(free_garrison_archers(T, &c), 50);
        c.castle_building = 0;
        assert_eq!(free_garrison_archers(T, &c), 200, "a new castle brings all of them");
        c.population = 399;
        assert_eq!(free_garrison_archers(T, &c), 0, "more than half the county");
    }

    #[test]
    fn a_county_not_building_anything_does_nothing() {
        let mut c = owned(2_000);
        let mut r = Realm::new();
        let mut out = Vec::new();
        for _ in 0..10 {
            assert!(build_tick(T, &mut c, &mut r, 1, &mut out).is_none());
        }
        assert!(out.is_empty());
        assert_eq!(c.castle_work_left, 0);
        assert_eq!(c.castle_percent, 0);
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
/// castle in wood — so it is the one every player-owned county in
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

