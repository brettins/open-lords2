//! Screen `0x1F` — game setup, and the thirteen sub-pages `g_setupPage`
//! selects.
//!
//! # This is the front end
//!
//! `docs/screens-county.md` §1 called `0x1C` *"the front end"*. It is not, and
//! [`super::conquest`] says what it is. The screen a player of the
//! original meets first is **this one, page 1**: `Ui_DrawCentred(11, 0, …)` is
//! *"Lords of the Realm 2"* over `gateway.pl8`, with *"The siege is on"* under
//! it and four items in `panels2.pl8` recesses.
//!
//! # The thirteen pages
//!
//! **[D]** `FUN_0041E7E1` (`0x0041E7E1`) is one `if`/`else if` chain on
//! `g_setupPage` (`0x005530F0`) with thirteen arms, and `FUN_0041E61D`
//! (`0x0041E61D`) is the background loader in front of it. Together they are
//! the whole page table:
//!
//! | page | painter | background | what it is |
//! |---:|---|---|---|
//! | 1 | `0x0041EA14` | `gateway` + `panels2` | the title menu — group 11.0/11.1, items 11.2, 11.3, 11.47, 11.4 |
//! | 2 | `0x0041EC8A` | `gateway` + `panels2` | *"Your options"* — items 11.6, 11.7, 11.19, 11.8, 11.9 |
//! | 3 | `0x0041EF42` → `FUN_004148E4(5)` | `gateway` + `panels2` | load a game — group 40.5 |
//! | 4 | `0x0041EF57` | `gateway` + `panels2` | *"Choose your title and your shield."* — 11.10 |
//! | 5 | `0x0041F592` | `gateway` + `panels2` | original campaign or the new one — group 39.0, 39.4, 39.5 |
//! | 6 | `0x0041F3E9` | `gateway` + `panels2` | full game or skirmish — group 39.0, 39.1, 39.2 |
//! | 7 | `0x0041F6C7` | `custom` | the custom game, single player |
//! | 8 | `0x0041F77A` | `custom` | the custom game, multiplayer |
//! | 9 | `0x0041FDD6` | *the page underneath* | one open drop-down over page 7, 8, 11 or 12 |
//! | 10 | `0x00420428` | `gateway` + `panels2` | *"No Lords of the Realm CD"* — 11.16, 11.17, 11.18, 11.48, 11.49 |
//! | 11 | `0x00420630` | `skirmish` / `skircust` | skirmish setup, multiplayer |
//! | 12 | `0x0042051C` | `skirmish` / `skircust` | skirmish setup, single player |
//! | 13 | `0x0042150B` | `skirmish` / `skircust` | the skirmish file box, over page 12 |
//!
//! **[V]** for every row that names an `L2.eng` group; the string reads as what
//! the page is.
//!
//! # Which pages a single player can reach
//!
//! **[V]**, from every write of `g_setupPage` in the corpus, and it changes how
//! two rows above should be read:
//!
//! * **Page 6 is multiplayer-only.** The two writers are the `g_multiplayer !=
//! 0` arm of page 4's *Continue* handler and the net message handler
//!   `FUN_00445F80`. A single player leaving page 4 goes straight to page 7
//!   (custom) or page 12 (skirmish). So *"full game or skirmish"* is a question
//!   put to a **host**, and its `g_netIsMaster == 0` arm — a smaller window and
//!   the wrapped 39.3 *"Please wait while the session creator decides what type
//!   of game to play."*, with no buttons at all — is what a **joiner** sees.
//!   `g_netIsMaster` (`0x00553248`) is in `.bss`, so it is 0 until DirectPlay
//!   writes it. Nothing in single player ever does.
//! * **Page 8 is multiplayer-only** for the same reason, and pages 11 and 13
//!   are only reached with `DAT_0055302C == 3` (the skirmish choice).
//! * `FUN_0041E7E1`'s ladder has thirteen arms and
//!   none of them is 0, and so does `Screen_DrawWidgets`'s. `g_setupPage` 0 is
//!   the `.bss` initial value and every writer sets 1..=13.
//! * **Pages 9 and 10 have a `Screen_Draw` arm and no `Screen_DrawWidgets`
//!   arm, and both are reachable.** That combination is what made screen `0x28`
//! suspicious, and here it is benign: an open drop-down and the no-CD notice
//!   are both static, so there is nothing for the per-frame pass to repaint.
//!   Page 9 is written by `FUN_00432xxx`'s drop-down opener (`DAT_00553E5C =
//!   g_setupPage; g_setupPage = 9`) and page 10 by two arms of the page-1
//!   handler.
//!
//! # The painter, address by address
//!
//! Two ladders, and **both run**: `FUN_0041E7E1` from `Screen_Draw` on a
//! repaint, and `Screen_DrawWidgets`'s own `0x1F` arm every frame. Coordinates
//! resolved to decimal in the trailing comment.
//!
//! ```text
//! FUN_0041E61D()                                            0x0041E61D
//!   gateway.256 + gateway.pl8 + panels2.pl8   pages 1..6 and 10
//!   custom.256 + custom.pl8                   pages 7..9
//!   skirmish.256 + skirmish.pl8 / skircust.pl8  pages 11..13 on DAT_0056899C
//!
//! page 1  FUN_0041EA14()                                    0x0041EA14
//!   FUN_00409346(panels2, 0xA0, 10, 0x14, 0x0F)  window (160,10) 320x240
//!   Ui_DrawCentred(11, 0, 0x80, 0x1E, 0x180, heading)  "Lords of the Realm 2"
//!   Ui_DrawCentred(11, 1, 0x80, 0x3A, 0x180, body)     "The siege is on"
//!   FUN_0041EAA3()  — and again every frame from Screen_DrawWidgets:
//!     4 x  FUN_00403EE4(0xE0, 0x5B + 0x24 n, 0xC0, 0x18)  recess (224, 91+36n)
//!     4 x  Ui_DrawCentred(11, [2,3,47,4], 0xE0, +5, 0xC0, body, sel?0xF9:0x3F)
//!
//! page 2  FUN_0041EC8A()                                    0x0041EC8A
//!   FUN_00409346(panels2, 0xB0, 10, 0x12, 0x12)  window (176,10) 288x288
//!   Ui_DrawCentred(11, 5, 0xB0, 0x2D, 0x120, heading)  "Your options"
//!   FUN_0041ECE6()  — five of the same rows, indices 6, 7, 19, 8, 9
//!
//! page 3  FUN_0041EF42() -> FUN_004148E4(5)                 0x004148E4
//!   FUN_00409346(panels2, 0x60, 10, 0x1C, 0x15)  window (96,10) 448x336
//!   Ui_DrawCentred(40, 5, 0x60, 0x22, 0x1C0, heading)  "Loading a game."
//!   FUN_00403EE4(0x70, 0x42, 400, 0x100)         recess (112, 66) 400x256
//!   FUN_00403CF4(0x78, 0x4A, 0xC0, 0x20, 0x3F)   outline (120, 74) 192x32
//!   FUN_00403CF4(0x78, 0x72, 0x160, 0xA4, 0x3F)  outline (120,114) 352x164
//!   FUN_00403CF4(0x78, 0x11E, 0x180, 0x1C, 0x3F) outline (120,286) 384x28
//!   SaveLoad_DrawStatus()   — and again every frame; see below
//!
//! page 4  FUN_0041EF57()                                    0x0041EF57
//!   FUN_00409346(panels2, 0x50, 10, 0x1E, 0x10)  window (80,10) 480x256
//!                                  ... 0x0E rows for a net client
//!   Ui_DrawCentred(11, 10, 0x50, 0x23, 0x1E0, heading)
//!   FUN_0041F321()   the name plate:  panels2 frame 0xCC at (208, 72)
//!                    Ui_DrawText(g_options, 0xD6, 0x50)     (214, 80)
//!                    FUN_0040ACCE(caret) at (0xD6 + pen, 0x52)
//!   FUN_0041F1DD()   for i in 1..6, x = 0x70 + 0x58(i-1):
//!                      free:   panels2 frame 2i + 0xCB at (x, 0x8C)
//!                      chosen: panels2 frame 0xD7 at (x + 4, 0x70), then free
//!                      taken:  panels2 frame 2i + 0xCC at (x, 0x8C)
//!   FUN_0041F01F()   two 192x24 recesses at (112, 215) and (336, 215),
//!                    captions 11.9 "Back" and 11.11 "Continue"
//!
//! page 5  FUN_0041F592()                                    0x0041F592
//!   FUN_00409346(panels2, 0x40, 0x32, 0x20, 8)   window (64,50) 512x128
//!   Ui_DrawCentred(39, 0, 0x40, 0x50, 0x200, heading)
//!   FUN_0041F5E8()   FUN_00403EE4(0x6E, 0x74, 0xA4, 0x18)  (110,116) 164x24
//!                    Ui_DrawCentred(39, 4, 0x70, 0x7A, 0xA0)
//!                    the same again at x = 0x16E with index 5
//!
//! page 6  FUN_0041F3E9()                                    0x0041F3E9
//!   master: FUN_00409346(panels2, 0x40, 0x32, 0x20, 8) + Ui_DrawCentred(39,0)
//!           FUN_0041F4A1()  the same two recesses, indices 1 and 2
//!   client: FUN_00409346(panels2, 0x80, 0x32, 0x18, 6)  (128,50) 384x96
//!           FUN_0040328E(39, 3, 0xA0, 0x50, 0x140, 100, …)  wrapped, no buttons
//!
//! page 7  FUN_0041F6C7(999)                                 0x0041F6C7
//!   Pl8_DrawFrame(misc_sel, 0x0F, 0xA0, 0)       the top banner  (160, 0)
//!   3 x Ui_DrawCentred(11, [12,13,14], [0xA5,0xF3,0x141], 0xC6, 0x4C)
//!   ScenarioList_Draw()      0x0041F98B, gated on DAT_0057C948 > 0
//!     Pl8_DrawFrame(misc_sel, 0x10, 0x1F0, 9)    the list plate  (496, 9)
//!     FUN_00410C71(0, 0x1F0, 9)                  minimap at      (494, 12)
//!     5 x FUN_0040437D(0x1F0, 0x8D + 16n, 0x69, 0x10, sel?0x3F:0x20)
//!     5 x Eng_DrawString(101, DAT_0050A460[scroll+n], 0x1F2, 0x8E + 16n)
//!     3 x FUN_0040437D(0x25C, 0xA3 + …, 0x14, …)  the scroll bar (604,163)
//!   FUN_0041FBCB()           0x0041FBCB — Realms_AssignLords(), then per lord
//!     Pl8_DrawFrame(misc_sel, 2 * shieldIndex - 2, 10, 0x5E n + 6)
//!     Pl8_DrawFrame(misc_sel, lord + 9 (14 if human), 0x54, 0x5E n + 10)
//!     FUN_004025D7(g_playerNames[r], 0, 0x5E n + 0x50, 0xA0, body, realm+0x8)
//!     FUN_004B13ED(8, 0x5E n + 6, 0x91, 0x5A)     the "not ready" veil
//!   FUN_0041F86D(999)        0x0041F86D, gated on DAT_0055CE94 > 0
//!     12 x FUN_0040328E(102, i, x, labelY, 100, 100, …)   the label, wrapped
//!     12 x FUN_004093E0(x, boxY, 6, 3)                    the value box, 96x48
//!     12 x Ui_DrawCentred(103, base[i] + value, x + 1, boxY + 0x10, 0x60, 0xF9)
//!     ... the option numbered by the argument is skipped; 999 skips none
//!   FUN_0041FF75(0)          the eight-line chat log at (168, 8), 303x133
//!   FUN_00420147(0)          the chat input line at (168, 150), 303x22
//!     ... both open `if (g_multiplayer != 0)`, so both draw nothing on page 7
//!
//! page 8  FUN_0041F77A(999)                                 0x0041F77A
//!   page 7 without the banner; 11.12 always, 11.13/14/15 only for a host
//!
//! page 9  FUN_0041FDD6()                                    0x0041FDD6
//!   the page underneath first (DAT_00553E5C: 7, 8, 11 or 12), then
//!   FUN_004093E0(x, y, 6, rows + 2)   from 0x004D3158, rows = DAT_00553FB4
//!   rows x Ui_DrawCentred(103, base + n, x + 1, y + 0x10 + 16n, 0x60)
//!   ... option 2 only: y -= (rows - 1) * 0x10, so Nobles opens upward
//!
//! page 10 FUN_00420428()                                    0x00420428
//!   FUN_00409346(panels2, 0x50, 10, 0x1E, 0x13)  window (80,10) 480x304
//!   Ui_DrawCentred(11, 16, 0x50, 0x24, 0x1E0, heading)
//!   FUN_0040328E(11, 17, 0x80, 0x48, 0x180, …)   wrapped (128, 72)
//!   FUN_0040328E(11, 18, 0x80, 0x78, 0x180, …)   wrapped (128, 120)
//!   Eng_DrawString(11, 48, 0x80, 0xDC, body, **1**)  "Siege Pack" (128, 220)
//!   FUN_0040328E(11, 49, 0x80, 0xF0, 0x180, …)   wrapped (128, 240)
//!
//! page 11 FUN_00420630()  page 12 FUN_0042051C()   0x00420630 / 0x0042051C
//!   Ui_DrawCentred(11, 37, 0x1CD, 0x1B8, 0x38)   "Back"   (461, 440)
//!   Ui_DrawCentred(11, 38 or **39**, 0x207, 0x1B8, 0x38)  "Cust." / "Norm."
//!                                                 on DAT_0056899C  (519, 440)
//!   Ui_DrawCentred(11, 36, 0x241, 0x1B8, 0x38)   "Go"     (577, 440)
//!   page 11 also: Pl8_DrawFrame(misc_sel, 0, 0, 0)
//!   both then: FUN_004207C3 (8), FUN_00420D40 (1, a 12-frame animation on a
//!   100 ms timer at misc_sel 0x21 + n), and either FUN_00421231 + FUN_004209C1
//!   (23) or FUN_0042130F + FUN_00420DE4 (19), on DAT_0056899C
//!   Widget_Draw(0, 0, &DAT_004DE000, DAT_0056D5C0) only when DAT_0056899C == 1
//!
//! page 13 FUN_0042150B()                                    0x0042150B
//!   the three captions again, this time 36 / 37 / 38 left to right
//!   FUN_00421231(), FUN_004207C3()
//!   Ui_DrawBox(0x60, 100, 0x1C, 0x12)   border set **0** — (96,100) 448x288
//!   Ui_DrawCentred(40, 6, 0x60, 0x84, 0x1C0, heading)
//!   FUN_00403CF4(0x78, 0xAC, 0x160, 0xA5, 0x3F)   outline (120,172) 352x165
//!   Ui_DrawCentred(40, 8, 0x60, 0x164, 0x1C0, body)  "Right click to exit."
//!   FUN_00414E06(999)   Ui_DrawBoxInterior(0x7E, 0xAE, 0x15, 0xB)
//!                       FUN_00403CF4(0x78, 0xAC, 0x160, 0xA5, 0x3F) again
//!                       up to 10 x Ui_DrawText(name, 0x80, 0xB0 + 16n)
//!   Ui_OkButton(0x1F8, 0x164, 0)                            (504, 356)
//!
//! SaveLoad_DrawStatus()                                     0x004149EC
//!   ... two arguments Ghidra did not detect: the selected row, and a flag
//!   that is 1 on setup page 3 and 0 on screens 0x35 / 0x36. The flag picks
//!   both the origin — (0x60, 10) here, (0x10, 0x90) there — and whether the
//!   four plates come from panels2 (Sprite_GenBlank) or from the campaign
//!   Panels.pl8 (Ui_DrawBoxInterior). It is the same evidence as the two
//!   Widget_Draw offsets in Screen_DrawWidgets.
//!   plate (124, 76) 160x16 ; plate (124, 86) ; plate (126, 116) 336x160
//!   Ui_DrawText(g_editBuffer, 0x80, 0x52)  the file name being typed
//!   FUN_0040ACCE(caret)
//!   up to 30 x Ui_DrawText(name, 0x80 + 0x78 col, 0x76 + 16 row)  3 columns
//!   FUN_004B414A(x - 2, y - 1, 0x3F)  the 6 x 16 selection bar
//!   plate (128, 290) 336x16
//!   Eng_DrawString(40, 2 / 3 / 4, 0x80, 0x124)  only while DAT_0057D3C4 != 0
//!
//! # What the scenario list draws when there are no scenarios
//!
//! **[V]**, and it is a real configuration: `FUN_0046A101` (`0x0046A101`)
//! builds the list by asking, for each of the sixty map slots, whether
//! `MAPnn.PL8` — `"map01.pl8" + (slot >> 2) * 0x10` — **opens**, in the working
//! directory, then on the hard disk, then on the CD. Slots whose file is there
//! are packed into `DAT_0050A460` and counted into `DAT_00554018`.
//!
//! So `ScenarioList_Draw` indexes group 101 *through* that table, and on a
//! shipped install — eleven of the fifteen files, `Map01`…`Map06` and
//! `MAP11`…`MAP15` — it holds slots 0…23 and 40…59, 44 of them. Row 24 shows
//! group 101 index **40**, not 24. **This module still indexes group 101
//! directly, so rows 24 and up name the wrong map on a real install.** The fix
//! needs the packed table, which [`Ctx`]'s assets already have the evidence for
//! (`Assets::minimap` returns `None` for a slot with no file), and it moves the
//! meaning of every row — so it is recorded here.
//!
//! With **no** `MAPnn.PL8` at all the table is sixty zeroes and the count is 0.
//! The painter still draws its five rows: five bars, and `Eng_DrawString(101,
//! 0, …)` five times — **five copies of *"England"***. The scroll bar divides
//! by zero-safe `PctOf`, which returns 0, so all three segments come out zero
//! and the thumb takes the whole 44-pixel track. Nothing says the list is
//! empty; it looks like a list of five Englands.
//!
//! # The custom game's twelve options close exactly
//!
//! **[V]**, and this is the arithmetic. Three tables in `.data` describe the
//! twelve drop-downs: `0x004D3098` gives each one `(x, boxY, labelY)`,
//! `0x004D3128` gives its base index into `L2.eng` group 103, and `0x004D3158`
//! gives `(x, y, rows)` for the open list, where `rows` is the item count plus
//! two — a border cell at each end.
//!
//! Take the counts out of that third table and lay them against the bases:
//! every option's run ends exactly where the next one's begins, and the twelve
//! runs cover group 103's 46 strings with **one** left over — index 4, *"one"*,
//! which nothing reaches because *Nobles* starts at *"two"*. Twelve labels in
//! group 102, twelve bases, twelve counts, 45 of 46 strings used, and no
//! remainder. None of that was chosen by us.
//!
//! # What this page does, and the one thing it still does not
//!
//! **The twelve options reach the game now.** This section used to say the
//! opposite. [`crate::setup`] is what each of them means —
//! a module is that the drop-downs do **not**
//! write `g_optDifficulty` and its neighbours: they write a separate block at
//! `0x0053F288`, and `Setup_CommitOptions` (`0x00499DC3`) turns those twelve
//! selections into the eleven values a game runs on, five of them through a
//! lookup table and one by arithmetic. [`SetupScreen::start`] runs the same two
//! steps in the same order the original's *Start* handler does.
//!
//! **The map is built now too** — `l2_scenario::newgame` is `Map_InitScenario`
//! — so the slot the list names is the world a custom game opens in. This
//! section used to say that was missing; it was, until C62.
//!
//! # There are two Start buttons and they are not the same button
//!
//! **This is what the campaign-start defect was**
//! the top of the file because reading the front end as *one* way of beginning
//! a game is what caused it.
//!
//! | | the arm | what it starts |
//! |---|---|---|
//! | page 7/8, *Start* | `FUN_004335F0` hotspot 2 | `Setup_CommitOptions` then `Setup_StartGame` — the **map list's** slot, with the twelve options |
//! | page 4, *Continue* | `FUN_00433155` hotspot 2 | `Campaign_LoadEntry` then `Setup_StartGame` — the **campaign table's** row, with the row's options — *and only if a campaign was chosen* |
//!
//! `DAT_0057D320` ([`SetupScreen::campaign`]) is what tells them apart, and
//! `FUN_00433155`'s other limb starts nothing at all: it walks on to page 7, 8,
//! 11 or 12. Reading that button as *Start* made *Play Now!* build the map
//! list's untouched slot 0, England, when the campaign's first map is slot 17,
//! Quaintville. `docs/decisions.md` C117.
//!
//! # One divergence still here, and it is the page graph
//!
//! `FUN_00432CC8` sends *Custom game* and *Skirmish!* to page **4** as well —
//! with `DAT_0055302C` at 2 or 3 — so in the original every route to a game
//! passes through the shield page and *Continue* is what leaves it. Ours sends
//! them straight to pages 7 and 12. That is **[not reproduced]** and recorded
//! it is the same reading of page 4 that hid the
//! campaign start, and closing it is the rest of `front-end-pages` in
//! `docs/arms.json`.

mod helpers;
pub use helpers::*;
mod constants;
pub use constants::*;
mod ui;
pub use ui::*;

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

/// `g_setupPage` (`0x005530F0`). The thirteen values `FUN_0041E7E1` switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SetupPage {
    /// 1 — the title menu. The first screen of the original.
    Title,
    /// 2 — *"Your options"*.
    Options,
    /// 3 — load a game.
    Load,
    /// 4 — *"Choose your title and your shield."*
    Shield,
    /// 5 — the original campaign, or the one the expansion adds.
    Campaign,
    /// 6 — full game, or skirmish.
    GameType,
    /// 7 — the custom game, single player.
    Custom,
    /// 8 — the custom game, multiplayer.
    CustomMulti,
    /// 9 — one option's drop-down, open over the page underneath.
    Dropdown,
    /// 10 — *"No Lords of the Realm CD"*.
    NoCd,
    /// 11 — skirmish setup, multiplayer.
    SkirmishMulti,
    /// 12 — skirmish setup, single player.
    Skirmish,
    /// 13 — the skirmish file box, over page 12.
    SkirmishFile,
}

impl SetupPage {
    /// The value `g_setupPage` holds for this page.
    pub fn number(self) -> u8 {
        match self {
            SetupPage::Title => 1,
            SetupPage::Options => 2,
            SetupPage::Load => 3,
            SetupPage::Shield => 4,
            SetupPage::Campaign => 5,
            SetupPage::GameType => 6,
            SetupPage::Custom => 7,
            SetupPage::CustomMulti => 8,
            SetupPage::Dropdown => 9,
            SetupPage::NoCd => 10,
            SetupPage::SkirmishMulti => 11,
            SetupPage::Skirmish => 12,
            SetupPage::SkirmishFile => 13,
        }
    }

    /// All thirteen, in `g_setupPage` order.
    pub const ALL: [SetupPage; 13] = [
        SetupPage::Title,
        SetupPage::Options,
        SetupPage::Load,
        SetupPage::Shield,
        SetupPage::Campaign,
        SetupPage::GameType,
        SetupPage::Custom,
        SetupPage::CustomMulti,
        SetupPage::Dropdown,
        SetupPage::NoCd,
        SetupPage::SkirmishMulti,
        SetupPage::Skirmish,
        SetupPage::SkirmishFile,
    ];

    /// Which full-screen background `FUN_0041E61D` reads for this page.
    ///
    /// **[D]** The chain is: page 10 and pages below 7 take `gateway.256` and
    /// `gateway.pl8` with `panels2.pl8` on top; pages 7, 8 and 9 take
    /// `custom.256` and `custom.pl8`; pages 11 and up take `skirmish.256` and
    /// either `skirmish.pl8` or `skircust.pl8`. Page 9 draws the page beneath
    /// it first and so inherits whichever that was.
    pub fn background(self) -> &'static str {
        match self {
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => "Custom.pl8",
            SetupPage::SkirmishMulti | SetupPage::Skirmish | SetupPage::SkirmishFile => {
                "Skirmish.pl8"
            }
            _ => "Gateway.pl8",
        }
    }

    /// The `.256` that background comes with.
    pub fn palette(self) -> &'static str {
        match self {
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => "Custom.256",
            SetupPage::SkirmishMulti | SetupPage::Skirmish | SetupPage::SkirmishFile => {
                "Skirmish.256"
            }
            _ => "Gateway.256",
        }
    }
}

/// `L2.eng` group 11 — the front end's own strings, from index 0
/// *"Lords of the Realm 2"* to index 49.
pub const GROUP: usize = 11;
/// Group 39, the expansion pack's two choices.
/// `Eng_Seek(7, realm.lord)` — *"The Knight, The Baron, The Countess, The
/// Bishop, No player"*. Five strings, indexed by the lord.
pub const LORD_TITLE_GROUP: usize = 7;

pub const GROUP_EXPANSION: usize = 39;
/// Group 40, the load/save captions.
pub const GROUP_FILE: usize = 40;
/// Group 101, the sixty map names `g_scenarioIndex` indexes.
pub const GROUP_MAPS: usize = 101;
/// Group 102, the twelve custom-game option labels.
pub const GROUP_OPTIONS: usize = 102;
/// Group 103, their 46 values.
pub const GROUP_VALUES: usize = 103;

/// `DAT_00553F98`, the lobby's head count. One person, in this build.
const HUMAN_PLAYERS: usize = 1;

/// `panels2.pl8` — the box kit these pages draw their windows from. It has the
/// same frame layout as `Panels.pl8` (`docs/screens-county.md` §4.1).
const BOX_SHEET: &str = "Panels2.pl8";
/// `misc_sel.pl8` — `g_miscCtySheet` while the setup screen is up.
const ICON_SHEET: &str = "Misc_sel.pl8";

// --------------------------------------------------------------- the screen

pub struct SetupScreen {
    page: SetupPage,
    /// Which item the pointer or the keyboard is on — `DAT_00553114`, the
    /// global the painters compare against to pick `0xF9` over `0x3F`.
    selected: usize,
    /// Which page the drop-down is open over: `DAT_00553E5C`.
    under: SetupPage,
    /// Which of the twelve options is open: `DAT_0055306C`.
    open: usize,
    /// **The twelve settings, and they now reach the game.**
    ///
    /// This used to be a bare `[usize; 12]` with a comment saying nothing read
    /// it. It is [`SetupOptions`] — `0x0053F288 … 0x0053F2B4`, the block
    /// `Setup_SetOption` writes — and pressing *Start* puts it through
    /// [`SetupOptions::commit`] and applies the result. `crate::setup` is what
    /// each of the twelve does.
    options: SetupOptions,
    /// Which of the five shields page 4 has picked. Realm `+0x0A` in the
    /// original, one-based there and zero-based here because this is an index
    /// into the five frame pairs and nothing else yet.
    shield: usize,
    /// The top row of the map list, and the selected map — `g_scenarioIndex`,
    /// which *is* the map slot (`Game::map_slot`).
    ///
    /// **Zero is a real value here and it is where the list sits untouched**,
    /// which is England. A campaign does not read this: `Campaign_LoadEntry`
    /// *writes* it, from the campaign row, the same way it writes the global —
/// so [`SetupScreen::start_campaign`] sets it
    /// slot beside the world it just built.
    map_top: usize,
    map: usize,
    /// `g_playerStartCount` for [`SetupScreen::map`] — how many lords that map
    /// seats. `Map_LoadPlanes` recomputes it every time the scenario changes
    /// and three call sites then push it into the *Nobles* drop-down, so it is
/// cached beside the map.
    ///
    /// **Five until a map has been read**, which is what an install without
    /// `L2_maps.dat` leaves it at: the full drop-down, and no seat count
    /// invented from nothing.
    player_starts: usize,
    /// Whether [`SetupScreen::player_starts`] has been read for
    /// [`SetupScreen::map`] yet. The file is [`Ctx`]'s and the constructor has
    /// no `Ctx`, so the first read happens on the first tick.
    map_read: bool,
    /// What the last *Start* could not honour, as `L2.eng` group 102 indices —
    /// [`crate::setup::Settings::unhonoured`]. Drawn under the grid, in our own
    /// font. `docs/decisions.md` C21.
    unhonoured: Vec<usize>,
    /// **Why the last *Start* did not start**, if it did not.
    ///
    /// A slot that will not build a world — no `L2_maps.dat`, an empty
    /// template, more lords than seats — leaves the game untouched and says so
    /// under the grid. Silence would look exactly like a button that works.
    failure: Option<String>,
    /// **The lord's name, being typed.** `g_editBuffer` while page 4 is up.
    ///
    /// A player reported *"I can't type my name in the start menu?"*, and this
    /// is the field they were looking for. Both arms that open page 4 —
    /// `FUN_00432B05`'s hotspot 2 and `FUN_00432CC8`'s hotspots 3 and 5 — run
    /// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` before anything else, so the
    /// field is seeded with the name you already have, sixteen characters, one
    /// hundred and ninety-two pixels, free text. [`crate::text`] is the engine
    /// and `docs/arms.json`'s `text` group is the inventory.
    ///
/// It lives here because our page 4 is
    /// reachable from three places and a field rebuilt on each of them would
    /// lose what was typed; [`SetupScreen::go`] does the `Edit_Begin` at
    /// exactly the moments the original does.
    name: crate::text::TextField,
    /// **`DAT_0057D320` — whether *Continue* on page 4 starts a campaign.**
    ///
    /// Page 4 is reached from four places and the button at the bottom of it
    /// means something different depending on which. This is the flag the
    /// original uses to tell them apart, and it is written by every one of
    /// those arms: `Setup_ChooseCampaign` (`0x00433461`) sets it to **1**, and
    /// `FUN_00432B05` (page 1) and three arms of `FUN_00432CC8` (page 2) set it
    /// to **0**. [`SetupScreen::continue_pressed`] is what reads it.
    ///
    /// Without it, our page 4 started the map list's slot whichever way the
    /// person had arrived — so *Play Now!* built England instead of
    /// Quaintville. `docs/decisions.md` C117.
    campaign: bool,
    /// **A `Save_RotateAndWrite` is owed** — `Game_NewGame`'s own call to it
    /// (`0x00497E2B`), raised when *Start* has built a world. See
    /// [`crate::screen::Screen::take_autosave`].
    autosave: bool,
    /// `g_campaignTrack` (`DAT_0053F640`) — which of the two campaigns page 5
    /// chose. `Setup_ChooseCampaign` stores the hotspot here.
    track: crate::victory::Track,
    /// The persisted `g_options` name — what the field is seeded *from*, and
    /// where a commit goes back to.
    ///
    /// `g_options` is one 0x468-byte block the original `fread`s and `fwrite`s
    /// whole, and byte 0 begins a 31-byte name. We have no settings file yet,
    /// so this is that byte run and nothing else, defaulted the way
    /// `Options_SetDefaults` defaults it.
    saved_name: String,
    /// **Which minute the drawn clock says** — [`crate::wallclock::minute`] of
    /// the reading the last tick saw, or `None` before the first.
    ///
    /// A still screen costs nothing here: [`Machine::update`] only repaints
    /// when [`Screen::take_redraw`] says so, and this is what makes it say so
/// **once a minute**. It is a cached
    /// *picture* fact, not a clock — the reading itself is handed in through
    /// `Assets` and is never read from the system by anything in this crate.
    clock_minute: Option<i64>,
    /// Whether the minute turned since the last paint.
    clock_redraw: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_thirteen_pages_and_each_knows_its_number() {
        assert_eq!(SetupPage::ALL.len(), 13);
        for (i, p) in SetupPage::ALL.iter().enumerate() {
            assert_eq!(p.number() as usize, i + 1);
        }
    }

    #[test]
    fn the_twelve_option_runs_cover_group_103_with_one_string_left_over() {
        // The counts come out of the drop-down table as rows minus two.
        for i in 0..12 {
            assert_eq!(OPTION_COUNT[i], OPTION_LIST[i].2 as usize - 2, "option {i}");
        }
        // Every run ends where the next begins, except the gap at index 4.
        for i in 0..11 {
            let end = OPTION_BASE[i] + OPTION_COUNT[i];
            let next = OPTION_BASE[i + 1];
            assert!(end <= next, "option {i} runs past option {}", i + 1);
            assert!(next - end <= 1, "option {i} leaves {} strings unused", next - end);
        }
        let used: usize = OPTION_COUNT.iter().sum();
        assert_eq!(used, 45);
        assert_eq!(OPTION_BASE[11] + OPTION_COUNT[11], 46, "the last run ends at the group's end");
        // The one gap is "one", which Nobles skips: it starts at "two".
        let gaps: Vec<usize> =
            (0..11).filter(|&i| OPTION_BASE[i + 1] != OPTION_BASE[i] + OPTION_COUNT[i]).collect();
        assert_eq!(gaps, vec![1], "exactly one gap, after Exploration");
    }

    #[test]
    fn the_option_grid_is_four_columns_of_three() {
        let xs: Vec<i32> = OPTION_CELLS.iter().map(|c| c.0).collect();
        assert_eq!(xs, vec![170, 170, 170, 290, 290, 290, 410, 410, 410, 530, 530, 530]);
        // Every drop-down opens at its own box's x.
        for i in 0..12 {
            assert_eq!(OPTION_LIST[i].0, OPTION_CELLS[i].0, "option {i}");
        }
    }

    #[test]
    fn the_menu_items_step_thirty_six_apart_from_ninety_one() {
        assert_eq!(item_rect(0), Rect::new(0xE0, 0x5B, 0xC0, 0x18));
        assert_eq!(item_rect(1).y, 0x7F);
        assert_eq!(item_rect(2).y, 0xA3);
        assert_eq!(item_rect(3).y, 0xC7);
        assert_eq!(item_rect(4).y, 0xEB);
    }

    /// **Three rectangles that all look like a border, and page 3 drew the
    /// wrong one.** The load box's outer frame is `FUN_00403EE4`, a recess;
    /// the three inside it are `FUN_00403CF4`, flat outlines. The sizes are the
    /// painter's literal arguments.
    #[test]
    fn the_load_boxs_three_inner_rectangles_are_outlines_not_recesses() {
        assert_eq!(LOAD_OUTLINES[0], Rect::new(0x78, 0x4A, 0xC0, 0x20), "the name field");
        assert_eq!(LOAD_OUTLINES[1], Rect::new(0x78, 0x72, 0x160, 0xA4), "the file list");
        assert_eq!(LOAD_OUTLINES[2], Rect::new(0x78, 0x11E, 0x180, 0x1C), "the status line");
        // All three sit inside FUN_00403EE4(0x70, 0x42, 400, 0x100).
        let recess = Rect::new(0x70, 0x42, 400, 0x100);
        for r in LOAD_OUTLINES {
            assert!(r.x >= recess.x && r.y >= recess.y, "{r:?} starts outside the recess");
            assert!(r.x + r.w <= recess.x + recess.w, "{r:?} is wider than the recess");
        }
    }

    /// `ScenarioList_Draw`'s scroll bar is 44 pixels of track whatever the
    /// scroll position, because the thumb absorbs the rounding.
    ///
    /// **[V] **: `PctOf` returns 0
    /// when the total is 0, so a machine with no `MAPnn.PL8` gets a thumb the
    /// full length of the track and five rows of *"England"*.
    #[test]
    fn the_scroll_bar_is_always_forty_four_pixels() {
        let pct_of = |a: i32, b: i32| if b == 0 { 0 } else { a * 100 / b };
        let pct = |x: i32, p: i32| p * x / 100;
        for total in [0, 1, 5, 44, 60] {
            for top in 0..=total.max(0) {
                let above = pct(SCROLLBAR_H, pct_of(top, total));
                let below =
                    pct(SCROLLBAR_H, pct_of(total - top - MAP_LIST_ROWS as i32, total));
                let thumb = SCROLLBAR_H - above - below;
                assert_eq!(above + thumb + below, SCROLLBAR_H, "total {total} top {top}");
                assert!(thumb > 0, "the thumb vanished at total {total} top {top}");
            }
        }
        assert_eq!(SCROLLBAR_H, 0x2C);
    }

    /// The map thumbnail is drawn two pixels left and three down of the plate,
    /// which is `FUN_00410C71`'s offset everywhere it is called — the
    /// send-supplies panel and the diplomacy county picker use the same one.
    #[test]
    fn the_scenario_thumbnail_carries_the_same_offset_every_caller_of_that_helper_has() {
        assert_eq!(MAP_THUMB, (MAP_LIST_X - 2, 12));
        assert_eq!(MAP_LIST_X, 0x1F0);
    }

    #[test]
    fn every_page_names_a_background_and_a_palette_that_go_together() {
        for p in SetupPage::ALL {
            let (b, pal) = (p.background(), p.palette());
            assert_eq!(
                b.split('.').next(),
                pal.split('.').next(),
                "page {} loads {b} but sets {pal}",
                p.number()
            );
        }
    }
}

