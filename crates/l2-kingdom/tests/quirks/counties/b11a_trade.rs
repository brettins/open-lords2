#![allow(unused_imports)]
use super::*;
use super::b12_b15_quirks::*;
use super::b16_b17_quirks::*;
use super::*;
use super::economy::*;
use super::victory::*;
use super::wire::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

#[test]
fn b11a_a_lordless_county_sells_grain_it_does_not_have_or_is_refused() {
    use l2_kingdom::trade::{Good, Order, Quote, Refusal};
    let (faithful, fixed) = pair(Quirk::UnownedCountyTradesUnchecked);
    let quote = Quote { buy: 10, sell: 8 };

    let run = |quirks: Quirks| {
        let mut k = furnished_kingdom(11);
        k.options.quirks = quirks;
        k.counties[3].owner = 0; // nobody's county
        k.counties[3].grain = 0; // and nothing in the barn
        l2_kingdom::trade::trade(&mut k, Order::sell(Good::Grain, 100, quote, 0, 3))
            .map(|r| r.crowns)
            .map_err(|e| e)
    };

    assert_eq!(run(faithful), Ok(800), "reproduced: it sells what it has not got");
    assert_eq!(run(fixed), Err(Refusal::NotEnoughStock));
}

#[test]
fn b11a_a_lordless_county_buys_with_an_empty_purse_or_is_refused() {
    use l2_kingdom::trade::{Good, Order, Quote, Refusal};
    let (faithful, fixed) = pair(Quirk::UnownedCountyTradesUnchecked);
    let quote = Quote { buy: 10, sell: 8 };

    let run = |quirks: Quirks| {
        let mut k = furnished_kingdom(12);
        k.options.quirks = quirks;
        k.counties[3].owner = 0;
        k.counties[3].purse = 0;
        l2_kingdom::trade::trade(&mut k, Order::buy(Good::Grain, 100, quote, 0, 3))
            .map(|_| k.counties[3].purse)
    };

    assert_eq!(run(faithful), Ok(-1000), "reproduced: the purse goes negative");
    assert_eq!(run(fixed), Err(Refusal::NotEnoughGold));
}

/// An **owned** county was always guarded, so the switch must not touch it.
#[test]
fn b11a_an_owned_county_is_refused_either_way() {
    use l2_kingdom::trade::{Good, Order, Quote, Refusal};
    let (faithful, fixed) = pair(Quirk::UnownedCountyTradesUnchecked);
    let quote = Quote { buy: 10, sell: 8 };
    for quirks in [faithful, fixed] {
        let mut k = furnished_kingdom(13);
        k.options.quirks = quirks;
        k.counties[3].owner = 1;
        k.counties[3].grain = 0;
        assert_eq!(
            l2_kingdom::trade::trade(&mut k, Order::sell(Good::Grain, 100, quote, 1, 3)),
            Err(Refusal::NotEnoughStock)
        );
    }
}

// ---------------------------------------------------------------------------
// B12 — turning castle building on removes its labour share
// ---------------------------------------------------------------------------

