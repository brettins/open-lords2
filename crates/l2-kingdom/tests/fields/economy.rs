#![allow(unused_imports)]
use super::*;
use super::counts::*;
use super::painting::*;
use super::herd_vis::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

/// **The cattle forecast moves when an input to it moves.**
///
/// A player: *"I right now have −11 cattle. If I move it so the people are
/// eating cattle, it still says −11 cattle in the sidebar."* The figure was
/// never wrong — `Herd_LabourEstimate`'s tail writes
/// `(births − deaths) − herdEaten`, so slaughter **is** in it, and `L2.eng`
/// group 77 index 28 calls it *"Overall change"*. What was wrong is *when*:
/// the tail ran only from `herd_season_tick`, so no control could move it.
///
/// The original calls `Herd_LabourEstimate` from **both** `Herd_SeasonTick`'s
/// last line and `County_RefreshEstimates`; ours now does too.
///
/// **The assertion is the player's own diagnostic** — change an input the
/// figure depends on and require the figure to change — which is the signal
/// that found this
/// it. It deliberately does not assert a *value*: a test that pinned −11 would
/// pass just as well with the forecast frozen, which is the whole defect.
///
/// Ablating the `herd_preview` call in `field::refresh_estimates` fails it.
#[test]
fn the_cattle_forecast_follows_the_labour_it_depends_on() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;
    let county = k
        .county_ids()
        .find(|&id| k.counties[id].fields_cattle > 0 && k.counties[id].herd > 0)
        .expect("a county with a herd");

    // Staff the dairy fully and record the forecast.
    k.counties[county].labour[cattle] = k.counties[county].herd * 3;
    k.refresh_estimates(county);
    let staffed = k.counties[county].herd_change_expected;

    // Take every hand off it. Understaffing is added to the death rate — at
    // zero staffing the band's 1 becomes 1 + 33 — so the forecast must fall.
    k.counties[county].labour[cattle] = 0;
    k.refresh_estimates(county);
    let bare = k.counties[county].herd_change_expected;

    assert_ne!(
        staffed, bare,
        "the forecast did not move when the dairy was emptied: {staffed} both times",
    );
    assert!(
        bare < staffed,
        "an unstaffed herd should forecast worse than a fully staffed one: {staffed} -> {bare}",
    );
    eprintln!("county {county}: herd {} forecasts {staffed} staffed, {bare} bare",
        k.counties[county].herd);
}

/// **The reclamation row's two figures, and what the "+1" counts.**
///
/// A player: *"the figure is missing in the sidebar — it draws the serf
/// reclaiming, but not the +1 I'm used to."* `Field_ReclaimEstimate`
/// (`0x0044C278`) is a work-outstanding loop **plus a tail** that simulates the
/// coming season, and only the loop was ported. The tail's `+0x20C` counts
/// **fields that will be finished next season** — fields, not units of work,
/// which is the thing a small integer could plausibly have been either of.
///
/// Three claims, and each is a different line of the tail:
///
/// 1. **a field one season's work from done finishes** — one field, one gang;
/// 2. **the gang starts on the nearest-to-finished field**
///    completes before a field at 0 gets touched;
/// 3. **a finished field hands its surplus on**
///    finishes two in a season — which is the only way the figure ever reads
/// more than 1.
#[test]
fn the_reclamation_forecast_counts_fields_finished_not_work_done() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let job = l2_kingdom::tables::JOB_FIELD_RECLAMATION;
    let per_season = k.tables.field.reclaim_per_season;
    let full = k.tables.field.progress_max;

    let county = k.county_ids().find(|&id| k.counties[id].field_slots_used() >= 2).expect("fields");
    // Two fields under reclamation: one nearly done, one untouched.
    let slots: Vec<usize> =
        (0..l2_kingdom::MAX_FIELDS).filter(|&s| k.counties[county].field_tile(s).is_some()).collect();
    let (near, far) = (slots[0], slots[1]);
    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::RECLAIM_FIRST;
    }
    k.counties[county].field_progress[near] = (full - per_season) as u16;
    k.counties[county].field_progress[far] = 0;

    // 1 and 2 — one gang's worth of labour finishes the near field only.
    k.counties[county].labour[job] = per_season;
    k.refresh_estimates(county);
    assert_eq!(
        k.counties[county].reclaim_fields_finishing, 1,
        "one season's work on the nearest-to-finished field completes it and nothing else",
    );
    assert_eq!(
        k.counties[county].reclaim_seasons_to_next, 1,
        "and it is one season away",
    );

    // 3 — enough for both
    // The far field needs a full 800, so this is deliberately generous: what is
    // being asserted is that the count can exceed 1 at all.
    k.counties[county].field_progress[far] = (full - per_season) as u16;
    k.counties[county].labour[job] = per_season * 2;
    k.refresh_estimates(county);
    assert_eq!(
        k.counties[county].reclaim_fields_finishing, 2,
        "two gangs' worth finishes two fields, which is why this is a count and not a flag",
    );

// And nothing being reclaimed forecasts nothing,
    // last answer — the original zeroes both before its guard.
    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::FALLOW;
    }
    k.refresh_estimates(county);
    assert_eq!(k.counties[county].reclaim_fields_finishing, 0);
    assert_eq!(k.counties[county].reclaim_seasons_to_next, 0);
}

/// **The grain row's forecast, driven the way a player produces it.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative
/// number."* `Grain_LabourEstimate` (`0x0044D374`) is a search loop **plus a
/// tail**
/// ported, and `docs/decisions.md` C123 is the repair.
///
/// **The repair landed and had no test.** Its two siblings each got one —
/// [`the_cattle_forecast_follows_the_labour_it_depends_on`] and
/// [`the_reclamation_forecast_counts_fields_finished_not_work_done`] — and the
/// one the player reported did not, so this exists.
///
/// It is deliberately driven from the **brush and the season**
/// writing the county's fields: `docs/agents.md`'s *a field is only tested if
/// something a test reads was written by something the game runs*. Every input
/// here is something a player does.
///
/// Three claims, one per arm of the tail's ladder (`season` is `g_seasonNext`):
///
/// ```c
/// if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
/// else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
/// else                  county.field_0x22C = -county.grainEaten;
/// ```
///
/// 1. **Facing Spring the row is a loss and cannot be anything else** — sowing
///    spends the store, so the forecast for the season about to begin is
///    negative. That is the player's sentence with the arithmetic under it.
/// 2. **It is exactly `-sown - eaten`**, and `sown` is the tail's own
///    `Grain_Sow(county, staff, grain)`
///    `Grain_Sow(county, workers, grain - grainEaten)` — a different third
///    argument, so the number cannot be recovered from the ceiling.
/// 3. **Facing Winter it is the harvest less the eating**, so the same row
/// turns positive once there is a crop to bring in. A test that only ever
///    looked at Spring would pass with the other two arms deleted.
///
/// Ablation, run: deleting the `crate::land::grain_preview` call from
/// `field::refresh_estimates` fails claims 1 and 3, at `0` both times.
#[test]
fn the_grain_forecast_is_the_sowing_loss_the_player_reported() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let mine = (1..=k.county_count)
        .find(|&id| k.counties[id].owner == scenario.local_player)
        .expect("the local player holds a county");

    // Seed the granary before painting, for the reason
    // `a_player_can_paint_a_field_to_grain_and_harvest_it_four_seasons_later`
    // states: an empty store is told it has no use for a farmer.
    k.counties[mine].grain = 10_000;
    assert!(paint_all_fallow_to_grain(&mut k, mine) > 0, "county {mine} had fields to paint");

    // The fixture opens facing Spring,
    // player was looking at.
    assert_eq!(k.season_next, 1, "the England position faces Spring");
    k.refresh_estimates(mine);
    let c = &k.counties[mine];
    let (sown, eaten, shown) = (c.grain_sown_expected, c.grain_eaten, c.grain_change_expected);
    assert!(sown > 0, "farmers on grain fields forecast a sowing: {sown}");
    // 1 — the sign, which is the whole report.
    assert!(shown < 0, "facing Spring the grain row is a loss, not {shown}");
    // 2 — and it is the tail's arithmetic, not something that resembles it.
    assert_eq!(shown, -sown - eaten, "Spring is `-sown - eaten`");

    // 3 — the other end of the year. Run the crop round to Winter and the same
    // row becomes the harvest less the eating.
    for _ in 0..3 {
        k.advance_season();
    }
    assert_eq!(k.season_next, 4, "facing Winter, the harvest turn");
    k.refresh_estimates(mine);
    let c = &k.counties[mine];
    assert!(c.crop[2] > 0, "there is a harvest to forecast: {:?}", c.crop);
    assert_eq!(
        c.grain_change_expected,
        c.crop[2] - c.grain_eaten,
        "facing Winter the row is `harvest - eaten`",
    );
    assert!(
        c.grain_change_expected > 0,
        "and a county with a crop in the ground forecasts a gain, not {}",
        c.grain_change_expected,
    );
}

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

