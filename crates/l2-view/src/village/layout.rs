#![allow(unused_imports)]
use super::*;
use super::icons::*;
use super::art::*;
use crate::sheet::Sheet;
use crate::Canvas;

/// `Job_SlotForCluster` (`0x004517CA`).
///
/// Cluster 0 is the stone quarry unless the county has **no** quarry and **does**
/// have a mine, in which case the same spot is the mine.
pub fn slot_for_cluster(cluster: usize, has_quarry: bool, has_mine: bool) -> usize {
    let slot = CLUSTER_TO_SLOT_BALANCE[cluster.min(CLUSTER_TO_SLOT_BALANCE.len() - 1)];
    if cluster == 0 && !has_quarry && has_mine {
        4
    } else {
        slot
    }
}

/// `FUN_0045183A`: whether clicking a cluster opens its job popup.
///
/// Only cluster 0 can refuse, and only for a county with neither a quarry nor a
/// mine. **The cluster is still drawn** — `Village_DrawPeasants` loops 0 … 7
/// with no test at all —.
/// mine still shows the slot; it simply has nobody in it.
pub fn cluster_is_clickable(cluster: usize, has_quarry: bool, has_mine: bool) -> bool {
    cluster != 0 || has_quarry || has_mine
}

/// Where cluster `c`'s icon grid starts, for a scene whose top is `top`.
pub fn cluster_origin(cluster: usize, top: i32) -> (i32, i32) {
    let (x, y) = ORIGINS[cluster.min(CLUSTER_COUNT - 1)];
    (x + SCENE_X, y + top)
}

/// Where one icon of one cluster is drawn.
pub fn icon_position(cluster: usize, slot: usize, top: i32) -> (i32, i32) {
    let (ox, oy) = cluster_origin(cluster, top);
    let (dx, dy) = ICON_OFFSETS[slot.min(ICONS_PER_CLUSTER - 1)];
    (ox + dx + ICON_DX, oy + dy + ICON_DY)
}

/// The point `Village_BoxSelect` tests an icon at — the icon's middle, ten
/// pixels right and nine down from where it is drawn.
pub fn icon_hit_point(cluster: usize, slot: usize, top: i32) -> (i32, i32) {
    let (x, y) = icon_position(cluster, slot, top);
    (x + 10, y + 9)
}

/// `Village_BoxSelect`'s per-cluster bounding box: a band reaches into a
/// cluster if it overlaps `origin.x + 0x40 … + 0x98` by `origin.y + top - 0x14
/// … + 0x3C`.
pub fn cluster_band_box(cluster: usize, top: i32) -> (i32, i32, i32, i32) {
    let (x, y) = ORIGINS[cluster.min(CLUSTER_COUNT - 1)];
    (x + 0x40, y + top - 0x14, x + 0x98, y + top + 0x3C)
}

