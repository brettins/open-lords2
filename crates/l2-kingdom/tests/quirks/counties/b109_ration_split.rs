use super::*;
use l2_kingdom::ration::{self, Sowing};

const T: &Tables = &Tables::DEFAULT;

fn county(population: i32, herd: i32, grain: i32, wanted: i32, split: i32) -> County {
    let mut c = County::new();
    c.population = population;
    c.herd = herd;
    c.grain = grain;
    c.ration_wanted = wanted;
    c.ration_split = split;
    c
}

fn fed(mut c: County, quirks: Quirks) -> County {
    ration::preview(T, &mut c, false, Sowing::NONE.with_quirks(quirks));
    c
}

/// Issue #15's case: no herd, wanted Triple, slider 33 % livestock, Double achieved.
/// Ablation: `settle` returning `choose_within`'s plan whatever the quirk.
#[test]
fn b109_a_grain_county_pays_for_double_in_full_only_when_fixed() {
    let (faithful, fixed) = pair(Quirk::SplitShortfallGoesUneaten);
    let honest = fed(county(600, 0, 1000, 4, 0), faithful);
    assert_eq!((honest.ration_achieved, honest.grain_eaten), (4, 200));

    let cheat = fed(county(600, 0, 1000, 5, 33), faithful);
    assert_eq!((cheat.ration_achieved, cheat.grain_eaten), (4, 134), "reproduced");

    let paid = fed(county(600, 0, 1000, 5, 33), fixed);
    assert_eq!((paid.ration_achieved, paid.grain_eaten), (4, 200), "fixed");
    assert_eq!(paid.d_hap_ration, cheat.d_hap_ration, "the level is the original's");
}

/// The mirror: a herd county with the slider at grain takes the shortfall in heads.
/// 1200 people, 100 head (500 dairy), no grain, 20 % livestock: Half, 600 food.
#[test]
fn b109_a_cattle_county_takes_the_grain_share_in_heads_when_fixed() {
    let (faithful, fixed) = pair(Quirk::SplitShortfallGoesUneaten);
    let cheat = fed(county(1200, 100, 0, 3, 20), faithful);
    let paid = fed(county(1200, 100, 0, 3, 20), fixed);
    assert_eq!((cheat.ration_achieved, cheat.herd_eaten), (2, 2));
    assert_eq!((paid.ration_achieved, paid.herd_eaten), (2, 10));
    assert_eq!(paid.grain_eaten, 0);
}

/// A split that fits its stores is untouched by the fix.
#[test]
fn b109_a_split_the_stores_can_meet_eats_the_same_either_way() {
    let (faithful, fixed) = pair(Quirk::SplitShortfallGoesUneaten);
    for split in [0, 25, 50, 100] {
        let a = fed(county(800, 200, 2000, 5, split), faithful);
        let b = fed(county(800, 200, 2000, 5, split), fixed);
        let got = |c: &County| (c.ration_achieved, c.grain_eaten, c.herd_eaten);
        assert_eq!(got(&a), got(&b), "split {split}");
    }
}

/// `Kingdom::sowing` carries the kingdom's quirks into the pass.
/// Ablation: drop `.with_quirks(self.options.quirks)` from `Kingdom::sowing`.
#[test]
fn b109_the_kingdom_hands_its_quirks_to_the_ration_pass() {
    let (_, fixed) = pair(Quirk::SplitShortfallGoesUneaten);
    let mut k = furnished_kingdom(11);
    k.options.quirks = fixed;
    let c = &mut k.counties[3];
    (c.population, c.herd, c.grain, c.ration_split) = (600, 0, 1000, 33);
    c.friendly_troops = 0;
    c.enemy_troops = 0;
    k.options.armies_eat = false;
    k.set_ration_wanted(3, 5);
    assert_eq!((k.counties[3].ration_achieved, k.counties[3].grain_eaten), (4, 200));
}
