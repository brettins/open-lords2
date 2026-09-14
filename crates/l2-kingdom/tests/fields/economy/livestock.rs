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

/// **Why more milkmaids stop helping**, which is the half of the player's
/// cattle question that is not about the sidebar at all.
///
/// > *"I had lots of milk maids with low herd crowding and we were only getting
/// > 1 cow, and if I added more milk maids they were idle."*
///
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
/// 1. **The ceiling never exceeds six a head** — twice
///    [`l2_kingdom::tables::HERD_LABOUR_PER_HEAD`], which is where
///    `PctOf(labour, herd * 3)` reaches its 200% cap and births stop rising.
/// 2. **For a small herd it is three a head, not six.** A herd of five gets
///    `+5000` on its birth rate the moment staffing reaches 100%, and
///    `herd * birthRate / 10000` then rounds to the same integer at 100% as at
///    200% — so the argmax is the *first* of the two, and every milkmaid past
///    three a head is idle. That is the player's county.
/// 3. **The season moves the answer**,
///    Spring multiplies the births by `3/2` and Winter the deaths.
///
/// Nothing here is asserted against a typed constant: the counts come out of
/// [`l2_kingdom::land::herd_growth`], which `docs/kingdom.md` §13 reproduces
/// instruction for instruction.
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
/// * Disabling the `staffing >= 100` small-herd bonus fails **claim 2**
///   directly: the herd of five's ceiling drops from 15 to 7.
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

    // 1 — the bound, over every herd size a county plausibly holds.
    for herd in 1..=400 {
        c.herd = herd;
        c.herd_crowding = herd_crowding(t, herd, fields);
        for season in 1..=4u8 {
            let ceiling = herd_labour_estimate(t, &c, season).useful;
            assert!(
                ceiling <= herd * per_head * 2,
                "herd {herd} season {season}: ceiling {ceiling} is more than six a head",
            );
            // And it is a real argmax: nobody past it does any good.
            let at = herd_growth(t, herd, fields, ceiling, c.herd_crowding, season);
            let past = herd_growth(t, herd, fields, ceiling + 500, c.herd_crowding, season);
            assert!(
                past.net() <= at.net(),
                "herd {herd} season {season}: 500 more hands beat the ceiling",
            );
        }
    }

    // 2 — the player's case. A herd of five tops out at three a head, so the
    // sixteenth milkmaid is idle and so is the twentieth.
    c.herd = 5;
    c.herd_crowding = herd_crowding(t, 5, fields);
    let small = herd_labour_estimate(t, &c, 2).useful;
    assert_eq!(small, 5 * per_head, "a herd of five uses three milkmaids a head, not six");
    assert_eq!(
        herd_growth(t, 5, fields, small, c.herd_crowding, 2).net(),
        herd_growth(t, 5, fields, small * 2, c.herd_crowding, 2).net(),
        "and doubling the dairy buys exactly nothing",
    );

    // 3 — and the season is one of the inputs
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
    // Winter's half is on the deaths, so it needs a herd that has any: a
    // fully-staffed low-crowding herd loses none at all and `x * 3 / 2` on zero
    // would pass with the multiplier deleted.
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

/// **The small-herd bonus steps down harder than the herd steps up**
/// smaller herd outbreeds a larger one at all three of its boundaries.
/// `docs/bugs.md` B98.
///
/// The bonus in `FUN_0044DA99` is `+10000` below 5 head, `+5000` below 10 and
/// `+2000` below 25, per ten thousand, added to the birth rate once staffing
/// reaches 100 %. Each band is sensible on its own — a county reduced to three
/// cows has to be able to come back — and the three together are not monotone:
/// the animal that crosses a boundary costs more births than it brings.
///
/// **Reproduced on purpose**, and pinned here so that a ruleset which smooths
/// the ladder has to say so. The staffing is the one
/// `Herd_LabourEstimate` would assign, not a number chosen to make the point.
///
/// Ablation, run: flattening [`l2_kingdom::tables::HERD_SMALL_BONUS`] to three
/// equal bonuses makes every pair monotone and fails all three.
#[test]
fn a_smaller_herd_outbreeds_a_larger_one_at_each_bonus_step() {
    use l2_kingdom::land::{herd_crowding, herd_growth, herd_labour_estimate};
    let t = &l2_kingdom::tables::Tables::DEFAULT;
    let fields = 8;
    let spring = 1u8;

    // Fully staffed means "at the ceiling the game itself would assign", which
    // is what a player who has filled the dairy is looking at.
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

// The three boundaries, with the numbers —
    // an inequality alone would survive the whole ladder being scaled away.
    for &(small, big, fewer, more) in &[(4, 5, 7, 4), (9, 10, 10, 6), (24, 25, 16, 10)] {
        assert_eq!(calves(small), fewer, "a herd of {small} in spring");
        assert_eq!(calves(big), more, "a herd of {big} in spring");
        assert!(
            calves(small) > calves(big),
            "the bonus step at {big} head is supposed to cost more than the cow is worth",
        );
    }

    // And it is a boundary effect, not a trend: inside a band the larger herd
    // does breed faster, which is the half that says the ladder is otherwise
    // working.
    assert!(calves(8) > calves(6), "inside a band, more cows means more calves");
}


