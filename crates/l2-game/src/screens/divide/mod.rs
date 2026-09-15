//! **The army-division screen** — `Screen_ArmyDivision` (`0x004192B1`) and
//! `Screen_SplitArmyRows` (`0x00419354`), `g_screenId` `0x11`, `L2.eng` group
//! 17.
//!
//! ```text
//! File_ReadChunk("icon_tmp.pl8", ...)
//! Ui_DrawBox(8, 0x30, 0x1C, 0x1A)                  the window, 448 x 416 at (8, 48)
//! Ui_OkButton(0x1AC, 0x1B4, 0)
//! Eng_DrawString(17, 0, 0x68, 0x44, heading)       "Army Division."
//! Eng_DrawString(17, 1, 0x78, 0x1AE)               "Split the army?"
//! Ui_DrawBoxInterior(0x18, 0x80, 0x1A, 0x12)       the rows' well
//! for t in 0..7:   y = 0x80 + t * 0x20
//!     Ui_DrawUnitNoun(2, 0x34 + t*2, 0x18, y)          the troop's plural
//!     misc_cty frame 0x2F + t at (0xA8, y - 8)          the troop's PICTURE
//!     Ui_DrawNumber(basket[t].chosen,    0xD8, y)       who stays
//!     misc_cty frame 0x2F + t at (0x158, y - 8)         the same picture again
//!     Ui_DrawNumber(basket[t].available, 0x188, y)      who leaves
//! row 7, only when the army carries a band:
//! L2.eng 16/mercBand at (0x18, 0x180) and the troop noun at (0x58, 0x190)
//! the two "Total men" lines at y 0x184 with a band, 0x160 without
//! ```
//!
//! **`Screen_FrameInput`'s `0x11` arm holds no verb at all** — three exits, all
//! of them to `g_screenId = 0x04`. The buttons are `Screen_HandleInput`'s
//! `Widget_Test(0, 0, &DAT_004DD388, DAT_0055321C)`, which is the *second* of
//! the three places input hides (`docs/arms.json`'s `_note`), and nothing had
//! read it. So this module derived its hit boxes from the painter above, put
//! them on the two columns of `Misc_cty` troop pictures at `0xA8` and `0x158`,
//! and left the real arrows at 256 and 288 with nothing over them. See
//! [`PARENT_BUTTON_X`].
//!
//! **It also had three buttons of its own** — SPLIT, DISBAND and CANCEL in our
//! own font at y 446 — where the table has a tick and a cross at (288, 420) and
//! (336, 424). CANCEL overlapped the tick, so **our cancel sat on the
//! original's confirm**. All three are gone: [`SPLIT_TICK`] is the split,
//! [`SPLIT_CROSS`] is the cancel, and the disband is
//! [`crate::screens::info`]'s, which is where `Panel_DisbandButton`
//! (`0x0043733A`) has always lived — record 1 of the information panel's
//! `g_infoUnitButtons`. `docs/arms.json` counts all three as inventions.
//!
//! *"An army normally can only be split only at the start of its movement in a
//! turn."* — `FUN_004378B3` refuses with message `0x95` when `movesUsed >= 1`,
//! and the screen never opens. Ours opens and says why
//! silently will not appear is a screen a player thinks is broken.

mod constants;
pub use constants::*;
mod screen;
pub use screen::*;
mod tests_part;
pub use tests_part::*;

use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_view::{text, Canvas};

use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

