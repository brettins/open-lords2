//! `g_selectedCounty` (`0x0057C93C`) comes back from the save, and `Event_Post`
//! (`0x00448D7E`) posts for it alone on the first frame.

use l2_game::scenario;
use l2_kingdom::tables::Tables;

// Ablate: scenario.rs `save.i32_at(SELECTED_COUNTY)` -> selected 1, and 312 is posted.
#[test]
fn a_loaded_game_selects_the_saved_county_and_posts_no_stale_event() {
    let Ok(save) = l2_testkit::fixture_save("siege-sieging.sav") else {
        l2_testkit::skip!("no siege-sieging.sav fixture");
    };
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    assert_eq!(game.selected, 4, "the save's g_selectedCounty");
    assert!(game.kingdom.counties[1].event_fired, "county 1 holds an unposted event");
    assert!(!l2_game::message::post_event(&mut game), "the original posts no 312 on the load frame");
}
