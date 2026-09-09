//! Rations — `docs/kingdom.md` §4.2 and §4.3, `Ration_Apply` (`0x0044DF5F`).
//!
//! The rule in one paragraph. A ration level sets a food *requirement* through
//! `g_rationTable`. The standing herd covers part of it for free — five people
//! per head, as cheese, without the animal being slaughtered. What is left is
//! split by the county's `rationSplit` percentage between livestock (one
//! slaughtered animal feeds ten) and grain (one sack feeds six). If either side
//! runs out, the county drops a ration level and tries again. Whatever level
//! survives that becomes `rationAchieved`, and is worth `3L - 8` happiness.
//!
//! # Two places this crate departs from the document
//!
//! **The loop starts at `rationWanted`, not at `rationWanted + 1`.** §4.3 says
//! it *"descends from `rationWanted + 1`"*, which reads naturally as a
//! `do { level--; ... } while (!fits)` — a loop written that way is *entered*
//! at `wanted + 1` and first *evaluated* at `wanted`. Taking the phrase
//! literally breaks §4.3's own reproduction: the unowned county there (pop 456,
//! herd 67, Normal, 100% split) would be fed at Double, which fits its herd
//! comfortably and slaughters 58 head, where the save stores **13**. Starting
//! at `wanted` gives exactly 13. The reproduction is the stronger evidence.
//!
//! **A preview does not spend.** §3.4 has `Ration_Apply` running twice per
//! season, the second time as next season's preview, and §4.3 records that the
//! stored `rationAchieved` is consequently the *next* season's level rather
//! than the one that was applied. [`apply`] spends; [`preview`] writes the same
//! display fields and leaves the store alone. That is a choice, not a finding:
//! §4.3 explicitly says it *"did not untangle which write survives"*, and it is
//! the reason the four player-owned counties in the England turn-one fixture do not
//! reproduce.

use crate::county::County;
use crate::math::{div_ceil, pct};
use crate::tables::{Tables, RATION_LEVEL_COUNT};

/// What feeding a county at one ration level would take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// The level this plan is for, 0..=5.
    pub level: i32,
    /// `DivCeil(people, divisor) * multiplier` — people-worth of food needed.
    pub requirement: i32,
    /// People fed free by the standing herd.
    pub dairy: i32,
    /// Head that would be slaughtered.
    pub heads: i32,
    /// Sacks that would be eaten.
    pub sacks: i32,
}

impl Plan {
    /// True when the county's store can actually cover the plan.
    pub fn fits(&self, herd: i32, grain: i32) -> bool {
        self.heads <= herd && self.sacks <= grain
    }
}

/// `Food_FromDairy` — the standing herd feeds five people per head per season
/// without being slaughtered.
///
/// **`[V]`**, and unusually well corroborated: a strategy guide states *"each
/// portion of cheese being enough to feed 5 people"*, and a player measuring a
/// save reported 80 cows feeding 400 people.
pub fn food_from_dairy(t: &Tables, herd: i32) -> i32 {
    herd.max(0).saturating_mul(t.food.dairy_per_head)
}

/// `Food_HeadsForPeople` — one slaughtered animal feeds ten.
pub fn heads_for_people(t: &Tables, people: i32) -> i32 {
    div_ceil(people, t.food.food_per_head)
}

/// `Food_SacksForPeople` — one sack of grain feeds six.
pub fn sacks_for_people(t: &Tables, people: i32) -> i32 {
    div_ceil(people, t.food.food_per_sack)
}

/// The food requirement at a ration level, `DivCeil(people, divisor) * mult`.
pub fn requirement(t: &Tables, people: i32, level: i32) -> i32 {
    let row = t.ration[clamp_level(level) as usize];
    let (divisor, multiplier) = (row.divisor, row.multiplier);
    div_ceil(people, divisor) * multiplier
}

fn clamp_level(level: i32) -> i32 {
    level.clamp(0, RATION_LEVEL_COUNT as i32 - 1)
}

/// Cost out one ration level, without deciding whether it is affordable.
///
/// `split` is `rationSplit`: the percentage of the remaining requirement taken
/// from livestock rather than grain. The grain side is the *remainder* rather
/// than `Pct(remainder, 100 - split)`, so the two sides always sum back to the
/// whole and a split of 33% does not silently lose a person.
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

/// The people a county has to feed: its population, plus the troops standing in
/// it when *Armies Eat* is on.
///
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
/// ```c
/// f = 0;
/// if (county.herd           > 0) f  = county.herd           * g_dairyPerHead;  /* x5  */
/// if (county.herdAvailable  > 0) f += county.herdAvailable  * g_foodPerHead;   /* x10 */
/// if (county.grainAvailable > 0) f += county.grainAvailable * g_foodPerSack;   /* x6  */
/// ```
///
/// > **`docs/armies.md` §3.3b calls this *"`herd*5 + slaughterable*10 +
/// > grain*6` — the county's whole feeding capacity"*, and the shorthand hides
/// > the part that matters: only the first term is a stock.**
/// > [`County::herd_available`] and [`County::grain_available`] are the
/// > *per-season caps* [`apply`] writes — what this season's ration pass
/// > released — not the larder. Two consequences a reimplementation following
/// > the shorthand would get wrong: **the herd is counted twice**, once as
/// > cheese at five people a head and again as meat at ten; and **the value
/// > moves when the ration slider moves**, which is exactly why `Army_Create`
/// > re-runs the county's food passes before it charges anything.
/// >
/// > Each term is guarded independently, so a negative field contributes
/// > nothing rather than subtracting, and nothing clamps the sum. `[D]`
///
/// There are two sibling functions computing superficially similar sums from
/// *different* county fields (`+0x178`/`+0x17C`, the food actually eaten). Any
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

/// Descend from `rationWanted` to the first level the county can afford.
///
/// Level 0 costs nothing and therefore always fits, so this always terminates
/// with a plan.
pub fn choose(t: &Tables, county: &County, armies_eat: bool) -> Plan {
    let people = people_to_feed(county, armies_eat);
    let mut level = clamp_level(county.ration_wanted);
    loop {
        let p = plan(t, people, level, county.herd, county.ration_split);
        if level == 0 || p.fits(county.herd, county.grain) {
            return p;
        }
        level -= 1;
    }
}

/// Write the outcome's display fields onto the county. Shared by [`apply`] and
/// [`preview`]; the only difference between them is whether the store is
/// debited.
fn record(t: &Tables, county: &mut County, p: Plan) {
    county.ration_achieved = p.level;
    county.herd_eaten = p.heads;
    county.grain_eaten = p.sacks;
    county.herd_available = county.herd;
    county.grain_available = county.grain;
    county.d_hap_ration = t.ration_happiness(p.level);
}

/// The real pass: choose a level, spend the food, and set the ration happiness
/// term.
pub fn apply(t: &Tables, county: &mut County, armies_eat: bool) -> Plan {
    let p = choose(t, county, armies_eat);
    record(t, county, p);
    // Each side is capped at what is actually in store. At the chosen level the
    // plan already fits, so the cap only bites at level 0 with a negative store,
    // which cannot happen - but the original applies it and so does this.
    let heads = p.heads.min(county.herd);
    let sacks = p.sacks.min(county.grain);
    county.herd -= heads;
    county.grain -= sacks;
    p
}

/// The second call: next season's preview. Writes the same display fields and
/// spends nothing.
pub fn preview(t: &Tables, county: &mut County, armies_eat: bool) -> Plan {
    let p = choose(t, county, armies_eat);
    record(t, county, p);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
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

        apply(T, &mut c, false);
        assert_eq!(c.herd_eaten, 13);
        assert_eq!(c.herd, 54);
        assert_eq!(c.ration_achieved, 3);
        assert_eq!(c.d_hap_ration, 1, "Normal is worth +1 happiness");
    }

    /// The reason the loop cannot start at `rationWanted + 1`, spelled out:
    /// that county could afford Double, and the save says it ate at Normal.
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
        // Triple needs 1200, Double 800, Normal 400, Half 200 -> 34 sacks.
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
        let p = apply(T, &mut c, false);
        assert_eq!(p.level, 0);
        assert_eq!(c.d_hap_ration, -8);
        assert_eq!(c.herd_eaten, 0);
        assert_eq!(c.grain_eaten, 0);
    }

    /// Eighty cows feed four hundred people, which is the measurement
    /// `docs/kingdom.md` §4.3 cites.
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

    #[test]
    fn a_preview_writes_the_panel_but_spends_nothing() {
        let mut c = County::new();
        c.population = 456;
        c.herd = 67;
        c.ration_wanted = 3;
        c.ration_split = 100;

        preview(T, &mut c, false);
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
