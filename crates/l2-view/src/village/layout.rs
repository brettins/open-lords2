#![allow(unused_imports)]
use super::*;
use super::icons::*;
use super::art::*;
use crate::sheet::Sheet;
use crate::Canvas;

/// `Job_SlotForCluster` (`0x004517CA`).
pub fn slot_for_cluster(cluster: usize, has_quarry: bool, has_mine: bool) -> usize {
    let slot = CLUSTER_TO_SLOT_BALANCE[cluster.min(CLUSTER_TO_SLOT_BALANCE.len() - 1)];
    if cluster == 0 && !has_quarry && has_mine {
        4
    } else {
        slot
    }
}

/// `FUN_0045183A`: whether clicking a cluster opens its job popup.
pub fn cluster_is_clickable(cluster: usize, has_quarry: bool, has_mine: bool) -> bool {
    cluster != 0 || has_quarry || has_mine
}

pub fn cluster_origin(cluster: usize, top: i32) -> (i32, i32) {
    let (x, y) = ORIGINS[cluster.min(CLUSTER_COUNT - 1)];
    (x + SCENE_X, y + top)
}

pub fn icon_position(cluster: usize, slot: usize, top: i32) -> (i32, i32) {
    let (ox, oy) = cluster_origin(cluster, top);
    let (dx, dy) = ICON_OFFSETS[slot.min(ICONS_PER_CLUSTER - 1)];
    (ox + dx + ICON_DX, oy + dy + ICON_DY)
}

pub fn icon_hit_point(cluster: usize, slot: usize, top: i32) -> (i32, i32) {
    let (x, y) = icon_position(cluster, slot, top);
    (x + 10, y + 9)
}

pub fn cluster_band_box(cluster: usize, top: i32) -> (i32, i32, i32, i32) {
    let (x, y) = ORIGINS[cluster.min(CLUSTER_COUNT - 1)];
    (x + 0x40, y + top - 0x14, x + 0x98, y + top + 0x3C)
}

