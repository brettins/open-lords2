//! A battle figure: one drawn man standing for several real soldiers.
//!
//! The original has two levels — *units* are what the player orders, *figures*
//! are what is drawn and what fights. Combat happens entirely between figures.
//! See `docs/battle.md` §1 and §2.

use crate::troop::{Troop, TroopStats};

/// Which figure of a melee pair is currently swinging. The roles swap when the
/// attacker's exchange counter runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Attacking,
    Defending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Standing, looking for something to fight.
    Idle,
    /// Locked in a melee duel with `opponent`.
    Melee,
    /// All its men are gone.
    Dead,
}

/// Sides are numbered 0 and 4 in the original, not 0 and 1 — side 0 deploys at
/// the `0x04` terrain marker and side 4 at `0x0F`. Kept as the original's
/// numbering so state read out of a live game compares directly.
pub type Side = u8;
pub const SIDE_A: Side = 0;
pub const SIDE_B: Side = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Figure {
    pub troop: Troop,
    pub side: Side,
    /// Real soldiers this drawn figure represents. At zero the figure dies.
    pub men: u16,
    /// Accumulated damage. At `hits_per_casualty` one man dies and the
    /// remainder carries over — damage is never wasted.
    pub hits: u16,
    /// Ticks until this figure can be struck again. Counts down; while positive
    /// the figure takes no melee damage. This is the game's only melee defence.
    pub recovery_counter: i32,
    pub state: State,
    pub role: Role,
    /// Index of the figure this one is duelling.
    pub opponent: Option<usize>,
    /// Blows left before the roles swap.
    pub exchange: i32,
    /// The heavy blow lands once. In the original this flag is set and, in the
    /// paths that were traced, never cleared — which would mean one heavy blow
    /// per figure per battle. `docs/battle.md` flags that as unresolved, so it
    /// is reproduced faithfully and marked here rather than quietly "fixed".
    pub blow_used: bool,
    /// Owner is a human player. Only observable effect found is that human-owned
    /// oil gets less armour, which is real in the original but unexplained.
    pub owner_is_human: bool,
}

impl Figure {
    pub fn new(troop: Troop, side: Side, men: u16) -> Self {
        let stats = troop.stats();
        Figure {
            troop,
            side,
            men,
            hits: 0,
            recovery_counter: 0,
            state: State::Idle,
            role: Role::Defending,
            opponent: None,
            exchange: stats.exchange as i32,
            blow_used: false,
            owner_is_human: false,
        }
    }

    pub fn stats(&self) -> TroopStats {
        self.troop.stats()
    }

    pub fn is_alive(&self) -> bool {
        self.state != State::Dead && self.men > 0
    }

    /// Missile defence. The one place `armour` is used, and the one place the
    /// human-owner asymmetry shows up.
    pub fn armour(&self) -> u16 {
        match (self.troop, self.owner_is_human) {
            (Troop::Oil, true) => 25,
            _ => self.stats().armour,
        }
    }

    /// Apply damage, converting whole multiples of the kill threshold into
    /// casualties. Returns how many men died.
    pub fn take_hits(&mut self, amount: u16) -> u16 {
        let threshold = self.troop.hits_per_casualty();
        self.hits = self.hits.saturating_add(amount);
        let mut killed = 0;
        while self.hits >= threshold && self.men > 0 {
            self.hits -= threshold;
            self.men -= 1;
            killed += 1;
        }
        if self.men == 0 {
            self.state = State::Dead;
            self.opponent = None;
        }
        killed
    }
}
