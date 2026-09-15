//! It is a claim that they
//! are part of the agreed configuration, which is what this crate is for. See
//! `docs/decisions.md` C62.

mod impls;
pub use impls::*;

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Quirk {
    /// `Grain_Harvest` (`0x0044D1E5`). Four of the six bands overwrite the
    /// labour-capped figure with a multiple of the *standing* crop,
    /// that sends one reaper into a sunny field reaps 150 % of everything it
    /// grew. Only bites with *Advanced Farming* on.
    HarvestIgnoresLabourCap = 0,

    /// `Event_DealAll` / `0x00448822 MOV EAX,[0x0058FD60] / ADD EAX,EAX`. The
    /// deck's seed is always even, the wrap resets to an even index, and every
    /// filled slot is odd — so counties 2, 4, 6 … are exempt from every event
    /// in the game.
    EventDeckParityLocksOutEvenCounties = 1,

    /// `FUN_0044938C` (*"Weapons found."*) and `FUN_00449688`
    /// (*"Corruption."*) index the realm's weapon array with
    /// `(countyId & 3) + 1` instead of the county's own weapon type at
    /// `+0x290`. Crossbows can never be found or stolen.
    FoundWeaponFollowsCountyId = 2,

    /// `Tax_SumEmpireHappiness` (`0x0044B99A`) sums up to −240 into a signed
    /// byte and nothing clamps it, so taxing a large empire hard enough can
    /// make its people happier.
    EmpireTaxHappinessWraps = 3,

    /// **B10** — any ale at all buys the full five happiness in a village
    /// under ten people. `Ale_Apply` (`0x00428C42`) walks rungs of
    /// `crowns >= rung * (population / 10)`; below ten people the step is 0 and
    /// the top rung passes.
    AnyAleFillsATinyVillage = 4,

    UnownedCountyTradesUnchecked = 5,

    CastleSwitchMovesShareBackwards = 6,

    InflowListHasNoBreak = 7,

    /// **Negative on the season it dies, as `docs/bugs.md` says**: a county of
    /// one at no happiness in the worst band records −2, and an empty county goes
    /// on recording a negative number every season after. This note used to say
    /// the season of death lands on exactly 0 and only the next season goes
    /// negative; that was our own comparison of the *unscaled* birth rate against
    /// the death rate, not the original's (C170). Both cases are
    /// switched here and asserted in `crates/l2-kingdom/tests/quirks/main.rs`.
    ExtinctCountyRecordsNegativeDeaths = 8,

    /// `Unrest_UpdateAll` (`0x0044AA41`): ≥ 41 resets, 11 … 40 walks down,
    /// below 1 walks up — and 1 … 10 does nothing at all, so an AI county deep
    /// in revolt territory has its unrest frozen.
    AiUnrestDeadBand = 9,

    MercenaryBandOvershoots = 10,

    /// `Score_RankRealms` (`0x0049AA0E`): leader and trailer are both 0,
    /// `0 == 0` passes, and the original crowns `g_realms[0]`
    /// realm.
    EmptyGameIsWonBySlotZero = 11,

    /// **B52** — the human can win a game in which the human is dead. When the
    /// last realm standing is an AI, the first call sends group 195 and sets
    /// the one-shot guard `+0xED`; the *next* call takes the other branch and
    /// sends the human group 225 *"Victory!"*.
    DeadHumanCanStillWin = 12,

    MutualDestructionIsAWin = 13,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quirks {
    bits: u64,
}

/// Three states, because "all fixed" and "all reproduced" are both pure and a
/// player who has changed one child needs to see that the parent no longer
/// speaks for the group. `docs/decisions.md` C26's lesson applies to the
/// transitions between them, not only to the states: a child changed while the
/// parent is on, and every child changed back so the parent returns to a pure
/// state, are the two that a fixture testing one value never sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    AllReproduced,
    AllFixed,
    Mixed,
}

impl Encode for Quirks {
    fn encode(&self, out: &mut Canonical) {
        out.u64(self.bits);
    }
}

impl Decode for Quirks {
    fn decode(input: &mut Reader<'_>) -> Result<Quirks, CodecError> {
        Ok(Quirks::from_bits(input.u64()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bit_positions_are_pinned() {
        assert_eq!(Quirk::HarvestIgnoresLabourCap.bit(), 0);
        assert_eq!(Quirk::EventDeckParityLocksOutEvenCounties.bit(), 1);
        assert_eq!(Quirk::FoundWeaponFollowsCountyId.bit(), 2);
        assert_eq!(Quirk::EmpireTaxHappinessWraps.bit(), 3);
        assert_eq!(Quirk::AnyAleFillsATinyVillage.bit(), 4);
        assert_eq!(Quirk::UnownedCountyTradesUnchecked.bit(), 5);
        assert_eq!(Quirk::CastleSwitchMovesShareBackwards.bit(), 6);
        assert_eq!(Quirk::InflowListHasNoBreak.bit(), 7);
        assert_eq!(Quirk::ExtinctCountyRecordsNegativeDeaths.bit(), 8);
        assert_eq!(Quirk::AiUnrestDeadBand.bit(), 9);
        assert_eq!(Quirk::MercenaryBandOvershoots.bit(), 10);
        assert_eq!(Quirk::EmptyGameIsWonBySlotZero.bit(), 11);
        assert_eq!(Quirk::DeadHumanCanStillWin.bit(), 12);
        assert_eq!(Quirk::MutualDestructionIsAWin.bit(), 13);
    }

    #[test]
    fn all_is_every_quirk_once_in_bit_order() {
        for (i, q) in Quirk::ALL.iter().enumerate() {
            assert_eq!(q.bit() as usize, i, "{} is out of order", q.name());
        }
        let mut seen = 0u64;
        for q in Quirk::ALL {
            let bit = 1u64 << q.bit();
            assert_eq!(seen & bit, 0, "{} appears twice", q.name());
            seen |= bit;
        }
        assert_eq!(seen, Quirks::MASK);
    }

    #[test]
    fn faithful_is_zero_and_reproduces_everything() {
        assert_eq!(Quirks::FAITHFUL.bits(), 0);
        assert_eq!(Quirks::default(), Quirks::FAITHFUL);
        for q in Quirk::ALL {
            assert!(Quirks::FAITHFUL.reproduces(*q), "{}", q.name());
            assert!(!Quirks::FAITHFUL.is_fixed(*q), "{}", q.name());
        }
    }

    #[test]
    fn an_unknown_bit_reads_as_faithful() {
        let smuggled = Quirks::from_bits(u64::MAX);
        assert_eq!(smuggled, Quirks::FIXED);
        assert_eq!(smuggled.bits(), Quirks::MASK);
        assert_eq!(Quirks::from_bits(1 << 63), Quirks::FAITHFUL);
    }

    /// **Every state of the tri-state parent, and every transition between
    /// them.** `docs/decisions.md` C26: a rule can be wrong at 45 of its 51
    /// inputs and stay invisible when the fixture exercises one value.
    #[test]
    fn the_parent_has_three_states_and_every_transition_is_walked() {
        let mut q = Quirks::FAITHFUL;
        assert_eq!(q.group(), Group::AllReproduced);
        assert_eq!(q.tally(), (Quirk::ALL.len(), Quirk::ALL.len()));

        q.set_all(false);
        assert_eq!(q.group(), Group::AllFixed);
        assert_eq!(q.tally(), (0, Quirk::ALL.len()));
        for k in Quirk::ALL {
            assert!(q.is_fixed(*k), "{}", k.name());
        }

        q.set_all(true);
        assert_eq!(q.group(), Group::AllReproduced);

        for k in Quirk::ALL {
            let mut q = Quirks::FAITHFUL;
            q.toggle(*k);
            assert_eq!(q.group(), Group::Mixed, "{} alone", k.name());
            assert_eq!(q.tally(), (Quirk::ALL.len() - 1, Quirk::ALL.len()));
            q.toggle(*k);
            assert_eq!(q.group(), Group::AllReproduced, "{} restored", k.name());
        }

        for k in Quirk::ALL {
            let mut q = Quirks::FIXED;
            q.toggle(*k);
            assert_eq!(q.group(), Group::Mixed, "{} alone", k.name());
            q.toggle(*k);
            assert_eq!(q.group(), Group::AllFixed, "{} restored", k.name());
        }

        let mut q = Quirks::FAITHFUL;
        for (i, k) in Quirk::ALL.iter().enumerate() {
            q.set_reproduced(*k, false);
            let expected = if i + 1 == Quirk::ALL.len() { Group::AllFixed } else { Group::Mixed };
            assert_eq!(q.group(), expected, "after {}", k.name());
        }
        for (i, k) in Quirk::ALL.iter().enumerate() {
            q.set_reproduced(*k, true);
            let expected =
                if i + 1 == Quirk::ALL.len() { Group::AllReproduced } else { Group::Mixed };
            assert_eq!(q.group(), expected, "after restoring {}", k.name());
        }
    }

    #[test]
    fn setting_twice_is_setting_once() {
        let mut q = Quirks::FAITHFUL;
        for k in Quirk::ALL {
            q.set_reproduced(*k, false);
            let once = q;
            q.set_reproduced(*k, false);
            assert_eq!(q, once, "{}", k.name());
        }
        assert_eq!(q, Quirks::FIXED);
    }

    #[test]
    fn every_quirk_names_a_catalogue_entry_and_they_are_distinct() {
        let mut seen: Vec<&str> = Vec::new();
        for q in Quirk::ALL {
            let e = q.entry();
            assert!(e.starts_with('B'), "{} names {e}", q.name());
            assert!(!seen.contains(&e), "two quirks claim {e}");
            seen.push(e);
            assert!(!q.summary().is_empty(), "{} has no summary", q.name());
        }
    }
}

