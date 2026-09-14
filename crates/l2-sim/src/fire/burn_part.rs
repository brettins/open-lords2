#![allow(unused_imports)]
use super::*;
use super::ignite_part::*;
use super::woodland::*;
use super::tests_part::*;
use crate::figure::{Figure, State};
use crate::missile::{Missile, Missiles, CLASS_FIRE, CLASS_OIL, SUB_CELL};
use crate::terrain::{Battlefield, DIM};

/// **A man stands in fire** — `BattleMan_BurnTick` (`0x0049459A`), one frame.
///
/// | size class | a man's figure | a siege engine (7, 8, 9) |
/// |---:|---:|---:|
/// | 0, 1 | 3 | 1 |
/// | 2 | 6 | 3 |
/// | 3 | 9 | 5 |
/// | 4 and up | 12 | 7 |
/// | a human's, add | 1 | 2 |
///
/// …against a threshold of **100** hits a man, or **160** for an engine, and
/// **one** man a frame at most: `hits -= threshold; men -= 1`. The remainder is
/// carried. `engine` is the figure's `+0x194`, which `BattleUnit_Create` sets
/// for troop types 7, 8 and 9 — **not 10**: a pot of oil burns as a man does,
/// at a man's threshold, whatever its own table says.
///
/// Both thresholds are literals in the body
/// `hits_per_casualty` does not reach this — as it does not reach the
/// original's. Returns `true` when the figure's last man died of it, which is
/// when the original plays the dying figure's side's cry.
pub fn burn(f: &mut Figure, size_class: u8, engine: bool) -> bool {
    if f.state == State::Dead {
        return false;
    }
    const MAN: [u16; 4] = [3, 6, 9, 12];
    const ENGINE: [u16; 4] = [1, 3, 5, 7];
    let rung = match size_class {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    };
    let (threshold, add, human) =
        if engine { (160u16, ENGINE[rung], 2u16) } else { (100u16, MAN[rung], 1u16) };
    f.hits = f.hits.saturating_add(add);
    if f.owner_is_human {
        f.hits = f.hits.saturating_add(human);
    }
    if f.hits >= threshold {
        f.hits -= threshold;
        f.men = f.men.saturating_sub(1);
    }
    if f.men < 1 {
        f.state = State::Dead;
        f.opponent = None;
        return true;
    }
    false
}

/// Whether a troop's figure carries `+0x194`, `isSiegeEngine` —
/// `BattleUnit_Create`: `6 < troopType && troopType < 10`. Oil is not one.
pub fn is_engine(troop: crate::Troop) -> bool {
    (7..=9).contains(&troop.index())
}

