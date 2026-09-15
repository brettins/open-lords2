#![allow(unused_imports)]
use super::*;
use super::engine::*;
use crate::county::County;
use crate::kingdom::Kingdom;
use crate::math::pct;
use crate::realm::Realm;
use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;
    use crate::tables::Tables;

    const T: &Tables = &Tables::DEFAULT;

    fn kingdom() -> Kingdom {
        let mut k = Kingdom::new(1);
        k.county_count = 2;
        k.counties[1].owner = 1;
        k.counties[1].population = 1000;
        k.counties[1].grain = 100;
        k.counties[1].herd = 50;
        k.realms[1].gold = 1000;
        k
    }

    #[test]
    fn buying_cattle_into_a_county_with_no_pasture_converts_a_field_in_cursor_order() {
        let mut k = kingdom();
        for slot in 0..4 {
            let tile = crate::map::index(slot as u8, 8);
            k.counties[1].set_field_tile(slot, Some(tile));
            k.campaign.map.terrain[tile] = crate::field::terrain::FALLOW;
        }
        let q = quote(T, Good::Cattle, 100);
        trade(&mut k, Order::buy(Good::Cattle, 1, q, 1, 1)).unwrap();
        let tile = |k: &Kingdom, slot| k.counties[1].field_tile(slot).unwrap();
        assert_eq!(crate::field::classify(k.campaign.map.terrain[tile(&k, 1)]), crate::field::FieldType::Pasture);
        assert_eq!(k.counties[1].pasture_cursor, 1);

        for slot in 0..4 {
            let t = tile(&k, slot);
            k.campaign.map.terrain[t] = crate::field::terrain::GRAIN;
        }
        k.counties[1].crop[1] = 400;
        k.counties[1].fields_grain_standing = 4;
        trade(&mut k, Order::buy(Good::Cattle, 1, q, 1, 1)).unwrap();
        assert_eq!(crate::field::classify(k.campaign.map.terrain[tile(&k, 2)]), crate::field::FieldType::Pasture);
        assert_eq!(k.counties[1].pasture_cursor, 2);
        assert_eq!(k.counties[1].crop[1], 300, "one field's share of the standing crop");
    }

    #[test]
    fn a_merchant_at_full_morale_charges_double_what_he_pays() {
        for good in ALL_GOODS {
            let q = quote(T, good, 100);
            assert_eq!(q.sell, T.good[good.id()].sell_price, "{good:?}");
            if good == Good::Ale {
                assert_eq!(q.markup(), 0, "ale is exempt from the markup");
                assert_eq!(q.buy, q.sell);
            } else if q.sell == 0 {
                assert_eq!(q.buy, 1, "{good:?}");
            } else {
                assert_eq!(q.buy, q.sell * 2, "{good:?}");
            }
        }
    }

    #[test]
    fn the_markup_never_rounds_away_to_nothing() {
        for morale in 0..=100 {
            let q = quote(T, Good::Grain, morale);
            assert!(q.markup() >= 1, "morale {morale} bought grain at cost");
            assert_eq!(quote(T, Good::Ale, morale).markup(), 0, "ale at morale {morale}");
        }
    }

    #[test]
    fn buying_grain_moves_sacks_and_crowns_and_nothing_else() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        let before = k.realms[1].gold;
        let r = trade(&mut k, Order::buy(Good::Grain, 10, q, 1, 1)).unwrap();
        assert_eq!(r.moved, 10);
        assert_eq!(r.crowns, 10 * q.buy);
        assert_eq!(k.counties[1].grain, 110);
        assert_eq!(k.realms[1].gold, before - 10 * q.buy);
    }

    #[test]
    fn an_unaffordable_order_is_refused_whole() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        k.realms[1].gold = 10 * q.buy - 1;
        let before = (k.realms[1].gold, k.counties[1].grain);
        assert_eq!(trade(&mut k, Order::buy(Good::Grain, 10, q, 1, 1)), Err(Refusal::NotEnoughGold));
        assert_eq!((k.realms[1].gold, k.counties[1].grain), before);
        assert!(trade(&mut k, Order::buy(Good::Grain, 9, q, 1, 1)).is_ok());
    }

    #[test]
    fn selling_more_than_the_county_holds_is_refused_whole() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        let before = (k.realms[1].gold, k.counties[1].grain);
        assert_eq!(
            trade(&mut k, Order::sell(Good::Grain, 101, q, 1, 1)),
            Err(Refusal::NotEnoughStock)
        );
        assert_eq!((k.realms[1].gold, k.counties[1].grain), before);
        let r = trade(&mut k, Order::sell(Good::Grain, 100, q, 1, 1)).unwrap();
        assert_eq!(r.crowns, 100 * q.sell);
        assert_eq!(k.counties[1].grain, 0);
    }

    /// The weapons go to the realm and to the slot the group 6 mapping names,
    /// not to the slot their id order suggests.
    #[test]
    fn weapons_land_in_the_armoury_slots_group_six_names() {
        let mut k = kingdom();
        k.realms[1].gold = 100_000;
        for good in [Good::Pikes, Good::Bows, Good::Maces, Good::Crossbows, Good::Swords, Good::Mail]
        {
            let q = quote(T, good, 100);
            trade(&mut k, Order::buy(good, 3, q, 1, 1)).unwrap();
            let slot = good.weapon_slot().unwrap();
            assert_eq!(k.realms[1].weapons[slot], 3, "{good:?} -> weapons[{slot}]");
        }
        assert_eq!(k.realms[1].weapons, [3; WEAPON_TYPE_COUNT]);
    }

    #[test]
    fn sheep_and_wool_can_be_named_and_cannot_be_moved() {
        let mut k = kingdom();
        for good in [Good::Sheep, Good::Wool] {
            assert!(!good.tradeable(), "{good:?}");
            assert_eq!(T.good[good.id()].sell_price, 0, "{good:?} is priced zero");
            let q = quote(T, good, 100);
            let gold = k.realms[1].gold;
            let r = trade(&mut k, Order::buy(good, 5, q, 1, 1)).unwrap();
            assert_eq!(r.moved, 0, "{good:?} went somewhere");
            assert_eq!(r.crowns, 5);
            assert_eq!(k.realms[1].gold, gold - 5);
        }
    }

    #[test]
    fn ale_becomes_happiness_and_is_never_stored() {
        let mut k = kingdom();
        k.counties[1].happiness = 50;
        let q = quote(T, Good::Ale, 100);
        assert_eq!(q.buy, 1, "a barrel is a crown");
        let r = trade(&mut k, Order::buy(Good::Ale, 300, q, 1, 1)).unwrap();
        assert_eq!(r.ale_happiness, 3);
        assert_eq!(r.moved, 0);
        assert_eq!(k.counties[1].happiness, 53);
        assert_eq!(k.realms[1].gold, 1000 - 300);
    }

    #[test]
    fn ale_has_no_sale_limit_at_all() {
        let k = kingdom();
        assert_eq!(min_qty(&k.counties, &k.realms, Good::Ale, 1, 1), 0);
        assert_eq!(min_qty(&k.counties, &k.realms, Good::Grain, 1, 1), -100);
        assert_eq!(min_qty(&k.counties, &k.realms, Good::Cattle, 1, 1), -50);
    }

    #[test]
    fn the_arrows_clamp_to_the_treasury_above_and_the_store_below() {
        let k = kingdom();
        let q = quote(T, Good::Grain, 100);
        assert_eq!(q.buy, 4);
        assert_eq!(max_buy(k.realms[1].gold, q.buy), 250);
        assert_eq!(max_buy(0, q.buy), 0);
        assert_eq!(max_buy(1000, 0), 0, "a free good does not divide by zero");
    }

    #[test]
    fn an_unowned_county_trades_out_of_its_purse_and_is_never_refused() {
        let mut k = kingdom();
        k.counties[1].owner = 0;
        k.counties[1].purse = 5;
        k.counties[1].population = 0;
        let q = quote(T, Good::Grain, 100);
        let r = trade(&mut k, Order::buy(Good::Grain, 100, q, 0, 1)).unwrap();
        assert_eq!(r.moved, 100);
        assert_eq!(k.counties[1].purse, 5 - 100 * q.buy, "the purse goes negative");
        k.counties[1].grain = 0;
        let purse = k.counties[1].purse;
        let r = trade(&mut k, Order::sell(Good::Grain, 10, q, 0, 1)).unwrap();
        assert_eq!(r.moved, -10);
        assert_eq!(k.counties[1].purse, purse + 10 * q.sell);
        // > This line read `assert_eq!(grain, 0)`, with a comment explaining
        // > that *"the tail's `Ration_Apply` then pulled it back to 0 — `sacks =
        // > min(wanted, grain)` is negative against a negative store, so
        // > subtracting it adds. The missing guard is therefore worth free
// > crowns, so nothing
        // > has ever noticed it."* Every step of that was true **of our tree**
        // > and none of it is true of the original: `Ration_Apply`
        // > (`0x0044DF5F`) contains no store `-=` at all, so nothing in the
        // > binary ever pulled the number back. The explanation was reasoning
        // > about a defect of ours as though it were a rule of the game's, and
        // > the test that held it in place was written from the same reading.
        //
        // > `docs/decisions.md` C149; `docs/bugs.md` B11a.
        assert_eq!(k.counties[1].grain, -10, "the store really does go negative");
    }

    #[test]
    fn a_trade_writes_both_accumulators_of_the_pair_it_belongs_to() {
        let mut k = kingdom();
        let q = quote(T, Good::Grain, 100);
        trade(&mut k, Order::buy(Good::Grain, 10, q, 1, 1)).unwrap();
        let r = &k.realms[1];
        assert_eq!((r.trade_spent_a, r.trade_spent_b), (10 * q.buy, 10 * q.buy));
        assert_eq!((r.trade_received_a, r.trade_received_b), (0, 0));

        trade(&mut k, Order::sell(Good::Grain, 4, q, 1, 1)).unwrap();
        let r = &k.realms[1];
        assert_eq!((r.trade_received_a, r.trade_received_b), (4 * q.sell, 4 * q.sell));
        assert_eq!((r.trade_spent_a, r.trade_spent_b), (10 * q.buy, 10 * q.buy));
    }

    #[test]
    fn a_zero_quantity_does_nothing_at_all() {
        let mut k = kingdom();
        let before = k.clone();
        let q = quote(T, Good::Grain, 100);
        assert_eq!(trade(&mut k, Order::buy(Good::Grain, 0, q, 1, 1)), Ok(Receipt::default()));
        assert_eq!(k.counties[1], before.counties[1]);
        assert_eq!(k.realms[1], before.realms[1]);
    }

    #[test]
    fn the_arrows_step_by_one_until_the_repeat_counter_reaches_forty_four() {
        assert_eq!(arrow_step(0), 1);
        assert_eq!(arrow_step(0x2B), 1);
        assert_eq!(arrow_step(0x2C), 10);
        assert_eq!(arrow_step(0x2F), 10);
    }

    /// Every good id round-trips, and the ids are group 6's.
    #[test]
    fn the_good_ids_are_one_to_fourteen_with_no_gap() {
        for (i, good) in ALL_GOODS.iter().enumerate() {
            assert_eq!(good.id(), i + 1);
            assert_eq!(Good::from_id(good.id()), Some(*good));
        }
        assert_eq!(Good::from_id(0), None);
        assert_eq!(Good::from_id(15), None);
        assert_eq!(ALL_GOODS.len() + 1, GOOD_COUNT);
    }
}

