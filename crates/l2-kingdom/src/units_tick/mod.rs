//! `Units_Tick` (`0x004650B0`) and the four phase kick-offs — **what
//! moves on the campaign map, and when**.
//!
//! `crate::movement` can take a unit one step and `crate::conquest` can resolve
//! the castle it arrives at, and between them there was still nothing that
//! *called* either during a turn. This module is the caller.
//!
//! # The finding that shapes the whole file: movement is not in a phase
//!
//! `docs/kingdom.md` §3.1's table says phase 2 is *"army movement, including
//! battle resolution"* and that it waits on unit type 1. Reading `Turn_Tick`
//! and `Units_Tick` end to end says something different, and it changes where
//! the code goes:
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
//! **`Units_Tick` is a sibling of `Turn_Tick`, not a child of it**, it has
//! exactly one call site, and it never reads `g_turnPhase`. Every unit with
//! `moving == 2` takes a step on every tick of the whole turn. What the phases
//! do is *narrower* than the table suggests:
//!
//! | phase | what its first step really does | what it waits for |
//! |---:|---|---|
//! | 2 | `Siege_StartPhase` — validate garrison/besieger links. | the siege cursor sweep, `Siege_TickPhase() == 0` — **not** a unit predicate |
//! | 3 | re-target every transport at its cargo county's anchor and re-path it | no type-4 unit is moving |
//! | 5 | give every peasant mob a destination off a shared county cursor | no type-2 unit is moving |
//! | 6 | `Merchant_AdvanceAll` — the next leg of each merchant's route | no type-3 unit is moving |
//!
//! So the phases **originate** the game's own move orders and then wait for
//! them to finish; they never step anything themselves. A player's order is not
//! in that list at all — `Unit_OrderMove` sets the unit walking directly, and
//! it walks on the next tick whatever phase is current.
//!
//! Two consequences worth stating because a reimplementation gets them wrong by
//! default:
//!
//! * **`docs/kingdom.md` §3.1's phase-2 row is wrong**, and so was
//!   `Phase::ArmyMovement.wait()`. Phase 2 does not wait on armies. See
//!   [`crate::phase::Phase::wait`], which is corrected, and `docs/decisions.md`
//!   C35.
//! * **An army moving is not confined to a phase**, so [`Kingdom::tick_units`]
//! is called on *every* tick by the turn driver, and the per-phase work is
//!   [`Kingdom::begin_unit_phase`].
//!
//! # One tile per tick, not a march
//!
//! `Unit_Step` (`0x00465D28`) loops only on the sub-tile animation code and
//! returns as soon as a tile is committed,
//! per tick**. With an army's 15 points that is five open-ground tiles or
//! fifteen road tiles a season; with the other three types' 10 it is three and
//! ten. [`crate::movement::march`] — which walks to exhaustion — is therefore
//! the right shape for a test and the wrong shape for the turn,
//! battle has to be able to interrupt the sweep between two tiles.
//!
//! # `moving` is really three states, and this crate has two
//!
//! `+0x14C` holds **0 idle, 1 ordered but not started, 2 stepping**. The phase
//! wait predicates (`FUN_004A4F5B`, `FUN_004A4E3D`) are not read-only queries:
//! they promote every 1 to a 2 *and* report that the phase is still busy, which
//! is how a phase both starts and waits for its units with one call.
//!
//! [`crate::unit::Unit::moving`] is a `bool`, and that is adequate here rather
//! than convenient. The 1 → 2 promotion always happens on the same tick
//! the order was given, in the same phase, before `Units_Tick` runs; nothing
//! observes a unit sitting at 1. The one place the distinction has teeth is a
//! *different* phase's units, and this driver steps every kind on every tick
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

/// Two units met and neither will share the tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encounter {
    /// The unit that tried to move — the attacker.
    pub mover: usize,
    /// The unit already standing there — the defender.
    pub occupant: usize,
    /// The county the fight is in, taken from the **occupant**, which is what
    /// `FUN_004A7158` writes to `g_battleCounty`. The mover is still on its own
    /// tile, so its county is the wrong one whenever the pair straddle a
    /// border.
    pub county: u8,
}

/// Something that happened during a tick that the caller has to act on.
///
/// The variants are deliberately *reports*: a battle
/// needs `l2-sim`, which this crate must never depend on, and a county changing
/// hands needs to reach the interface. See [`Contact::Battle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    /// **Two armies met and a battle is due.** The caller fights it and brings
    /// the result back; nothing here resolves it.
    ///
    /// `FUN_004A7158` (`0x004A7158`) is the original's seam. It sets
    /// `g_battleIsSiege = 0`, `g_battleCounty` from the occupant, `g_battleArmyA`
    /// to the mover and `g_battleArmyB` to the occupant, clears both sides'
    /// battle scratch fields, and then either auto-resolves or opens the battle
    /// screen. On the interactive path it sets a latch that **abandons the rest
    /// of the unit sweep for that tick** and puts back the waypoint the step had
    /// already consumed, so the army resumes on the same tile afterwards.
    ///
    /// This driver reproduces the abandon — [`UnitsTick::battle`] stops the
    /// sweep — and does not need to reproduce the waypoint restore, because
    /// [`crate::movement::step`] returns [`Entry::Occupied`] *before* consuming
    /// anything.
    Battle(Encounter),
    /// An army reached a county's castle tile and [`conquest::attack_county`]
    /// said what came of it. A [`Attack::Battle`] here is the same handoff as
    /// [`Contact::Battle`] and is reported separately only because the caller
    /// has to change the county's owner afterwards.
    Castle { unit: usize, county: u8, outcome: Attack },
    /// **An army reached the castle building itself**, and
    /// [`conquest::reach_castle_building`] garrisoned it or laid siege.
    ///
    /// A different tile from [`Contact::Castle`] and a different rule: that one
    /// is the county *town* (plane-0 bit `0x40`) and takes the county; this is
    /// the castle's own 2×2 block (bit `0x80` with a castle standing on it) and
    /// is the only way into either a garrison or a siege.
    CastleBuilding { unit: usize, county: u8, arrival: conquest::CastleArrival },
    /// **Two of an AI realm's armies met and merged into one**, with no order
    /// and no prompt. The mover's slot is gone. See
/// [`Kingdom::merge_on_contact`], and a *person's* two armies do
    /// not do this.
    Merged { mover: usize, into: usize },
    /// A unit's next tile is held by somebody it will not fight — its own side,
/// an ally, or a merchant. The move ends.
    ///
    /// **A divergence, and a known one.** The original lets these through: the
    /// tile record carries a linked list of stacked units and
    /// `Unit_EnterOccupiedTile` returns the ordinary cost code, so the mover
    /// walks on and the pair stack. `crate::unit::Units` answers occupancy from
    /// the unit array and has no stack, so here the move stops instead. Nothing
    /// deadlocks — the unit is re-pathed by its phase next season, and a
    /// player's army can be re-ordered — but a friendly unit is an obstacle
    /// where the original has none. Modelling the stack is a `Units` change and
    /// is not this work.
    Blocked { mover: usize, occupant: usize },
}

/// What became of an army that walked onto its own castle — the three arms of
/// `Army_GarrisonApply` (`0x004A79A3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Garrison {
    /// The castle was empty and this army is now its garrison.
    Took,
    /// A garrison was already in place and the two were merged into it —
    /// `Army_Combine`, so the arriving slot is gone.
    Joined { into: usize },
    /// The two together would be over
    /// [`crate::industry::garrison_cap`], so nobody goes in. The army's
    /// mission is reset to [`crate::ai_army::Mission::SEEK_ENEMY`] and it
    /// stops — which is the original's own answer and is what stops a lord
    /// marching the same men at the same full castle every turn.
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
///
/// **Reported, not stored.** The flag is `g_localPlayer`'s, which is a peer's
/// and not the world's, so the local-player test and the flag itself are
/// `l2_game::tip`'s; this carries the half that is the same on every peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Incursion {
    pub unit: usize,
    /// The army's owner at the crossing.
    pub owner: u8,
    pub county: u8,
}

/// **A `Msg_Enqueue` the unit sweep made**, in the order the sweep made it.
///
/// Two kinds, and they differ in who decides the recipient. A [`Posted::Letter`]
/// is fully addressed by the world — `County_GreetArmy` writes to the army's
/// owner and the invasion letter to the county's —
/// keeps or drops it. A [`Posted::Capture`] is not: `County_ChangeOwner` chooses
/// its letter by comparing realms against `g_localPlayer`, so what is reported
/// is the facts, and the peer chooses. `l2_game::arrival` posts both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posted {
    /// `Unit_EnterCounty` (`0x004ABB36`): a greeting or an invasion letter.
    Letter(crate::diplomacy::Letter),
    /// `County_ChangeOwner` (`0x004A72FE`), reached from `Army_AttackCounty`.
    Capture(crate::conquest::Capture),
}

/// What one call to [`Kingdom::tick_units`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitsTick {
/// How many units entered a tile.
    pub stepped: usize,
    /// In the order they happened, which is ascending slot order.
    pub contacts: Vec<Contact>,
    /// Diplomatic hits earned by trampling, for a caller with a diplomacy
    /// layer. See [`crate::movement::Offence`].
    pub offences: Vec<Offence>,
    /// Armies that crossed into somebody else's county. See [`Incursion`].
    pub incursions: Vec<Incursion>,
    /// The letters the sweep posted, oldest first. See [`Posted`].
    pub posted: Vec<Posted>,
}

impl UnitsTick {
    /// The battle this tick raised, if any. At most one: the sweep stops on the
    /// first,
    pub fn battle(&self) -> Option<Encounter> {
        self.contacts.iter().find_map(|c| match c {
            Contact::Battle(e) => Some(*e),
            Contact::Castle { unit, outcome: Attack::Battle { defender, .. }, county } => {
                Some(Encounter { mover: *unit, occupant: *defender, county: *county })
            }
            _ => None,
        })
    }

    /// Counties that changed hands this tick.
    pub fn captures(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.contacts.iter().filter_map(|c| match c {
            Contact::Castle { unit, county, outcome: Attack::Captured(_) } => Some((*unit, *county)),
            _ => None,
        })
    }
}

/// Realm 6 — the owner byte merchants and peasant mobs carry. Not a realm: it
/// is one past the five, so `Units::realm_totals` and the wage bill
/// never see them.
/// What `Army_GarrisonApply` charges an army for walking into a castle.
pub const GARRISON_MOVE_COST: i32 = 5;

pub const OWNERLESS: u8 = 6;

/// The season every peasant mob is re-targeted in, whatever it was already
/// doing. `g_season == 2`, and season 2 is **spring** (`docs/kingdom.md` §3.3).
pub const MOB_RETARGET_SEASON: u8 = 2;

/// Whether a unit of this kind holds its phase open. A convenience for a
/// caller that has a [`Phase`].
pub fn kind_for_phase(phase: Phase) -> Option<UnitKind> {
    match phase.wait() {
        crate::phase::PhaseWait::Units(kind) => Some(kind),
        _ => None,
    }
}

/// Every unit of a kind, in ascending slot order. Used by the tests and by
/// anything that wants to ask a question of one type.
pub fn ids_of_kind(units: &Units, kind: UnitKind) -> Vec<usize> {
    units.iter().filter(|(_, u)| u.kind == kind).map(|(id, _)| id).collect()
}

/// Rebuild every unit's move allowance from its type, the way each tick handler
/// does before anything else it does.
///
/// `Army_Tick` writes **15** to `+0x154` as its first statement, and the other
/// three write **10** — *every tick*, unconditionally. The field is therefore
/// derived,
/// in `england-turn1.sav` have `moveAllowance = 0` on disk because nothing had
/// ticked them since the load.
///
/// This matters for a unit that arrives from a save or from the levy without
/// one: it would otherwise have an allowance of zero and never take a step.
pub fn refresh_allowances(units: &mut Units) {
    for (_, u) in units.iter_mut() {
        u.move_allowance = u.kind.move_allowance();
    }
}

/// **One tick of walking across the tile the unit is on.** Answers whether the
/// far edge was reached, which is when — and only when — the next tile is
/// entered.
///
/// This is `Unit_StepOnce` (`0x0046634D`)'s **`(field_0x14b & 1) == 0` arm**,
/// and [`crate::movement::step`] is its other one. The original is one
/// function with two halves picked by a latch; here the halves are in the two
/// crates that already own them — the driver decides *when* a tile is entered,
/// the mover decides *what happens* when it is — and [`Unit::at_tile_edge`] is
/// the latch, in the unit record, where the original keeps it.
///
/// ```c
/// cVar1 = onRoad ? 0 : 3;
/// if (cVar1 < ++field_0x14a) {
///     field_0x14a = 0;
///     field_0x149 += (g_multiplayer == 0) ? 2 : 4;
///     if (field_0x149 >= 0x10) { field_0x14b |= 1; field_0x149 = 0; return 2; }
/// }
/// return 1;                       /* still crossing: no tile is entered */
/// ```
///
/// # What it costs a tile, and why this is the whole of the defect
///
/// Sixteen has to be reached in steps of two, so **eight admissions a tile**;
/// off a road only one tick in four is admitted and on a road every one is.
/// Single player, therefore:
///
/// | | admissions | ticks a tile |
/// |---|---:|---:|
/// | road | 8 | **8** |
/// | anything else | 8 | **32** |
///
/// A merchant on the England position walks a ten-tile road route, so its leg
/// takes eighty ticks.
/// tile every tick, which at our 16 ms tick is sixty-two tiles a second, and
/// what a player described as *"they move insanely fast"*. `docs/decisions.md`
/// **C134**.
///
/// **It must not be moved above this crate.**
/// The tick a unit arrives on decides which tick a battle starts on, which
/// county changes hands first, and when a phase's wait comes true; two peers
/// that disagreed about it would be playing different games
/// (`docs/netcode.md` §5). It is in the save and therefore in the digest.
///
/// # A merchant is interpolated exactly like an army — **[V]**, and asked
///
/// *Does he glide, or did the original step him tile to tile?* He glides, in
/// three places and none of them tests the kind byte:
///
/// * `Merchant_Tick` (`0x00465622`) and `Transport_Tick` (`0x00465761`) call
///   `Unit_Step` on every tick they are `moving == 2`, the same statement
///   `Army_Tick` (`0x0046521F`) ends on. `moveAllowance 10` is the turn's
///   budget, not a frame delay.
/// * `Unit_StepOnce` (`0x0046634D`) has no kind test at all: `+0x149` and the
///   road-keyed divider above are every unit's.
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
    // Already at the edge — the original leaves the latch set when a step is
    // refused, and the retry commits without re-crossing the tile.
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
                // `County_MakeIndependent` first — its `Labour_Allocate` deals
                // the population the mob is not yet out of — then the debit.
                self.make_county_independent(id);
                self.counties[id].population -= men;
                revolted = true;
            }
        }
        crate::mob::settle(&mut self.counties[id], &crossing, revolted);
    }
}
