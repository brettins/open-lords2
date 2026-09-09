//! Campaign movement — the flood fill, the path extractor and the stepper.
//! `docs/armies.md` §2.
//!
//! # This is not the battlefield pathfinder, and reusing that one would be a bug
//!
//! `l2-sim` already has a pathfinder, and the obvious economy would be to share
//! it. It is the wrong one, and the two are different functions in the original
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
//! Two of those differences change where an army walks, so a shared
//! implementation would be a silent divergence rather than a saving. There is
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
//! breadth-first search, and the difference is load-bearing: without the
//! relaxation branch a FIFO queue over non-uniform weights produces a field
//! that is not a shortest-cost field at all, and the greedy descent in
//! [`extract_path`] would then be unsound. Everything below is `[D]` from
//! `0x0046F700` unless marked otherwise.

use crate::county::{County, MAX_COUNTIES};
use crate::map::{coords, index, terrain, CampaignMap, CostMap, MAP_DIM, MAP_TILES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{UnitKind, Units, MAX_PATH};

/// The eight neighbours **the fill** expands, in the order it expands them:
/// N, E, S, W, then NE, SE, SW, NW.
///
/// The order is part of the specification rather than an implementation
/// detail — but note that it is *not* the order [`extract_path`] scans, which
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
/// **wraps**, so a fill that outgrows it silently overwrites its own queue and
/// stops early with a half-filled field.
///
/// Reproduced rather than fixed, for the same reason
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
/// `true cost + 1`, which is why `Map_DrawPathMarker` computes `distance - 1`
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
/// fill leaves behind, and the preview reads.
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

/// `Move_FloodFill` (`0x0046F700`) — fill the whole reachable component
/// outward from a tile.
///
/// ```text
/// dist[] = 0;  dist[start] = 1;  queue = [start]
/// while queue not empty:
///     cur = pop
///     dirs = N,E,S,W  and, only if cost[cur] != 1,  NE,SE,SW,NW
///     for nbr in dirs:
///         c = cost[nbr];  if roadPreference and c > 1: c = 100
///         if c == 0: continue                       # impassable
///         if dist[nbr] == 0 or dist[cur] + c < dist[nbr]:
///             dist[nbr] = dist[cur] + c;  push nbr
/// ```
///
/// Four things worth reading twice, because each of them is a place a
/// reasonable reimplementation goes wrong:
///
/// * **The cost charged is the cost of the tile being *entered*.** The start
///   tile's own cost is never paid.
/// * **A cell is re-relaxed and re-queued when a cheaper route arrives.** This
///   is not an optimisation — a FIFO queue over weights of 1, 3, 6 and 100 does
///   not produce distances in sorted order, and without relaxation the field
///   would simply be wrong.
/// * **Cost 0 is impassable and there is no separate blocked mask.** An
///   impassable cell keeps `dist == 0` forever, which is indistinguishable
///   from unreached — deliberately, since neither can be walked to.
/// * **A road tile expands orthogonally only.** `if (cost[cur] != 1)` gates the
///   four diagonals, and the gate reads the *raw* cost, so it holds in both
///   routing modes. `docs/armies.md` §2.3 records the extractor's half of this
///   rule and not the fill's; this is the other half. Off a road a diagonal
///   costs exactly what an orthogonal step costs — no √2, no scaling — so
///   armies prefer diagonals everywhere they are allowed.
///
/// # Where this deliberately departs from the original
///
/// **Bounds.** The original has no `x`/`y` guard anywhere: it is flat index
/// arithmetic on 4,096 cells, so stepping east from `x = 63` wraps into the
/// next row, and expanding a tile in row 0 writes *before* the array — into,
/// among other things, the fill's own queue head cursor. It survives only
/// because every shipped map has an impassable sea border. Reproducing the
/// wrap would let a path teleport across the map edge, and reproducing the
/// underflow is not reproducible behaviour at all, it is memory corruption. So
/// neighbours off the grid are rejected here, which is behaviour-identical
/// wherever the original does not corrupt itself. `[D]` on the absence of the
/// checks; `[I]` that the border is always impassable in practice.
pub fn flood_fill(cost: &CostMap, start: (u8, u8), routing: Routing) -> DistanceField {
    let mut dist = vec![0i16; MAP_TILES];
    let si = index(start.0, start.1);
    dist[si] = START_DISTANCE;

    // The original's circular queue, capacity and wrap included.
    let mut queue = vec![0u16; QUEUE_CAP];
    queue[0] = si as u16;
    let (mut head, mut tail) = (0usize, 1usize);

    while head != tail {
        let cur = queue[head] as usize;
        head = (head + 1) % QUEUE_CAP;
        let d = dist[cur];
        // The raw cost, before the road preference rewrites anything: the gate
        // is on whether this tile *is* a road, not on what it is priced at.
        let on_road = cost.at_index(cur) == crate::tables::STEP_COST_ROAD as i16;
        let dirs = if on_road { ORTHOGONALS } else { FILL_NEIGHBOURS.len() };

        let (cx, cy) = coords(cur);
        for &(dx, dy) in &FILL_NEIGHBOURS[..dirs] {
            let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
            if nx < 0 || ny < 0 || nx >= MAP_DIM as i32 || ny >= MAP_DIM as i32 {
                continue;
            }
            let n = ny as usize * MAP_DIM + nx as usize;
            let c = routing.adjust(cost.at_index(n));
            if c == 0 {
                continue;
            }
            let reached = d.saturating_add(c);
            if dist[n] == 0 || reached < dist[n] {
                dist[n] = reached;
                queue[tail] = n as u16;
                tail = (tail + 1) % QUEUE_CAP;
            }
        }
    }
    DistanceField { dist, start }
}

/// `Move_ExtractPath` (`0x004701AC`) — walk the field downhill from the
/// destination back to the start.
///
/// Returned **in travel order and start-exclusive**, which is the reverse of
/// how the original stores it: `g_pathBuf[0]` is the destination and
/// `Unit_Step` decrements the length before reading, so it consumes the buffer
/// back to front. Storage reversed, consumption reversed, net forward — this
/// returns the net.
///
/// The scan:
///
/// * on a **road** tile (raw cost 1) the direction index steps by **2**,
///   hitting N, E, S, W only, and the full eight-direction pass is re-run
///   verbatim only if that found nothing. `[D]`, and it confirms
///   `docs/armies.md` §2.3.
/// * the running best is seeded with `dist[cur]` itself and the test is a
///   strict `<`, so **only a strictly cheaper neighbour is a candidate and the
///   lowest direction index wins a tie**. That is the whole tie-break rule.
/// * the candidate's *cost* is never consulted — only the distance field, and
///   only the road test on the tile being left.
///
/// **An unreachable destination comes back as an empty path, not as a
/// failure.** `dist[dest] == 0` makes the first `< 2` test true immediately, so
/// the original returns success with `pathLen = 0`, the caller copies it,
/// sets `moveState = 2`, and the army does not move. Reproduced: the order is
/// accepted and nothing happens, which is observable and therefore not ours to
/// improve.
///
/// The result is capped at [`MAX_PATH`]. The original does *not* cap it — the
/// AI checks the length before copying and `Unit_OrderMove` does not, so a path
/// over 150 steps writes past `g_pathBuf`'s 300-byte slot and stores a length
/// the unit's array cannot hold. That one is a buffer overrun rather than a
/// rule, and it is clamped here.
///
/// **`None` and `Some(vec![])` are different answers**, and [`order_move`] acts
/// on the difference: `None` is the original's `return 0`, a dead end in the
/// descent, and makes the whole order a no-op; an empty `Some` is its
/// `return 1` with `pathLen = 0`, which is a *successful* order to walk
/// nowhere.
pub fn extract_path(cost: &CostMap, field: &DistanceField, dest: (u8, u8)) -> Option<Vec<(u8, u8)>> {
    let mut path = Vec::new();
    let (mut x, mut y) = dest;
    let mut here = field.raw(x, y) as i32;

    while here >= START_DISTANCE as i32 + 1 && path.len() < MAX_PATH {
        path.push((x, y));
        let stride = if cost.at(x, y) == crate::tables::STEP_COST_ROAD as i16 { 2 } else { 1 };
        let mut best: Option<(usize, i32, u8, u8)> = None;
        // The first pass; then, only from a road tile, the full eight.
        for stride in [stride, 1] {
            let mut running = here;
            for (d, &(dx, dy)) in STEP_DIRECTIONS.iter().enumerate() {
                if d % stride != 0 {
                    continue;
                }
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= MAP_DIM as i32 || ny >= MAP_DIM as i32 {
                    continue;
                }
                let value = field.raw(nx as u8, ny as u8) as i32;
                if value != 0 && value < running {
                    running = value;
                    best = Some((d, value, nx as u8, ny as u8));
                }
            }
            if best.is_some() || stride == 1 {
                break;
            }
        }
        match best {
            Some((_, value, nx, ny)) => {
                x = nx;
                y = ny;
                here = value;
            }
            // A dead end. The original returns 0 and leaves a partial buffer
            // behind with the length still at the zero `Path_ClearBuf` wrote,
            // so the buffer is unreadable and every caller tests the return.
            None => return None,
        }
    }
    path.reverse();
    Some(path)
}

/// `Unit_OrderMove` (`0x004A98FE`) — rebuild the cost map, fill, extract, and
/// hand the path to the unit.
///
/// ```c
/// Move_BuildCostMap();
/// Move_FloodFill(0, unit.x, unit.y, 0);
/// if (Move_ExtractPath(0, x, y) != 0) {
///     Path_CopyToUnit(0, unit);
///     unit.stepTargetX = unit.destX = x;
///     unit.stepTargetY = unit.destY = y;
///     unit.moveState = 2;
///     unit.orderMode  = mode;
///     unit.orderFlags = 1, 1, 1;
/// }
/// ```
///
/// > **Every write is inside the `if`.** `docs/armies.md` §2.3 renders the
/// > destination and `moveState = 2` as unconditional statements after it; they
/// > are not. A failed extraction leaves the unit exactly as it was — no
/// > destination, not moving, its previous path intact. Corrected in the
/// > document. `[D]`
/// >
/// > Note that this is *not* the same as "an unreachable destination does
/// > nothing": `Move_ExtractPath` returns success with a zero-length path when
/// > the destination was never reached, so that order **is** accepted and the
/// > army stands still with `moveState = 2`. Only a dead end in the descent —
/// > which a fully relaxed field should not produce — fails.
///
/// Note the **cost map is rebuilt on every order**, not once a season — it has
/// four callers and this is one of them, which corrects the "once a season"
/// note on `Move_BuildCostMap` in `docs/symbols.json`. It is 4,096 iterations,
/// so the original can afford it; the point is that a tile trampled two steps
/// ago is already impassable to the next order.
///
/// Returns the number of steps ordered, or `None` if nothing was written.
pub fn order_move(map: &CampaignMap, units: &mut Units, id: usize, dest: (u8, u8), routing: Routing) -> Option<usize> {
    let cost = map.cost_map();
    let unit = units.get(id)?;
    let field = flood_fill(&cost, unit.tile(), routing);
    let path = extract_path(&cost, &field, dest)?;
    let steps = path.len();
    let dest_county = map.county_at(dest.0, dest.1);
    let unit = units.get_mut(id).expect("checked just above");
    unit.path = path;
    unit.dest = Some(dest);
    unit.dest_county = dest_county;
    unit.moving = true;
    unit.needs_destination = false;
    Some(steps)
}

/// The **two-pass** order every non-player mover uses: road-hugging first, and
/// the direct route only if the road route will not fit.
///
/// ```c
/// Move_FloodFill(0, x, y, 1);                       /* prefer roads */
/// if (Move_ExtractPath(0, destX, destY)) {
///     if (g_pathLen < 150) Path_CopyToUnit(0, unit);
///     else { Move_FloodFill(0, x, y, 0);            /* plain costs */
///            if (Move_ExtractPath(0, destX, destY)) Path_CopyToUnit(0, unit); }
/// }
/// ```
///
/// Three callers run it statement for statement — the transport re-target
/// (`FUN_00429418`, turn phase 3), `Merchant_AdvanceAll` (phase 6), and the
/// AI's army walk — against [`order_move`] with [`Routing::Direct`], which is
/// what a *human* order does. So [`Routing::PreferRoads`]'s doc comment is
/// exactly right that "AI armies prefer roads and the player's do not", and the
/// rule is wider than armies: **everything the game moves for itself hugs
/// roads.**
///
/// The 150 is [`crate::unit::MAX_PATH`], the capacity of the unit's stored
/// path. [`extract_path`] already clamps there rather than overrunning the way
/// the original's `Unit_OrderMove` does, so the retry is triggered by the
/// clamp being *reached* — a road route that comes back at exactly the cap is
/// the one that could not be represented.
///
/// Returns the number of steps ordered, or `None` if neither pass produced a
/// path.
pub fn order_move_by_road(map: &CampaignMap, units: &mut Units, id: usize, dest: (u8, u8)) -> Option<usize> {
    let by_road = order_move(map, units, id, dest, Routing::PreferRoads);
    match by_road {
        Some(steps) if steps < crate::unit::MAX_PATH => Some(steps),
        _ => order_move(map, units, id, dest, Routing::Direct).or(by_road),
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
    /// step charges 3, so a standing field costs 6 in total.
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

/// `Unit_TryEnterTile` (`0x00466C3C`) — classify the tile in front.
///
/// The tests are made in this order and the order is the rule, because the bits
/// combine: occupancy first, then road, castle, settlement, plot, farmland,
/// and ordinary ground last. Note this is **not** the cost map's order — the
/// cost map tests farmland before the castle — and the two only agree because
/// no shipped tile carries both bits.
///
/// The settlement bit is masked off first when the terrain is the county town
/// (`0x14`) or when the unit is flagged to ignore settlements, which is what
/// lets an army walk into its own town.
pub fn try_enter(map: &CampaignMap, units: &Units, x: u8, y: u8) -> Entry {
    use crate::map::flags;
    if let Some(other) = units.at(x, y) {
        return Entry::Occupied(other);
    }
    let mut f = map.flags_at(x, y);
    if map.terrain_at(x, y) == terrain::TOWN {
        f &= !flags::SETTLEMENT;
    }
    if f & flags::ROAD != 0 {
        Entry::Road
    } else if f & flags::CASTLE != 0 {
        Entry::Castle
    } else if f & flags::SETTLEMENT != 0 {
        Entry::Settlement
    } else if f & flags::PLOT != 0 {
        Entry::Plot
    } else if f & flags::FARMLAND != 0 {
        Entry::Field
    } else {
        Entry::Open
    }
}

/// The three cases in which an occupied tile is **not** an obstacle —
/// `Unit_EnterOccupiedTile` (`0x004658C1`), read to the end.
///
/// The function opens by computing its return value from one bit:
///
/// ```c
/// local_8 = (flags & 1) ? 3 : 1;                     /* road or open */
/// if (mover.kind == 3) return local_8;               /* a merchant  walks through */
/// if (mover.kind == 4) return local_8;               /* a transport walks through */
/// if (occupant.kind == 3) return local_8;            /* and through a merchant */
/// ...the merge / battle / capture ladder...
/// ```
///
/// **3 and 1 are ordinary Road and Open**, not stop codes: `Unit_StepOnce`
/// returns early only above 4. So a merchant walks *onto and past* whatever is
/// standing in its way, and anything walks past a merchant, at the ordinary
/// cost of the tile — and, because the occupancy test happens first in
/// `Unit_TryEnterTile`, without the field surcharge, the trample, or the castle
/// capture that tile's other bits would otherwise have earned.
///
/// > **Corrected, and this is what the correction was worth.** [`step`] treated
/// > every [`Entry::Occupied`] as the end of the move. Six merchants imported
/// > onto the England map jam within two seasons: three of them meet on the road
/// > junction between counties 11 and 12, each stops in front of the next, and
/// > none of them ever reaches a county again. In the original they walk through
/// > each other. `docs/armies.md` §2.7 described the ladder below the guards and
/// > did not mention them; correction **C40** in `docs/decisions.md`. `[V]` —
/// > four `if`s at the top of one function.
fn pass_through(
    map: &CampaignMap,
    units: &Units,
    kind: UnitKind,
    entry: Entry,
    x: u8,
    y: u8,
) -> Entry {
    let Entry::Occupied(other) = entry else { return entry };
    let mover_ignores = matches!(kind, UnitKind::Merchant | UnitKind::Transport);
    let occupant_is_merchant = units.get(other).map(|u| u.kind) == Some(UnitKind::Merchant);
    if !mover_ignores && !occupant_is_merchant {
        return entry;
    }
    if map.flags_at(x, y) & crate::map::flags::ROAD != 0 {
        Entry::Road
    } else {
        Entry::Open
    }
}

/// A diplomatic hit a step earned, for a caller that has a diplomacy layer to
/// apply it to.
///
/// This crate has no diplomacy module yet — `docs/diplomacy.md` is traced and
/// unimplemented — so `Diplo_Offend` is *reported* rather than invented. The
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
    /// True when the unit actually changed tile.
    pub moved: bool,
    /// Set when the unit crossed into a different county — the caller's cue to
    /// run the border-crossing rules and recount the county's troops.
    pub entered_county: Option<u8>,
    /// The army reached a county's castle tile. **This is a capture attempt**,
    /// and the caller resolves it with [`crate::conquest::attack_county`] —
    /// which is what [`march_and_fight`] does. Reported rather than resolved
    /// inside [`step`] because taking a county needs the realm array, the
    /// ruleset and the name counters, and a stepper that took all of those
    /// would be a stepper nothing could test in isolation.
    pub reached_castle: Option<u8>,
    /// **The army reached the castle *building*** — a settlement tile
    /// (plane-0 [`crate::map::flags::SETTLEMENT`], `0x80`) carrying a standing
    /// castle, terrain [`crate::map::terrain::CASTLE_FROM`] … `CASTLE_TO`.
    /// `Unit_Step`'s other code-6 handler.
    ///
    /// **This is a different tile from [`Step::reached_castle`] and a different
    /// rule**, and the two are easy to cross. `flags::CASTLE` (`0x40`) is *the
    /// county town* — `docs/decisions.md` C25, and the constant keeps the wrong
    /// name — and walking onto it is how a county is **taken**. `0x80` with a
    /// castle terrain is the castle itself.
    ///
    /// `Unit_ReachCastleBuilding` (`0x004686A0`) splits on ownership:
    /// **`Army_Garrison` when the county is the mover's own, `Army_BeginSiege`
    /// when it is not.** `docs/armies.md` §9's target table. Both need the realm
    /// array and one of them opens a screen, so this is reported rather than
    /// resolved — the same division [`Step::reached_castle`] already makes. See
    /// [`crate::conquest::reach_castle_building`], and
    /// [`crate::Kingdom::tick_units`] for the garrison half.
    pub reached_castle_building: Option<u8>,
    /// A field was destroyed, and this is the county that lost it.
    pub field_destroyed: Option<u8>,
    /// A resource site was ruined: the county, and which of its four industry
    /// records went down.
    pub site_ruined: Option<(u8, usize)>,
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
            offence: None,
        }
    }
}

/// `Unit_StepOnce` (`0x0046634D`) — take exactly one step along the unit's
/// path.
///
/// ```c
/// unit.onRoad = 0;                       /* cleared every step */
/// if (code == 3)      unit.onRoad = 1;
/// else if (code == 8) { Unit_CrossField(); code = 1; }
/// else if (code > 4)  return code;       /* 5, 6, 7, 10: no charge, no move */
/// unit.movesUsed += unit.onRoad ? 1 : 3;
/// unit.facing = dir;
/// ...move...
/// ```
///
/// **The road flag is per-step, not sticky**, which is easy to get wrong: it is
/// cleared at the top of every step and set again only if the tile just
/// classified as a road. An army that walks off a road pays 3 again
/// immediately.
///
/// Returns `None` when the unit is not there, has no path left, or has no moves
/// left. The budget test is `moveAllowance <= movesUsed`, so a unit with
/// exactly its allowance spent stops.
pub fn step(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    id: usize,
) -> Option<Step> {
    let (next, kind, owner, owner_is_human) = {
        let u = units.get(id)?;
        if u.path.is_empty() || u.moves_used >= u.move_allowance {
            return None;
        }
        (u.path[0], u.kind, u.owner, u.owner_is_human)
    };
    let (nx, ny) = next;
    let entry = pass_through(map, units, kind, try_enter(map, units, nx, ny), nx, ny);
    let tile_county = map.county_at(nx, ny);
    let mut out = Step::nothing(entry);

    match entry {
        // Everything above code 4 returns before the charge and before the
        // move. What the handler does is the handler's business; the step
        // itself does nothing at all.
        Entry::Castle => {
            out.reached_castle = Some(tile_county);
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Occupied(_) => {
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Settlement => {
            // **Two branches wrote this arm on the same day and only one of
            // them found the guard.** `ai-lords-play` reported the castle and
            // trampled unconditionally, which is the behaviour the line below
            // corrects; both agreed exactly on the tile — plane-0 `0x80` with
            // terrain `0x15 … 0x19`, garrison if yours and besiege if not —
            // and on the name and type of the field it sets.
            // **Code 6 is two handlers, not one**, and this arm only had the
            // first — so an army that walked into a castle *trampled* it:
            //
            // ```c
            // if (terrain < 0x10)                     Unit_TrampleTile(...);
            // if (0x14 < terrain && terrain < 0x1A)   Unit_ReachCastleBuilding(...);
            // ```
            //
            // Two disjoint ranges with a gap between them: `0x10 … 0x14` — an
            // occupied dwelling and the bare castle plot — does neither.
            // `Unit_ReachCastleBuilding` (`0x004686A0`) is the whole route into
            // both garrisoning and besieging, and without it a county's castle
            // could never be manned and never be besieged from the map. `[V]`
            let t = map.terrain_at(nx, ny);
            if t < terrain::DWELLING {
                let r = trample(map, counties, realms, units, id, nx, ny);
                out.charged = r.0;
                out.site_ruined = r.1;
            }
            if (terrain::CASTLE_FROM..=terrain::CASTLE_TO).contains(&t) {
                out.reached_castle_building = Some(tile_county);
            }
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Plot => {
            // `Unit_BurnDwelling` (`0x00468AE2`) charges +7, exactly like
            // trampling — the row `docs/armies.md` §2.2 leaves as "—". It fires
            // only on an occupied plot in a county the unit does not own.
            if kind != UnitKind::Merchant
                && kind != UnitKind::Transport
                && map.terrain_at(nx, ny) == terrain::DWELLING
                && counties.get(tile_county as usize).map(|c| c.owner) != Some(owner)
            {
                let u = units.get_mut(id)?;
                u.moves_used += crate::tables::STEP_COST_TRAMPLE;
                out.charged = crate::tables::STEP_COST_TRAMPLE;
            }
            units.get_mut(id)?.moving = false;
            return Some(out);
        }
        Entry::Field => {
            let (charged, destroyed, offence) =
                cross_field(map, counties, units, id, nx, ny, owner, owner_is_human);
            out.charged += charged;
            out.field_destroyed = destroyed;
            out.offence = offence;
        }
        Entry::Open | Entry::Road => {}
    }

    let u = units.get_mut(id)?;
    u.on_road = entry == Entry::Road;
    let base = if u.on_road {
        crate::tables::STEP_COST_ROAD
    } else {
        crate::tables::STEP_COST_OPEN
    };
    u.moves_used += base;
    out.charged += base;

    let (dx, dy) = (nx as i32 - u.x as i32, ny as i32 - u.y as i32);
    if let Some(d) = STEP_DIRECTIONS.iter().position(|&s| s == (dx, dy)) {
        u.facing = d as u8;
    }
    u.x = nx;
    u.y = ny;
    u.path.remove(0);
    out.moved = true;

    // `Army_Tick` keeps `+0x10` equal to the tile's county byte and calls
    // `Unit_EnterCounty` whenever it changes.
    if u.county != tile_county {
        u.county = tile_county;
        out.entered_county = Some(tile_county);
    }
    if u.path.is_empty() {
        u.moving = false;
        u.needs_destination = true;
    }
    Some(out)
}

/// Walk a unit until it runs out of moves, runs out of path, or is stopped.
///
/// The natural shape of a turn's worth of movement, and the loop the caller
/// would otherwise write. Returns every step taken, in order, so a caller can
/// raise the border messages and recount the counties without re-deriving what
/// happened.
pub fn march(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    id: usize,
) -> Vec<Step> {
    let mut steps = Vec::new();
    while let Some(s) = step(map, counties, realms, units, id) {
        let stop = s.entry.ends_the_move();
        steps.push(s);
        if stop {
            break;
        }
    }
    steps
}

/// `Unit_CrossField` (`0x0046673C`) — the extra 3 moves, and the field.
///
/// ```c
/// if (type != 3 && type != 4) {
///     unit.movesUsed += 3;
///     if (unit.owner != county[tileCounty].owner && tileIsField(tile)) {
///         if (realm[unit.owner].isHuman) Diplomacy_Worsen(countyOwner, unit.owner, 10);
///         County_DestroyField(tile);
///     }
/// }
/// ```
///
/// > **Can an army destroy its own fields? On this path, no.** The guard is
/// > `unit.owner != countyOwner`, tested against the county the *tile* belongs
/// > to. It does damage **neutral** counties, because owner 0 is no realm.
/// > `[D]` — a plain reading of one `if`, and a negative claim, which is the
/// > weaker kind.
///
/// The +3 is charged for **every** non-merchant crossing, including one through
/// your own fields: a farm is slow going whoever owns it, and only the damage
/// is conditional.
fn cross_field(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    units: &mut Units,
    id: usize,
    x: u8,
    y: u8,
    owner: u8,
    owner_is_human: bool,
) -> (i32, Option<u8>, Option<Offence>) {
    let Some(u) = units.get(id) else { return (0, None, None) };
    if matches!(u.kind, UnitKind::Merchant | UnitKind::Transport) {
        return (0, None, None);
    }
    if let Some(u) = units.get_mut(id) {
        u.moves_used += crate::tables::STEP_COST_FIELD_EXTRA;
    }
    let charged = crate::tables::STEP_COST_FIELD_EXTRA;

    let tile_county = map.county_at(x, y);
    let county_owner = counties.get(tile_county as usize).map_or(0, |c| c.owner);
    if county_owner == owner || !map.is_standing_field(x, y) {
        return (charged, None, None);
    }
    let offence = owner_is_human.then_some(Offence {
        against: county_owner,
        by: owner,
        amount: FIELD_TRAMPLE_OFFENCE,
    });
    if let Some(county) = counties.get_mut(tile_county as usize) {
        destroy_field(county, map, x, y);
    }
    (charged, Some(tile_county), offence)
}

/// What trampling a field costs the trampler diplomatically — `Diplo_Offend`'s
/// third argument, `'\n'` = 10.
///
/// **These two lines were silently stolen by a merge.** Two branches added a
/// constant here; the resolution glued this doc onto the head of the other
/// one's and left this constant bare. It compiles, it reads plausibly, and
/// nothing catches it.
pub const FIELD_TRAMPLE_OFFENCE: i32 = 10;

/// `County_DestroyField` (`0x00469E5B`) — remove one field and its share of
/// what it was carrying.
///
/// ```c
/// if (terrain < 0x0F && fieldsSown != 0) {                  /* a grain field */
///     share = (fieldsGrain <= fieldsSown) ? PctOf(1, fieldsSown) : 0;
///     lost  = Pct(crop, min(share, 100));
///     crop -= lost;   cropLost += lost;
///     if (fieldsGrain <= fieldsSown) fieldsSown--;
///     fieldsGrain--;
/// } else if (terrain >= 0x0F && fieldsCattle != 0 && herd != 0) {   /* a pasture */
///     lost = Pct(herd, PctOf(1, fieldsCattle));
///     herd -= lost;   herdLost += lost;
///     fieldsCattle--;
/// }
/// ```
///
/// **`PctOf(1, n)` is `100 / n`** — what share of the county's fields this one
/// field is — and `Pct(crop, share)` then takes that share of the crop. So
/// wrecking one of eight grain fields costs an eighth of the standing crop, and
/// the crop that goes is `+0x244`, the middle of the three growth stages.
///
/// > **Two divergences from the original, both stated rather than hidden.**
/// > The original keeps a *second* field counter at `+0x206` — the fields
/// > actually **sown** this season, snapshotted from `fields_grain` by the
/// > sowing pass — and divides by that one while decrementing both.
/// > [`County`] has no such field, so this divides by `fields_grain`. They are
/// > set equal at sowing and only drift if the player reassigns fields
/// > mid-year, so the two agree for the season a crop is actually standing,
/// > which is the only season this function can fire. The original also
/// > accumulates what was lost into two display fields (`+0x234` and `+0x27C`)
/// > that [`County`] does not carry; the loss is returned instead.
pub fn destroy_field(county: &mut County, map: &mut CampaignMap, x: u8, y: u8) -> i32 {
    use crate::math::{pct, pct_of};
    let terrain_byte = map.terrain_at(x, y);
    let lost = if terrain_byte < terrain::PASTURE_FROM {
        if county.fields_grain <= 0 {
            return 0;
        }
        let share = pct_of(1, county.fields_grain).min(100);
        let lost = pct(county.crop[1], share);
        county.crop[1] -= lost;
        county.fields_grain -= 1;
        lost
    } else {
        if county.fields_cattle <= 0 || county.herd <= 0 {
            return 0;
        }
        let share = pct_of(1, county.fields_cattle);
        let lost = pct(county.herd, share);
        county.herd -= lost;
        county.fields_cattle -= 1;
        lost
    };
    // The tile is repainted as bare ground, which also takes its cost back down
    // from 6 to 3.
    map.set_terrain(x, y, 0);
    lost
}

/// `Unit_TrampleTile` (`0x0046873F`) — an army marching over a resource site
/// shuts it down.
///
/// **This is the writer of `disabled_seasons` that
/// `crates/l2-kingdom`'s [`crate::county::Industry`] had none of.** Four
/// terrain ladders, one per industry record, and each writes the same three
/// things:
///
/// | terrain | ruined to | industry record |
/// |---|---:|---:|
/// | 0…2 | 3 | 1 — iron |
/// | 4…5 | 6 | 3 — stone |
/// | 7…8 | 9 | 2 — weapons |
/// | 10…11 | 12 | 0 — wood |
///
/// **`[V]` the placer and the trampler agree three for three.**
/// `County_PlaceResourceSites` puts record 1 on terrain 1, record 3 on terrain
/// 4 and record 0 on terrain 10; the fourth ladder targets record 2, the one
/// with no map site, which is the blacksmith. Two functions sharing no data
/// path pick the same four records from the same four terrain groups.
///
/// **It always writes 3 seasons** — no ladder, no dependence on the army's
/// size — and only when the army's owner differs from the *county's* owner, so
/// you cannot wreck your own. The `+7` is charged **inside** each branch, so a
/// site that is already ruined costs nothing and an army walking over its own
/// county's mine costs nothing either: `docs/armies.md` §2.2's flat "+7" for a
/// settlement is conditional on both.
fn trample(
    map: &mut CampaignMap,
    counties: &mut [County; MAX_COUNTIES],
    _realms: &[Realm; MAX_REALMS],
    units: &mut Units,
    id: usize,
    x: u8,
    y: u8,
) -> (i32, Option<(u8, usize)>) {
    let Some(u) = units.get(id) else { return (0, None) };
    if matches!(u.kind, UnitKind::Merchant | UnitKind::Transport) {
        return (0, None);
    }
    let owner = u.owner;
    let tile_county = map.county_at(x, y);
    let Some(county) = counties.get_mut(tile_county as usize) else { return (0, None) };
    if county.owner == owner {
        return (0, None);
    }
    let t = map.terrain_at(x, y);
    let Some((ruined, record)) = RUIN_LADDER.iter().copied().find(|&(from, to, _)| t >= from && t < to).map(|(_, to, r)| (to, r)) else {
        return (0, None);
    };
    if t == ruined {
        // Already a ruin. No charge, no second shutdown.
        return (0, None);
    }
    map.set_terrain(x, y, ruined);
    county.industry[record].efficiency = 0;
    county.industry[record].disabled_seasons = crate::tables::TRAMPLE_DISABLED_SEASONS;
    if let Some(u) = units.get_mut(id) {
        u.moves_used += crate::tables::STEP_COST_TRAMPLE;
    }
    (crate::tables::STEP_COST_TRAMPLE, Some((tile_county, record)))
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::flags;
    use crate::unit::Unit;

    fn open_map() -> CampaignMap {
        let mut m = CampaignMap::empty();
        for i in 0..MAP_TILES {
            m.county[i] = 1;
        }
        m
    }

    fn blank() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        (core::array::from_fn(|_| County::new()), core::array::from_fn(|_| Realm::new()))
    }

    fn army_at(units: &mut Units, owner: u8, x: u8, y: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, x, y);
        u.men = 200;
        u.county = 1;
        units.spawn(u).unwrap()
    }

    // --- the fill ----------------------------------------------------------

    /// The start cell is seeded with 1 and its own cost is never charged, so
    /// every reported cost is `raw - 1`.
    #[test]
    fn the_start_costs_nothing_and_a_neighbour_costs_its_own_tile() {
        let f = flood_fill(&open_map().cost_map(), (10, 10), Routing::Direct);
        assert_eq!(f.raw(10, 10), START_DISTANCE);
        assert_eq!(f.cost_to(10, 10), Some(0));
        assert_eq!(f.cost_to(11, 10), Some(3), "one step of open ground");
        assert_eq!(f.cost_to(11, 11), Some(3), "a diagonal costs the same");
        assert_eq!(f.cost_to(13, 10), Some(9));
    }

    #[test]
    fn an_impassable_tile_is_never_reached_and_reads_as_unreached() {
        let mut m = open_map();
        // Wall the destination in.
        for (dx, dy) in STEP_DIRECTIONS {
            m.set_flags((20 + dx) as u8, (20 + dy) as u8, flags::NO_COUNTY);
        }
        let f = flood_fill(&m.cost_map(), (5, 5), Routing::Direct);
        assert_eq!(f.cost_to(20, 20), None, "walled in");
        assert_eq!(f.cost_to(20, 19), None, "and the wall itself is unreached");
        // Everything outside the ring is still reachable, at the diagonal
        // distance the fill charges: 13 steps of open ground.
        assert_eq!(f.cost_to(18, 18), Some(3 * 13));
    }

    /// The half of the road rule `docs/armies.md` records only for the
    /// extractor: **a road tile expands orthogonally only.**
    #[test]
    fn a_road_tile_expands_orthogonally_only() {
        let mut m = open_map();
        m.set_flags(10, 10, flags::ROAD);
        let f = flood_fill(&m.cost_map(), (10, 10), Routing::Direct);
        // The four orthogonals are one road-step away.
        for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
            let (x, y) = ((10 + dx) as u8, (10 + dy) as u8);
            assert_eq!(f.cost_to(x, y), Some(3), "{x},{y} is one open tile away");
        }
        // The diagonal is *not* reached directly off the road: it costs two
        // steps round, not one.
        assert_eq!(f.cost_to(11, 11), Some(6), "the diagonal had to go round");

        // Off a road, the diagonal is one step.
        let plain = flood_fill(&open_map().cost_map(), (10, 10), Routing::Direct);
        assert_eq!(plain.cost_to(11, 11), Some(3));
    }

    /// Relaxation, which is what makes this SPFA rather than a breadth-first
    /// search: an expensive first arrival is corrected by a cheaper later one.
    #[test]
    fn a_cheaper_route_arriving_later_rewrites_the_cost() {
        let mut m = open_map();
        // A wall of standing crop (cost 6) with a cheap road running round it.
        for y in 8..=12u8 {
            m.set_flags(11, y, flags::FARMLAND);
            m.set_terrain(11, y, 10);
        }
        let cost = m.cost_map();
        assert_eq!(cost.at(11, 10), 6);
        let f = flood_fill(&cost, (10, 10), Routing::Direct);
        // Straight through is 6; round the top of the wall is 3+3+3 = 9. The
        // direct route wins and is what is recorded.
        assert_eq!(f.cost_to(11, 10), Some(6));

        // Now make the direct crossing ruinous and the detour cheap, and check
        // the field takes the detour's number rather than the first arrival's.
        let mut m = open_map();
        for y in 0..MAP_DIM as u8 {
            if y != 0 {
                m.set_flags(11, y, flags::NO_COUNTY);
            }
        }
        let f = flood_fill(&m.cost_map(), (10, 10), Routing::Direct);
        // The only way round is over row 0, and the diagonals cut the corners:
        // nine steps up to (10, 1), one diagonal to (11, 0), one more to
        // (12, 1), then nine down. Twenty steps of open ground.
        assert_eq!(f.cost_to(12, 10), Some(3 * 20));
    }

    /// Road preference is the AI's, and it is not subtle.
    #[test]
    fn preferring_roads_prices_everything_else_at_a_hundred() {
        let mut m = open_map();
        for x in 0..20u8 {
            m.set_flags(x, 10, flags::ROAD);
        }
        let cost = m.cost_map();
        let direct = flood_fill(&cost, (0, 10), Routing::Direct);
        let hugging = flood_fill(&cost, (0, 10), Routing::PreferRoads);
        assert_eq!(direct.cost_to(10, 10), Some(10));
        assert_eq!(hugging.cost_to(10, 10), Some(10), "a road is 1 in both modes");
        // **13, not 11** — and the 2 is the road's diagonal rule, not a
        // rounding. A road tile expands orthogonally only, so the cheapest way
        // onto (10, 11) is to walk the road to (10, 10) at 10 and step south
        // for 3, rather than cutting the corner diagonally from (9, 10) at 9.
        assert_eq!(direct.cost_to(10, 11), Some(13), "one step off the road");
        assert_eq!(hugging.cost_to(10, 11), Some(110), "…and a hundred to the road-hugger");
    }

    #[test]
    fn the_fill_never_walks_off_the_edge_of_the_map() {
        let f = flood_fill(&open_map().cost_map(), (0, 0), Routing::Direct);
        // If the original's row wrap were reproduced, (63, 0) would be one
        // step west of (0, 0) rather than sixty-three steps east.
        assert_eq!(f.cost_to(63, 0), Some(3 * 63));
        assert_eq!(f.cost_to(1, 0), Some(3));
    }

    #[test]
    fn the_same_map_always_produces_the_same_field() {
        let m = open_map();
        let first = flood_fill(&m.cost_map(), (7, 9), Routing::Direct);
        for _ in 0..5 {
            assert_eq!(flood_fill(&m.cost_map(), (7, 9), Routing::Direct), first);
        }
    }

    // --- extraction --------------------------------------------------------

    #[test]
    fn a_path_is_returned_in_travel_order_without_the_starting_tile() {
        let m = open_map();
        let cost = m.cost_map();
        let f = flood_fill(&cost, (10, 10), Routing::Direct);
        let path = extract_path(&cost, &f, (14, 10)).expect("a clear route");
        assert_eq!(path.len(), 4);
        assert_eq!(*path.last().unwrap(), (14, 10));
        assert!(!path.contains(&(10, 10)), "the starting tile is not in the path");

        // **And it is not the straight line**, which is worth asserting rather
        // than assuming. On open ground a diagonal costs exactly what an
        // orthogonal step costs, so every route of four steps ties; the
        // extractor scans N, NE, E, SE, S, SW, W, NW and takes the first
        // strictly-cheaper neighbour, so **SW is reached before W** and the
        // descent leans south-west before turning back. The zigzag is the
        // original's tie-break made visible.
        assert_eq!(path, vec![(11, 11), (12, 12), (13, 11), (14, 10)]);
    }

    #[test]
    fn every_step_of_an_extracted_path_is_to_a_neighbouring_tile() {
        let mut m = open_map();
        for y in 0..MAP_DIM as u8 {
            if y != 40 {
                m.set_flags(20, y, flags::NO_COUNTY);
            }
        }
        let cost = m.cost_map();
        let f = flood_fill(&cost, (5, 10), Routing::Direct);
        let path = extract_path(&cost, &f, (35, 10)).expect("through the gap");
        assert!(!path.is_empty());
        let mut prev = (5u8, 10u8);
        for &(x, y) in &path {
            let d = ((x as i32 - prev.0 as i32).abs()).max((y as i32 - prev.1 as i32).abs());
            assert_eq!(d, 1, "{prev:?} -> {:?} is not one step", (x, y));
            assert_ne!(cost.at(x, y), 0, "walked onto impassable ground");
            prev = (x, y);
        }
        assert_eq!(*path.last().unwrap(), (35, 10));
        assert!(path.iter().any(|&(x, y)| x == 20 && y == 40), "through the one gap");
    }

    /// **An unreachable destination is accepted and does nothing** — the
    /// original returns success with a zero-length path, so the order is taken
    /// and the army stands still.
    #[test]
    fn an_unreachable_destination_gives_an_empty_path_rather_than_a_failure() {
        let mut m = open_map();
        for (dx, dy) in STEP_DIRECTIONS {
            m.set_flags((30 + dx) as u8, (30 + dy) as u8, flags::NO_COUNTY);
        }
        let cost = m.cost_map();
        let f = flood_fill(&cost, (5, 5), Routing::Direct);
        assert_eq!(extract_path(&cost, &f, (30, 30)), Some(Vec::new()), "reached the descent and stopped at once");

        let mut units = Units::new();
        let id = army_at(&mut units, 1, 5, 5);
        assert_eq!(order_move(&m, &mut units, id, (30, 30), Routing::Direct), Some(0));
        assert!(units.get(id).unwrap().moving, "the order is still accepted");
        assert_eq!(units.get(id).unwrap().dest, Some((30, 30)));
    }

    #[test]
    fn a_path_is_capped_at_a_hundred_and_fifty_steps() {
        let m = open_map();
        let cost = m.cost_map();
        let f = flood_fill(&cost, (0, 0), Routing::Direct);
        assert!(extract_path(&cost, &f, (63, 63)).expect("open ground").len() <= MAX_PATH);
    }

    // --- stepping ----------------------------------------------------------

    #[test]
    fn a_road_step_costs_one_and_an_open_step_costs_three() {
        let mut m = open_map();
        for x in 10..14u8 {
            m.set_flags(x, 10, flags::ROAD);
        }
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (15, 10), Routing::Direct);

        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        let charged: Vec<i32> = steps.iter().map(|s| s.charged).collect();
        // (11,10) (12,10) (13,10) are road; (14,10) and (15,10) are open.
        assert_eq!(charged, vec![1, 1, 1, 3, 3]);
        assert_eq!(units.get(id).unwrap().tile(), (15, 10));
        assert_eq!(units.get(id).unwrap().moves_used, 9);
    }

    /// The road flag is per-step. An army that steps off a road pays 3 again on
    /// the very next tile.
    #[test]
    fn the_road_flag_does_not_stick_after_leaving_the_road() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::ROAD);
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (13, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(steps.iter().map(|s| s.charged).collect::<Vec<_>>(), vec![1, 3, 3]);
        assert!(!units.get(id).unwrap().on_road);
    }

    #[test]
    fn an_army_stops_when_its_fifteen_moves_are_spent() {
        let mut m = open_map();
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (30, 10), Routing::Direct);
        let ordered = order_move(&m, &mut units, id, (30, 10), Routing::Direct).unwrap();
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(steps.len(), 5, "fifteen moves at three a tile");
        assert_eq!(units.get(id).unwrap().moves_left(), 0);
        let left = units.get(id).unwrap().path.len();
        assert_eq!(left, ordered - 5, "the rest of the path is kept");

        // Next season it picks the path up where it left off, and five more
        // tiles come off it.
        units.reset_moves();
        march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(units.get(id).unwrap().path.len(), left - 5);
        assert_eq!(units.get(id).unwrap().moves_used, 15);
    }

    /// A standing field costs 6: `Unit_CrossField`'s 3 plus the step's own 3,
    /// which is exactly what the cost map holds for the same tile.
    #[test]
    fn crossing_a_standing_field_costs_six_and_matches_the_cost_map() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        assert_eq!(m.cost_map().at(11, 10), 6, "the cost map's number");

        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (11, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(steps[0].charged, 6, "and the stepper's, assembled from two threes");
    }

    /// The ownership guard: you cannot wreck your own fields on this path, and
    /// a neutral county's fields are fair game because owner 0 is no realm.
    #[test]
    fn an_army_tramples_a_foreign_field_and_not_its_own() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].fields_grain = 4;
        counties[1].crop[1] = 400;

        // Mine: crossed, charged, untouched.
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (11, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(steps[0].charged, 6);
        assert_eq!(steps[0].field_destroyed, None);
        assert_eq!(counties[1].fields_grain, 4);

        // Theirs: crossed, charged, and a quarter of the crop is gone.
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().owner_is_human = true;
        order_move(&m, &mut units, id, (11, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(steps[0].field_destroyed, Some(1));
        assert_eq!(counties[1].fields_grain, 3);
        assert_eq!(counties[1].crop[1], 300, "one field of four, so a quarter of the crop");
        assert_eq!(
            steps[0].offence,
            Some(Offence { against: 1, by: 2, amount: FIELD_TRAMPLE_OFFENCE })
        );
    }

    /// **An AI army wrecks fields for free** — the diplomatic penalty is
    /// applied only when the trampling realm is human.
    #[test]
    fn an_ai_army_pays_no_diplomatic_price_for_trampling() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].fields_grain = 2;
        counties[1].crop[1] = 100;

        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().owner_is_human = false;
        order_move(&m, &mut units, id, (11, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(steps[0].field_destroyed, Some(1), "the field still goes");
        assert_eq!(steps[0].offence, None, "and it costs nothing");
    }

    #[test]
    fn a_trampled_field_becomes_bare_ground_and_costs_three_afterwards() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].fields_grain = 1;
        counties[1].crop[1] = 60;
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        order_move(&m, &mut units, id, (11, 10), Routing::Direct);
        march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(m.terrain_at(11, 10), 0);
        assert_eq!(m.cost_map().at(11, 10), 3, "bare farmland is ordinary ground");
    }

    /// A pasture takes its share off the herd instead of off the crop.
    #[test]
    fn trampling_a_pasture_takes_a_share_of_the_herd() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 0x16);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].fields_cattle = 5;
        counties[1].herd = 100;
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        order_move(&m, &mut units, id, (11, 10), Routing::Direct);
        march(&mut m, &mut counties, &realms, &mut units, id);
        assert_eq!(counties[1].fields_cattle, 4);
        assert_eq!(counties[1].herd, 80, "one pasture of five is a fifth of the herd");
    }

    // --- trampling a resource site -----------------------------------------

    /// The rule `crates/l2-kingdom`'s `disabled_seasons` had no writer for.
    #[test]
    fn marching_over_an_enemy_mine_shuts_it_down_for_three_seasons() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::SETTLEMENT);
        m.set_terrain(11, 10, 1); // an iron site
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].industry[1].efficiency = 80;

        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        units.get_mut(id).unwrap().moving = true;

        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.entry, Entry::Settlement);
        assert_eq!(s.charged, 7);
        assert!(!s.moved, "the move ends at the site, it does not enter");
        assert_eq!(s.site_ruined, Some((1, 1)));
        assert_eq!(counties[1].industry[1].disabled_seasons, 3);
        assert_eq!(counties[1].industry[1].efficiency, 0);
        assert_eq!(m.terrain_at(11, 10), 3, "the ruined state");
        assert_eq!(m.cost_map().at(11, 10), 0, "and a hole in the map afterwards");
    }

    /// All four ladders, and the correspondence with the placer.
    #[test]
    fn each_terrain_group_ruins_its_own_industry_record() {
        for (terrain_byte, ruined, record) in
            [(0u8, 3u8, 1usize), (2, 3, 1), (4, 6, 3), (5, 6, 3), (7, 9, 2), (8, 9, 2), (10, 12, 0), (11, 12, 0)]
        {
            let mut m = open_map();
            m.set_flags(11, 10, flags::SETTLEMENT);
            m.set_terrain(11, 10, terrain_byte);
            let (mut counties, realms) = blank();
            counties[1].owner = 1;
            let mut units = Units::new();
            let id = army_at(&mut units, 2, 10, 10);
            units.get_mut(id).unwrap().path = vec![(11, 10)];
            let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
            assert_eq!(s.site_ruined, Some((1, record)), "terrain {terrain_byte}");
            assert_eq!(m.terrain_at(11, 10), ruined);
        }
    }

    /// You cannot ruin your own county's mine, and an already-ruined site costs
    /// nothing — both of which `docs/armies.md` §2.2's flat "+7" hides.
    #[test]
    fn trampling_is_free_on_your_own_site_and_on_one_already_ruined() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::SETTLEMENT);
        m.set_terrain(11, 10, 1);
        let (mut counties, realms) = blank();
        counties[1].owner = 2;

        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.charged, 0, "your own mine");
        assert_eq!(s.site_ruined, None);
        assert_eq!(counties[1].industry[1].disabled_seasons, 0);

        // Now a foreign site that is already a ruin.
        counties[1].owner = 1;
        m.set_terrain(11, 10, 3);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.charged, 0, "there is nothing left to wreck");
    }

    /// The dwelling-plot row `docs/armies.md` §2.2 records as costing nothing.
    #[test]
    fn burning_a_dwelling_costs_seven_moves_which_the_document_records_as_nothing() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::PLOT);
        m.set_terrain(11, 10, terrain::DWELLING);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        let mut units = Units::new();
        let id = army_at(&mut units, 2, 10, 10);
        units.get_mut(id).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.entry, Entry::Plot);
        assert_eq!(s.charged, 7);
        assert!(!s.moved);
    }

    // --- classification ----------------------------------------------------

    #[test]
    fn the_entry_classification_tests_the_bits_in_the_originals_order() {
        let mut m = open_map();
        let units = Units::new();
        let cases: [(u8, u8, Entry); 6] = [
            (flags::ROAD | flags::FARMLAND, 0, Entry::Road),
            (flags::CASTLE, 0, Entry::Castle),
            (flags::SETTLEMENT, 0, Entry::Settlement),
            (flags::PLOT, 0, Entry::Plot),
            (flags::FARMLAND, 10, Entry::Field),
            (0, 0, Entry::Open),
        ];
        for (f, t, want) in cases {
            m.set_flags(5, 5, f);
            m.set_terrain(5, 5, t);
            assert_eq!(try_enter(&m, &units, 5, 5), want, "flags {f:#04x}");
        }
    }

    /// The county town is a settlement whose bit is masked off, so an army
    /// walks into it as ordinary ground.
    #[test]
    fn the_county_town_is_walked_into_rather_than_trampled() {
        let mut m = open_map();
        m.set_flags(5, 5, flags::SETTLEMENT);
        m.set_terrain(5, 5, terrain::TOWN);
        assert_eq!(try_enter(&m, &Units::new(), 5, 5), Entry::Open);
        assert_eq!(m.cost_map().at(5, 5), 3, "and the cost map agrees");
    }

    #[test]
    fn a_unit_in_the_way_stops_the_move_without_charging_anything() {
        let mut m = open_map();
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let mover = army_at(&mut units, 1, 10, 10);
        let blocker = army_at(&mut units, 2, 11, 10);
        units.get_mut(mover).unwrap().path = vec![(11, 10), (12, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
        assert_eq!(s.entry, Entry::Occupied(blocker));
        assert_eq!(s.charged, 0);
        assert!(!s.moved);
        assert_eq!(units.get(mover).unwrap().tile(), (10, 10));
    }

    /// `Unit_EnterOccupiedTile`'s three guards, each of which turns a blocked
    /// tile back into an ordinary one. See [`pass_through`].
    #[test]
    fn a_merchant_walks_through_whatever_is_standing_in_its_way() {
        for (kind, blocker_kind, through) in [
            (UnitKind::Merchant, UnitKind::Army, true),
            (UnitKind::Transport, UnitKind::Army, true),
            (UnitKind::Army, UnitKind::Merchant, true),
            (UnitKind::PeasantMob, UnitKind::Merchant, true),
            (UnitKind::Army, UnitKind::Army, false),
            (UnitKind::Army, UnitKind::PeasantMob, false),
        ] {
            let mut m = open_map();
            let (mut counties, realms) = blank();
            let mut units = Units::new();
            let mover = units.spawn(Unit::new(kind, 1, 10, 10)).unwrap();
            units.spawn(Unit::new(blocker_kind, 2, 11, 10)).unwrap();
            units.get_mut(mover).unwrap().path = vec![(11, 10), (12, 10)];
            let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
            assert_eq!(
                s.moved, through,
                "{kind:?} meeting {blocker_kind:?}: entry was {:?}",
                s.entry
            );
            if through {
                // Code 1: the ordinary open-ground charge, and none of the
                // bits the tile might otherwise have carried.
                assert_eq!(s.entry, Entry::Open);
                assert_eq!(s.charged, crate::tables::STEP_COST_OPEN);
                assert_eq!(units.get(mover).unwrap().tile(), (11, 10));
            }
        }
    }

    /// Code **3** rather than 1 when the shared tile is a road, which is the
    /// one bit `Unit_EnterOccupiedTile` looks at before it decides.
    #[test]
    fn passing_through_on_a_road_costs_a_road_step() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::ROAD);
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let mover = units.spawn(Unit::new(UnitKind::Merchant, 1, 10, 10)).unwrap();
        units.spawn(Unit::new(UnitKind::Merchant, 6, 11, 10)).unwrap();
        units.get_mut(mover).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
        assert_eq!(s.entry, Entry::Road);
        assert_eq!(s.charged, crate::tables::STEP_COST_ROAD);
        assert!(units.get(mover).unwrap().on_road);
    }

    /// **Occupancy is tested first**, so a merchant standing on a castle tile
    /// hides it: the mover walks on at open-ground cost and no capture is
    /// reported. A consequence of the order in `Unit_TryEnterTile` rather than
    /// a rule anybody wrote, and the kind of thing that only shows up once
    /// units exist.
    #[test]
    fn a_unit_on_a_castle_tile_hides_it_from_a_merchant() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::CASTLE);
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let mover = units.spawn(Unit::new(UnitKind::Merchant, 1, 10, 10)).unwrap();
        units.spawn(Unit::new(UnitKind::Army, 2, 11, 10)).unwrap();
        units.get_mut(mover).unwrap().path = vec![(11, 10)];
        let s = step(&mut m, &mut counties, &realms, &mut units, mover).unwrap();
        assert_eq!(s.entry, Entry::Open);
        assert_eq!(s.reached_castle, None);
    }

    #[test]
    fn crossing_a_border_is_reported_once_and_updates_the_unit() {
        let mut m = open_map();
        for y in 0..MAP_DIM as u8 {
            for x in 12..MAP_DIM as u8 {
                m.set_county(x, y, 2);
            }
        }
        let (mut counties, realms) = blank();
        let mut units = Units::new();
        let id = army_at(&mut units, 1, 10, 10);
        order_move(&m, &mut units, id, (14, 10), Routing::Direct);
        let steps = march(&mut m, &mut counties, &realms, &mut units, id);
        let crossings: Vec<_> = steps.iter().filter_map(|s| s.entered_county).collect();
        assert_eq!(crossings, vec![2], "one crossing, at x = 12");
        assert_eq!(units.get(id).unwrap().county, 2);
    }

    #[test]
    fn a_merchant_pays_no_field_surcharge_and_wrecks_nothing() {
        let mut m = open_map();
        m.set_flags(11, 10, flags::FARMLAND);
        m.set_terrain(11, 10, 10);
        let (mut counties, realms) = blank();
        counties[1].owner = 1;
        counties[1].fields_grain = 4;
        counties[1].crop[1] = 400;

        let mut units = Units::new();
        let mut trader = Unit::new(UnitKind::Merchant, 2, 10, 10);
        trader.county = 1;
        let id = units.spawn(trader).unwrap();
        units.get_mut(id).unwrap().path = vec![(11, 10)];

        let s = step(&mut m, &mut counties, &realms, &mut units, id).unwrap();
        assert_eq!(s.charged, 3, "the general step only");
        assert_eq!(counties[1].fields_grain, 4);
        assert_eq!(counties[1].crop[1], 400);
    }
}
