#![allow(unused_imports)]
use super::*;
use super::county_records::*;
use super::*;
use super::structure::*;
use super::units_and_merchants::*;
use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

/// Two independent fields saying the same thing is the point: `+0x05` of a
/// county and `+0x29` of a realm are written by different code, so agreement is
/// evidence the offsets are right.
#[test]
fn owner_bytes_and_realm_tallies_agree_in_every_save() {
    let saves = saves!();
    for s in &saves {
        let counties = s.save.counties().expect("counties");
        let realms = s.save.realms().expect("realms");
        let g = s.save.globals().expect("globals");

        for c in counties.iter().filter(|c| c.is_county()) {
            assert!(
                (c.owner as usize) < REALM_RECORDS,
                "{}: county {} owner {}",
                s.label(),
                c.index,
                c.owner
            );
            if c.is_owned() {
                assert!(
                    realms[c.owner as usize].in_play(),
                    "{}: county {} is owned by realm {}, which is not in play",
                    s.label(),
                    c.index,
                    c.owner
                );
            }
        }
        for r in realms.iter().skip(1) {
            let held =
                counties.iter().filter(|c| c.is_county() && c.owner as usize == r.index).count();
            assert_eq!(
                held, r.county_count as usize,
                "{}: realm {} says it holds {} counties, {held} name it",
                s.label(),
                r.index,
                r.county_count
            );
            if !r.in_play() {
                assert_eq!(held, 0, "{}: realm {} is out of play but holds land", s.label(), r.index);
            }
        }
        assert!(
            (1..REALM_RECORDS as i32).contains(&g.local_player),
            "{}: g_localPlayer {}",
            s.label(),
            g.local_player
        );
        assert!(
            realms[g.local_player as usize].in_play(),
            "{}: g_localPlayer names a realm that is not in play",
            s.label()
        );
    }
}

#[test]
fn neighbour_lists_are_symmetric_in_every_save() {
    let saves = saves!();
    let mut edges = 0usize;
    for s in &saves {
        let g = s.save.globals().expect("globals");
        let counties = s.save.counties().expect("counties");
        let top = g.county_count as usize;

        for c in counties.iter().filter(|c| c.is_county()) {
            assert!(
                c.neighbour_count as usize <= NEIGHBOUR_SLOTS,
                "{}: county {} claims {} neighbours",
                s.label(),
                c.index,
                c.neighbour_count
            );
            assert!(
                !c.neighbours().is_empty(),
                "{}: county {} borders nobody, so the map is not connected",
                s.label(),
                c.index
            );
            let mut seen = Vec::new();
            for &n in c.neighbours() {
                let n = n as usize;
                assert!((1..=top).contains(&n), "{}: county {} names {n}", s.label(), c.index);
                assert_ne!(n, c.index, "{}: county {} borders itself", s.label(), c.index);
                assert!(!seen.contains(&n), "{}: county {} names {n} twice", s.label(), c.index);
                seen.push(n);
                assert!(
                    counties[n].neighbours().contains(&(c.index as u8)),
                    "{}: county {} names {n}, which does not name it back",
                    s.label(),
                    c.index
                );
                edges += 1;
            }
            for &spare in &c.neighbours[c.neighbour_count as usize..] {
                assert_eq!(
                    spare, 0,
                    "{}: county {} has a stale id past its count",
                    s.label(),
                    c.index
                );
            }
        }
        assert_eq!(edges % 2, 0, "{}: an odd number of directed edges", s.label());
    }
    eprintln!("adjacency: {edges} directed edges symmetric across {} saves", saves.len());
}

#[test]
fn every_bounded_field_is_inside_its_bound_in_every_save() {
    let saves = saves!();
    for s in &saves {
        for c in s.save.counties().expect("counties").iter().filter(|c| c.is_county()) {
            let at = |what: &str| format!("{}: county {} {what}", s.label(), c.index);
            assert!((0..=100).contains(&c.happiness), "{}", at("happiness"));
            assert!((0..=100).contains(&c.happiness_last), "{}", at("happinessLast"));
            assert!((0..=100).contains(&c.health_meter), "{}", at("healthMeter"));
            assert!((0..=4).contains(&c.health_band), "{}", at("healthBand"));
            assert!(c.unrest <= 4, "{}", at("unrest"));
            assert!((0..=5).contains(&c.weather), "{}", at("weather"));
            assert!((0..=5).contains(&c.ration_achieved), "{}", at("rationAchieved"));
            assert!((0..=5).contains(&c.ration_wanted), "{}", at("rationWanted"));
            assert!((0..=100).contains(&c.ration_split), "{}", at("rationSplit"));
            assert!((-100..=100).contains(&c.fertility), "{}", at("fertility"));
            assert!(c.tax_rate <= 100, "{}", at("taxRate"));
            assert!(c.population >= 0, "{}", at("population"));
            assert!(c.pop_last >= 0, "{}", at("popLast"));
            assert!(c.births >= 0 && c.deaths >= 0, "{}", at("births/deaths"));
            assert!(c.grain >= 0 && c.herd >= 0, "{}", at("stores"));
            assert!(c.grain_eaten >= 0 && c.herd_eaten >= 0, "{}", at("eaten"));
            assert!(c.grain_eaten <= c.grain_available, "{}", at("grain eaten beyond store"));
            assert!(c.herd_eaten <= c.herd_available, "{}", at("herd eaten beyond store"));
        }
        let g = s.save.globals().expect("globals");
        assert!((1..=4).contains(&g.season), "{}: g_season {}", s.label(), g.season);
        assert!((1..=4).contains(&g.season_next), "{}", s.label());
        assert!(g.year >= 1268, "{}: g_year {}", s.label(), g.year);
        assert!(g.turn_count >= 0, "{}", s.label());
        assert!((0..=2).contains(&g.opt_difficulty), "{}", s.label());
        assert!(
            (1..=g.county_count).contains(&g.weather_county),
            "{}: g_weatherCounty {} outside 1..={}",
            s.label(),
            g.weather_county,
            g.county_count
        );
    }
}

#[test]
fn migration_conserves_people_across_the_whole_map() {
    let saves = saves!();
    for s in &saves {
        let counties = s.save.counties().expect("counties");
        let out: i32 = counties.iter().filter(|c| c.is_county()).map(|c| c.emigrants).sum();
        let inn: i32 = counties.iter().filter(|c| c.is_county()).map(|c| c.immigrants).sum();
        assert_eq!(out, inn, "{}: {out} left and {inn} arrived", s.label());
    }
}

/// Something else moves people — army recruitment and battle losses are the
/// obvious candidates and neither is read here yet — so the five fields are not
/// a closed system and asserting that they are would have been C12's shape
/// again: a rule that holds on the one file anybody looked at.
#[test]
fn births_and_deaths_are_bounded_by_the_population_they_moved() {
    let saves = saves!();
    for s in &saves {
        for c in s.save.counties().expect("counties").iter().filter(|c| c.is_county()) {
            assert!(c.deaths <= c.pop_last + c.births, "{}: county {}", s.label(), c.index);
            assert!(c.emigrants <= c.pop_last + c.births, "{}: county {}", s.label(), c.index);
        }
    }
}

#[test]
fn the_suite_reports_which_saves_it_ran_over() {
    let found: Vec<SaveFile> = l2_testkit::every_available_save();
    if found.is_empty() {
        skip!("no saves reachable; every invariant in this file asserted nothing");
    }
    for s in &found {
        let g = s.save.globals().unwrap();
        eprintln!(
            "  {} - {} counties, turn {}, season {}, year {}",
            s.label(),
            g.county_count,
            g.turn_count,
            g.season,
            g.year
        );
    }
    assert!(found.iter().any(|s| s.origin == l2_testkit::Origin::Fixture)
        || found.iter().any(|s| s.origin == l2_testkit::Origin::Install));
}



