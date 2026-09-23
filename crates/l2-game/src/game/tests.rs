use super::*;
// The clamp lives in `Kingdom::set_ration_wanted` now, so the lib no longer
// names this constant — only the test that pins the cap does.
use l2_kingdom::tables::RATION_LEVEL_COUNT;

fn two_counties() -> Game {
    let mut g = Game::new(7);
    g.kingdom.set_county_count(2);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g
}

#[test]
fn only_the_players_own_counties_take_orders() {
    let mut g = two_counties();
    assert!(g.set_tax_rate(1, 9));
    assert_eq!(g.kingdom.counties[1].tax_rate, 9);

    assert!(!g.set_tax_rate(2, 9), "county 2 belongs to another realm");
    assert_eq!(g.kingdom.counties[2].tax_rate, 0, "and it is unchanged");
    assert!(!g.set_ration(2, 5));
    assert!(!g.set_tax_rate(9, 1), "and 9 is not a county at all");
}

#[test]
fn orders_are_clamped_to_the_ranges_the_rules_have() {
    let mut g = two_counties();
    g.set_tax_rate(1, -40);
    assert_eq!(g.kingdom.counties[1].tax_rate, 0);
    g.set_tax_rate(1, 10_000);
    assert_eq!(g.kingdom.counties[1].tax_rate, MAX_TAX_RATE);

    g.set_ration(1, 99);
    assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
    g.set_ration(1, -3);
    assert_eq!(g.kingdom.counties[1].ration_wanted, 0);

    // **What the player asks for (`+0x15E`) is not what the county managed
    // to feed (`+0x15D`)**, and the way to show that is a county that
    // cannot afford what is asked — *not* by requiring the control to leave
    // `ration_achieved` alone.
    //
    // This assertion used to be `ration_achieved == achieved`, and it was
    // asserting the defect: `Ration_IncreaseCounty` (`0x0043A23F`) calls
    // `Ration_Apply` before it repaints, so in the original the achieved
    // level moves the instant you press the arrow. Our setter wrote the
    // field and returned, the test agreed with it, and both were wrong
    // together. `docs/decisions.md` C125.
    //
    // People, and nothing to feed them with. A county with **no** people is
    // fed at triple rations trivially — the requirement is zero — which is
    // what the first draft of this assertion tripped over.
    g.kingdom.counties[1].population = 400;
    g.kingdom.counties[1].herd = 0;
    g.kingdom.counties[1].grain = 0;
    g.set_ration(1, RATION_LEVEL_COUNT as i32 - 1);
    assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
    assert_eq!(
        g.kingdom.counties[1].ration_achieved, 0,
        "an empty larder feeds nobody, whatever the player asked for",
    );
}

#[test]
fn selection_refuses_ids_that_are_not_counties_on_this_map() {
    let mut g = two_counties();
    assert!(g.select(2));
    assert_eq!(g.selected, 2);
    assert!(!g.select(3), "county 3 is past g_countyCount");
    assert_eq!(g.selected, 2, "and the refusal leaves the old selection alone");
    assert!(g.select(0));
    assert_eq!(g.selected, 0);
}

#[test]
fn counting_owners_walks_the_map_rather_than_trusting_the_realm_record() {
    let mut g = two_counties();
    g.kingdom.realms[1].county_count = 99; // a stale record
    assert_eq!(g.owned_by(1), 1);
    assert_eq!(g.owned_by(0), 0);
}
