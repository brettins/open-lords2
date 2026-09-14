#![allow(unused_imports)]
use super::*;
use super::site::*;
use super::castle::*;
use super::campaign::*;
use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

/// `g_moveCost` (`0x004F4080`) — a 64×64 `i16` grid, **0 meaning impassable**.
///
/// The flood fill's only blocked test is
/// `cost != 0`, so "free to enter" is not representable and never needs to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostMap {
    cost: Vec<i16>,
}

impl CostMap {
    /// A map where every tile costs the same. Only useful in tests; a real one
    /// comes from [`CampaignMap::cost_map`].
    pub fn uniform(cost: i16) -> CostMap {
        CostMap { cost: vec![cost; MAP_TILES] }
    }

    #[inline]
    pub fn at(&self, x: u8, y: u8) -> i16 {
        self.cost[index(x, y)]
    }

    #[inline]
    pub fn at_index(&self, i: usize) -> i16 {
        self.cost[i]
    }

    pub fn set(&mut self, x: u8, y: u8, cost: i16) {
        self.cost[index(x, y)] = cost;
    }

    /// A tile no unit can ever enter.
    #[inline]
    pub fn is_impassable(&self, i: usize) -> bool {
        self.cost[i] == MOVE_COST_IMPASSABLE as i16
    }

    pub fn as_slice(&self) -> &[i16] {
        &self.cost
    }
}

