#![allow(unused_imports)]
use super::*;
use super::update::*;
use super::tests_part::*;
use crate::county::{ChangeReason, County, CHANGE_REASON_MIN_PCT, MAX_INFLOW_SOURCES};
use crate::math::pct;
use crate::tables::{Season, Tables};
use l2_net::{Quirk, Quirks};

/// How many people would leave a county for a happier neighbour.
///
/// ```text
/// pct    = Pct(bestNeighbourHappiness - happiness, (100 - happiness) / 3);
/// movers = Pct(population, pct);
/// if (movers > 100) movers = 100;
/// if (owner == 0)   movers /= 2;
/// ```
///
/// Two consequences worth stating, both asserted below: migration is capped at
/// 100 people per county per season, and **a county at 100 happiness never
/// emigrates**, whatever its neighbours do — `(100 - 100) / 3` is zero.
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

/// `Migration_UpdateAll` — every county's emigrants and immigrants for this
/// season.
///
/// Happiness is snapshotted before anything moves,
/// not depend on how many counties happen to be earlier in the array. The
/// original walks the array in index order and so does this, which is the same
/// thing said twice — but only one of the two is still true if somebody later
/// parallelises the loop (`docs/netcode.md` §3).
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
        // The happiest neighbour, first one wins on a tie. Stored order, never
        // sorted: the tie-break is part of the simulation.
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

/// Record a source in a destination's sixteen-slot inflow list.
///
/// **This reproduces a documented bug.** `docs/kingdom.md` §5.3: *"the loop
/// that records the source county in the destination's 16-byte inflow list has
/// no `break`: it writes the source id into **every** free slot
/// first."* The list therefore ends up holding one repeated value.
///
/// It is marked **`[D]`** — read from decompiled C, not observed — and it is
/// cosmetic: the pass that reads it back only takes a maximum, so the visible
/// effect is limited to the *"arrive from"* line of the population panel naming
/// the wrong county. It is reproduced
/// reimplementation that quietly corrects the original's bugs cannot be
/// differentially tested against it.
///
/// **Switchable** — [`Quirk::InflowListHasNoBreak`], `docs/bugs.md` B15. The
/// fixed path stops at the first free slot, which turns the sixteen bytes back
/// into the list they are named for.
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

