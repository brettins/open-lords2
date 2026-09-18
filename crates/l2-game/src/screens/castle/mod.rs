//! **The castle chooser** — `Screen_CastleBuild` (`0x00419789`), `g_screenId`
//! `0x1B`, `L2.eng` group 71.
//!
//! **This screen is the whole "castle designer".** **[V]** The game says so in
//! its own help text: *"The castle screen displays five castle types. As you
//! click on each castle, the display in the upper right tells you the materials
//! required for that design and how long it will take workers to build it."*
//! and *"Start with a simple castle design, then upgrade as your materials and
//! builders increase."* — the only two occurrences of `design` in `L2.eng`, and
//! `Lords2.exe` holds none at all.
//!
//! It was one of the shells in [`crate::screens::shells`]: it drew a window and
//! did nothing. It is **the only place a player can order a castle**, and a
//! county with no castle cannot be besieged, so without it half the campaign
//! layer had no way in — `docs/decisions.md` C27's shape
//! [`l2_kingdom::County::castle_degraded`] had no reachable writer.
//!
//! ```text
//! g_castleTypeWidgets  0x004DC818  five kind-1 rectangles -> CastleBuild_Select
//!     (17,270)-(95,415)  (96,270)-(209,415)  (210,270)-(290,415)
//!     (291,270)-(414,415)  (415,270)-(618,415)          hotspot ids 0 … 4
//! g_castleBuildWidgets 0x004DDB80  two kind-5 sprites   -> CastleBuild_Confirm
//!     tick  frame 29 at (432,440)  hotspot 1   OK
//!     cross frame 31 at (472,444)  hotspot 0   cancel
//! ```
//!
//! **The five are the five castle pictures laid side by side**, and their widths
//! differ because the pictures do: 79, 114, 81, 124 and 204 pixels, tiling x 17
//! to 618 with no gap. So *"five buttons and an OK"* is literally a row of five
//! castles you point at. The selection is `DAT_0056D898`, a plain 0…4 that
//! [`crate::screens::map`]'s sidebar seeds from the county's own castle.
//!
//! ```text
//! Screen_CastleBuild(firstFrame):                              0x00419789
//!   File_ReadChunk("cas_back.256", &DAT_004EA8A0, 0x300)   the palette only
//!   FUN_00408FCB("cas_back.pl8", 0x1E0)      the backdrop: 640 x 480 raw
//!   Ui_OkButton(stride - 0x1C, height - 0x1C, 1)    the corner OK (612, 452)
//!   if (DAT_004D2DD0[sel] != 0):
//!     FUN_0040AE12("caspics.pl8", buf, DAT_004D2DD0[sel] - 1)
//!     Blit_Raster(buf, 0x9E, 0x14, 0x140, 200)   the big picture (158, 20)
//!   File_ReadChunk("cas_bits.pl8", g_villani2Sheet, 150000)
//!   DAT_005440B8 = 1;  Screen_CastleBuildPanel()
//!
//! Screen_CastleBuildPanel():   (only when DAT_005440B8)       0x004198AA
//!   Pl8_DrawFrame(cas_bits, DAT_004D2DE8[sel])   frame sel, (19, 63) — the plate
//!   Pl8_DrawFrame(cas_bits, DAT_004D2E28[sel])   frame 5+sel, over the chosen strip
//!   FUN_0040328E(71, sel + 1, 0x1F6, 0x18, 0xA0, heading)
//!                                  "Wooden palisade." … "Royal castle." (502, 24)
//!   Pl8_DrawFrame(cas_bits, 0x0A, 0x208, 0x5C)      the stone caption (520, 92)
//!   Ui_DrawNumber(stone, '@', "", 0x230, 0x60)                      (560, 96)
//!   Pl8_DrawFrame(cas_bits, 0x0B, 0x208, 0x7C)       the wood caption (520, 124)
//!   Ui_DrawNumber(wood,  '@', "", 0x230, 0x80)                      (560, 128)
//!   if (standing != 0):
//!     Pl8_DrawFrame(cas_bits, 0x0C, DAT_004D2E64[standing] - 10, 0x110)
//!   Eng_DrawString(71, 8, 0x20C, 0xB4, body)         "will take"    (524, 180)
//!   Ui_DrawCount(workforce, 0x26, 0x1F2, 0xC4, body)  N Builder(s)  (498, 196)
//!   Ui_DrawCount(1, 0x42, 0x1FC, 0xD4, body)          1 Season      (508, 212)
//!   Eng_DrawString(71, 9, 0x20C, 0xE4, body)         "to build."    (524, 228)
//!   Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)          the tax plaque (112, 428) 416 x 48
//!   Eng_DrawString(71, 0x10, 0x90, 0x1B4)   "Boosts tax revenues by" (144, 436)
//!   Ui_DrawNumber(bonus, ' ', " %", pen + 0x90, 0x1B4)
//!   Eng_DrawString(71, 0xF, 0xE0, 0x1C8)      "Start construction?"  (224, 456)
//!   Ui_DrawBox(8, 200, 8, 3)             the barracks plaque (8, 200) 128 x 48
//!   Eng_DrawString(71, 0xB, 0xC, 0xD2)        "Barracks for"          (12, 210)
//!   Ui_DrawNumber(cap, '@', " ", 0xC, 0xE2)                           (12, 226)
//!   Eng_DrawString(71, 0xC, pen + 0xE, 0xE2)  "troops."
//! ```
//!
//! `Castle_DrawStatusBlock` (`0x0041DEDB`), the map-information panel's castle
//! block, is their only consumer. [`STONE_NEEDED`] and [`WOOD_NEEDED`] name
//! them here so the next reader does not go looking on this screen.
//!
//! **Nor is `71/0` *"Select a castle to build"*.** Grepping every
//! `Eng_DrawString`, `Ui_DrawCentred` and `FUN_0040328E` call with a **literal**
//! group-71 index finds 6, 7, 8, 9, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12
//! and 0x13 — and neither 0 nor 0xA *"Build this castle"*. Index 0 of a group
//! is the group's own label (`docs/formats/eng.md` §5) and this heading is
//! painted into `cas_back.pl8`; 0xA is very likely a tooltip and is **[I]**.
//!
//! # `caspics.pl8` has four pictures for five castles  **[V]**
//!
//! `DAT_004D2DD0` is `{1, 0, 2, 3, 4}`, one-based with **0 meaning none**, and
//! the guard is `if (DAT_004D2DD0[sel] != 0)`. So selection 1, the motte and
//! bailey, blits **no big picture at all** and the backdrop shows through. The
//! shipped `Caspics.pl8` is 256,072 bytes, which is 72 of header and
//! `4 x 320 x 200` exactly — four rasters for four used slots, so the table and
//! the file close on each other and the gap is the original's, not a misread.
//!
//! `CastleBuild_Confirm` (`0x00436B59`) has exactly two guards, both of which
//! close the screen with a message:
//!
//! * the type picked is the one already standing — message `0x93`, `L2.eng`
//!   **147**: *"The castle in this county is already of the type you are
//!   proposing to change it to!!"*
//! * the type picked is **smaller** — message `0x122`, `L2.eng` **290**: *"Your
//!   current castle is stronger than the one you propose to upgrade to, my
//!   lord."*

mod constants;
pub use constants::*;
mod screen;
pub use screen::*;
mod tests_part;
pub use tests_part::*;

use l2_kingdom::industry::{self, CastleRefusal};
use l2_view::{text, Canvas};

use crate::press::{Press, Widget};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

// ---------------------------------------------------------------------------
// `L2.eng` group 71, and every index this screen draws
// ---------------------------------------------------------------------------

/// `L2.eng` group 71 — the literal first argument of all six `Eng_DrawString`
/// sites in `Screen_CastleBuildPanel` and of its one `FUN_0040328E`.
pub const GROUP: usize = 71;

pub const TITLE: usize = 0;
/// 1…5, `FUN_0040328E(71, sel + 1, …)`.
pub const NAME_BASE: usize = 1;
/// **Not drawn here.** `Castle_DrawStatusBlock` (`0x0041DEDB`) draws it; this
/// panel puts `cas_bits.pl8` frame [`STONE_CAPTION`] in its place.
pub const STONE_NEEDED: usize = 6;
pub const WOOD_NEEDED: usize = 7;
pub const WILL_TAKE: usize = 8;
pub const TO_BUILD: usize = 9;
pub const BARRACKS_FOR: usize = 0xB;
pub const TROOPS: usize = 0xC;
pub const START_CONSTRUCTION: usize = 0xF;
pub const BOOSTS_TAX: usize = 0x10;

/// `Ui_DrawCount(workforce, 0x26, …)` — `L2.eng` group 8 `0x26`/`0x27`,
/// *"Builder"* / *"Builders"*. **[V]** against the words.
pub const BUILDER_NOUN: usize = 0x26;
/// `Ui_DrawCount(1, 0x42, …)` — group 8 `0x42`/`0x43`, *"Season"* /
/// *"Seasons"*, and the value is the **literal 1**: every castle takes one
/// season regardless of type, which is a rule stated only by this draw call.
pub const SEASON_NOUN: usize = 0x42;


