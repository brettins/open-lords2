//! `FUN_004A6A30` chooses between three settlements and this module honours it
//! exactly — see [`l2_kingdom::battle::Settlement`]. What the original does
//! *inside* a fought battle that we do not:
//!
//! * **The battlefield now has terrain on it.** `Battlefield_BuildRandom`
//!   (`0x0047AAA3`) is read: it builds the field from one `batfield.pl8`
//!   frame's 6,400-byte raster, not from the campaign tile, and
//!   [`l2_sim::terrain::build_field`] reproduces it. What is still ours is
//!   *which* frame — the original walks a 48-entry playlist at `0x0057CAE0`
//! that a new game shuffles and the network syncs; [`crate::batfield`] takes
//!   the map from the battle seed instead. A checkout with no install still
//!   gets [`l2_sim::runner::blank_field`].
//!
//! * **Nobody clicks — but only where nobody is watching.** A human side gets
//!   no AI order handler in the original either (`Battle_UpdateAllUnits`
//!   (`0x00489401`) guards on `humanControlled`), and `Battle_Start`
//!   (`0x004778A0`) issues no order to anybody, so an unordered unit there
//!   stands on the cell it deployed on and shoots whatever comes into range —
//!   `BattleMan_FireMissile`'s `Missile_FindTarget` arm carries no human guard.
//!
//! * **Mercenaries lose their band.** `FUN_0047F474` tells a mercenary figure
//!   from a levied one by a flag on the figure record; `l2_sim::Figure` has no
//! such flag, so a band that goes into a fought battle comes out folded into
//!   its troop type. The autocalc path scales the band correctly.

mod report;
pub use report::*;
mod fight;
pub use fight::*;
mod siege;
pub use siege::*;
mod tests;
pub use tests::*;

use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// `FUN_004AAD07`: strength scores, a ratio and a survival percentage.
    Autocalc,
    /// `l2-sim` ran it to a conclusion — [`BattleRunner::conclusion`], which is
    /// `FUN_00477DFC`'s field arms.
    Fought { ticks: u32, cause: End },
    Stalled { ticks: u32 },
}

/// **The original has no such limit.** `FUN_00477DFC` ends a field battle on
/// one condition — a side's men reaching zero — or on a withdrawal, and it
/// waits as long as that takes. This exists because our simulation can stall
/// where the original would not, and [`Resolution::Stalled`] is how it says so
/// Twelve thousand ticks is several
/// times the longest battle `l2-sim`'s own tests produce.
pub const MAX_TICKS: u32 = 12_000;

const CHECK_EVERY: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleReport {
    pub settlement: Settlement,
    pub resolution: Resolution,
    pub verdict: Verdict,
    pub aftermath: Aftermath,
    pub defenders_returned: i32,
    pub attacker_men: (i32, i32),
    pub defender_men: (i32, i32),
    pub county: u8,
    /// Whether this was a siege assault. Three of `L2.eng` group 82's seven
    /// banners are unreachable without it, and so is the branch in
    /// [`battle::return_to_campaign`].
    pub is_siege: bool,
    pub attacker_owner: u8,
    pub defender_owner: u8,
    /// `FUN_004224E7` is the painter both battle screens share, and mode 1 —
    /// the one screen `0x13` passes — prints the *before* count in parentheses
    /// beside the *after* count on every one of its seven rows. It reads the
    /// before counts out of `DAT_00568420` / `DAT_0056843C`, two arrays screen
    /// `0x12` filled on its way past. This is those two arrays, kept on the
/// report, because the losing record is gone by the
    /// time anybody draws them.
    pub attacker_roster: (Roster, Roster),
    pub defender_roster: (Roster, Roster),
    /// **What the assault did to the castle** — the six numbers
    /// `Siege_RecordCastleDamage` (`0x004784CA`) bills the repair from, kept on
    /// the report so a screen can say *"the walls are breached"* without
    /// re-reading a county that may since have changed hands.
    pub castle_damage: l2_sim::CastleDamage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    TakeTheField,
    /// No. `FUN_0043B622` runs the autocalc and shows the report, so declining
/// is a way out of *watching* it.
    Decline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegePhase {
    cursor: l2_kingdom::siege::SiegeCursor,
    round: usize,
}

/// The eleven-column order is shared: `docs/battle.md` §4.1 has the campaign
/// record, `TROOPS*.ENG` and the `.skr` army record all agreeing on
/// peasant, crossbowman, maceman, swordsman, pikeman, archer, knight — which is
/// [`TroopType`]'s order and [`Troop`]'s alike. The four battle-only slots
/// (`+0x16C + t*2` for `t` 7…10) hold siege engines and oil and are filled by
/// `Army_PrepareForBattle` on the siege path only, which is out of scope.
#[cfg(test)]
fn muster_of(kingdom: &Kingdom, id: usize) -> Option<Vec<(Troop, u32)>> {
    muster_with(kingdom, id, l2_kingdom::siege::BattleEngines::default())
}

