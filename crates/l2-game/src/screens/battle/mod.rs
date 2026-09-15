//! **The two battle screens** — `Screen_BattlePrompt` (`0x00422D22`, screen
//! `0x12`) and `Screen_BattleResult` (`0x00422FF1`, screen `0x13`).
//!
//! ```text
//!                                 0x12                      0x13
//! Ui_DrawBoxBorder(1, …)          0x20,0x30,0x1A,0x19       same
//! county name, group 100          (132, 66) centred w306    same, drawn later
//! heading                         80.0  / 80.7 siege        81.0 / 81.8 siege
//! paragraph                       80.1 / 80.2 / 80.3        — none —
//! medallion + two shields         icon_tmp.pl8              same
//! two lord names                  (40,160) and (240,160)    same
//! roster FUN_004224E7             mode 0                    mode 1
//! buttons                         thumb up / thumb down     the OK corner
//! ```
//!
//! Transcribed from `Screen_BattlePrompt` (`0x00422D22`); `Screen_BattleResult`
//! (`0x00422FF1`) is the same list with `Ui_OkButton(400, 400, 0)` in place of
//! the paragraph, group 81 in place of group 80, roster mode 1 in place of mode
//! 0, and the county name and heading moved to the end. Coordinates resolved to
//! decimal in the trailing comment.
//!
//! ```text
//! Screen_BattlePrompt():                                        0x00422D22
//!   Screen_DrawCampaign(1)                       the map underneath — excluded
//!   FUN_004093E0(0x20, 0x30, 0x1A, 0x19)     window, set 1, (32, 48) 416 x 400
//!   Ui_DrawCentred(100, slot*20 + county, 0x84, 0x42, 0x132, body)
//!                                                centred in x 132..438, y 66
//!   Eng_DrawString(80, siege ? 7 : 0, 0x8E, 0x56, heading)         (142, 86)
//!   FUN_0040328E(80, 1|2|3, 0x8E, 0x7A, 0xB4|0x118, 400, 0, 0, body)
//!                                       wrapped at 180 for 1, 280 for 2 and 3
//!   File_ReadChunk("icon_tmp.pl8", spriteBuffer, 160000, 0)
//!   Ui_DrawBevelRect(0x34, 0x44, 0x52, 0x52)   (52, 68) 82 x 82, FOUR LINES
//!   Sprite_WGenSprite(siege ? 0x38 : 0x37, 0x35, 0x45)      (53, 69) crossed
//!                                              swords, or a castle for a siege
//!   Sprite_WGenSprite(shieldA + 0x30, 0x7A, 0xB4)                (122, 180)
//!   Sprite_WGenSprite(shieldB + 0x30, 0x142, 0xB4)               (322, 180)
//!   FUN_004025D7(playerNames[ownerA], 0x28, 0xA0, 200, body)   centred, y 160
//!     — or Ui_DrawCentred(99, 0, …) when the owner is 6, "The people."
//!   FUN_004025D7(playerNames[ownerB], 0xF0, 0xA0, 200, body)     (240, 160)
//!   FUN_004224E7(armyA, armyB, 0x3E, 0xE0, body, 0)               the roster
//!
//! FUN_004224E7(a, b, x=0x3E, y=0xE0, font, mode):                0x004224E7
//!   mode 0, seven rows at y + row*0x18:
//!     Ui_DrawNumber(a.troops[row], ' ', " ", x+0x28,  y+4)             x 102
//!     Pl8_DrawFrame(misc_cty, 0x2F + row, x+0x55,  y)                  x 147
//!     Ui_DrawUnitNoun(2, 0x34 + row*2, x+0x7D, y+4)                    x 187
//!     Pl8_DrawFrame(misc_cty, 0x2F + row, x+0xF5, y)                   x 307
//!     Ui_DrawNumber(b.troops[row], ' ', " ", x+0x118, y+4)             x 342
//! and the row's two counts are STASHED in DAT_00568420/DAT_0056843C
//!   then Ui_DrawCount(a.menTotal, 0x48, x+0x14,  y + 7*0x18 + 0xC)  (82, 404)
//!        Ui_DrawCount(b.menTotal, 0x48, x+0xDC, …)                (282, 404)
//!   mode 1, the same seven rows plus the two stashed figures:
//!     Ui_DrawNumber(stashedA[row], '(', ")", x-6,     y+4)              x 56
//!     Ui_DrawNumber(stashedB[row], '(', ")", x+0x146, y+4)             x 388
//!   mode 2 is UnitPanel_Draw's two-column army panel and is unreachable here
//! ```
//!
//! **The mercenary band is folded into its troop type before the row is drawn**
//! — `if (unit.mercTroop == row) count += unit.mercMen` — in both modes, and it
//! is the stashed value too,
//! swordsmen. **This listing described the fold from
//! the day it was written and neither screen did it**, and neither drew
//! `menTotal` either — both are [`crate::engagement::roster_of`] and
//! `draw_roster`'s `totals` now. C189.
//!
//! `FUN_004093E0` is not `Ui_DrawBox`. It is the *other* frame kit, and passing
//! set 0 draws a visibly wrong window — [`Pen::window`]'s last argument is 1 for
//! both of these and 0 for almost everything else in the game.
//!
//! * **The lords' names.** `g_playerNames` **is** modelled — `Game::player_names`,
//!   filled by `Player_SetHuman` (`0x0049BAE9`) for the person and by
//!   `Eng_Seek(7, realm.lord)` for the AIs — and [`side_name`] reads it. It drew
//! `"LORD {owner}"` until C189,
//!   modelled and stopped being true when they were. The other name here is
//!   the original's own — `L2.eng` group 99, the single string
//!   `"The people."`, which it draws for an ownerless army. A county levy is
//!   exactly that. **The original's test is `owner == 6`**, because 6 is the
//!   peasant faction's owner byte; ours is `owner == 0`, because
//!   `l2_kingdom::realm` numbers realms 1..=5 and reserves 0 for nobody.
//!
//! * **A victory sentence.** There isn't one. `L2.eng` group 81 carries *"are
//!   victorious." / "have been defeated." / "have been crushed." / "have been
//!   annihilated."* and group 80 carries *"The army of" / "The people of" / "The
//!   bandits from"* — and **none of the seven is drawn anywhere in the
//!   binary.** Every `Eng_DrawString` on group 80 is index 0, 1, 2, 3 or 7 and
//!   every one on group 81 is index 0 or 8. They are dead strings, and a screen
//!   that composed *"The army of X have been crushed"* out of them would be
//!   inventing a sentence the game never printed. Recorded here because the
//!   strings look exactly like an instruction to do it.
//!
//! Not on `0x13`. `L2.eng` group 82's seven heading/body pairs belong to
//! `Screen_BattleOutcome`, screen `0x2B`, which is a separate screen this module
//! does not build. It was read for the draw-call audit and is recorded here
//!
//! ```text
//! Screen_BattleOutcome():                                       0x00423241
//!   if (g_battleChoiceOwner == 0)              the local player is a bystander
//!     FUN_004093E0(0x10, 0x90, 0x1C, 10)   window, set 1, (16, 144) 448 x 160
//!     Ui_OkButton(0x1A0, 0x100, 0)                                (416, 256)
//!     Eng_DrawString(82, 0xC, 0x30, 0xA8, heading)  "The conflict is over."
//!     FUN_0040328E(82, 0xD, 0x30, 0xE0, 0x180, 100, 0, 0, body)  wrapped at 384
//!   else if (g_optAnimations == 0)                  the same window, the pair
//!     FUN_004093E0(0x10, 0x90, 0x1C, 10)                   chosen by outcome
//!     Ui_OkButton(0x1A0, 0x100, 0)
//!     Eng_DrawString(82, outcome * 2,   0x30, 0xA8, heading)
//!     FUN_0040328E(82, outcome * 2 + 1, 0x30, 0xE0, 0x180, 100, 0, 0, body)
//!   else                                   animations on: a taller window and
//!     FUN_004093E0(0x10, 0x30, 0x1C, 0x16) a recess for the video, (16, 48)
//!     Ui_DrawInsetRect(0x27, 0x48, 0x192, 0xC2)        (39, 72) 402 x 194
//!     Ui_OkButton(0x1A0, 0x160, 0)                              (416, 352)
//!     Eng_DrawString(82, outcome * 2,   0x30, 0x138, heading)        y 312
//!     FUN_0040328E(82, outcome * 2 + 1, 0x30, 0x158, 0x180, 100, 0x20, 0x1A0, body)
//! ```
//!
//! `Battle_SelectOutcomeBanner` (`0x00478419`) writes `g_battleOutcome` and only
//! ever writes **0…5**: `siege ? (localWon ? (Awon ? 2 : 4) : (Awon ? 5 : 3))
//!: (localWon ? 0: 1)`. The seventh pair, 12/13,
//! `g_battleOutcome` at all — it is the `g_battleChoiceOwner == 0` branch, *"a
//! battle between two other realms"*. **That is what makes the count seven, and
//! all seven are reachable.** `docs/battle.md` does not name them; the seven
//! are `L2.eng` 82's pairs at 0/1 won, 2/3 lost, 4/5 siege won, 6/7 siege lost,
//! 8/9 siege lifted, 10/11 castle lost, 12/13 the conflict is over.

mod render;
pub use render::*;
mod prompt;
pub use prompt::*;
mod result;
pub use result::*;
mod tests_part;
pub use tests_part::*;

use l2_kingdom::battle::Outcome;
use l2_kingdom::unit::ALL_TROOP_TYPES;
use l2_view::chrome::system;
use l2_view::{text, Canvas};

use crate::engagement::{Answer, Roster};
use crate::input::{Event, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::turn::{self, Question, TurnStep};

/// `L2.eng` group 80 — the prompt's own text.
pub const GROUP_PROMPT: usize = 80;
/// `L2.eng` group 81 — the result's.
pub const GROUP_RESULT: usize = 81;
/// `L2.eng` group 82 — the seven outcome heading/body pairs.
pub const GROUP_BANNER: usize = 82;
/// `L2.eng` group 99 — one string, `"The people."`, quotes included, which the
/// original draws in place of a lord's name for an ownerless army.
pub const GROUP_OWNERLESS: usize = 99;
/// `L2.eng` group 8's troop nouns, two apiece: singular then plural. The battle
/// roster always takes the plural, because `Ui_DrawUnitNoun`'s count argument
/// is the literal 2.
pub const NOUN_BASE: usize = 0x34;
/// `L2.eng` group 100 — the county names, `map_slot * 20 + county`.
pub const GROUP_COUNTY: usize = 100;
pub const NOUN_TOTAL: usize = 0x48;

/// `FUN_004093E0(0x20, 0x30, 0x1A, 0x19)` — **border set 1**, 26 × 25 cells of
/// sixteen pixels, so 416 × 400 at (32, 48).
pub const BOX_X: i32 = 0x20;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1A;
pub const BOX_ROWS: i32 = 0x19;
pub const BOX_SET: usize = 1;

pub fn window() -> Rect {
    Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16)
}

const COUNTY: (i32, i32, i32) = (0x84, 0x42, 0x132);
const HEADING: (i32, i32) = (0x8E, 0x56);
/// `FUN_0040328E(0x50, …, 0x8E, 0x7A, …)` — the wrapped paragraph.
const PARAGRAPH: (i32, i32) = (0x8E, 0x7A);
const MEDALLION: Rect = Rect::new(0x34, 0x44, 0x52, 0x52);
pub const MEDALLION_SHEET: &str = "Icon_tmp.pl8";
pub const MEDALLION_BATTLE: usize = 0x37;
pub const MEDALLION_SIEGE: usize = 0x38;
pub const MEDALLION_AT: (i32, i32) = (0x35, 0x45);
/// `Sprite_WGenSprite(shield + 0x30, …)` — the two realms' shields, above their
/// names. `Battle_ChooseSettlement` (`0x004A6A30`) writes
/// `if (shield == 0) shield = 6`, so an ownerless army takes frame 54.
pub const SHIELD_FRAME0: usize = 0x30;
pub const OWNERLESS_SHIELD: usize = 6;
pub const SHIELD_A: (i32, i32) = (0x7A, 0xB4);
pub const SHIELD_B: (i32, i32) = (0x142, 0xB4);
const LORD_A: (i32, i32, i32) = (40, 160, 200);
const LORD_B: (i32, i32, i32) = (240, 160, 200);

/// `FUN_004224E7(a, b, 0x3E, 0xE0, …)` — the roster's origin, and the pitch of
/// its seven rows.
const ROSTER_Y: i32 = 0xE0;
const ROW_PITCH: i32 = 0x18;
/// Absolute x of each column: `FUN_004224E7`'s `param_3` is `0x3E` and every
/// column is an offset from it. Two of these were two pixels out until the
/// listing above was written from the decompilation.
const COL_A_BEFORE: i32 = 0x3E - 6; // 56
const COL_A_AFTER: i32 = 0x3E + 0x28; // 102
const COL_A_ICON: i32 = 0x3E + 0x55; // 147
const COL_NOUN: i32 = 0x3E + 0x7D; // 187, and this module had 185
const COL_B_ICON: i32 = 0x3E + 0xF5; // 307
const COL_B_AFTER: i32 = 0x3E + 0x118; // 342
const COL_B_BEFORE: i32 = 0x3E + 0x146; // 388, and this module had 390
const TROOP_ICON_FRAME0: usize = 0x2F;
const TOTAL_Y: i32 = 404;
const TOTAL_A_X: i32 = 82;
const TOTAL_B_X: i32 = 282;

/// The two widgets of `DAT_004DDBB0`, **box-relative**
/// holds them: `(x, y, System.pl8 frame, side)`.
pub const TAKE_THE_FIELD: (i32, i32, usize, i32) = (BOX_X + 300, BOX_Y + 68, 29, 32);
pub const DECLINE: (i32, i32, usize, i32) = (BOX_X + 340, BOX_Y + 68, 31, 32);

pub const OK: (i32, i32) = (400, 400);

pub fn widget_rect(w: (i32, i32, usize, i32)) -> Rect {
    Rect::new(w.0, w.1, w.3, w.3)
}

pub fn ok_rect() -> Rect {
    Rect::new(OK.0, OK.1, system::OK_DIM, system::OK_DIM)
}

fn pen<'a>(ctx: &'a Ctx) -> Pen<'a> {
    Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    }
}

