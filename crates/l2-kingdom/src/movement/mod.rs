//! Campaign movement — the flood fill, the path extractor and the stepper.
//! `docs/armies.md` §2.
//!
//! # This is not the battlefield pathfinder, and reusing that one would be a bug
//!
//! `l2-sim` already has a pathfinder
//! It is the wrong one, and the two are different functions in the original
//! with different behaviour, not one function called twice:
//!
//! | | battlefield `Path_Search` `0x0047095E` | campaign `Move_FloodFill` `0x0046F700` |
//! |---|---|---|
//! | grid | 80 × 80 | 64 × 64 |
//! | how cost is charged | weighting **and** deferral: an expensive cell is re-queued `stepCost` times | weighting only — `dist[n] = dist[cur] + cost[n]` |
//! | relaxation | **none** — the first cost written to a cell stands | **full** — a cheaper later route rewrites and re-enqueues |
//! | blocked | sentinels 998 (friendly figure) and 999 (terrain) | cost **0**, and nothing else |
//! | occupancy | routes around friendly figures | **units are invisible to it** |
//! | early out | stops when the destination is reached | no goal test at all; always fills the component |
//! | diagonals | always | **forbidden out of a road tile** |
//!
//! Two of those differences change where an army walks
//! implementation would be a silent divergence.
//! also a hard structural reason: `l2-kingdom` may not depend on `l2-sim`
//! (`docs/netcode.md` D-3 and both crates' manifests), and moving the search
//! into a shared crate would put the battle's 80 × 80 sentinels into the
//! campaign's vocabulary. So: a second pathfinder, deliberately, with the
//! comparison written down.
//!
//! # What was `[I]` in `docs/armies.md` §8 and is now read
//!
//! §8 said *"`Move_FloodFill` (2,115 bytes) was not read … that the fill itself
//! is a cost-weighted breadth-first search is `[I]`"*. It has been read. It is
//! **SPFA — a FIFO-queue Bellman–Ford with full relaxation** — not a
//! breadth-first search
//! relaxation branch a FIFO queue over non-uniform weights produces a field
//! that is not a shortest-cost field at all
//! [`extract_path`] would then be unsound. Everything below is `[D]` from
//! `0x0046F700` unless marked otherwise.

mod pathfinding;
pub use pathfinding::*;
mod stepper;
pub use stepper::*;
mod tests_part;
pub use tests_part::*;

use crate::county::{County, MAX_COUNTIES};
use crate::map::{coords, index, terrain, CampaignMap, CostMap, MAP_DIM, MAP_TILES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{UnitKind, Units, MAX_PATH};

/// The eight neighbours **the fill** expands, in the order it expands them:
/// N, E, S, W, then NE, SE, SW, NW.
///
/// The order is part of the specification
/// detail — but it is *not* the order [`extract_path`] scans, which
/// is clockwise from north. Two adjacent functions in the original, two
/// different orderings, and only the extractor's decides a tie.
pub const FILL_NEIGHBOURS: [(i32, i32); 8] =
    [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];

/// How many of [`FILL_NEIGHBOURS`] are orthogonal, and therefore how many a
/// road tile expands. See [`flood_fill`].
pub const ORTHOGONALS: usize = 4;

/// The eight directions **the extractor** scans, clockwise from north: N, NE,
/// E, SE, S, SW, W, NW. Also the numbering the unit record's `facing` byte
/// uses.
pub const STEP_DIRECTIONS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// The original's frontier queue holds exactly this many cell indices and
/// **wraps**
/// stops early with a half-filled field.
///
/// Reproduced, for the same reason
/// `l2_sim::pathfind::QUEUE_CAP` is: a search that would have overrun must
/// produce the original's result, not a better one. On a 64 × 64 map with a
/// diamond frontier it takes pathological re-relaxation to reach, and probably
/// never happens in play — but "probably" is not something a lockstep peer can
/// rely on, and reproducing it costs one modulo.
pub const QUEUE_CAP: usize = 1024;

/// The value [`flood_fill`] writes into the **start** cell.
///
/// Not 0: 0 is the unvisited sentinel, so the start needs a value of its own
/// and the original uses 1. Every distance in the field is therefore
/// `true cost + 1`, so `Map_DrawPathMarker` computes `distance - 1`
/// before choosing a sprite — that subtraction is undoing this seed, not
/// correcting an off-by-one.
pub const START_DISTANCE: i16 = 1;

/// What the *road-preference* mode rewrites every non-road cost to.
///
/// `if (mode != 0 && cost > 1) cost = 100`. Roads become a hundred times
/// cheaper than anything else — a hard road preference.
pub const ROAD_PREFERENCE_COST: i16 = 100;

/// Who is asking for a path, which in the original selects one of two distance
/// fields and one of two queues.
///
/// **The original's separation is not by player.** `Move_FloodFill`'s first
/// argument is a realm id compared against `g_localPlayer`; every call site but
/// one passes the literal 0, and `Unit_OrderMove` ignores its own parameter and
/// hard-codes 0. So the two fields are really "the local player's" and
/// "everyone else's", and when the human is realm 0 the AI's searches land in
/// the local field too. Reproduced as an enum because the *field* is scratch
/// space and nothing reads it across calls — only the road preference is
/// observable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    /// `mode = 0`. **Every human-ordered move.** Costs are used as they stand.
    Direct,
    /// `mode = 1`. The AI's first attempt: every cost above 1 becomes 100, so
    /// the army road-hugs. The AI falls back to [`Routing::Direct`] when the
    /// road route comes back at 150 steps or more, which is the capacity of the
    /// unit's stored path.
    ///
    /// **A player-visible difference nothing in `docs/armies.md` mentions: AI
    /// armies prefer roads and the player's do not.**
    PreferRoads,
}

impl Routing {
    #[inline]
    fn adjust(self, cost: i16) -> i16 {
        match self {
            Routing::Direct => cost,
            Routing::PreferRoads if cost > 1 => ROAD_PREFERENCE_COST,
            Routing::PreferRoads => cost,
        }
    }
}

/// `g_moveDistLocal` / `g_moveDistOther` — the 64 × 64 `i16` distance field the
/// fill leaves behind
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistanceField {
    dist: Vec<i16>,
    start: (u8, u8),
}

impl DistanceField {
    /// The raw stored value, `true cost + 1`, or 0 for unreached.
    pub fn raw(&self, x: u8, y: u8) -> i16 {
        self.dist[index(x, y)]
    }

    /// The accumulated movement cost of reaching a tile, or `None` if the fill
    /// never got there. The start tile costs 0 — its own terrain is never
    /// charged, because a unit is already standing on it.
    pub fn cost_to(&self, x: u8, y: u8) -> Option<i32> {
        match self.dist[index(x, y)] {
            0 => None,
            d => Some(d as i32 - START_DISTANCE as i32),
        }
    }

    pub fn start(&self) -> (u8, u8) {
        self.start
    }

    /// Every tile the fill reached, in ascending index order. For a preview
    /// overlay, and for tests that need to compare two fields.
    pub fn reached(&self) -> impl Iterator<Item = ((u8, u8), i32)> + '_ {
        self.dist.iter().enumerate().filter_map(|(i, &d)| {
            (d != 0).then(|| (coords(i), d as i32 - START_DISTANCE as i32))
        })
    }
}

/// What [`try_enter`] makes of the tile a unit is about to step onto — the
/// return codes of `Unit_TryEnterTile` (`0x00466C3C`), named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// Return 1 — ordinary ground. Costs 3.
    Open,
    /// Return 3 — a road. Costs 1, and sets the unit's road flag for this step.
    Road,
    /// Return 8 — farmland. `Unit_CrossField` charges 3 and then the general
    /// step charges 3
    Field,
    /// Return 5 — a castle site. The move ends and the step itself charges
    /// nothing.
    ///
    /// **This is the objective, not an obstacle.** The mover's code-5 branch
    /// calls `Transport_Deliver` *and* `Army_AttackCounty`, so stepping onto a
    /// county's castle tile is how a county is taken. `docs/armies.md` §2.2
    /// lists only the transport half. See [`crate::conquest::attack_county`].
    Castle,
    /// Return 6 — a settlement. The move ends; `Unit_TrampleTile` charges 7,
    /// **conditionally**.
    Settlement,
    /// Return 7 — a dwelling plot. The move ends; `Unit_BurnDwelling` charges
    /// 7, conditionally.
    Plot,
    /// Another unit is standing there. The original branches into a merge, a
    /// battle or a capture; none of those is this module's business.
    Occupied(usize),
}

impl Entry {
    /// True for the four codes above 4, which `Unit_StepOnce` returns on
    /// **before** the charge and before the position update. A unit that meets
    /// one of these does not enter the tile at all; anything it is charged
    /// comes from the handler, not from the step.
    pub fn ends_the_move(self) -> bool {
        !matches!(self, Entry::Open | Entry::Road | Entry::Field)
    }
}

/// A diplomatic hit a step earned, for a caller that has a diplomacy layer to
/// apply it to.
///
/// This crate has no diplomacy module yet — `docs/diplomacy.md` is traced and
/// unimplemented — so `Diplo_Offend` is *reported*. The
/// numbers are the original's: **−10 for trampling a field**, and it fires
/// **only when the trampling realm is human**. An AI army wrecks fields for
/// free, which is a fourth `ownerIsHuman` branch to add to the three
/// `docs/battle.md` lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offence {
    /// The realm that has been wronged — the *county's* owner.
    pub against: u8,
    /// The realm that did it.
    pub by: u8,
    /// How much worse the standing gets.
    pub amount: i32,
}

/// What one call to [`step`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The tile classification that decided everything else.
    pub entry: Entry,
    /// Moves charged to the unit by this step.
    pub charged: i32,
/// True when the unit changed tile.
    pub moved: bool,
    /// Set when the unit crossed into a different county — the caller's cue to
    /// run the border-crossing rules and recount the county's troops.
    pub entered_county: Option<u8>,
    /// The army reached a county's castle tile. **This is a capture attempt**,
    /// and the caller resolves it with [`crate::conquest::attack_county`] —
/// which is what [`march_and_fight`] does. Reported
    /// inside [`step`] because taking a county needs the realm array, the
    /// ruleset and the name counters
    /// would be a stepper nothing could test in isolation.
    pub reached_castle: Option<u8>,
    /// **The army reached the castle *building*** — a settlement tile
    /// (plane-0 [`crate::map::flags::SETTLEMENT`], `0x80`) carrying a standing
    /// castle, terrain [`crate::map::terrain::CASTLE_FROM`] … `CASTLE_TO`.
    /// `Unit_Step`'s other code-6 handler.
    ///
    /// **This is a different tile from [`Step::reached_castle`] and a different
    /// rule**
    /// county town* — `docs/decisions.md` C25
    /// name — and walking onto it is how a county is **taken**. `0x80` with a
    /// castle terrain is the castle itself.
    ///
    /// `Unit_ReachCastleBuilding` (`0x004686A0`) splits on ownership:
    /// **`Army_Garrison` when the county is the mover's own, `Army_BeginSiege`
    /// when it is not.** `docs/armies.md` §9's target table. Both need the realm
/// array and one of them opens a screen, so this is reported
    /// resolved — the same division [`Step::reached_castle`] already makes. See
    /// [`crate::conquest::reach_castle_building`], and
    /// [`crate::Kingdom::tick_units`] for the garrison half.
    pub reached_castle_building: Option<u8>,
    /// A field was destroyed, and this is the county that lost it.
    pub field_destroyed: Option<u8>,
    /// A resource site was ruined: the county, and which of its four industry
    /// records went down.
    pub site_ruined: Option<(u8, usize)>,
    /// **A dwelling was burnt down**, and this is the county that lost a
    /// quarter of its people. `Unit_BurnDwelling` (`0x00468AE2`).
    pub dwelling_burnt: Option<u8>,
    pub offence: Option<Offence>,
}

impl Step {
    fn nothing(entry: Entry) -> Step {
        Step {
            entry,
            charged: 0,
            moved: false,
            entered_county: None,
            reached_castle: None,
            reached_castle_building: None,
            field_destroyed: None,
            site_ruined: None,
            dwelling_burnt: None,
            offence: None,
        }
    }
}

/// What trampling a field costs the trampler diplomatically — `Diplo_Offend`'s
/// third argument, `'\n'` = 10.
///
/// **These two lines were silently stolen by a merge.** Two branches added a
/// constant here; the resolution glued this doc onto the head of the other
/// one's and left this constant bare. It compiles, it reads plausibly, and
/// nothing catches it.
pub const FIELD_TRAMPLE_OFFENCE: i32 = 10;

/// What burning a dwelling costs — `Unit_BurnDwelling` (`0x00468AE2`) passes
/// `'\x14'` = 20, and unlike the trample it does so whoever the burner is.
///
/// The rest of `Unit_BurnDwelling` is in the [`Entry::Plot`] arm: the tile
/// rewrite to [`terrain::DWELLING_BURNT`] and the quarter of the population.
/// The `frame = 0x3C` beside it is the renderer's.
///
/// One difference, visible only in a save with a corrupt `g_movingUnit`: the
/// original passes `g_units[g_movingUnit].owner` as the offender
/// the unit it was handed. The mover sets `g_movingUnit` to that unit before
/// the call, so the two are the same value on every path. `[D]`
pub const DWELLING_BURN_OFFENCE: i32 = 20;

/// `(first terrain, ruined terrain, industry record)` for the four ladders
/// `Unit_TrampleTile` branches on. A terrain in `first .. ruined` is ruined *to*
/// the second value; the record is the commodity index in
/// [`crate::county::County::industry`].
pub const RUIN_LADDER: [(u8, u8, usize); 4] = [
    (0, 3, 1),   // iron
    (4, 6, 3),   // stone
    (7, 9, 2),   // weapons
    (10, 12, 0), // wood
];

