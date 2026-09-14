//! Missile combat: the damage a hit does, and **the arrow that carries it**.
//!
//! Reimplemented from `docs/battle.md` §6.2 and §14.7. Three things distinguish
//! the damage from melee, and all three are easy to lose by accident:
//!
//! * **Armour is a flat subtraction, and applies here only.** It is never read
//!   during a melee exchange.
//! * **High ground protects the target, and only against missiles.** Nothing in
//!   melee reads elevation.
//! * **A missile kill discards the leftover damage; a melee kill carries it.**
//!
//! Integer arithmetic throughout, in the original's order of operations —
//! division before the armour subtraction, clamping after. Reordering changes
//! results, so the order is part of the specification.
//!
//! # The shot is an object, not an outcome
//!
//! This module used to stop at [`resolve_power`]: a hit resolved and nothing
//! flew, so sixty archers fought as sixty men carrying bows. The question that
//! had to be settled before any of this could be written was whether the
//! original resolves a shot at launch and merely animates it, or whether the
//! arrow genuinely traverses. **It genuinely traverses**
//! one line of `Missile_Step` (`0x00492C8B`):
//!
//! ```c
//! g_otherBattleMan = g_battlefield[missile.cellOffset].figure;
//! ```
//!
//! The victim is read out of the **cell the missile has just entered**, fresh,
//! every sub-step. Nothing anywhere on the 0x4C-byte record remembers who the
//! shot was aimed at — `+0x06` is the *shooter* — so an arrow cannot check
//! whether it hit the right man, and does not. Four consequences, all of them
//! visible in play and all of them reproduced here:
//!
//! * **A body in the way takes the arrow.** Anyone who has walked into the
//!   flight path is hit instead, which is what makes a screening line work.
//! * **A miss keeps flying.** When the Bresenham line is exhausted the missile
//!   switches to [`Missile::step_octant`] and coasts on in its launch
//!   direction until it runs out of range, hits somebody else or leaves the
//!   map. Overshoot can kill a second rank.
//! * **A target that dies or walks away is not tracked.** The impact point is
//!   frozen at launch; the arrow arrives at an empty cell and carries on.
//! * **Friendly fire cannot happen**, because the test is on the *owner* byte
//! — and a friendly body does not stop the arrow either.
//!
//! # The timing is one number wearing two hats
//!
//! A missile advances [`SUB_STEPS`] sub-steps a tick and one sub-step is
//! [`SUB_CELL`]⁻¹ of a cell, The range in
//! `g_missileStats` is stored in **eighths of a cell**, so the same number is
//! both the distance and the tick budget, and `range >> 3` is the range in
//! cells with no conversion anywhere. That is why [`MissileStats::range_ticks`]
//! and [`MissileStats::range`] are the same fact twice.
//!
//! # The three classes that are not weapons
//!
//! Missile classes **4** (catapult debris), **5** (a burning cell) and **7**
//! (the boiling-oil stream) share the same 100-slot array in the original, and
//! so they share it here: a battle full of fire has fewer arrows to loose.
//! Debris is a spent shot that counts down. Classes 5 and 7 are
//! [`crate::fire`]'s — a fire is a record that sits on one cell and puts the
//! cell's surface back when it goes out, and a stream of oil is a record that
//! flies and sets a cross of cells burning under itself every tick.
//! `docs/battle.md` §0 once called class 7 *"falling men"*; it is boiling oil,
//! and the only spawner in the binary is `FUN_0047A814`.

mod types;
pub use types::*;

mod combat;
pub use combat::*;

use crate::facing::facing_from_delta;
use crate::figure::Figure;
use crate::troop::Troop;

/// Sub-cell units a cell is divided into. A missile's position is in these.
/// **[V]** `Missile_Spawn` writes `x << 5` and `Missile_Step` recovers
/// `(x + 8) / 32`.
pub const SUB_CELL: i16 = 32;

/// Sub-steps a missile takes per tick — `g_missileStats[class][2]`, **4 for
/// every weapon class**. One sub-step is one unit of [`SUB_CELL`],
/// an eighth of a cell.
pub const SUB_STEPS: i8 = 4;

/// The array bound: `Missile_UpdateAll` runs `for (i = 1; i < 0x65; i++)`, and
/// slot 0 is the "none" value. **A hundred and first arrow is silently
/// dropped**
pub const MAX_MISSILES: usize = 100;

/// **A missile is born a whole cell out from its shooter.**
/// `BattleMan_FireMissile` runs eight `Missile_Step`s on the spot before the
/// missile is ever linked to a cell or drawn — eight ticks, thirty-two
/// sub-steps, one cell — and those eight come out of the range budget.
/// shot can already have hit something before anybody sees it.
pub const LAUNCH_STEPS: u32 = 8;

/// Ticks a **blocked** missile keeps trying before it is given up on. Past this
/// the shooter is marked engaged and the arrow is discarded.
pub const BLOCKED_LIMIT: u8 = 0x20;

/// What a hit sets the missile's countdown to. `Missile_UpdateAll` decrements
/// it, so an arrow lives exactly one more tick after impact — which is what
/// makes **one hit per missile** structural
pub const HIT_TTL: i16 = 2;

/// What a catapult shot striking a wall sets its countdown to, as it becomes
/// class 4 debris.
pub const DEBRIS_TTL: i16 = 0x78;

/// **The cell byte a shot may not pass.** `Missile_Step`: `cell.terrain++; if
/// (0xF < cell.terrain) Wall_Collapse(cell);` — and because
/// `Battlefield_BuildCastle` seeds a castle's non-moat cells at 1, a wall takes
/// **fifteen** shots, not sixteen. **[V]**
///
/// The same byte drives the picture: the renderer's second pass draws slot-1
/// frame `cell[+0] + 0x8B`, so 1 is the untouched wall and 15 the last rubble
/// frame before it comes down.
pub const WALL_DAMAGE_MAX: u8 = 0x0F;

/// Catapult hits one wall cell absorbs before it collapses, counted from a
/// zeroed byte. Kept for [`crate::siege`]'s callers; the rule is
/// [`WALL_DAMAGE_MAX`].
pub const WALL_HITS_PER_COLLAPSE: u8 = 16;

/// **A wall this high cannot be shot down.** `Missile_Step`'s class-3 arm
/// counts a hit only while `cell.elevation < 4`; at 4 and above the shot is
/// debris with nothing counted and `Sound_PlaySlot(0x10)`, `catmiss.wav`.
/// `[V]`.
pub const WALL_TOO_HIGH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    Bow = 1,
    Crossbow = 2,
    Catapult = 3,
}

/// The missile classes that are not weapons — the other three users of the same
/// array. Class 6 does not exist: nothing in the binary writes it.
pub const CLASS_DEBRIS: u8 = 4;
/// **Half a cell up and left**, in the same thirty-seconds the position is in:
/// `Missile_Step`'s wall arm ends `m[+0x0A] -= 0x10; m[+0x0C] -= 0x10;` as it
/// turns the shot into debris. **[V]**
pub const DEBRIS_NUDGE: i16 = 0x10;
/// A burning cell — `FUN_00485675` and `FUN_00485861` write it. See
/// [`crate::fire`].
pub const CLASS_FIRE: u8 = 5;
/// A stream of boiling oil — `FUN_0047A814` is its only writer.
pub const CLASS_OIL: u8 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissileStats {
    /// Range in cells. Stored in the original as sub-cell units of 8.
    pub range: u16,
    /// Ticks between shots.
    pub reload: u16,
    /// Base damage before band scaling, elevation and armour.
    pub damage: u16,
}

impl MissileStats {
    /// **The same number as [`MissileStats::range`], in the units the original
    /// stores it in** — eighths of a cell — which is also the missile's tick
    /// budget, because a tick is an eighth of a cell. `g_missileStats[class][0]`
    /// is 120, 64 and 160.
    ///
    /// ```
    /// # use l2_sim::WeaponClass;
    /// assert_eq!(WeaponClass::Bow.stats().range_ticks(), 120);
    /// assert_eq!(WeaponClass::Crossbow.stats().range_ticks(), 64);
    /// assert_eq!(WeaponClass::Catapult.stats().range_ticks(), 160);
    /// ```
    pub fn range_ticks(&self) -> i16 {
        (self.range * 8) as i16
    }
}

/// **When a shot's target is chosen.** `BattleMan_FireMissile` acquires ten
/// ticks before the reload expires,
/// last ten ticks of a cycle does not shoot at all.
pub const ACQUIRE_LEAD: u16 = 10;

/// What `BattleMan_FireMissile` resets the reload counter to when
/// `Missile_FindTarget` finds nothing: the figure does **not** wait a full
/// cycle before looking again.
pub const NO_TARGET_RESET: u16 = 4;

impl WeaponClass {
    pub fn stats(self) -> MissileStats {
        match self {
            WeaponClass::Bow => MissileStats { range: 15, reload: 50, damage: 50 },
            WeaponClass::Crossbow => MissileStats { range: 8, reload: 100, damage: 200 },
            WeaponClass::Catapult => MissileStats { range: 20, reload: 100, damage: 200 },
        }
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    /// Whether a missile of this class can hurt a **figure**.
    ///
    /// `Missile_Step`'s hit test is gated on `class < 3`,
    /// passing straight through a crowd does nothing to the men in it at all.
    /// A catapult is a wall-breaker and only a wall-breaker. **[V]**
    pub fn hits_men(self) -> bool {
        (self as u8) < 3
    }

    /// Which troops carry a missile weapon. Everything else never enters the
    /// firing state at all.
    pub fn for_troop(troop: Troop) -> Option<WeaponClass> {
        match troop {
            Troop::Archers => Some(WeaponClass::Bow),
            Troop::Crossbowmen => Some(WeaponClass::Crossbow),
            Troop::Catapults => Some(WeaponClass::Catapult),
            _ => None,
        }
    }
}

/// **The missile array** — a hundred fixed slots and no allocation
/// the original has it.
///
/// Slot 0 is never used; it is the "none" value. Allocation always restarts the
/// scan at 1 and takes the lowest free slot, which is what makes it
/// deterministic enough for lockstep: two peers with the same state allocate the
/// same index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missiles {
    slots: Vec<Missile>,
}

impl Default for Missiles {
    fn default() -> Self {
        Missiles::new()
    }
}

impl Missiles {
    pub fn new() -> Missiles {
        Missiles { slots: vec![Missile::default(); MAX_MISSILES + 1] }
    }

    /// `Missile_Spawn`'s allocator: the lowest free slot, or `None` once all
    /// hundred are in flight. **The original drops the shot silently**, and so
    /// does this.
    pub fn alloc(&mut self) -> Option<usize> {
        (1..=MAX_MISSILES).find(|&i| !self.slots[i].is_live())
    }

    /// Zero a slot — `FUN_0046EB78`, which clears all `0x4C` bytes.
    pub fn free(&mut self, i: usize) {
        self.slots[i] = Missile::default();
    }

    pub fn get(&self, i: usize) -> &Missile {
        &self.slots[i]
    }

    pub fn get_mut(&mut self, i: usize) -> &mut Missile {
        &mut self.slots[i]
    }

    /// How many are in the air. For tests and for a renderer; nothing branches
    /// on it.
    pub fn live(&self) -> usize {
        self.slots.iter().filter(|m| m.is_live()).count()
    }

    /// Every live missile with its slot, ascending — the order
    /// `Missile_UpdateAll` walks and the only order anything may walk them in.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Missile)> {
        self.slots.iter().enumerate().skip(1).filter(|(_, m)| m.is_live())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::SIDE_B;

    /// The manual states this in one sentence: archers have greater range and a
    /// faster rate of fire than crossbowmen, but do less damage per shot. Three
    /// independent agreements from a source nobody here wrote.
    #[test]
    fn bows_out_range_and_out_shoot_crossbows_but_hit_softer() {
        let bow = WeaponClass::Bow.stats();
        let xbow = WeaponClass::Crossbow.stats();
        assert!(bow.range > xbow.range, "greater range");
        assert!(bow.reload < xbow.reload, "faster rate of fire");
        assert!(bow.damage < xbow.damage, "less damage per shot");
    }

    #[test]
    fn only_three_troop_types_carry_a_missile_weapon() {
        assert_eq!(WeaponClass::for_troop(Troop::Archers), Some(WeaponClass::Bow));
        assert_eq!(WeaponClass::for_troop(Troop::Crossbowmen), Some(WeaponClass::Crossbow));
        assert_eq!(WeaponClass::for_troop(Troop::Catapults), Some(WeaponClass::Catapult));
        for t in [Troop::Swordsmen, Troop::Knights, Troop::Pikemen, Troop::Peasants] {
            assert_eq!(WeaponClass::for_troop(t), None);
        }
    }

    /// Armour subtracts flatly, which is the whole reason crossbows are the
    /// anti-armour weapon: an arrow mostly bounces off plate, a bolt does not.
    #[test]
    fn armour_is_a_flat_subtraction_not_a_ratio() {
        let armoured = Figure::new(Troop::Swordsmen, SIDE_B, 4); // armour 35
        let soft = Figure::new(Troop::Peasants, SIDE_B, 4); // armour 0

        let arrow_vs_plate = resolve_power(WeaponClass::Bow, 50, 0, &armoured, 0);
        let arrow_vs_soft = resolve_power(WeaponClass::Bow, 50, 0, &soft, 0);
        let bolt_vs_plate = resolve_power(WeaponClass::Crossbow, 200, 0, &armoured, 0);

        assert_eq!(arrow_vs_plate, 15, "50 - 35");
        assert_eq!(arrow_vs_soft, 50, "nothing subtracted");
        assert_eq!(bolt_vs_plate, 165, "200 - 35");
        // An arrow needs about seven hits to kill one swordsman; a bolt under two.
        assert!(100 / arrow_vs_plate >= 6);
        assert!(100 / bolt_vs_plate < 1);
    }

    #[test]
    fn a_hit_always_lands_for_at_least_two() {
        // Oil has the heaviest armour in the game, far above an arrow's damage.
        let mut oil = Figure::new(Troop::Oil, SIDE_B, 4);
        oil.owner_is_human = false;
        let p = resolve_power(WeaponClass::Bow, 50, 0, &oil, 0);
        assert!(p >= 2, "nothing is immune, got {p}");
    }

    #[test]
    fn high_ground_protects_the_target_and_only_against_missiles() {
        let target = Figure::new(Troop::Peasants, SIDE_B, 4);
        let level = resolve_power(WeaponClass::Crossbow, 200, 0, &target, 0);
        let slight = resolve_power(WeaponClass::Crossbow, 200, 2, &target, 0);
        let commanding = resolve_power(WeaponClass::Crossbow, 200, 4, &target, 0);
        assert!(commanding < slight && slight < level);
        assert_eq!(commanding, 200 / 5, "a fifth from four levels up");
    }

    /// The asymmetry documented but not explained: a human-owned figure gets
    /// *less* protection from high ground.
    #[test]
    fn human_owned_targets_get_less_high_ground_protection() {
        let mut ai = Figure::new(Troop::Peasants, SIDE_B, 4);
        ai.owner_is_human = false;
        let mut human = ai;
        human.owner_is_human = true;

        let ai_power = resolve_power(WeaponClass::Crossbow, 200, 4, &ai, 0);
        let human_power = resolve_power(WeaponClass::Crossbow, 200, 4, &human, 0);
        assert!(human_power > ai_power, "human takes more: {human_power} vs {ai_power}");
    }

    #[test]
    fn a_missile_kill_discards_the_remainder_but_melee_carries_it() {
        let mut by_missile = Figure::new(Troop::Peasants, SIDE_B, 4);
        apply_hit(&mut by_missile, 150);
        assert_eq!(by_missile.men, 3);
        assert_eq!(by_missile.hits, 0, "missile throws the extra 50 away");

        let mut by_melee = Figure::new(Troop::Peasants, SIDE_B, 4);
        by_melee.take_hits(150);
        assert_eq!(by_melee.men, 3);
        assert_eq!(by_melee.hits, 50, "melee carries the extra 50 forward");
    }

    #[test]
    fn one_missile_can_take_up_to_four_men_but_no_more() {
        let mut f = Figure::new(Troop::Peasants, SIDE_B, 8);
        assert_eq!(apply_hit(&mut f, 1000), 4, "capped at four");
        assert_eq!(f.men, 4);
    }

    #[test]
    fn a_missile_cannot_kill_more_men_than_are_present() {
        let mut f = Figure::new(Troop::Peasants, SIDE_B, 2);
        assert_eq!(apply_hit(&mut f, 1000), 2);
        assert_eq!(f.men, 0);
        assert_eq!(f.state, crate::figure::State::Dead);
    }
}


