//! Screen `0x1F` — game setup, and the thirteen sub-pages `g_setupPage`
//! selects.
//!
//! # This is the front end
//!
//! `docs/screens-county.md` §1 called `0x1C` *"the front end"*. It is not, and
//! [`super::conquest`] says what it actually is. The screen a player of the
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
//! # Which pages a single player can actually reach
//!
//! **[V]**, from every write of `g_setupPage` in the corpus, and it changes how
//! two rows above should be read:
//!
//! * **Page 6 is multiplayer-only.** The two writers are the `g_multiplayer !=
//!   0` arm of page 4's *Continue* handler and the net message handler
//!   `FUN_00445F80`. A single player leaving page 4 goes straight to page 7
//!   (custom) or page 12 (skirmish). So *"full game or skirmish"* is a question
//!   put to a **host**, and its `g_netIsMaster == 0` arm — a smaller window and
//!   the wrapped 39.3 *"Please wait while the session creator decides what type
//!   of game to play."*, with no buttons at all — is what a **joiner** sees.
//!   `g_netIsMaster` (`0x00553248`) is in `.bss`, so it is 0 until DirectPlay
//!   writes it. Nothing in single player ever does.
//! * **Page 8 is multiplayer-only** for the same reason, and pages 11 and 13
//!   are only reached with `DAT_0055302C == 3` (the skirmish choice).
//! * **Page 0 is not a page.** `FUN_0041E7E1`'s ladder has thirteen arms and
//!   none of them is 0, and so does `Screen_DrawWidgets`'s. `g_setupPage` 0 is
//!   the `.bss` initial value and every writer sets 1..=13.
//! * **Pages 9 and 10 have a `Screen_Draw` arm and no `Screen_DrawWidgets`
//!   arm, and both are reachable.** That combination is what made screen `0x28`
//!   suspicious, and here it is benign: an open drop-down and the no-CD notice
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
//! meaning of every row — so it is recorded here rather than guessed at.
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
//! opposite. [`crate::setup`] is what each of them means — and the reason it is
//! a module rather than twelve assignments is that the drop-downs do **not**
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
//! **This is what the campaign-start defect was**, and it is worth stating at
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
//! rather than quietly correct: it is the same reading of page 4 that hid the
//! campaign start, and closing it is the rest of `front-end-pages` in
//! `docs/arms.json`.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

/// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` — the name field's own three
/// arguments, in one place because three call sites open it.
fn begin_name(seed: &str) -> TextField {
    TextField::begin(seed, text::NAME_MAX_TYPED, text::NAME_MAX_PIXELS, text::Kind::Text)
}

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

// ------------------------------------------------------------ menu geometry

/// A menu item on pages 1, 2 and 4: `FUN_00403EE4(x, y, 192, 24)` with the
/// label centred in the same 192 pixels, four below the top.
pub const ITEM_W: i32 = 0xC0;
pub const ITEM_H: i32 = 0x18;
/// Pages 1 and 2 put every item at the same x.
pub const ITEM_X: i32 = 0xE0;
/// And step them 36 apart from y = 91: `0x5B, 0x7F, 0xA3, 0xC7, 0xEB`.
pub const ITEM_Y: i32 = 0x5B;
pub const ITEM_STEP: i32 = 0x24;
/// The label sits five pixels into the recess.
const ITEM_TEXT: i32 = 5;

pub fn item_rect(index: usize) -> Rect {
    Rect::new(ITEM_X, ITEM_Y + index as i32 * ITEM_STEP, ITEM_W, ITEM_H)
}

/// `FUN_00403CF4(x, y, w, h, colour)` — **a flat one-pixel rectangle in one
/// colour**, four `FUN_00403A8F` line draws and nothing else.
///
/// It is not [`shell::inset_rect`] (two-tone, `0x10`/`0x1F`) and it is not
/// [`shell::button_recess`] / `FUN_00403EE4` (two-tone the other way,
/// `0x35`/`0x28`). Three different rectangles that all look like a border, and
/// this module drew the wrong one of the three on pages 3 and 13 until the
/// painters were read side by side.
fn outline_rect(canvas: &mut Canvas, r: Rect, colour: u8) {
    canvas.fill_rect(r.x, r.y, r.w, 1, colour);
    canvas.fill_rect(r.x, r.y + r.h - 1, r.w, 1, colour);
    canvas.fill_rect(r.x, r.y, 1, r.h, colour);
    canvas.fill_rect(r.x + r.w - 1, r.y, 1, r.h, colour);
}

/// `FUN_004148E4`'s three `FUN_00403CF4` calls: the name field, the file list
/// and the status line, inside the one `FUN_00403EE4` recess at (112, 66).
pub const LOAD_OUTLINES: [Rect; 3] = [
    Rect::new(0x78, 0x4A, 0xC0, 0x20),
    Rect::new(0x78, 0x72, 0x160, 0xA4),
    Rect::new(0x78, 0x11E, 0x180, 0x1C),
];

/// Page 1's four items, as `L2.eng` group 11 indices — *"Single player"*,
/// *"Multiple players"*, *"Lords of Magic?"*, *"Exit game"*.
pub const TITLE_ITEMS: [usize; 4] = [2, 3, 47, 4];
/// Page 2's five — *"Play Now!"*, *"Load a game"*, *"Skirmish!"*,
/// *"Custom game"*, *"Back"*.
pub const OPTION_ITEMS: [usize; 5] = [6, 7, 19, 8, 9];

/// Page 4's two buttons, from `FUN_0041F01F`: *"Back"* at `(0x70, 0xD7)` and
/// *"Continue"* at `(0x150, 0xD7)`, both 192 × 24.
pub const SHIELD_BUTTONS: [(i32, i32, usize); 2] = [(0x70, 0xD7, 9), (0x150, 0xD7, 11)];

/// The five shields: `x = 0x70 + 0x58 i`, `y = 0x8C`, from `FUN_0041F1DD`.
/// `Pl8_DrawFrameHere(panels2, 0xCC, 0xD0, 0x48)` — the 224 x 32 recess the
/// name sits in, and `Ui_DrawText(&g_options, 0xD6, 0x50, …)` the text inside
/// it. Six pixels in and eight down from the plate's corner.
pub const NAME_PLATE_X: i32 = 0xD0;
pub const NAME_PLATE_Y: i32 = 0x48;
pub const NAME_X: i32 = 0xD6;
pub const NAME_Y: i32 = 0x50;

const SHIELD_X: i32 = 0x70;
const SHIELD_STEP: i32 = 0x58;
const SHIELD_Y: i32 = 0x8C;

/// Pages 5 and 6 share a geometry: `FUN_00403EE4(0x6E, 0x74, 0xA4, 0x18)` and
/// the same 164 × 24 again at `x = 0x16E`, with the label centred in 160
/// pixels from two inside each recess.
pub const PAIR_X: [i32; 2] = [0x6E, 0x16E];
const PAIR_TEXT_X: [i32; 2] = [0x70, 0x170];
pub const PAIR_Y: i32 = 0x74;
pub const PAIR_W: i32 = 0xA4;
const PAIR_TEXT_W: i32 = 0xA0;

// ------------------------------------------------- the custom game's tables

/// `0x004D3098` — twelve records of `(x, boxY, labelY)`, four columns of three.
///
/// The value box is `FUN_004093E0(x, boxY, 6, 3)`: a `Panels.pl8` box in border
/// set 1, 96 × 48 pixels, with the value centred in 96 from `x + 1` at
/// `boxY + 16`. The label is the wrapped group 102 string at `(x, labelY)`.
pub const OPTION_CELLS: [(i32, i32, i32); 12] = [
    (170, 280, 246),
    (170, 353, 337),
    (170, 426, 410),
    (290, 280, 246),
    (290, 353, 337),
    (290, 426, 410),
    (410, 280, 246),
    (410, 353, 337),
    (410, 426, 410),
    (530, 280, 246),
    (530, 353, 337),
    (530, 426, 410),
];

/// `0x004D3128` — each option's base index into `L2.eng` group 103.
pub const OPTION_BASE: [usize; 12] = [0, 2, 5, 9, 11, 15, 19, 25, 29, 34, 37, 44];

/// `0x004D3158` — the open drop-down: `(x, y, rows)`, `rows` being the item
/// count plus two. [`OPTION_COUNT`] is that minus two.
pub const OPTION_LIST: [(i32, i32, i32); 12] = [
    (170, 280, 4),
    (170, 353, 4),
    (170, 426, 6),
    (290, 280, 4),
    (290, 337, 6),
    (290, 378, 6),
    (410, 280, 8),
    (410, 337, 6),
    (410, 362, 7),
    (530, 280, 5),
    (530, 305, 9),
    (530, 426, 4),
];

/// How many values each option has: `OPTION_LIST[i].2 - 2`.
pub const OPTION_COUNT: [usize; 12] = [2, 2, 4, 2, 4, 4, 6, 4, 5, 3, 7, 2];

/// The value box is six cells wide and three tall.
const OPTION_BOX_W: i32 = 6 * 16;
const OPTION_BOX_H: i32 = 3 * 16;
/// And the label above it wraps at a hundred pixels.
const OPTION_LABEL_W: i32 = 100;

/// Page 7's three buttons — *"Cancel"*, *"Start"*, *"Defaults"* — centred in
/// 76 pixels at `y = 0xC6`. Page 8 adds *"Load"* at `x = 399`.
pub const CUSTOM_BUTTONS: [(i32, usize); 4] = [(0xA5, 12), (0xF3, 13), (0x141, 14), (399, 15)];
pub const CUSTOM_BUTTON_Y: i32 = 0xC6;
const CUSTOM_BUTTON_W: i32 = 0x4C;

/// The map list on the custom-game pages: `misc_sel.pl8` frame 0x10 at
/// `(0x1F0, 9)`, five rows of 16 pixels of group 101 from `(0x1F2, 0x8E)`, the
/// selected row filled 105 × 16 from `(0x1F0, 0x8D)`.
pub const MAP_LIST_X: i32 = 0x1F0;
const MAP_LIST_TEXT_X: i32 = 0x1F2;
pub const MAP_LIST_Y: i32 = 0x8D;
pub const MAP_LIST_ROW: i32 = 0x10;
pub const MAP_LIST_ROWS: usize = 5;
const MAP_LIST_W: i32 = 0x69;
/// Group 101 has sixty entries, one per map slot.
pub const MAP_COUNT: usize = 60;

/// `ScenarioList_Draw`'s scroll bar: three stacked fills at x = 604, from
/// y = 163, 20 wide, **44 pixels of track in total**.
///
/// The heights are `Pct(0x2C, PctOf(part, total))` for the three parts — above
/// the window, the window itself, below it — and the *thumb* is given whatever
/// the three roundings lost: `hb += 0x2C - ha - hb - hc`. A zero-height segment
/// is skipped rather than drawn one pixel tall.
pub const SCROLLBAR_X: i32 = 0x25C;
pub const SCROLLBAR_Y: i32 = 0xA3;
pub const SCROLLBAR_W: i32 = 0x14;
pub const SCROLLBAR_H: i32 = 0x2C;

/// `FUN_00410C71(0, 0x1F0, 9)` inside `ScenarioList_Draw` — the selected map's
/// 128 x 128 minimap, blitted at `(x - 2, y + 3)` like every other caller of
/// that helper.
pub const MAP_THUMB: (i32, i32) = (MAP_LIST_X - 2, 9 + 3);

/// `FUN_0042150B` → `FUN_00414E06`: the skirmish file list's outline, drawn
/// twice, and the corner close button.
pub const SKIRMISH_FILE_LIST: Rect = Rect::new(0x78, 0xAC, 0x160, 0xA5);
pub const SKIRMISH_FILE_OK: (i32, i32) = (0x1F8, 0x164);

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
    /// so [`SetupScreen::start_campaign`] sets it rather than leaving a stale
    /// slot beside the world it just built.
    map_top: usize,
    map: usize,
    /// `g_playerStartCount` for [`SetupScreen::map`] — how many lords that map
    /// seats. `Map_LoadPlanes` recomputes it every time the scenario changes
    /// and three call sites then push it into the *Nobles* drop-down, so it is
    /// cached beside the map rather than asked for at every hit test.
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
    /// It lives here rather than being made on entry because our page 4 is
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
}

impl SetupScreen {
    pub fn new(page: SetupPage) -> SetupScreen {
        SetupScreen {
            page,
            selected: 0,
            under: SetupPage::Custom,
            open: 0,
            options: SetupOptions::new(),
            shield: 0,
            map_top: 0,
            map: 0,
            player_starts: 5,
            map_read: false,
            unhonoured: Vec::new(),
            failure: None,
            campaign: false,
            track: crate::victory::Track::First,
            name: begin_name(text::DEFAULT_PLAYER_NAME),
            saved_name: text::DEFAULT_PLAYER_NAME.to_string(),
        }
    }

    /// What the name field holds, for a test or a caller that wants to know
    /// what *Start* would name the lord.
    pub fn name(&self) -> String {
        self.name.commit(text::PLAYER_NAME_LEN)
    }

    /// The field itself, for a test that wants to look at the caret.
    pub fn name_field(&self) -> &text::TextField {
        &self.name
    }

    /// The twelve selections, for a test or a caller that wants to know what
    /// the screen would start.
    pub fn options(&self) -> &SetupOptions {
        &self.options
    }

    /// The map slot the list has selected — `g_scenarioIndex`.
    pub fn map(&self) -> usize {
        self.map
    }

    /// How many lords the selected map seats.
    pub fn player_starts(&self) -> usize {
        self.player_starts
    }

    /// `Map_LoadPlanes`'s side effect on the option block: read the seat count
    /// off the chosen map and set *Nobles* from it.
    ///
    /// **Both halves, or neither.** A map whose planes cannot be read leaves
    /// the seat count alone rather than reporting zero seats, because zero
    /// would silently drive the lord count to two.
    fn read_map(&mut self, ctx: &Ctx) {
        self.map_read = true;
        let Some(slot) = ctx.assets.slot(self.map) else { return };
        let seats = slot.player_start_count();
        if seats == 0 {
            return;
        }
        self.player_starts = seats;
        self.options.set_nobles_from_map(seats);
    }

    pub fn page(&self) -> SetupPage {
        self.page
    }

    /// The value of option `i`, as an index into `L2.eng` group 103.
    pub fn option_value(&self, i: usize) -> usize {
        OPTION_BASE[i] + self.options.get(i)
    }

    /// How many rows option `i`'s open list shows.
    ///
    /// Everything but *Nobles* shows its whole run. *Nobles* is shortened to
    /// the map's seat count — `FUN_00433999`: `DAT_00553FB4 =
    /// g_playerStartCount - 1` when the map seats fewer than five — which is
    /// how the original stops a person asking for more lords than the map has
    /// castles for.
    fn rows(&self, i: usize) -> usize {
        if i == crate::setup::option::NOBLES {
            SetupOptions::nobles_rows_for_map(self.player_starts)
        } else {
            OPTION_COUNT[i]
        }
    }

    /// The clickable rectangles of the page, in the order the painter draws
    /// them. Hit-testing and highlighting read the same list, so they cannot
    /// disagree — the failure `menu.rs` avoids by the same means.
    fn hotspots(&self) -> Vec<(Rect, Action)> {
        let mut v = Vec::new();
        match self.page {
            SetupPage::Title => {
                for (i, _) in TITLE_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Options => {
                for (i, _) in OPTION_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Shield => {
                for i in 0..5 {
                    v.push((Rect::new(SHIELD_X + i * SHIELD_STEP, SHIELD_Y, 60, 65), {
                        Action::Item(i as usize)
                    }));
                }
                for (i, (x, y, _)) in SHIELD_BUTTONS.iter().enumerate() {
                    v.push((Rect::new(*x, *y, ITEM_W, ITEM_H), Action::Item(5 + i)));
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                for (i, x) in PAIR_X.iter().enumerate() {
                    v.push((Rect::new(*x, PAIR_Y, PAIR_W, ITEM_H), Action::Item(i)));
                }
            }
            SetupPage::Custom | SetupPage::CustomMulti => {
                for (i, &(x, y, _)) in OPTION_CELLS.iter().enumerate() {
                    v.push((Rect::new(x, y, OPTION_BOX_W, OPTION_BOX_H), Action::Open(i)));
                }
                for row in 0..MAP_LIST_ROWS {
                    let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
                    v.push((
                        Rect::new(MAP_LIST_X, y, MAP_LIST_W, MAP_LIST_ROW),
                        Action::Map(row),
                    ));
                }
                let n = if self.page == SetupPage::Custom { 3 } else { 4 };
                for (i, (x, _)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
                    // [I] The painter centres the caption in 76 pixels at
                    // y = 0xC6 and registers no rectangle of its own; the
                    // hit box is that caption's box, four pixels above it.
                    v.push((
                        Rect::new(*x, CUSTOM_BUTTON_Y - 4, CUSTOM_BUTTON_W, ITEM_H),
                        Action::Item(i),
                    ));
                }
            }
            SetupPage::Dropdown => {
                let n = self.rows(self.open);
                let (x, y, _) = OPTION_LIST[self.open];
                let y = self.dropdown_y(y, n);
                for i in 0..n {
                    v.push((
                        Rect::new(x, y + 16 + i as i32 * 16, OPTION_BOX_W, 16),
                        Action::Choose(i),
                    ));
                }
            }
            SetupPage::NoCd | SetupPage::Load | SetupPage::SkirmishFile => {
                // One way out, and the whole page is it.
                v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti => {
                for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
                    v.push((Rect::new(*x, 0x1B8 - 4, 0x38, ITEM_H), Action::Item(i)));
                }
            }
        }
        v
    }

    /// `FUN_0041FDD6` shifts the *Nobles* drop-down up by one row per item so
    /// that a list opened from the bottom row of the grid still fits on the
    /// screen. It is the only option that gets the treatment.
    ///
    /// **`rows` is the item count, `DAT_00553FB4`** — the same number the
    /// painter loops over, not the geometry table's count-plus-two. That
    /// matters now that the count can be shortened: on a map that seats three
    /// lords the list is two rows and rides two rows lower, exactly as the
    /// original's does, because both read the one variable.
    fn dropdown_y(&self, y: i32, rows: usize) -> i32 {
        if self.open == crate::setup::option::NOBLES {
            y - (rows as i32 - 1) * 16
        } else {
            y
        }
    }

    fn at(&self, x: i32, y: i32) -> Option<(usize, Action)> {
        self.hotspots()
            .into_iter()
            .enumerate()
            .find(|(_, (r, _))| r.contains(x, y))
            .map(|(i, (_, a))| (i, a))
    }

    fn count(&self) -> usize {
        self.hotspots().len()
    }

    fn activate(&mut self, ctx: &mut Ctx) -> Transition {
        let Some(&(_, action)) = self.hotspots().get(self.selected) else {
            return Transition::Stay;
        };
        self.act(action, ctx)
    }

    /// The page graph, read out of `FUN_00432B05` (page 1) and `FUN_00432CC8`
    /// (page 2).
    ///
    /// **[D], and one thing in it is unresolved.** Those handlers branch on
    /// `g_uiHotspotId`, and the ids do not run in the order the painter draws
    /// the items: on page 1 id 3 sets the quit flag and id 4 plays `lom.smk`,
    /// while the painter draws *"Lords of Magic?"* third and *"Exit game"*
    /// fourth. Either the widget table is not in drawing order or one of the
    /// two readings is wrong, and nothing in the decompilation settles it. The
    /// destinations below are keyed to the **captions**, which are [V], not to
    /// the ids.
    fn act(&mut self, action: Action, ctx: &mut Ctx) -> Transition {
        match action {
            Action::Item(i) => self.item(i, ctx),
            Action::Open(i) => {
                self.under = self.page;
                self.open = i;
                self.page = SetupPage::Dropdown;
                self.selected = self.options.get(i);
                Transition::Stay
            }
            Action::Choose(v) => {
                // `FUN_00433A23`: `Setup_SetOption(open, row - 1)`, then back to
                // the page underneath.
                self.options.set(self.open, v);
                self.page = self.under;
                self.selected = 0;
                Transition::Stay
            }
            Action::Map(row) => {
                // `FUN_00433905`: the row sets `g_scenarioIndex`, the planes are
                // loaded, and the seat count that comes out of them sets
                // *Nobles*. All three, or the lord count is left claiming a
                // number the new map cannot seat.
                self.map = (self.map_top + row).min(MAP_COUNT - 1);
                self.read_map(ctx);
                Transition::Stay
            }
        }
    }

    fn item(&mut self, i: usize, ctx: &mut Ctx) -> Transition {
        match (self.page, i) {
            // Page 1. "Single player" opens page 2; "Multiple players" opens
            // page 4 (or page 10 with no disc); "Lords of Magic?" plays an
            // advertisement we have no player for; "Exit game" quits.
            (SetupPage::Title, 0) => self.go(SetupPage::Options),
            (SetupPage::Title, 1) => {
                // `FUN_00432B05` opens with `DAT_0057D320 = 0` before any of
                // its four arms: arriving at page 4 from the title menu is
                // **not** a campaign, whatever the last visit set.
                //
                // arm: 0x00432B05/multiplayer-clears-campaign
                self.campaign = false;
                self.go(SetupPage::Shield)
            }
            (SetupPage::Title, 2) => self.go(SetupPage::NoCd),
            (SetupPage::Title, 3) => Transition::Quit,
            // Page 2.
            (SetupPage::Options, 0) => self.go(SetupPage::Campaign),
            (SetupPage::Options, 1) => self.go(SetupPage::Load),
            (SetupPage::Options, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Options, 3) => self.go(SetupPage::Custom),
            (SetupPage::Options, 4) => self.go(SetupPage::Title),
            // Page 4: five shields, then "Back" and "Continue".
            (SetupPage::Shield, 0..=4) => {
                self.shield = i;
                Transition::Stay
            }
            (SetupPage::Shield, 5) => self.go(SetupPage::Title),
            (SetupPage::Shield, 6) => self.continue_pressed(ctx),
            // Page 5: either campaign. Page 6: full game or skirmish.
            //
            // **`Setup_ChooseCampaign` (`0x00433461`)**, and it does three
            // things this used to do none of: it stores the hotspot in
            // `g_campaignTrack`, it starts `g_campaignMap` at that track's first
            // row — `Campaign::new` — and it raises `DAT_0057D320` so that
            // *Continue* on page 4 loads a campaign row instead of the map
            // list's slot. The counter is not kept here; it is
            // [`crate::victory::Track::first_map`], which is where it was
            // already read out of this function.
            //
            // arm: 0x00433461/choose-campaign
            (SetupPage::Campaign, i) => {
                self.track = if i == 1 {
                    crate::victory::Track::Second
                } else {
                    crate::victory::Track::First
                };
                self.campaign = true;
                self.go(SetupPage::Shield)
            }
            (SetupPage::GameType, 0) => self.go(SetupPage::Shield),
            (SetupPage::GameType, 1) => self.go(SetupPage::Skirmish),
            // Pages 7 and 8: "Cancel", "Start", "Defaults", "Load".
            (SetupPage::Custom | SetupPage::CustomMulti, 0) => self.go(SetupPage::Options),
            (SetupPage::Custom | SetupPage::CustomMulti, 1) => self.start(ctx),
            // *Defaults*: `Setup_DefaultOptions` (`0x004AE539`) and then
            // `FUN_004AE5E2(g_playerStartCount)`, which is the click handler's
            // own order — the twelve go back to the game's defaults and the map
            // then overrules *Nobles* again. **It is not twelve zeroes**, which
            // is what this used to write: six of the twelve defaults are not 0,
            // so the button was resetting to a game the original never offers.
            (SetupPage::Custom | SetupPage::CustomMulti, 2) => {
                self.options = SetupOptions::new();
                self.options.set_nobles_from_map(self.player_starts);
                self.unhonoured.clear();
                Transition::Stay
            }
            (SetupPage::Custom | SetupPage::CustomMulti, _) => self.go(SetupPage::Load),
            // Pages 11 and 12: "Back", "Cust."/"Norm.", "Go".
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 0) => self.go(SetupPage::Options),
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, _) => Transition::Stay,
            // The three pages with one way out.
            (SetupPage::NoCd, _) => self.go(SetupPage::Title),
            (SetupPage::Load, _) => self.go(SetupPage::Options),
            (SetupPage::SkirmishFile, _) => self.go(SetupPage::Skirmish),
            _ => Transition::Stay,
        }
    }

    /// **Every arrival at page 4 re-seeds the name field**, because every one
    /// of the original's does.
    ///
    /// `FUN_00432B05` (page 1 → 4, *Multiple players*), `FUN_00432CC8` (page 2
    /// → 4, both of its two arms) **and `Setup_ChooseCampaign` (page 5 → 4)**
    /// each set `g_setupPage = 4` and then immediately run
    /// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` and `Edit_RecomputeLength`.
    ///
    /// **Corrected: there is no exception, and this comment used to claim
    /// one.** It said "arriving from page 5 or 6 does not — those two arms set
    /// the page and nothing else", and `Setup_ChooseCampaign`'s third statement
    /// is that very `Edit_Begin` call. All four writers of `g_setupPage = 4`
    /// re-seed, which is what the code below has always done, so the code was
    /// right and the sentence describing it was not.
    /// `docs/decisions.md` C117.
    fn go(&mut self, page: SetupPage) -> Transition {
        if page == SetupPage::Shield && self.page != SetupPage::Shield {
            // arm: 0x00432B05/name-field-open
            self.name = begin_name(&self.saved_name);
        }
        self.page = page;
        self.selected = 0;
        Transition::Stay
    }

    /// ***Start*, and the twelve settings now go with it.**
    ///
    /// The original's own order is `FUN_004335F0`'s hotspot-2 arm:
    ///
    /// ```text
    /// if (humanPlayers <= g_playerStartCount) {
    ///     Setup_CommitOptions();      /* 0x00499DC3 - the twelve into the eleven */
    ///     Setup_StartGame();          /* 0x004329EC - which calls Game_NewGame  */
    /// }
    /// ```
    ///
    /// — and the guard is real: **pressing *Start* on a map that seats fewer
    /// lords than there are people does nothing at all.** No message, no
    /// refusal; the button is simply inert. Reproduced, because a person who
    /// meets it in the original meets a button that does not work and a
    /// reimplementation that helpfully explained itself would be a different
    /// program. In a single-player game there is one person and every shipped
    /// map seats at least two, so it never fires here.
    ///
    /// # The map is built now
    ///
    /// This section used to say the opposite, and it named exactly what was
    /// missing: *"building a world from a `L2_maps.dat` slot means
    /// `Map_InitScenario` … and none of that exists here"*. It does now —
    /// `l2_scenario::newgame` — so **the slot the list names is the world the
    /// game starts in**. Pick Ireland and you play Ireland.
    ///
    /// The three steps are `Game_NewGame`'s, in its order:
    ///
    /// 1. [`crate::scenario::new_game`] — `Map_InitScenario` and
    ///    `County_Reset`, which is the world;
    /// 2. [`crate::setup::Settings::apply_to`] — `FUN_0049BD99`'s option half:
    ///    the stores, the treasury, the armoury, the castle and the lord count;
    /// 3. `Kingdom::start_new_game` — the one immediate `Season_Advance` that
    ///    is why a new game begins in **Winter 1268**.
    ///
    /// **A world that cannot be built is not half-started.** An install with no
    /// `L2_maps.dat`, or a slot that is an empty template, leaves the game
    /// exactly as it was and says so under the grid in our own font
    /// (`docs/decisions.md` C21) rather than dropping the player onto a
    /// different map than the one they chose.
    fn start(&mut self, ctx: &mut Ctx) -> Transition {
        // One person, in this build. `DAT_00553F98` is the lobby's count and
        // there is no lobby.
        if !self.map_read {
            self.read_map(ctx);
        }
        if HUMAN_PLAYERS > self.player_starts {
            return Transition::Stay;
        }
        // **The quirk set is already on the game**, because the quirks page
        // writes it there whether or not a campaign is running - one home for
        // the value rather than a pending copy that could disagree with it.
        // See [`crate::screens::options`].
        let settings = self.options.commit(HUMAN_PLAYERS, ctx.game.kingdom.options.quirks);
        self.unhonoured = settings.unhonoured();
        let slot = self.map;
        self.new_game(ctx, slot, settings, None)
    }

    /// ***Continue*, at the bottom of page 4 — `FUN_00433155`'s hotspot-2
    /// arm.**
    ///
    /// This button is not the custom game's *Start* and it was being treated as
    /// though it were. The original's arm branches first:
    ///
    /// ```text
    /// else if (g_uiHotspotId == 2 && (g_multiplayer == 0 || DAT_0057C940 != 0)) {
    ///     if (DAT_0057D320 == 1) {        /* a campaign */
    ///         Campaign_LoadEntry();       /* 0x00499E5D — the row, not the list */
    ///         Setup_StartGame();          /* 0x004329EC */
    ///     } else {                        /* a custom game or a skirmish */
    ///         ...g_setupPage = 7 / 8 / 0xB / 0xC...
    ///     }
    /// }
    /// ```
    ///
    /// **Two things were wrong with reading it as *Start*.** The campaign limb
    /// never ran, so *Play Now!* started the map list's slot — slot 0, England,
    /// which is the campaign's **fifth** map — and the other limb started a
    /// game at all, where the original walks on to the page that chooses one.
    ///
    /// **There is no seat-count guard here.** `FUN_004335F0`'s
    /// `humanPlayers <= g_playerStartCount` test guards the *custom* Start and
    /// this arm has none, which is right: a campaign row's map and lord count
    /// come from the same table and cannot disagree.
    ///
    /// arm: 0x00433155/continue
    fn continue_pressed(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.campaign {
            // `DAT_0055302C` picks between page 7/8 and page 11/12 here. This
            // build has one person and no skirmish setup, so the one live
            // destination is the custom page.
            return self.go(SetupPage::Custom);
        }
        self.start_campaign(ctx)
    }

    /// **`Campaign_LoadEntry` (`0x00499E5D`) and then `Setup_StartGame`.**
    ///
    /// The map a campaign starts on is **not** the map list's slot and not
    /// slot 0: it is column `+0x00` of row `g_campaignMap` of the track's table,
    /// which for the original campaign's first map is slot **17,
    /// Quaintville** — four counties against one lord. `crate::victory` holds
    /// the table and [`crate::victory::CampaignMap::settings`] is the rest of
    /// `Campaign_LoadEntry`.
    ///
    /// The counter is `Campaign::new(track)`, which is
    /// [`crate::victory::Track::first_map`] — 0 for the first campaign and
    /// **2** for the second, exactly as `Setup_ChooseCampaign` sets it.
    ///
    /// **The campaign goes onto the new game, not the old one.** `new_game`
    /// returns a whole fresh [`crate::game::Game`], whose `campaign` field is a
    /// default `Campaign::new(Track::First)`; the track and counter chosen on
    /// page 5 have to be written over it or the second campaign would play the
    /// first campaign's maps from its second win onward. That is the same
    /// three-globals-survive rule `crate::victory`'s header states, arriving at
    /// the one moment the world is replaced.
    fn start_campaign(&mut self, ctx: &mut Ctx) -> Transition {
        let campaign = crate::victory::Campaign::new(self.track);
        let Some(row) = campaign.current() else {
            self.failure = Some("the campaign has no first map".into());
            return Transition::Stay;
        };
        let settings = row.settings(ctx.game.kingdom.options.quirks);
        self.unhonoured = settings.unhonoured();
        // `g_scenarioIndex` is one global, so the list follows the campaign's
        // choice rather than keeping a stale one beside it.
        self.map = row.scenario;
        self.map_read = false;
        self.new_game(ctx, row.scenario, settings, Some(campaign))
    }

    /// `Setup_StartGame` → `Game_NewGame`, shared by both of page 4's and
    /// page 7's routes into it, so that neither can drift from the other.
    fn new_game(
        &mut self,
        ctx: &mut Ctx,
        slot: usize,
        settings: crate::setup::Settings,
        campaign: Option<crate::victory::Campaign>,
    ) -> Transition {
        let tables = ctx.game.kingdom.tables;
        match crate::scenario::new_game(
            ctx.assets,
            slot,
            &settings,
            HUMAN_PLAYERS,
            crate::scenario::SEED,
            tables,
        ) {
            Ok(game) => {
                *ctx.game = game;
                self.failure = None;
            }
            Err(e) => {
                self.failure = Some(e.to_string());
                return Transition::Stay;
            }
        }
        if let Some(c) = campaign {
            ctx.game.campaign = c;
        }
        settings.apply_to(ctx.game);
        self.name_the_lords(ctx);
        // `Game_NewGame`'s last economic call. Everything above is the position
        // the original hands to it.
        ctx.game.last_report = Some(ctx.game.kingdom.start_new_game());
        Transition::Push(ScreenId::Campaign)
    }

    /// **Fill `g_playerNames`** — the one place the typed name stops being a
    /// keystroke and becomes part of the game.
    ///
    /// Two sources, and both are the original's:
    ///
    /// * the local player's is `Player_SetHuman` (`0x0049BAE9`), which is
    ///   `g_realms[p].isHuman = 1; g_playerNames[p] = g_options; …` — the
    ///   thirty-one bytes of the settings block's name field, copied by
    ///   `FUN_00401136(0x53F1E0, &g_playerNames + p * 0x2C, 0x1F)`;
    /// * every other realm's is `Eng_Seek(7, realm.lord)` and **sixteen** bytes
    ///   copied. `L2.eng` group 7 is *"The Knight, The Baron, The Countess, The
    ///   Bishop"* and index 4 is *"No player"*, and the index is the **lord**,
    ///   not the realm and not the colour — `docs/diplomacy.md` §0.1.
    ///
    /// **Sixteen, not thirty-one, for the AI half** — the copy width really is
    /// different between the two paths, and none of group 7's four titles is
    /// long enough for it to show.
    fn name_the_lords(&self, ctx: &mut Ctx) {
        let local = ctx.game.player as usize;
        for realm in 0..l2_kingdom::realm::MAX_REALMS {
            let name = if realm == local {
                // arm: 0x0049BAE9/name-to-playernames
                self.name.commit(text::PLAYER_NAME_LEN)
            } else {
                let lord = ctx.game.kingdom.realms[realm].lord as usize;
                let title = ctx.assets.shell.text(LORD_TITLE_GROUP, lord.min(4));
                title.chars().take(0x10).collect()
            };
            ctx.game.player_names[realm] = text::PlayerName::new(&name);
        }
    }
}

/// What a rectangle on one of these pages does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// The n'th menu item or button of this page.
    Item(usize),
    /// Open custom-game option n's drop-down.
    Open(usize),
    /// Choose value n from the open drop-down.
    Choose(usize),
    /// The n'th visible row of the map list.
    Map(usize),
}

impl Screen for SetupScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Setup(self.page)
    }

    fn title(&self, ctx: &Ctx) -> String {
        let name = ctx.assets.shell.text(GROUP, 0);
        if name.is_empty() {
            format!("Lords of the Realm II — setup page {}", self.page.number())
        } else {
            format!("{name} — setup page {}", self.page.number())
        }
    }

    fn palette(&self) -> Option<&'static str> {
        Some(self.page.palette())
    }

    /// `Map_LoadPlanes`'s effect on the option block, once, on the first tick.
    ///
    /// The original loads the planes the moment the custom page is opened and
    /// again on every change of scenario, and `g_playerStartCount` falls out of
    /// that load. The constructor has no [`Ctx`] and so no `L2_maps.dat`, so
    /// the first read waits for the first tick — which is also what makes a
    /// screen built in a test with no install work: it never gets a slot, and
    /// the seat count stays at its five.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.map_read {
            self.read_map(ctx);
        }
        // `Edit_DrawCaret` counts its own frames; ours counts ticks, because
        // nothing under the renderer may read a clock. `crate::text`.
        if self.page == SetupPage::Shield {
            self.name.tick();
        }
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let n = self.count().max(1);
        // **The name field gets first refusal on page 4, and only there.**
        //
        // `Screen_HandleInput`'s page-4 arm is the one that sets `g_editActive`
        // (`0x005AEB78`), and that flag is what decides whether a keystroke
        // reaches the buffer at all — so on every other page of the front end
        // the keys below keep the meaning they have here today. On page 4 they
        // do not: `Space` is a space in a name, and `I` is the letter I.
        //
        // The commit is `Edit_Commit(&g_options, 0x1F)`, which the original
        // runs **every frame** while the page is up rather than on a button.
        // Doing it per keystroke is the same thing at the only moments the
        // buffer can have changed.
        if self.page == SetupPage::Shield {
            // arm: 0x004BA9C8/setup-name
            let metrics = text::FontMetrics::of(&ctx.assets.shell);
            if self.name.event(event, &metrics) {
                self.saved_name = self.name.commit(text::PLAYER_NAME_LEN);
                return Transition::Stay;
            }
        }
        // **Everything below this line is ours.** The front end has no keyboard
        // at all in the original: not one of `Screen_HandleInput`'s thirteen
        // `g_setupPage` arms tests a key, and the window procedure has no
        // `g_screenId == 0x1F` case. Its whole interface is `Hotspot_Test` and
        // `Widget_Test`. That is recorded rather than removed — a menu a person
        // cannot drive from the keyboard is worse, not more faithful — and the
        // records are `ours/setup-*` in `docs/arms.json`.
        match event {
            // arm: ours/setup-key-up
            Event::KeyDown(Key::Up) => self.selected = (self.selected + n - 1) % n,
            // arm: ours/setup-key-down
            Event::KeyDown(Key::Down) => self.selected = (self.selected + 1) % n,
            // arm: ours/setup-key-activate
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(ctx),
            // Ours: the demo's index of every screen. `screens::index` says
            // why it exists and marks itself as not the game's.
            //
            // arm: ours/setup-key-index
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            // arm: ours/setup-key-escape
            Event::KeyDown(Key::Escape) => {
                // Whatever the page is, Escape is its own way back — the
                // original's Back button where there is one, and out of the
                // front end where there is not.
                return match self.page {
                    // Pop, not Quit. Popping the last screen quits anyway —
                    // `Machine::apply` — so this is the right answer both when
                    // the front end is the root and when it was opened from
                    // somewhere else, without the screen having to know which.
                    SetupPage::Title => Transition::Pop,
                    SetupPage::Dropdown => {
                        self.page = self.under;
                        Transition::Stay
                    }
                    _ => self.go(SetupPage::Title),
                };
            }
            Event::Pointer { x, y } => {
                if let Some((i, _)) = self.at(x, y) {
                    self.selected = i;
                }
            }
            Event::Click { x, y } => {
                if let Some((i, action)) = self.at(x, y) {
                    self.selected = i;
                    return self.act(action, ctx);
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        // **[D]** The front end's own text flags. `DAT_005AEA40` is set around
        // every menu item, button caption and body line and cleared for the
        // heading, so on these pages *only the heading is embossed*; and
        // `DAT_0058FE2C` is set around the heading alone, which draws its
        // capitals in colour 1. So there are two pens here, not one: `head`
        // for the heading and `pen` — flat, no drop capitals — for everything
        // else. A reimplementation that embossed the lot would be wrong on
        // every page of the front end at once.
        let head = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW_GATEWAY),
            caps: Some(1),
        };
        let pen = head.flat();
        // Page 9 is drawn over whatever was underneath it, so the background
        // and the page beneath are painted first and only then the open list.
        let base = if self.page == SetupPage::Dropdown { self.under } else { self.page };
        if !shell::background(canvas, a, base.background()) {
            // No install, or a partial one. Say so in our own font — never in
            // the original's — so that an empty page can never be mistaken for
            // a page the game drew empty.
            canvas.clear(ctx.assets.ink.background);
            let line = format!(
                "SETUP PAGE {} - NO {}",
                base.number(),
                base.background().to_uppercase()
            );
            l2_view::text::draw(canvas, 4, 4, &line, ctx.assets.ink.dim);
        }
        self.paint(ctx, canvas, &pen, &head, base);
        if self.page == SetupPage::Dropdown {
            self.paint_dropdown(canvas, &pen, ctx);
        }
    }
}

impl SetupScreen {
    fn colour(&self, index: usize) -> u8 {
        if index == self.selected {
            font::HIGHLIGHT
        } else {
            font::TEXT
        }
    }

    /// A menu item: the recess, then the caption centred in it.
    fn draw_item(
        &self,
        canvas: &mut Canvas,
        pen: &Pen,
        index: usize,
        rect: Rect,
        group: usize,
        s: usize,
    ) {
        shell::button_recess(canvas, rect.x, rect.y, rect.w, rect.h);
        pen.eng_centred(
            canvas,
            group,
            s,
            rect.x,
            rect.y + ITEM_TEXT,
            rect.w,
            self.colour(index),
        );
    }

    fn paint(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        match page {
            SetupPage::Title => {
                pen.window_from(canvas, BOX_SHEET, 0xA0, 10, 0x14, 0xF);
                head.eng_heading_centred(canvas, GROUP, 0, 0x80, 0x20, 0x180, font::TEXT);
                pen.eng_centred(canvas, GROUP, 1, 0x80, 0x3A, 0x180, font::TEXT);
                for (i, s) in TITLE_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
                // **Ours, and the one caption on this screen that has to be.**
                // Not the original's — see [`crate::build_id`], which exists
                // because a player spent an evening reporting three defects
                // against a binary four merges old.
                crate::build_id::draw(canvas, pen);
            }
            SetupPage::Options => {
                pen.window_from(canvas, BOX_SHEET, 0xB0, 10, 0x12, 0x12);
                head.eng_heading_centred(canvas, GROUP, 5, 0xB0, 0x2D, 0x120, font::TEXT);
                for (i, s) in OPTION_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
            }
            SetupPage::Load => {
                // `FUN_004148E4(5)`, transcribed. The box, the caption, one
                // **recess** and three **outlines** — and the difference
                // between the two was wrong here until the draw-call audit read
                // the painter: `FUN_00403EE4` is the bevelled recess (top and
                // right `0x35`, bottom and left `0x28`) and `FUN_00403CF4` is a
                // flat one-pixel rectangle in a single colour. Only the outer
                // frame is a recess; the name field, the file list and the
                // status line are outlines in `0x3F`.
                pen.window_from(canvas, BOX_SHEET, 0x60, 10, 0x1C, 0x15);
                head.eng_heading_centred(canvas, GROUP_FILE, 5, 0x60, 0x22, 0x1C0, font::TEXT);
                shell::button_recess(canvas, 0x70, 0x42, 400, 0x100);
                for r in LOAD_OUTLINES {
                    outline_rect(canvas, r, font::TEXT);
                }
                // **What used to be here was invented.** The line under the
                // list read `L2.eng` 40/8 *"Right click to exit."*, which the
                // original draws on **page 13** and never here.
                // `SaveLoad_DrawStatus` puts 40/2 *"Loading game. Please
                // wait."* at (128, 292) and only while `DAT_0057D3C4` — a
                // frame countdown set to 150 or 400 when a load actually
                // starts, and zeroed when the box opens — is running. An idle
                // load box has an empty status line, so ours has one too.
                //
                // The rest of `SaveLoad_DrawStatus` is not drawn: the four
                // `Panels2.pl8` plates, the file name being typed with its
                // caret, and up to thirty save names in three columns from
                // (128, 118). [`super::saveload`] is the same function on
                // screens `0x35`/`0x36`; page 3 is that screen inside the front
                // end's window, and joining them is a job on its own.
            }
            SetupPage::Shield => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x10);
                head.eng_heading_centred(canvas, GROUP, 10, 0x50, 0x23, 0x1E0, font::TEXT);
                self.paint_shields(canvas, pen);
                for (i, (x, y, s)) in SHIELD_BUTTONS.iter().enumerate() {
                    self.draw_item(
                        canvas,
                        pen,
                        5 + i,
                        Rect::new(*x, *y, ITEM_W, ITEM_H),
                        GROUP,
                        *s,
                    );
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                pen.window_from(canvas, BOX_SHEET, 0x40, 0x32, 0x20, 8);
                head.eng_heading_centred(canvas, GROUP_EXPANSION, 0, 0x40, 0x50, 0x200, font::TEXT);
                let items: [usize; 2] =
                    if page == SetupPage::Campaign { [4, 5] } else { [1, 2] };
                for i in 0..2 {
                    shell::button_recess(canvas, PAIR_X[i], PAIR_Y, PAIR_W, ITEM_H);
                    pen.eng_centred(
                        canvas,
                        GROUP_EXPANSION,
                        items[i],
                        PAIR_TEXT_X[i],
                        PAIR_Y + 6,
                        PAIR_TEXT_W,
                        self.colour(i),
                    );
                }
            }
            SetupPage::NoCd => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x13);
                head.eng_heading_centred(canvas, GROUP, 0x10, 0x50, 0x24, 0x1E0, font::TEXT);
                // Three wrapped paragraphs at width 0x180 and one plain line.
                // The plain one is drawn in colour 1, not 0x3F — the only
                // string on any of these pages that is.
                let a = pen.assets;
                for (i, y) in [(0x11usize, 0x48), (0x12, 0x78)] {
                    let t = a.text(GROUP, i).to_string();
                    pen.body_wrapped(canvas, 0x80, y, 0x180, &t, font::TEXT);
                }
                pen.eng(canvas, GROUP, 0x30, 0x80, 0xDC, 1);
                let t = a.text(GROUP, 0x31).to_string();
                pen.body_wrapped(canvas, 0x80, 0xF0, 0x180, &t, font::TEXT);
            }
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => {
                self.paint_custom(ctx, canvas, pen, page)
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti | SetupPage::SkirmishFile => {
                self.paint_skirmish(canvas, pen, head, page)
            }
        }
    }

    /// `FUN_0041F1DD` and `FUN_0041F321`: the name field and the five shields.
    ///
    /// **[V]** and worth writing down, because the obvious reading is wrong.
    /// The painter blits from `DAT_004EABEC` — the general scratch buffer,
    /// which on this page holds **`panels2.pl8`** — and *not* from
    /// `g_miscCtySheet`. `Misc_sel.pl8` has seventeen frames; the indices here
    /// run to 215, and `Panels2.pl8` has 216. The file settles it: frames
    /// 205 … 214 are five pairs of roughly 60 × 65 shields, one pair per realm
    /// colour, 204 is a 224 × 32 plate the size of a name field, and 215 is a
    /// 54 × 27 plaque. Nothing else in either file is that shape.
    ///
    /// Frame `2i + 0xCB` is the shield when the colour is free and `2i + 0xCC`
    /// when it is taken, at `x = 0x70 + 88(i - 1)`, `y = 0x8C`; the 54 × 27
    /// plaque marks the chosen one at `(x + 4, 0x70)`.
    fn paint_shields(&self, canvas: &mut Canvas, pen: &Pen) {
        // **The name field, and it now has a name in it.**
        //
        // `FUN_0041F321` is five statements and every one of them is here:
        //
        // ```c
        // g_caretPlaced = 0; g_caretX = 0; g_drawIndex = 0;
        // g_editDrawing = 1; g_penAdvance = 0;
        // Pl8_DrawFrameHere(panels2, 0xCC, 0xD0, 0x48);      /* the plate      */
        // Ui_DrawText(&g_options, 0xD6, 0x50, &g_fontBody, 0x3F);
        // if (!g_caretPlaced) { g_caretX = g_penAdvance; g_caretPlaced = 1; }
        // g_caretX += 0xD6;  g_caretY = 0x52;
        // Edit_DrawCaret(0x5AF8F0, 0x3F);                    /* the caret      */
        // ```
        //
        // The plate is drawn **before** the text and the caret **after** it,
        // which is why the caret is a solid bar rather than a shape the plate
        // eats. `g_caretPlaced` is set by `Ui_DrawText` itself when the drawing
        // index reaches `g_editCaret`, so the caret x is the pen after that
        // many characters and needs nothing from the caller;
        // `TextField::caret_x` computes the same number the same way.
        //
        // **The plate is the same frame whether or not the field is being
        // typed into.** There is no focus ring and no second frame: the caret
        // is the whole of the affordance, which is why it had to be built
        // rather than skipped.
        let sheet = pen.assets.sheet(BOX_SHEET);
        match sheet.and_then(|s| s.frame(0xCC)) {
            Some(f) => canvas.blit(&f, NAME_PLATE_X, NAME_PLATE_Y),
            // No `Panels2.pl8`. A recess of our own, so the field is still a
            // field on a placeholder install and a test can still find it.
            None => shell::button_recess(canvas, NAME_PLATE_X, NAME_PLATE_Y, 0xE0, 0x20),
        }
        pen.body(canvas, NAME_X, NAME_Y, &self.name.text(), font::TEXT);
        // `font::TEXT` is `0x3F`, a palette index; with no font loaded the
        // fallback renderer draws in named interface colours instead, and the
        // caret has to follow the text it belongs to.
        let ink = if pen.assets.body.is_some() { font::TEXT } else { pen.ink.text };
        self.name.draw_caret(canvas, NAME_X, NAME_Y, ink, &text::FontMetrics::of(pen.assets));
        let Some(sheet) = sheet else { return };
        for i in 1..6usize {
            let x = SHIELD_X + (i as i32 - 1) * SHIELD_STEP;
            if i - 1 == self.shield {
                if let Some(f) = sheet.frame(0xD7) {
                    canvas.blit(&f, x + 4, 0x70);
                }
            }
            // Free, not taken: this shell has no lobby, so every colour is
            // offered and the taken variant (`2i + 0xCC`) is never drawn.
            if let Some(f) = sheet.frame(i * 2 + 0xCB) {
                canvas.blit(&f, x, 0x8C);
            }
        }
    }

    /// Pages 7 and 8: the twelve options, the map list, the buttons.
    fn paint_custom(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen, page: SetupPage) {
        let a = pen.assets;
        if page == SetupPage::Custom {
            if let Some(s) = a.sheet(ICON_SHEET) {
                if let Some(f) = s.frame(0x0F) {
                    canvas.blit(&f, 0xA0, 0);
                }
            }
        }
        // The map list plate, its five rows, and the row the pointer is on.
        if let Some(s) = a.sheet(ICON_SHEET) {
            if let Some(f) = s.frame(0x10) {
                canvas.blit(&f, MAP_LIST_X, 9);
            }
        }
        // **The thumbnail of the map the list is pointing at**, which nothing
        // here drew. `ScenarioList_Draw`'s second statement is
        // `FUN_00410C71(0, 0x1F0, 9)` — the same helper the send-supplies panel
        // and the diplomacy county picker use, at the same `(x - 2, y + 3)`
        // offset — so the plate is a frame round a live minimap and not a
        // picture of one. County 0 is passed, so nothing is highlighted.
        if let Some(m) = ctx.assets.minimap(self.map) {
            let owner = |c: u8| ctx.game.kingdom.counties.get(c as usize).map_or(0, |c| c.owner);
            l2_view::chrome::draw_minimap_at(
                canvas,
                &m,
                MAP_THUMB,
                0,
                &l2_view::chrome::MinimapTint::Owner(&owner),
            );
        }
        for row in 0..MAP_LIST_ROWS {
            let slot = self.map_top + row;
            if slot >= MAP_COUNT {
                break;
            }
            let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
            let chosen = slot == self.map;
            canvas.fill_rect(
                MAP_LIST_X,
                y,
                MAP_LIST_W,
                MAP_LIST_ROW,
                if chosen { font::TEXT } else { font::DISABLED },
            );
            let name = a.text(GROUP_MAPS, slot).to_string();
            pen.body(
                canvas,
                MAP_LIST_TEXT_X,
                y + 1,
                &name,
                if chosen { font::DISABLED } else { font::TEXT },
            );
        }
        self.paint_scrollbar(canvas);
        // The twelve options.
        let chrome = pen.chrome;
        for (i, &(x, boxy, labely)) in OPTION_CELLS.iter().enumerate() {
            // `FUN_0040328E(102, i, x, labelY, 100, …)`: wrapped at 100 pixels,
            // which is what makes "Advanced Farming" two lines that end where
            // the box begins instead of one that runs into the next column.
            let label = a.text(GROUP_OPTIONS, i).to_string();
            pen.body_wrapped(canvas, x, labely, OPTION_LABEL_W, &label, font::TEXT);
            match chrome {
                // `FUN_004093E0` is `Ui_DrawBox` with border set 1.
                Some(c) => c.draw_box(canvas, x, boxy, 6, 3, 1),
                None => shell::button_recess(canvas, x, boxy, OPTION_BOX_W, OPTION_BOX_H),
            }
            let value = a.text(GROUP_VALUES, self.option_value(i)).to_string();
            pen.body_centred(canvas, x + 1, boxy + 16, 0x60, &value, font::HIGHLIGHT);
        }
        let n = if page == SetupPage::Custom { 3 } else { 4 };
        for (i, (x, s)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
            pen.eng_centred(
                canvas,
                GROUP,
                *s,
                *x,
                CUSTOM_BUTTON_Y,
                CUSTOM_BUTTON_W,
                self.colour(12 + MAP_LIST_ROWS + i),
            );
        }
        // **Both custom pages draw the five player cards, not only page 8.**
        // This comment used to say page 8; `FUN_0041F6C7` — page 7's painter —
        // calls `FUN_0041FBCB` with no guard, exactly as `FUN_0041F77A` does.
        // The card is `misc_sel` frame `2 * shieldIndex - 2` at (10, 94n + 6),
        // the lord's portrait frame `lord + 9` (14 for a human) at (84, 94n +
        // 10), and the name centred in 160 pixels at y = 94n + 80 in the
        // realm's own palette byte. `Realms_AssignLords` runs *inside* the
        // painter, so the cards are the assignment as much as a picture of it.
        //
        // Not drawn: the front end has not assigned lords in this workspace and
        // a card built from `Realm::default()` would be five copies of one
        // face. The chat log (`FUN_0041FF75`) and the chat input line
        // (`FUN_00420147`) are **multiplayer only** — both open with
        // `if (g_multiplayer != 0)` — so on page 7 the original draws nothing
        // for them either, and that is four call sites correctly absent rather
        // than missing.
        self.paint_gaps(canvas, pen);
    }

    /// `ScenarioList_Draw`'s scroll bar, transcribed.
    ///
    /// Three stacked fills: the run above the window in `0x20`, the window in
    /// `0x3F`, the run below in `0x20`. The thumb absorbs the rounding error of
    /// all three percentages so the track is always exactly 44 pixels.
    ///
    /// **[V] with an empty list it is one full-length thumb**: `PctOf` returns
    /// 0 when the total is 0, so all three heights come out 0 and the
    /// correction hands the whole 44 to the middle segment.
    fn paint_scrollbar(&self, canvas: &mut Canvas) {
        // **The total is the one number here that is not the original's.**
        // `ScenarioList_Draw` divides by `DAT_00554018`, which
        // `FUN_0046A101` sets to *how many of the sixty slots have a
        // `MAPnn.PL8` on disk* — 44 on a shipped install, because the game
        // ships eleven of the fifteen files. We divide by all sixty. The
        // module header says what it would take to have the real one.
        let total = MAP_COUNT as i32;
        let pct_of = |a: i32, b: i32| if b == 0 { 0 } else { a * 100 / b };
        let pct = |x: i32, p: i32| p * x / 100;
        let top = self.map_top as i32;
        let rows = MAP_LIST_ROWS as i32;
        let above = pct(SCROLLBAR_H, pct_of(top, total));
        let below = pct(SCROLLBAR_H, pct_of(total - top - rows, total));
        // `iVar2 + ((0x2C - iVar4) - iVar2 - iVar3)`, which is the window's own
        // percentage plus whatever the three roundings lost — and simplifies to
        // the track minus the other two, exactly.
        let thumb = SCROLLBAR_H - above - below;
        for (y, h, colour) in [
            (SCROLLBAR_Y, above, font::DISABLED),
            (SCROLLBAR_Y + above, thumb, font::TEXT),
            (SCROLLBAR_Y + above + thumb, below, font::DISABLED),
        ] {
            if h != 0 {
                canvas.fill_rect(SCROLLBAR_X, y, SCROLLBAR_W, h, colour);
            }
        }
    }

    /// **What this build cannot honour, said on the page.**
    ///
    /// `docs/decisions.md` C21: a switch wired to nothing must not look
    /// finished. Two things go here — an option whose behaviour does not exist
    /// (*Exploration*), and the map, which the list can select and the world
    /// builder cannot yet build.
    ///
    /// **In our own font, never the original's**, for the same reason
    /// [`Screen::draw`]'s missing-background line is: nothing the original
    /// never drew may appear in its typeface, or a screenshot stops being
    /// evidence of anything.
    fn paint_gaps(&self, canvas: &mut Canvas, pen: &Pen) {
        let mut y = 462;
        let mut say = |line: &str| {
            l2_view::text::draw(canvas, MAP_LIST_X - 180, y, line, font::HIGHLIGHT);
            y += 9;
        };
        for &i in &self.unhonoured {
            let label = pen.assets.text(GROUP_OPTIONS, i).to_string();
            say(&format!("NOT IMPLEMENTED: {}", label.to_uppercase()));
        }
        // **The map line is gone**, and that is the point of this commit: the
        // slot the list names is now the world *Start* builds. What is left is
        // the case where it cannot be built at all.
        if let Some(why) = &self.failure {
            say(&format!("CANNOT START THIS MAP: {}", why.to_uppercase()));
        }
    }

    /// Page 9: the box the option opens, over the page underneath.
    fn paint_dropdown(&self, canvas: &mut Canvas, pen: &Pen, _ctx: &Ctx) {
        // `DAT_00553FB4` is the item count and the box is that plus the two
        // border cells — the painter and the hit test read the one number, so a
        // *Nobles* list shortened to the map cannot draw four rows and accept
        // three.
        let n = self.rows(self.open);
        let (x, y, _) = OPTION_LIST[self.open];
        let y = self.dropdown_y(y, n);
        pen.window(canvas, x, y, 6, n as i32 + 2, 1);
        for i in 0..n {
            let s = pen.assets.text(GROUP_VALUES, OPTION_BASE[self.open] + i).to_string();
            let colour = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            pen.body_centred(canvas, x + 1, y + 16 + i as i32 * 16, 0x60, &s, colour);
        }
    }

    /// Pages 11, 12 and 13. The battlefield itself is `l2-sim`'s, and the
    /// skirmish editor's palette of tools is `L2.eng` group 41; neither is
    /// drawn here. What is drawn is the page's own furniture: the background,
    /// and the three captions at `y = 0x1B8` that `FUN_0042051C` and
    /// `FUN_00420630` put there.
    fn paint_skirmish(&self, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        let items: [usize; 3] = match page {
            // 12 and 11: "Back", "Cust.", "Go".
            SetupPage::SkirmishFile => [0x24, 0x25, 0x26],
            _ => [0x25, 0x26, 0x24],
        };
        for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
            pen.eng_centred(canvas, GROUP, items[i], *x, 0x1B8, 0x38, self.colour(i));
        }
        if page == SetupPage::SkirmishFile {
            // `FUN_0042150B` opens `Ui_DrawBox(0x60, 100, 0x1C, 0x12)` — border
            // set **0**, the only window on any of these pages that is not set
            // 1 — over the skirmish page, and draws group 40's file captions
            // into it. The rest is `FUN_00414E06(999)`: a parchment plate, the
            // same outline again, and up to ten `.skr` names at (128, 176 + 16n)
            // with the selected one on a `0x3F` bar. Nothing here reads a
            // directory of skirmish files, so the rows are absent and the box
            // they sit in is not.
            pen.window(canvas, 0x60, 100, 0x1C, 0x12, 0);
            head.eng_heading_centred(canvas, GROUP_FILE, 6, 0x60, 0x84, 0x1C0, font::TEXT);
            outline_rect(canvas, SKIRMISH_FILE_LIST, font::TEXT);
            pen.eng_centred(canvas, GROUP_FILE, 8, 0x60, 0x164, 0x1C0, font::TEXT);
            // `FUN_00414E06`'s own two, in its order: the parchment first and
            // **the same outline again** over it. The plate is 336 x 176 and
            // the outline 352 x 165, so the plate overhangs the bottom edge and
            // the second draw puts it back — the original's overdraw, kept.
            pen.box_interior(canvas, 0x7E, 0xAE, 0x15, 0xB);
            outline_rect(canvas, SKIRMISH_FILE_LIST, font::TEXT);
            // `Ui_OkButton(0x1F8, 0x164, 0)` — the last statement of the
            // painter, and the only page of the thirteen that has one.
            pen.ok_button(canvas, SKIRMISH_FILE_OK.0, SKIRMISH_FILE_OK.1, 0);
        }
    }
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
    /// **[V] and the empty case is the one that matters**: `PctOf` returns 0
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
