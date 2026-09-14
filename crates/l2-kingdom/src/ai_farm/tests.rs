use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::terrain;
    use crate::map::CampaignMap;

    const T: &Tables = &Tables::DEFAULT;

    fn env() -> FarmEnv {
        FarmEnv::new(Season::Winter, Season::Spring)
    }

    /// Five realms, all in play, all AI. The counties below are given to realm
    /// 2 wherever the owner matters; the rest of the array exists because
    /// [`lay_out`] looks its county's owner up in it.
    fn realms() -> Vec<Realm> {
        let mut realms = vec![Realm::new(); crate::realm::MAX_REALMS];
        for (i, r) in realms.iter_mut().enumerate().skip(1) {
            r.in_play = true;
            r.lord = ((i - 1) % crate::tables::AI_PERSONALITY_COUNT + 1) as u8;
        }
        realms
    }

    /// [`lay_out`] over a **one-county world**, which is what every style test
    /// below wants: the county lands at index 1 of a two-slot array so the
    /// weapon share has a realm to sum over, and is copied back out.
    ///
    /// Passing the county alone is exactly what the signature no longer allows,
/// and for a reason worth restating here:
    /// the blacksmith's ceiling is a share of the realm's stockpile divided
    /// across its staffed smithies,
    fn lay(style: FarmStyle, c: &mut County, map: &mut CampaignMap, e: &FarmEnv) {
        let mut counties = vec![County::new(), c.clone()];
        lay_out(T, style, &mut counties, 1, 1, map, &realms(), &mut NoMarket, e);
        *c = counties.remove(1);
    }

    /// A county with `n` field tiles laid along one row, all fallow.
    fn county_with(n: usize) -> (County, CampaignMap) {
        let mut c = County::new();
        let mut map = CampaignMap::empty();
        for i in 0..n {
            let tile = crate::map::index(i as u8, 8);
            c.set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::FALLOW;
        }
        c.population = 500;
        crate::field::recount(&mut c, &map);
        (c, map)
    }

    /// **An AI realm that never plants is the original's rule, not a gap in
    /// ours** — `Ai_FarmStyleGrazing` (`0x004A42E3`), whose first act is to
    /// clear every grain field, in every season.
    ///
    /// `Ai_ManageCountyFarms` (`0x0049DD01`) copies the lord's style byte into
    /// county `+0x1FE` and dispatches on it,
    /// `[1, 1, 0, 9]` — **two of the four graze**, so an AI realm behind either
    /// sows nothing all game.
    ///
    /// `a_grazing_lord_clears_grain_in_every_season` already pins the clearing.
    /// This pins **which lords it happens to**, which is the half that makes an
    /// AI realm look broken: ablation, carried inside — lord 3's style is 0 and
    /// the same county sows.
    #[test]
    fn two_of_the_four_lords_graze_so_their_realms_never_sow() {
        assert_eq!(crate::tables::AI_PERSONALITY_FARM_STYLE, [1, 1, 0, 9]);
        for lord in [1u8, 2] {
            assert_eq!(T.ai_farm_style(lord), Some(1), "lord {lord}");
            assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
        }
        assert_eq!(T.ai_farm_style(3), Some(0));
        assert_eq!(T.ai_farm_style(4), Some(9));

        let sown = |style| {
            let (mut c, mut map) = county_with(8);
            let e = FarmEnv::new(Season::Winter, Season::Spring);
            lay(style, &mut c, &mut map, &e);
            counts(&c, &map).0
        };
        assert_eq!(sown(FarmStyle::RealmGrazing), 0, "lords 1 and 2");
        assert!(sown(FarmStyle::RealmArable) > 0, "lord 3 sows the same county");
    }

    fn counts(county: &County, map: &CampaignMap) -> (i32, i32, i32) {
        let mut probe = county.clone();
        crate::field::recount(&mut probe, map);
        (probe.fields_grain, probe.fields_cattle, probe.fields_fallow)
    }

    /// A market that records every request and grants the ones it is told to.
    struct Ledger {
        asked: Vec<(i32, Good)>,
        grant: bool,
    }

    impl Market for Ledger {
        fn buy(
            &mut self,
            _id: usize,
            county: &mut County,
            _map: &mut CampaignMap,
            qty: i32,
            good: Good,
        ) -> bool {
            self.asked.push((qty, good));
            if !self.grant {
                return false;
            }
            match good {
                Good::Grain => county.grain += qty,
                Good::Cattle => county.herd += qty,
            }
            true
        }
    }

    fn ledger(grant: bool) -> Ledger {
        Ledger { asked: Vec::new(), grant }
    }

    // --- the five, and that they really are five ---------------------------

    /// **The correction this module was written for.** Five allocators, two
    /// outer passes,
    /// style 9.
    #[test]
    fn there_are_five_allocators_and_the_two_passes_reach_different_ones() {
        assert_eq!(FarmStyle::ALL.len(), 5);
        // Every one has a distinct address in the binary.
        let mut addrs: Vec<u32> = FarmStyle::ALL.iter().map(|s| s.address()).collect();
        addrs.sort_unstable();
        addrs.dedup();
        assert_eq!(addrs.len(), 5, "five distinct allocators");

        assert_eq!(FarmStyle::for_realm(0), Some(FarmStyle::RealmArable));
        assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
        assert_eq!(FarmStyle::for_realm(9), Some(FarmStyle::RealmMixed));
        assert_eq!(FarmStyle::for_realm(2), None, "no other byte dispatches");

        assert_eq!(FarmStyle::for_neutral(0), Some(FarmStyle::NeutralArable));
        assert_eq!(FarmStyle::for_neutral(1), Some(FarmStyle::NeutralGrazing));
        assert_eq!(
            FarmStyle::for_neutral(9),
            None,
            "an ex-Knight county is left alone by the neutral pass"
        );
    }

    /// `FUN_004A3C67` is the **neutral** pass's style 0 and no AI realm can
    /// reach it. This is the briefing error, asserted.
    #[test]
    fn the_neutral_arable_style_is_unreachable_from_an_ai_realm() {
        assert_eq!(FarmStyle::NeutralArable.address(), 0x004A3C67);
        for byte in 0..=255u8 {
            assert_ne!(
                FarmStyle::for_realm(byte),
                Some(FarmStyle::NeutralArable),
                "byte {byte}"
            );
        }
    }

    /// Every one of the four lords' styles resolves, and between them they use
    /// all three realm allocators.
    #[test]
    fn the_four_lords_between_them_use_every_realm_allocator() {
        let mut seen = Vec::new();
        for lord in 1..=crate::tables::AI_PERSONALITY_COUNT as u8 {
            let byte = T.ai_farm_style(lord).expect("lord {lord} has a record");
            let style = FarmStyle::for_realm(byte).expect("and a live allocator");
            seen.push(style);
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen,
            vec![FarmStyle::RealmArable, FarmStyle::RealmGrazing, FarmStyle::RealmMixed]
        );
        assert_eq!(T.ai_farm_style(crate::realm::LORD_HUMAN), None);
    }

    // --- the shopping cascade ----------------------------------------------

    /// The cascade is **not** "the biggest affordable lot". A refused lot lets
    /// the next one be tried; an accepted one ends the run.
    #[test]
    fn a_lot_that_arrives_stops_the_cascade_and_one_refused_does_not() {
        let (mut c, mut cmap) = county_with(4);
        c.grain = 0;
        let mut m = ledger(true);
        run_buys(FarmStyle::NeutralArable, 1, &mut c, &mut cmap, &mut m);
        assert_eq!(m.asked, vec![(400, Good::Grain)], "the first lot satisfied the floor");
        assert_eq!(c.grain, 400);

        let (mut c, mut cmap) = county_with(4);
        c.grain = 0;
        let mut m = ledger(false);
        run_buys(FarmStyle::NeutralArable, 1, &mut c, &mut cmap, &mut m);
        assert_eq!(
            m.asked,
            vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain), (50, Good::Grain)],
            "every lot is offered when the purse refuses"
        );
    }

    // --- the stall,

    /// Build a county with a stall and a purse, the way the battle fixture's
    /// unowned counties are: grazing style, a merchant standing in it, and its
    /// own money from a season of tax.
    fn neutral_with_purse(purse: i32) -> (County, CampaignMap, crate::unit::Units) {
        let (mut c, map) = county_with(12);
        c.owner = 0;
        c.farm_style = 1;
        c.purse = purse;
        c.grain = 57;
        c.herd = 135;
        c.merchant_count = 1;
        c.merchant_unit = 4;
        let mut units = crate::unit::Units::new();
        let mut m = crate::unit::Unit::new(crate::unit::UnitKind::Merchant, 6, 8, 8);
        // `Merchant_SpawnAll` writes 100 into every merchant and nothing in the
        // binary ever writes a merchant's morale again.
        m.morale = 100;
        m.county = 1;
        units.put(4, m);
        (c, map, units)
    }

    /// A stall over a throwaway realm array, for the unowned-county tests that
/// never read a realm back. Leaked
    /// call, because a test process ends and a borrow checker does not care.
    fn stall<'a>(t: &'a Tables, c: &County, units: &crate::unit::Units) -> CountyStall<'a> {
        stall_with(t, c, units, Box::leak(realms().into_boxed_slice()))
    }

    fn stall_with<'a>(
        t: &'a Tables,
        c: &County,
        units: &crate::unit::Units,
        realms: &'a mut [Realm],
    ) -> CountyStall<'a> {
        let mut counties = vec![County::new(); crate::county::MAX_COUNTIES];
        counties[1] = c.clone();
        let counties: [County; crate::county::MAX_COUNTIES] =
            counties.try_into().expect("MAX_COUNTIES entries");
        CountyStall::new(t, &counties, units, realms, Season::Spring, true)
    }

    /// `Ai_TradeForCounty` (`0x0049E39B`) — the surplus sale, then the weapon
    /// order, on one county of the Knight's.
    ///
    /// The reserves are `reserve < held`, **strict**: 300 wood against 250
    /// sells 50, 100 iron sells nothing and 250 stone — exactly the reserve —
    /// sells nothing either. Timber banks the *base* price (1), maces cost the
    /// marked-up one (`10 + Pct(10, 100)` = 20), which is the two halves of
    /// `Merchant_Trade`
    ///
    /// *Ablation*: return `trade_for_county` to the trait's empty default and
    /// every assertion below goes red at once.
    #[test]
    fn a_lord_sells_his_surplus_and_buys_the_countys_weapon() {
        let (mut c, mut map, units) = neutral_with_purse(0);
        c.owner = 1; // realm 1 is lord 1, the Knight: reserve 250, floor 1,000, 100 weapons
        c.weapon_type = 1; // maces, `Ai_TradeForCounty`'s good 11
        let mut rs = realms();
        rs[1].gold = 2_015;
        rs[1].wood = 300;
        rs[1].iron = 100;
        rs[1].stone = 250;
        let mut m = stall_with(T, &c, &units, &mut rs);
        m.trade_for_county(1, &mut c, &mut map);
        drop(m);
        assert_eq!((rs[1].wood, rs[1].iron, rs[1].stone), (250, 100, 250));
        assert_eq!(rs[1].weapons[1], 100, "good 11 lands in weapons[1]");
        assert_eq!(rs[1].gold, 2_015 + 50 * 1 - 100 * 20);
        assert_eq!((rs[1].trade_received_a, rs[1].trade_received_b), (50, 50));
        assert_eq!((rs[1].trade_spent_a, rs[1].trade_spent_b), (2_000, 2_000));
    }

    /// The gold floor is strict
    /// `Ai_BuyGoodDownTo` (`0x004A4C41`)'s `qty = qty / 2` loop.
    ///
    /// A treasury of exactly the floor buys nothing; one crown over it cannot
    /// afford 100 maces at 2,000 and takes 50 at 1,000 instead. **The gold is
    /// read once, before the loop**, so this is the whole of the cascade.
    #[test]
    fn the_weapon_order_halves_until_the_treasury_covers_it() {
        for (gold, bought) in [(1_000, 0), (1_001, 50), (2_000, 100)] {
            let (mut c, mut map, units) = neutral_with_purse(0);
            c.owner = 1;
            c.weapon_type = 1;
            let mut rs = realms();
            rs[1].gold = gold;
            let mut m = stall_with(T, &c, &units, &mut rs);
            m.trade_for_county(1, &mut c, &mut map);
            drop(m);
            assert_eq!(rs[1].weapons[1], bought, "treasury {gold}");
            assert_eq!(rs[1].gold, gold - bought * 20, "treasury {gold}");
        }
    }

    /// Two refusals in front of all of it: **no stall** (county `+0x1A4`) and a
    /// **bankrupt realm** (`bankruptStage != 0`).
    #[test]
    fn a_lord_with_no_stall_or_no_credit_trades_nothing() {
        for case in ["no stall", "bankrupt"] {
            let (mut c, mut map, units) = neutral_with_purse(0);
            c.owner = 1;
            c.weapon_type = 1;
            let mut rs = realms();
            rs[1].gold = 5_000;
            rs[1].wood = 5_000;
            if case == "no stall" {
                c.merchant_count = 0;
            } else {
                rs[1].bankrupt_stage = 1;
            }
            let mut m = stall_with(T, &c, &units, &mut rs);
            m.trade_for_county(1, &mut c, &mut map);
            drop(m);
            assert_eq!((rs[1].gold, rs[1].wood, rs[1].weapons[1]), (5_000, 5_000, 0), "{case}");
        }
    }

    /// **An owned county pays out of its lord's treasury, not its own purse**
    /// — `Ai_BuyGood`'s second clause, `realm == 0 || price * qty <= gold`,
    /// then `Merchant_Trade`'s owned buy limb: `gold -= bill`,
    /// `tradeSpentB += bill`, `tradeSpentA += bill`.
    ///
    /// The grazing realm style has one grain line, `if (grain < 100) buy 400`,
    /// so the 1,600-crown bill is all or nothing: 1,600 in the treasury buys it
    /// and 1,599 does not. The county's own purse is loaded with more than
    /// enough both times, which is what says the purse is not the money.
    ///
    /// *Ablation*: put the owned arm back to `county.owner != 0 → false` and
    /// the first case goes red (nothing bought); test the purse for an owned
    /// county instead of the gold
    #[test]
    fn an_ai_lords_county_pays_for_its_grain_out_of_the_treasury() {
        for (gold, bought) in [(1_600, 400), (1_599, 0)] {
            let (mut c, mut map, units) = neutral_with_purse(10_000);
            c.owner = 2;
            c.grain = 0;
            c.herd = 50; // above the grazing style's cattle floor of 41
            let mut rs = realms();
            rs[2].gold = gold;
            let mut m = stall_with(T, &c, &units, &mut rs);
            run_buys(FarmStyle::RealmGrazing, 1, &mut c, &mut map, &mut m);
            drop(m);
            assert_eq!(c.grain, bought, "treasury {gold}");
            assert_eq!(c.purse, 10_000, "treasury {gold}: an owned county's purse is not spent");
            let bill = bought * 4;
            assert_eq!(rs[2].gold, gold - bill, "treasury {gold}");
            assert_eq!((rs[2].trade_spent_a, rs[2].trade_spent_b), (bill, bill), "treasury {gold}");
            for (i, r) in rs.iter().enumerate().filter(|(i, _)| *i != 2) {
                assert_eq!(r.gold, 0, "realm {i} is not the owner and pays nothing");
            }
        }
    }

    /// **A sack of grain costs four crowns**, which is the whole of why the
    /// cascade lands where it does.
    ///
    /// `g_goodsPrice[1]` is 2 and `Merchant_SpawnAll` gives every merchant a
    /// morale of 100, so `Ai_BuyGood` quotes `2 + max(1, Pct(2, 100))`. The
/// number is pinned from the table
    /// expression the code under test uses — `docs/agents.md`, *compute the
    /// probe from the constant you are ablating*.
    #[test]
    fn a_sack_of_grain_costs_four_crowns_at_the_countys_own_stall() {
        let (c, _, units) = neutral_with_purse(0);
        let s = stall(T, &c, &units);
        assert_eq!(s.price(Good::Grain, 100), 4, "base 2 doubled by a morale of 100");
//
        assert_eq!(s.price(Good::Grain, 0), 3, "the one-crown markup floor");
        assert_eq!(s.price(Good::Grain, 200), 6);
    }

    /// **The fifty sacks are `min(lot : 4 * lot <= purse)`, not a constant.**
    ///
    /// This is the fixture's own arithmetic, both ways round: county 3 of
    /// `old_turn.sav` holds 316 crowns and buys the 50-sack lot for 200; the
    /// same county of `safeturn.sav` one turn earlier holds 195 and buys
    /// **nothing**, because 200 is more than 195 and
    /// A purse of 400 reaches the 100-sack lot instead, which is what says the
/// rule is a threshold.
    #[test]
    fn the_lot_an_unowned_county_takes_is_the_largest_its_purse_covers() {
        for (purse, expect_lot, why) in [
            (195, 0, "one turn earlier, 195 crowns buys nothing at all"),
            (200, 50, "exactly the 50-sack bill"),
            (316, 50, "the fixture's own purse"),
            (399, 50, "still short of the 100-sack lot's 400"),
            (400, 100, "and the next lot up opens at exactly 400"),
            (1600, 400, "the top of the cascade"),
        ] {
            let (mut c, mut map, units) = neutral_with_purse(purse);
            let before = c.grain;
            let mut m = stall(T, &c, &units);
            run_buys(FarmStyle::NeutralGrazing, 1, &mut c, &mut map, &mut m);
            assert_eq!(c.grain - before, expect_lot, "purse {purse}: {why}");
            assert_eq!(c.purse, purse - expect_lot * 4, "purse {purse}: {why}");
        }
    }

    /// **No stall, no purchase** — not a smaller one, none. `Ai_BuyGood`'s
    /// whole body is inside `if (county.merchantCount != '\0')`.
    ///
    /// *Ablation*: delete the `if county.merchant_count == 0 { continue }` in
    /// [`CountyStall::new`] and this goes red — the county has a purse of 1,000
    /// and would buy the 200-sack lot.
    #[test]
    fn a_county_with_no_merchant_in_it_cannot_buy_at_any_price() {
        let (mut c, mut map, units) = neutral_with_purse(1_000);
        c.merchant_count = 0;
        let mut m = stall(T, &c, &units);
        run_buys(FarmStyle::NeutralGrazing, 1, &mut c, &mut map, &mut m);
        assert_eq!(c.grain, 57, "the stall gate refuses before the purse is even read");
        assert_eq!(c.purse, 1_000);
    }

    /// **A trade does not feed anybody.** `Merchant_Trade`'s tail recomputes
    /// the ration; it does not serve it.
    ///
    /// `Ration_Apply` (`0x0044DF5F`) has no store `-=` anywhere in it, and
    /// `crate::trade::settle_county` used to call the debiting `ration::apply`
    /// twice —
    ///
    /// *Ablation*: put either `ration::preview` in `settle_county` back to
    /// `ration::apply` and this goes red, because the store loses two meals it
    /// should not. **The herd is cut to 11 on purpose**: the first draft of
    /// this test left the fixture's 135 head in place, the dairy alone fed the
    /// county, no grain was ever eaten,
    /// test that could not have failed. `docs/agents.md`, *ablate something the
    /// check claims*.
    #[test]
    fn buying_grain_does_not_make_the_county_eat_it() {
        let (mut c, mut map, units) = neutral_with_purse(400);
        // Just above the cascade's cattle floor of 11, so no herd is bought and
        // the dairy cannot cover five hundred people.
        c.herd = 11;
        c.ration_wanted = 3;
        c.ration_split = 0;
        let mut m = stall(T, &c, &units);
        // Establish that this county DOES eat grain, or the assertion below is
        // about nothing. `ration::apply` prices the meal and shadows it into
        // `+0x18C`; `land::grain_season_tick` is what takes it out of the
        // granary.
        let mut control = c.clone();
        crate::ration::apply(T, &mut control, true);
        assert!(
            control.grain_eaten_shadow > 0,
            "the fixture must actually eat grain for this to test anything"
        );

        run_buys(FarmStyle::NeutralGrazing, 1, &mut c, &mut map, &mut m);
        assert_eq!(
            c.grain, 157,
            "57 in store plus the 100-sack lot, with nothing eaten on the way through"
        );
    }

    /// **The arable realm style has two floors**, 600 for the big lots and 100
    /// for the small ones. A lord with 300 sacks keeps shopping; one with 150
    /// stops after the three big lots are offered.
    #[test]
    fn the_arable_realm_style_tops_up_to_six_hundred_then_to_one_hundred() {
        let (mut c, mut cmap) = county_with(4);
        c.grain = 150;
        let mut m = ledger(false);
        run_buys(FarmStyle::RealmArable, 1, &mut c, &mut cmap, &mut m);
        assert_eq!(
            m.asked,
            vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain)],
            "150 sacks is under 600 but not under 100"
        );

        let (mut c, mut cmap) = county_with(4);
        c.grain = 50;
        let mut m = ledger(false);
        run_buys(FarmStyle::RealmArable, 1, &mut c, &mut cmap, &mut m);
        assert_eq!(m.asked.len(), 5, "under 100, the two small lots are tried too");
    }

    /// The grazing realm style is the only one that stocks a herd of forty.
    #[test]
    fn only_the_grazing_realm_style_tops_its_herd_up_to_forty() {
        for (style, floor) in [
            (FarmStyle::NeutralGrazing, 11),
            (FarmStyle::RealmGrazing, 41),
            (FarmStyle::RealmMixed, 11),
        ] {
            let line = style.buys()[0];
            assert_eq!((line.good, line.floor, line.lot), (Good::Cattle, floor, 10), "{style:?}");
        }
        // The two arable styles never buy an animal.
        for style in [FarmStyle::NeutralArable, FarmStyle::RealmArable] {
            assert!(style.buys().iter().all(|l| l.good == Good::Grain), "{style:?}");
        }
    }

    /// Only the neutral arable style hands the county a purse, and only when it
    /// is short of grain.
    #[test]
    fn the_unowned_county_is_given_a_hundred_crowns_to_shop_with() {
        let (mut c, _) = county_with(4);
        c.grain = 0;
        assert_eq!(neutral_purse_top_up(FarmStyle::NeutralArable, &c), 100);
        c.grain = 100;
        assert_eq!(neutral_purse_top_up(FarmStyle::NeutralArable, &c), 0);
        c.grain = 0;
        for style in [FarmStyle::NeutralGrazing, FarmStyle::RealmArable, FarmStyle::RealmMixed] {
            assert_eq!(neutral_purse_top_up(style, &c), 0, "{style:?}");
        }
    }

    /// With no merchant the styles still run: they farm what is already there.
    #[test]
    fn a_style_runs_to_completion_against_a_market_that_refuses_everything() {
        let (mut c, mut map) = county_with(8);
        c.owner = 2;
        c.herd = 40;
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        assert_eq!(c.industry_share, 50);
        assert!(counts(&c, &map).0 > 0, "it still planted");
    }

    // --- the layouts --------------------------------------------------------

    /// **The arable styles plant half the county and keep one pasture.**
    #[test]
    fn the_arable_style_plants_half_the_county_in_winter() {
        let (mut c, mut map) = county_with(8);
        c.herd = 50;
        c.fertility = 0;
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        let (g, p, _) = counts(&c, &map);
        assert_eq!(g, 4, "half of eight");
        assert_eq!(p, 1, "and exactly one pasture, because the herd is over ten");
    }

    /// A county with ten head or fewer gets **no** pasture at all from an
    /// arable lord — the herd is left to graze nothing.
    #[test]
    fn an_arable_lord_gives_a_small_herd_no_pasture_whatever() {
        let (mut c, mut map) = county_with(8);
        c.herd = 10;
// Start it with a pasture, so the test shows the clear
        // absence.
        map.terrain[c.field_tile(0).unwrap()] = terrain::PASTURE;
        crate::field::recount(&mut c, &map);
        assert_eq!(c.fields_cattle, 1);
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        assert_eq!(counts(&c, &map).1, 0, "the pasture was cleared and not replaced");
    }

    /// **The grazing styles never plant grain**, whatever the season.
    #[test]
    fn a_grazing_lord_clears_grain_in_every_season() {
        for season in Season::ALL {
            let (mut c, mut map) = county_with(6);
            for slot in 0..3 {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN;
            }
            crate::field::recount(&mut c, &map);
            assert_eq!(c.fields_grain, 3);
            c.herd = 200;
            c.herd_crowding = 50;
            let mut e = env();
            e.season = season;
            lay(FarmStyle::RealmGrazing, &mut c, &mut map, &e);
            assert_eq!(counts(&c, &map).0, 0, "no grain in {}", season.name());
        }
    }

    /// The grazing styles grow their pasture **one field a pass**, up to all
    /// but one of the county.
    #[test]
    fn a_grazing_lord_adds_one_pasture_a_pass_and_stops_one_short() {
        let (mut c, mut map) = county_with(5);
        c.owner = 2;
        c.herd = 400;
        let mut seen = Vec::new();
        for _ in 0..8 {
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            lay(FarmStyle::RealmGrazing, &mut c, &mut map, &env());
            seen.push(counts(&c, &map).1);
        }
        assert_eq!(seen, vec![1, 2, 3, 4, 4, 4, 4, 4], "one a pass, capped at total - 1");
    }

    /// **Style 9 splits the county in three** — a third grain, a third pasture.
    #[test]
    fn the_mixed_style_gives_a_third_to_each() {
        let (mut c, mut map) = county_with(9);
        c.owner = 2;
        c.herd = 400;
        c.fertility = 0;
        for _ in 0..5 {
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            lay(FarmStyle::RealmMixed, &mut c, &mut map, &env());
        }
        let (g, p, _) = counts(&c, &map);
        assert_eq!(g, 3, "a third of nine in grain");
        assert_eq!(p, 3, "and a third in pasture");
    }

    /// **Finding 1, asserted: the `-2` rung is dead code.** No fertility
    /// anywhere on the scale produces `base - 2`.
    #[test]
    fn the_fertility_ladders_middle_rung_can_never_be_reached() {
        for fertility in -100..=100 {
            let q = winter_grain_quota(fertility, 10);
            assert_ne!(q, 8, "fertility {fertility} reached the unreachable rung");
            assert_eq!(q, if fertility < -20 { 9 } else { 10 });
        }
    }

    /// **Finding 2, asserted: turning Advanced Farming off plants more.**
    #[test]
    fn switching_advanced_farming_off_makes_the_ai_plant_far_more_grain() {
        let plant = |style: FarmStyle, advanced: bool| {
            let (mut c, mut map) = county_with(9);
            c.herd = 0;
            c.fertility = 0;
            let mut e = env();
            e.advanced_farming = advanced;
            lay(style, &mut c, &mut map, &e);
            counts(&c, &map).0
        };
        assert_eq!((plant(FarmStyle::RealmArable, true), plant(FarmStyle::RealmArable, false)), (4, 8));
        assert_eq!((plant(FarmStyle::RealmMixed, true), plant(FarmStyle::RealmMixed, false)), (3, 4));
        assert_eq!(
            (plant(FarmStyle::NeutralArable, true), plant(FarmStyle::NeutralArable, false)),
            (4, 6),
            "the neutral pass keeps three fields back where the realm one keeps one"
        );
    }

    /// Outside Winter the arable styles re-lay nothing: the standing crop is
    /// left where it is. Only the pasture clear runs.
    #[test]
    fn an_arable_lord_leaves_a_standing_crop_alone_outside_winter() {
        for season in [Season::Spring, Season::Summer, Season::Autumn] {
            let (mut c, mut map) = county_with(8);
            for slot in 0..6 {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN + 3;
            }
            crate::field::recount(&mut c, &map);
            c.herd = 0;
            let mut e = env();
            e.season = season;
            lay(FarmStyle::RealmArable, &mut c, &mut map, &e);
            assert_eq!(counts(&c, &map).0, 6, "{} left the crop standing", season.name());
        }
    }

    // --- rations ------------------------------------------------------------

    /// **The dairy rungs are a downgrade, not an upgrade.** A cattle county is
    /// fed Double where a grain county with the same amount of food is fed
    /// Triple — finding 4.
    #[test]
    fn a_herd_county_is_fed_worse_than_a_grain_county_with_the_same_food() {
        let mut c = County::new();
        c.population = 100;
        // 57 head: store 855 (> 8 * 100, which alone would say Triple) but
        // dairy 285, which is over 2 * 100 and not over 3 * 100.
        c.herd = 57;
        c.grain = 0;
        assert_eq!(food_in_store(T, &c), 855);
        assert_eq!(ration_wanted(T, &c, false), 4, "Double: the dairy rung got there first");

        // The same feeding capacity, entirely as grain.
        c.herd = 0;
        c.grain = 143;
        assert_eq!(food_in_store(T, &c), 858);
        assert_eq!(ration_wanted(T, &c, false), 5, "Triple, on the same food as bread");
    }

    ///: `store` counts
    /// the herd twice, so it is always at least three times `dairy`.
    #[test]
    fn the_triple_dairy_rung_is_redundant_at_every_herd_and_population() {
        let with_rung = |c: &County| ration_wanted(T, c, false) == 5;
        for people in [1i32, 7, 100, 999, 5_000] {
            for herd in [1i32, 10, 61, 400, 3_000] {
                let mut c = County::new();
                c.population = people;
                c.herd = herd;
                let dairy = ration::food_from_dairy(T, herd);
                if dairy > people * 3 {
                    assert!(
                        with_rung(&c),
                        "the rung fired at people {people}, herd {herd}"
                    );
                    assert!(
                        food_in_store(T, &c) > people * 8,
                        "and the store rung below would have said 5 anyway"
                    );
                }
            }
        }
    }

    /// The bottom of the ladder, at each boundary.
    #[test]
    fn the_ration_ladder_descends_with_the_larder() {
        let want = |grain: i32| {
            let mut c = County::new();
            c.population = 600;
            c.grain = grain;
            ration_wanted(T, &c, false)
        };
        assert_eq!(want(0), 0, "nothing at all");
        assert_eq!(want(49), 0, "294 < 300");
        assert_eq!(want(50), 1, "300 is not under 300");
        assert_eq!(want(100), 2, "600 clears the population");
        assert_eq!(want(200), 3, "1200 clears twice it");
        assert_eq!(want(601), 4, "3606 > 3600");
        assert_eq!(want(801), 5, "4806 > 4800");
    }

    /// **The two search directions differ only on ties**,
    /// normal case: many splits reach the same level.
    #[test]
    fn the_two_split_searches_take_the_opposite_end_of_a_tie() {
        let mut base = County::new();
        base.population = 200;
        base.herd = 300;
        base.grain = 300;

        let mut low = base.clone();
        set_rations(T, &mut low, SplitSearch::PreferGrain, false);
        let mut high = base.clone();
        set_rations(T, &mut high, SplitSearch::PreferHerd, false);

        assert_eq!(low.ration_wanted, high.ration_wanted);
        assert!(low.ration_split < high.ration_split, "{} < {}", low.ration_split, high.ration_split);
        assert_eq!(low.ration_achieved, high.ration_achieved, "and both feed the county the same");
    }

    /// A fixed split is taken as given and nothing is searched.
    #[test]
    fn a_fixed_split_is_used_as_it_stands() {
        let mut c = County::new();
        c.population = 100;
        c.grain = 500;
        set_rations(T, &mut c, SplitSearch::Fixed(37), false);
        assert_eq!(c.ration_split, 37);
    }

    /// **Which search a style asks for**, including the hundred-head hinge.
    #[test]
    fn only_an_arable_lord_with_a_hundred_head_spare_eats_the_herd_first() {
        let mut c = County::new();
        c.herd = 100;
        assert_eq!(split_search(FarmStyle::RealmArable, &c), SplitSearch::PreferGrain);
        c.herd = 101;
        assert_eq!(split_search(FarmStyle::RealmArable, &c), SplitSearch::PreferHerd);
        assert_eq!(split_search(FarmStyle::NeutralArable, &c), SplitSearch::PreferHerd);
        for style in [FarmStyle::RealmGrazing, FarmStyle::NeutralGrazing, FarmStyle::RealmMixed] {
            assert_eq!(split_search(style, &c), SplitSearch::PreferGrain, "{style:?}");
        }
    }

    /// The AI's food total counts the herd **twice** and reads the stores, not
    /// the season's caps. Distinct from `ration::food_available`.
    #[test]
    fn the_ai_food_total_counts_the_herd_twice_and_ignores_the_season_caps() {
        let mut c = County::new();
        c.herd = 10;
        c.grain = 10;
        c.herd_available = 0;
        c.grain_available = 0;
        assert_eq!(food_in_store(T, &c), 10 * 5 + 10 * 10 + 10 * 6);
        assert_eq!(
            crate::ration::food_available(T, &c),
            10 * 5,
            "the sibling reads the caps and sees only the dairy"
        );
    }

    // --- the shares ---------------------------------------------------------

    /// **Finding 3, asserted**: the AI takes the *Built* defaults, and both
    /// halves close on 100.
    #[test]
    fn every_style_takes_the_castle_favouring_default_shares() {
        use crate::tables::{JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_FIELD_RECLAMATION};
        let mut c = County::new();
        default_shares_built(&mut c);
        assert_eq!(&c.labour_share[0..3], &[33, 50, 17]);
        assert_eq!(&c.labour_share[3..8], &[40, 15, 15, 15, 15]);
        assert_eq!(c.labour_share[0..=JOB_FIELD_RECLAMATION].iter().sum::<i32>(), 100);
        assert_eq!(c.labour_share[JOB_CASTLE_BUILDING..=JOB_BLACKSMITH].iter().sum::<i32>(), 100);
    }

    /// The industry share is the styles' loudest difference,
    /// lord — not the grazier — is the one who industrialises.
    #[test]
    fn the_arable_realm_style_puts_half_the_county_into_industry() {
        assert_eq!(FarmStyle::RealmArable.industry_share(), 50);
        assert_eq!(FarmStyle::RealmMixed.industry_share(), 40);
        assert_eq!(FarmStyle::RealmGrazing.industry_share(), 20);
        assert_eq!(FarmStyle::NeutralArable.industry_share(), 0);
        assert_eq!(FarmStyle::NeutralGrazing.industry_share(), 0);
    }

    // --- the outer passes ---------------------------------------------------

    /// Step 5 stamps the lord's style on every county he holds and farms it.
    #[test]
    fn step_five_stamps_the_lords_style_on_every_county_he_holds() {
        let mut counties = vec![County::new(); 4];
        let mut map = CampaignMap::empty();
        for id in 1..=3 {
            for i in 0..6 {
                let tile = crate::map::index(i as u8, id as u8);
                counties[id].set_field_tile(i, Some(tile));
                map.terrain[tile] = terrain::FALLOW;
            }
            counties[id].population = 500;
            crate::field::recount(&mut counties[id], &map);
        }
        counties[1].owner = 2;
        counties[2].owner = 3;
        counties[3].owner = 2;

        // Lord 4 farms style 9.
        manage_county_farms(T, &mut counties, 3, &mut map, &realms(), 2, 4, &mut NoMarket, &env());
        assert_eq!(counties[1].farm_style, 9);
        assert_eq!(counties[3].farm_style, 9);
        assert_eq!(counties[2].farm_style, 0, "another realm's county is untouched");
        assert_eq!(counties[1].industry_share, FarmStyle::RealmMixed.industry_share());
        assert_eq!(counties[2].industry_share, 25, "a fresh county's default");
    }

    /// The neutral pass **reads** the style byte and never writes it,
    /// county that has changed hands keeps the old lord's habits — and one that
    /// kept a style-9 lord's byte is left entirely alone.
    #[test]
    fn an_unowned_county_farms_by_whichever_lord_held_it_last() {
        let mut counties = vec![County::new(); 3];
        let mut map = CampaignMap::empty();
        for id in 1..=2 {
            for i in 0..6 {
                let tile = crate::map::index(i as u8, id as u8);
                counties[id].set_field_tile(i, Some(tile));
                map.terrain[tile] = terrain::FALLOW;
            }
            counties[id].population = 500;
            counties[id].herd = 200;
            crate::field::recount(&mut counties[id], &map);
            counties[id].herd_crowding =
                crate::land::herd_crowding(T, counties[id].herd, counties[id].fields_cattle);
        }
        counties[1].farm_style = 1; // an ex-grazier's county
        counties[2].farm_style = 9; // an ex-Knight's

        manage_neutral_fields(T, &mut counties, 2, &mut map, &realms(), &mut NoMarket, &env());
        assert_eq!(counties[1].farm_style, 1, "never rewritten");
        assert!(counts(&counties[1], &map).1 > 0, "it grazed");
        assert_eq!(
            counts(&counties[2], &map),
            (0, 0, 6),
            "style 9 dispatches nowhere in the neutral pass"
        );
    }

    /// **The "add a field" ladder starts a reclamation on a wasteland tile.**
    /// It does not conjure a fallow field, which is what this crate used to do —
    /// and [`crate::field::recount`] would have wiped that on the next pass.
    #[test]
    fn ordering_a_field_puts_a_wasteland_tile_under_reclamation() {
        let mut counties = vec![County::new(); 2];
        let mut map = CampaignMap::empty();
        counties[1].owner = 2;
        counties[1].population = 10;
        for i in 0..4 {
            let tile = crate::map::index(i as u8, 3);
            counties[1].set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::WASTE;
        }
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_waste, 4);

        assert_eq!(
            manage_county_farms(T, &mut counties, 1, &mut map, &realms(), 2, 4, &mut NoMarket, &env()),
            1,
            "an empty county always gets its first field"
        );
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_reclaiming, 1);
        assert_eq!(counties[1].fields_waste, 3);
    }

    /// **The ladder itself**, at every boundary the original branches on. It is
    /// an `if`/`else if` chain,
    /// conditions gains nothing even though it is small.
    #[test]
    fn the_field_ladder_orders_a_field_only_when_both_conditions_hold() {
        // Twenty wasteland tiles, so the quota is never the binding constraint,
        // and `fields` of them already fallow to set the county's total.
        let case = |fields: usize, population: i32| {
            let mut c = County::new();
            let mut map = CampaignMap::empty();
            for i in 0..20 {
                let tile = crate::map::index(i as u8, 8);
                c.set_field_tile(i, Some(tile));
                map.terrain[tile] = if i < fields { terrain::FALLOW } else { terrain::WASTE };
            }
            crate::field::recount(&mut c, &map);
            c.population = population;
            order_fields(&c, &mut map)
        };
        assert_eq!(case(0, 10), 1, "an empty county always gets its first field");
        assert_eq!(case(2, 150), 0, "small, but not populous enough for any row");
        assert_eq!(case(2, 201), 1);
        assert_eq!(case(4, 401), 1);
        assert_eq!(case(6, 601), 1);
        assert_eq!(case(8, 1001), 1);
        assert_eq!(case(8, 601), 0, "under 1000 the fields < 9 row does not fire");
        assert_eq!(case(12, 1201), 2, "a big county adds two");
        assert_eq!(case(12, 1200), 0, "and the test is strictly greater");
    }

    /// A county whose twenty tiles are all in use gains nothing, however big.
    #[test]
    fn a_full_county_cannot_gain_another_field() {
        let (mut c, mut map) = county_with(crate::county::MAX_FIELDS);
        c.population = 5_000;
        assert_eq!(order_fields(&c, &mut map), 0);
        crate::field::recount(&mut c, &map);
        assert_eq!(c.field_total(), crate::county::MAX_FIELDS as i32);
    }

    /// A county with no wasteland left is told to add a field and adds nothing.
    #[test]
    fn a_county_with_nothing_left_to_reclaim_gains_no_field() {
        let (mut c, mut map) = county_with(2);
        c.population = 10;
        assert_eq!(order_fields(&c, &mut map), 0, "two fallow fields, no waste");
    }

    /// **A field already under reclamation eats a place in the quota.** Told to
    /// add one, a county already reclaiming one starts nothing; told to add
    /// two, it starts one.
    #[test]
    fn a_reclamation_already_running_consumes_the_quota_without_doing_anything() {
        let build = || {
            let mut c = County::new();
            let mut map = CampaignMap::empty();
            for i in 0..4 {
                let tile = crate::map::index(i as u8, 5);
                c.set_field_tile(i, Some(tile));
                map.terrain[tile] = terrain::WASTE;
            }
            // Slot 0 is already halfway through a reclamation.
            map.terrain[c.field_tile(0).unwrap()] = terrain::RECLAIM_FIRST + 1;
            crate::field::recount(&mut c, &map);
            (c, map)
        };
        let (c, mut map) = build();
        assert_eq!(crate::field::order_reclamation(&c, &mut map, 1), 0);
        let (c, mut map) = build();
        assert_eq!(crate::field::order_reclamation(&c, &mut map, 2), 1);
        // An in-use field, by contrast, is skipped and costs nothing.
        let (mut c, mut map) = build();
        map.terrain[c.field_tile(0).unwrap()] = terrain::GRAIN;
        crate::field::recount(&mut c, &map);
        assert_eq!(crate::field::order_reclamation(&c, &mut map, 1), 1);
    }

/// A realm whose lord has no personality record farms nothing
    /// falling through to style 0 — the same refusal `ai::set_tax_rates` makes.
    #[test]
    fn a_lord_with_no_personality_record_farms_nothing() {
        let mut counties = vec![County::new(); 2];
        let mut map = CampaignMap::empty();
        counties[1].owner = 2;
        counties[1].population = 900;
        for i in 0..4 {
            let tile = crate::map::index(i as u8, 7);
            counties[1].set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::WASTE;
        }
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(
            manage_county_farms(T, &mut counties, 1, &mut map, &realms(), 2, 5, &mut NoMarket, &env()),
            0
        );
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_reclaiming, 0, "not even a field ordered");
    }

    // --- FUN_004A4694 -------------------------------------------------------

    /// The nudge is **one field at a time in both directions**,
    /// needs three conditions where the growth needs two.
    #[test]
    fn the_cattle_nudge_moves_one_field_and_only_under_its_own_conditions() {
        let grow = |crowding: i32, herd: i32, cattle: usize, cap: i32| {
            let (mut c, mut map) = county_with(8);
            for slot in 0..cattle {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::PASTURE;
            }
            crate::field::recount(&mut c, &map);
            c.herd = herd;
            c.herd_crowding = crowding;
            fit_cattle_fields(T, &mut c, &mut map, cap);
            c.fields_cattle
        };
        assert_eq!(grow(11, 100, 2, 5), 3, "crowded and under the cap: one more");
        assert_eq!(grow(11, 100, 5, 5), 5, "at the cap: nothing");
        assert_eq!(grow(10, 5, 3, 5), 2, "uncrowded, tiny herd, spare fields: one back");
        assert_eq!(grow(10, 5, 2, 5), 2, "two fields is the floor");
        assert_eq!(grow(10, 50, 3, 5), 3, "a herd of ten or more keeps its fields");
    }
}

