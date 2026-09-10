//! **The armoury** — `Screen_Armoury` (`0x00417EA7`), `g_screenId` `0x0A`, and
//! one weapon's rack, `Armoury_LoadScreen` (`0x004184C6`), `g_screenId` `0x0D`.
//!
//! # The screen a player said was missing
//!
//! > *"The hire an army is pretty botched at the moment. Instead of the
//! > blacksmith with a listing of their tools, it's just a weird popup with a
//! > lot of placeholder stuff."*
//!
//! Both halves of that sentence are literal. The *listing of tools* is
//! `FUN_00418426`: six weapons hang on the walls of the armoury at fixed
//! positions, and **each is drawn only if the realm owns one** — so the picture
//! is an inventory, and an empty treasury is an empty room. The *blacksmith* is
//! the picture itself, `Armoury.pl8`, a 640 × 480 painting of a forge. And the
//! *weird popup* was the raise-army screen, which this module now puts back on
//! the surface it belongs on.
//!
//! # `0x17` is drawn on top of `0x0A`, and that was the finding
//!
//! `Screen_Draw`'s arms:
//!
//! ```c
//! else if (g_screenId == '\n') { Screen_Armoury(firstFrame); }
//! else if (g_screenId == '\x17') { if (firstFrame == 1) Screen_Armoury(1);
//!                                  Screen_RaiseArmy(); }
//! ```
//!
//! One painter, two screens. The raise-army window is a `Ui_DrawBox` **over the
//! armoury**, under `armoury.256`, not over the campaign map under the campaign
//! palette — which is what `screens/army.rs` used to claim and what the player
//! was looking at. [`page`] is that shared painter and both screens call it,
//! which is the same sharing the binary does.
//!
//! # The painter, address by address
//!
//! ```text
//! Screen_Armoury(firstFrame):                                     0x00417EA7
//!   File_ReadChunk("armoury.256", g_displayPalette, 0x300)   NOT a draw
//!   FUN_00408FCB("armoury.pl8", 0x1E0)   THE 640 x 480 BACKDROP - a draw:
//!       g_screenStride * 0x1E0 bytes from offset 0x18 into g_backBufferBits
//!   File_ReadChunk("arm_grid.pl8", &g_villageGrid, 0x12D8)   the 80 x 60 hit map
//!   Ui_OkButton(640 - 0x1C, 480 - 0x70, 1)                        (612, 368)
//!   File_ReadChunk(g_armouryItemSheets[realm.shieldIndex], DAT_0056D5B8, 0x4B320)
//!       ... falling back to "base1a.pl8" if that read fails
//!   File_ReadChunk("armtorch.pl8", DAT_005533C4, 150000)
//!   FUN_00418426()                              the six weapons on the walls
//!   save four background strips for the walking soldier:
//!       y 0xD8 h 0x9E; x 0 / 0xA0 / 0x140 w 0x3C, x 0x1E0 w 0x28
//!   FUN_004181EB()                     the eight portraits and their numbers
//!   Ui_DrawCentred(69, 6, 0x21E, 0x194, 100, body, 0x37)        "Create"
//!   Ui_DrawCentred(69, 7, 0x21E, 0x1AE, 100, body, 0x37)        "Change"
//!   Ui_DrawCentred(69, 8, 0x21E, 0x1C8, 100, body, 0x37)        "Cancel"
//!   Palette_Set(armoury.256)
//! ```
//!
//! `FUN_004180F6` is the same function with the two big reads and the grid
//! dropped: it is what `0x0D` repaints the armoury with on the way back, and it
//! is why the sheets are loaded once rather than per frame.
//!
//! **`FUN_00418426` — the walls.** Table `g_armouryWallItems` (`0x004D2D88`),
//! six records of `(frame, x, y)`, one per weapon slot, drawn when
//! `realm.weapons[t - 1] > 0`. [`WALL`].
//!
//! **`FUN_004181EB` — the racks.** Table `g_armouryRacks` (`0x004D2CE8`), eight
//! records of `(frame, spriteX, spriteY, numberX, numberY)` indexed by *basket
//! slot*. [`RACKS`]. Slot 0 is the unequipped peasants, 1…6 the weapon types,
//! 7 the levy total — and slot 7's frame is `13 + shieldIndex - 1`, so the
//! figure at the end of the row wears the player's own colours. A rack is drawn
//! when the realm has that weapon in stock **or** when the mercenary band on
//! offer is of that troop type, in which case the band's men are added to the
//! number. The loop's guard is `if (6 < i) return`, so **slot 7's arm is dead
//! code**: the total is never drawn, and the three lines of the painter that
//! would have drawn it — including the shield-tinted frame — are unreachable.
//! `[V]`, and [`the test`](tests::the_totals_rack_is_dead_code_in_the_original)
//! keeps it that way.
//!
//! # The click dispatch
//!
//! `Screen_HandleInput`'s two arms, both of them `FUN_0043582A` first:
//!
//! ```c
//! 0x0A:  FUN_0043582A()  ||  Hotspot_Test(0, 0, &g_armouryHotspots, 9)
//! 0x0D:  FUN_0043582A()  ||  Hotspot_Test(0, 0, &g_armouryHotspots, 7)
//!                        ||  Widget_Test(0x60, 4, &g_armouryBuyWidgets, 4)
//! ```
//!
//! `FUN_0043582A` is the grid: `arm_grid.pl8[(x >> 3) + (y >> 3) * 0x50]`, whose
//! cells hold the weapon slot under the pointer, so **the weapon hanging on the
//! wall is the button.** It runs before the rectangles and therefore wins where
//! they overlap, which they do not.
//!
//! `g_armouryHotspots` (`0x004DC938`) is nine 24-byte records: six racks along
//! the bottom carrying troop ids 1, 2, 4, 6, 5, 3 in screen order, then the
//! three buttons on the right — and screen `0x0D` tests only the first seven of
//! them, so *Change* and *Cancel* are dead there while *Create* is not.
//!
//! Both handlers are one function each:
//!
//! ```c
//! FUN_004358B0():                      a rack, from the grid or a rectangle
//!     if (basket[id].available <= 0) return;       /* an empty rack is inert */
//!     FUN_004AABD8(county, DAT_00553F20);          /* the walking soldier    */
//!     DAT_00553F20 = id;  g_screenId = 0x0D;
//!     basket[id].+0x0C = basket[id].chosen;        /* the animation latch    */
//!     Armoury_LoadScreen();
//!
//! FUN_00435AE8():                                       the three buttons
//!     id 1 -> g_confirmAnswer = 1; Army_RaiseConfirm();          "Create"
//!     id 2 -> g_screenId = 0x17;                                 "Change"
//!     id 3 -> g_confirmAnswer = 0; Army_RaiseConfirm();          "Cancel"
//! ```
//!
//! **So `Army_RaiseConfirm` is a button on the armoury.** There is no confirm
//! anywhere on the raise-army screen: `0x17`'s widget table is the *Continue*
//! button and the mercenary tick and cross, and nothing else. `screens/army.rs`
//! carried a `RAISE` button of ours because this screen was a shell; it is gone,
//! and the door it stood in front of is this one.
//!
//! # `0x0D`, one weapon — and `Screen_Draw` has no arm for it
//!
//! **Verified against `Screen_Draw` (`0x0040F1A0`): there is no `'\r'` case.**
//! `0x0D` is painted exactly once, by `Armoury_ClickRack` calling
//! `Armoury_LoadScreen` on the way in, and after that only
//! `Screen_DrawWidgets`' `0x0D` arm runs:
//!
//! ```c
//! Armoury_RestoreWalkerStrip(); Armoury_DrawTorches(); Armoury_DrawRacks();
//! FUN_00418E2D(); Armoury_DrawWalker();
//! Widget_Draw(0x60, 4, &g_armouryBuyWidgets, 4);
//! ```
//!
//! So `FUN_00418E2D` is in that arm *as well as* in the painter — which is what
//! makes the count and the spare update when a button is pressed, on a screen
//! nothing repaints. The armoury's own racks are redrawn under the panel every
//! frame too, which is why the room does not go stale behind it.
//!
//! ## Two corner pictures, one of them dead
//!
//! `Ui_OkButton` (`0x0040D1BC`) **stashes only the last call's position** into
//! `DAT_0055CE78`/`DAT_0057C8A0`, and `Ui_OkButtonClicked` hit-tests a 24 × 24
//! box at whatever is stashed. `Screen_Armoury` stashed (612, 368) on the way
//! in; `Armoury_LoadScreen` then stashes (0x1E4, 0x58). So on `0x0D` the
//! armoury's corner picture is **still painted at (612, 368) and is not
//! clickable** — the panel's is the only live one. [`RackScreen`] tests
//! [`RACK_OK`] and nothing else, which reproduces that; it is said here because
//! it looks like an omission and is not.
//!
//! ```text
//! Armoury_LoadScreen():                                         0x004184C6
//!   File_ReadChunk("arm_cros|mace|swor|pike|bow|mail.pl8", DAT_0053E918, 250000)
//!       chosen by DAT_00553F20, the rack clicked; anything else reads arm_cros
//!   Ui_DrawBox(0x60, 4, 0x1A, 7)
//!   Ui_OkButton(0x1E4, 0x58, 0)
//!   Ui_DrawInsetRect(0x65, 9, 0x66, 0x66)
//!   Pl8_DrawFrame(weapon sheet, 0, 0x66, 10)     24 frames of 100 x 100, animated
//!   FUN_00418E2D()
//!
//! FUN_00418E2D():                                               0x00418E2D
//!   Pl8_DrawFrame(weapon sheet, DAT_005AEA48, 0x66, 10)
//!   spare = remaining[sel] - chosen[sel];  if (peasants < spare) spare = peasants;
//!   Ui_DrawBoxInterior(0xEC, 0x0C, 0x0F, 2)
//!   Ui_DrawCount(chosen[sel], sel * 2 + 0x34, 0xEC, 0x0D, heading)   "N Swordsmen"
//!   Ui_DrawBoxInterior(0xD8, 0x4C, 0x10, 2)
//!   Ui_DrawNumber(spare, 0xD8, 0x54) + Eng_DrawString(69, 5)
//!                                          "N more could still be raised."
//! ```
//!
//! `g_armouryBuyWidgets` (`0x004DD8C8`), drawn and tested at an offset of
//! `(0x60, 4)`, is four 32-pixel buttons in a row at y 44:
//!
//! | x | frame | handler | what it does |
//! |---:|---:|---|---|
//! | 176 | 68 | `0x0043593A` | **one man picks the weapon up** |
//! | 208 | 66 | `0x004359BC` | one man puts it down |
//! | 240 | 58 | `0x00435A0C` | every man of this type puts it down |
//! | 272 | 60 | `0x00435A61` | as many men as there are weapons pick it up |
//!
//! > **`docs/hypotheses.json` had the first two backwards, and so did the tool
//! > that generated them.** It named `0x0043593A` `Armoury_BuyLess` and
//! > `0x004359BC` `Armoury_BuyMore` on the strength of
//! > `tools/oracle/widgets.js`'s note that *"68/66 [are] minus and plus"*. The
//! > bodies say the opposite, and so does the only other table in the binary
//! > that uses the pair: the diplomacy gift row `0x004DD9D0` gives its frame-68
//! > record hotspot id 1 and its frame-66 record id 0, and `FUN_00436372` adds
//! > ten crowns for id 1 and subtracts ten for id 0. **Frame 68 is the plus.**
//! > `[V]` — two independent tables, and the correction is in the tool as well
//! > as in the database, because the tool is what would have said it again.
//!
//! And neither pair is a *buy*: nothing on either armoury screen spends money.
//! Weapons are made in a county by a blacksmith and paid for in iron and wood
//! (`docs/kingdom.md` §7.4); the armoury is where men pick them up. The four
//! renamed functions say so.
//!
//! # What is not here
//!
//! Three animations, all of them presentation and none of them a rule:
//!
//! * `FUN_00419243` — `armtorch.pl8` at (0x9A, 0x76) and (0x19A, 0x76), the
//!   second thirteen frames behind the first: **two guttering torches**;
//! * `FUN_00418FC5` / `FUN_004190DB` — a soldier of the type just equipped
//!   walking in from x = −80 along y = 0xD8, out of `trp_xb_r.pl8` and its
//!   sixty siblings, with the four saved strips restored behind him;
//! * `DAT_005AEA48` — the weapon on `0x0D` turning through its 24 frames.
//!
//! They are listed rather than drawn so that the next reader knows the room is
//! meant to move.
//!
//! # The denominator, for the draw audit
//!
//! `0x0A`: **twelve** draws — the backdrop, four in `Screen_Armoury`'s own
//! body, one in `Armoury_DrawWallItems`, three in `Armoury_DrawRacks` (**one of
//! them dead**, slot 7), and three in the `Screen_DrawWidgets` arm (two torches
//! and the walker). We draw eight, and the three we do not are the animations
//! above.
//!
//! **`FUN_00408FCB("armoury.pl8", 0x1E0)` is a draw**, and counting it as a
//! file load is the mistake this section was written with. It reads
//! `g_screenStride * height` bytes from offset `0x18` **straight into
//! `g_backBufferBits`**, and `Armoury.pl8` is 307,224 bytes = 640 × 480 + 24 —
//! so the file *is* the picture, not a sheet to pick frames out of. Eleven
//! painters in the binary call it. [`page`]'s `shell::background` is its
//! counterpart and is counted on our side too.
//!
//! The `.256` beside it is **not** a draw and the two are easy to conflate:
//! `File_ReadChunk("armoury.256", &DAT_004EA8A0, 0x300)` puts 768 bytes into a
//! palette buffer and `Palette_Set` points the hardware at it. No pixel moves.
//!
//! `0x0D`: **sixteen** — four in `Armoury_LoadScreen`, six in `FUN_00418E2D`,
//! and six more in the widget arm (torches, racks, walker); plus the four
//! widget records. It draws no backdrop of its own: it is a window over the
//! room `0x0A` already painted. `tools/audit/draws-B.json` carries the whole
//! reckoning.
//!
//! Excluded on both, and this is the whole exclusion list: the four
//! `FUN_004B3F0A` strip *saves* (they copy the backdrop out, they do not draw),
//! the `.256` read and `Palette_Set` above, `File_ReadChunk("arm_grid.pl8")`
//! which is a hit map, `Gfx_MarkSpriteDirty`, and the `Blit_*` family, which is
//! the implementation of every primitive above rather than a draw of its own.
//!
//! **Neither screen draws `L2.eng` 31/21 *"Morale"***, or any of group 31 —
//! checked by reading every one of the seven functions above rather than by
//! grepping for the string. `docs/armies.md` rests a `[V]` on unit `+0x166`
//! against that label, and nothing in this module resources it.

use l2_kingdom::levy::{self, LevyRefusal};
use l2_kingdom::tables::WEAPON_TYPE_COUNT;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::widget;

/// `L2.eng` group 69 — **shared with the raise-army screen**, which is what
/// `docs/formats/eng.md` §5 already said: *"Mercenary hire and the
/// raise-army/armoury panel"*. `screens/shells.rs` filed this screen under
/// group 16 (the twelve mercenary nationalities); the painter never touches it.
pub const GROUP: usize = 69;

/// Group 69's own indices, in the order the painter draws them.
pub const SPARE: usize = 5;
pub const CREATE: usize = 6;
pub const CHANGE: usize = 7;
pub const CANCEL: usize = 8;

/// `L2.eng` group 8's troop nouns, two apiece from index `0x34`: singular then
/// plural. `Ui_DrawCount(n, 0x34 + t * 2)` picks between them.
pub const NOUN_GROUP: usize = 8;
pub const NOUN_BASE: usize = 0x34;

/// `g_armouryItemSheets` (`0x004D2C88`), stride `0x10`, indexed by the realm's
/// `shield_index`.
///
/// Six slots for five colours: index 0 and index 1 are both `arm_it_r.pl8`,
/// which is the table agreeing with `l2_view::chrome::realm_colour`'s clamp of
/// a zero colour up to 1 — a realm with no banner is red in both places, from
/// two authors who never met. `[V]`
pub const ITEM_SHEETS: [&str; 6] = [
    "Arm_it_r.pl8",
    "Arm_it_r.pl8",
    "Arm_it_y.pl8",
    "Arm_it_k.pl8",
    "Arm_it_p.pl8",
    "Arm_it_b.pl8",
];

/// The sheet this realm's armoury is furnished from.
pub fn items_sheet(shield_index: u8) -> &'static str {
    ITEM_SHEETS[(shield_index as usize).min(ITEM_SHEETS.len() - 1)]
}

/// `Armoury_LoadScreen`'s six-way branch on `DAT_00553F20`. **The fall-through
/// is the crossbow**, not nothing: an unrecognised rack still loads a sheet.
pub const WEAPON_SHEETS: [&str; WEAPON_TYPE_COUNT] = [
    "Arm_cros.pl8",
    "Arm_mace.pl8",
    "Arm_swor.pl8",
    "Arm_pike.pl8",
    "Arm_bow.pl8",
    "Arm_mail.pl8",
];

/// One weapon hanging on the wall: `(frame, x, y)`, from `g_armouryWallItems`
/// (`0x004D2D88`), six records of twelve bytes, indexed by weapon slot — that
/// is, by `TroopType::weapon_slot`, one less than the basket slot.
pub const WALL: [(usize, i32, i32); WEAPON_TYPE_COUNT] = [
    (0, 59, 178),  // crossbow
    (1, 157, 240), // mace
    (2, 496, 235), // sword
    (3, 199, 130), // pike
    (4, 373, 232), // bow
    (5, 290, 182), // armour
];

/// One rack: `(frame, spriteX, spriteY, numberX, numberY)`, from
/// `g_armouryRacks` (`0x004D2CE8`), eight records of twenty bytes, indexed by
/// **basket slot** — 0 peasants, 1…6 the weapon types, 7 the total.
///
/// The x column is not sorted, and that is the whole shape of the table: the
/// *frames* run 6, 7, 8, 9, 10, 11, 12, 13 left to right along the bottom of
/// the picture, and it is the slot each one belongs to that jumps about. Read
/// by x, the row is peasant, crossbowman, maceman, pikeman, knight, archer,
/// swordsman, lord.
pub const RACKS: [(usize, i32, i32, i32, i32); 8] = [
    (6, 11, 396, 12, 450),   // 0 peasants
    (7, 85, 396, 86, 450),   // 1 crossbowmen
    (8, 162, 396, 163, 450), // 2 macemen
    (12, 462, 396, 463, 450), // 3 swordsmen
    (9, 237, 396, 238, 450), // 4 pikemen
    (11, 387, 396, 388, 450), // 5 archers
    (10, 311, 396, 312, 450), // 6 knights
    (13, 552, 396, 568, 450), // 7 the total — never drawn; see the module docs
];

/// The slot the painter's loop stops before. `FUN_004181EB` opens with
/// `if (6 < i) return`, so it draws slots 0…6 and slot 7's arm is unreachable.
pub const RACKS_DRAWN: usize = 7;

/// The six rack hotspots of `g_armouryHotspots` (`0x004DC938`), in table order,
/// as `(x0, y0, x1, y1, troopType)`. `Hotspot_Test` is **half-open**:
/// `x0 <= mx < x1`.
///
/// They are the fallback for a missing `arm_grid.pl8` and they are also the
/// original's — the grid is tested first and these are tested second, so a
/// click on the *floor* below a rack, where the grid holds nothing, still opens
/// it. The bottom edge is 480 on all six: the row runs to the foot of the
/// screen.
pub const RACK_HOTSPOTS: [(i32, i32, i32, i32, u8); WEAPON_TYPE_COUNT] = [
    (85, 396, 161, 480, 1),
    (162, 396, 236, 480, 2),
    (237, 396, 310, 480, 4),
    (311, 396, 386, 480, 6),
    (387, 396, 461, 480, 5),
    (461, 396, 538, 480, 3),
];

/// The three buttons down the right-hand edge, records 6, 7 and 8 of the same
/// table. Their labels are `L2.eng` 69/6, 69/7 and 69/8 — *"Create"*,
/// *"Change"*, *"Cancel"* — drawn by `Ui_DrawCentred` at x `0x21E` in a
/// hundred-pixel column, which lands inside each box.
pub const CREATE_BOX: Rect = Rect::new(542, 396, 634 - 542, 424 - 396);
pub const CHANGE_BOX: Rect = Rect::new(542, 425, 634 - 542, 450 - 425);
pub const CANCEL_BOX: Rect = Rect::new(542, 450, 634 - 542, 479 - 450);

/// `Ui_DrawCentred(69, i, 0x21E, y, 100, …)` — the three labels' own geometry.
pub const LABEL_X: i32 = 0x21E;
pub const LABEL_W: i32 = 100;
pub const LABEL_Y: [i32; 3] = [0x194, 0x1AE, 0x1C8];

/// `Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x70, 1)` on `0x0A`,
/// and `Ui_OkButton(0x1E4, 0x58, 0)` on `0x0D`. Both are 24 × 24 and both close
/// the screen they are on; `Ui_OkButtonClicked` tests a 24 × 24 box at whatever
/// position the *last* call stashed.
pub const OK: Rect = Rect::new(640 - 0x1C, 480 - 0x70, 24, 24);
pub const RACK_OK: Rect = Rect::new(0x1E4, 0x58, 24, 24);

// ------------------------------------------------------------------- 0x0D

/// `Ui_DrawBox(0x60, 4, 0x1A, 7)` — the rack panel's window.
pub const RACK_BOX_X: i32 = 0x60;
pub const RACK_BOX_Y: i32 = 4;
pub const RACK_BOX_COLS: i32 = 0x1A;
pub const RACK_BOX_ROWS: i32 = 7;

pub fn rack_window() -> Rect {
    Rect::new(RACK_BOX_X, RACK_BOX_Y, RACK_BOX_COLS * 16, RACK_BOX_ROWS * 16)
}

/// `Ui_DrawInsetRect(0x65, 9, 0x66, 0x66)` with `Pl8_DrawFrame(sheet, n, 0x66,
/// 10)` inside it — the weapon's own picture, 100 × 100 in a 102 × 102 well.
pub const WEAPON_WELL: Rect = Rect::new(0x65, 9, 0x66, 0x66);
pub const WEAPON_AT: (i32, i32) = (0x66, 10);

/// `Ui_DrawBoxInterior(0xEC, 0x0C, 0x0F, 2)`, with the count and the group 8
/// noun at `(0xEC, 0x0D)`.
pub const COUNT_WELL: Rect = Rect::new(0xEC, 0x0C, 0x0F * 16, 2 * 16);
pub const COUNT_AT: (i32, i32) = (0xEC, 0x0D);

/// `Ui_DrawBoxInterior(0xD8, 0x4C, 0x10, 2)`, with the spare count and 69/5 at
/// `(0xD8, 0x54)`.
pub const SPARE_WELL: Rect = Rect::new(0xD8, 0x4C, 0x10 * 16, 2 * 16);
pub const SPARE_AT: (i32, i32) = (0xD8, 0x54);

/// The four buttons of `g_armouryBuyWidgets` (`0x004DD8C8`) — 32 pixels square,
/// in a row at y 44, from x 176 — **plus `Widget_Draw`/`Widget_Test`'s
/// `(0x60, 4)` offset**, which is the rack window's own origin.
pub const BUTTON_DIM: i32 = 32;
pub const BUTTON_Y: i32 = 44;
pub const BUTTON_X: [i32; 4] = [176, 208, 240, 272];
/// The button-sheet frames the four records carry, in the same order.
pub const BUTTON_FRAMES: [usize; 4] = [68, 66, 58, 60];

/// What one of the four buttons does. The order is the table's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// `FUN_0043593A` — `if (chosen < remaining && peasants != 0) { chosen++;
    /// peasants--; }`.
    EquipOne,
    /// `FUN_004359BC` — `if (chosen > 0) { chosen--; peasants++; }`.
    UnequipOne,
    /// `FUN_00435A0C` — the same, in a `while`.
    UnequipAll,
    /// `FUN_00435A61` — `while (peasants != 0 && chosen < remaining)`, so it
    /// stops at whichever of the two runs out.
    EquipAll,
}

pub const BUTTONS: [Button; 4] =
    [Button::EquipOne, Button::UnequipOne, Button::UnequipAll, Button::EquipAll];

pub fn button_box(i: usize) -> Rect {
    Rect::new(RACK_BOX_X + BUTTON_X[i], RACK_BOX_Y + BUTTON_Y, BUTTON_DIM, BUTTON_DIM)
}

// ------------------------------------------------------------- the painter

/// **The whole static page**, shared by `0x0A` and by the first frame of
/// `0x17` exactly as `Screen_Draw` shares `Screen_Armoury` between them.
///
/// `buttons` is false for the raise-army screen, which draws the three labels
/// too — the painter is one function and does not know which screen called it —
/// but where they are *dead*: `0x17`'s input arm tests only its own three
/// widgets, so *Create*, *Change* and *Cancel* are visible and inert until the
/// player presses Continue. We draw them dimmed there rather than not at all,
/// because a label that is painted and does nothing is what the original shows.
pub fn page(ctx: &Ctx, canvas: &mut Canvas, buttons: bool) {
    let a = &ctx.assets.shell;
    let ink = &ctx.assets.ink;
    let pen = Pen {
        assets: a,
        ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };

    if !shell::background(canvas, a, "Armoury.pl8") {
        canvas.clear(ink.background);
    }

    let realm = ctx.game.kingdom.realms.get(ctx.game.player as usize);
    let sheet = realm.map(|r| items_sheet(r.shield_index)).unwrap_or(ITEM_SHEETS[1]);

    // `FUN_00418426`: the walls. A weapon the realm does not own is not there.
    if let Some(sheet) = a.sheet(sheet) {
        for (slot, &(frame, x, y)) in WALL.iter().enumerate() {
            if realm.is_some_and(|r| r.weapons[slot] > 0) {
                if let Some(f) = sheet.frame(frame) {
                    canvas.blit(&f, x, y);
                }
            }
        }
    }

    // `FUN_004181EB`: the racks. Slot 0 is unconditional, 1..=6 need stock in
    // the armoury or a band of that type on offer, and slot 7 is never reached.
    let basket = &ctx.game.levy.basket;
    let band = ctx.game.kingdom.counties.get(ctx.game.levy.county as usize).map_or(0, |c| {
        if ctx.game.levy.hire {
            c.mercenary_offer
        } else {
            0
        }
    });
    let (band_troop, band_men) = if band == 0 {
        (usize::MAX, 0)
    } else {
        let rules = &l2_kingdom::mercenary::ROSTER[band as usize];
        (rules.troop.index(), rules.men)
    };

    for (slot, &(frame, sx, sy, nx, ny)) in RACKS.iter().enumerate().take(RACKS_DRAWN) {
        let extra = if slot == band_troop { band_men } else { 0 };
        if slot > 0 && basket.slots[slot].available <= 0 && extra == 0 {
            continue;
        }
        if let Some(f) = a.sheet(sheet).and_then(|s| s.frame(frame)) {
            canvas.blit(&f, sx, sy);
        } else {
            // **Ours**, and only with no artwork: name the rack where its
            // picture would have stood, so the row is still readable and still
            // visibly ours.
            let name = TroopType::from_index(slot).map_or("", |t| t.name());
            text::draw(canvas, sx, sy + 40, &name.to_uppercase(), ink.dim);
        }
        // `Ui_DrawNumber(v, '@', &DAT_004D403C, x, y, &g_fontBody, 0x3F)` —
        // the game's own body font, not our 5 × 7 one.
        pen.number(canvas, nx, ny, basket.slots[slot].chosen + extra, true, font::TEXT);
    }

    // The three labels, in their own hundred-pixel column.
    for (i, &index) in [CREATE, CHANGE, CANCEL].iter().enumerate() {
        let colour = if buttons { font::TEXT } else { ink.dim };
        pen.eng_centred(canvas, GROUP, index, LABEL_X, LABEL_Y[i], LABEL_W, colour);
    }
}

// --------------------------------------------------------- 0x0A, the room

/// Screen `0x0A` for one county's levy.
pub struct ArmouryScreen {
    county: u8,
    /// One line of feedback. **Ours.**
    status: String,
    /// What the last *Create* did, for a test that wants to know without
    /// reading the kingdom.
    pub outcome: Raised,
}

/// What the *Create* button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    None,
    Army(usize),
    Refused(LevyRefusal),
}

impl ArmouryScreen {
    pub fn new(county: u8) -> ArmouryScreen {
        ArmouryScreen { county, status: String::new(), outcome: Raised::None }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    /// `FUN_0043582A` then `Hotspot_Test(&g_armouryHotspots, 6)`: the grid
    /// first, the rectangles second. Returns the weapon type under the pointer.
    pub fn rack_at(ctx: &Ctx, x: i32, y: i32) -> Option<u8> {
        if let Some(t) = ctx.assets.shell.armoury_grid(x, y) {
            return Some(t);
        }
        RACK_HOTSPOTS
            .iter()
            .find(|&&(x0, y0, x1, y1, _)| (x0..x1).contains(&x) && (y0..y1).contains(&y))
            .map(|&(.., t)| t)
    }

    /// `FUN_004358B0` — open a rack, **if it has anything in it**. An empty
    /// rack is inert: the guard is on `available`, the stock the realm owns,
    /// not on `chosen`.
    fn open_rack(&mut self, ctx: &mut Ctx, troop: u8) -> Transition {
        let slot = troop as usize;
        if ctx.game.levy.basket.slots.get(slot).is_none_or(|s| s.available <= 0) {
            let name = TroopType::from_index(slot).map_or("", |t| t.name());
            self.status = format!("NO {} IN THE ARMOURY", name.to_uppercase());
            return Transition::Stay;
        }
        ctx.game.levy.rack = troop;
        Transition::Push(ScreenId::Rack(self.county, troop))
    }

    /// `FUN_00435AE8`'s id 1 and id 3 — `Army_RaiseConfirm` with the answer set
    /// either way. A no is not a refusal: it closes the screen and raises
    /// nothing, which is `g_screenId = 0` down both arms of the handler.
    fn confirm(&mut self, ctx: &mut Ctx, yes: bool) -> Transition {
        if !yes {
            return Transition::Pop;
        }
        let levy = ctx.game.levy;
        let band = ctx.game.kingdom.counties.get(self.county as usize).map_or(0, |c| c.mercenary_offer);
        let affordable = band != 0
            && ctx.game.gold() >= l2_kingdom::mercenary::ROSTER[band as usize].price;
        let hire = (levy.hire && affordable).then_some(band);
        match ctx.game.raise_army(self.county, &levy.basket, levy.happiness_cost, hire) {
            Ok(id) => {
                self.outcome = Raised::Army(id);
                ctx.game.levy = crate::game::LevyOrder { percent: levy.percent, ..Default::default() };
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Raised::Refused(no);
                self.status = match no {
                    LevyRefusal::NoMen => "AN ARMY OF ZERO MEN IS NO ARMY".into(),
                    LevyRefusal::TooFew => "FEWER THAN 50 MEN IS IMPRACTICAL".into(),
                    LevyRefusal::NowhereToStand => "NOWHERE IN THE COUNTY TO STAND".into(),
                };
                Transition::Stay
            }
        }
    }
}

impl Screen for ArmouryScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Armoury(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The armoury".to_string()
    }

    /// `armoury.256`, set by the painter's last call before it returns.
    fn palette(&self) -> Option<&'static str> {
        Some("Armoury.256")
    }

    /// A 640 × 480 background is a page, not an inset.
    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // The corner picture and the right button both leave for the map
            // without raising anything — `g_screenId = 0; Gfx_LoadCountyMode()`
            // down every arm of `Screen_FrameInput`'s `0x0A` case.
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Enter) => self.confirm(ctx, true),
            Event::Click { x, y } => {
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                if CREATE_BOX.contains(x, y) {
                    return self.confirm(ctx, true);
                }
                if CHANGE_BOX.contains(x, y) {
                    return Transition::Replace(ScreenId::RaiseArmy(self.county));
                }
                if CANCEL_BOX.contains(x, y) {
                    return self.confirm(ctx, false);
                }
                let read = Ctx { game: ctx.game, assets: ctx.assets };
                if let Some(troop) = ArmouryScreen::rack_at(&read, x, y) {
                    return self.open_rack(ctx, troop);
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        page(ctx, canvas, true);
        let ink = &ctx.assets.ink;

        // The corner picture, mode 1 — `Ui_OkButton(640 - 0x1C, 480 - 0x70, 1)`.
        // **Nothing else is drawn round the three buttons**: the painter draws
        // three words and the hotspots are invisible, so a frame of ours here
        // would be an invented interface on a screen that does not have one.
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.ok_button(canvas, OK.x, OK.y, 1);

        // **Ours**, both of them: one line of feedback and one warning that the
        // hit map is missing. The original draws neither.
        if !self.status.is_empty() {
            text::draw(canvas, 8, 8, &self.status, ink.dim);
        }
        if !ctx.assets.shell.has_armoury_grid() {
            text::draw(canvas, 8, 20, "NO ARM_GRID.PL8 - RACKS ARE RECTANGLES", ink.dim);
        }
    }
}

// -------------------------------------------------------- 0x0D, one weapon

/// Screen `0x0D` — one rack, opened from the armoury.
pub struct RackScreen {
    county: u8,
    /// `DAT_00553F20`, 1…6. Held here as well as on the order because the
    /// screen is identified by it.
    troop: u8,
}

impl RackScreen {
    pub fn new(county: u8, troop: u8) -> RackScreen {
        RackScreen { county, troop }
    }

    pub fn troop(&self) -> u8 {
        self.troop
    }

    fn troop_type(&self) -> Option<TroopType> {
        TroopType::from_index(self.troop as usize)
    }

    /// The count `FUN_00418E2D` prints against 69/5: how many more men could
    /// still take this weapon.
    ///
    /// The original writes it `remaining[sel] - chosen[sel]`, clamped by the
    /// unequipped pool, and its `remaining` is the untouched stock the basket
    /// was seeded with. **Ours already is the difference** —
    /// [`l2_kingdom::LevyBasket::equip`] decrements `remaining` as it fills
    /// `chosen`, so the two conventions meet at the same number and only the
    /// expression differs. Written out rather than transcribed, because
    /// transcribing it would have subtracted `chosen` twice.
    fn spare(&self, ctx: &Ctx) -> i32 {
        let basket = &ctx.game.levy.basket;
        let slot = self.troop as usize;
        basket.slots.get(slot).map_or(0, |s| s.remaining).min(basket.unequipped()).max(0)
    }

    fn press(&mut self, ctx: &mut Ctx, button: Button) {
        let Some(troop) = self.troop_type() else { return };
        let basket = &mut ctx.game.levy.basket;
        let slot = self.troop as usize;
        match button {
            Button::EquipOne => {
                basket.equip(troop, 1);
            }
            Button::UnequipOne => {
                basket.unequip(troop, 1);
            }
            Button::UnequipAll => {
                let held = basket.slots[slot].chosen;
                basket.unequip(troop, held);
            }
            Button::EquipAll => {
                let room = basket.slots[slot].remaining;
                basket.equip(troop, room);
            }
        }
    }
}

impl Screen for RackScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Rack(self.county, self.troop)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("The armoury — {}", self.troop_type().map_or("", |t| t.name()))
    }

    fn palette(&self) -> Option<&'static str> {
        Some("Armoury.256")
    }

    /// `Ui_DrawBox(0x60, 4, 0x1A, 7)` over the armoury, with no clear.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // Both ways out go back to the armoury: `g_screenId = 0x0A;
            // FUN_004180F6()`.
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Right) => {
                self.press(ctx, Button::EquipOne);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.press(ctx, Button::UnequipOne);
                Transition::Stay
            }
            Event::Click { x, y } => {
                if RACK_OK.contains(x, y) {
                    return Transition::Pop;
                }
                for (i, button) in BUTTONS.iter().enumerate() {
                    if button_box(i).contains(x, y) {
                        self.press(ctx, *button);
                        return Transition::Stay;
                    }
                }
                // **The racks are still live here**, on the first seven records
                // of the same hotspot table: the player can walk from one
                // weapon to the next without going back. `Create` is record 6
                // and is live too; `Change` and `Cancel` are records 7 and 8
                // and are not.
                let read = Ctx { game: ctx.game, assets: ctx.assets };
                if let Some(troop) = ArmouryScreen::rack_at(&read, x, y) {
                    if troop != self.troop
                        && ctx.game.levy.basket.slots[troop as usize].available > 0
                    {
                        ctx.game.levy.rack = troop;
                        return Transition::Replace(ScreenId::Rack(self.county, troop));
                    }
                    return Transition::Stay;
                }
                if CREATE_BOX.contains(x, y) {
                    // Record 6 of the table, and the only one of the three the
                    // `0x0D` arm reaches. It is the armoury's button, so the
                    // armoury runs it: pass the click down.
                    return Transition::Pass;
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let w = rack_window();
        pen.window(canvas, w.x, w.y, RACK_BOX_COLS, RACK_BOX_ROWS, 0);

        // The weapon's own picture, frame 0 of its 24, in the four-line
        // `Ui_DrawInsetRect` well the painter opens for it.
        shell::inset_rect(canvas, WEAPON_WELL.x, WEAPON_WELL.y, WEAPON_WELL.w, WEAPON_WELL.h);
        let slot = (self.troop as usize).saturating_sub(1).min(WEAPON_TYPE_COUNT - 1);
        if let Some(f) = a.sheet(WEAPON_SHEETS[slot]).and_then(|s| s.frame(0)) {
            canvas.blit(&f, WEAPON_AT.0, WEAPON_AT.1);
        }

        // `Ui_DrawCount(chosen, 0x34 + t * 2)` — the count and the group 8
        // noun, singular or plural by the count.
        let held = ctx.game.levy.basket.slots[self.troop as usize].chosen;
        pen.box_interior(canvas, COUNT_WELL.x, COUNT_WELL.y, 0x0F, 2);
        let noun = noun(a, self.troop as usize, held, self.troop_type());
        pen.heading(canvas, COUNT_AT.0, COUNT_AT.1, &format!("{held} {noun}"), font::TEXT);

        // `Ui_DrawNumber(spare) + 69/5` — "N more could still be raised."
        let spare = self.spare(ctx);
        pen.box_interior(canvas, SPARE_WELL.x, SPARE_WELL.y, 0x10, 2);
        let x = pen.body(canvas, SPARE_AT.0, SPARE_AT.1, &format!("{spare}"), font::TEXT);
        pen.eng(canvas, GROUP, SPARE, x, SPARE_AT.1, font::TEXT);

        // The four buttons. `Widget_Draw(0x60, 4, &g_armouryBuyWidgets, 4)`
        // draws them out of the **button sheet** at the frames the records
        // carry — 68, 66, 58, 60 — and our own labelled boxes are the fallback
        // for an install without it, not the picture.
        for (i, button) in BUTTONS.iter().enumerate() {
            let r = button_box(i);
            if pen.system_frame(canvas, BUTTON_FRAMES[i], r.x, r.y) {
                continue;
            }
            // **Ours**, no artwork only.
            let label = match button {
                Button::EquipOne => "+1",
                Button::UnequipOne => "-1",
                Button::UnequipAll => "NONE",
                Button::EquipAll => "ALL",
            };
            widget::button(canvas, ink, r, label, false);
        }

        // `Ui_OkButton(0x1E4, 0x58, 0)`.
        pen.ok_button(canvas, RACK_OK.x, RACK_OK.y, 0);
    }
}

/// `Ui_DrawCount`'s noun: `L2.eng` group 8 at `0x34 + t * 2`, singular at **±1**
/// and plural at everything else including zero. Falls back to our own name
/// when `L2.eng` is not installed.
///
/// The `-1` arm is `Ui_DrawCount`'s (`0x0041AB67`) and not
/// `Ui_DrawUnitNoun`'s (`0x0041AC3E`), which takes the singular only at exactly
/// 1 — the rack panel draws the first, the army-division rows the second, and
/// they are two different ladders in the binary. [`shell::count_noun`].
fn noun(a: &shell::ShellAssets, troop: usize, n: i32, ty: Option<TroopType>) -> String {
    let index = shell::count_noun(n, NOUN_BASE + troop * 2);
    let s = a.text(NOUN_GROUP, index);
    if !s.is_empty() {
        return s.to_string();
    }
    let name = ty.map_or("", |t| t.name());
    if index % 2 == 0 {
        name.to_string()
    } else {
        format!("{name}s")
    }
}

/// `Levy_SetPercent`'s two refusals, reachable from this screen only.
pub fn would_refuse(men: i32, hiring: bool) -> Option<LevyRefusal> {
    levy::refuse_levy(men, hiring)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eight racks are a row: same sprite y, same number y, and the eight
    /// sprite x's are strictly increasing once sorted — which is the check that
    /// the slot-to-x scramble in [`RACKS`] is a scramble and not a typo.
    #[test]
    fn the_racks_are_one_row_across_the_bottom_of_the_screen() {
        let mut xs: Vec<i32> = RACKS.iter().map(|r| r.1).collect();
        for &(_, _, sy, _, ny) in &RACKS {
            assert_eq!(sy, 396, "every rack sprite sits on one line");
            assert_eq!(ny, 450, "and every number under it");
        }
        xs.sort_unstable();
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] >= 70, "racks {pair:?} would overlap");
        }
        assert!(xs[7] + 76 <= 640, "the last rack runs off the screen");

        // Read left to right the frames are 6, 7, 8, 9, 10, 11, 12, 13: the
        // sheet's own order, which is what makes the slot column the odd one.
        let mut by_x: Vec<(i32, usize)> = RACKS.iter().map(|r| (r.1, r.0)).collect();
        by_x.sort_unstable();
        assert_eq!(by_x.iter().map(|p| p.1).collect::<Vec<_>>(), vec![6, 7, 8, 9, 10, 11, 12, 13]);
    }

    /// Slot 7 is in the table and the painter never reaches it. If somebody
    /// "fixes" the loop bound, this says what they changed.
    #[test]
    fn the_totals_rack_is_dead_code_in_the_original() {
        assert_eq!(RACKS_DRAWN, 7, "FUN_004181EB's guard is `if (6 < i) return`");
        assert_eq!(RACKS.len(), 8, "the table has eight records all the same");
        assert!(RACKS_DRAWN < RACKS.len(), "the last record is never drawn");
    }

    /// Every hotspot names a real troop type, the six between them name all six
    /// weapon types once, and each sits under the rack it opens.
    #[test]
    fn each_rack_hotspot_covers_the_rack_it_opens() {
        let mut seen: Vec<u8> = RACK_HOTSPOTS.iter().map(|h| h.4).collect();
        seen.sort_unstable();
        assert_eq!(seen, vec![1, 2, 3, 4, 5, 6], "six weapon types, once each");
        for &(x0, y0, x1, y1, troop) in &RACK_HOTSPOTS {
            let (_, sx, sy, ..) = RACKS[troop as usize];
            assert!(x0 <= sx && sx < x1, "hotspot {x0}..{x1} misses rack {troop} at x {sx}");
            assert!(y0 <= sy && sy < y1, "hotspot {y0}..{y1} misses rack {troop} at y {sy}");
            assert!(x1 <= 640 && y1 <= 480, "hotspot for {troop} runs off the screen");
        }
    }

    /// The three boxes do not overlap each other, do not overlap the racks, and
    /// each contains the label the painter centres inside it. Three
    /// wrong-screen bugs have reached this player through a near-miss on a hit
    /// box; this is the one for these three.
    #[test]
    fn the_three_buttons_are_disjoint_and_hold_their_own_labels() {
        let boxes = [CREATE_BOX, CHANGE_BOX, CANCEL_BOX];
        for (i, a) in boxes.iter().enumerate() {
            assert!(a.x + a.w <= 640 && a.y + a.h <= 480, "button {i} is off screen");
            // The label's hundred-pixel column starts inside the box.
            assert!(a.x <= LABEL_X && LABEL_X < a.x + a.w, "button {i}'s label starts outside it");
            assert!(a.y <= LABEL_Y[i] && LABEL_Y[i] < a.y + a.h, "button {i}'s label is not in it");
            for (j, b) in boxes.iter().enumerate().skip(i + 1) {
                assert!(
                    a.y + a.h <= b.y || b.y + b.h <= a.y,
                    "buttons {i} and {j} overlap: a click would take the first",
                );
            }
            for &(hx0, hy0, hx1, hy1, t) in &RACK_HOTSPOTS {
                assert!(
                    a.x + a.w <= hx0 || hx1 <= a.x || a.y + a.h <= hy0 || hy1 <= a.y,
                    "button {i} overlaps rack {t}",
                );
            }
        }
        // 0x21E + 100 runs two pixels past the screen; the boxes stop at 634,
        // which is `Ui_DrawCentred` being given a column wider than the room
        // that is left rather than a coordinate we misread.
        assert_eq!(LABEL_X + LABEL_W - 640, 2, "the painter's column overhangs by two");
        assert_eq!(CREATE_BOX.x + CREATE_BOX.w, 634, "and the hotspot stops short of it");
    }

    /// The four buttons are inside the rack window, in a row, and none of them
    /// touches the corner picture that closes it.
    #[test]
    fn the_four_rack_buttons_are_inside_the_window_and_clear_of_the_ok() {
        let w = rack_window();
        for i in 0..4 {
            let b = button_box(i);
            assert!(w.contains(b.x, b.y), "button {i} starts outside the window");
            assert!(w.contains(b.x + b.w - 1, b.y + b.h - 1), "button {i} runs off it");
            assert!(
                b.x + b.w <= RACK_OK.x
                    || RACK_OK.x + RACK_OK.w <= b.x
                    || b.y + b.h <= RACK_OK.y
                    || RACK_OK.y + RACK_OK.h <= b.y,
                "button {i} overlaps the close button",
            );
            if i > 0 {
                assert!(button_box(i - 1).x + BUTTON_DIM <= b.x, "buttons {i} and {} overlap", i - 1);
            }
        }
        assert_eq!(BUTTON_FRAMES[0], 68, "record 0 carries the plus picture");
        assert_eq!(BUTTON_FRAMES[1], 66, "record 1 carries the minus");
        assert_eq!(BUTTONS[0], Button::EquipOne, "and record 0's body is the plus");
    }

    /// Every wall position is on the screen, and the six of them are the six
    /// weapon slots in order.
    #[test]
    fn the_weapons_on_the_walls_are_six_and_in_slot_order() {
        for (slot, &(frame, x, y)) in WALL.iter().enumerate() {
            assert_eq!(frame, slot, "wall record {slot} draws frame {frame}");
            assert!((0..640).contains(&x) && (0..480).contains(&y), "wall {slot} is off screen");
        }
    }

    /// The item sheets are five colours in six slots, and slot 0 is slot 1.
    #[test]
    fn a_realm_with_no_banner_gets_the_same_sheet_as_realm_one() {
        assert_eq!(items_sheet(0), items_sheet(1));
        assert_eq!(items_sheet(1), "Arm_it_r.pl8");
        assert_eq!(items_sheet(9), items_sheet(5), "out of range clamps to the last");
        let mut distinct: Vec<&str> = ITEM_SHEETS.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 5, "five colours");
    }
}
