//! Neither replaces reading the binary. Together they mean an invented hotspot
//! in the campaign column, or an overlay that eats the minimap, goes red on the
//! commit that introduces it.

mod geometry;
pub use geometry::*;
mod overlays_part;
pub use overlays_part::*;
pub(crate) mod end_turn;
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

fn hotspot(t: &l2_testkit::pe::Table, i: usize, off: (i32, i32)) -> Rect {
    let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
    let (x0, y0, x1, y1) = (s(0), s(1), s(2), s(3));
    Rect::new(x0 + off.0, y0 + off.1, x1 - x0, y1 - y0)
}

fn widget(t: &l2_testkit::pe::Table, i: usize, off: (i32, i32)) -> Rect {
    let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
    let dim = s(3);
    Rect::new(s(0) + off.0, s(1) + off.1, dim, dim)
}

