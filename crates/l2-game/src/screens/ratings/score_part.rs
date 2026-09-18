#![allow(unused_imports)]
use super::*;
use super::render::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

impl Snapshot {
    pub fn strength(&self) -> i32 {
        self.troops.iter().zip(STRENGTH_WEIGHT).map(|(n, w)| n * w).sum()
    }
}

/// **`DAT_0056D5CC`, the skirmish opponent's realm**, written only by
/// `Skirmish_Setup` (`0x0042B7F7`): `g_localPlayer == 1 ? 2 : 1`. **[V]**, the
/// one write in the binary.
pub fn skirmish_opponent(local: u8) -> u8 {
    if local == 1 {
        2
    } else {
        1
    }
}

impl Ratings {
    pub fn for_local(local: u8) -> Ratings {
        Ratings { realms: (local, skirmish_opponent(local)), ..Ratings::default() }
    }
}

impl Default for Ratings {
    fn default() -> Ratings {
        Ratings {
            mine: (Snapshot::default(), Snapshot::default()),
            theirs: (Snapshot::default(), Snapshot::default()),
            ending: Ending::Fought,
            realms: (1, skirmish_opponent(1)),
        }
    }
}

fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a * 100 / b
    }
}

/// **`FUN_0042C64F`** — both scores, `(local, opponent)`.
pub fn score(r: &Ratings) -> (i32, i32) {
    let lost_mine = r.mine.0.strength() - r.mine.1.strength();
    let lost_theirs = r.theirs.0.strength() - r.theirs.1.strength();
    let total = lost_mine + lost_theirs;
    let kill_mine = pct_of(lost_theirs, total);
    let kill_theirs = pct_of(lost_mine, total);
    let survive_mine = pct_of(r.mine.1.men, r.mine.0.men);
    let survive_theirs = pct_of(r.theirs.1.men, r.theirs.0.men);

    let full = |kill: i32, survive: i32| {
        KILL_SHARE_SCALE * kill + SURVIVAL_SCALE * survive + WIN_BONUS
    };
    let no_bonus = |kill: i32, survive: i32| KILL_SHARE_SCALE * kill + SURVIVAL_SCALE * survive;
    let kill_only = |kill: i32| KILL_SHARE_SCALE * kill;

    match r.ending {
        Ending::Withdrew { local: true } => (0, no_bonus(kill_theirs, survive_theirs)),
        Ending::Withdrew { local: false } => (no_bonus(kill_mine, survive_mine), 0),
        Ending::CastleFell { local: true } => {
            (kill_only(kill_mine), full(kill_theirs, survive_theirs))
        }
        Ending::CastleFell { local: false } => {
            (full(kill_mine, survive_mine), kill_only(kill_theirs))
        }
        Ending::Fought if r.mine.1.men < 1 => {
            (kill_only(kill_mine), full(kill_theirs, survive_theirs))
        }
        Ending::Fought => (full(kill_mine, survive_mine), kill_only(kill_theirs)),
    }
}

