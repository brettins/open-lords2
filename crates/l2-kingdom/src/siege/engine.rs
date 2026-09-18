#![allow(unused_imports)]
use super::*;
use super::lifecycle::*;
use super::assault::*;
use crate::county::{County, MAX_COUNTIES};
use crate::math::pct_of;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{Unit, UnitKind, Units, MAX_UNITS};

/// The lord's `personality +0xA0`, or `None` for the human (lord 0) and for a
/// lord past the four the table holds.
pub fn siege_doctrine(t: &Tables, lord: u8) -> Option<i32> {
    if lord == 0 {
        return None;
    }
    t.ai.personality.get(lord as usize - 1).map(|p| p.siege_doctrine)
}

/// `Siege_RecomputeBuildTime` (`0x004A80DB`) — refresh each record's percentage
/// and write the seasons the screen prints.
pub fn recompute_build_time(units: &mut Units, army: usize) -> u8 {
    let Some(u) = units.get_mut(army) else { return 0 };
    let men = u.men;
    if men < 1 {
        return u.siege_seasons_left;
    }
    let mut remaining = 0i32;
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 {
            record.percent = 0;
            record.work_done = 0;
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        record.percent = pct_of(done, total) as i16;
        if record.percent < 100 {
            remaining += total - done;
        } else {
            record.percent = 100;
        }
    }
    u.siege_seasons_left = crate::math::div_ceil(remaining, men).clamp(0, 255) as u8;
    u.siege_seasons_left
}

/// `Siege_BuildTick` (`0x004A8507`) — **one season of construction**
/// only thing turn phase 2 does.
pub fn build_tick(units: &mut Units, army: usize) -> bool {
    let Some(u) = units.get_mut(army) else { return false };
    if u.owner == 0 || u.kind != UnitKind::Army || u.besieging_county == 0 {
        return false;
    }
    let men = u.men;
    if men < 1 {
        return false;
    }

    let incomplete = |u: &Unit| {
        (0..3).filter(|&e| u.engines[e].ordered > 0 && u.engines[e].percent < 100).count() as i32
    };
    let n = incomplete(u);
    let share = if n != 0 { men / n } else { men };

    let mut pot = men;
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 || record.percent >= 100 {
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        let spent = if share < total - done {
            record.work_done += share as i16;
            share
        } else {
            record.work_done = total as i16;
            total - done
        };
        pot -= spent;
    }

    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 || record.percent >= 100 {
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        if done >= total {
            continue;
        }
        if pot < total - done {
            record.work_done += pot as i16;
            pot = 0;
        } else {
            record.work_done = total as i16;
            pot -= total - done;
        }
    }

    let mut remaining = 0i32;
    for (record, cost) in u.engines.iter_mut().zip(ENGINE_WORK) {
        if record.ordered < 1 || record.percent >= 100 {
            continue;
        }
        let total = cost * record.ordered as i32;
        let done = record.work_done as i32;
        record.percent = pct_of(done, total) as i16;
        if record.percent < 100 {
            remaining += total - done;
        } else {
            record.percent = 100;
        }
    }
    u.siege_seasons_left = crate::math::div_ceil(remaining, men).clamp(0, 255) as u8;
    u.siege_seasons_left == 0
}

/// The four battle-only troop slots, `+0x17A … +0x180`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BattleEngines {
    pub catapults: i32,
    pub siege_towers: i32,
    pub battering_rams: i32,
    pub oil: i32,
}

impl BattleEngines {
    pub fn counts(&self) -> [i32; 4] {
        [self.catapults, self.siege_towers, self.battering_rams, self.oil]
    }

    pub fn total(&self) -> i32 {
        self.counts().iter().sum()
    }
}

/// **The counts that go into the battle are what was *ordered*, not what was
/// finished.** The original copies `+0x182`, `+0x188` and `+0x18E` — the
/// `ordered` field of each record — with no reference to the percentages, and
/// it can do that because [`tick_phase`] only reaches the assault when
/// `siegeSeasonsLeft` is zero, which means every ordered engine is complete.
pub fn prepare_besieger(unit: &Unit) -> BattleEngines {
    BattleEngines {
        catapults: unit.engines[Engine::Catapult.index()].ordered as i32,
        siege_towers: unit.engines[Engine::SiegeTower.index()].ordered as i32,
        battering_rams: unit.engines[Engine::BatteringRam.index()].ordered as i32,
        oil: 0,
    }
}

pub fn prepare_garrison(castle_level: u8) -> BattleEngines {
    BattleEngines {
        oil: OIL_BY_CASTLE_LEVEL.get(castle_level as usize).copied().unwrap_or(0),
        ..BattleEngines::default()
    }
}

