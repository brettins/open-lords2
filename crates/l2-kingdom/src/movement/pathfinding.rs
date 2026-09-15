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

    let mut queue = vec![0u16; QUEUE_CAP];
    queue[0] = si as u16;
    let (mut head, mut tail) = (0usize, 1usize);

    while head != tail {
        let cur = queue[head] as usize;
        head = (head + 1) % QUEUE_CAP;
        let d = dist[cur];
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
/// * on a **road** tile (raw cost 1) the direction index steps by **2**,
/// hitting N, E, S, W only
///   verbatim only if that found nothing. `[D]`, and it confirms
///   `docs/armies.md` §2.3.
pub fn extract_path(cost: &CostMap, field: &DistanceField, dest: (u8, u8)) -> Option<Vec<(u8, u8)>> {
    let mut path = Vec::new();
    let (mut x, mut y) = dest;
    let mut here = field.raw(x, y) as i32;

    while here >= START_DISTANCE as i32 + 1 && path.len() < MAX_PATH {
        path.push((x, y));
        let stride = if cost.at(x, y) == crate::tables::STEP_COST_ROAD as i16 { 2 } else { 1 };
        let mut best: Option<(usize, i32, u8, u8)> = None;
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
            None => return None,
        }
    }
    path.reverse();
    Some(path)
}

