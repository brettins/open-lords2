#![allow(unused_imports)]
use super::*;
use super::render::*;
use l2_view::Canvas;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};
use crate::Game;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sidebar {
    pub minimap_mode: u8,
    pub owned: bool,
    /// `DAT_0053F690`, `FUN_0040FEC1`'s farm list — see
    /// [`crate::screens::county::farm_rows`].
    pub farm: Vec<usize>,
    /// `DAT_00553FD0`, its industry list —
    /// [`crate::screens::county::industry_rows`].
    pub industry: Vec<usize>,
}

impl Sidebar {
    pub fn of(game: &Game, minimap_mode: u8) -> Sidebar {
        let k = &game.kingdom;
        let c = k.counties.get(game.selected as usize);
        Sidebar {
            minimap_mode,
            owned: c.is_some_and(|c| c.owner == game.player),
            farm: c.map(crate::screens::county::farm_rows).unwrap_or_default(),
            industry: c.map(crate::screens::county::industry_rows).unwrap_or_default(),
        }
    }
}

/// **`FUN_00477320` (`0x00477320`)** — the campaign sidebar's tip under the
/// pointer, or 0. The ladder is the original's, comparison for comparison.
///
// arm: 0x00477320/campaign-sidebar-tips hover
pub fn campaign_tip(s: &Sidebar, x: i32, y: i32) -> u8 {
    if y < 0x18 || x < 0x1DE {
        return 0;
    }
    if y < 0x9B {
        return if x < 0x261 {
            1
        } else if s.minimap_mode < 1 {
            if y < 0x3E {
                2
            } else if y < 0x5E {
                3
            } else if y < 0x7D {
                4
            } else {
                5
            }
        } else if y < 0x7D {
            match s.minimap_mode {
                1 => 2,
                2 => 3,
                _ => 4,
            }
        } else {
            0x1F
        };
    }
    if y < 0xFA {
        if !s.owned {
            return 0;
        }
        let middle = x >= 0x223 && x < 0x23B;
        return match (y < 0xD2, x < 0x223, middle) {
            (_, _, true) => 0x22,
            (true, true, _) => 6,
            (true, false, _) => 7,
            (false, true, _) => 8,
            (false, false, _) => 9,
        };
    }
    if y < 300 {
        return if s.owned { 10 } else { 0 };
    }
    if y < 0x1AE {
        if !s.owned {
            return 0;
        }
        if x < 0x230 {
            let pitch = crate::screens::county::farm_pitch(s.farm.len());
            let row = (y - 0x130) / pitch;
            return match s.farm.get(row as usize) {
                Some(1) => 0x0F,
                Some(0) => 0x10,
                Some(2) => 0x11,
                _ => 0,
            };
        }
        let pitch = crate::screens::county::industry_pitch(s.industry.len());
        let row = (y - 0x130) / pitch;
        return match s.industry.get(row as usize) {
            Some(6) => 0x12,
            Some(5) => 0x13,
            Some(4) => 0x14,
            Some(7) => 0x15,
            Some(3) => 0x16,
            _ => 0,
        };
    }
    if y < 0x1CC {
        return if x < 0x200 {
            0x0B
        } else if x < 0x221 {
            0x0C
        } else if x < 0x241 {
            0x0D
        } else if x < 0x260 {
            0x20
        } else {
            0x21
        };
    }
    0x0E
}

/// **`FUN_004777AA` (`0x004777AA`)** — the battlefield's tip under the
/// pointer, or 0: the overview, the selected troops, the troop levels, and the
/// five buttons along the bottom. **Eight ids**, 23 … 30.
///
// arm: 0x004777AA/battle-hud-tips hover
pub fn battle_tip(x: i32, y: i32) -> u8 {
    if y < 0x18 || x < 0x1DE {
        0
    } else if y < 0xB8 {
        0x17
    } else if y < 0x19C {
        0x18
    } else if y < 0x1C1 {
        0x19
    } else if x < 0x200 {
        0x1A
    } else if x < 0x220 {
        0x1B
    } else if x < 0x240 {
        0x1C
    } else if x < 0x260 {
        0x1D
    } else {
        0x1E
    }
}

/// **Where the box goes** — `FUN_00477131`'s two offsets and `FUN_00477249`'s
/// clamp, for a pointer at `(mx, my)`.
pub fn place(mx: i32, my: i32) -> (i32, i32) {
    let mut x = if mx < 0x141 { mx + 0x1E } else { mx - 0xDC };
    let mut y = if my < 0xF1 { my + 0x1E } else { my - 0x1E };
    // FUN_00477249, four independent tests.
    if x < 0 {
        x = 0;
    }
    if x > 0x1B7 {
        x = 0x1B8;
    }
    if y < 0 {
        y = 0;
    }
    if y > 0x1B7 {
        y = 0x1B8;
    }
    (x, y)
}

