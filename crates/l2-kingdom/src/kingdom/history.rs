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

impl History {
    pub fn new() -> History {
        History {
            entries: vec![
                [HistoryEntry::default(); crate::tables::HISTORY_COUNTIES];
                crate::tables::HISTORY_SEASONS
            ],
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    /// How many seasons are held, up to [`crate::tables::HISTORY_SEASONS`].
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Write this season's line for counties 1..=16 and advance the ring.
    ///
    /// The original's loop is `for (c = 1; c < 0x11; c++)` — **counties 1..=16
    /// unconditionally**, not `1..=g_countyCount` — so the slots above the map's
    /// county count are filled with whatever the unused records hold, which is
    /// zero. Reproduced, because it is what a reader of the ring has to expect.
    pub fn record(&mut self, counties: &[County]) {
        let slot = &mut self.entries[self.head];
        for c in 1..=crate::tables::HISTORY_COUNTIES {
            let county = &counties[c];
            slot[c - 1] = HistoryEntry {
                population: county.population,
                // The original stores a signed byte. Happiness is clamped
                // 0..=100 by `Happiness_UpdateAll`, so the narrowing is safe
// here
                // come to depend on a range the original does not have.
                happiness: county.happiness as i8,
            };
        }
        self.len += 1;
        if self.len > crate::tables::HISTORY_SEASONS {
            self.len = crate::tables::HISTORY_SEASONS;
            self.tail += 1;
            if self.tail >= crate::tables::HISTORY_SEASONS {
                self.tail = 0;
            }
        }
        self.head += 1;
        if self.head >= crate::tables::HISTORY_SEASONS {
            self.head = 0;
        }
    }

    /// One county's history, oldest season first. `county` is 1-based.
    pub fn county(&self, county: usize) -> Vec<HistoryEntry> {
        if county < 1 || county > crate::tables::HISTORY_COUNTIES {
            return Vec::new();
        }
        (0..self.len)
            .map(|i| {
                let slot = (self.tail + i) % crate::tables::HISTORY_SEASONS;
                self.entries[slot][county - 1]
            })
            .collect()
    }

    /// The most recent season recorded, or `None` before the first.
    pub fn latest(&self) -> Option<&[HistoryEntry; crate::tables::HISTORY_COUNTIES]> {
        if self.len == 0 {
            return None;
        }
        let slot =
            (self.head + crate::tables::HISTORY_SEASONS - 1) % crate::tables::HISTORY_SEASONS;
        Some(&self.entries[slot])
    }
}

