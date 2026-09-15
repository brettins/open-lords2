#![allow(unused_imports)]
use super::*;
use super::site::*;
use super::castle::*;
use super::campaign::*;
use crate::tables::{MOVE_COST_BLOCKED, MOVE_COST_IMPASSABLE};

/// `g_moveCost` (`0x004F4080`) — a 64×64 `i16` grid, **0 meaning impassable**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostMap {
    pub(super) cost: Vec<i16>,
}

impl CostMap {
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

    #[inline]
    pub fn is_impassable(&self, i: usize) -> bool {
        self.cost[i] == MOVE_COST_IMPASSABLE as i16
    }

    pub fn as_slice(&self) -> &[i16] {
        &self.cost
    }
}

