//! **The seam.** An army reaches an enemy county, a battle is fought, and the
//! result comes back onto the campaign map.
//!
//! Two crates that had never met. `l2-kingdom` takes counties by arithmetic and
//! *reports* the battle case — [`l2_kingdom::conquest::Attack::Battle`] names
//! two unit slots and stops. `l2-sim` fights battles and has never been told
//! where an army comes from. This module is the only place in the workspace
//! that can join them, because it is the only crate that depends on both, and
//! `docs/plan.md`'s one-way dependency rule says it must stay that way: neither
//! simulation learns the other exists.
//!
//! # The whole path, and where each piece lives
//!
//! ```text
//! movement::march              l2-kingdom  the army walks
//! conquest::attack_county      l2-kingdom  a defender is found or levied
//! battle::settlement           l2-kingdom  autocalc? a report? or ask?
//!   ├── battle::auto_resolve   l2-kingdom  strength, a ratio, a percentage
//!   └── fight (here)           l2-sim      figures, cells, ticks, casualties
//! battle::return_to_campaign   l2-kingdom  the county, the moves, the loser
//! battle::disband_defence      l2-kingdom  the levy walks home
//! ```
//!
//! **Only one of those seven steps needs `l2-sim`**, so the
//! other six are in `l2-kingdom` where the campaign can reach them without a
//! battle simulation present. An AI-versus-AI war runs entirely without this
//! module.
//!
//! # What the original decides, and what we decide
//!
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
//! * **Nobody clicks — but only where nobody is watching.** A human side gets
//!   no AI order handler in the original either (`Battle_UpdateAllUnits`
//!   (`0x00489401`) guards on `humanControlled`), and `Battle_Start`
//!   (`0x004778A0`) issues no order to anybody, so an unordered unit there
//!   stands on the cell it deployed on and shoots whatever comes into range —
//!   `BattleMan_FireMissile`'s `Missile_FindTarget` arm carries no human guard.
//!   [`begin_fight`] reproduces that exactly. [`fight`], the **headless** path,
//!   then adds [`charge_for_the_absent_player`], because a battle nobody is at
//!   the keyboard for has no other way to happen; a battle a player watches
//!   gets no such order and his men stand until he moves them.
//!
//!   This used to be inside [`begin_fight`] and therefore in the watched path
//!   too. A player reported *"my men in battle started moving before I
//!   clicked"*. `tests/military.rs`
//!   `a_raised_battle_gives_the_players_own_units_no_orders`.
//!
//! > This list used to carry a fourth entry: *"Missiles do not fly. `l2-sim`
//! > resolves a missile hit but nothing drives reload and flight, so an army of
//! > archers fights as an army of men with bows they do not use. This is the
//! > single largest reason a fought battle here and a fought battle there would
//! > not agree."* It was, and it is closed. `l2-sim` flies them, and the
//! > measurement that said so was the fixture: the same position that was won
//! > by the player with 56 men of 178 is now lost by him, which is what the
//! > saved game records. `tests/seam.rs` asserts the verdict
//! > excusing it.
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

/// How the battle was settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// `FUN_004AAD07`: strength scores, a ratio and a survival percentage.
    Autocalc,
    /// `l2-sim` ran it to a conclusion — [`BattleRunner::conclusion`], which is
    /// `FUN_00477DFC`'s field arms.
    Fought { ticks: u32, cause: End },
    /// `l2-sim` ran it and it was **still going** at [`MAX_TICKS`], so the
    /// winner was settled on the men left standing.
    ///
    /// This is not an outcome the original has. A field battle there ends only
    /// by annihilation or withdrawal and has no clock at all, so reaching this
    /// means our simulation stalled — two sides that cannot find each other, or
/// a melee that cannot resolve. It is a distinct variant
/// so that a caller cannot mistake it for a real result, and so a
    /// test can assert it never happens.
    Stalled { ticks: u32 },
}

/// How long a fought battle may run before we give up on it.
///
/// **The original has no such limit.** `FUN_00477DFC` ends a field battle on
/// one condition — a side's men reaching zero — or on a withdrawal, and it
/// waits as long as that takes. This exists because our simulation can stall
/// where the original would not, and [`Resolution::Stalled`] is how it says so
/// Twelve thousand ticks is several
/// times the longest battle `l2-sim`'s own tests produce.
pub const MAX_TICKS: u32 = 12_000;

/// Ticks between checks that the battle is over. One thought-cycle of the
/// battle AI is 200 frames, so a hundred is fine grain.
const CHECK_EVERY: u32 = 100;

/// Everything one battle did, from the prompt to the county.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleReport {
    /// Which of the original's three settlements this battle qualified for.
    pub settlement: Settlement,
/// How it was settled — a [`Settlement::Prompt`] the player
    /// declined is [`Resolution::Autocalc`].
    pub resolution: Resolution,
    pub verdict: Verdict,
    pub aftermath: Aftermath,
    /// Men a dissolved county defence walked back into its county.
    pub defenders_returned: i32,
    /// The two armies' men before and after, for a caller drawing the
    /// after-battle roster (screen `0x13` draws exactly this).
    pub attacker_men: (i32, i32),
    pub defender_men: (i32, i32),
    /// The county the battle was fought in — the besieged one for an assault.
    pub county: u8,
    /// Whether this was a siege assault. Three of `L2.eng` group 82's seven
    /// banners are unreachable without it, and so is the branch in
    /// [`battle::return_to_campaign`].
    pub is_siege: bool,
    /// The two realms, **captured before the loser was destroyed**.
    ///
    /// [`Verdict`] names two unit *slots* and [`battle::return_to_campaign`]
    /// empties the loser's, so by the time a screen reads this report the only
    /// place the loser's realm still exists is here. It is what
    /// [`BattleReport::outcome`] needs, and without it three of the seven
    /// banners cannot be chosen.
    pub attacker_owner: u8,
    pub defender_owner: u8,
    /// **The two rosters, before and after** — the seven troop counts each side
    /// took onto the field and the seven it brought off.
    ///
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
    ///
    /// All zeroes for a field battle, and all zeroes for a siege that was
/// **calculated**: the accumulators only move while men
    /// are shovelling and shot is landing.
    pub castle_damage: l2_sim::CastleDamage,
}

/// Whether the player took the field, when they were asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// *"Will you take the field?"* — yes. `Battle_Start`.
    TakeTheField,
    /// No. `FUN_0043B622` runs the autocalc and shows the report, so declining
/// is a way out of *watching* it.
    Decline,
}

/// **Turn phase 2, one assault at a time** — the pump [`run_siege_phase`] runs
/// to exhaustion in a loop, exposed so a caller that has a screen can stop
/// between two assaults and ask the player about each.
///
/// It is the same code either way: `run_siege_phase` *is* a `while let` over
/// this, so the two cannot drift apart and the phase's tests cover both.
///
/// **[`SiegePhase::next`] has already launched the assault it hands back.**
/// `Siege_LaunchAssault` breaks the siege link whichever way the assault goes,
/// so a caller that takes a value from `next`
/// siege and fought nothing. The pair is not optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiegePhase {
    cursor: l2_kingdom::siege::SiegeCursor,
    /// Which assault of this phase the next one is, mixed into its seed so two
    /// assaults in one phase do not fight the same battle.
    round: usize,
}

/// A campaign record's seven counts as `l2-sim` troops, plus its mercenary band
/// folded into its own troop type.
///
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

