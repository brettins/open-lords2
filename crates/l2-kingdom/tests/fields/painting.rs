#![allow(unused_imports)]
use super::*;
use super::counts::*;
use super::herd_vis::*;
use super::economy::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

/// **The end-to-end one.** A human player paints a field to grain, ends four
/// turns
///
/// This is the first time this engine has driven its own economy from the
/// player's side.
/// a fabricated county, because `docs/decisions.md` C26 is what happens when
/// the only fixture exercises one value of a rule's input.
#[test]
fn a_player_can_paint_a_field_to_grain_and_harvest_it_four_seasons_later() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);

    // The human's county. `g_localPlayer` is realm 1 and which counties it
// holds is rolled per game, so it is found (C23).
    let human = scenario.local_player;
    let mine = (1..=k.county_count)
        .find(|&id| k.counties[id].owner == human)
        .expect("the local player holds a county");

    // Seed grain, **before** painting. The five owned counties of this
    // position store none at all (C20 — the file records no *opening* store),
    // and the order matters for a reason worth stating: the grain ceiling the
    // brush computes is a search over `Grain_Sow`, which asks whether the
    // store can afford the seed. A county with an empty granary is told it has
    // no use for a farmer, and none is assigned.
    k.counties[mine].grain = 10_000;
    let before = k.counties[mine].grain;

    // Paint every fallow field this county has to grain.
    let painted = paint_all_fallow_to_grain(&mut k, mine);
    assert!(painted > 0, "county {mine} had fields to paint");
    assert_eq!(k.counties[mine].fields_grain, painted, "and they are grain now");
    assert!(
        k.counties[mine].labour[0] > 0,
        "and painting put farmers on them: {:?}",
        k.counties[mine].labour
    );

    let sown = k.advance_season();
    assert!(
        k.counties[mine].grain < before,
        "sowing debits the seed: {} -> {}",
        before,
        k.counties[mine].grain
    );
    let crop_after_sowing: i32 = k.counties[mine].crop.iter().sum();
    assert!(crop_after_sowing > 0, "and it puts a crop in the ground: {sown:?}");

    // Spring and Summer grow it; Autumn harvests it into the store.
    let mut low = k.counties[mine].grain;
    for _ in 0..3 {
        k.advance_season();
        low = low.min(k.counties[mine].grain);
    }
    assert_eq!(k.season, 4, "back to Winter");
    assert!(
        k.counties[mine].grain > low,
        "the harvest put grain back in the store: low {low}, now {}",
        k.counties[mine].grain
    );
    // **The crop words are not cleared by the harvest**, and this test used to
// say they were. `crop` is *seed, standing crop, harvest*
    // growth stages: `Grain_SeasonTick` clears `crop[2]` at the top of every
    // season and fills it at the harvest, and `crop[0]` and `crop[1]` keep the
    // year's record until the next sowing overwrites them. See
    // [`l2_kingdom::land`].
    assert!(
        k.counties[mine].crop[2] > 0,
        "the harvest is the third word: {:?}",
        k.counties[mine].crop
    );
    assert_eq!(
        k.counties[mine].crop[0] * l2_kingdom::tables::GRAIN_YIELD_PER_SACK,
        k.counties[mine].crop[1],
        "and the first two are still the seed and the crop it became"
    );
}

/// The same painting, done twice, is the same kingdom — the brush is a rule and
/// rules are deterministic (`docs/netcode.md`).
#[test]
fn painting_is_deterministic() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let run = || {
        let mut k = scenario.kingdom(7);
        let mine = (1..=k.county_count)
            .find(|&id| k.counties[id].owner == scenario.local_player)
            .expect("a county");
        paint_all_fallow_to_grain(&mut k, mine);
        k.counties[mine].grain = 10_000;
        for _ in 0..4 {
            k.advance_season();
        }
        k
    };
    assert!(run() == run(), "two identical playthroughs diverged");
}

/// Paint every fallow field of one county to grain, the way the player does it:
/// one tile, one brush stroke. Returns how many strokes landed.
fn paint_all_fallow_to_grain(k: &mut Kingdom, county: usize) -> i32 {
    let tiles: Vec<usize> = k
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, kind)| kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    let mut painted = 0;
    for tile in tiles {
        k.paint_field(county, tile, FieldType::Grain)
            .expect("a fallow field takes the grain brush");
        painted += 1;
    }
    painted
}

