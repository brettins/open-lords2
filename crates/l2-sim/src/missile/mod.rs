//! This module used to stop at [`resolve_power`]: a hit resolved and nothing
//! flew, so sixty archers fought as sixty men carrying bows. The question that
//! had to be settled before any of this could be written was whether the
//! original resolves a shot at launch and merely animates it, or whether the
//! arrow genuinely traverses. **It genuinely traverses**
//! one line of `Missile_Step` (`0x00492C8B`):
//!
//! The victim is read out of the **cell the missile has just entered**, fresh,
//! every sub-step. Nothing anywhere on the 0x4C-byte record remembers who the
//! shot was aimed at — `+0x06` is the *shooter* — so an arrow cannot check
//! whether it hit the right man, and does not. Four consequences, all of them
//! visible in play and all of them reproduced here:
//!
//! `docs/battle.md` §0 once called class 7 *"falling men"*; it is boiling oil,
//! and the only spawner in the binary is `FUN_0047A814`.

mod types;
pub use types::*;

mod combat;
pub use combat::*;

use crate::facing::facing_from_delta;
use crate::figure::Figure;
use crate::troop::Troop;

/// **[V]** `Missile_Spawn` writes `x << 5` and `Missile_Step` recovers
/// `(x + 8) / 32`.
pub const SUB_CELL: i16 = 32;

pub const SUB_STEPS: i8 = 4;

pub const MAX_MISSILES: usize = 100;

pub const LAUNCH_STEPS: u32 = 8;

pub const BLOCKED_LIMIT: u8 = 0x20;

pub const HIT_TTL: i16 = 2;

pub const DEBRIS_TTL: i16 = 0x78;

/// **The cell byte a shot may not pass.** `Missile_Step`: `cell.terrain++; if
/// (0xF < cell.terrain) Wall_Collapse(cell);` — and because
/// `Battlefield_BuildCastle` seeds a castle's non-moat cells at 1, a wall takes
/// **fifteen** shots, not sixteen. **[V]**
pub const WALL_DAMAGE_MAX: u8 = 0x0F;

pub const WALL_HITS_PER_COLLAPSE: u8 = 16;

/// `[V]`.
pub const WALL_TOO_HIGH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    Bow = 1,
    Crossbow = 2,
    Catapult = 3,
}

pub const CLASS_DEBRIS: u8 = 4;
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
    pub range: u16,
    pub reload: u16,
    pub damage: u16,
}

impl MissileStats {
    pub fn range_ticks(&self) -> i16 {
        (self.range * 8) as i16
    }
}

pub const ACQUIRE_LEAD: u16 = 10;

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

    /// A catapult is a wall-breaker and only a wall-breaker. **[V]**
    pub fn hits_men(self) -> bool {
        (self as u8) < 3
    }

    pub fn for_troop(troop: Troop) -> Option<WeaponClass> {
        match troop {
            Troop::Archers => Some(WeaponClass::Bow),
            Troop::Crossbowmen => Some(WeaponClass::Crossbow),
            Troop::Catapults => Some(WeaponClass::Catapult),
            _ => None,
        }
    }
}

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

    pub fn live(&self) -> usize {
        self.slots.iter().filter(|m| m.is_live()).count()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Missile)> {
        self.slots.iter().enumerate().skip(1).filter(|(_, m)| m.is_live())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::SIDE_B;

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
        assert!(100 / arrow_vs_plate >= 6);
        assert!(100 / bolt_vs_plate < 1);
    }

    #[test]
    fn a_hit_always_lands_for_at_least_two() {
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


