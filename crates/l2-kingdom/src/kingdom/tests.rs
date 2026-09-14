#![allow(unused_imports)]
use super::*;
use crate::ai;
use crate::county::{County, MAX_COUNTIES, MAX_COUNTY_ID};
use crate::event;
use crate::happiness;
use crate::health;
use crate::industry;
use crate::land;
use crate::phase::{Pass, Phase, PhaseTick, TurnMachine, SEASON_PIPELINE};
use crate::population;
use crate::ration;
use crate::realm::{Realm, MAX_REALMS};
use crate::report::{Message, SeasonReport};
use crate::tables::{Commodity, Season, Tables};
use crate::tax;
use crate::unrest;
use crate::weather;
use l2_net::Pcg32;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Weather;

    #[test]
    fn a_new_kingdom_holds_seventeen_counties_and_six_realms_with_slot_zero_unused() {
        let k = Kingdom::new(1);
        assert_eq!(k.counties.len(), 17);
        assert_eq!(k.realms.len(), 6);
        assert_eq!(k.county_count, 0);
    }

    #[test]
    fn the_county_count_is_bounded_by_the_array() {
        let mut k = Kingdom::new(1);
        assert!(k.set_county_count(14), "the England map");
        assert!(k.set_county_count(16), "the last usable id");
        assert!(!k.set_county_count(17), "index 0 is not a county");
        assert_eq!(k.county_count, 16, "and a refused count changes nothing");
    }

    /// **`docs/kingdom.md` §9 point 1 and §3.3.** A new game starts in Winter
    /// 1268: `Game_NewGame` sets season 3, next 4, year 1267, next 1268, and
    /// one `Season_Advance` rolls the year.
    #[test]
    fn a_new_game_starts_in_winter_1268_on_turn_one() {
        let mut k = Kingdom::new(1);
        k.set_county_count(14);
        k.start_new_game();
        assert_eq!(k.season, 4);
        assert_eq!(k.season(), Some(Season::Winter));
        assert_eq!(k.year, 1268);
        assert_eq!(k.turn_count, 1);
        assert_eq!(k.season_next, 1, "Spring is next");
        assert_eq!(k.season_prev, 3);
    }

    /// The clock, walked for four years. The year rolls in the same call that
    /// begins Winter — see the errata note in the crate documentation, which is
    /// **not** what §3.3's prose says.
    #[test]
    fn the_year_rolls_in_the_call_that_begins_winter() {
        let mut k = Kingdom::new(1);
        k.start_new_game();
        let mut seen = vec![(k.season, k.year)];
        for _ in 0..8 {
            k.advance_season();
            seen.push((k.season, k.year));
        }
        assert_eq!(
            seen,
            vec![
                (4, 1268),
                (1, 1268),
                (2, 1268),
                (3, 1268),
                (4, 1269),
                (1, 1269),
                (2, 1269),
                (3, 1269),
                (4, 1270),
            ]
        );
    }

    #[test]
    fn the_season_cycles_one_two_three_four_forever() {
        let mut k = Kingdom::new(1);
        k.start_new_game();
        let mut last = k.season;
        for _ in 0..40 {
            k.advance_season();
            let expected = if last == 4 { 1 } else { last + 1 };
            assert_eq!(k.season, expected);
            last = k.season;
        }
    }

    #[test]
    fn a_season_runs_every_pass_in_the_documented_order() {
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        let report = k.advance_season();
        assert_eq!(report.passes, SEASON_PIPELINE.to_vec());
    }

    /// **A new game runs no phase-7 pass.** `Game_NewGame` (`0x00497CED`)
    /// calls `Season_Advance` and `Score_RankRealms`; `Mercenary_AdvanceAll`,
    /// `Units_ResetMoves` (`0x004651B9`) and `Diplo_ReconcileAlliances`
    /// (`0x004A1847`) are `Turn_Tick`'s phase-7 arm, which a game that has not
    /// started has not reached. End Turn still runs all three, and that is the
    /// second half here.
    ///
    /// Ablation: drop either name from the `matches!` in [`Kingdom::start_new_game`]
    /// — red, *"a new game ran a phase-7 pass"*.
    #[test]
    fn a_new_game_runs_no_phase_seven_pass_and_a_season_end_runs_all_three() {
        let phase7 =
            [Pass::MercenaryAdvance, Pass::UnitsResetMoves, Pass::ReconcileAlliances];
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        let new = k.start_new_game();
        for pass in phase7 {
            assert!(!new.passes.contains(&pass), "a new game ran a phase-7 pass: {pass:?}");
        }
        assert_eq!(
            new.passes.len(),
            SEASON_PIPELINE.len() - phase7.len(),
            "and it ran every other pass"
        );
        let ended = k.advance_season();
        for pass in phase7 {
            assert!(ended.passes.contains(&pass), "End Turn stopped running {pass:?}");
        }
    }

    /// The whole point of the phase machine: only phase 7 advances the season.
    #[test]
    fn only_the_seventh_phase_advances_the_season() {
        let mut k = Kingdom::new(1);
        k.set_county_count(2);
        let mut seasons = 0;
        for _ in 0..64 {
            let (tick, report) = k.tick(true);
            if report.is_some() {
                seasons += 1;
                assert_eq!(tick.phase, Phase::SeasonEnd);
            }
        }
        assert!(seasons >= 3, "several full cycles should have run");
        assert_eq!(k.turn_count as usize, seasons);
    }

    /// **`Ai_ManageFarmsAll` (`0x0049A990`) farms every realm with strength
    /// and no person behind it, at the stall, and nobody else.**
    ///
    /// Three realms each hold one county set up identically — a merchant
    /// standing in it, no grain, a herd above the grazing style's cattle floor,
    /// and 4,000 crowns in the treasury. Realm 2 is an AI lord (lord 1, style 1,
    /// whose only grain line is `if (grain < 100) buy 400`), realm 3 is a
    /// person, and realm 4 is an AI at strength 0. Only realm 2 buys.
    ///
    /// **The treasury is 4,000 because `Ai_TradeForCounty` (`0x0049E39B`) now
    /// shops first**: the Knight's floor of 1,000 is cleared, the county makes
    /// crossbows (weapon type 0, good 12) at 24 + `Pct(24, 100)` = 48, and
    /// `Ai_BuyGoodDownTo` halves 100 to 50 for 2,400. That leaves exactly the
    /// 1,600 the grain lot costs, so both orders land and the test still says
    /// what it said.
    ///
    /// *Ablation*: empty `ai_manage_farms_all`'s loop and the first assertion
    /// goes red; drop its `!realm.is_human` and the realm-3 one does; drop the
    /// `strength != 0` and the realm-4 one does.
    #[test]
    fn the_season_head_farms_ai_realms_with_strength_and_leaves_the_rest() {
        let mut k = Kingdom::new(1);
        assert!(k.set_county_count(3));
        for (id, realm, human, strength) in [(1usize, 2usize, false, 1u8), (2, 3, true, 1), (3, 4, false, 0)] {
            let r = &mut k.realms[realm];
            r.in_play = true;
            r.strength = strength;
            r.is_human = human;
            r.lord = 1;
            r.gold = 4_000;
            let c = &mut k.counties[id];
            c.owner = realm as u8;
            c.population = 500;
            c.grain = 0;
            c.herd = 50;
            c.merchant_count = 1;
            c.merchant_unit = id as u8;
            let mut m = crate::unit::Unit::new(crate::unit::UnitKind::Merchant, 6, 8, 8);
            m.morale = 100;
            m.county = id as u8;
            k.campaign.units.put(id, m);
        }
        assert_eq!(k.tables.ai_farm_style(1), Some(1), "lord 1 grazes");

        k.ai_manage_farms_all();

        assert_eq!((k.counties[1].grain, k.realms[2].gold), (400, 0), "the AI lord buys 50 crossbows for 2,400 and 400 sacks for 1,600");
        assert_eq!(k.realms[2].weapons[0], 50);
        assert_eq!(k.realms[2].trade_spent_a, 4_000);
        assert_eq!((k.counties[2].grain, k.realms[3].gold), (0, 4_000), "a persons realm is not farmed");
        assert_eq!((k.counties[3].grain, k.realms[4].gold), (0, 4_000), "a realm at strength 0 is not farmed");
    }

    /// The determinism property lockstep depends on. Two kingdoms built the
    /// same way and driven the same way must stay bit-identical, generator
    /// included.
    #[test]
    fn two_identical_kingdoms_stay_identical_for_forty_seasons() {
        let build = || {
            let mut k = Kingdom::new(0xA11CE);
            k.options = Options { difficulty: 2, advanced_farming: true, armies_eat: true, ..Options::default() };
            k.set_county_count(14);
            for id in 1..=5 {
                k.realms[id].in_play = true;
                k.realms[id].lord = (id - 1) as u8;
            }
            k.realms[1].is_human = true;
            for id in 1..=14 {
                let owner = if id <= 4 { 1 } else if id <= 8 { 2 } else { 0 };
                let c = &mut k.counties[id];
                c.owner = owner;
                c.population = 400 + id as i32 * 7;
                c.happiness = 60;
                c.health_meter = 65;
                c.health_band = crate::tables::health_band(65);
                c.herd = 60;
                c.grain = 300;
                c.fields_grain = 6;
                c.fields_fallow = 3;
                c.labour[crate::tables::JOB_GRAIN_FARMING] = 500;
                c.labour[crate::tables::JOB_WOOD_CUTTING] = 100;
                c.castle_type = crate::tables::CASTLE_STARTING_TYPE;
                c.tax_rate = 7;
                c.dryness = 40;
                for n in 1..=14u8 {
                    if n as usize != id {
                        c.add_neighbour(n);
                    }
                }
            }
            k.start_new_game();
            k
        };

        let (mut a, mut b) = (build(), build());
        for season in 0..40 {
            let ra = a.advance_season();
            let rb = b.advance_season();
            assert_eq!(ra, rb, "reports diverged at season {season}");
            assert_eq!(a, b, "state diverged at season {season}");
        }
    }

    /// A kingdom with no counties still keeps its clock, and never panics.
    #[test]
    fn an_empty_kingdom_advances_its_clock_and_nothing_else() {
        let mut k = Kingdom::new(1);
        k.start_new_game();
        for _ in 0..20 {
            let report = k.advance_season();
            assert!(report.messages.is_empty());
        }
        assert_eq!(k.turn_count, 21);
    }

    /// **`docs/kingdom.md` §9 point 3.** With Advanced Farming off, every
    /// county ends the season Cloudy with zero fertility — the two overrides
    /// §7.2 and §7.3 say the option forces.
    #[test]
    fn basic_farming_forces_cloudy_and_zero_fertility_across_the_map() {
        let mut k = Kingdom::new(1);
        k.set_county_count(14);
        for id in 1..=14 {
            k.counties[id].fields_fallow = 5;
            k.counties[id].fields_grain = 1;
            k.counties[id].dryness = 100;
        }
        k.start_new_game();
        for id in 1..=14 {
            assert_eq!(k.counties[id].weather, Weather::Cloudy, "county {id}");
            assert_eq!(k.counties[id].fertility, 0, "county {id}");
        }
    }

    #[test]
    fn tax_is_collected_into_the_owning_realm_and_nowhere_else() {
        let mut k = Kingdom::new(1);
        k.set_county_count(3);
        k.realms[1].in_play = true;
        k.counties[1].owner = 1;
        k.counties[1].population = 1000;
        k.counties[1].tax_rate = 10;
        k.counties[2].owner = 0; // unowned
        k.counties[2].population = 1000;
        k.counties[2].tax_rate = 10;

        let mut report = SeasonReport::new();
        k.run_pass(Pass::TaxCollect, &mut report);
        assert_eq!(k.realms[1].gold, 320);
        assert_eq!(k.counties[2].tax_collected, 320, "computed but banked nowhere");
        assert_eq!(k.realms[0].gold, 0, "realm 0 is not a realm");
    }

    // --- the history ring --------------------------------------------------

    /// The ring's shape
    /// 51,200 bytes, and `400 * 16 * 8` is 51,200.
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

    /// **Counties 1..=16 unconditionally**, not `1..=g_countyCount`. A map with
    /// four counties still writes sixteen lines, twelve of them zero.
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

    /// A hundred years in, the ring is full and starts forgetting.
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

    /// The ring is written by the season driver, not only by hand.
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


    // --- the tax rate -------------------------------------------------------

    /// **`Tax_RecomputePreview` writes three fields and the panel draws all
    /// three**, so
    /// panel stale. A player reported both halves of that in one sentence:
    /// *"'People pay 0 crowns' on the tax thing always says 0 crowns. And the
    /// happiness bonus/minus on the tax screen is also stuck."*
    ///
    /// `tax_shown` had exactly one writer, [`crate::tax::collect`], which runs
    /// once a season — so before the first collection it is zero and after it it
    /// describes last season's rate.
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
        // Ablation: drop the `tax_shown` line from `tax::recompute_preview` and
        // the second and fourth assertions fail with 0.
    }

    /// **The *Other counties* line really is stuck
    ///
    /// `taxHapOther` is `g_taxHappinessOther[rate]`, a table
    /// flat zero from 0 to 19. Every county in every fixture sits at rate 0 and
    /// the highest an AI reaches in a hundred turns is 12, so **most of this
    /// mechanic is human-only and no run of ours exercises it** —
    /// `docs/decisions.md` C26.
    ///
    /// This is the half of the player's report
    /// asserted so that nobody "fixes" it later.
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

    /// **`taxShown` ignores suppression and `taxCollected` does not**, which is
    /// the whole of how the two fields differ — `docs/kingdom.md` §1.3 lists
    /// them side by side and says it does not know.
    ///
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

    // --- the ration split ---------------------------------------------------


    /// **The third control on the ration panel, found by enumerating the class
/// `Ration_IncreaseCounty` (`0x0043A23F`) is
    /// `rationWanted++`, `Ration_Apply`, `County_RefreshEstimates`,
    /// `Panel_Ration` — so asking for more food changes what the county is
    /// recorded as eating, on the spot.
    ///
    /// Ours wrote `ration_wanted` and returned, like the split slider and like
    /// the tax arrows before it.
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
        // Ablation: drop the `ration::preview` call from `set_ration_wanted`
        // and both comparisons collapse to equality.
    }

    /// The cap is the table's length and the recompute is **outside** the
/// guard: a click at the top still re-applies, as
    /// `if (rationWanted < 5) rationWanted++;` followed by an unconditional
    /// `Ration_Apply` says.
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
        // A sentinel no pass would ever leave, so the assertion is about the
// pass having run.
        k.counties[1].ration_achieved = -7;
        assert!(!k.set_ration_wanted(1, 9), "the level did not move");
        assert_eq!(k.counties[1].ration_wanted, 5, "and is clamped to the table");
        assert_ne!(
            k.counties[1].ration_achieved, -7,
            "but the pass ran anyway, because the guard is only on the increment",
        );
    }

    /// It writes what the player asked for and never what the county managed.
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

    /// A county that eats some of its herd and some of its grain, which is the
    /// only state in which the slider's search does anything at all.
    fn a_county_that_eats_both() -> Kingdom {
        let mut k = Kingdom::new(9);
        k.set_county_count(2);
        k.realms[1].in_play = true;
        let c = &mut k.counties[1];
        c.owner = 1;
        c.population = 2000;
        c.pop_band = 80;
        // The standing herd feeds five people a head for free, so it has to be
        // small enough that there is a requirement left to split.
        c.herd = 100;
        c.grain = 900;
        c.ration_wanted = 3;
        c.ration_split = 50;
        k
    }

    /// **The write is not the behaviour.** `Ration_SetSplit` runs the food pass
    /// on the spot, so the numbers the panel prints move with the slider — and
    /// a slider whose effect is invisible is what a player reported as *"moves
    /// but is inoperable"*.
    ///
    /// The old test asserted `ration_split == 37` and passed
    /// broken the whole time: it was checking the field the gesture writes, and
    /// the defect was the absence of everything after the write.
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
        // Ablation: delete the `ration::preview` call in `set_ration_split` and
        // every one of these is whatever the last season left, so all four
        // comparisons collapse.
    }

    /// **The store is not touched.** `Ration_Apply` computes and records; the
    /// season spends. A drag runs it up to a hundred times, so if this were
    /// [`crate::ration::apply`] the county would be eaten alive by its own
    /// slider.
    #[test]
    fn dragging_the_split_does_not_feed_anybody() {
        let mut k = a_county_that_eats_both();
        let (herd, grain) = (k.counties[1].herd, k.counties[1].grain);
        for split in 0..=100 {
            k.set_ration_split(1, split, true);
        }
        assert_eq!((k.counties[1].herd, k.counties[1].grain), (herd, grain));
    }

    /// **A track jump that changes nothing springs back**, and an arrow does
    /// not. The two gestures differ only in `sweep`, and a player can see it.
    ///
    /// The search runs when the county has a herd, is eating some of it, the
    /// value moved, the request is strictly inside 0…100, and `herdEaten` came
    /// out unchanged. It then walks one point at a time from the old value
    /// towards the request looking for a split that moves `herdEaten`.
    ///
    /// * with `sweep`, reaching the request having found nothing **restores the
    ///   old split**;
    /// * without it, `(rationSplit != split) || (sweep == 0)` never terminates
    ///   the walk at the request, so it carries on in the same direction — one
    ///   click of an arrow can move the split a long way, and it does not spring
    ///   back.
    #[test]
    fn a_track_jump_springs_back_where_an_arrow_keeps_walking() {
        // Chosen so that one point of split is below the rounding: thirty
        // people, two head of dairy feeding ten of them, so twenty people-worth
        // left to split. `pct(20, 50)` and `pct(20, 51)` are both 10, and ten
        // people-worth is one head either way — so
        // nothing and the search is forced to run. Without that the guard
        // `herd_eaten != 0` is false, the search never fires, and this test
        // passes while asserting nothing, which is what its first draft did.
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


    /// The search is bounded
    /// Nothing here may loop for ever on a county whose herd never moves.
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

    /// Another realm's county is refused, and
    /// than the screen's: `Ration_SliderClick` opens
    /// `if (counties[sel].owner != g_localPlayer) return 0;`, and
    /// `Game::set_ration_split` is the gate here.
    #[test]
    fn the_split_of_a_county_out_of_range_is_refused() {
        let mut k = a_county_that_eats_both();
        assert!(!k.set_ration_split(0, 50, true), "county 0 is not a county");
        assert!(!k.set_ration_split(99, 50, true), "and neither is one past the count");
    }
}

