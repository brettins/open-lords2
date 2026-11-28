use super::*;
use l2_kingdom::report::SeasonReport;
use l2_kingdom::Pass;

fn healthy_county_after_one_pass(quirks: Quirks) -> (i32, u8) {
    let mut k = furnished_kingdom(11);
    k.options.quirks = quirks;
    let c = &mut k.counties[3];
    (c.population, c.health_meter, c.health_band, c.ration_achieved) = (300, 100, 3, 5);
    k.run_pass(Pass::HealthUpdate, &mut SeasonReport::new());
    (k.counties[3].health_meter, k.counties[3].health_band)
}

/// Issue #16. Ablation: `health_update` passing `Quirks::FAITHFUL` to `health::update`.
#[test]
fn b274_the_kingdom_bands_full_health_good_unless_fixed() {
    let (faithful, fixed) = pair(Quirk::FullHealthBandsGood);
    assert_eq!(healthy_county_after_one_pass(faithful), (100, 3), "reproduced");
    assert_eq!(healthy_county_after_one_pass(fixed), (100, 4), "fixed");
}

#[test]
fn b274_is_always_fixed_online() {
    assert!(Quirk::FullHealthBandsGood.fixed_online());
    assert!(Quirks::FAITHFUL.online().is_fixed(Quirk::FullHealthBandsGood));
}
