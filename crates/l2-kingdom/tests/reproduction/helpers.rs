#![allow(unused_imports)]
use super::*;
use super::reproduction::*;
use super::food_and_ration::*;
use super::population_and_labour::*;
use super::simulation::*;
use l2_formats::save::Save;
use l2_kingdom::county::County;
use l2_kingdom::phase::SEASON_PIPELINE;
use l2_kingdom::tables::{health_band, Season, Tables, Weather};
use l2_scenario::{Scenario, STARTING_HEALTH_METER};

/// The one county the imported starting position cannot feed, because the save
/// does not record what it ate.
///
/// **Corrected.** This was `const DEAD_END: usize = 1`, and it conflated two
/// facts that happened to coincide in the one save anybody had looked at:
/// county 1 is the map's dead end, *and* county 1 was the county that starts on
/// Half rations. A second England turn-one save separates them — there the
/// hungry county is 8, and county 1 is still the dead end.
///
/// What actually predicts it is the **realm**: county 1 belonged to realm 5 in
/// the first save and county 8 belongs to realm 5 in the second. One lord always
/// begins short of food, and it is always realm 5. That is a game-design fact
/// nobody here had noticed while it was written down as a county index.
///
/// > **Not a game-design fact, and not "short of food" either.** Every county
/// > began on 95 head (county `+0x254`, the herd `Herd_SeasonTick` found) and
/// > was fed on cheese; this is the one whose herd the season then took below
/// > 84, where cheese stops covering 417 people, so the *rewound* position —
/// > which starts from the post-season herd — cannot feed it. Two saves agreeing
/// > on realm 5 is two saves; why that realm's herd falls furthest is `[I]`.
/// > See the module documentation's correction.
pub(crate) fn hungry_county(s: &Scenario) -> usize {
    s.county_ids()
        .find(|&id| s.counties[id].as_ref().is_some_and(|c| c.owner == 5))
        .expect("realm 5 holds a county in an England turn-one save")
}

/// Everything worth comparing between a county we computed and a county the
/// file holds. Twenty-four fields, named, so a failure says which one.
///
/// # Why `herd` and `herd_eaten` are not two of them any more
///
/// They were, and **they reproduced because a rule was missing.** The herd used
/// to move only with the weather, which is neutral on this map, so "put back
/// what the ration pass ate" was the whole of it and the inversion in
/// [`solve_opening`] closed. Now that a herd is born, dies and has to be tended
/// (`docs/kingdom.md` §13), the season moves it — and the file says outright
/// that the inversion's answer `+0x254` is the herd as
/// `Herd_SeasonTick` found it, and it is **95 in every one of the fourteen
/// counties**: the new-game starting herd from the table at `0x004DC0D0`, not
/// the 73 the inversion recovers.
///
/// Getting from 95 to the stored herd needs the **pre-season** cattle labour,
/// and the save records only the post-season allocation — `FUN_0044F6E7`
/// reassigns every county's workers after the population moves. County 4 is the
/// proof: its stored herd needs a staffing of 86%, about 246 workers, and the
/// file holds 261. A search over openings settles it — with the file's labour
/// there are two or three openings that land for each *owned* county and
/// **none at all** for the nine unowned ones. `herd_eaten` follows the herd
/// out, because it is the ration *preview*, computed last from the herd the
/// season ended on.
///
/// So this list drops the two fields it was reproducing for the wrong reason,
/// and [`the_herds_own_forecast_reproduces_for_every_county`] adds four per
/// county that it reproduces for the right one — read straight out of the file,
/// no inversion anywhere. Modelling the labour allocator would bring these two
/// back; that is a separate piece of work, and it is named here so it is not
/// lost.
pub(crate) fn comparison(ours: &County, file: &County) -> Vec<(&'static str, i32, i32)> {
    vec![
        ("owner", ours.owner as i32, file.owner as i32),
        ("happiness", ours.happiness, file.happiness),
        ("happiness_last", ours.happiness_last, file.happiness_last),
        ("happiness_sum", ours.happiness_sum, file.happiness_sum),
        ("happiness_avg", ours.happiness_avg, file.happiness_avg),
        ("shown_tax", ours.shown_tax, file.shown_tax),
        ("shown_ration", ours.shown_ration, file.shown_ration),
        ("shown_health", ours.shown_health, file.shown_health),
        ("shown_events", ours.shown_events, file.shown_events),
        ("d_hap_ration", ours.d_hap_ration, file.d_hap_ration),
        ("health_meter", ours.health_meter, file.health_meter),
        ("health_band", ours.health_band as i32, file.health_band as i32),
        ("unrest", ours.unrest as i32, file.unrest as i32),
        ("population", ours.population, file.population),
        ("pop_last", ours.pop_last, file.pop_last),
        ("pop_band", ours.pop_band, file.pop_band),
        ("births", ours.births, file.births),
        ("deaths", ours.deaths, file.deaths),
        ("emigrants", ours.emigrants, file.emigrants),
        ("immigrants", ours.immigrants, file.immigrants),
        ("tax_collected", ours.tax_collected, file.tax_collected),
        ("ration_achieved", ours.ration_achieved, file.ration_achieved),
        ("grain_eaten", ours.grain_eaten, file.grain_eaten),
        ("grain", ours.grain, file.grain),
    ]
}

/// Search a generous box of pre-season `(herd, grain)` for the openings that
/// (a) feed the county at the level the file's `shownRation` records and
/// (b) leave behind exactly the stores the file holds. Panics unless there is
/// exactly one.
///
/// Inverting a rule to recover its input is an oracle's business, so it lives
/// here and not in `l2-scenario`: an importer that solved the rules to build its
/// own starting position would be handing this test the answer.
pub(crate) fn solve_opening(stored: &County, t: &Tables) -> (i32, i32) {
    let level = (stored.shown_ration + 8) / 3;
    let mut probe = County::new();
    probe.population = stored.pop_last;
    probe.ration_wanted = stored.ration_wanted;
    probe.ration_split = stored.ration_split;
    let mut found = None;
    for herd in stored.herd..=stored.herd + 200 {
        for grain in stored.grain..=stored.grain + 200 {
            probe.herd = herd;
            probe.grain = grain;
            let plan = l2_kingdom::ration::choose(t, &probe, false);
            if plan.level == level
                && herd - plan.heads == stored.herd
                && grain - plan.sacks == stored.grain
            {
                assert!(found.is_none(), "more than one opening store fits this county");
                found = Some((herd, grain));
            }
        }
    }
    found.expect("an opening store must exist")
}

/// The starting position with the food the season ate put back — the openings
/// [`the_food_the_season_ate_is_recoverable_and_unique`] proves unique.
pub(crate) fn kingdom_after_the_first_season(s: &Scenario) -> l2_kingdom::Kingdom {
    let file = s.kingdom(SEED);
    let t = &Tables::DEFAULT;
    let mut k = s.starting_kingdom(SEED);
    for id in s.county_ids() {
        let (herd, grain) = solve_opening(&file.counties[id], t);
        k.counties[id].herd = herd;
        k.counties[id].grain = grain;
    }
    k.start_new_game();
    k
}

// ---------------------------------------------------------------------------
// The scenario itself — the correction C12 was hiding
// ---------------------------------------------------------------------------

