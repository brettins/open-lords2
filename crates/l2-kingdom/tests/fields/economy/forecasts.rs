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

/// A player: *"I right now have −11 cattle. If I move it so the people are
/// eating cattle, it still says −11 cattle in the sidebar."* The figure was
/// never wrong — `Herd_LabourEstimate`'s tail writes
/// `(births − deaths) − herdEaten`, so slaughter **is** in it, and `L2.eng`
/// group 77 index 28 calls it *"Overall change"*. What was wrong is *when*:
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

    k.counties[county].labour[cattle] = k.counties[county].herd * 3;
    k.refresh_estimates(county);
    let staffed = k.counties[county].herd_change_expected;

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

/// A player: *"the figure is missing in the sidebar — it draws the serf
/// reclaiming, but not the +1 I'm used to."* `Field_ReclaimEstimate`
/// (`0x0044C278`) is a work-outstanding loop **plus a tail** that simulates the
/// coming season, and only the loop was ported. The tail's `+0x20C` counts
/// **fields that will be finished next season** — fields, not units of work,
/// which is the thing a small integer could plausibly have been either of.
#[test]
fn the_reclamation_forecast_counts_fields_finished_not_work_done() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let job = l2_kingdom::tables::JOB_FIELD_RECLAMATION;
    let per_season = k.tables.field.reclaim_per_season;
    let full = k.tables.field.progress_max;

    let county = k.county_ids().find(|&id| k.counties[id].field_slots_used() >= 2).expect("fields");
    let slots: Vec<usize> =
        (0..l2_kingdom::MAX_FIELDS).filter(|&s| k.counties[county].field_tile(s).is_some()).collect();
    let (near, far) = (slots[0], slots[1]);
    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::RECLAIM_FIRST;
    }
    k.counties[county].field_progress[near] = (full - per_season) as u16;
    k.counties[county].field_progress[far] = 0;

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

    k.counties[county].field_progress[far] = (full - per_season) as u16;
    k.counties[county].labour[job] = per_season * 2;
    k.refresh_estimates(county);
    assert_eq!(
        k.counties[county].reclaim_fields_finishing, 2,
        "two gangs' worth finishes two fields, which is why this is a count and not a flag",
    );

    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::FALLOW;
    }
    k.refresh_estimates(county);
    assert_eq!(k.counties[county].reclaim_fields_finishing, 0);
    assert_eq!(k.counties[county].reclaim_seasons_to_next, 0);
}

/// A player: *"Sidebar doesn't show grain being planted as a negative
/// number."* `Grain_LabourEstimate` (`0x0044D374`) is a search loop **plus a
/// tail**
/// ported, and `docs/decisions.md` C123 is the repair.
#[test]
fn the_grain_forecast_is_the_sowing_loss_the_player_reported() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let mine = (1..=k.county_count)
        .find(|&id| k.counties[id].owner == scenario.local_player)
        .expect("the local player holds a county");

    k.counties[mine].grain = 10_000;
    assert!(paint_all_fallow_to_grain(&mut k, mine) > 0, "county {mine} had fields to paint");

    assert_eq!(k.season_next, 1, "the England position faces Spring");
    k.refresh_estimates(mine);
    let c = &k.counties[mine];
    let (sown, eaten, shown) = (c.grain_sown_expected, c.grain_eaten, c.grain_change_expected);
    assert!(sown > 0, "farmers on grain fields forecast a sowing: {sown}");
    assert!(shown < 0, "facing Spring the grain row is a loss, not {shown}");
    assert_eq!(shown, -sown - eaten, "Spring is `-sown - eaten`");

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

