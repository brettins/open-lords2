#![allow(unused_imports)]
use super::*;
use super::update::*;
use super::tests_part::*;
use crate::county::{ChangeReason, County, CHANGE_REASON_MIN_PCT, MAX_INFLOW_SOURCES};
use crate::math::pct;
use crate::tables::{Season, Tables};
use l2_net::{Quirk, Quirks};

pub fn movers(population: i32, happiness: i32, best_neighbour: i32, unowned: bool) -> i32 {
    if best_neighbour <= happiness {
        return 0;
    }
    let rate = pct(best_neighbour - happiness, (100 - happiness) / 3);
    let mut movers = pct(population, rate);
    if movers > MIGRATION_CAP {
        movers = MIGRATION_CAP;
    }
    if unowned {
        movers /= 2;
    }
    movers.max(0)
}

pub fn migrate_all(counties: &mut [County], county_count: usize, quirks: Quirks) {
    for id in 1..=county_count {
        counties[id].emigrants = 0;
        counties[id].immigrants = 0;
        counties[id].largest_inflow = 0;
        counties[id].largest_inflow_source = 0;
        counties[id].emigrant_destination = 0;
        counties[id].inflow_sources = [0; MAX_INFLOW_SOURCES];
    }

    let happiness: Vec<i32> = (0..counties.len()).map(|i| counties[i].happiness).collect();

    for id in 1..=county_count {
        let mut best: Option<(u8, i32)> = None;
        for &n in counties[id].neighbours() {
            let n = n as usize;
            if n == 0 || n >= counties.len() {
                continue;
            }
            if best.map_or(true, |(_, h)| happiness[n] > h) {
                best = Some((n as u8, happiness[n]));
            }
        }
        let Some((dest, best_happiness)) = best else { continue };

        let leaving = movers(
            counties[id].population,
            happiness[id],
            best_happiness,
            counties[id].is_unowned(),
        );
        if leaving == 0 {
            continue;
        }

        counties[id].emigrants = leaving;
        counties[id].emigrant_destination = dest;

        let d = dest as usize;
        counties[d].immigrants += leaving;
        if leaving > counties[d].largest_inflow {
            counties[d].largest_inflow = leaving;
            counties[d].largest_inflow_source = id as u8;
        }
        record_inflow_source(&mut counties[d], id as u8, quirks);
    }
}

/// It is marked **`[D]`** — read from decompiled C, not observed — and it is
/// cosmetic: the pass that reads it back only takes a maximum, so the visible
/// effect is limited to the *"arrive from"* line of the population panel naming
/// the wrong county. It is reproduced
/// reimplementation that quietly corrects the original's bugs cannot be
/// differentially tested against it.
fn record_inflow_source(dest: &mut County, source: u8, quirks: Quirks) {
    for slot in 0..MAX_INFLOW_SOURCES {
        if dest.inflow_sources[slot] == 0 {
            dest.inflow_sources[slot] = source;
            if !quirks.reproduces(Quirk::InflowListHasNoBreak) {
                return;
            }
        }
    }
}

