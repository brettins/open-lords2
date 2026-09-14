#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use l2_kingdom::field::FieldType;
use l2_kingdom::industry;
use l2_formats::maps::Plane;
use l2_view::campaign::{self, Dir, Lattice, Viewport, Zoom, FAR, NEAR, PANEL_W, PANEL_X};
use l2_view::chrome::{self, Minimap, MinimapMode, MinimapTint};
use l2_view::village;
use l2_view::{text, Canvas, Clip, Ink, Tags};
use crate::input::{Event, Key, Rect};
use crate::press::Press;
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::battlefield::{
    BOX_SET, CONFIRM_BOX, CONFIRM_COLS, CONFIRM_NO_FRAME, CONFIRM_ROWS, CONFIRM_WIDGETS,
    CONFIRM_YES_FRAME, GROUP_CONFIRM,
};
use crate::screens::county;
use crate::screens::menubar;
use crate::screens::saveload::Mode as SaveLoadMode;
use crate::turn;
use crate::shell::{font, Pen};
use crate::widget;
use paint::*;

/// **`g_minimapModeButtons` (`0x004DC620`) — the four icons in the strip beside
/// the minimap**, `Misc_cty` frame `0x5C` (29 × 123) at (611, 32).
/// `FUN_0043292D` tests them at offset (610, 32) and `Minimap_ModeButton`
/// (`0x0043AB76`) handles all four.
///
/// The top three switch `g_minimapMode` — 1 the labour rating, 2 the food
/// rating, 3 happiness — which recolours the minimap from a second ramp
/// (`g_minimapRatingRamp`, `0x004D28F8`, transcribed as
/// [`chrome::MINIMAP_RATING_RAMP`]). The fourth is **the zoom toggle** in mode
/// 0 and **the way back out of an overlay** in every other mode; it is the
/// control `docs/screens.md` §7 says we replaced with the `Z` key.
/// [`MapScreen::minimap_mode_button`] has the whole of that behaviour.
///
/// The second record's `y1` is `0x42` where the pattern wants `0x3F`, so band 2
/// is 34 pixels tall and overlaps band 3's first two rows. `Hotspot_Test`
/// returns on the first match, so y 96 and 97 select mode 2. **That is the
/// original's own data**, transcribed.
/// What our status line calls each overlay. **Ours** — the original labels them
/// only with the button icons
///
/// **`L2.eng` does have words for them, and this comment said it did not.** The
/// original's tooltip layer (`FUN_00476E95`, gated on `g_optToolTips`) resolves
/// the three mode buttons through `FUN_00477320` to tip ids 2, 3 and 4 and
/// draws group **220** at those indices: *"Labour, red if needed, purple if
/// idle."*, *"Ration status"* and *"Overall happiness"* — with *"Overview map"*
/// on the fourth button and *"Return census map to empire mode"* (index 31) once
/// an overlay is up. `docs/draws-map.md` §5.1, **C86**. The status line stays
/// ours; the tips themselves are drawn
/// from the player's own group 220 by [`crate::tooltip`], which reads
/// [`MapScreen::minimap_mode`] through [`Screen::minimap_mode`].
fn minimap_mode_name(mode: MinimapMode) -> &'static str {
    match mode {
        MinimapMode::Owner => "OWNERS",
        MinimapMode::Labour => "LABOUR",
        MinimapMode::Food => "FOOD",
        MinimapMode::Happiness => "HAPPINESS",
    }
}

impl SidebarButton {
    pub const fn rect(&self) -> Rect {
        Rect::new(PANEL_X + self.x, chrome::PANEL_STATUS_Y, self.w, SIDEBAR_H)
    }
}

/// What one of them does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    /// The `g_screenId` the table's handler sets. Four of the five are still
    /// [`crate::screens::shells`] entries, so the button reaches the original's
    /// own artwork
    /// fifth is the raise-army screen, which is built. See
    /// [`sidebar_destination`].
    Screen(u8),
}

/// The slider's own arithmetic, verbatim: left of the track steps down by four,
/// right of it up by four, and on the track the value is
/// `((x - 531) * 2) & 0xFC` — masked, so it lands on a multiple of four.
///
/// **The three zones are half-open
/// original is `if (mx < 0x213) down; else if (mx < 0x252) track; else up;` —
/// so x = 594 steps the share **up**. This read `x > 594` and put that one
/// column on the track instead: a wrong arm.
/// kind nothing looks broken about.
pub fn split_from_click(x: i32, current: i32) -> i32 {
    let next = if x < 531 {
        current - 4
    } else if x >= 594 {
        current + 4
    } else {
        ((x - 531) * 2) & 0xFC
    };
    next.clamp(0, 100)
}

