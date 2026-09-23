#![allow(unused_imports)]
use super::*;



/// **The third control on the ration panel, found by enumerating the class
/// `Ration_IncreaseCounty` (`0x0043A23F`) is
/// `rationWanted++`, `Ration_Apply`, `County_RefreshEstimates`,
/// `Panel_Ration` — so asking for more food changes what the county is
/// recorded as eating, on the spot.
#[test]
fn asking_for_more_food_changes_what_the_county_eats_at_once() {
    let mut k = Kingdom::new(31);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 2000;
        c.pop_band = 80;
        c.herd = 100;
        c.grain = 2000;
        c.ration_split = 0; // all grain, so the level alone moves the number
        c.ration_wanted = 1;
    }
    k.set_ration_wanted(1, 1);
    let (level, sacks) = (k.counties[1].ration_achieved, k.counties[1].grain_eaten);

    k.set_ration_wanted(1, 5);
    assert!(k.counties[1].ration_achieved > level, "the county can afford more and takes it");
    assert!(
        k.counties[1].grain_eaten > sacks,
        "and the Eaten row moves with it: {} was {sacks}",
        k.counties[1].grain_eaten,
    );
}

#[test]
fn a_click_at_the_top_of_the_ration_scale_still_recomputes() {
    let mut k = Kingdom::new(32);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 500;
        c.grain = 500;
        c.ration_wanted = 5;
    }
    k.counties[1].ration_achieved = -7;
    assert!(!k.set_ration_wanted(1, 9), "the level did not move");
    assert_eq!(k.counties[1].ration_wanted, 5, "and is clamped to the table");
    assert_ne!(
        k.counties[1].ration_achieved, -7,
        "but the pass ran anyway, because the guard is only on the increment",
    );
}

#[test]
fn the_ration_control_writes_wanted_and_the_pass_writes_achieved() {
    let mut k = Kingdom::new(33);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 1000;
        c.herd = 0;
        c.grain = 0; // nothing in store at all
    }
    k.set_ration_wanted(1, 5);
    assert_eq!(k.counties[1].ration_wanted, 5, "he asked for triple");
    assert_eq!(k.counties[1].ration_achieved, 0, "and the county feeds nobody");
}

fn a_county_that_eats_both() -> Kingdom {
    let mut k = Kingdom::new(9);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    let c = &mut k.counties[1];
    c.owner = 1;
    c.population = 2000;
    c.pop_band = 80;
    c.herd = 100;
    c.grain = 900;
    c.ration_wanted = 3;
    c.ration_split = 50;
    k
}

#[test]
fn moving_the_split_moves_the_numbers_the_panel_prints() {
    let mut k = a_county_that_eats_both();
    k.set_ration_split(1, 100, true);
    let (herd_all, grain_all) = (k.counties[1].herd_eaten, k.counties[1].grain_eaten);

    k.set_ration_split(1, 0, true);
    let (herd_none, grain_none) = (k.counties[1].herd_eaten, k.counties[1].grain_eaten);

    assert!(herd_all > 0, "all-livestock eats the herd");
    assert_eq!(herd_none, 0, "all-grain eats none of it");
    assert!(grain_none > grain_all, "and the grain takes the whole requirement instead");
}

#[test]
fn dragging_the_split_does_not_feed_anybody() {
    let mut k = a_county_that_eats_both();
    let (herd, grain) = (k.counties[1].herd, k.counties[1].grain);
    for split in 0..=100 {
        k.set_ration_split(1, split, true);
    }
    assert_eq!((k.counties[1].herd, k.counties[1].grain), (herd, grain));
}

#[test]
fn a_track_jump_springs_back_where_an_arrow_keeps_walking() {
    let mut k = Kingdom::new(11);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 30;
        c.pop_band = 2;
        c.herd = 2;
        c.grain = 100;
        c.ration_wanted = 3;
        c.ration_split = 50;
    }
    k.set_ration_split(1, 50, true); // settle herd_eaten for where we start
    let settled = k.counties[1].ration_split;
    let eaten = k.counties[1].herd_eaten;
    assert_eq!(settled, 50);
    assert!(eaten > 0, "the search only runs on a county that is eating its herd");

    let mut track = k.clone();
    let mut arrow = k.clone();
    track.set_ration_split(1, settled + 1, true);
    arrow.set_ration_split(1, settled + 1, false);

    assert_eq!(
        track.counties[1].ration_split, settled,
        "a track jump that finds no split worth having puts the old one back",
    );
    assert_eq!(track.counties[1].herd_eaten, eaten, "and the numbers with it");
    assert!(
        arrow.counties[1].ration_split > settled + 1,
        "an arrow does not stop at the request: it walks on until the herd moves, and \
         ended at {} rather than past {}",
        arrow.counties[1].ration_split,
        settled + 1,
    );
    assert_ne!(
        arrow.counties[1].herd_eaten, eaten,
        "and it stops at the first split that changes something",
    );
}


#[test]
fn the_search_terminates_on_a_county_whose_herd_never_changes() {
    let mut k = Kingdom::new(12);
    k.set_county_count(2);
    k.realms[1].in_play = true;
    {
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 100;
        c.pop_band = 4;
        c.herd = 1;
        c.grain = 1000;
        c.ration_wanted = 3;
        c.ration_split = 40;
    }
    k.set_ration_split(1, 60, true);
    assert!((0..=100).contains(&k.counties[1].ration_split));
    k.set_ration_split(1, 20, false);
    assert!((0..=100).contains(&k.counties[1].ration_split));
}

#[test]
fn the_split_of_a_county_out_of_range_is_refused() {
    let mut k = a_county_that_eats_both();
    assert!(!k.set_ration_split(0, 50, true), "county 0 is not a county");
    assert!(!k.set_ration_split(99, 50, true), "and neither is one past the count");
}

