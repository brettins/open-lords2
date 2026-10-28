use crate::kingdom::Kingdom;
use crate::unit::{Unit, UnitKind};

/// Realm 1 holds county 2, and a garrison of realm 1 is in its castle, besieged by unit `b`.
fn held() -> (Kingdom, usize, usize) {
    let mut k = Kingdom::new(1);
    k.set_county_count(2);
    k.counties[2].owner = 1;
    k.counties[2].castle_type = 4;
    let g = k.campaign.units.spawn(Unit::new(UnitKind::Army, 1, 0, 0)).unwrap();
    let b = k.campaign.units.spawn(Unit::new(UnitKind::Army, 3, 1, 0)).unwrap();
    k.counties[2].garrison_unit = g;
    let u = k.campaign.units.get_mut(g).unwrap();
    u.garrison_county = 2;
    u.besieged_by = b as u8;
    (k, g, b)
}

#[test]
fn a_county_that_goes_independent_sends_its_garrison_out() {
    let (mut k, g, _) = held();
    k.make_county_independent(2);
    assert_eq!(k.counties[2].owner, 0);
    // Ablation: without `march_garrison_out` the castle keeps realm 1's men.
    assert_eq!(k.counties[2].garrison_unit, 0);
    let u = k.campaign.units.get(g).expect("the men stand in the field");
    assert_eq!((u.owner, u.garrison_county, u.besieged_by), (1, 0, 0));
}

#[test]
fn a_load_releases_a_garrison_that_is_not_its_countys_and_keeps_the_others() {
    let (mut k, g, _) = held();
    assert!(k.release_foreign_garrisons().is_empty(), "realm 1's men in realm 1's castle");
    k.counties[2].owner = 0;
    assert_eq!(k.release_foreign_garrisons(), vec![2]);
    assert_eq!(k.counties[2].garrison_unit, 0);
    assert_eq!(k.campaign.units.get(g).map(|u| u.garrison_county), Some(0));
}
