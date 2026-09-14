#![allow(unused_imports)]
use super::*;
use super::livestock::*;
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

