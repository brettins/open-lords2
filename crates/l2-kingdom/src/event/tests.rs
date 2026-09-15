#![allow(unused_imports)]
use super::*;
use super::deck::*;
use super::effects::*;
use crate::county::County;
use crate::report::Message;
use crate::tables::{Season, Tables};
use l2_net::{Pcg32, Quirk, Quirks};

#[cfg(test)]
mod tests {
    use super::*;

    const Q: Quirks = Quirks::FAITHFUL;

    const T: &Tables = &Tables::DEFAULT;
    use std::collections::BTreeSet;

    fn county(owner: u8) -> County {
        let mut c = County::new();
        c.owner = owner;
        c
    }

    #[test]
    fn the_deck_holds_exactly_the_twenty_four_dispatched_ids() {
        let dealt: BTreeSet<u16> = EVENT_DECK.iter().map(|(_, k)| k.id()).collect();
        let handled: BTreeSet<u16> = EVENT_KINDS.iter().map(|k| k.id()).collect();
        assert_eq!(dealt, handled);
        assert_eq!(handled.len(), 24);
    }

    #[test]
    fn the_ids_are_the_two_contiguous_runs_the_document_names() {
        let ids: Vec<u16> = EVENT_KINDS.iter().map(|k| k.id()).collect();
        assert_eq!(ids[..8], (0x87..=0x8E).collect::<Vec<u16>>()[..]);
        assert_eq!(ids[8..], (0x12E..=0x13D).collect::<Vec<u16>>()[..]);
        for pair in ids.windows(2) {
            assert!(pair[1] > pair[0], "the kinds are in ascending id order");
        }
    }

    #[test]
    fn every_dealt_slot_sits_at_the_end_of_a_group_of_eight() {
        for (slot, kind) in EVENT_DECK {
            assert_eq!(slot % 8, 7, "{} is at slot {slot}", kind.name());
            assert!(slot < EVENT_DECK_SLOTS);
        }
        for pair in EVENT_DECK.windows(2) {
            assert!(pair[1].0 > pair[0].0, "the deck is in slot order");
        }
    }

    #[test]
    fn two_events_are_dealt_twice_and_the_rest_once() {
        let mut doubled: Vec<&str> = Vec::new();
        for kind in EVENT_KINDS {
            let n = EVENT_DECK.iter().filter(|(_, k)| *k == kind).count();
            assert!(n == 1 || n == 2, "{} appears {n} times", kind.name());
            if n == 2 {
                doubled.push(kind.name());
            }
        }
        doubled.sort_unstable();
        assert_eq!(doubled, vec!["Grain found.", "Plague."]);
        assert_eq!(EVENT_DECK.len(), 26);
    }

    #[test]
    fn about_one_county_season_in_ten_draws_anything() {
        let filled = (0..EVENT_DECK_SLOTS).filter(|s| deck_slot(*s).is_some()).count();
        assert_eq!(filled, EVENT_DECK.len());
        assert_eq!(EVENT_DECK_SLOTS - filled, 230, "230 empty slots");
    }

    #[test]
    fn only_a_human_county_after_1268_is_eligible() {
        let c = county(1);
        assert!(!eligible(T, &c, true, 1268), "the first year is excluded");
        assert!(eligible(T, &c, true, 1269));
        assert!(!eligible(T, &c, false, 1269), "an AI county never draws");
        assert!(!eligible(T, &county(0), true, 1269), "nor an unowned one");
    }

    #[test]
    fn four_events_hit_harder_in_winter_than_in_summer() {
        let hit = |k: EventKind, s: Season| match k.effect(s) {
            Effect::PopulationPct(p) | Effect::GrainPct(p) => p,
            other => panic!("{other:?} is not a percentage"),
        };
        assert_eq!(hit(EventKind::Plague, Season::Summer), -20);
        assert_eq!(hit(EventKind::Plague, Season::Winter), -40);
        assert_eq!(hit(EventKind::Rats, Season::Autumn), -45, "the harvest is in the barn");
        assert_eq!(hit(EventKind::Rats, Season::Summer), -25);
        assert_eq!(hit(EventKind::GrainFound, Season::Spring), 40);
        assert_eq!(hit(EventKind::GrainFound, Season::Winter), 15);
        assert_eq!(hit(EventKind::WeddingFever, Season::Spring), 60);
        assert_eq!(hit(EventKind::WeddingFever, Season::Winter), 30);
    }

    #[test]
    fn every_effect_fits_the_signed_byte_the_original_stores_it_in() {
        for kind in EVENT_KINDS {
            for season in Season::ALL {
                if let Effect::PopulationPct(p) | Effect::GrainPct(p) | Effect::HerdPct(p) =
                    kind.effect(season)
                {
                    assert!((-128..=127).contains(&p), "{} gives {p}", kind.name());
                }
            }
        }
    }

    #[test]
    fn a_failed_guard_changes_nothing_at_all() {
        let mut c = county(1);
        c.herd = 39; // one short of Wolves' guard
        let before = c.clone();
        let mut purse = RealmPurse::default();
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring, Q));
        assert_eq!(c, before);
        assert!(!c.event_fired);
        assert_eq!(c.event_id, 0);

        c.herd = 40;
        assert!(fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring, Q));
        assert_eq!(c.event_herd_pct, -40);
        assert!(c.event_fired);
        assert_eq!(c.event_id, 0x89);
    }

    #[test]
    fn stop_thief_sets_the_gate_that_zeroes_the_tax_take() {
        let mut c = county(1);
        c.tax_shown = 9;
        let mut purse = RealmPurse::default();
        assert!(!fire(&mut c, 1, &mut purse, EventKind::StopThief, Season::Spring, Q));
        assert!(!c.tax_suppressed, "not worth robbing");

        c.tax_shown = 10;
        assert!(fire(&mut c, 1, &mut purse, EventKind::StopThief, Season::Spring, Q));
        assert!(c.tax_suppressed);

        c.population = 1000;
        c.tax_rate = 10;
        c.castle_type = 0;
        assert_eq!(crate::tax::collect(T, &mut c, 0), 0, "the collectors were waylaid");
    }

    #[test]
    fn no_bull_stops_the_herd_growing_rather_than_doubling_it() {
        let furnish = |c: &mut County| {
            c.herd = 50;
            c.fields_cattle = 5;
            c.labour[T.job.cattle_farming] = 150; // three a head
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            c.weather = crate::tables::Weather::Sunny; // +5% if anything grew
        };

        let mut spared = county(1);
        furnish(&mut spared);
        crate::land::herd_season_tick(T, &mut spared, Season::Spring as u8, Season::Summer as u8);
        assert!(spared.herd > 50, "the same county calves when the bull is alive: {}", spared.herd);

        let mut c = county(1);
        furnish(&mut c);
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::NoBull, Season::Spring, Q));
        assert_eq!(c.event_herd_pct, HERD_NO_GROWTH);

        crate::land::herd_season_tick(T, &mut c, Season::Spring as u8, Season::Summer as u8);
        assert_eq!(c.herd, 50, "no growth, and certainly not +99%");
    }

    #[test]
    fn no_bull_does_not_stop_an_understaffed_herd_dying() {
        let mut c = county(1);
        c.herd = 10_000;
        c.fields_cattle = 100; // density 100: massive overcrowding
        c.labour[T.job.cattle_farming] = 0; // and nobody tending it
        c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
        c.weather = crate::tables::Weather::Sunny;
        c.event_herd_pct = HERD_NO_GROWTH;
        crate::land::herd_season_tick(T, &mut c, Season::Summer as u8, Season::Autumn as u8);
        assert_eq!(c.herd, 10_000 - 4_000, "(7 + 33) per 10,000 of 100x the herd, uncontested");
    }

    #[test]
    fn the_plague_knocks_a_perfect_county_into_the_sick_band() {
        let mut c = county(1);
        c.population = 500;
        c.health_meter = 100;
        c.health_band = 4;
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::Plague, Season::Winter, Q));
        assert_eq!(c.event_population_pct, -40);
        assert_eq!(c.health_meter, 25, "clamped down to 25, not 100 - 20");
        assert_eq!(c.health_band, 1, "Sick");
    }

    #[test]
    fn a_found_weapon_is_chosen_by_the_county_id_not_by_the_blacksmith() {
        for id in 1..=16usize {
            let mut c = county(1);
            c.weapon_type = 5; // armour: what the county actually makes
            let mut purse = RealmPurse::default();
            assert!(fire(&mut c, id, &mut purse, EventKind::WeaponsFound, Season::Spring, Q));
            let slot = weapon_slot(id, c.weapon_type, Q);
            assert_eq!(purse.weapons[slot], 25, "county {id}");
            assert_ne!(slot, 0, "slot 0, the crossbow, is unreachable");
            assert_ne!(slot, 5, "and it is never what the county makes here");
        }
    }

    #[test]
    fn the_three_thefts_are_guarded_on_there_being_something_to_take() {
        let mut c = county(1);
        let mut purse = RealmPurse { gold: 499, wood: 299, ..RealmPurse::default() };
        purse.weapons[weapon_slot(1, c.weapon_type, Q)] = 24;
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Fraud, Season::Spring, Q));
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Termites, Season::Spring, Q));
        assert!(!fire(&mut c, 1, &mut purse, EventKind::Corruption, Season::Spring, Q));
        assert_eq!((purse.gold, purse.wood), (499, 299));

        purse = RealmPurse { gold: 500, wood: 300, ..RealmPurse::default() };
        purse.weapons[weapon_slot(1, c.weapon_type, Q)] = 25;
        assert!(fire(&mut c, 1, &mut purse, EventKind::Fraud, Season::Spring, Q));
        assert!(fire(&mut c, 1, &mut purse, EventKind::Termites, Season::Spring, Q));
        assert!(fire(&mut c, 1, &mut purse, EventKind::Corruption, Season::Spring, Q));
        assert_eq!((purse.gold, purse.wood), (0, 0));
        assert_eq!(purse.weapons[weapon_slot(1, c.weapon_type, Q)], 0);
    }

    #[test]
    fn the_two_field_events_need_a_field_to_work_on() {
        let mut full = county(1);
        full.fields_fallow = crate::county::MAX_FIELDS as i32;
        let mut purse = RealmPurse::default();
        assert!(!fire(&mut full, 1, &mut purse, EventKind::MotherNature, Season::Spring, Q));

        let mut room = county(1);
        room.fields_grain = 4;
        assert!(fire(&mut room, 1, &mut purse, EventKind::MotherNature, Season::Spring, Q));
        assert_eq!(room.fields_fallow, 1);
        assert!(fire(&mut room, 1, &mut purse, EventKind::Locusts, Season::Spring, Q));
        assert_eq!(room.fields_grain, 3);

        let mut none = county(1);
        assert!(!fire(&mut none, 1, &mut purse, EventKind::Locusts, Season::Spring, Q));
    }

    #[test]
    fn the_stream_advances_identically_whoever_owns_the_counties() {
        let run = |human: bool| {
            let mut rng = Pcg32::from_seed(1234);
            let mut c = vec![County::new(); 17];
            let mut purses = vec![RealmPurse::default(); 6];
            for id in 1..=14 {
                c[id].owner = if id % 2 == 1 { 1 } else { 0 };
                c[id].grain = 5_000;
                c[id].herd = 500;
                c[id].population = 500;
                c[id].happiness = 60;
                c[id].health_meter = 60;
                c[id].tax_shown = 100;
                c[id].fields_grain = 6;
            }
            let mut out = Vec::new();
            for _ in 0..40 {
                roll_all(T, 
                    &mut c,
                    14,
                    &|_| human,
                    &mut purses,
                    1300,
                    Season::Spring,
                    &mut rng,
                    Q,
                    &mut out,
                );
            }
            (rng, out.len())
        };
        let (a, events_a) = run(true);
        let (b, events_b) = run(false);
        assert_eq!(a, b, "the generator must be in the same place either way");
        assert!(events_a > 0, "somebody should have drawn something");
        assert_eq!(events_b, 0, "and an all-AI kingdom draws nothing");
    }

    #[test]
    fn the_season_draws_exactly_one_number_however_many_counties_there_are() {
        for n in [1usize, 4, 14, 16] {
            let mut rng = Pcg32::from_seed(9);
            let mut c = vec![County::new(); 17];
            let mut purses = vec![RealmPurse::default(); 6];
            let mut out = Vec::new();
            roll_all(T, &mut c, n, &|_| true, &mut purses, 1300, Season::Spring, &mut rng, Q, &mut out);
            let mut reference = Pcg32::from_seed(9);
            reference.below(EVENT_SEED_BOUND);
            assert_eq!(rng, reference, "kingdom of {n}");
        }
    }

    #[test]
    fn the_same_seed_rolls_the_same_events() {
        let run = || {
            let mut rng = Pcg32::from_seed(7);
            let mut c = vec![County::new(); 17];
            let mut purses = vec![RealmPurse::default(); 6];
            for id in 1..=14 {
                c[id].owner = 1;
                c[id].grain = 900;
                c[id].herd = 200;
                c[id].population = 500;
            }
            let mut out = Vec::new();
            for _ in 0..10 {
                roll_all(T, 
                    &mut c,
                    14,
                    &|_| true,
                    &mut purses,
                    1300,
                    Season::Autumn,
                    &mut rng,
                    Q,
                    &mut out,
                );
            }
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn no_two_neighbouring_counties_can_draw_in_the_same_season() {
        for pair in EVENT_DECK.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= 8, "slots {} and {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn only_odd_numbered_counties_can_ever_draw_an_event() {
        for slot in EVENT_DECK.iter().map(|(s, _)| *s) {
            assert_eq!(slot % 2, 1, "slot {slot} is even");
        }
        let mut drew: Vec<usize> = Vec::new();
        for seed in 0..EVENT_SEED_BOUND as usize {
            let mut index = seed * 2;
            for county in 1..=crate::county::MAX_COUNTY_ID as usize {
                index += 1;
                if index > EVENT_DECK_SLOTS - 1 {
                    index = 0;
                }
                if deck_slot(index).is_some() && !drew.contains(&county) {
                    drew.push(county);
                }
            }
        }
        drew.sort_unstable();
        assert_eq!(drew, vec![1, 3, 5, 7, 9, 11, 13, 15], "the even counties never come up");
    }

    #[test]
    fn a_kingdom_of_even_numbered_counties_never_sees_an_event() {
        let mut rng = Pcg32::from_seed(4242);
        let mut c = vec![County::new(); 17];
        let mut purses = vec![RealmPurse::default(); 6];
        for id in 1..=16 {
            c[id].owner = if id % 2 == 0 { 1 } else { 0 };
            c[id].grain = 5_000;
            c[id].herd = 500;
            c[id].population = 500;
            c[id].happiness = 60;
            c[id].health_meter = 60;
            c[id].tax_shown = 100;
        }
        let mut out = Vec::new();
        for _ in 0..500 {
            roll_all(T, &mut c, 16, &|_| true, &mut purses, 1300, Season::Spring, &mut rng, Q, &mut out);
        }
        assert!(out.is_empty(), "five hundred seasons and not one event");
    }

    #[test]
    fn a_letter_nobody_read_is_still_waiting_seasons_later() {
        let mut c = county(1);
        c.herd = 500;
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring, Q));
        assert!(c.event_fired);

        let mut counties = vec![County::new(); 17];
        counties[1] = c;
        let mut rng = Pcg32::from_seed(11);
        let mut purses = vec![RealmPurse::default(); 6];
        let mut out = Vec::new();
        for _ in 0..10 {
            roll_all(T, &mut counties, 16, &|_| false, &mut purses, 1300, Season::Summer, &mut rng, Q, &mut out);
        }
        assert!(out.is_empty(), "nothing new was dealt");
        assert!(counties[1].event_fired, "the letter is still waiting");
        assert_eq!(counties[1].event_id, EventKind::Wolves.id());
        assert_eq!(counties[1].event_herd_pct, 0, "but the modifier was spent and cleared");
        assert!(!counties[1].tax_suppressed);
    }

    #[test]
    fn a_new_event_that_cannot_fire_takes_the_old_letter_with_it() {
        let mut c = county(1);
        c.herd = 500;
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::Wolves, Season::Spring, Q));
        c.herd = 0; // the wolves and a bad winter between them
        assert!(!fire(&mut c, 1, &mut purse, EventKind::MadCows, Season::Spring, Q));
        assert!(!c.event_fired, "the Wolves letter went with the failed Mad cows");
        assert_eq!(c.event_id, 0);
    }

    #[test]
    fn an_event_is_consumed_by_the_pass_that_reads_it() {
        let mut c = county(1);
        c.grain = 1000;
        c.weather = crate::tables::Weather::Cloudy;
        let mut purse = RealmPurse::default();
        assert!(fire(&mut c, 1, &mut purse, EventKind::Rats, Season::Winter, Q));
        crate::land::grain_season_tick(T, &mut c, Season::Winter, true, Q);
        assert_eq!(c.grain, 600, "a 40% winter loss");
        assert_eq!(c.event_grain_pct, 0);
    }

    #[test]
    fn every_id_round_trips_through_from_id() {
        for kind in EVENT_KINDS {
            assert_eq!(EventKind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(EventKind::from_id(0), None);
        assert_eq!(EventKind::from_id(0x8F), None, "the gap between the two runs");
    }
}

