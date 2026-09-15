
use l2_kingdom::county::County;
use l2_kingdom::land::{herd_growth, herd_labour_estimate, herd_preview, herd_season_tick};
use l2_kingdom::ration::{self, Sowing};
use l2_kingdom::tables::{Tables, JOB_CATTLE_FARMING, JOB_COUNT};
use l2_scenario::Scenario;
use l2_testkit::FixtureState;

const T: &Tables = &Tables::DEFAULT;
const SEED: u64 = 0x10D_52;
const SPRING: u8 = 1;
const AUTUMN: u8 = 3;

fn england() -> Option<Scenario> {
    match l2_testkit::england_turn1() {
        FixtureState::Ready(s) => Some(Scenario::from_save(&s).expect("the fixture must import")),
        FixtureState::Absent(why) => {
            eprintln!("skipped: {why}");
            None
        }
        FixtureState::WrongGame(why) => panic!("{why}"),
    }
}

/// `Herd_LabourEstimate` (`0x0044DD4D`) forecasts from `herd − herdEaten` and
/// writes `+0x258 = (births − deaths) − herdEaten`. The three consumers split
/// that back out: `Panel_JobCattle` and the county panel's herd report draw
/// `L2.eng` group 77 index 6 *"Cow deaths expected"* against
/// `herdDeathsExpected` and index 0x1B *"Change due to eating"* against
/// `−herdEaten`, two separate rows.
#[test]
fn the_slaughtered_thirteen_never_lands_on_a_deaths_line() {
    let Some(s) = england() else { return };
    let k = s.kingdom(SEED);
    let mut counted = 0;
    for id in s.county_ids() {
        let c = &k.counties[id];
        if c.herd_eaten == 0 {
            continue;
        }
        counted += 1;
        assert_eq!((c.herd, c.herd_eaten), (67, 13), "county {id}");
        assert_eq!(c.herd_deaths_expected, 0, "county {id}: the ration is not a death");
        assert_eq!(
            c.herd_change_expected,
            c.herd_births_expected - c.herd_deaths_expected - c.herd_eaten,
            "county {id}: `+0x258` splits into a farming row and an eating row"
        );

        let mut d = c.clone();
        herd_preview(T, &mut d, SPRING);
        assert_eq!(
            (d.herd_births_expected, d.herd_deaths_expected, d.herd_change_expected),
            (c.herd_births_expected, c.herd_deaths_expected, c.herd_change_expected),
            "county {id}"
        );
        assert_eq!((d.herd_births_expected, d.herd_deaths_expected), (22, 0), "county {id}");
    }
    assert_eq!(counted, 9, "nine unowned counties eat their cattle on turn one");
}

/// **The ration is spent once.** `Ration_ApplyAll` (`0x0044BF04`) shadows
/// `herdEaten` into `+0x190` and writes nothing to the store; `Herd_SeasonTick`
/// (`0x0044D60D`) opens with `herd -= +0x190` and the survivors are what
/// breeds.
#[test]
fn the_season_spends_the_slaughter_once() {
    let Some(s) = england() else { return };
    let k = s.kingdom(SEED);
    let id = s.county_ids().find(|&i| k.counties[i].herd_eaten == 13).expect("an unowned county");

    let mut c = k.counties[id].clone();
    c.herd_eaten_shadow = 0;
    ration::apply(T, &mut c, false, Sowing::NONE);
    assert_eq!((c.herd_eaten, c.herd_eaten_shadow), (13, 13), "priced and shadowed");
    assert_eq!(c.herd, 67, "and nothing is taken yet");

    let opening = c.herd - c.herd_eaten_shadow;
    let g = herd_growth(T, opening, c.fields_cattle, c.labour[JOB_CATTLE_FARMING], c.herd_crowding, SPRING);
    herd_season_tick(T, &mut c, SPRING, AUTUMN);
    assert_eq!(c.herd, opening + g.net(), "one debit, then the growth of what is left");

    let debited_twice = opening - 13 + g.net();
    assert_eq!(c.herd - debited_twice, 13, "the rule that debited in the ration pass too");
}

#[test]
fn the_players_herd_dies_of_understaffing_not_of_eating() {
    let mut c = County::new();
    c.owner = 1;
    c.population = 180;
    c.pop_band = 6;
    c.herd = 92;
    c.herd_eaten = 0;
    c.fields_cattle = 8;
    c.herd_crowding = 20;
    c.labour = [0; JOB_COUNT];
    c.labour[JOB_CATTLE_FARMING] = 135;

    let g = herd_growth(T, c.herd, c.fields_cattle, 135, c.herd_crowding, AUTUMN);
    assert_eq!((g.births, g.deaths), (3, 18), "the season he was looking at");
    assert!(g.net() < 0, "and the herd is going down: {}", g.net());

    let tended = herd_growth(T, c.herd, c.fields_cattle, 92 * 3, c.herd_crowding, AUTUMN);
    assert!(tended.net() > 0, "fully tended it grows: {tended:?} against {g:?}");
    assert!(tended.deaths < g.deaths / 4, "{tended:?} against {g:?}");

    let e = herd_labour_estimate(T, &c, AUTUMN);
    assert_eq!((e.wanted, e.useful), (172, 172), "the cannot-break-even arm: floor is the argmax");
    assert!(e.wanted > c.labour[JOB_CATTLE_FARMING], "which is what paints the count red");
    assert!(e.wanted < c.herd * 3, "and no staffing his 180 people could supply breaks even");
}
