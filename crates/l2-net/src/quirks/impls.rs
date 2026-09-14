#![allow(unused_imports)]
use super::*;

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};

impl Quirk {
    /// Every quirk, in declaration order, which is bit order.
    ///
    /// **The only supported way to walk the set.** Nothing anywhere builds a
    /// map keyed by [`Quirk`] and iterates it; `docs/netcode.md` D-4.
    pub const ALL: &'static [Quirk] = &[
        Quirk::HarvestIgnoresLabourCap,
        Quirk::EventDeckParityLocksOutEvenCounties,
        Quirk::FoundWeaponFollowsCountyId,
        Quirk::EmpireTaxHappinessWraps,
        Quirk::AnyAleFillsATinyVillage,
        Quirk::UnownedCountyTradesUnchecked,
        Quirk::CastleSwitchMovesShareBackwards,
        Quirk::InflowListHasNoBreak,
        Quirk::ExtinctCountyRecordsNegativeDeaths,
        Quirk::AiUnrestDeadBand,
        Quirk::MercenaryBandOvershoots,
        Quirk::EmptyGameIsWonBySlotZero,
        Quirk::DeadHumanCanStillWin,
        Quirk::MutualDestructionIsAWin,
    ];

    /// Its bit position in [`Quirks`].
    pub const fn bit(self) -> u32 {
        self as u32
    }

    /// The `docs/bugs.md` entry this switches — `"B1"`, `"B11a"`.
    ///
    /// This is the join between the code and the catalogue, and
    /// `tools/quirks/quirks.js` is what checks the join holds.
    pub const fn entry(self) -> &'static str {
        match self {
            Quirk::HarvestIgnoresLabourCap => "B1",
            Quirk::EventDeckParityLocksOutEvenCounties => "B2",
            Quirk::FoundWeaponFollowsCountyId => "B3",
            Quirk::EmpireTaxHappinessWraps => "B4",
            Quirk::AnyAleFillsATinyVillage => "B10",
            Quirk::UnownedCountyTradesUnchecked => "B11a",
            Quirk::CastleSwitchMovesShareBackwards => "B12",
            Quirk::InflowListHasNoBreak => "B15",
            Quirk::ExtinctCountyRecordsNegativeDeaths => "B16",
            Quirk::AiUnrestDeadBand => "B17",
            Quirk::MercenaryBandOvershoots => "B42",
            Quirk::EmptyGameIsWonBySlotZero => "B51",
            Quirk::DeadHumanCanStillWin => "B52",
            Quirk::MutualDestructionIsAWin => "B53",
        }
    }

    /// The identifier, for the toggle list and for a check's output.
    pub const fn name(self) -> &'static str {
        match self {
            Quirk::HarvestIgnoresLabourCap => "HarvestIgnoresLabourCap",
            Quirk::EventDeckParityLocksOutEvenCounties => {
                "EventDeckParityLocksOutEvenCounties"
            }
            Quirk::FoundWeaponFollowsCountyId => "FoundWeaponFollowsCountyId",
            Quirk::EmpireTaxHappinessWraps => "EmpireTaxHappinessWraps",
            Quirk::AnyAleFillsATinyVillage => "AnyAleFillsATinyVillage",
            Quirk::UnownedCountyTradesUnchecked => "UnownedCountyTradesUnchecked",
            Quirk::CastleSwitchMovesShareBackwards => "CastleSwitchMovesShareBackwards",
            Quirk::InflowListHasNoBreak => "InflowListHasNoBreak",
            Quirk::ExtinctCountyRecordsNegativeDeaths => {
                "ExtinctCountyRecordsNegativeDeaths"
            }
            Quirk::AiUnrestDeadBand => "AiUnrestDeadBand",
            Quirk::MercenaryBandOvershoots => "MercenaryBandOvershoots",
            Quirk::EmptyGameIsWonBySlotZero => "EmptyGameIsWonBySlotZero",
            Quirk::DeadHumanCanStillWin => "DeadHumanCanStillWin",
            Quirk::MutualDestructionIsAWin => "MutualDestructionIsAWin",
        }
    }

    /// One line for the player, in the player's words
    /// binary's. Shown on the quirks page.
    pub const fn summary(self) -> &'static str {
        match self {
            Quirk::HarvestIgnoresLabourCap => {
                "Weather replaces the harvest instead of scaling it"
            }
            Quirk::EventDeckParityLocksOutEvenCounties => {
                "Even-numbered counties never draw a random event"
            }
            Quirk::FoundWeaponFollowsCountyId => {
                "Found and embezzled weapons ignore the county's smithy"
            }
            Quirk::EmpireTaxHappinessWraps => {
                "Taxing a large empire hard can make it happier"
            }
            Quirk::AnyAleFillsATinyVillage => {
                "One barrel buys full ale happiness under ten people"
            }
            Quirk::UnownedCountyTradesUnchecked => {
                "Lordless counties trade with no stock and no gold"
            }
            Quirk::CastleSwitchMovesShareBackwards => {
                "The castle switch moves its labour share the wrong way"
            }
            Quirk::InflowListHasNoBreak => {
                "The migration panel names the wrong county"
            }
            Quirk::ExtinctCountyRecordsNegativeDeaths => {
                "A county that dies out reports negative deaths"
            }
            Quirk::AiUnrestDeadBand => {
                "AI unrest is frozen between happiness 1 and 10"
            }
            Quirk::MercenaryBandOvershoots => {
                "A band that made an offer skips the next county"
            }
            Quirk::EmptyGameIsWonBySlotZero => "An empty game is won by nobody",
            Quirk::DeadHumanCanStillWin => "You can win a game you died in",
            Quirk::MutualDestructionIsAWin => "Mutual destruction counts as a win",
        }
    }
}

impl Quirks {
    /// **What the original does.** Zero.
    pub const FAITHFUL: Quirks = Quirks { bits: 0 };

    /// Every defined quirk fixed.
    pub const FIXED: Quirks = Quirks { bits: Quirks::MASK };

    /// The bits [`Quirk::ALL`] occupies. Everything else is reserved and held
    /// at zero.
    const MASK: u64 = {
        // A `const fn` loop, so the mask cannot drift from the enum: adding a
        // variant to `ALL` widens it with no second edit.
        let mut m = 0u64;
        let mut i = 0;
        while i < Quirk::ALL.len() {
            m |= 1u64 << (Quirk::ALL[i] as u32);
            i += 1;
        }
        m
    };

    /// Read a raw bitfield, discarding bits no quirk claims.
    pub const fn from_bits(bits: u64) -> Quirks {
        Quirks { bits: bits & Quirks::MASK }
    }

    /// The raw bitfield, as the save and the digest carry it.
    pub const fn bits(self) -> u64 {
        self.bits
    }

    /// **Does the simulation reproduce this defect?**
    ///
    /// The question every rule site asks, phrased so that the faithful answer
    /// is the one a reader expects from a project whose premise is fidelity.
    pub const fn reproduces(self, q: Quirk) -> bool {
        self.bits & (1u64 << q.bit()) == 0
    }

    /// The complement of [`Quirks::reproduces`], for the interface, which
    /// thinks in *"turn off the original game's bugs"*.
    pub const fn is_fixed(self, q: Quirk) -> bool {
        !self.reproduces(q)
    }

    /// Reproduce this defect, or do not.
    pub fn set_reproduced(&mut self, q: Quirk, reproduced: bool) {
        if reproduced {
            self.bits &= !(1u64 << q.bit());
        } else {
            self.bits |= 1u64 << q.bit();
        }
    }

    /// Flip one quirk. What a checkbox does.
    pub fn toggle(&mut self, q: Quirk) {
        self.bits ^= 1u64 << q.bit();
    }

    /// **The group switch.** Set every quirk at once.
    ///
    /// The parent control's whole behaviour: `set_all(true)` is *"reproduce
    /// them all"*, `set_all(false)` is *"turn off the original game's bugs"*.
    /// It does not remember what the children were — a parent that restored a
    /// previous mixture would be a fourth state the checkbox cannot show.
    pub fn set_all(&mut self, reproduced: bool) {
        self.bits = if reproduced { 0 } else { Quirks::MASK };
    }

    /// What the parent control shows.
    pub fn group(self) -> Group {
        if self.bits == 0 {
            Group::AllReproduced
        } else if self.bits == Quirks::MASK {
            Group::AllFixed
        } else {
            Group::Mixed
        }
    }

    /// How many quirks are reproduced, and how many there are — the *"9 of
    /// 14"* a mixed parent shows beside itself.
    pub fn tally(self) -> (usize, usize) {
        let fixed = self.bits.count_ones() as usize;
        (Quirk::ALL.len() - fixed, Quirk::ALL.len())
    }
}

