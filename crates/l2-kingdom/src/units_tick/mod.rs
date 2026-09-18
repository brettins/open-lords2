//! `Units_Tick` (`0x004650B0`) and the four phase kick-offs — **what
//! moves on the campaign map, and when**.
//!
//! ```c
//! /* the frame loop, 0x004B99C0 */
//! if ((g_battlePhase == 0) && (ticksDue != 0)) {
//!     FUN_0040490D();
//!     Turn_Tick();          /* the seven phases */
//!     Units_Tick();         /* every unit, every tick, whatever the phase */
//! }
//! ```
//!
//! * **`docs/kingdom.md` §3.1's phase-2 row is wrong**, and so was
//!   `Phase::ArmyMovement.wait()`. Phase 2 does not wait on armies. See
//!   [`crate::phase::Phase::wait`], which is corrected, and `docs/decisions.md`
//!   C35.
//!
//! `Unit_Step` (`0x00465D28`) loops only on the sub-tile animation code and
//! returns as soon as a tile is committed,
//! per tick**. With an army's 15 points that is five open-ground tiles or
//! fifteen road tiles a season; with the other three types' 10 it is three and
//! ten. [`crate::movement::march`] — which walks to exhaustion — is therefore
//! the right shape for a test and the wrong shape for the turn,
//! battle has to be able to interrupt the sweep between two tiles.
//!
//! `+0x14C` holds **0 idle, 1 ordered but not started, 2 stepping**. The phase
//! wait predicates (`FUN_004A4F5B`, `FUN_004A4E3D`) are not read-only queries:
//!,
//! state to gate. **`[D]`** — recorded because if a mob is ever seen stepping a
//! tick early, this is the paragraph that is wrong.

mod tick;
pub use tick::*;
mod tests;
pub use tests::*;

use crate::conquest::{self, Attack};
use crate::kingdom::Kingdom;
use crate::merchant;
use crate::movement::{self, Entry, Offence};
use crate::phase::Phase;
use crate::unit::{UnitKind, Units};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encounter {
    pub mover: usize,
    pub occupant: usize,
    /// The county the fight is in, taken from the **occupant**, which is what
    /// `FUN_004A7158` writes to `g_battleCounty`. The mover is still on its own
    /// tile, so its county is the wrong one whenever the pair straddle a
    /// border.
    pub county: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    /// `FUN_004A7158` (`0x004A7158`) is the original's seam. It sets
    /// `g_battleIsSiege = 0`, `g_battleCounty` from the occupant, `g_battleArmyA`
    /// to the mover and `g_battleArmyB` to the occupant, clears both sides'
    /// battle scratch fields, and then either auto-resolves or opens the battle
    /// screen. On the interactive path it sets a latch that **abandons the rest
    /// of the unit sweep for that tick** and puts back the waypoint the step had
    /// already consumed, so the army resumes on the same tile afterwards.
    Battle(Encounter),
    Castle { unit: usize, county: u8, outcome: Attack },
    CastleBuilding { unit: usize, county: u8, arrival: conquest::CastleArrival },
    Merged { mover: usize, into: usize },
    Blocked { mover: usize, occupant: usize },
}

/// What became of an army that walked onto its own castle — the three arms of
/// `Army_GarrisonApply` (`0x004A79A3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Garrison {
    Took,
    Joined { into: usize },
    TooMany,
}

/// **An army crossed into a county its owner does not own** — the comparison
/// `Unit_EnterCounty` (`0x004ABB36`) makes on every border crossing:
///
/// ```c
/// if (g_counties[county].owner != unit.owner) {
///     if (unit.owner == g_localPlayer) DAT_00553210 = 1;      /* the invasion tip */
///     …the taunt, when the county is owned and is the destination…
/// }
/// ```
///
/// A neutral county counts: its owner is 0 and no army's is. `Army_Tick`
/// (`0x0046521F`) is the only caller, so only armies cross this way — a
/// merchant, a cart or a mob never does. `County_GreetArmy` runs first and
/// only enqueues a reply, so the owner compared is the owner at the crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Incursion {
    pub unit: usize,
    pub owner: u8,
    pub county: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posted {
    /// `Unit_EnterCounty` (`0x004ABB36`): a greeting or an invasion letter.
    Letter(crate::diplomacy::Letter),
    /// `County_ChangeOwner` (`0x004A72FE`), reached from `Army_AttackCounty`.
    Capture(crate::conquest::Capture),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitsTick {
    pub stepped: usize,
    pub contacts: Vec<Contact>,
    pub offences: Vec<Offence>,
    pub incursions: Vec<Incursion>,
    pub posted: Vec<Posted>,
}

impl UnitsTick {
    pub fn battle(&self) -> Option<Encounter> {
        self.contacts.iter().find_map(|c| match c {
            Contact::Battle(e) => Some(*e),
            Contact::Castle { unit, outcome: Attack::Battle { defender, .. }, county } => {
                Some(Encounter { mover: *unit, occupant: *defender, county: *county })
            }
            _ => None,
        })
    }

    pub fn captures(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.contacts.iter().filter_map(|c| match c {
            Contact::Castle { unit, county, outcome: Attack::Captured(_) } => Some((*unit, *county)),
            _ => None,
        })
    }
}

pub const GARRISON_MOVE_COST: i32 = 5;

pub const OWNERLESS: u8 = 6;

pub const MOB_RETARGET_SEASON: u8 = 2;

pub fn kind_for_phase(phase: Phase) -> Option<UnitKind> {
    match phase.wait() {
        crate::phase::PhaseWait::Units(kind) => Some(kind),
        _ => None,
    }
}

pub fn ids_of_kind(units: &Units, kind: UnitKind) -> Vec<usize> {
    units.iter().filter(|(_, u)| u.kind == kind).map(|(id, _)| id).collect()
}

/// `Army_Tick` writes **15** to `+0x154` as its first statement, and the other
/// three write **10** — *every tick*, unconditionally. The field is therefore
/// derived,
/// in `england-turn1.sav` have `moveAllowance = 0` on disk because nothing had
/// ticked them since the load.
pub fn refresh_allowances(units: &mut Units) {
    for (_, u) in units.iter_mut() {
        u.move_allowance = u.kind.move_allowance();
    }
}

/// This is `Unit_StepOnce` (`0x0046634D`)'s **`(field_0x14b & 1) == 0` arm**,
/// and [`crate::movement::step`] is its other one. The original is one
/// function with two halves picked by a latch; here the halves are in the two
/// crates that already own them — the driver decides *when* a tile is entered,
/// the mover decides *what happens* when it is — and [`Unit::at_tile_edge`] is
/// the latch, in the unit record, where the original keeps it.
///
/// tile every tick, which at our 16 ms tick is sixty-two tiles a second, and
/// what a player described as *"they move insanely fast"*. `docs/decisions.md`
/// **C134**.
///
/// # A merchant is interpolated exactly like an army — **[V]**, and asked
///
/// * `Merchant_Tick` (`0x00465622`) and `Transport_Tick` (`0x00465761`) call
///   `Unit_Step` on every tick they are `moving == 2`, the same statement
///   `Army_Tick` (`0x0046521F`) ends on. `moveAllowance 10` is the turn's
///   budget, not a frame delay.
///
/// * `Unit_StepOnce` (`0x0046634D`) has no kind test at all: `+0x149` and the
///   road-keyed divider above are every unit's.
///
/// * `Map_DrawArmies` (`0x00408438`) reads its six `8 × 16` offset tables
///   **before** the first `kind` comparison; the only kind-dependent parts are
/// sheet B for a transport, the nudge and the mark-tile size.
///
/// His walk frames advance too, and faster than an army's: `Merchant_Tick`
/// writes `+0x07 = ((facing + 1) & 7) * 6 + g_merchantWalkFrames[+0x1B]` — six
/// frames against the army's three — off the counter bumped in the same branch
/// as `+0x149`. So there is nothing to take off this type.
fn cross_sub_tile(units: &mut Units, id: usize) -> bool {
    let Some(u) = units.get_mut(id) else { return false };
    if u.at_tile_edge {
        return true;
    }
    u.sub_frame = u.sub_frame.wrapping_add(1);
    if u.sub_frame <= crate::tables::SUBTILE_DIVIDER[usize::from(u.on_road)] {
        return false;
    }
    u.sub_frame = 0;
    u.sub_tile = u.sub_tile.saturating_add(crate::tables::SUBTILE_STEP_SOLO);
    if u.sub_tile < crate::tables::SUBTILE_SPAN {
        return false;
    }
    u.at_tile_edge = true;
    u.sub_tile = 0;
    true
}


impl Kingdom {
    /// **`FUN_004ABD0F` (`0x004ABD0F`)** — a peasant mob has crossed into
    /// `county`. Posts one of `L2.eng` 154…156 to its owner, may raise that
    /// county too, and takes ten off its happiness. [`crate::mob`] holds the
    /// decompilation and the four things its shape decides.
    ///
    /// The revolt is `County_RaiseRevolt` (`0x004AC185`) itself, so it carries
    /// the same three writes the season pass makes around
    /// [`crate::unrest::raise_revolt`]: the county goes independent, the
    /// population loses the men who left, and the unrest counter clears —
    /// the last of which [`crate::mob::settle`] does.
    fn mob_crossed_border(&mut self, county: u8, out: &mut UnitsTick) {
        let id = county as usize;
        let Some(c) = self.counties.get(id) else { return };
        let Some(crossing) = crate::mob::crossing(c, county) else { return };
        out.posted.push(Posted::Letter(crossing.letter));

        let mut revolted = false;
        if crossing.raise_revolt {
            if let Some((_slot, men)) = crate::unrest::raise_revolt(
                &self.campaign.map,
                &self.counties[id],
                &mut self.campaign.units,
                id,
                self.year,
            ) {
                self.make_county_independent(id);
                self.counties[id].population -= men;
                revolted = true;
            }
        }
        crate::mob::settle(&mut self.counties[id], &crossing, revolted);
    }
}
