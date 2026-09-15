#![allow(unused_imports)]
use super::*;
use super::basket::*;
use super::raise::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use crate::unit::{ArmyNames, TroopType, Unit, UnitKind, Units, TROOP_TYPES};

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


    #[test]
    fn the_levy_takes_a_percentage_of_the_people_and_the_table_prices_it() {
        let (counties, _) = world();
        let l = set_percent(T, &counties[1], 10);
        assert_eq!(l.men, 50, "a tenth of five hundred");
        assert_eq!(l.happiness_cost, 5, "the table's index 10");
        assert_eq!(l.settled, 10);
    }

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
        assert!(counties[1].happiness - T.army_happiness_cost(60) < 1);
    }

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

    #[test]
    fn the_surcharge_is_added_before_the_cost_is_clamped_at_a_hundred() {
        let (mut counties, _) = world();
        counties[1].levy_surcharge = crate::tables::LEVY_SURCHARGE;
        let l = set_percent(T, &counties[1], 20);
        assert_eq!(l.happiness_cost, 25);
        assert_eq!(l.settled, 20, "still affordable at happiness 100");

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

    #[test]
    fn auto_equip_skips_an_empty_rack_and_keeps_filling_the_others() {
        let (_, mut realms) = world();
        realms[1].weapons = [25, 0, 0, 0, 0, 100];
        let mut b = LevyBasket::seed(&realms[1], 200);
        b.auto_equip();
        let troops = b.troops();
        assert_eq!(troops[TroopType::Crossbowman.index()], 20, "two batches, then 5 left");
        assert!(troops[TroopType::Knight.index()] >= 100, "the armour kept being handed out");
        assert_eq!(troops.iter().sum::<i32>(), 200, "nobody is lost");
    }

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


    #[test]
    fn an_army_of_fewer_than_fifty_is_refused_unless_mercenaries_supply_the_men() {
        assert_eq!(refuse_levy(0, false), Some(LevyRefusal::NoMen));
        assert_eq!(refuse_levy(49, false), Some(LevyRefusal::TooFew));
        assert_eq!(refuse_levy(50, false), None);
        assert_eq!(refuse_levy(0, true), None, "a band supplies the men");
        assert_eq!(refuse_levy(49, true), None);
    }


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
            &mut crate::explore::Explored::new(),
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
            &mut crate::explore::Explored::new(),
        )
        .unwrap();
        assert_eq!(units.get(id).unwrap().morale, 100, "not the 60 it is left at");
        assert_eq!(counties[1].happiness, 60);
    }

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
            &mut crate::explore::Explored::new(),
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
        assert_eq!(muster_tile(&m, &units, (20, 20)), Some((20, 20)));

        let mut units = Units::new();
        units.spawn(Unit::new(UnitKind::Army, 1, 20, 20));
        assert_eq!(muster_tile(&m, &units, (20, 20)), Some((19, 19)));
    }

    /// **C47.** The finders search a box around the county's *anchor*, radius 1
    /// then 2 then 3, and stop. A road tile four tiles away is out of reach,
    #[test]
    fn the_muster_never_reaches_further_than_three_tiles_from_the_anchor() {
        let mut m = open_map();
        m.set_flags(0, 0, flags::ROAD);
        m.set_flags(24, 20, flags::ROAD); // four east of the anchor
        let units = Units::new();
        let at = muster_tile(&m, &units, (20, 20)).expect("open ground beside the anchor");
        assert_eq!(at, (19, 19), "neither road is within three of (20, 20)");

        m.set_flags(23, 20, flags::ROAD);
        assert_eq!(muster_tile(&m, &units, (20, 20)), Some((23, 20)));
    }

    #[test]
    fn farmland_is_not_open_ground_for_a_muster() {
        let mut m = open_map();
        for y in 16..25u8 {
            for x in 16..25u8 {
                m.set_flags(x, y, flags::FARMLAND);
            }
        }
        let units = Units::new();
        assert_eq!(muster_tile(&m, &units, (20, 20)), None);
    }

    #[test]
    fn a_county_with_nowhere_to_stand_refuses_the_army() {
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
                &mut crate::explore::Explored::new(),
            ),
            Err(LevyRefusal::NowhereToStand)
        );
    }

    #[test]
    fn a_neutral_countys_defence_is_ownerless_rather_than_owned_by_realm_zero() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        counties[2].owner = 0;
        counties[2].population = 500;
        counties[2].happiness = 77;
        for i in 0..MAP_TILES {
            if i % 64 > 40 {
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
            &mut crate::explore::Explored::new(),
        )
        .expect("five hundred people can defend themselves");
        let u = units.get(id).unwrap();
        assert_eq!(u.owner, OWNERLESS);
        assert_eq!(u.shield, 0);
        assert_eq!(u.men, 125, "a quarter of five hundred");
        assert_eq!(u.troops[TroopType::Archer.index()], 60);
        assert_eq!(u.troops[TroopType::Peasant.index()], 65);
        assert_eq!(counties[2].happiness, 77, "a militia costs the county nothing");
    }

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
            T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, 40, Defence::AiCounty, 1268,
            &mut crate::explore::Explored::new(),
        )
        .is_none());
        assert_eq!(units.len(), 0);
    }

    #[test]
    fn a_human_countys_defence_is_all_peasants_however_full_the_armoury_is() {
        let m = open_map();
        let (mut counties, mut realms) = world();
        realms[1].weapons = [500; WEAPON_TYPE_COUNT];
        let mut units = Units::new();
        let mut names = ArmyNames::new();

        let id = raise_defence(
            T, &m, &mut counties, &mut realms, &mut units, &mut names, 1, DEFENCE_PERCENT,
            Defence::HumanCounty, 1268, &mut crate::explore::Explored::new(),
        )
        .unwrap();
        let u = units.get(id).unwrap();
        assert_eq!(u.men, 200);
        assert_eq!(u.troops, [200, 0, 0, 0, 0, 0, 0]);
        assert_eq!(realms[1].weapons, [500; WEAPON_TYPE_COUNT], "nothing was issued");
    }

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
            Defence::AiCounty, 1268, &mut crate::explore::Explored::new(),
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

