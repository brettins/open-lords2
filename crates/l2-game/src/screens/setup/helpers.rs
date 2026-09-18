#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::ui::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

pub(super) fn begin_name(seed: &str) -> TextField {
    TextField::begin(seed, text::NAME_MAX_TYPED, text::NAME_MAX_PIXELS, text::Kind::Text)
}

/// `FUN_00403CF4(x, y, w, h, colour)` — **a flat one-pixel rectangle in one
/// colour**, four `FUN_00403A8F` line draws and nothing else.
///
/// It is not [`shell::inset_rect`] (two-tone, `0x10`/`0x1F`) and it is not
/// [`shell::button_recess`] / `FUN_00403EE4` (two-tone the other way,
/// `0x35`/`0x28`). Three different rectangles that all look like a border, and
/// this module drew the wrong one of the three on pages 3 and 13 until the
/// painters were read side by side.
pub(super) fn outline_rect(canvas: &mut Canvas, r: Rect, colour: u8) {
    canvas.fill_rect(r.x, r.y, r.w, 1, colour);
    canvas.fill_rect(r.x, r.y + r.h - 1, r.w, 1, colour);
    canvas.fill_rect(r.x, r.y, 1, r.h, colour);
    canvas.fill_rect(r.x + r.w - 1, r.y, 1, r.h, colour);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    Item(usize),
    Open(usize),
    Choose(usize),
    Map(usize),
    Skirmish(SkirmishArm),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SkirmishArm {
    /// `FUN_0043D929`, hotspot 0…5.
    Row(usize),
    /// `FUN_0043D9CD`, hotspot −1 and 1.
    Scroll(i32),
    /// `FUN_0043DC1D`, hotspot 0…3 — the category itself.
    Kind(usize),
    /// `FUN_0043DD83`, either muster.
    Sides,
    /// `FUN_0043DAF3`, hotspot 1 and 2.
    Handicap(usize),
    /// `FUN_0043DDF4` — the `.skr` field, which opens page 13.
    OpenFiles,
    /// `FUN_00434174`, the n'th of page 13's ten visible rows. Page 13's own
    /// scroll arrows are `SaveLoad_Scroll` (`0x00434346`) under list 2, and
    /// their widget record is not one we have read — see
    /// [`SetupScreen::scroll_skirmish_files`], which nothing on the page
    /// reaches yet.
    File(usize),
}

/// The kind byte at `+0x0F` of each record of the widget table at
/// `0x004DCF68` (`node tools/oracle/widgets.js widgets 4dcf68 22`): five of
/// page 12's records read **2**, `Hotspot_Test` (`0x0040E3EE`) kind 2 — the
/// down edge and then a flat pulse while held, `docs/input.md` §2. The other
/// five are kind 1 and answer the down edge alone.
pub(super) fn held_kind(arm: SkirmishArm) -> Option<crate::press::Kind> {
    Some(match arm {
        SkirmishArm::Scroll(_) => crate::arm!("0x0043D9CD/skirmish-scroll", Held),
        SkirmishArm::Handicap(_) => crate::arm!("0x0043DAF3/skirmish-handicap", Held),
        SkirmishArm::Kind(_) => crate::arm!("0x0043DC1D/skirmish-category", Held),
        SkirmishArm::Sides => crate::arm!("0x0043DD83/skirmish-swap-sides", Held),
        SkirmishArm::OpenFiles => crate::arm!("0x0043DDF4/skirmish-open-files", Held),
        // `FUN_0043D929` and `FUN_00434174` are kind 1.
        SkirmishArm::Row(_) | SkirmishArm::File(_) => return None,
    })
}

