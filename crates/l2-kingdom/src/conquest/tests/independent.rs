use crate::kingdom::Kingdom;
use crate::phase::Pass;
use crate::report::SeasonReport;
use crate::unit::{Unit, UnitKind};
use crate::units_tick::Encounter;

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
    k.campaign.units.get_mut(b).unwrap().besieging_county = 2;
    (k, g, b)
}

#[test]
fn a_county_that_goes_independent_sends_its_garrison_out() {
    let (mut k, g, b) = held();
    k.campaign.units.get_mut(g).unwrap().besieged_by = 0;
    k.campaign.units.get_mut(b).unwrap().besieging_county = 0;
    // Ablation: without `march_garrison_out` the castle keeps realm 1's men.
    assert_eq!(k.make_county_independent(2), None, "nobody besieges it, so there is no sortie");
    assert_eq!(k.counties[2].owner, 0);
    assert_eq!(k.counties[2].garrison_unit, 0);
    let u = k.campaign.units.get(g).expect("the men stand in the field");
    assert_eq!((u.owner, u.garrison_county), (1, 0));
}

/// `FUN_00437535` (`0x00437535`) tail: `Battle_BeginFromCampaign(garrison, besiegedBy)`.
/// Ablation: `march_garrison_out` returning `None`.
#[test]
fn a_besieged_garrison_that_marches_out_returns_the_sortie() {
    let (mut k, g, b) = held();
    let sortie = k.make_county_independent(2);
    assert_eq!(sortie, Some(Encounter { mover: g, occupant: b, county: 2 }));
    let u = k.campaign.units.get(g).expect("the men stand in the field");
    assert_eq!((u.garrison_county, u.besieged_by), (0, b as u8), "the battle clears the link");
}

/// `Siege_StartPhase` (`0x004A82B9`) calls `Siege_ValidateLink` (`0x004A8426`) for each besieger,
/// and the besieger of a county without a garrison loses its link there.
#[test]
fn the_besieger_of_a_garrison_that_marched_out_loses_its_link_in_the_next_siege_phase() {
    let (mut k, _, b) = held();
    k.make_county_independent(2);
    assert_eq!(k.campaign.units.get(b).map(|u| u.besieging_county), Some(2));
    crate::siege::start_phase(&k.counties, &mut k.campaign.units);
    assert_eq!(k.campaign.units.get(b).map(|u| u.besieging_county), Some(0));
}

/// Counties 1 to 3 in a row. Realm 2 holds county 2, so county 1 is cut off from realm 1.
#[test]
fn a_secession_hands_up_the_sortie_of_a_besieged_garrison() {
    let mut k = Kingdom::new(1);
    k.set_county_count(3);
    for id in 1..=3 {
        let c = &mut k.counties[id];
        c.owner = if id == 2 { 2 } else { 1 };
        c.population = 100 * id as i32;
        c.pop_band = c.compute_pop_band();
        for next in [id.wrapping_sub(1), id + 1].into_iter().filter(|n| (1..=3).contains(n)) {
            c.add_neighbour(next as u8);
        }
    }
    k.realms[1].strength = 3;
    k.realms[2].strength = 3;
    let g = k.campaign.units.spawn(Unit::new(UnitKind::Army, 1, 0, 0)).unwrap();
    let b = k.campaign.units.spawn(Unit::new(UnitKind::Army, 3, 1, 0)).unwrap();
    k.counties[1].garrison_unit = g;
    let u = k.campaign.units.get_mut(g).unwrap();
    (u.garrison_county, u.besieged_by) = (1, b as u8);
    k.campaign.units.get_mut(b).unwrap().besieging_county = 1;

    let mut report = SeasonReport::new();
    k.run_pass(Pass::SecedeIsolatedCounties, &mut report);
    assert_eq!(k.counties[1].owner, 0, "county 1 seceded");
    assert_eq!(report.sorties, vec![Encounter { mover: g, occupant: b, county: 1 }]);
}
