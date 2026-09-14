#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::drawing::*;
use super::*;
use l2_sim::runner::Formation;
use l2_sim::terrain::DIM;
use l2_view::{text, Canvas};
use crate::battlefield::{
    self, BannerLayout, Button, Cursor, LiveBattle, Mode, OVERVIEW, TILE, VIEW, VIEW_COLS,
    VIEW_ROWS,
};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::menubar;
use crate::shell::{font, Pen};
use crate::turn::{self, TurnStep};

/// **Cell byte `+5`, as `FUN_004BC51A` reads it, already turned into the
/// `t2_spri.pl8` frame it picks.** One byte a cell: the man's owning realm's
/// `shieldIndex`, `6` for the ownerless, `0` —
/// also the value the original's `if (DAT_005C9288 != 0)` guard drops.
///
/// ```c
/// if (g_battleMen[cell[+5]].owner == 6) colour = 6;
/// else colour = g_realms[g_battleMen[cell[+5]].owner].shieldIndex;
/// ```
///
/// The cell is [`l2_view::scene::drawn_cell`]'s, not `(f.x, f.y)`: byte `+5`
/// moves with `mapX`/`mapY`, and `FUN_00491B1F` moves those at the *start* of a
/// crossing. A man walking east is on the minimap's next cell for the whole of
/// it, as he is in the viewport. **[V]**
pub(super) fn overview_occupants(game: &crate::Game, live: &LiveBattle) -> Vec<u8> {
    let mut occupants = vec![0u8; l2_sim::terrain::CELLS];
    for i in 0..live.runner.fighters.len() {
        if !live.runner.is_alive(i) {
            continue;
        }
        let f = &live.runner.fighters[i];
        let owner = live.runner.sim.figures[f.sim].owner;
        let colour = if owner == l2_kingdom::levy::OWNERLESS {
            l2_kingdom::levy::OWNERLESS
        } else {
            game.kingdom.realms.get(owner as usize).map_or(0, |r| r.shield_index)
        };
        if colour == 0 {
            continue;
        }
        let ((cx, cy), _) = l2_view::scene::drawn_cell(f);
        if (0..DIM as i32).contains(&cx) && (0..DIM as i32).contains(&cy) {
            occupants[cy as usize * DIM + cx as usize] = colour;
        }
    }
    occupants
}

/// The cursor the ladder picks, exposed for the tests — the picture has no
/// cursor sheet to draw it with.
pub fn cursor_of(live: &LiveBattle) -> Cursor {
    live.cursor()
}


