#![allow(unused_imports)]

mod screen;
pub use screen::*;
mod drawing;
pub use drawing::*;
mod helpers;
pub use helpers::*;

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

/// `Tick_Pulses` (`0x004BBC80`)' 80 ms pulse, kept for the one counter the
/// battlefield reads off it. The dividers and the rounding are
/// [`crate::screens::armoury::Anim`]'s — a pulse is 20 ms rounded **up** to
/// whole frames, and every fourth is `g_pulse80`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Banner {
    acc_ms: u32,
    div: u8,
    /// `DAT_004E5B18`, 0 … 7.
    pub(super) phase: u8,
}

impl Banner {
    fn tick(&mut self) {
        self.acc_ms += crate::screens::armoury::TICK_MS;
        if self.acc_ms < crate::screens::armoury::PULSE_MS {
            return;
        }
        self.acc_ms = 0;
        self.div += 1;
        if self.div >= crate::screens::armoury::PULSE80_DIVIDER {
            self.div = 0;
            self.phase = (self.phase + 1) % l2_view::scene::BANNER_PHASES;
        }
    }
}

/// **The overview panel's framebuffer and its row cursor.**
///
/// `FUN_004BC1D1` (`0x004BC1D1`) is the whole schedule:
///
/// ```c
/// DAT_004E5D74 += param_1;                                  /* the cursor  */
/// if (DAT_004E6570 - param_1 < DAT_004E5D74) DAT_004E5D74 = 0;   /* 80 rows */
/// if (DAT_004E5D58 == 2) FUN_004BC51A(DAT_004E5D74, param_1);
/// ```
///
/// and its two callers set the rhythm. `Screen_DrawBattlefield` (`0x004233F7`)
/// enters the screen with `g_mapRedraw = 1; FUN_004bc1d1(0x50);` — a full
/// eighty-row pass. `Battle_Frame` (`0x004B99C0`) then runs, once a frame while
/// `g_battlePhase == 2` and `0x27 < g_screenId < 0x2B`:
///
/// ```c
/// if (g_mapRedraw == 0) { FUN_004bc1d1(4);    Gfx_MarkSpriteDirty(0x1E0, 0x18, 10, 10, 1); }
/// else                  { FUN_004bc1d1(0x50); Gfx_MarkAllDirty(); }
/// FUN_004bc142(cameraX, cameraY);      /* …which ends `if (g_mapRedraw) g_mapRedraw--;` */
/// ```
///
/// **So the panel is full only on the frame after it is entered, and four rows
/// a frame — a twenty-frame sweep — for the rest of the battle.** `[V]`; that
/// last decrement is what settles it, and without reading `FUN_004BC142` the
/// obvious reading is that the full pass runs every frame.
///
/// The original paints into the back buffer and the seventy-six rows it did not
/// visit keep the pixels they already had; ours keeps them in a raster of its
/// own and blits the whole of it, because our canvas has a yes/no box and a film
/// pushed over it and the original's screen has neither.
///
/// One difference that follows from that and is left: the original repeats the
/// entry pass every time `Screen_DrawBattlefield` runs, which includes the
/// return from an outcome film. The raster survives the push, so ours does not
/// need to — it is up to twenty frames behind for that one moment instead of
/// none.
pub(super) struct Overview {
    pub(super) raster: Canvas,
    /// `DAT_004E5D74`.
    pub(super) row: usize,
    /// `g_mapRedraw`, as this panel sees it: the next visit paints all eighty
    /// rows. Set on entry, cleared by the visit itself.
    pub(super) full: bool,
}

impl Overview {
    pub(super) fn new() -> Overview {
        Overview {
            raster: Canvas::new(l2_view::scene::OVERVIEW_SIDE, l2_view::scene::OVERVIEW_SIDE),
            row: 0,
            full: true,
        }
    }
}

