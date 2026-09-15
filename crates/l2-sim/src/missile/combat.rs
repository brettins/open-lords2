#![allow(unused_imports)]
use super::*;

use crate::facing::facing_from_delta;
use crate::figure::Figure;
use crate::troop::Troop;

/// **Launch one missile.** `Missile_Spawn` (`0x0046E767`) writes geometry only;
/// everything else is written by the caller through `g_lastMissile`, which is
/// why this takes the whole record's worth of arguments.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    missiles: &mut Missiles,
    owner: u8,
    class: WeaponClass,
    shooter: usize,
    from: (u8, u8),
    to: (u8, u8),
    power: u16,
    launch_elevation: u8,
) -> Option<usize> {
    let slot = missiles.alloc()?;
    let m = missiles.get_mut(slot);
    *m = Missile {
        owner,
        class: class.index(),
        shooter: shooter as u16,
        x: from.0 as i16 * SUB_CELL,
        y: from.1 as i16 * SUB_CELL,
        target_x: to.0 as i16 * SUB_CELL,
        target_y: to.1 as i16 * SUB_CELL,
        cell_x: from.0 as i16,
        cell_y: from.1 as i16,
        dir: facing_from_delta(to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32)
            .unwrap_or(8),
        launch_elevation,
        blocked_ticks: ((shooter as u8) & 0x10) + 4,
        sub_steps: SUB_STEPS,
        range_ticks: class.stats().range_ticks(),
        power,
        ..Missile::default()
    };
    m.setup_line();
    Some(slot)
}

pub fn band_scaled(damage: u16, band: u8) -> u16 {
    match band {
        0 => damage,
        1 => damage * 4 / 5,
        2 => damage * 3 / 4,
        _ => damage / 2,
    }
}

pub fn resolve_power(
    weapon: WeaponClass,
    band_scaled_power: u16,
    elevation_delta: i32,
    target: &Figure,
    size_class: u16,
) -> u16 {
    let mut power = band_scaled_power as i32;

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

    power -= target.armour() as i32;

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

    power.max(2) as u16
}

pub fn apply_hit(target: &mut Figure, power: u16) -> u16 {
    let threshold = target.hits_per_casualty;
    target.hits = target.hits.saturating_add(power);
    if target.hits < threshold {
        return 0;
    }
    let multiples = (target.hits / threshold).min(4);
    let killed = multiples.min(target.men);
    target.men -= killed;
    target.hits = 0;
    if target.men == 0 {
        target.state = crate::figure::State::Dead;
        target.opponent = None;
    }
    killed
}

