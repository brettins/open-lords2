//! **Battle Master ratings** — `Screen_BattleMasterRatings` (`0x00421707`),
//! `g_screenId` `0x2E`.
//!
//! # `L2.eng` 37/1, *"Before"*, is drawn by nothing
//!
//! ```text
//! Screen_BattleMasterRatings():                                 0x00421707
//!   FUN_00498DCB()                    battle assets; g_miscCtySheet := misc_ske.PL8
//!   File_ReadChunk("score1.256", palette, 0x300, 0)
//!   FUN_00408FCB("score1.pl8", 0x1E0)     a raw 640 x 480 page, not a sheet
//!   Palette_Set(score1.256)
//!   Ui_OkButton(0x204, 0x186, 0)                                 (516, 390)
//!   Ui_DrawCentred(37, 0, 0x60, 0x44, 0x1BE, heading)   centred in x 96..542
//!   block A, the local player, at y 100:
//!     Pl8_DrawFrame(misc_ske, realm.shieldIndex + 8, 0x70, 100)     (112, 100)
//!     Ui_DrawText(name, 0xD8, 0x6E, body)                          (216, 110)
//!     Eng_DrawString(37, 4, pen + 0xD8, 0x6E, body)     "Scored"
//!     Ui_DrawNumber(score, ' ', " ", pen + 0xEC, 0x69, heading)    y 105
//!     Eng_DrawString(37, 2, 0x68, 200, body)            "Killed"   (104, 200)
//!     Eng_DrawString(37, 3, 0x68, 0xDC, body)           "Kills"    (104, 220)
//!     for c in 0..7:
//!       hue = after[c] == 0 ? 0x3F : 0x20
//!       Ui_DrawNumberRight(before[c],            ' ', " ", c*0x32+0xAD, 0x0B4, 0x3C, body, hue)
//!       Ui_DrawNumberRight(before[c] - after[c], ' ', " ", c*0x32+0xAD, 200,   0x3C, body, hue)
//!       Ui_DrawNumberRight(theirBefore[c] - theirAfter[c], …, 0x0DC, …, 0xF9)
//!   block B, the opponent: the same, +160 in y, with the two sides swapped
//! ```
//!
//! Block pitch is exactly 160 and row pitch exactly 20. The column x is
//! `c * 0x32 + 0xAD` in a **60-pixel box** at a 50-pixel pitch, so the boxes
//! overlap by ten — and `Ui_DrawNumberRight` **centres**
//! right-aligning (`FUN_004025D7` is `(width − textWidth) / 2`, the same helper
//! `Ui_DrawCentred` uses), which `docs/symbols.json` has wrong for every caller
//! in the binary.
//!
//! # The scoring rule — `FUN_0042C64F` (`0x0042C64F`), and it is documented nowhere
//!
//! Both scores are computed **once, at the end of the battle**, and this
//! painter only reads them. The weights are `g_troopStrengthWeight`
//! (`0x004D4B98`) — `2, 16, 8, 13, 9, 13, 22` — read out of the executable, in
//! the same troop order the seven columns are drawn in.
//!
//! ## The defensive-advantage handicap is computed and thrown away — **[D]**
//!
//! The two sum to 300, which is the shape of the `3 * survive` term they were
//! meant to replace. As decompiled, in every battle category except 0 — that is
//! every castle battle and every `.skr` scenario — the handicap has no effect
//! on either score. Marked `[D]`
//! dead-value elimination and not on the disassembly; one look at `0x0042C8xx`
//! would settle it. [`HANDICAP_IS_DEAD`] is the switch, and it is *off*,
//! because reproducing a dead computation is reproducing nothing.
//!
//! Seven sites write `g_screenId = 0x2E`, every one guarded by `DAT_0057A0F0`
//! — *this is a stand-alone battle, not a campaign one* — whose `== 0` sibling
//! goes to campaign screen `0x13` instead. So the ratings screen is the
//! skirmish's end and `screens/battle.rs` is the campaign's.
//!
//! # `0x2F`, the rank sheet — `Screen_BattleMasterRank` (`0x00421D09`)
//!
//! ```text
//! Screen_BattleMasterRank():                                    0x00421D09
//!   File_ReadChunk("score2.256", palette, 0x300, 0)
//!   FUN_00408FCB("score2.pl8", 0x1E0)      a raw 640 x 480 page, like score1
//!   Palette_Set(score2.256)
//!   DAT_0058FE2C := 1; DAT_005AEA40 := 1   drop capitals on, emboss OFF
//!   FUN_00403CF4(0x82, 10, 0x17C, 0x10E, 0x3F)   rectangle OUTLINE (130, 10)
//!                                                          380 x 270
//!   Ui_DrawCentred(37, 5, 0, 0x16, 0x280, heading)   "The skirmish masters!!"
//!                                                  centred across all 640, y 22
//!   DAT_0058FE2C := 0
//!   for row in 0..10:
//!     Ui_DrawText(DAT_0051FBC0 + row*0x20, 200, row*0x14 + 0x46, body)
//!     Ui_DrawNumber(DAT_0051FD00 + row*4, ' ', " ", 400, row*0x14 + 0x46, body)
//!   FUN_00403CF4(0x82, 300, 0x17C, 0xAA, 0x3F)   rectangle OUTLINE (130, 300)
//!                                                          380 x 170
//!   DAT_0058FE2C := 1
//!   Ui_DrawCentred(38, DAT_0053E9E0, 0, 0x138, 0x280, heading)  the player's rank
//!   DAT_0058FE2C := 0
//!   for row in 0..5:
//!     Ui_DrawText(g_playerNames[g_localPlayer], 200, row*0x14 + 0x168, body)
//!     Ui_DrawNumber(DAT_0051FD28 + row*4, ' ', " ", 400, row*0x14 + 0x168, body)
//!   DAT_005AEA40 := 0
//! ```
//!
//! So it is **a top-ten table and the local player's own last five scores**,
//! under `L2.eng` 37/5 *"The skirmish masters!!"* and a **group 38** heading —
//! twelve strings, *"Rank of Private"* through *"Rank of Supreme commander"* —
//! selected by `DAT_0053E9E0`. The ten names are a 32-byte-stride table at
//! `0x0051FBC0` and the scores four-byte tables at `0x0051FD00` and
//! `0x0051FD28`; `score.dat` is where they come from and nothing in this tree
//! reads it.
//!
//! ## The two `Screen_DrawWidgets` calls draw nothing — **[V]**
//!
//! `Screen_DrawWidgets`' `0x2F` arm is `FUN_004360F2(); FUN_0043F24B();`, the
//! *same two* as its `0x2E` arm, which made it look as though the rank sheet's
//! real content lived in them. It does not. `FUN_004360F2` (41 bytes) clears
//! `DAT_0057A0CC` and `DAT_005CD41C` and sets `g_redrawRequest = 2`;
//! `FUN_0043F24B` (185 bytes) zeroes an 8 × 65 array at `0x005651E0` and two
//! 65-entry arrays. **Neither contains a single draw call.** They are the
//! skirmish's state teardown, re-run every frame for as long as either results
//! screen is up — which is also why both screens repaint continuously.
//!
//! `FUN_00403CF4(x, y, w, h, colour)` is a **rectangle outline** — four
//! `FUN_00403A8F` line draws — and it is not in the audit's list of 26 pixel
//! primitives, so the mechanical count of this painter (6) is two short. The
//! same blind spot costs `screens/siege.rs` eight, where the missing primitive
//! is `FUN_0040437D`, the filled rectangle.

mod score_part;
pub use score_part::*;
mod render;
pub use render::*;

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

/// `L2.eng` group 37.
pub const GROUP: usize = 37;
pub const HEADING: usize = 0;
pub const BEFORE: usize = 1;
pub const KILLED: usize = 2;
pub const KILLS: usize = 3;
pub const SCORED: usize = 4;

pub const BACKGROUND: &str = "Score1.pl8";
pub const PALETTE: &str = "Score1.256";

pub const OK: Rect = Rect::new(0x204, 0x186, 24, 24);
pub const HEADING_AT: (i32, i32, i32) = (0x60, 0x44, 0x1BE);

pub const BLOCK_Y: [i32; 2] = [100, 260];
pub const SHIELD_X: i32 = 0x70;
pub const SHIELD_FRAME0: usize = 8;
pub const NAME_AT: (i32, i32) = (0xD8, 10);
pub const SCORE_DX: i32 = 0xEC - 0xD8;
pub const SCORE_DY: i32 = 5;
pub const LABEL_X: i32 = 0x68;
pub const ROW_DY: [i32; 3] = [80, 100, 120];
pub const COL_X0: i32 = 0xAD;
pub const COL_PITCH: i32 = 0x32;
pub const COL_W: i32 = 0x3C;
pub const COLUMNS: usize = 7;

/// `g_troopStrengthWeight` (`0x004D4B98`), read out of `Lords2.exe`, in the
/// `TROOPS*.ENG` column order the seven rating columns are drawn in: peasant,
/// crossbowman, maceman, swordsman, pikeman, archer, knight.
pub const STRENGTH_WEIGHT: [i32; COLUMNS] = [2, 16, 8, 13, 9, 13, 22];

pub const KILL_SHARE_SCALE: i32 = 6;
pub const SURVIVAL_SCALE: i32 = 3;
pub const WIN_BONUS: i32 = 500;
pub const MAX_SCORE: i32 = KILL_SHARE_SCALE * 100 + SURVIVAL_SCALE * 100 + WIN_BONUS;

pub const HANDICAP_IS_DEAD: bool = true;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub troops: [i32; COLUMNS],
    pub men: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Withdrew { local: bool },
    CastleFell { local: bool },
    Fought,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratings {
    pub mine: (Snapshot, Snapshot),
    pub theirs: (Snapshot, Snapshot),
    pub ending: Ending,
    /// **The two blocks' realm ids**, `(g_localPlayer, DAT_0056D5CC)` — not
    /// shield indices, which is what this used to hold. Both globals index
    /// `g_playerNames` and `g_realms` the same way, so the shield is
    /// `g_realms[realm].shieldIndex` and the name `g_playerNames[realm]`.
    pub realms: (u8, u8),
}

pub struct RatingsScreen {
    ratings: Ratings,
}

impl RatingsScreen {
    pub fn new() -> RatingsScreen {
        RatingsScreen { ratings: Ratings::default() }
    }

    pub fn with(ratings: Ratings) -> RatingsScreen {
        RatingsScreen { ratings }
    }

    pub fn ratings(&self) -> &Ratings {
        &self.ratings
    }
}

impl Default for RatingsScreen {
    fn default() -> RatingsScreen {
        RatingsScreen::new()
    }
}

