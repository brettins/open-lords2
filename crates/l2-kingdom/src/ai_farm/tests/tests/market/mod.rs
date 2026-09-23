#![allow(unused_imports)]

mod styles;
pub use styles::*;
mod feeding;
pub use feeding::*;
mod fields;
pub use fields::*;

use super::*;

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
    assert_eq!(s.price(Good::Grain, 0), 3, "the one-crown markup floor");
    assert_eq!(s.price(Good::Grain, 200), 6);
}

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
    c.herd = 11;
    c.ration_wanted = 3;
    c.ration_split = 0;
    let mut m = stall(T, &c, &units);
    // Establish that this county DOES eat grain, or the assertion below is
    // about nothing. `ration::apply` prices the meal and shadows it into
    // `+0x18C`; `land::grain_season_tick` is what takes it out of the
    // granary.
    let mut control = c.clone();
    crate::ration::apply(T, &mut control, true, crate::ration::Sowing::NONE);
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

