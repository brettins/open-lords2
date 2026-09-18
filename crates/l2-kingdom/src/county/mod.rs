//! The original is 768 bytes at `0x0053F9B0`, 17 of them, and 52 of its 201
//! referenced offsets are identified. **This is not that layout.** We are
//! writing a new engine, not a memory-compatible clone, so what is reproduced
//! here is the *semantics* of the 52 identified fields, with the offset each
//! one came from recorded in a comment so a future differential test against
//! the original can find it again.

mod county;
pub use county::*;

use crate::tables::{
    Commodity, Weather, FIELD_PROGRESS_MAX, JOB_CATTLE_FARMING, JOB_COUNT, JOB_GRAIN_FARMING,
    JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT,
};

/// The three per-county ratings the minimap's statistic overlays colour by,
/// recomputed by `FUN_00451BBA` on every minimap draw and stored in the five
/// bytes at county `+0x00 … +0x04` that `Sync_CompareState` skips — they are
/// interface state, so they are computed here
/// on demand.
///
/// **They are the county's bytes `+0x03`, `+0x02` and `+0x01`, in that order**;
/// `docs/screens.md` §3.2 called them `+0x0B3`, `+0x0B2` and `+0x0B1`, which is
/// the literal `0x0053F9B3` in the disassembly read as an offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinimapBands {
    /// `+0x03` — 0 short of farm workers, 5 has idle or surplus labour, 6 no
    /// slack at all. Never anything else.
    pub labour: u8,
    /// `+0x02` — 0 the ration achieved fell short of the ration wanted, 6 it
    /// did not. Only a debug toggle makes this use the middle of the ramp.
    pub food: u8,
    /// `+0x01` — `happiness / 20`, so 0 … 5 over the 0 … 100 range.
    pub happiness: u8,
}

pub const MAX_COUNTIES: usize = 17;

pub const MAX_COUNTY_ID: u8 = (MAX_COUNTIES - 1) as u8;

pub const MAX_FIELDS: usize = 20;

/// The inflow list at `+0x48 … +0x57` is sixteen bytes (`docs/kingdom.md`
/// §1.2).
pub const MAX_INFLOW_SOURCES: usize = 16;

/// **`[D]`.** Every writer but grain's and cattle's stores −1
/// readers both compare `labour < wanted`, which −1 can never satisfy.
pub const LABOUR_NO_FLOOR: i32 = -1;

/// **`[D]`.** `FUN_0044F318` writes 100,000 for iron, stone and wood — every
/// industry except the blacksmith, whose output is capped by the iron store.
pub const LABOUR_UNBOUNDED: i32 = 100_000;

/// The threshold `Village_BalanceJob` (`0x00439F6A`) treats as "no ceiling".
///
/// **`[V]`.** It guards its surplus arithmetic with `if (useful < 99999)`, one
/// short of [`LABOUR_UNBOUNDED`] — so the double click sheds nobody from iron,
/// stone or wood however many people are in them, which is right, because those
/// three have no ceiling. It is written down separately
/// folded into the constant above because the two numbers are not the same and
/// the difference is a real one: a hypothetical ceiling of exactly 99,999 would
/// bind the allocator and not this gesture.
pub const LABOUR_CEILING_IGNORED: i32 = 99_999;

/// **`[D]`.** The allocator (`FUN_0044F6E7`) opens by reading all eight
/// ceilings and rewriting this value to **0** — so an unset ceiling allocates
/// nobody.
pub const LABOUR_UNSET: i32 = 999_999;

/// **The grain-to-livestock split's range**
/// same number by construction: `Ration_SliderClick` (`0x0043A379`) clamps
/// `mouseX - 224` to `0 … 100` over a track exactly 100 pixels wide.
pub const MAX_RATION_SPLIT: i32 = 100;

/// A county cannot neighbour more counties than there are counties. The
/// original stores the count at `+0x5A` and the ids from `+0x5C`; the array
/// length is not stated, so this is the tightest bound the county array itself
/// implies.
pub const MAX_NEIGHBOURS: usize = MAX_COUNTIES - 2;

/// The `L2.eng` group 65 population-change reasons, in the order the five
/// strings appear. `docs/kingdom.md` §1.2, county `+0x5B`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum ChangeReason {
    #[default]
    None = 0,
    Births = 1,
    Deaths = 2,
    Emigration = 3,
    Immigration = 4,
}

impl ChangeReason {
    pub fn name(self) -> &'static str {
        match self {
            ChangeReason::None => "None",
            ChangeReason::Births => "Births",
            ChangeReason::Deaths => "Deaths",
            ChangeReason::Emigration => "Emigration",
            ChangeReason::Immigration => "Immigration",
        }
    }
}

pub const CHANGE_REASON_MIN_PCT: i32 = 6;

/// One commodity's production record, county `+0x294 + c*0x18` — not `+0x290`,
/// which is [`County::weapon_type`]. `docs/kingdom.md` §1.3 and §7.4,
/// `docs/decisions.md` C153.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Industry {
    /// What the last `Industry_Produce` pass produced — the difference between
    /// the running total at `+0x2A0` and its snapshot at `+0x2A4`.
    pub output: i32,
    /// County `+0x294`, the efficiency percentage. It **ramps**: see
    /// [`crate::industry::efficiency_ramp`], which is `FUN_0044F248`.
    ///
    /// Production multiplies by **this** byte, and both
    /// `Industry_Produce` (`0x0044EA92`) and `Industry_LabourEstimate`
    /// (`0x0044F318`) write it.
    pub efficiency: i32,
    /// County `+0x29C` — the **ramp's input**
    /// pass can write [`Industry::efficiency`] without compounding.
    ///
    /// `Industry_EfficiencyRamp` (`0x0044F248`) reads `+0x29C`; only
    /// `Industry_Produce` (`0x0044EA92`) writes it, as a copy of `+0x294`
    /// right after the season's ramp. So the four `County_RefreshEstimates`
    /// calls inside one `Industry_ToggleFromMap` all ramp from the same
    /// season-old number and land on the same answer. `[V]`, both offsets read
    /// out of the two functions.
    pub last_efficiency: i32,
    /// County `+0x29E` — the worker count this industry can absorb at full
    /// value. Above it the ramp's increment is scaled down by
    /// `capacity / workers`, so piling on more serfs raises the output but
    /// slows the improvement. `[D]` — the field is read by the ramp and
    /// written by the industry driver from county `+0x108`, whose meaning was
    /// not traced.
    pub capacity: i32,
    /// County `+0x295` — the county has this resource in the ground at all
    /// (ore, stone, forest). Weapons ignore it.
    pub has_resource: bool,
    /// County `+0x297` — this industry is switched on. Both this and
    /// [`Industry::has_resource`] gate the `resourceLimit`, so an industry
    /// missing either produces nothing however many workers it is given.
    pub enabled: bool,
    /// County `+0x296` — a countdown. While non-zero the pass produces nothing
    /// at all, zeroes the running total and decrements; at zero the industry is
    /// reinstated. `[D]` — a depleted seam or a razed workshop would both fit
    /// and neither is established.
    pub disabled_seasons: i32,
    /// The running total at `+0x2A0`, which the pass adds this season's output
/// to.
    pub total: i32,
    /// County **`+0x2A8 + c*0x18`** — `+0x14`, the last field of this
    /// commodity's **own** record, with the array based at `+0x294`:
    ///
    /// | commodity | forecast at | record |
    /// |---|---|---|
    /// | wood `[0]` | `+0x2A8` | `[0] +0x14` |
    /// | iron `[1]` | `+0x2C0` | `[1] +0x14` |
    /// | weapons `[2]` | `+0x2D8` | `[2] +0x14` |
    /// | stone `[3]` | `+0x2F0` | `[3] +0x14` |
    ///
    /// Under the `+0x290` base `docs/records.json` carried, `+0x2A8 + c*0x18`
    /// looked like the head word of record `c + 1` — *"a second per-commodity
    /// array … shifted one whole record along"* — and stone's `+0x2F0` looked
    /// one past the end. The base was four bytes low. `+0x290` is
    /// [`County::weapon_type`], a byte of its own
    /// `+0x294 … +0x2F3`. `docs/decisions.md` C153.
    ///
    /// `[V]`, three ways. `Industry_LabourEstimate` (`0x0044F318`) zeroes
    /// `county[0x2A8 + industry*0x18]` before its first guard and writes the
    /// forecast there last, with the commodity index. `Unit_TrampleTile`
    /// (`0x0046873F`) zeroes it in the same arm as that commodity's own
    /// `disabledSeasons` and `efficiency`. And every save on this machine stores
    /// there the number record `c`'s own workers produce, never record
    /// `c + 1`'s — `crates/l2-scenario/tests/import/main.rs`.
    pub next_season: i32,
}

impl Industry {
    pub fn new(c: Commodity) -> Industry {
        Industry {
            output: 0,
            efficiency: c.base_efficiency(),
            last_efficiency: c.base_efficiency(),
            capacity: 0,
            has_resource: true,
            enabled: true,
            disabled_seasons: 0,
            total: 0,
            next_season: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{JOB_FIELD_RECLAMATION, JOB_STONE_QUARRYING};

    #[test]
    fn a_new_county_is_unowned_and_on_normal_rations() {
        let c = County::new();
        assert!(c.is_unowned());
        assert_eq!(c.ration_wanted, 3);
        assert_eq!(c.ration_achieved, 3);
        assert_eq!(c.ration_split, 100);
        assert_eq!(c.weather, Weather::Cloudy);
    }

    #[test]
    fn pop_band_reproduces_the_two_values_in_the_save() {
        let mut c = County::new();
        c.population = 435;
        assert_eq!(c.compute_pop_band(), 18);
        c.population = 456;
        assert_eq!(c.compute_pop_band(), 19);
        c.population = 417;
        assert_eq!(c.compute_pop_band(), 17);
    }

    #[test]
    fn neighbours_are_bounded_rather_than_growing() {
        let mut c = County::new();
        for i in 0..MAX_NEIGHBOURS {
            assert!(c.add_neighbour(i as u8 + 1), "neighbour {i} should fit");
        }
        assert!(!c.add_neighbour(99), "the seventeenth should be refused");
        assert_eq!(c.neighbours().len(), MAX_NEIGHBOURS);
    }

    #[test]
    fn reclaiming_a_field_takes_four_seasons_and_stops_at_eight_hundred() {
        let mut c = County::new();
        let mut seen = Vec::new();
        for _ in 0..6 {
            seen.push(c.reclaim_field(0, crate::tables::FIELD_RECLAIM_PER_SEASON));
        }
        assert_eq!(seen, vec![200, 400, 600, 800, 800, 800]);
    }

    #[test]
    fn a_season_cannot_reclaim_more_than_a_quarter_of_a_field() {
        let mut c = County::new();
        assert_eq!(c.reclaim_field(0, 10_000), 200);
    }

    /// `FUN_00451BBA`'s three ratings, band by band.
    #[test]
    fn the_minimap_ratings_are_happiness_over_twenty_and_two_binary_flags() {
        let mut c = County::new();
        c.labour_wanted = [LABOUR_NO_FLOOR; JOB_COUNT];

        for (happiness, want) in [(0, 0), (19, 0), (20, 1), (79, 3), (99, 4), (100, 5)] {
            c.happiness = happiness;
            assert_eq!(c.minimap_bands().happiness, want, "happiness {happiness}");
        }
        c.happiness = -20;
        assert!(c.minimap_bands().happiness > 5, "negative happiness is not band 0");
        c.happiness = 60;

        c.ration_wanted = 3;
        for (achieved, want) in [(0, 0), (2, 0), (3, 6), (5, 6)] {
            c.ration_achieved = achieved;
            assert_eq!(c.minimap_bands().food, want, "ration {achieved} of 3");
        }
        c.ration_achieved = 3;

        assert_eq!(c.minimap_bands().labour, 6);
        c.labour[JOB_IDLE_TOWNSFOLK] = 4;
        assert_eq!(c.minimap_bands().labour, 5);
        c.labour[JOB_IDLE_TOWNSFOLK] = 0;
        c.labour[JOB_STONE_QUARRYING] = 9;
        c.labour_useful[JOB_STONE_QUARRYING] = 2;
        assert_eq!(c.minimap_bands().labour, 5);
        c.labour_wanted[JOB_CATTLE_FARMING] = 10;
        c.labour[JOB_CATTLE_FARMING] = 3;
        assert_eq!(c.minimap_bands().labour, 0);
        c.labour_wanted[JOB_CATTLE_FARMING] = LABOUR_NO_FLOOR;
        c.labour_wanted[JOB_FIELD_RECLAMATION] = 10;
        assert_eq!(c.minimap_bands().labour, 5, "only the two farm jobs carry a floor");
    }

    #[test]
    fn the_field_total_is_the_sum_of_the_three_usage_counts() {
        let mut c = County::new();
        c.fields_fallow = 3;
        c.fields_cattle = 4;
        c.fields_grain = 6;
        assert_eq!(c.field_total(), 13);
        assert!((8..=MAX_FIELDS as i32).contains(&c.field_total()));
    }
}

