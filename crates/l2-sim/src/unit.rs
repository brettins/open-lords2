//! Battle **units** — what the AI thinks with.
//!
//! The original has two levels: *figures* are drawn and fight ([`crate::figure`]),
//! *units* are what an order is given to. `docs/battle.md` §1 lists the unit
//! record; `docs/battle-ai.md` §0 and §3.2 establish what the AI reads out of
//! it. This module is that record and the two per-frame passes that maintain
//! it: `BattleUnits_RebuildFromFigures` (`0x00488DFE`) and `BattleUnit_Recentre`
//! (`0x004891AD`).
//!
//! # Where positions live
//!
//! Nowhere in this crate, deliberately. `l2-sim` owns rules, not coordinates —
//! [`crate::figure::Figure`] has men, hits and a recovery counter and no `x`.
//! Every routine here that needs to know where somebody stands takes a
//! `positions` slice indexed by figure, supplied by whatever *does* own the
//! battlefield. That is the same seam [`crate::TroopTable`] uses for the combat
//! constants: plain data in, no loader, no knowledge of the caller.
//!
//! # Field offsets
//!
//! Every field carries the original's offset into the 0x34-byte unit record at
//! `0x00566520`, because those offsets are how a claim here is checked against
//! the binary or against a live process (`docs/battle.md` §9).
//!
//! # Determinism
//!
//! Integer arithmetic, `Vec` walked by index, no hashing, no clock
//! (`docs/netcode.md`).

use crate::figure::{Figure, Side, State};

/// `g_battleUnits` holds 81 slots and the sweeps run `1 ..= 80`; slot 0 is
/// never used, and `0` is therefore also the "no such unit" value that
/// [`BattleUnit::last_attacker`] and `Enemy_NearestUnit` return. **[D]**
pub const MAX_UNITS: usize = 80;

/// One unit record. **[D]** from `0x00566520`, stride `0x34`.
///
/// Only the fields the order handlers, the rebuild pass and the reform
/// countdown actually touch are modelled. The rest of the 52 bytes is drawing
/// and player-order state that no AI decision reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleUnit {
    /// `+0x00` owner. **Zero means the slot is free**, which is why every
    /// handler's "is my attacker still alive" test is `units[a].owner != 0`,
    /// and why `Enemy_NearestUnit` tells friend from foe by comparing this
    /// byte rather than [`side`](Self::side).
    pub owner: u8,
    /// `+0x01` this unit is controlled by a human, so no handler runs for it.
    pub human: bool,
    /// `+0x02` live figures. Rebuilt from the figure array every frame.
    pub figures: u8,
    /// `+0x03` side, 0 or 4. Picks which half of every position table applies.
    pub side: Side,
    /// `+0x04`/`+0x06` lowest and highest figure index belonging to this unit.
    /// The original scans that range rather than the whole array.
    pub first: u16,
    pub last: u16,
    /// `+0x08` dispatch category, 0…10. See [`crate::ai`] §1.2.
    pub category: u8,
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
    /// Decremented only once `hit_memory` has reached zero, so it measures hit
    /// *frequency* rather than a total.
    pub times_hit: u8,
    /// `+0x13` **[I]** read by `BattleUnit_NeedsReform` and by nothing else
    /// this crate implements. Its meaning was not established; the reform
    /// exemption for small human units is gated on it being clear.
    pub reform_gate: bool,
    /// `+0x14` the debug panel's `re targ`, counted down from 500. At zero the
    /// unit **reforms its own figures** — it does not pick a target.
    /// `docs/battle-ai.md` §5 corrects `battle.md` on exactly this.
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
    /// `+0x2A` halted, set by `Order_ChargeNearest`. It switches the
    /// every-500-frame reform off, which is why *a charged unit stops being a
    /// formation*.
    pub halted: bool,
    /// `+0x2B` withdrawals taken. `UnitOrder_FieldFoot` allows one,
    /// `UnitOrder_FieldMelee` two.
    pub withdrawals: u8,
}

impl BattleUnit {
    /// An empty slot. `owner == 0` is the original's "free".
    pub const EMPTY: BattleUnit = BattleUnit {
        owner: 0,
        human: false,
        figures: 0,
        side: 0,
        first: 0,
        last: 0,
        category: 0,
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
        halted: false,
        withdrawals: 0,
    };

    pub fn is_live(&self) -> bool {
        self.owner != 0
    }
}

/// `+0x14` is reset to this whenever it reaches zero. **[D]**
pub const REFORM_INTERVAL: i16 = 500;

/// The countdown `+0x0C` is set to when a figure of the unit is hit. **[D]**
///
/// Fifty *frames*, against a think interval of two hundred.
pub const HIT_MEMORY: u8 = 50;

/// The order lock `BattleUnit_Order` sets when it re-orders a unit that is
/// already in melee, buying it that many passes of immunity from
/// `BattleUnit_JoinMelee`. **[D]** `docs/battle-ai.md` §4.2.
pub const ORDER_LOCK: u8 = 64;

/// Which dispatch category a troop type is given by `BattleUnit_Create`
/// (`0x00480662`), read from the eleven-way ladder at `0x00480743`. **[D]**
///
/// `docs/battle-ai.md` §1.2. Categories 9 and 10 are *not* here: they are
/// assigned to the first two missile units a siege **defender** raises, by two
/// one-shot latches, and so depend on raise order rather than on troop type.
pub const CATEGORY_OF_TROOP: [u8; 11] = [2, 1, 3, 3, 2, 1, 4, 5, 6, 7, 8];

/// The unit array, indexed the original's way.
///
/// Index 0 exists and is permanently empty so that a stored unit index of `0`
/// keeps meaning "nobody" without a sentinel of our own.
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

    /// Claim the lowest free slot, as `BattleUnit_Create` does. Returns the
    /// unit index, or `None` once all eighty are taken — the original has no
    /// growth path either.
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

    /// Indices of the live slots, low to high — the order every sweep uses.
    pub fn live(&self) -> impl Iterator<Item = usize> + '_ {
        (1..=MAX_UNITS).filter(move |&i| self.slots[i].is_live())
    }

    pub fn get(&self, i: usize) -> &BattleUnit {
        &self.slots[i]
    }

    pub fn get_mut(&mut self, i: usize) -> &mut BattleUnit {
        &mut self.slots[i]
    }

    /// The liveness test every handler makes before acting on a remembered
    /// attacker: an out-of-range index and a wiped-out unit answer the same.
    pub fn owner_of(&self, i: usize) -> u8 {
        self.slots.get(i).map_or(0, |u| u.owner)
    }

    /// `BattleUnits_RebuildFromFigures` (`0x00488DFE`), once per frame.
    ///
    /// Two passes, in the original's order, because the first clears what the
    /// second fills:
    ///
    /// 1. every live unit ages its timers, and `owner`, `figures` and `first`
    ///    are **zeroed** — a unit whose last figure died this frame therefore
    ///    goes free here and stops being a target for everyone else;
    /// 2. every live figure re-establishes its unit's owner, count and index
    ///    range, and a figure that raised its was-hit flag hands its unit a
    ///    fresh 50-frame grudge naming the *unit* of whoever hit it.
    ///
    /// The hit flag is consumed here, which is why the grudge is per frame and
    /// not per blow.
    pub fn rebuild_from_figures(&mut self, figures: &mut [Figure]) {
        for i in 1..=MAX_UNITS {
            let u = &mut self.slots[i];
            if !u.is_live() {
                continue;
            }
            u.figures = 0;
            u.owner = 0;
            // times_hit ages only once the grudge itself has expired, so it is
            // a rate rather than a total.
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
                // The original tests `first == 0` because its figure array is
                // one-based, so index 0 can never be a real member. Ours is
                // zero-based, so the equivalent test is "no member yet".
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
    ///
    /// `positions` is indexed by figure. A unit with no figures keeps the
    /// position it had, exactly as the original's `figures != 0` guard does.
    ///
    /// The tail of the original is easy to miss and load-bearing: a unit whose
    /// destination is still `(0, 0)` — i.e. one that has never been ordered —
    /// has it seeded from its own position, so an un-ordered unit stands still
    /// instead of walking to the map corner.
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

/// Chebyshev distance, the metric `Enemy_NearestUnit` uses. **[D]**
pub fn chebyshev(ax: i16, ay: i16, bx: i16, by: i16) -> i32 {
    let dx = (ax as i32 - bx as i32).abs();
    let dy = (ay as i32 - by as i32).abs();
    dx.max(dy)
}

/// `PctOf` (`0x00404DC1`): `a * 100 / b`, and **0 when `b` is 0**. **[D]**
///
/// The zero case is not a guard we added. It is why a side with no living
/// enemy reads a strength advantage of `-100` rather than dividing by zero.
pub fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a * 100 / b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};
    use crate::troop::Troop;

    fn one_unit(troop: Troop, men: u16, count: usize) -> (Units, Vec<Figure>, Vec<(u8, u8)>) {
        let mut units = Units::new();
        let cat = CATEGORY_OF_TROOP[troop.index()];
        let u = units.create(1, false, SIDE_A, cat).unwrap();
        let mut figs = Vec::new();
        let mut pos = Vec::new();
        for i in 0..count {
            let mut f = Figure::new(troop, SIDE_A, men);
            f.unit = u as u16;
            f.owner = 1;
            figs.push(f);
            pos.push((10 + i as u8, 20));
        }
        (units, figs, pos)
    }

    #[test]
    fn slot_zero_is_never_handed_out_so_zero_can_mean_nobody() {
        let mut units = Units::new();
        for _ in 0..MAX_UNITS {
            let i = units.create(1, false, SIDE_A, 2).unwrap();
            assert!((1..=MAX_UNITS).contains(&i));
        }
        assert_eq!(units.create(1, false, SIDE_A, 2), None, "the array does not grow");
        assert_eq!(units.owner_of(0), 0);
    }

    #[test]
    fn a_unit_stands_at_the_centre_of_its_figures_bounding_box() {
        let (mut units, mut figs, mut pos) = one_unit(Troop::Swordsmen, 4, 3);
        // Deliberately lopsided: the centroid and the box centre differ.
        pos[0] = (10, 10);
        pos[1] = (11, 10);
        pos[2] = (20, 30);
        units.rebuild_from_figures(&mut figs);
        units.recentre(1, &figs, &pos);
        assert_eq!((units.get(1).x, units.get(1).y), (15, 20), "box centre, not centroid");
    }

    #[test]
    fn an_unordered_unit_takes_its_own_position_as_its_destination() {
        let (mut units, mut figs, pos) = one_unit(Troop::Archers, 4, 2);
        units.rebuild_from_figures(&mut figs);
        assert_eq!((units.get(1).target_x, units.get(1).target_y), (0, 0));
        units.recentre(1, &figs, &pos);
        let u = units.get(1);
        assert_eq!((u.target_x, u.target_y), (u.x, u.y), "seeded from the position");
        // And it is seeded once: moving the unit does not drag the destination.
        let mut pos2 = pos.clone();
        pos2[0] = (40, 40);
        pos2[1] = (41, 40);
        units.recentre(1, &figs, &pos2);
        assert_eq!((units.get(1).target_x, units.get(1).target_y), (10, 20));
    }

    #[test]
    fn the_rebuild_frees_a_unit_whose_last_figure_died() {
        let (mut units, mut figs, _) = one_unit(Troop::Peasants, 1, 2);
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.get(1).figures, 2);
        assert_eq!(units.owner_of(1), 1);
        for f in &mut figs {
            f.take_hits(10_000);
        }
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.owner_of(1), 0, "a wiped-out unit reads as a free slot");
        assert_eq!(units.get(1).figures, 0);
    }

    /// The grudge is fifty **frames**, and the think interval is two hundred.
    /// Rounding it up to "until the next decision" is the mistake
    /// `docs/battle-ai.md` §3.2 warns about, so pin the real number.
    #[test]
    fn the_memory_of_an_attacker_expires_in_fifty_frames() {
        let mut units = Units::new();
        let victim = units.create(1, false, SIDE_A, 3).unwrap();
        let attacker = units.create(2, true, SIDE_B, 3).unwrap();
        let mut figs = vec![
            Figure::new(Troop::Swordsmen, SIDE_A, 4),
            Figure::new(Troop::Swordsmen, SIDE_B, 4),
        ];
        figs[0].unit = victim as u16;
        figs[0].owner = 1;
        figs[1].unit = attacker as u16;
        figs[1].owner = 2;

        figs[0].was_hit = true;
        figs[0].hit_by = Some(1);
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.get(victim).last_attacker, attacker as u16);
        assert_eq!(units.get(victim).hit_memory, HIT_MEMORY);
        assert_eq!(units.get(victim).times_hit, 1);
        assert!(!figs[0].was_hit, "the flag is consumed by the pass that reads it");

        for _ in 0..HIT_MEMORY {
            units.rebuild_from_figures(&mut figs);
        }
        assert_eq!(units.get(victim).hit_memory, 0, "fifty frames, not two hundred");
        // The attacker index survives its own expiry; the handlers gate on the
        // countdown, not on the index.
        assert_eq!(units.get(victim).last_attacker, attacker as u16);
        assert_eq!(units.get(victim).times_hit, 1, "and it only ages once the grudge has");
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.get(victim).times_hit, 0);
    }

    #[test]
    fn a_figure_in_melee_marks_its_whole_unit() {
        let (mut units, mut figs, _) = one_unit(Troop::Macemen, 4, 3);
        units.rebuild_from_figures(&mut figs);
        assert!(!units.get(1).in_melee);
        figs[2].state = State::Melee;
        units.rebuild_from_figures(&mut figs);
        assert!(units.get(1).in_melee);
        figs[2].state = State::Idle;
        units.rebuild_from_figures(&mut figs);
        assert!(!units.get(1).in_melee, "and it is cleared, not latched");
    }

    #[test]
    fn pct_of_answers_zero_rather_than_dividing_by_zero() {
        assert_eq!(pct_of(500, 250), 200);
        assert_eq!(pct_of(0, 250), 0);
        assert_eq!(pct_of(500, 0), 0, "an army with no enemy left");
    }

    #[test]
    fn the_category_ladder_matches_the_documented_eleven_way_assignment() {
        assert_eq!(CATEGORY_OF_TROOP[Troop::Peasants.index()], 2);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Crossbowmen.index()], 1);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Archers.index()], 1);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Macemen.index()], 3);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Swordsmen.index()], 3);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Pikemen.index()], 2);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Knights.index()], 4);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Catapults.index()], 5);
        assert_eq!(CATEGORY_OF_TROOP[Troop::SiegeTowers.index()], 6);
        assert_eq!(CATEGORY_OF_TROOP[Troop::BatteringRams.index()], 7);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Oil.index()], 8);
    }
}
