#![allow(unused_imports)]
use super::*;
use super::stepper::*;
use super::tests_part::*;
use crate::county::{County, MAX_COUNTIES};
use crate::map::{coords, index, terrain, CampaignMap, CostMap, MAP_DIM, MAP_TILES};
use crate::realm::{Realm, MAX_REALMS};
use crate::unit::{UnitKind, Units, MAX_PATH};

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
/// Four things worth reading twice
/// reasonable reimplementation goes wrong:
///
/// * **The cost charged is the cost of the tile being *entered*.** The start
///   tile's own cost is never paid.
/// * **A cell is re-relaxed and re-queued when a cheaper route arrives.** This
///   is not an optimisation — a FIFO queue over weights of 1, 3, 6 and 100 does
///   not produce distances in sorted order, and without relaxation the field
///   would be wrong.
/// * **Cost 0 is impassable.** An
///   impassable cell keeps `dist == 0` forever, which is indistinguishable
///   from unreached — deliberately, since neither can be walked to.
/// * **A road tile expands orthogonally only.** `if (cost[cur] != 1)` gates the
/// four diagonals
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
/// hitting N, E, S, W only
///   verbatim only if that found nothing. `[D]`, and it confirms
///   `docs/armies.md` §2.3.
/// * the running best is seeded with `dist[cur]` itself and the test is a
/// strict `<`
///   lowest direction index wins a tie**. That is the whole tie-break rule.
/// * the candidate's *cost* is never consulted — only the distance field, and
///   only the road test on the tile being left.
///
/// **An unreachable destination comes back as an empty path, not as a
/// failure.** `dist[dest] == 0` makes the first `< 2` test true immediately, so
/// the original returns success with `pathLen = 0`, the caller copies it,
/// sets `moveState = 2`
/// accepted and nothing happens, which is observable and therefore not ours to
/// improve.
///
/// The result is capped at [`MAX_PATH`]. The original does *not* cap it — the
/// AI checks the length before copying and `Unit_OrderMove` does not
/// over 150 steps writes past `g_pathBuf`'s 300-byte slot and stores a length
/// the unit's array cannot hold. That one is a buffer overrun
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

