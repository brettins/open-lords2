//! **The original's defects, as switches** — and why they live in the
//! netcode crate.
//!
//! This project reproduces *Lords of the Realm II* including its bugs
//! (`docs/bugs.md`), and the owner's standing intent is that a player can turn
//! them off: *"definitely keep track of bugs though we will likely fix them in
//! a menu option to have them on or off."* This is the value behind that menu.
//!
//! # Why here
//!
//! A quirk changes what the simulation computes. That makes it part of the
//! **lockstep contract**, in exactly the sense [`crate::Pcg32`] and
//! [`crate::Canonical`] are: two peers that disagree about it produce different
//! states from identical commands. `docs/netcode.md` D-12 — *"the ruleset is
//! part of the version"* — is about a mod set, and a quirk set is the same
//! argument with a different noun.
//!
//! It is also the only place it can go. `l2-kingdom`, `l2-sim`, `l2-view` and
//! `l2-game` all carry reproduced defects, and `l2-net` is the one crate every
//! one of them can see. Putting the enum in `l2-kingdom` would split the group
//! switch across two crates that cannot name each other's halves, and a
//! tri-state parent over two lists is a tri-state parent that will one day
//! disagree with itself.
//!
//! **This is not a claim that quirks are networking.** It is a claim that they
//! are part of the agreed configuration, which is what this crate is for. See
//! `docs/decisions.md` C62.
//!
//! # The sense is inverted, deliberately
//!
//! A set bit means the quirk is **fixed** — the bug is *not* reproduced. So
//! [`Quirks::FAITHFUL`], which is what the original does and what this project
//! ships, is the integer **zero**.
//!
//! That looks backwards for about ten seconds and then pays for itself twice:
//!
//! * **The default is byte-stable forever.** `Quirks` goes into the save body
//!   and the per-tick digest as one `u64`. Adding a quirk next month does not
//!   change the bytes a faithful game writes, so it costs no save-format
//!   version bump — where a "1 means reproduce" bitset would need the mask
//!   widened, and every old save reinterpreted, on every addition.
//! * **An unknown bit reads as faithful.** A save written by an older build has
//!   0 in the slot a newer build has just defined, and 0 is the original's
//!   behaviour — which is the answer that cannot be wrong, because it is the
//!   answer the original gives.
//!
//! `docs/bugs.md` §6.3 asked for *"a single `Quirks` value — a struct of named
//! `bool`s, or a bitfield with a documented assignment and room to grow"*, and
//! required the bump be paid once. This is the bitfield, and the inversion is
//! how it is paid once rather than once per bug.
//!
//! # Determinism
//!
//! [`Quirks`] is a `Copy` integer. There is no map, no set, and nothing to
//! iterate in hash order — `docs/netcode.md` D-4. [`Quirk::ALL`] is a slice in
//! declaration order, which is also bit order, and every walk of the quirk set
//! anywhere in the workspace goes through it.
//!
//! # The rule this module exists to enforce
//!
//! **A quirk that nothing reads is worse than no quirk**, because a checkbox
//! claims a behaviour is configurable. Every variant of [`Quirk`] is read by
//! the simulation and has a test that flips it and observes a different answer:
//! `crates/l2-testkit/tests/quirks_catalogue.rs` reads this file and
//! `docs/bugs.md` as text and fails on a variant nothing calls `reproduces(` on,
//! and `crates/l2-kingdom/tests/quirks.rs` flips each one and watches the
//! simulation give a different answer.
//! Catalogue entries that cannot be switched off for a reasonable price are
//! **not** variants here — they are rows in that catalogue marked unswitchable,
//! with the reason written down.

use crate::canonical::{Canonical, CodecError, Decode, Encode, Reader};

/// One switchable defect of the original.
///
/// The discriminant **is** the bit position in [`Quirks`], and it is written
/// out explicitly for that reason: a variant reordered silently would change
/// what every existing save means. `the_bit_positions_are_pinned` asserts each
/// one.
///
/// Every variant names the `docs/bugs.md` entry it switches and the original
/// function it reproduces, because a feature we implement must name the
/// original function it reproduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Quirk {
    /// **B1** — the harvest weather band throws away the labour cap.
    /// `Grain_Harvest` (`0x0044D1E5`). Four of the six bands overwrite the
    /// labour-capped figure with a multiple of the *standing* crop, so a county
    /// that sends one reaper into a sunny field reaps 150 % of everything it
    /// grew. Only bites with *Advanced Farming* on.
    HarvestIgnoresLabourCap = 0,

    /// **B2** — half the counties can never draw a random event.
    /// `Event_DealAll` / `0x00448822 MOV EAX,[0x0058FD60] / ADD EAX,EAX`. The
    /// deck's seed is always even, the wrap resets to an even index, and every
    /// filled slot is odd — so counties 2, 4, 6 … are exempt from every event
    /// in the game.
    EventDeckParityLocksOutEvenCounties = 1,

    /// **B3** — the weapon a county finds is chosen by its county number.
    /// `FUN_0044938C` (*"Weapons found."*) and `FUN_00449688`
    /// (*"Corruption."*) index the realm's weapon array with
    /// `(countyId & 3) + 1` instead of the county's own weapon type at
    /// `+0x290`. Crossbows can never be found or stolen.
    FoundWeaponFollowsCountyId = 2,

    /// **B4** — the empire tax happiness term is summed into a signed byte.
    /// `Tax_SumEmpireHappiness` (`0x0044B99A`) sums up to −240 into a signed
    /// byte and nothing clamps it, so taxing a large empire hard enough can
    /// make its people happier.
    EmpireTaxHappinessWraps = 3,

    /// **B10** — any ale at all buys the full five happiness in a village
    /// under ten people. `Ale_Apply` (`0x00428C42`) walks rungs of
    /// `crowns >= rung * (population / 10)`; below ten people the step is 0 and
    /// the top rung passes.
    AnyAleFillsATinyVillage = 4,

    /// **B11a** — an unowned county trading on its own account is checked for
    /// neither stock nor gold. Both guards in `Merchant_Trade` are inside
    /// `if (realm != 0)`, so a lordless county sells grain it does not have and
    /// buys with a purse it has already emptied.
    UnownedCountyTradesUnchecked = 5,

    /// **B12** — turning castle building on removes its labour share.
    /// `Industry_ToggleFromMap` reads the switch **before** flipping it and
    /// passes that stale value to the share toggle, so the share moves the
    /// wrong way on every click.
    CastleSwitchMovesShareBackwards = 6,

    /// **B15** — the migration inflow list is written with no `break`, so a
    /// county's sixteen `inflowSources` bytes hold one repeated value and the
    /// population panel's *"arrive from"* line names the wrong county.
    InflowListHasNoBreak = 7,

    /// **B16** — a county that dies out records a wrong death count. `deaths =
    /// pop` after `pop` has been driven below one.
    ///
    /// **Two faces, and `docs/bugs.md` describes the second.** On the season a
    /// county loses its last person the arithmetic lands on exactly 0, so the
    /// figure recorded is 0 — wrong, but not negative. The negative number
    /// appears the *next* season, when the pass runs again over a county that is
    /// already empty and drives it to −1. Both are switched here; both are
    /// asserted in `crates/l2-kingdom/tests/quirks.rs`.
    ExtinctCountyRecordsNegativeDeaths = 8,

    /// **B17** — the AI unrest ladder has a dead band from happiness 1 to 10.
    /// `Unrest_UpdateAll` (`0x0044AA41`): ≥ 41 resets, 11 … 40 walks down,
    /// below 1 walks up — and 1 … 10 does nothing at all, so an AI county deep
    /// in revolt territory has its unrest frozen.
    AiUnrestDeadBand = 9,

    /// **B42** — a mercenary band overshoots a county after making an offer.
    /// The second `nextCounty++` has no wrap guard, so the band sits at
    /// `countyCount + 1` for a season and loses a county from its circuit.
    MercenaryBandOvershoots = 10,

    /// **B51** — a game with nobody in play is won by the array slot.
    /// `Score_RankRealms` (`0x0049AA0E`): leader and trailer are both 0,
    /// `0 == 0` passes, and the original crowns `g_realms[0]`, which is not a
    /// realm.
    EmptyGameIsWonBySlotZero = 11,

    /// **B52** — the human can win a game in which the human is dead. When the
    /// last realm standing is an AI, the first call sends group 195 and sets
    /// the one-shot guard `+0xED`; the *next* call takes the other branch and
    /// sends the human group 225 *"Victory!"*.
    DeadHumanCanStillWin = 12,

    /// **B53** — dying at the same moment as the last opponent is scored a
    /// win, because the opponents test is checked before the is-it-me test.
    MutualDestructionIsAWin = 13,
}

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

    /// One line for the player, in the player's words rather than the
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

/// **Which of the original's defects this game reproduces.**
///
/// A set bit means the quirk is *fixed*; [`Quirks::FAITHFUL`] is zero. See the
/// module documentation for why round that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quirks {
    /// One bit per [`Quirk`], at [`Quirk::bit`]. Bits above the defined set are
    /// **always zero** — [`Quirks::from_bits`] masks them off, so a save from a
    /// newer build loaded by an older one cannot smuggle in a flag this build
    /// would not otherwise honour.
    /// codec-via: `Quirks::from_bits`, so the name does not appear in `decode`.
    bits: u64,
}

/// What a tri-state parent control shows.
///
/// Three states, because "all fixed" and "all reproduced" are both pure and a
/// player who has changed one child needs to see that the parent no longer
/// speaks for the group. `docs/decisions.md` C26's lesson applies to the
/// transitions between them, not only to the states: a child changed while the
/// parent is on, and every child changed back so the parent returns to a pure
/// state, are the two that a fixture testing one value never sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Every quirk reproduced — the original's behaviour, and the default.
    AllReproduced,
    /// Every quirk fixed.
    AllFixed,
    /// Some of each.
    Mixed,
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

/// One `u64`, little-endian, in the save body and in the per-tick digest.
///
/// It is deliberately **not** a run of bools: a bitfield's width does not
/// change when a quirk is added, which is what keeps `docs/bugs.md` §6.3's
/// *"one save-format version per bug"* from happening.
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

    /// **The bit assignment is a wire format.** A variant that moved would
    /// change what every existing save means, silently and in the direction of
    /// a different game.
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

    /// `ALL` is the enum, not a hand-kept copy of it that drifts. Rust cannot
    /// enumerate a plain enum, so this is the closest thing to a proof: every
    /// entry appears once, at its own bit, in bit order.
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

    /// **Faithful is zero**, which is the whole reason for the inverted sense.
    #[test]
    fn faithful_is_zero_and_reproduces_everything() {
        assert_eq!(Quirks::FAITHFUL.bits(), 0);
        assert_eq!(Quirks::default(), Quirks::FAITHFUL);
        for q in Quirk::ALL {
            assert!(Quirks::FAITHFUL.reproduces(*q), "{}", q.name());
            assert!(!Quirks::FAITHFUL.is_fixed(*q), "{}", q.name());
        }
    }

    /// A bit no quirk claims cannot be set — so a save from a newer build,
    /// read by an older one, cannot turn on a flag this build does not
    /// understand. It reads as faithful instead, which is the answer that
    /// cannot be wrong.
    #[test]
    fn an_unknown_bit_reads_as_faithful() {
        let smuggled = Quirks::from_bits(u64::MAX);
        assert_eq!(smuggled, Quirks::FIXED);
        assert_eq!(smuggled.bits(), Quirks::MASK);
        // And the top bit specifically, which is the one a future variant
        // would not use for a very long time.
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

        // Parent off: all fixed.
        q.set_all(false);
        assert_eq!(q.group(), Group::AllFixed);
        assert_eq!(q.tally(), (0, Quirk::ALL.len()));
        for k in Quirk::ALL {
            assert!(q.is_fixed(*k), "{}", k.name());
        }

        // Parent back on: all reproduced.
        q.set_all(true);
        assert_eq!(q.group(), Group::AllReproduced);

        // A child changed while the parent is pure makes the parent mixed —
        // for **every** child, not for one sampled one.
        for k in Quirk::ALL {
            let mut q = Quirks::FAITHFUL;
            q.toggle(*k);
            assert_eq!(q.group(), Group::Mixed, "{} alone", k.name());
            assert_eq!(q.tally(), (Quirk::ALL.len() - 1, Quirk::ALL.len()));
            // And back: a lone child restored returns the parent to pure.
            q.toggle(*k);
            assert_eq!(q.group(), Group::AllReproduced, "{} restored", k.name());
        }

        // The same from the other pure end.
        for k in Quirk::ALL {
            let mut q = Quirks::FIXED;
            q.toggle(*k);
            assert_eq!(q.group(), Group::Mixed, "{} alone", k.name());
            q.toggle(*k);
            assert_eq!(q.group(), Group::AllFixed, "{} restored", k.name());
        }

        // Every child changed one at a time from faithful walks the parent
        // through Mixed exactly once and lands on AllFixed — the state a
        // "toggle them one by one" player reaches, which a set_all test does
        // not exercise at all.
        let mut q = Quirks::FAITHFUL;
        for (i, k) in Quirk::ALL.iter().enumerate() {
            q.set_reproduced(*k, false);
            let expected = if i + 1 == Quirk::ALL.len() { Group::AllFixed } else { Group::Mixed };
            assert_eq!(q.group(), expected, "after {}", k.name());
        }
        // …and all the way back.
        for (i, k) in Quirk::ALL.iter().enumerate() {
            q.set_reproduced(*k, true);
            let expected =
                if i + 1 == Quirk::ALL.len() { Group::AllReproduced } else { Group::Mixed };
            assert_eq!(q.group(), expected, "after restoring {}", k.name());
        }
    }

    /// `set_reproduced` is idempotent and `toggle` is its own inverse. Both
    /// are what a double click and a stuck key produce.
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
