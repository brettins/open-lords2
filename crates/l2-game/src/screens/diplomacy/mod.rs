//! `Diplo_DrawScreen` (`0x00416CF3`) and `Screen_DiploDialog` (`0x0041789B`).
//!
//! ```text
//! Diplo_DefaultTarget()                       if the target is gone
//! g_diploMenuState = 3 if pair[target][me].hasMail else 1/0/2 by my ally
//! FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)        the window, 448 x 432 at (16,32)
//! Ui_OkButton(0x1A8, 0x1A6, 0)
//! File_ReadChunk("faces.pl8", …)
//! for each rival still in play:  Diplo_DrawLordCard(realm, slot++)
//! Ui_DrawText(g_playerNames[target], 0xD0, 0x3D, heading)
//! then one of four menus from L2.eng group 72
//! ```
//!
//! and one card, out of `Diplo_DrawLordCard` (`0x004171EE`):
//!
//! ```text
//! Ui_DrawInsetRect(0x30, slot*100 + 0x31, 0x52, 0x4E)      82 x 78
//! faces.pl8 frame lord*3 - 3   at (0x31, slot*100 + 0x32)  frame 12 for a human
//! Misc_cty.pl8 frame shield + 0x55 at (0x20, slot*100 + 0x37)
//! Ui_DrawText(g_playerNames[realm], 0x20, slot*100 + 0x83)
//! if realm == target:  two outlines at (0x2F, +0x30) and (0x2E, +0x2F)
//! if pair[realm][me].hasMail:  frame 15 at (0x1E, slot*100 + 0x50)
//! if pair[realm][me].allied:   frame 13 at (0x9E, slot*100 + 0x41)
//! else if pair[realm][me].atWar: frame 14 at (0x92, slot*100 + 0x41)
//! the standing thermometer: 10 x 63 at x = 0x88, filled from +30 down
//! ```
//!
//! The test is `pair[target][localPlayer].hasMail` — the *target's* record,
//! indexed by *me* — and `Diplo_Post` sets `pair[to][from].hasMail`. So the
//! flag means **I** have a letter sitting unanswered in **their** inbox, which
//! is exactly what group 72 index 24 says: *"A message has been dispatched, my
//! Lord."* One letter per rival per turn
//! answer it.
//!
//! | kind | painter | widgets | what it needs |
//! |---|---|---|---|
//! | 0 gift | `Diplo_DrawGiftGold` `0x00417960` | `0x004DD9D0`, **four** | an amount |
//! | 1–4 letters | `Diplo_DrawLetter` `0x00417AEF` | `0x004DDA30`, two | a 199-character draft |
//! | 5–6 requests | `Diplo_DrawCountyRequest` `0x00417CEF` | `0x004DDA60`, two | a county |
//!
//! The gift table's extra pair is the **+10 / −10** stepper at (184, 240) and
//! (216, 240), handled by `FUN_00436372`, which clamps the amount to
//! `[0, my gold]` on every click.
//!
//! **The free-text letter is not written here
//! what you type into.** Screen `0x1A`'s arm calls
//! `FUN_0040210C(g_diploLetterDraft + (kind - 1) * 200, 199)` on every frame
//! the widget test declines — and `FUN_0040210C` is a bounded copy **out of
//! `DAT_005CD550`**, the game's one shared text-edit buffer, which
//! `FUN_00401D26(ch)` inserts typed characters into at cursor `DAT_005BB4A8`.
//!
//! So there is a single editor
//! `g_diploLetterDraft` are snapshots harvested from it once a frame;
//! `Diplo_OpenCompliment` and its three siblings run the copy the other way
//! (`FUN_00402009`) when the dialog opens.
//!
//! `Diplo_SendClicked` (`0x00436408`) is the tick. Two things about it are
//! worth having in front of you, because both are surprising:
//!
//! * **the county tests are asymmetric.** Kind 5 wants a county that is
//!   **mine** and has an enemy standing in it (`+0x19C`); kind 6 wants one that
//!   is **not** mine and not my ally's. Group 242 *"does not belong to us"* is
//!   raised only by kind 5 — kind 6 answers a county of my own with group 244,
//!   *"This county is part of our alliance"*, which is not what has gone wrong.

mod main;
pub use main::*;
mod screen;
pub use screen::*;
mod compose;
pub use compose::*;
mod tests;
pub use tests::*;

use l2_kingdom::diplomacy::{group, Kind};
use l2_kingdom::realm::MAX_REALMS;
use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
// **`Ui_DrawText(&g_playerNames + realm * 0x2C, …)`, once for the whole
// crate.** This module had a copy that stopped at `g_playerNames` and printed
// `REALM n` when it was empty; `docs/decisions.md` C189's pair of sources is
// the whole rule and lives in one place.
use crate::screens::message::lord_name;
use crate::shell::{font, Pen};
use crate::widget;

/// `L2.eng` group 72 — *"Diplomacy."*, and every label on both screens.
pub const GROUP: usize = 72;

/// `FUN_004093E0(0x10, 0x20, 0x1C, 0x1B)` — the window, in cells of 16.
pub const WINDOW: Rect = Rect::new(0x10, 0x20, 0x1C * 16, 0x1B * 16);
pub const WINDOW_COLS: i32 = 0x1C;
pub const WINDOW_ROWS: i32 = 0x1B;

pub const OK: Rect = Rect::new(0x1A8, 0x1A6, 32, 32);

/// `lord * 3 − 3` are the portraits (12 for a human rival) and 13, 14, 15 are
/// the three status icons. **[I]** the icon frames: the *positions* are the
/// painter's literals, the identification of 13/14/15 as allied, at-war and
/// mail is the branch each is drawn under and has not been checked against the
/// pictures.
const FACES: &str = "Faces.pl8";
const ALLIED_ICON: usize = 13;
const AT_WAR_ICON: usize = 14;
const MAIL_ICON: usize = 15;

const SHIELD_BASE: usize = 0x55;

const SEAL_FRAME: usize = 0x1D;
const SEAL_AT: (i32, i32) = (0x140, 0x140);

/// `Widget_Draw(0, 0, &g_diploWidgets, …)` — every one of the six records
/// carries `System.pl8` frame 64. **[V]**
/// `tools/oracle/widgets.js widgets 4dd940 6`.
pub const MENU_FRAME: usize = 64;

pub const MENU_INSET_X: i32 = 0xD0;
pub const MENU_INSET_Y: i32 = 0x60;
pub const MENU_INSET_W: i32 = 0xE8;

/// `FUN_0040328E(72, row, 0xE0, y, 0xA0, 100, …)` — the label, **wrapped** at
/// 160 pixels.
pub const MENU_LABEL_X: i32 = 0xE0;
pub const MENU_LABEL_W: i32 = 0xA0;

/// `FUN_00403CF4`'s two colours on the selected card: `0xF9` inside `0x3F`.
const SELECTED_INNER: u8 = 0xF9;
const SELECTED_OUTER: u8 = 0x3F;

pub fn card_rect(slot: usize) -> Rect {
    Rect::new(0x30, slot as i32 * 100 + 0x31, 0x52, 0x4E)
}

pub const THERMOMETER_W: i32 = 10;
pub const THERMOMETER_H: i32 = 63;
pub const THERMOMETER_X: i32 = 0x88;

pub const THERMOMETER_WARM: i8 = 11;
pub const THERMOMETER_COLD: i8 = -11;

/// The column inside the recess: `FUN_0040437D(0x89, slot*100 + 0x41, 8, 0x3D,
/// 0x3F)`, so 8 wide and **61** rows — one row per point of standing from +30
/// down to −30 inclusive.
pub const THERMOMETER_FILL_W: i32 = 8;
pub const THERMOMETER_FILL_H: i32 = 0x3D;
/// `FUN_0040437D`'s four palette literals, in the painter's own order.
pub const THERMOMETER_EMPTY: u8 = 0x3F;
pub const THERMOMETER_HIGH: u8 = 0xFA;
pub const THERMOMETER_MID: u8 = 0xFC;
pub const THERMOMETER_LOW: u8 = 0xF9;

/// `L2.eng` group 7 — the five lord titles. **Only a fallback here**: the
/// painter draws `g_playerNames`, and this is what the card shows when there is
/// no `Faces.pl8` to put a portrait in.
pub const LORD_TITLE_GROUP: usize = 7;

pub struct DiplomacyScreen {
    /// `g_diploTarget` (`0x0053F03C`). `None` until the first draw, because
    /// `Diplo_DefaultTarget` needs the realm array and a screen is built
    /// without one — the same resolve-on-first-use `castle.rs` uses.
    target: Option<u8>,
    /// `g_diploWidgets`' press timer. The six verb buttons are **kind 5**, read
    /// out of `+0x0F` of `0x004DD940` … `0x004DD9B8`: the button goes down and
    /// the dialog opens twenty frames later.
    press: Press,
    pending_rows: [Option<usize>; MENU_SLOTS],
}

/// Decoded out of `Lords2.exe`, `0x004DD9D0` is a contiguous run of 24-byte
/// widgets: records 0…3 are the gift's four, records 4…5 *are* `0x004DDA30`
/// (`0x004DD9D0 + 4 × 24`) and records 6…7 *are* `0x004DDA60`
/// (`+ 6 × 24`). Every slice is
/// `Screen_DrawWidgets` arm passes, so the `g_sendSuppliesWidgets` failure —
/// eight records, six ever drawn — has no counterpart here. The run continues
/// past the compose dialogs into other screens' tick/cross pairs at
/// `0x004DDA90` and beyond, whose handlers (`FUN_004367FF`, `FUN_00436872`,
/// `FUN_004368FD`) are message replies.
///
/// **[V]** `tools/oracle/widgets.js widgets 4dd9d0 8`.
pub const WIDGET_TABLE: u32 = 0x004D_D9D0;

pub struct ComposeScreen {
    target: u8,
    kind: Kind,
    /// `g_diploGold` (`0x0057A0F8`), for kind 0 only.
    pub gold: i32,
    pub county: u8,
    /// One of the four 200-byte buffers at `g_diploLetterDraft`
    /// (`0x0053F2B8`), as the editor that fills it.
    ///
    /// **`None` until the first frame with a [`Ctx`]**: `Edit_Begin` takes the
    /// buffer's current contents, and ours come from `L2.eng` group 226
    /// through [`letter_default`], which needs assets
    /// [`ComposeScreen::new`] has not got.
    letter: Option<crate::text::TextField>,
    pub sent: Option<Result<Kind, Refusal>>,
    press: Press,
}

