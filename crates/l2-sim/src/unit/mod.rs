//! The original has two levels: *figures* are drawn and fight ([`crate::figure`]),
//! *units* are what an order is given to. `docs/battle.md` §1 lists the unit
//! record; `docs/battle-ai.md` §0 and §3.2 establish what the AI reads out of
//! it. This module is that record and the two per-frame passes that maintain
//! it: `BattleUnits_RebuildFromFigures` (`0x00488DFE`) and `BattleUnit_Recentre`
//! (`0x004891AD`).
//!
//! Every field carries the original's offset into the 0x34-byte unit record at
//! `0x00566520`, because those offsets are how a claim here is checked against
//! the binary or against a live process (`docs/battle.md` §9).

mod helpers;
pub use helpers::*;
mod tests_part;
pub use tests_part::*;

use crate::figure::{Figure, Side, State};

/// `g_battleUnits` holds 81 slots and the sweeps run `1..= 80`; slot 0 is
/// never used, and `0` is therefore also the "no such unit" value that
/// [`BattleUnit::last_attacker`] and `Enemy_NearestUnit` return. **[D]**
pub const MAX_UNITS: usize = 80;

/// One unit record. **[D]** from `0x00566520`, stride `0x34`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleUnit {
/// `+0x00` owner. **Zero means the slot is free**, so every
    /// handler's "is my attacker still alive" test is `units[a].owner != 0`,
    /// and why `Enemy_NearestUnit` tells friend from foe by comparing this
/// byte.
    pub owner: u8,
    /// `+0x01` this unit is controlled by a human, so no handler runs for it.
    pub human: bool,
    /// `+0x02` live figures. Rebuilt from the figure array every frame.
    pub figures: u8,
    /// `+0x03` side, 0 or 4. Picks which half of every position table applies.
    pub side: Side,
    /// `+0x04`/`+0x06` lowest and highest figure index belonging to this unit.
    pub first: u16,
    pub last: u16,
    /// `+0x08` dispatch category, 0…10. See [`crate::ai`] §1.2.
    pub category: u8,
    /// `+0x09` **which way the formation rectangle lies** — 0 across, 1 down.
    ///
    /// `docs/battle.md` §1 lists `+0x09` as "unnamed and untraced". It is
    /// written in exactly one place, `BattleUnit_Order`'s `facing` arm, from a
    /// value only a player's keypress can supply, and read in exactly one,
    /// `0x00480F8B`'s `if (unit.field_0x9 == 1)`. So it is the `H` / `V` keys
    /// and nothing else touches it. **[V]** on the two references,
    /// **[I]** on "horizontal / vertical" as the reading of the two keys.
    pub orientation: u8,
    /// `+0x0A` the **unit** whose figure last hit one of ours.
    pub last_attacker: u16,
    /// `+0x0C` frames of memory of that attacker: set to 50 on a hit and
    /// counted down once per rebuild. It expires long before the 200-frame
    /// think interval, so most thinks find it already zero — `docs/battle-ai.md`
    /// §3.2 is explicit that rounding this up to "until the next decision"
    /// changes the behaviour.
    pub hit_memory: u8,
    /// `+0x0D` some figure of this unit is in melee. Thirteen of the seventeen
    /// handlers refuse to run while it is set.
    pub in_melee: bool,
    /// `+0x0E` order lock: 64 when a unit already in melee is re-ordered,
    /// counted down per rebuild.
    pub order_lock: u8,
    /// `+0x0F` firing. Blocks the missile handler's wall-slot rotation and its
    /// retreat.
    pub firing: u8,
    /// `+0x10` withdrawing — set by `Order_StepAwayFromUnit`, cleared by every
    /// other destination-writing order.
    pub withdrawing: bool,
    /// `+0x11` some figure of this unit is filling the moat (state 9).
    pub on_moat: bool,
    /// `+0x12` times hit. A `u8`, and it **wraps** — reproduced, not widened.
    pub times_hit: u8,
    /// `+0x13` **[I]** read by `BattleUnit_NeedsReform` and by nothing else
    /// this crate implements. Its meaning was not established; the reform
    /// exemption for small human units is gated on it being clear.
    pub reform_gate: bool,
    /// `+0x14` the debug panel's `re targ`, counted down from 500. At zero the
    /// unit **reforms its own figures** — it does not pick a target.
    pub reform: i16,
    /// `+0x1A` the debug panel's `orders`, and **not an order**: a
    /// monotonically increasing count of thinks, read as a script program
    /// counter (`orders < 10`, `orders % 5 == 0`). Never written by a player's
    /// click. `docs/battle-ai.md` §1.4.
    pub orders: i16,
    /// `+0x1C` think timer. Incremented every frame the handler runs.
    pub think: i16,
    /// `+0x1E`/`+0x20` the unit's position: the centre of the bounding box of
    /// its live figures, recomputed every frame.
    pub x: i16,
    pub y: i16,
    /// `+0x22`/`+0x24` the destination the handlers write. Almost every action
    /// in the AI's vocabulary is a way of choosing these two numbers.
    pub target_x: i16,
    pub target_y: i16,
    /// `+0x30` **`targetCell`** — the cell the player ordered this unit onto,
    /// as a byte offset `(y * 80 + x) * 8`, or 0. `BattleUnit_Order`
    /// (`0x00479E90`) clears it on every order and writes it only under its
    /// fifth argument, `DAT_0053E874` — *the hovered cell is woodland*. Every
    /// arrow `BattleMan_StateCloseToAttack` (`0x00484BF9`) looses carries it in
    /// `+0x44`, and `Missile_Step` lights that one cell. **The player's fire
    /// arrow.** `docs/battle.md` §17.5.
    pub target_cell: i32,
    /// `+0x2A` halted, set by `Order_ChargeNearest`. It switches the
/// every-500-frame reform off, so *a charged unit stops being a
    /// formation*.
    pub halted: bool,
    /// `+0x2B` withdrawals taken. `UnitOrder_FieldFoot` allows one,
    /// `UnitOrder_FieldMelee` two.
    pub withdrawals: u8,
}

impl BattleUnit {
    pub const EMPTY: BattleUnit = BattleUnit {
        owner: 0,
        human: false,
        figures: 0,
        side: 0,
        first: 0,
        last: 0,
        category: 0,
        orientation: 0,
        last_attacker: 0,
        hit_memory: 0,
        in_melee: false,
        order_lock: 0,
        firing: 0,
        withdrawing: false,
        on_moat: false,
        times_hit: 0,
        reform_gate: false,
        reform: REFORM_INTERVAL,
        orders: 0,
        think: 0,
        x: 0,
        y: 0,
        target_x: 0,
        target_y: 0,
        target_cell: 0,
        halted: false,
        withdrawals: 0,
    };

    pub fn is_live(&self) -> bool {
        self.owner != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Units {
    slots: Vec<BattleUnit>,
}

impl Default for Units {
    fn default() -> Self {
        Units::new()
    }
}

impl Units {
    pub fn new() -> Self {
        Units { slots: vec![BattleUnit::EMPTY; MAX_UNITS + 1] }
    }

    pub fn create(&mut self, owner: u8, human: bool, side: Side, category: u8) -> Option<usize> {
        let free = (1..=MAX_UNITS).find(|&i| !self.slots[i].is_live())?;
        self.slots[free] = BattleUnit {
            owner,
            human,
            side,
            category,
            ..BattleUnit::EMPTY
        };
        Some(free)
    }

    pub fn len(&self) -> usize {
        MAX_UNITS
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn live(&self) -> impl Iterator<Item = usize> + '_ {
        (1..=MAX_UNITS).filter(move |&i| self.slots[i].is_live())
    }

    pub fn get(&self, i: usize) -> &BattleUnit {
        &self.slots[i]
    }

    pub fn get_mut(&mut self, i: usize) -> &mut BattleUnit {
        &mut self.slots[i]
    }

    pub fn owner_of(&self, i: usize) -> u8 {
        self.slots.get(i).map_or(0, |u| u.owner)
    }

    /// `BattleUnits_RebuildFromFigures` (`0x00488DFE`), once per frame.
    pub fn rebuild_from_figures(&mut self, figures: &mut [Figure]) {
        for i in 1..=MAX_UNITS {
            let u = &mut self.slots[i];
            if !u.is_live() {
                continue;
            }
            u.figures = 0;
            u.owner = 0;
            if u.hit_memory == 0 {
                u.times_hit = u.times_hit.saturating_sub(1);
            } else {
                u.hit_memory -= 1;
            }
            u.firing = u.firing.saturating_sub(1);
            u.order_lock = u.order_lock.saturating_sub(1);
            u.in_melee = false;
        }

        for f in 0..figures.len() {
            if !figures[f].is_alive() {
                continue;
            }
            let unit = figures[f].unit as usize;
            if unit == 0 || unit > MAX_UNITS {
                continue;
            }
            {
                let u = &mut self.slots[unit];
                if u.figures == 0 {
                    u.first = f as u16;
                }
                u.last = f as u16;
                u.figures = u.figures.saturating_add(1);
                u.owner = figures[f].owner;
            }
            if figures[f].was_hit {
                figures[f].was_hit = false;
                let by = figures[f].hit_by;
                let attacker_unit = by
                    .and_then(|b| figures.get(b))
                    .map(|a| a.unit)
                    .unwrap_or(0);
                let u = &mut self.slots[unit];
                u.times_hit = u.times_hit.wrapping_add(1);
                u.hit_memory = HIT_MEMORY;
                u.last_attacker = attacker_unit;
            }
            match figures[f].state {
                State::Melee => self.slots[unit].in_melee = true,
                State::FillingMoat => self.slots[unit].on_moat = true,
                _ => {}
            }
        }
    }

    /// `BattleUnit_Recentre` (`0x004891AD`): the unit stands at the centre of
    /// the **bounding box** of its live figures, not at their centroid.
    pub fn recentre(&mut self, unit: usize, figures: &[Figure], positions: &[(u8, u8)]) {
        if self.slots[unit].figures == 0 {
            return;
        }
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (80i32, 0i32, 80i32, 0i32);
        let (first, last) = (self.slots[unit].first as usize, self.slots[unit].last as usize);
        let last = last.min(figures.len().saturating_sub(1));
        for (offset, fig) in figures[first..=last].iter().enumerate() {
            if !fig.is_alive() || fig.unit as usize != unit {
                continue;
            }
            let Some(&(x, y)) = positions.get(first + offset) else { continue };
            let (x, y) = (x as i32, y as i32);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let u = &mut self.slots[unit];
        u.x = (min_x + (max_x - min_x) / 2) as i16;
        u.y = (min_y + (max_y - min_y) / 2) as i16;
        if u.target_x == 0 && u.target_y == 0 {
            u.target_x = u.x;
            u.target_y = u.y;
        }
    }
}

