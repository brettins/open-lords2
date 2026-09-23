#![allow(unused_imports)]
use super::*;
use super::seasons::*;
use super::taxes::*;
use super::super::*;
use crate::tables::Weather;

#[test]
fn the_history_ring_is_four_hundred_seasons_of_sixteen_counties() {
    assert_eq!(crate::tables::HISTORY_SEASONS, 400);
    assert_eq!(crate::tables::HISTORY_COUNTIES, 16);
    assert_eq!(
        crate::tables::HISTORY_SEASONS * crate::tables::HISTORY_COUNTIES * 8,
        51_200,
        "save block 10 at 0x0056D8C0"
    );
    assert_eq!(
        crate::tables::HISTORY_COUNTIES,
        crate::county::MAX_COUNTY_ID as usize,
        "one slot per addressable county"
    );
}

#[test]
fn a_season_writes_one_line_per_county_and_the_ring_reads_back_in_order() {
    let mut k = Kingdom::new(1);
    k.set_county_count(3);
    assert!(k.history.is_empty());
    for season in 1..=5i32 {
        for id in 1..=3 {
            k.counties[id].population = 100 * season + id as i32;
            k.counties[id].happiness = 40 + season;
        }
        k.history.record(&k.counties.clone());
    }
    assert_eq!(k.history.len(), 5);

    let county_two = k.history.county(2);
    assert_eq!(county_two.len(), 5);
    assert_eq!(
        county_two.iter().map(|e| e.population).collect::<Vec<i32>>(),
        vec![102, 202, 302, 402, 502],
        "oldest first"
    );
    assert_eq!(county_two[4].happiness, 45);

    let latest = k.history.latest().expect("five seasons recorded");
    assert_eq!(latest[1].population, 502, "county 2 is slot 1");
}

#[test]
fn the_ring_writes_every_slot_whatever_the_map_holds() {
    let mut k = Kingdom::new(2);
    k.set_county_count(4);
    for id in 1..=4 {
        k.counties[id].population = 500;
    }
    k.history.record(&k.counties.clone());
    let latest = *k.history.latest().expect("one season");
    for slot in 0..4 {
        assert_eq!(latest[slot].population, 500);
    }
    for slot in 4..crate::tables::HISTORY_COUNTIES {
        assert_eq!(latest[slot].population, 0, "slot {slot} is an empty record");
    }
}

#[test]
fn the_ring_forgets_its_beginning_after_four_hundred_seasons() {
    let mut k = Kingdom::new(3);
    k.set_county_count(1);
    for season in 1..=(crate::tables::HISTORY_SEASONS as i32 + 50) {
        k.counties[1].population = season;
        k.history.record(&k.counties.clone());
    }
    assert_eq!(k.history.len(), crate::tables::HISTORY_SEASONS);
    let held = k.history.county(1);
    assert_eq!(held.len(), crate::tables::HISTORY_SEASONS);
    assert_eq!(held[0].population, 51, "the first fifty seasons are gone");
    assert_eq!(held[held.len() - 1].population, 450);
}

#[test]
fn asking_the_ring_for_a_county_it_does_not_hold_gives_nothing() {
    let k = Kingdom::new(4);
    assert!(k.history.county(0).is_empty());
    assert!(k.history.county(crate::tables::HISTORY_COUNTIES + 1).is_empty());
    assert!(k.history.latest().is_none(), "and nothing before the first season");
}

#[test]
fn advancing_a_season_records_a_line_in_the_ring() {
    let mut k = Kingdom::new(5);
    k.set_county_count(2);
    k.counties[1].population = 400;
    k.start_new_game();
    assert_eq!(k.history.len(), 1, "one season, one line");
    k.advance_season();
    assert_eq!(k.history.len(), 2);
}

#[test]
fn moving_the_tax_rate_moves_what_people_pay_and_the_local_happiness() {
    let mut k = Kingdom::new(21);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 1000;
    }
    k.set_tax_rate(1, 0);
    assert_eq!(k.counties[1].tax_shown, 0, "nobody pays anything at a rate of nothing");

    k.set_tax_rate(1, 20);
    let paid = k.counties[1].tax_shown;
    assert!(paid > 0, "at a fifth, a thousand people pay something");
    assert_eq!(
        k.counties[1].d_hap_tax_local, 5 - 20,
        "5 - rate, and it moves on every click",
    );

    k.set_tax_rate(1, 40);
    assert!(k.counties[1].tax_shown > paid, "and twice the rate is more crowns");
    assert_eq!(k.counties[1].d_hap_tax_local, 5 - 40);
}

/// `taxHapOther` is `g_taxHappinessOther[rate]`, a table
/// flat zero from 0 to 19. Every county in every fixture sits at rate 0 and
/// the highest an AI reaches in a hundred turns is 12, so **most of this
/// mechanic is human-only and no run of ours exercises it** —
/// `docs/decisions.md` C26.
#[test]
fn the_empire_tax_happiness_term_is_flat_until_the_rate_reaches_twenty() {
    let mut k = Kingdom::new(22);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    k.counties[1].owner = 1;
    k.counties[1].population = 1000;
    for rate in 0..20 {
        k.set_tax_rate(1, rate);
        assert_eq!(
            k.counties[1].tax_hap_other, 0,
            "rate {rate} is inside the flat part of g_taxHappinessOther",
        );
    }
    k.set_tax_rate(1, 20);
    assert_ne!(
        k.counties[1].tax_hap_other, 0,
        "and twenty is where the table finally moves",
    );
}

/// `Tax_RecomputePreview` has no suppression test in it; `Tax_Collect`
/// zeroes the base. So
/// his people *would* pay while the treasury banks nothing. `[D]`.
#[test]
fn a_suppressed_county_still_shows_what_people_would_pay() {
    let mut k = Kingdom::new(23);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 1000;
        c.tax_suppressed = true;
    }
    k.set_tax_rate(1, 30);
    assert!(k.counties[1].tax_shown > 0, "the panel shows the rate's worth");
    let banked = crate::tax::collect(&k.tables, &mut k.counties[1], 0);
    assert_eq!(banked, 0, "and the treasury gets none of it");
    assert_eq!(k.counties[1].tax_collected, 0);
}



