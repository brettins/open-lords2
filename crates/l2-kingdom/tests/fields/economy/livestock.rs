#![allow(unused_imports)]
use super::*;
use super::forecasts::*;
use super::*;
use super::counts::*;
use super::painting::*;
use super::herd_vis::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

/// Both halves are `Herd_LabourEstimate` (`0x0044DD4D`), whose search loop is
/// the dairy's ceiling: the **fewest** workers that reach the best
/// `births - deaths`, because the test inside it is a strict `<`. The sidebar
/// rings the cow the moment `labour > useful`, so *idle* is that ceiling being
/// hit.
///
/// `l2_kingdom::land::herd_labour_estimate` documents the closed form as
/// *"about `6 * herd`"* and marks it **`[I]`**. This measures it, and finds the
/// inference true as a **bound** and wrong as an estimate for exactly the case
/// the player was in:
///
/// Two ablations
/// in a way worth keeping:
///
/// * Relaxing the search's `best < net` to `best <= net` makes it take the
/// **last** argmax instead of the first. Claim 2 was expected to fail at six
///   a head; what fails is **claim 1**, at `ceiling 9999` for a herd
///   of one — the last argmax is the end of the scan, not `6 * herd`. The
///   ablation found the right defect for a reason one step away from the one
///   written down, so the note says what happened
///   was expected.
#[test]
fn the_dairy_ceiling_is_the_fewest_milkmaids_that_reach_the_best_herd() {
    use l2_kingdom::land::{herd_crowding, herd_growth, herd_labour_estimate};
    let t = &l2_kingdom::tables::Tables::DEFAULT;
    let per_head = l2_kingdom::tables::HERD_LABOUR_PER_HEAD;
    let fields = 8;
    let mut c = l2_kingdom::county::County::new();
    c.population = 10_000;
    c.pop_band = 1;
    c.fields_cattle = fields;

    for herd in 1..=400 {
        c.herd = herd;
        c.herd_crowding = herd_crowding(t, herd, fields);
        for season in 1..=4u8 {
            let ceiling = herd_labour_estimate(t, &c, season).useful;
            assert!(
                ceiling <= herd * per_head * 2,
                "herd {herd} season {season}: ceiling {ceiling} is more than six a head",
            );
            let at = herd_growth(t, herd, fields, ceiling, c.herd_crowding, season);
            let past = herd_growth(t, herd, fields, ceiling + 500, c.herd_crowding, season);
            assert!(
                past.net() <= at.net(),
                "herd {herd} season {season}: 500 more hands beat the ceiling",
            );
        }
    }

    c.herd = 5;
    c.herd_crowding = herd_crowding(t, 5, fields);
    let small = herd_labour_estimate(t, &c, 2).useful;
    assert_eq!(small, 5 * per_head, "a herd of five uses three milkmaids a head, not six");
    assert_eq!(
        herd_growth(t, 5, fields, small, c.herd_crowding, 2).net(),
        herd_growth(t, 5, fields, small * 2, c.herd_crowding, 2).net(),
        "and doubling the dairy buys exactly nothing",
    );

    c.herd = 74;
    c.herd_crowding = herd_crowding(t, 74, fields);
    let hands = herd_labour_estimate(t, &c, 1).useful;
    let spring = herd_growth(t, 74, fields, hands, c.herd_crowding, 1);
    let summer = herd_growth(t, 74, fields, hands, c.herd_crowding, 2);
    assert_eq!(
        spring.births,
        summer.births * 3 / 2,
        "Spring is half again as many calves: {spring:?} against {summer:?}",
    );
    let packed = herd_crowding(t, 400, fields);
    let winter = herd_growth(t, 400, fields, 2_400, packed, 4);
    let autumn = herd_growth(t, 400, fields, 2_400, packed, 3);
    assert!(autumn.deaths > 0, "the herd this claim is about has deaths to multiply");
    assert_eq!(
        winter.deaths,
        autumn.deaths * 3 / 2,
        "and Winter half again as many deaths: {winter:?} against {autumn:?}",
    );
}

/// The bonus in `FUN_0044DA99` is `+10000` below 5 head, `+5000` below 10 and
/// `+2000` below 25, per ten thousand, added to the birth rate once staffing
/// reaches 100 %. Each band is sensible on its own — a county reduced to three
/// cows has to be able to come back — and the three together are not monotone:
#[test]
fn a_smaller_herd_outbreeds_a_larger_one_at_each_bonus_step() {
    use l2_kingdom::land::{herd_crowding, herd_growth, herd_labour_estimate};
    let t = &l2_kingdom::tables::Tables::DEFAULT;
    let fields = 8;
    let spring = 1u8;

    let calves = |herd: i32| {
        let mut c = l2_kingdom::county::County::new();
        c.population = 10_000;
        c.pop_band = 1;
        c.herd = herd;
        c.fields_cattle = fields;
        c.herd_crowding = herd_crowding(t, herd, fields);
        let hands = herd_labour_estimate(t, &c, spring).useful;
        herd_growth(t, herd, fields, hands, c.herd_crowding, spring).births
    };

    for &(small, big, fewer, more) in &[(4, 5, 7, 4), (9, 10, 10, 6), (24, 25, 16, 10)] {
        assert_eq!(calves(small), fewer, "a herd of {small} in spring");
        assert_eq!(calves(big), more, "a herd of {big} in spring");
        assert!(
            calves(small) > calves(big),
            "the bonus step at {big} head is supposed to cost more than the cow is worth",
        );
    }

    assert!(calves(8) > calves(6), "inside a band, more cows means more calves");
}


