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
//! is why the sheets are loaded once.
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
//! **So `Army_RaiseConfirm` is a button on the armoury.**
//! anywhere on the raise-army screen: `0x17`'s widget table is the *Continue*
//! button and the mercenary tick and cross, and nothing else. `screens/army.rs`
//! carried a `RAISE` button of ours because this screen was a shell; it is gone,
//! and the door it stood in front of is this one.
//!
//! # `0x0D`, one weapon — and `Screen_Draw` has no arm for it
//!
//! **Verified against `Screen_Draw` (`0x0040F1A0`):**
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
//! frame too, so the room does not go stale behind it.
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
//! > `tools/oracle/widgets.js`'s *"68/66 [are] minus and plus"*. The
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
//! # The room moves, and this is what moves in it
//!
//! Three animations, all of them presentation and none of them a rule. They
//! were listed here and not drawn until a player said *"the animations when you
//! pick a weapon to assign during an army doesn't happen — usually a dude comes
//! and grabs a weapon."* He is describing the second one and he is right about
//! the trigger as well as the picture.
//!
//! * `Armoury_DrawTorches` (`0x00419243`) — `armtorch.pl8` at (0x9A, 0x76) and
//!   (0x19A, 0x76), the second thirteen frames behind the first: **two
//!   guttering torches**, [`TORCH_AT`];
//! * `Armoury_DrawWalker` (`0x004190DB`) with `FUN_004AABD8` (`0x004AABD8`) —
//!   **the soldier who walks over and takes the weapon**, [`Walker`];
//! * `DAT_005AEA48` — the weapon on `0x0D` turning through its 24 frames, which
//!   is [`Anim::weapon`] and is drawn by [`RackScreen`].
//!
//! All three run off `Tick_Pulses` (`0x004BBC80`) — a 20 ms gate feeding a
//! chain of dividers — and the armoury takes the 20 ms pulse and the 80 ms one.
//! [`Anim`] is that clock and [`overlay`] is the pass that draws it.
//!
//! **`Armoury_RestoreWalkerStrip` (`0x00418FC5`) erases; it does not draw.**
//! `docs/hypotheses.json` filed it as `Armoury_DrawPanel` and had the role
//! right and the verb wrong. It is why `Screen_Armoury` saves four strips of
//! backdrop at `y 0xD8`: the walker is a blit over a *restored* background
//!
//! have dirtied. [`walker_strip`] is that choice, and it says there why our own
//! full repaint means the call is not made.
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
//! the implementation of every primitive above.
//!
//! **Neither screen draws `L2.eng` 31/21 *"Morale"***, or any of group 31 —
//! checked by reading every one of the seven functions above.
//! grepping for the string. `docs/armies.md` rests a `[V]` on unit `+0x166`
//! against that label, and nothing in this module resources it.

mod wall;
pub use wall::*;
mod hotspots;
pub use hotspots::*;

mod walker;
pub use walker::*;
mod screen;
pub use screen::*;
mod tests;
pub use tests::*;

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

// --------------------------------------------------- the room, in motion

/// One fixed simulation tick in milliseconds — `main::TICK`.
///
/// **A constant, not a clock.** Nothing here asks how long a frame took; the
/// number exists so an interval the original states in milliseconds can be
/// converted to the whole ticks this crate is allowed to count.
/// `screens::map` carries its own copy for the same reason and both are the
/// same one number in `main.rs`; they are separate because neither module may
/// depend on the other and `docs/netcode.md` forbids either from reading a
/// clock instead.
pub const TICK_MS: u32 = 16;

/// **`Tick_Pulses` (`0x004BBC80`) is the interface's animation clock**, and the
/// armoury takes two of its eight pulses.
///
/// A 20 ms `timeGetTime` gate steps a counter; **every fourth step — 80 ms —
/// sets `g_pulse80`** and advances six further dividers. The armoury reads the
/// 20 ms pulse (`DAT_0058FCB0`) for the walking soldier's *position* and the
/// 80 ms one for every frame index on either screen:
///
/// | counter | wrap | what it drives |
/// |---|---|---|
/// | `DAT_0057CB10` | 8 | the soldier's walk cycle |
/// | `DAT_005AEA54` | 13 | the two torches, the second at `+13` |
/// | `DAT_005AEA48` | 24 | the weapon turning in the rack panel's well |
///
/// The last two are the *divider chain's own counters*, reused as frame
/// indices — so they wrap at 13 and 24,
/// and why `Armtorch.pl8` has exactly 26 frames and `Arm_*.pl8` exactly 24.
///
/// **Four documents called `Tick_Pulses` "once a frame and wrapped".** That
/// describes the call site. The rates are in the function, and this is where
/// they land on a screen.
pub const PULSE_MS: u32 = 20;

/// `if (3 < DAT_005AEB2C)` — four 20 ms steps make the 80 ms pulse.
pub const PULSE80_DIVIDER: u8 = 4;

/// `DAT_005AEA54`'s wrap: `if (0xC < n) n = 0`. `Armtorch.pl8` is 13 frames of
/// 77 × 49 followed by 13 of 77 × 48 — the two torches, and the second's
/// `+0x0D` is exactly the first block's length.
pub const TORCH_FRAMES: u8 = 13;

/// `DAT_005AEA48`'s wrap: `if (0x17 < n) n = 0`, and every `Arm_<weapon>.pl8`
/// holds exactly 24 frames of 100 × 100.
pub const WEAPON_FRAMES: u8 = 24;

/// `Armoury_DrawTorches` (`0x00419243`): `armtorch.pl8` at these two positions,
/// the second drawn at frame `+ TORCH_SECOND`.
pub const TORCH_SHEET: &str = "Armtorch.pl8";
pub const TORCH_AT: [(i32, i32); 2] = [(0x9A, 0x76), (0x19A, 0x76)];
pub const TORCH_SECOND: usize = 0x0D;

/// **The walking soldier's geometry**, all of it out of `Armoury_DrawWalker`
/// (`0x004190DB`) and `FUN_004AABD8` (`0x004AABD8`).
///
/// He starts off the left edge at `-0x50`, takes four pixels a 20 ms pulse and
/// the walk is over at `0x280`, one screen width. **Four pixels a pulse is not
/// 200 pixels a second**: `Tick_Pulses` resets its stamp to the frame that
/// fired, so a pulse is 20 ms rounded *up* to whole frames — 32 ms and 125
/// pixels a second on our 16 ms tick. See [`Anim::tick`].
pub const WALKER_START_X: i32 = -0x50;
pub const WALKER_END_X: i32 = 0x280;
pub const WALKER_STEP: i32 = 4;
pub const WALKER_Y: i32 = 0xD8;

/// **`g_armouryWalkStopX` (`0x004DE6F0`)** — where the soldier stops, by basket
/// slot.
///
/// Read against [`WALL`], the six weapons hanging on the walls: crossbow at
/// x 59 stops him at 45, mace at 157 at 120, sword at 496 at 490, pike at 199
/// at 170, bow at 373 at 380, armour at 290 at 270. **He walks to the weapon
/// and takes it off the wall.** `[V]` — two independent tables in the binary,
/// neither of which mentions the other, agreeing to within the width of a man.
pub const WALKER_STOP_X: [i32; 8] = [0, 45, 120, 490, 170, 380, 270, 50];

/// The soldier's three runs of frames, and the whole of the animation.
///
/// `Trp_xb_r.pl8` and its twenty-nine siblings hold **exactly 21 frames of
/// 89 × 158** — and 8 + 5 + 8 is 21, which is the artwork agreeing with the
/// arithmetic in `Armoury_DrawWalker` without either being asked. `158` is
/// `0x9E`, the height of the four strips `Screen_Armoury` saves, so a strip
/// is exactly one soldier tall. `[V]`
pub const WALK_PHASES: u8 = 8;
/// `DAT_0052F008 = DAT_005681F8 / 3 + 8` — five frames, each held three 80 ms
/// pulses. The counter starts at 1 and the run ends when it reaches 15, so it
/// is fourteen `g_pulse80`s — 1.1 s on a 20 ms pulse, 1.8 s on our 32 ms one.
pub const PICKUP_FIRST: usize = 8;
pub const PICKUP_LAST: usize = 0x0C;
pub const PICKUP_HOLD: u8 = 3;
/// `DAT_0052F008 = DAT_0057CB10 + 0xD` — the same eight phases, carrying it.
pub const CARRY_FIRST: usize = 0x0D;

/// **`Screen_Armoury`'s four saved strips**, `(x, width)` at `y = `
/// [`WALKER_Y`] and height [`STRIP_H`].
///
/// `FUN_004B3F0A` copies **dwords**, so `g_spriteWidth` of `0x3C` is 240
/// pixels and `0x28` is 160; the second argument is `640 - width * 4`, the
/// row remainder, which is what pins the unit down. The four buffer offsets
/// step by `0x2508` in `undefined4` units — `60 × 158` dwords — which is the
/// same number from the other end.
pub const STRIPS: [(i32, i32); 4] = [(0, 240), (0xA0, 240), (0x140, 240), (0x1E0, 160)];
pub const STRIP_H: i32 = 0x9E;

/// **`Armoury_RestoreWalkerStrip` (`0x00418FC5`) — it erases, it does not
/// draw.** `docs/hypotheses.json` had it as `Armoury_DrawPanel`, which had the
/// role right and the verb wrong.
///
/// The four-way choice is on the soldier's own x: under `0xA0` the first strip,
/// under `0x140` the second, under `0x1E0` the third, otherwise the fourth. So
/// **the walker is a blit over a restored background, not a composited
/// sprite** — that is why the strips exist and why there are four of them
///
///
/// **It is not called from [`overlay`], and this says so.
/// reader to wonder.** Our page is repainted whole every frame (the original
/// paints `Screen_Armoury` once and never clears again), so the erase has
/// already happened by the time the walker is drawn. What is reproduced here is
/// the *shape* and the rectangle, which
/// `crates/l2-game/tests/armoury/main.rs` asserts covers him — all but the five
/// pixels of his right shoulder that stand outside it at each band boundary,
/// which is the original's own smear and is measured there.
pub fn walker_strip(x: i32) -> Rect {
    let (sx, w) = match x {
        _ if x < 0xA0 => STRIPS[0],
        _ if x < 0x140 => STRIPS[1],
        _ if x < 0x1E0 => STRIPS[2],
        _ => STRIPS[3],
    };
    Rect::new(sx, WALKER_Y, w, STRIP_H)
}


