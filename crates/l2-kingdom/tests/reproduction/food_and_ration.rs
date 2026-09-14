#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::reproduction::*;
use super::population_and_labour::*;
use super::simulation::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

/// **Realm 5's county, rewound to the stores the season *ended* on, and what
/// its lord's own ration sweep does with them.**
///
/// It holds no grain, splits its ration entirely onto grain, and keeps a herd of
/// 74 — which feeds 370 of its 417 people. The file says it ate at Normal
/// (`shownRation = +1`).
///
/// # What this test used to say, and why it was wrong
///
/// It was `realm_fives_county_diverges_because_the_save_does_not_record_what_it_ate`,
/// and it pinned ours at Half (`2, −2`), a health meter of 59, happiness 68, 66
/// deaths and a population of 414, on the reading that *"it must have had grain
/// to eat"* — eight sacks the save supposedly did not record. **Both halves of
/// that were ours.**
///
/// * **The save does record what the season started with.** `Grain_SeasonTick`
///   (`0x0044C8AE`) opens `+0x228 = grain; grain -= eaten;` and
///   `Herd_SeasonTick` (`0x0044D60D`) opens `+0x254 = herd; herd -= eaten;`. In
///   this file `+0x254` is **95** in every county and `+0x228` is 0 in every
///   owned one: the county went into the ration pass with 95 head, whose cheese
///   alone feeds 475 people, and needed no grain at all. The eight sacks were an
///   inversion of a pass that debits the store and saw the post-season herd;
///   `Ration_Apply` (`0x0044DF5F`) does neither (`docs/decisions.md` C149).
///   [`every_county_reproduces_from_the_stores_the_season_found`] is the proof.
/// * **And from the rewound stores ours no longer starves**, because
///   `Ai_ManageFarmsAll` (`0x0049A990`) now runs at the head of the new game's
///   `Season_Advance`. That county's lord farms arable (`Ai_FarmStyleArable`,
///   `0x004A4052`), which with under 101 head calls `Ai_SetRations(county, 0)`
///   (`0x004A4782`): an upward sweep over the split that keeps the first strict
///   improvement. With no grain and 74 head, split 0 feeds Half and split 100
///   feeds Normal on slaughter, so the sweep settles on 100. The original's
///   sweep saw 95 head, found split 0 already Normal on cheese, and kept it.
///
/// So the chain downstream of the happiness term now **lands on the file** —
/// happiness, health, deaths and population — by a different road, and the
/// divergence has moved to the three things the road changes: the split, the
/// closing ration preview, and the head slaughtered.
///
/// *Ablation*: skip `Pass::AiManageFarms` and this goes red with the old
/// numbers back — `(0, 2, −2)`, 59/2, 68, 66, 414.
#[test]
fn realm_fives_county_is_fed_on_slaughter_by_its_lords_sweep_from_the_rewound_stores() {
    let s = england!();
    let file = s.kingdom(SEED);
    let mut k = s.starting_kingdom(SEED);
    k.start_new_game();

    let hungry = hungry_county(&s);
    let ours = &k.counties[hungry];
    let theirs = &file.counties[hungry];

    assert_eq!(
        (theirs.grain, theirs.herd, theirs.ration_split),
        (0, 74, 0),
        "the file's county {hungry}"
    );
    assert_eq!(theirs.shown_ration, 1, "the file says it was fed at Normal");

    // The road: the lord's sweep put the ration on the herd.
    assert_eq!(
        (ours.ration_split, ours.ration_achieved, ours.shown_ration),
        (100, 3, 1),
        "ours is fed at Normal by slaughter"
    );
    // Where it lands — on the file.
    assert_eq!((ours.health_meter, ours.health_band), (theirs.health_meter, theirs.health_band));
    assert_eq!((ours.health_meter, ours.health_band), (67, 3));
    assert_eq!(ours.happiness, theirs.happiness);
    assert_eq!(ours.deaths, theirs.deaths);
    assert_eq!(ours.population, theirs.population);
    // And the three things the road changes.
    assert_eq!((ours.d_hap_ration, theirs.d_hap_ration), (1, -2), "the closing preview");
    assert_eq!((ours.herd_eaten, theirs.herd_eaten), (12, 0), "the head slaughtered");

    // Everything upstream of the ration term lands either way.
    assert_eq!(ours.pop_last, theirs.pop_last);
    assert_eq!(ours.shown_tax, theirs.shown_tax);
    assert_eq!(ours.tax_collected, theirs.tax_collected);
}

/// **The food the season ate is recoverable, and the answer is unique.**
///
/// Exactly one pre-season `(herd, grain)` per county both feeds it at the level
/// the file's `shownRation` records and leaves behind the stores the file holds.
/// Nine unowned counties opened on **73 head** and closed on 67; county 1 opened
/// on **8 sacks** and closed on none.
///
/// The uniqueness is what makes these numbers a measurement.
/// It also settles `docs/kingdom.md` §4.3's open question about which of
/// `Ration_Apply`'s two calls survives: a pass that did not debit the store
/// could not have taken county 1's grain to zero, nor the unowned counties'
/// herds from 73 to 67.
///
/// > **Kept as arithmetic, retracted as a finding.** The assertions below are
/// > true of [`solve_opening`]: under a ration pass that debits the store and
/// > sees the herd the season ended on, these openings are the only ones that
/// > fit. Neither premise is the original's. `Ration_Apply` (`0x0044DF5F`) has
/// > no store subtraction (`docs/decisions.md` C149), and the herd it saw is in
/// > the file at county `+0x254` — **95** in every county, not 73 or 74 —
/// > because `Herd_SeasonTick` (`0x0044D60D`) copies the herd there before it
/// > debits what was eaten. Uniqueness inside a wrong model is not evidence for
/// > the model. [`every_county_reproduces_from_the_stores_the_season_found`]
/// > reproduces the map from the stores the file records, with no inversion.
#[test]
fn the_food_the_season_ate_is_recoverable_and_unique() {
    let s = england!();
    let file = s.kingdom(SEED);
    let t = &Tables::DEFAULT;

    for id in s.county_ids() {
        let stored = &file.counties[id];
        // The level the happiness update saw, read back out of its own display
        // copy: shownRation = 3L - 8.
        let level = (stored.shown_ration + 8) / 3;
        assert_eq!(t.ration_happiness(level), stored.shown_ration, "county {id}");

        let opening = solve_opening(stored, t);
        if stored.owner == 0 {
            assert_eq!(opening, (73, 100), "unowned county {id}");
        }
    }

    let hungry = hungry_county(&s);
    assert_eq!(
        solve_opening(&file.counties[hungry], t),
        (74, 8),
        "county {hungry} opened on eight sacks and ate all of them"
    );
    for id in s.county_ids().filter(|&id| id != hungry) {
        let stored = &file.counties[id];
        if stored.owner == 0 {
            continue;
        }
        assert_eq!(
            solve_opening(stored, t),
            (stored.herd, stored.grain),
            "owned county {id} lives on its dairy and spends nothing"
        );
    }
}

/// The ration rule against the file with **no reconstruction at all**: run the
/// end-of-season preview on the counties, and it
/// must produce the stored `rationAchieved`, `herdEaten`, `grainEaten` and
/// `dHapRation`.
///
/// Four different configurations and two different ration levels, so it is not
/// one case fourteen times: nine unowned counties on an all-livestock split
/// slaughtering thirteen head; three owned counties whose dairy alone covers
/// them; county 8 the same on the other split; and county 1 dropping to Half.
#[test]
fn the_ration_preview_reproduces_every_stored_food_field() {
    let s = england!();
    let file = s.kingdom(SEED);
    let t = &Tables::DEFAULT;

    let mut levels = std::collections::BTreeSet::new();
    for id in s.county_ids() {
        let stored = &file.counties[id];
        let mut c = stored.clone();
        l2_kingdom::ration::preview(t, &mut c, false);
        assert_eq!(c.ration_achieved, stored.ration_achieved, "county {id}");
        assert_eq!(c.herd_eaten, stored.herd_eaten, "county {id}");
        assert_eq!(c.grain_eaten, stored.grain_eaten, "county {id}");
        assert_eq!(c.d_hap_ration, stored.d_hap_ration, "county {id}");
        assert_eq!(c.herd, stored.herd, "county {id}: a preview spends nothing");
        assert_eq!(c.grain, stored.grain, "county {id}");
        levels.insert(c.ration_achieved);
    }
    assert_eq!(
        levels,
        [2, 3].into_iter().collect::<std::collections::BTreeSet<i32>>(),
        "two levels, not one case fourteen times"
    );
}

