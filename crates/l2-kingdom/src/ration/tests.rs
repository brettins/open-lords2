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
    let mut c = County::new();
    c.population = 456;
    c.herd = 67;
    c.herd_available = 67;
    c.ration_wanted = 3;
    c.ration_split = 100;

    // `Food_Available` (`0x0044E7B4`): 67*5 + 67*10 = 1005.
    assert_eq!(food_available(T, &c), 1005);
    assert!(requirement(T, 456, 4) <= 1005, "Double clears the total");
    assert_eq!(plan(T, 456, 4, 67, 100).heads, 58, "and would slaughter 58 head, not 13");
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

/// **The fit is on the total, and the shortfall is measured in food.**
/// `Ration_Apply` (`0x0044DF5F`) tests `local_2c <= iVar3` against
/// `Food_Available` (`0x0044E7B4`), then `local_2c <= iVar5 + iVar4 +
/// iVar3` against the dairy plus what `herdEaten`/`grainEaten` fetched.
/// Here the split asks for two sacks a county without grain cannot give,
/// and the rounding-up of `Food_HeadsForPeople` covers the gap exactly:
/// 200 from cheese + 3 head x 10 = 230, the whole requirement.
///
/// *Ablation*: replace the `demand(..) <= total` test with the two-sided
/// `heads <= herd && sacks <= grain` and this county is fed Half, `-2`.
#[test]
fn a_missing_sack_does_not_cost_a_level_when_the_total_still_covers_it() {
    let mut c = County::new();
    c.population = 230;
    c.herd = 40;
    c.grain = 0;
    c.ration_split = 70;
    c.ration_wanted = 3;

    let p = choose(T, &c, false);
    assert_eq!(p.level, 3, "Normal");
    assert_eq!((p.heads, p.sacks), (3, 2), "and it asks for two sacks it has not got");

    apply(T, &mut c, false, Sowing::NONE);
    assert_eq!(c.herd_eaten, 3);
    assert_eq!(c.grain_eaten, 0, "capped at grainAvailable");
    assert_eq!(c.ration_achieved, 3);
    assert_eq!(c.d_hap_ration, 1, "Normal, not Half");
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

/// **The panel's dairy line is not `Food_FromDairy`.** `00440000.c:8019`
/// writes `+0x16C = herd * g_dairyPerHead` bare; `Food_FromDairy`
/// (`0x0044E5BB`) guards the same product at `00440000.c:8040`.
///
/// *Ablation*: put `food_from_dairy(t, county.herd)` back as
/// [`people_fed`]'s third term and the dairy line reads 0 here, not -25.
#[test]
fn the_panel_multiplies_the_herd_where_food_from_dairy_would_guard_it() {
    let mut c = County::new();
    c.herd = -5;
    c.grain_eaten = -3;
    c.herd_eaten = -2;
    assert_eq!(food_from_dairy(T, c.herd), 0, "the guarded site");
    assert_eq!(people_fed(T, &c), (-18, -20, -25), "the three bare products");
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
