//! **"13 cows are dying and I don't know why."** The second herd report, and
//! the answer is not the first one's.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test cattle_thirteen
//! ```
//!
//! Thirteen is also the head the England opening slaughters — `herdEaten`
//! 13 in all nine unowned counties of `england-turn1.sav`, `DivCeil(456 −
//! 67*5, 10)` — so the first question is whether the ration's thirteen is
//! reaching a *deaths* line. It is not: [`the_slaughtered_thirteen_never_lands_on_a_deaths_line`].
//!
//! What the player's own `lastturn.l2sav` holds is the 16-cow report again.
//! His county: 92 head, eight pastures, 135 milkmaids, **180 people** — down
//! from 435 two seasons earlier — and `herdEaten` **0**, because his herd feeds
//! him on dairy and is never slaughtered. `Herd_BirthsAndDeaths` wants three a
//! head; 135 of 276 is 48 %, and [`l2_kingdom::land::herd_growth`] adds
//! `(100 − 48) / 3 = 17` points to the average-crowding band's 3. Eighteen die
//! and three are born. [`the_players_herd_dies_of_understaffing_not_of_eating`].

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

/// **The thirteen is `herdEaten`, and every screen that can say so says
/// "eaten".**
///
/// `Herd_LabourEstimate` (`0x0044DD4D`) forecasts from `herd − herdEaten` and
/// writes `+0x258 = (births − deaths) − herdEaten`. The three consumers split
/// that back out: `Panel_JobCattle` and the county panel's herd report draw
/// `L2.eng` group 77 index 6 *"Cow deaths expected"* against
/// `herdDeathsExpected` and index 0x1B *"Change due to eating"* against
/// `−herdEaten`, two separate rows.
///
/// Against the fixture: the nine unowned counties slaughter thirteen head and
/// **`herdDeathsExpected` is zero in every one of them**. Nothing a player can
/// read attributes the ration to deaths.
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

        // And the forecast reproduces from the stored county, so the split is
        // the original's arithmetic and not ours.
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
///
/// **Ablation:** the pre-`f1bf2c0f` rule — a ration pass that also debits the
/// store — is the second arm here, and it lands thirteen head low.
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

/// **The player's county, out of his `lastturn.l2sav`.** 92 head on eight
/// pastures at average crowding, 135 milkmaids, 180 people, `herdEaten` 0.
///
/// Three claims. The herd is shrinking; the shrinkage is the staffing and not
/// the pasture, the crowding or the season; and `Herd_LabourEstimate`'s
/// break-even floor — the one number the interface paints red — is above the
/// milkmaids he had, on the fallback arm, because 180 people cannot supply the
/// 276 the herd wants.
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

    // Ablation: the same herd, the same season, three hands a head.
    let tended = herd_growth(T, c.herd, c.fields_cattle, 92 * 3, c.herd_crowding, AUTUMN);
    assert!(tended.net() > 0, "fully tended it grows: {tended:?} against {g:?}");
    assert!(tended.deaths < g.deaths / 4, "{tended:?} against {g:?}");

    let e = herd_labour_estimate(T, &c, AUTUMN);
    assert_eq!((e.wanted, e.useful), (172, 172), "the cannot-break-even arm: floor is the argmax");
    assert!(e.wanted > c.labour[JOB_CATTLE_FARMING], "which is what paints the count red");
    assert!(e.wanted < c.herd * 3, "and no staffing his 180 people could supply breaks even");
}
