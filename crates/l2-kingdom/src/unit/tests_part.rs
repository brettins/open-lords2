#![allow(unused_imports)]
use super::*;

use army::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Tables;

    const T: &Tables = &Tables::DEFAULT;

    fn army(owner: u8, men: i32) -> Unit {
        let mut u = Unit::new(UnitKind::Army, owner, 10, 10);
        u.men = men;
        u.troops[TroopType::Peasant.index()] = men;
        u
    }

    #[test]
    fn slot_zero_is_never_a_unit_and_the_array_holds_a_hundred_and_fifty() {
        let mut units = Units::new();
        assert_eq!(units.free_slot(), Some(1), "the first free slot is 1, not 0");
        for i in 1..=MAX_UNIT_ID {
            assert_eq!(units.spawn(army(1, 50)), Some(i));
        }
        assert_eq!(units.free_slot(), None);
        assert_eq!(units.spawn(army(1, 50)), None, "the 151st is refused, not grown");
        assert_eq!(units.len(), MAX_UNIT_ID);
    }

    #[test]
    fn a_freed_slot_is_reused_by_the_next_spawn() {
        let mut units = Units::new();
        let a = units.spawn(army(1, 50)).unwrap();
        let b = units.spawn(army(1, 60)).unwrap();
        units.remove(a);
        assert_eq!(units.spawn(army(2, 70)), Some(a), "the lowest free slot wins");
        assert!(units.get(b).is_some());
    }

    #[test]
    fn an_army_gets_fifteen_moves_and_everything_else_gets_ten() {
        assert_eq!(UnitKind::Army.move_allowance(), 15);
        for k in [UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
            assert_eq!(k.move_allowance(), 10, "{}", k.name());
        }
        for b in 1..=4u8 {
            assert_eq!(UnitKind::from_byte(b).map(UnitKind::byte), Some(b));
        }
        assert_eq!(UnitKind::from_byte(0), None);
        assert_eq!(UnitKind::from_byte(5), None, "the NULL tick handler is unreachable");
    }

    #[test]
    fn only_armies_and_mobs_are_combatants() {
        assert!(UnitKind::Army.is_combatant());
        assert!(UnitKind::PeasantMob.is_combatant());
        assert!(!UnitKind::Merchant.is_combatant());
        assert!(!UnitKind::Transport.is_combatant());
    }

    #[test]
    fn a_knight_is_a_man_in_armour_and_a_peasant_carries_nothing() {
        assert_eq!(TroopType::Peasant.weapon_slot(), None);
        for t in ALL_TROOP_TYPES.iter().skip(1) {
            let slot = t.weapon_slot().expect("every equipped type has a weapon");
            assert!(slot < crate::tables::WEAPON_TYPE_COUNT);
        }
        assert_eq!(TroopType::Knight.weapon_slot(), Some(5));
        assert_eq!(crate::tables::WEAPON_NAMES[5], "Armour");
        assert_eq!(TroopType::Crossbowman.weapon_slot(), Some(0));
        assert_eq!(crate::tables::WEAPON_NAMES[0], "Crossbow");
    }

    #[test]
    fn an_empty_army_scores_one_and_a_single_peasant_scores_twenty_two() {
        let empty = Unit::new(UnitKind::Army, 1, 0, 0);
        assert_eq!(empty.strength_score(), 1);

        let mut one = Unit::new(UnitKind::Army, 1, 0, 0);
        one.troops[TroopType::Peasant.index()] = 1;
        one.men = 1;
        assert_eq!(one.strength_score(), 2 + STRENGTH_SCORE_BONUS);
    }

    #[test]
    fn the_strength_score_weights_each_type_and_adds_the_band() {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.troops[TroopType::Knight.index()] = 10;
        u.men = 10;
        assert_eq!(u.strength_score(), 10 * 22 + STRENGTH_SCORE_BONUS);

        u.mercenaries = Some(Mercenaries { band: 11, troop: TroopType::Knight, men: 50 });
        u.men += 50;
        assert_eq!(u.strength_score(), (10 + 50) * 22 + STRENGTH_SCORE_BONUS);
    }

    #[test]
    fn desertion_skips_any_troop_type_of_ten_or_fewer() {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.troops = [100, 11, 10, 9, 0, 250, 1];
        u.men = u.troops.iter().sum();
        let before = u.men;

        let lost = u.desert();
        assert_eq!(u.troops, [90, 10, 10, 9, 0, 225, 1]);
        assert_eq!(lost, 10 + 1 + 25);
        assert_eq!(u.men, before - lost);

        let mut small = Unit::new(UnitKind::Army, 1, 0, 0);
        small.troops = [10; TROOP_TYPES];
        small.men = 70;
        assert_eq!(small.desert(), 0);
        assert_eq!(small.men, 70, "seventy men in sevens never desert at all");
    }

    #[test]
    fn the_sprite_class_breaks_at_three_hundred_and_one_and_six_hundred_and_one() {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        for (men, class) in [(1, 0), (300, 0), (301, 1), (600, 1), (601, 2), (1500, 2)] {
            u.men = men;
            assert_eq!(u.size_class(), class, "{men} men");
        }
    }


    #[test]
    fn two_armies_totalling_fifteen_hundred_merge_and_fifteen_hundred_and_one_does_not() {
        let mut units = Units::new();
        let a = units.spawn(army(1, 750)).unwrap();
        let b = units.spawn(army(1, 750)).unwrap();
        assert_eq!(combine(&mut units, a, b), Ok(1500));
        assert!(units.get(b).is_none(), "the absorbed army is gone");
        assert_eq!(units.get(a).unwrap().men, 1500);

        let mut units = Units::new();
        let a = units.spawn(army(1, 751)).unwrap();
        let b = units.spawn(army(1, 750)).unwrap();
        assert_eq!(combine(&mut units, a, b), Err(CombineRefusal::TooMany));
        assert!(units.get(b).is_some(), "a refused merge leaves both armies");
    }

    /// `L2.eng` 167: *"The mercenaries in these armies will not fight
    /// together."*
    #[test]
    fn two_armies_both_carrying_mercenaries_refuse_to_merge() {
        let band = Mercenaries { band: 9, troop: TroopType::Maceman, men: 150 };
        let mut units = Units::new();
        let mut one = army(1, 200);
        one.mercenaries = Some(band);
        let mut two = army(1, 200);
        two.mercenaries = Some(band);
        let a = units.spawn(one).unwrap();
        let b = units.spawn(two).unwrap();
        assert_eq!(combine(&mut units, a, b), Err(CombineRefusal::TwoMercenaryBands));

        units.get_mut(b).unwrap().mercenaries = None;
        assert!(combine(&mut units, a, b).is_ok());
        assert_eq!(units.get(a).unwrap().mercenaries, Some(band));
    }

    #[test]
    fn a_merge_takes_the_higher_of_the_two_move_counts_in_both_directions() {
        for (a_used, b_used) in [(12, 2), (2, 12)] {
            let mut units = Units::new();
            let mut one = army(1, 100);
            one.moves_used = a_used;
            let mut two = army(1, 100);
            two.moves_used = b_used;
            let a = units.spawn(one).unwrap();
            let b = units.spawn(two).unwrap();
            assert!(combine(&mut units, a, b).is_ok());
            assert_eq!(
                units.get(a).unwrap().moves_used,
                12,
                "merging {a_used} and {b_used} must leave the army spent"
            );
        }
    }

    #[test]
    fn a_merge_sums_the_seven_troop_counts() {
        let mut units = Units::new();
        let mut one = Unit::new(UnitKind::Army, 1, 0, 0);
        one.troops = [10, 20, 30, 40, 50, 60, 70];
        one.men = one.troops.iter().sum();
        let mut two = Unit::new(UnitKind::Army, 1, 0, 0);
        two.troops = [1, 2, 3, 4, 5, 6, 7];
        two.men = two.troops.iter().sum();
        let a = units.spawn(one).unwrap();
        let b = units.spawn(two).unwrap();
        assert!(combine(&mut units, a, b).is_ok());
        assert_eq!(units.get(a).unwrap().troops, [11, 22, 33, 44, 55, 66, 77]);
        assert_eq!(units.get(a).unwrap().men, units.get(a).unwrap().troop_total());
    }


    #[test]
    fn a_realm_uses_every_name_before_repeating_one() {
        let mut names = ArmyNames::new();
        let mut seen = Vec::new();
        for _ in 0..ARMY_NAME_SLOTS {
            seen.push(names.pick(1));
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), ARMY_NAME_SLOTS, "all twenty-four before any repeat");
        assert_eq!(names.pick(1), 0, "and then round again from the first");
    }

    #[test]
    fn a_destroyed_army_gives_back_only_half_of_what_its_name_cost() {
        let mut names = ArmyNames::new();
        let n = names.pick(1);
        assert_eq!(names.counters(1)[n as usize], 2);
        names.release(1, n);
        assert_eq!(names.counters(1)[n as usize], 1, "not back to zero");
        assert_ne!(names.pick(1), n);
    }

    #[test]
    fn a_realm_outside_one_to_five_never_touches_a_counter() {
        let mut names = ArmyNames::new();
        assert_eq!(names.pick(0), 0);
        assert_eq!(names.pick(9), 0);
        assert_eq!(names.counters(0), &[0; ARMY_NAME_SLOTS]);
    }


    fn kingdom_bits() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let counties = core::array::from_fn(|_| County::new());
        let realms = core::array::from_fn(|_| Realm::new());
        (counties, realms)
    }

    #[test]
    fn the_troop_recount_splits_friendly_from_enemy_by_the_countys_owner() {
        let (mut counties, mut realms) = kingdom_bits();
        counties[1].owner = 1;
        counties[2].owner = 2;
        realms[1].in_play = true;
        realms[2].in_play = true;

        let mut units = Units::new();
        let mut mine = army(1, 300);
        mine.county = 1;
        let mut invading = army(1, 200);
        invading.county = 2;
        units.spawn(mine);
        units.spawn(invading);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!((counties[1].friendly_troops, counties[1].enemy_troops), (300, 0));
        assert_eq!((counties[2].friendly_troops, counties[2].enemy_troops), (0, 200));
    }

    /// An ally's army eats as a friendly. Realm `+0x81` read as an alliance
    /// flag is `[I]` — this is the only rule that reads it that way.
    #[test]
    fn an_allys_army_counts_as_friendly() {
        let (mut counties, mut realms) = kingdom_bits();
        counties[1].owner = 2;
        realms[1].ally = 2;

        let mut units = Units::new();
        let mut u = army(1, 400);
        u.county = 1;
        units.spawn(u);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!(counties[1].friendly_troops, 400);
        assert_eq!(counties[1].enemy_troops, 0);
    }

    #[test]
    fn a_garrison_is_excluded_from_the_recount_but_a_besieger_is_not() {
        let (mut counties, realms) = kingdom_bits();
        counties[1].owner = 1;

        let mut units = Units::new();
        let mut inside = army(1, 500);
        inside.county = 1;
        inside.garrison_county = 1;
        let mut outside = army(2, 300);
        outside.county = 1;
        outside.besieging_county = 1;
        units.spawn(inside);
        units.spawn(outside);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!(counties[1].friendly_troops, 0, "the garrison eats out of the castle");
        assert_eq!(counties[1].enemy_troops, 300, "the besieger forages the county");
    }

    #[test]
    fn revolting_peasants_are_counted_and_merchants_are_not() {
        let (mut counties, realms) = kingdom_bits();
        counties[1].owner = 1;

        let mut units = Units::new();
        let mut mob = Unit::new(UnitKind::PeasantMob, 6, 5, 5);
        mob.county = 1;
        mob.men = 90;
        let mut trader = Unit::new(UnitKind::Merchant, 1, 6, 6);
        trader.county = 1;
        trader.men = 1000;
        units.spawn(mob);
        units.spawn(trader);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!(counties[1].enemy_troops, 90);
        assert_eq!(counties[1].friendly_troops, 0, "a merchant is not troops");
    }

    #[test]
    fn a_recount_clears_the_counts_it_is_about_to_rebuild() {
        let (mut counties, realms) = kingdom_bits();
        counties[3].friendly_troops = 999;
        counties[3].enemy_troops = 999;
        Units::new().recount_county_troops(&mut counties, &realms);
        assert_eq!((counties[3].friendly_troops, counties[3].enemy_troops), (0, 0));
    }

    #[test]
    fn the_wage_bill_covers_armies_only_and_charges_a_quarter_a_man() {
        let (_, mut realms) = kingdom_bits();
        realms[1].is_human = true;
        realms[1].in_play = true;

        let mut units = Units::new();
        units.spawn(army(1, 250));
        units.spawn(army(1, 252));
        let mut mob = Unit::new(UnitKind::PeasantMob, 1, 3, 3);
        mob.men = 1000;
        units.spawn(mob);
        units.spawn(army(2, 400));

        assert_eq!(wages_for_realm(T, &units, &realms, 1, 0), 62 + 63);
        let bill = refresh_wages(T, &mut units, &mut realms, 1, 0);
        assert_eq!(bill, 62 + 63);
        assert_eq!(realms[1].wages, bill);
        assert_eq!(units.get(1).unwrap().wages, 62);
        assert_eq!(units.get(2).unwrap().wages, 63);
        assert_eq!(units.get(3).unwrap().wages, 0, "a mob is not paid");
    }

    #[test]
    fn a_garrisoned_army_is_still_paid() {
        let (_, mut realms) = kingdom_bits();
        realms[1].is_human = true;
        let mut units = Units::new();
        let mut inside = army(1, 400);
        inside.garrison_county = 3;
        units.spawn(inside);
        assert_eq!(refresh_wages(T, &mut units, &mut realms, 1, 0), 100);
    }

    #[test]
    fn resetting_moves_touches_every_slot_regardless_of_type() {
        let mut units = Units::new();
        for kind in [UnitKind::Army, UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
            let mut u = Unit::new(kind, 1, 0, 0);
            u.moves_used = 9;
            u.moving = true;
            units.spawn(u);
        }
        units.reset_moves();
        for (_, u) in units.iter() {
            assert_eq!(u.moves_used, 0, "{}", u.kind.name());
            assert!(!u.moving);
        }
    }

    #[test]
    fn destroying_an_army_frees_the_slot_and_rebuilds_the_wage_bill() {
        let (_, mut realms) = kingdom_bits();
        realms[1].is_human = true;
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = units.spawn(army(1, 400)).unwrap();
        let b = units.spawn(army(1, 800)).unwrap();
        units.get_mut(a).unwrap().name_index = names.pick(1);
        refresh_wages(T, &mut units, &mut realms, 1, 0);
        assert_eq!(realms[1].wages, 100 + 200);

        let gone = destroy(T, &mut units, &mut realms, &mut names, a, 0).unwrap();
        assert_eq!(gone.men, 400);
        assert!(units.get(a).is_none());
        assert_eq!(realms[1].wages, 200, "the bill is recomputed from what is left");
        assert!(units.get(b).is_some());
    }

    #[test]
    fn destroying_a_besieger_clears_the_garrisons_back_pointer() {
        let (_, mut realms) = kingdom_bits();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let garrison = units.spawn(army(1, 200)).unwrap();
        let besieger = units.spawn(army(2, 400)).unwrap();
        units.get_mut(garrison).unwrap().garrison_county = 4;
        units.get_mut(besieger).unwrap().besieging_county = 4;
        units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

        destroy(T, &mut units, &mut realms, &mut names, besieger, 0);
        assert_eq!(units.get(garrison).unwrap().besieged_by, 0);

        let besieger = units.spawn(army(2, 400)).unwrap();
        units.get_mut(besieger).unwrap().besieging_county = 4;
        destroy(T, &mut units, &mut realms, &mut names, garrison, 0);
        assert_eq!(units.get(besieger).unwrap().besieging_county, 0);
    }

    #[test]
    fn realm_totals_count_armies_and_their_men() {
        let mut units = Units::new();
        units.spawn(army(1, 100));
        units.spawn(army(1, 250));
        units.spawn(army(2, 900));
        let mut mob = Unit::new(UnitKind::PeasantMob, 1, 0, 0);
        mob.men = 40;
        units.spawn(mob);
        assert_eq!(units.realm_totals(1), (2, 350));
        assert_eq!(units.realm_totals(2), (1, 900));
        assert_eq!(units.realm_totals(3), (0, 0));
    }

    #[test]
    fn a_unit_is_found_by_the_tile_it_stands_on() {
        let mut units = Units::new();
        let a = units.spawn(army(1, 100)).unwrap();
        assert_eq!(units.at(10, 10), Some(a));
        assert_eq!(units.at(11, 10), None);
    }
}


