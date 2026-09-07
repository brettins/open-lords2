//! Missile combat.
//!
//! Reimplemented from `docs/battle.md` §6.2. Three things distinguish it from
//! melee, and all three are easy to lose by accident:
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

use crate::figure::Figure;
use crate::troop::Troop;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    Bow = 1,
    Crossbow = 2,
    Catapult = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissileStats {
    /// Range in cells. Stored in the original as sub-cell units of 8.
    pub range: u16,
    /// Ticks between shots.
    pub reload: u16,
    /// Base damage before band scaling, elevation and armour.
    pub damage: u16,
}

impl WeaponClass {
    pub fn stats(self) -> MissileStats {
        match self {
            WeaponClass::Bow => MissileStats { range: 15, reload: 50, damage: 50 },
            WeaponClass::Crossbow => MissileStats { range: 8, reload: 100, damage: 200 },
            WeaponClass::Catapult => MissileStats { range: 20, reload: 100, damage: 200 },
        }
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

/// Damage a missile delivers, before it is applied.
///
/// `elevation_delta` is the target's cell elevation minus the shooter's, so a
/// positive value means the target holds the high ground. It is always zero on
/// skirmish maps, which carry no elevation at all.
pub fn resolve_power(
    weapon: WeaponClass,
    band_scaled_power: u16,
    elevation_delta: i32,
    target: &Figure,
    size_class: u16,
) -> u16 {
    let mut power = band_scaled_power as i32;

    // 1. High ground. The human-owner case gets *less* protection, which is
    //    real in the original and unexplained - see docs/battle.md §6.2.
    if target.owner_is_human {
        if elevation_delta > 3 {
            power /= 4;
        } else if elevation_delta > 1 {
            power /= 2;
        }
    } else if elevation_delta > 3 {
        power /= 5;
    } else if elevation_delta > 1 {
        power /= 3;
    }

    // 2. Armour, as a flat subtraction. This is why a crossbow bolt still kills
    //    through plate and an arrow largely does not.
    power -= target.armour() as i32;

    // 3. Siege engines are a special case in several directions at once.
    if target.troop.is_siege() {
        match weapon {
            WeaponClass::Crossbow => {
                power /= if target.owner_is_human { 2 } else { 3 };
            }
            WeaponClass::Bow if power < 1 => {
                power = if target.owner_is_human { 6 } else { 4 };
            }
            _ => {}
        }
        let cap = 2 * (size_class as i32 * 5 + if target.owner_is_human { 10 } else { 5 });
        if power > cap {
            power = cap;
        }
    }

    // 4. A hit always lands for something. Nothing is immune.
    power.max(2) as u16
}

/// Apply a resolved missile hit. Returns men killed.
///
/// Unlike melee, leftover damage is thrown away, and a single hit can take up
/// to four men at once.
pub fn apply_hit(target: &mut Figure, power: u16) -> u16 {
    let threshold = target.troop.hits_per_casualty();
    target.hits = target.hits.saturating_add(power);
    if target.hits < threshold {
        return 0;
    }
    let multiples = (target.hits / threshold).min(4);
    let killed = multiples.min(target.men);
    target.men -= killed;
    // Discarded, not carried. The melee path keeps its remainder.
    target.hits = 0;
    if target.men == 0 {
        target.state = crate::figure::State::Dead;
        target.opponent = None;
    }
    killed
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
