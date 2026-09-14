#![allow(unused_imports)]
use super::*;
use super::generic_units::*;
use super::mercenaries::*;
use super::*;
use super::county::*;
use super::economy::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// The England position imports as six merchants owned by nobody, in the six
/// counties `docs/formats/plane4.md` predicted, each with the route it will
/// walk.
#[test]
fn the_england_fixture_imports_six_merchants_and_no_armies() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    assert_eq!(s.units.len(), 6);
    assert_eq!(s.merchant_start, [14, 5, 13, 11, 12, 4]);

    for (n, (slot, u)) in s.units.iter().enumerate() {
        assert_eq!(*slot, n + 1);
        assert_eq!(u.kind, l2_kingdom::UnitKind::Merchant);
        assert_eq!(u.owner, 6, "a merchant belongs to nobody");
        assert_eq!(u.county, s.merchant_start[n]);
        assert_eq!(u.cargo_county, s.merchant_start[n], "+0x167 is where it was born");
        assert_eq!(u.move_allowance, 0, "the file's zero, not the type's ten");
        assert_eq!(u.year_formed, 1, "the route cursor ships at 1, not 0");
        assert!(u.needs_destination);
        // Each merchant is standing in a county on its own route
        // it walks is its **slot** minus one — the coupling `Merchant_AdvanceAll`
        // rests on.
        let route = s.routes.row(slot - 1);
        assert!(
            route.contains(&u.county),
            "merchant {slot} started in county {}, which is not on route {route:?}",
            u.county,
        );
    }
    assert!(s.units.iter().all(|(_, u)| u.men == 0), "the England position has no troops on the map");
}

