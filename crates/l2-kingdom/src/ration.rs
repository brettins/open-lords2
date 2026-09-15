//! Rations — `docs/kingdom.md` §4.2 and §4.3, `Ration_Apply` (`0x0044DF5F`).
//!
//! **Neither call spends.** §3.4 has `Ration_Apply` running twice per season,
//! the second time as next season's preview, and §4.3 records that the stored
//! `rationAchieved` is consequently the *next* season's level
//! one that was applied. `Ration_Apply` (`0x0044DF5F`) debits no store at all:
//!
//! [`apply`] is `Ration_ApplyAll`'s body and shadows the cost into
//! `+0x18C`/`+0x190`, [`preview`] does not, and `Grain_SeasonTick` /
//! `Herd_SeasonTick` take the food out. That is a choice, not a finding:

use crate::county::County;
use crate::math::{div_ceil, pct};
use crate::tables::{Season, Tables, RATION_LEVEL_COUNT};

/// Every call in the binary passes `g_season` except `Ration_ApplyAll`
/// (`0x0044BF04`), which passes `g_seasonPrev` — so the season here is the one
/// *that call* was handed, not always the turn's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sowing {
    pub season: Option<Season>,
    pub advanced_farming: bool,
}

impl Sowing {
    pub const NONE: Sowing = Sowing { season: None, advanced_farming: false };

    pub fn new(season: Season, advanced_farming: bool) -> Sowing {
        Sowing { season: Some(season), advanced_farming }
    }

    pub fn from_index(season: u8, advanced_farming: bool) -> Sowing {
        Sowing { season: Season::from_index(season), advanced_farming }
    }
}

/// **The seed corn is not food.** `Ration_Apply` (`0x0044DF5F`) opens with
///
/// Season 4 is Winter and the spending pass is handed `g_seasonPrev`, so the
/// turn the gate fires on is the turn `Grain_SeasonTick` (`0x0044C8AE`) sows,
/// which is Spring. Without it a Spring county is offered its own seed as a
/// meal, eats it, and sows what the ration pass left.
///
/// `Grain_Sow` (`0x0044CFE1`) is called for its return value and is otherwise
/// pure but for [`County::sow_shortfall`] (`+0x1A7`), which it writes here as
/// from every other caller. Nothing reads that flag between this write and the
/// sowing that overwrites it, so the write is faithful rather than load-bearing
/// — the argument [`crate::land::grain::sow_seed`] records for the labour
/// estimate. **`[V]`** on the reservation, **`[D]`** on the flag being
/// unobservable.
pub fn seed_reserved(t: &Tables, county: &mut County, sowing: Sowing) -> i32 {
    if sowing.season != Some(Season::Winter) {
        return 0;
    }
    let labour = crate::land::grain_labour(t, county);
    let store = county.grain;
    crate::land::sow_sacks(t, county, store, labour, sowing.advanced_farming)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    pub level: i32,
    pub requirement: i32,
    pub dairy: i32,
    pub heads: i32,
    pub sacks: i32,
}

impl Plan {
    pub fn fits(&self, herd: i32, grain: i32) -> bool {
        self.heads <= herd && self.sacks <= grain
    }
}

/// **`[V]`**, and unusually well corroborated: a strategy guide states *"each
/// portion of cheese being enough to feed 5 people"*, and a player measuring a
/// save reported 80 cows feeding 400 people.
pub fn food_from_dairy(t: &Tables, herd: i32) -> i32 {
    herd.max(0).saturating_mul(t.food.dairy_per_head)
}

/// ```c
/// county[+0x16C] = county.herd      * g_dairyPerHead;   /* the standing herd  */
/// county[+0x170] = county.grainEaten * g_foodPerSack;   /* the grain eaten    */
/// county[+0x174] = county.herdEaten  * g_foodPerHead;   /* the beasts killed  */
/// ```
pub fn people_fed(t: &Tables, county: &County) -> (i32, i32, i32) {
    (
        county.grain_eaten.max(0).saturating_mul(t.food.food_per_sack),
        county.herd_eaten.max(0).saturating_mul(t.food.food_per_head),
        food_from_dairy(t, county.herd),
    )
}

pub fn heads_for_people(t: &Tables, people: i32) -> i32 {
    div_ceil(people, t.food.food_per_head)
}

pub fn sacks_for_people(t: &Tables, people: i32) -> i32 {
    div_ceil(people, t.food.food_per_sack)
}

pub fn requirement(t: &Tables, people: i32, level: i32) -> i32 {
    let row = t.ration[clamp_level(level) as usize];
    let (divisor, multiplier) = (row.divisor, row.multiplier);
    div_ceil(people, divisor) * multiplier
}

fn clamp_level(level: i32) -> i32 {
    level.clamp(0, RATION_LEVEL_COUNT as i32 - 1)
}

pub fn plan(t: &Tables, people: i32, level: i32, herd: i32, split: i32) -> Plan {
    let level = clamp_level(level);
    let requirement = requirement(t, people, level);
    let dairy = food_from_dairy(t, herd);
    let remainder = (requirement - dairy).max(0);
    let from_livestock = pct(remainder, split.clamp(0, 100));
    let from_grain = remainder - from_livestock;
    Plan {
        level,
        requirement,
        dairy: dairy.min(requirement),
        heads: heads_for_people(t, from_livestock),
        sacks: sacks_for_people(t, from_grain),
    }
}

/// **`[I]` on the enemy half.** `docs/kingdom.md` §1.3 lists `+0x198` and `+0x19C`
/// together as *"friendly / enemy troops … added to the food requirement when
/// Armies Eat is on"* and does not say whether both are added. Both are added
/// here; an occupying army foraging the county it stands in is the reading that
/// makes the field pair worth storing.
pub fn people_to_feed(county: &County, armies_eat: bool) -> i32 {
    if armies_eat {
        county.population + county.friendly_troops + county.enemy_troops
    } else {
        county.population
    }
}

/// `Food_Available` (`0x0044E7B4`) — the number [`crate::unit::starve`] tests
/// an army's size against.
///
/// There are two sibling functions computing superficially similar sums from
/// *different* county fields (`+0x178`/`+0x17C`, the food eaten). Any
/// reading that pattern-matches on "the food function" is `docs/decisions.md`
/// C3 waiting to happen.
pub fn food_available(t: &Tables, county: &County) -> i32 {
    let mut food = 0i64;
    if county.herd > 0 {
        food += county.herd as i64 * t.food.dairy_per_head as i64;
    }
    if county.herd_available > 0 {
        food += county.herd_available as i64 * t.food.food_per_head as i64;
    }
    if county.grain_available > 0 {
        food += county.grain_available as i64 * t.food.food_per_sack as i64;
    }
    food.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

pub fn choose(t: &Tables, county: &County, armies_eat: bool) -> Plan {
    choose_within(t, county, armies_eat, county.grain)
}

fn choose_within(t: &Tables, county: &County, armies_eat: bool, grain: i32) -> Plan {
    let people = people_to_feed(county, armies_eat);
    let mut level = clamp_level(county.ration_wanted);
    loop {
        let p = plan(t, people, level, county.herd, county.ration_split);
        if level == 0 || p.fits(county.herd, grain) {
            return p;
        }
        level -= 1;
    }
}

/// `Ration_Apply` (`0x0044DF5F`) proper — everything the original's function
/// writes. Shared by [`apply`] and [`preview`]; the only difference between
/// them is whether the shadow pair is written after it.
fn record(t: &Tables, county: &mut County, p: Plan, seed: i32) {
    county.ration_achieved = p.level;
    county.herd_available = county.herd;
    county.grain_available = county.grain - seed;
    county.herd_eaten = p.heads.min(county.herd_available);
    county.grain_eaten = p.sacks.min(county.grain_available);
    county.d_hap_ration = t.ration_happiness(p.level);
}

/// `Ration_ApplyAll` (`0x0044BF04`) for one county: choose a level, set the
/// ration happiness term, and **shadow** what was priced.
///
/// `Ration_Apply` (`0x0044DF5F`) itself never debits a store — its whole loop
/// reads `Food_Available` and writes display fields. `Ration_ApplyAll` follows
/// it with `+0x18C = grainEaten; +0x190 = herdEaten;`, and the debit happens
/// two passes later, at the top of `Grain_SeasonTick` (`0x0044C8AE`) and
/// `Herd_SeasonTick` (`0x0044D60D`). The shadow is not redundant with
/// `+0x178`/`+0x17C`: [`preview`] overwrites those with next season's forecast
/// before either tick runs (`docs/decisions.md` C20), and in the fixture every
/// unowned county stores `herdEaten` 13 against a shadow of 0.
pub fn apply(t: &Tables, county: &mut County, armies_eat: bool, sowing: Sowing) -> Plan {
    let seed = seed_reserved(t, county, sowing);
    let p = choose_within(t, county, armies_eat, county.grain - seed);
    record(t, county, p, seed);
    county.grain_eaten_shadow = county.grain_eaten;
    county.herd_eaten_shadow = county.herd_eaten;
    p
}

pub fn preview(t: &Tables, county: &mut County, armies_eat: bool, sowing: Sowing) -> Plan {
    let seed = seed_reserved(t, county, sowing);
    let p = choose_within(t, county, armies_eat, county.grain - seed);
    record(t, county, p, seed);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &Tables = &Tables::DEFAULT;

    /// **`docs/kingdom.md` §4.3's reproduction, exactly.** In the shipped
    /// `lastturn.sav` an unowned county has population 456, herd 67, ration
    /// *Normal*, split 100%: `456 - 67*5 = 121` people left to feed and
    /// `DivCeil(121, 10) = 13` head. The stored `+0x17C` is **13**, for all ten
    /// unowned counties.
    #[test]
    fn the_unowned_county_food_split_reproduces_from_the_save() {
        let mut c = County::new();
        c.population = 456;
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;
        c.grain = 0;

        let p = choose(T, &c, false);
        assert_eq!(p.level, 3, "Normal");
        assert_eq!(p.requirement, 456);
        assert_eq!(p.dairy, 335, "67 head x 5 people");
        assert_eq!(p.heads, 13, "the stored +0x17C");
        assert_eq!(p.sacks, 0);

        apply(T, &mut c, false, Sowing::NONE);
        assert_eq!(c.herd_eaten, 13);
        assert_eq!(c.herd_eaten_shadow, 13, "shadowed for `Herd_SeasonTick` to spend");
        assert_eq!(c.herd, 67, "`Ration_Apply` debits nothing");
        crate::land::herd_season_tick(T, &mut c, 1, 2);
        assert_eq!(c.herd_eaten_shadow, 13, "and the shadow survives the tick");
        assert_eq!(c.ration_achieved, 3);
        assert_eq!(c.d_hap_ration, 1, "Normal is worth +1 happiness");
    }

    #[test]
    fn starting_one_level_above_wanted_would_contradict_the_save() {
        let people = 456;
        let herd = 67;
        let double = plan(T, people, 4, herd, 100);
        assert!(double.fits(herd, 0), "Double is affordable for this county");
        assert_eq!(double.heads, 58, "and would slaughter 58 head, not 13");

        let mut c = County::new();
        c.population = people;
        c.herd = herd;
        c.ration_wanted = 3;
        c.ration_split = 100;
        assert_eq!(choose(T, &c, false).level, 3, "so the loop must begin at wanted");
    }

    #[test]
    fn a_county_that_cannot_feed_itself_drops_a_level_at_a_time() {
        let mut c = County::new();
        c.population = 400;
        c.herd = 0;
        c.grain = 40; // 40 sacks feeds 240 people
        c.ration_wanted = 5;
        c.ration_split = 0; // all grain

        let p = choose(T, &c, false);
        assert_eq!(p.level, 2, "Half is the first level 40 sacks covers");
        assert_eq!(p.sacks, sacks_for_people(T, 200));
        assert!(p.sacks <= c.grain);
    }

    #[test]
    fn a_county_with_nothing_at_all_falls_to_none_and_takes_the_happiness_hit() {
        let mut c = County::new();
        c.population = 400;
        c.herd = 0;
        c.grain = 0;
        c.ration_wanted = 3;
        let p = apply(T, &mut c, false, Sowing::NONE);
        assert_eq!(p.level, 0);
        assert_eq!(c.d_hap_ration, -8);
        assert_eq!(c.herd_eaten, 0);
        assert_eq!(c.grain_eaten, 0);
    }

    #[test]
    fn eighty_cows_feed_four_hundred_people_for_free() {
        assert_eq!(food_from_dairy(T, 80), 400);
        let mut c = County::new();
        c.population = 400;
        c.herd = 80;
        c.ration_wanted = 3;
        c.ration_split = 100;
        let p = choose(T, &c, false);
        assert_eq!(p.heads, 0, "the dairy alone covers Normal rations");
        assert_eq!(p.sacks, 0);
    }

    #[test]
    fn the_two_sides_of_the_split_always_sum_back_to_the_whole() {
        for split in 0..=100 {
            let p = plan(T, 1000, 3, 0, split);
            let from_livestock = pct(1000, split);
            let from_grain = 1000 - from_livestock;
            assert_eq!(p.heads, heads_for_people(T, from_livestock), "split {split}");
            assert_eq!(p.sacks, sacks_for_people(T, from_grain), "split {split}");
        }
    }

    #[test]
    fn armies_eat_only_when_the_option_is_on() {
        let mut c = County::new();
        c.population = 400;
        c.friendly_troops = 100;
        c.enemy_troops = 50;
        assert_eq!(people_to_feed(&c, false), 400);
        assert_eq!(people_to_feed(&c, true), 550);
    }

    /// **The seed corn is not on the menu.** `Ration_Apply` (`0x0044DF5F`)
    /// subtracts `Grain_Sow` (`0x0044CFE1`)'s answer from `grainAvailable`
    /// under its `season == 4` gate, and `Ration_ApplyAll` (`0x0044BF04`)
    /// hands it `g_seasonPrev` — so the gate fires on the turn the county
    /// sows. A county holding its seed plus four sacks feeds its people on the
    /// four.
    #[test]
    fn a_sowing_county_eats_what_is_over_the_seed_and_not_the_seed() {
        let mut c = County::new();
        c.population = 60; // Normal wants 60 people fed, all of it from grain
        c.ration_wanted = 3;
        c.ration_split = 0;
        c.fields_grain = 5;
        c.labour[T.job.grain_farming] = 5_000; // labour never binds here
        c.grain = 54;

        let seed = crate::land::sow_sacks(T, &mut c.clone(), 54, 5_000, false);
        assert_eq!(seed, 50, "5 fields at the full 10 sacks");

        let spring = Sowing::new(crate::tables::Season::Winter, false);
        let mut sowing = c.clone();
        apply(T, &mut sowing, false, spring);
        assert_eq!(sowing.grain_available, 4, "54 - 50");
        assert!(sowing.grain_eaten <= 4, "the eaters get the little");
        assert!(
            c.grain - sowing.grain_eaten >= seed,
            "and the seed survives the meal"
        );

        let mut whole = c.clone();
        apply(T, &mut whole, false, Sowing::NONE);
        assert_eq!(whole.grain_available, 54, "the ablation offers the store whole");
        assert_eq!(whole.grain_eaten, 10, "Normal rations, seed and all");
        assert!(
            whole.ration_achieved > sowing.ration_achieved,
            "the reservation costs the county a ration level"
        );
    }

    #[test]
    fn a_preview_writes_the_panel_but_spends_nothing() {
        let mut c = County::new();
        c.population = 456;
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;

        preview(T, &mut c, false, Sowing::NONE);
        assert_eq!(c.herd, 67, "a preview must not slaughter anything");
        assert_eq!(c.herd_eaten, 13, "but it does write the display field");
        assert_eq!(c.ration_achieved, 3);
    }

    #[test]
    fn every_ration_level_costs_more_than_the_one_below_it() {
        let mut last = -1;
        for level in 0..RATION_LEVEL_COUNT as i32 {
            let r = requirement(T, 1000, level);
            assert!(r > last, "level {level} should cost more");
            last = r;
        }
    }

    #[test]
    fn an_out_of_range_ration_level_clamps_rather_than_panicking() {
        assert_eq!(requirement(T, 1000, -5), requirement(T, 1000, 0));
        assert_eq!(requirement(T, 1000, 99), requirement(T, 1000, 5));
        let mut c = County::new();
        c.population = 100;
        c.ration_wanted = 99;
        assert!(choose(T, &c, false).level <= 5);
    }
}
