//! **Nothing of ours may sit on a control of the game's.**
//!
//! Three times now a rectangle we invented has been placed in the original's
//! coordinate space without anyone checking what was already there:
//!
//! 1. a COUNTY PANEL button of ours, with a status line written across it, drawn
//!    over the five `g_sidebarButtons` icons — a player reported the icons "did
//!    nothing and had text over them"; they were live artwork under a dead
//!    rectangle;
//! 2. a BACK TO MAP button at (478, 460) 162 × 20 on the four county panels,
//!    which is `g_sidebarButtons` record 5 — **End Turn** — to the pixel, and
//!    hit-tested first;
//! 3. a CANCEL button at (264, 446) 100 × 18 on the army-division screen, which
//!    overlapped `g_splitWidgets` record 0 — **the confirm tick** at (288, 420)
//!    32 × 32 — in a 32 × 6 strip. Our cancel sat on the original's split.
//!
//! Each was found by a person looking. The third had a test of its own that
//! *passed*: it asserted the three buttons were left of `OK`, calling `OK`
//! *"the original's tick"*. `OK` is the corner picture at (428, 436); the tick
//! is somewhere else entirely. **A check that names a rectangle can name the
//! wrong one**.
//!
//! # The two halves, and why they are different in kind
//!
//! * [`the_right_columns_geometry_is_the_exes_own_tables`] compares our
//!   constants against **the player's `Lords2.exe`**, decoded at run time. That
//!   is two artefacts maintained by different work — ours by us, the table by
//! Impressions in 1996 —
//!   catches things. It is install-gated and skips on CI.
//! * [`no_overlay_swallows_the_campaign_minimap`] drives the machine with
//!   `Event` values and asks a behavioural question no geometry can:
//!   **does this screen consume a press that belongs to something underneath?**
//!   It needs no install, so it runs everywhere, and it is the half that would
//!   have caught defects 1 and 2 on the day they were written.
//!
//! Neither replaces reading the binary. Together they mean an invented hotspot
//! in the campaign column, or an overlay that eats the minimap, goes red on the
//! commit that introduces it.

mod geometry;
pub use geometry::*;
mod overlays_part;
pub use overlays_part::*;
mod end_turn;
pub use end_turn::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

/// `g_sidebarButtons` (`0x004DC680`) — six 24-byte hotspot records, tested at an
/// offset of (`0x1DE`, `0x1AE`) by `Sidebar_ButtonClicked`.
const SIDEBAR_TABLE: u32 = 0x004D_C680;
const SIDEBAR_OFFSET: (i32, i32) = (0x1DE, 0x1AE);
/// `g_minimapModeButtons` (`0x004DC620`) — four, at (610, 32).
const MINIMAP_MODE_TABLE: u32 = 0x004D_C620;
/// `g_splitWidgets` (`0x004DD388`) — 24-byte **widget** records, tested at (0, 0).
const SPLIT_TABLE: u32 = 0x004D_D388;

/// One hotspot record: `x0, y0, x1, y1` as four `i16`, then the handler.
/// `Hotspot_Test` is **half-open** — `x0 + off <= mx < x1 + off` — so the width
/// is `x1 - x0`.
fn hotspot(t: &l2_testkit::pe::Table, i: usize, off: (i32, i32)) -> Rect {
    let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
    let (x0, y0, x1, y1) = (s(0), s(1), s(2), s(3));
    Rect::new(x0 + off.0, y0 + off.1, x1 - x0, y1 - y0)
}

/// One **widget** record: `x, y` as two `i16`, then frame, size, handler. The
/// box is `size` square at `(x + offx, y + offy)`.
fn widget(t: &l2_testkit::pe::Table, i: usize, off: (i32, i32)) -> Rect {
    let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
    let dim = s(3);
    Rect::new(s(0) + off.0, s(1) + off.1, dim, dim)
}

