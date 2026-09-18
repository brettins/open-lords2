//! **The map information panel** — `FUN_0041B032` (`0x0041B032`), `g_screenId`
//! `0x04`.
//!
//! ```c
//! void FUN_0041B032(void) {
//!   FUN_004B1DE0();                 /* an empty stub - 11 bytes, `return` */
//!   FUN_0045240A();                 /* dirty the whole 640 x 480          */
//!   if (g_pickedTileUnit == 0 && DAT_0052AFB4 == 0) FUN_0041BEFE();
//!   else                                            UnitPanel_Draw();
//!   FUN_00452160(1);
//!   DAT_004EB260 = 1;
//! }
//! ```
//!
//! * **`UnitPanel_Draw` (`0x0041B19D`)** — the unit half, four layouts by
//!   `unit.kind`: army, revolting peasants, merchant, transport.
//!
//! * **`FUN_0041BEFE`** — the tile half, which is itself only a layout chooser:
//!
//!   it computes the panel's top row, draws the frame and hands off to
//!   **`TileInfo_Draw` (`0x0041C208`)**. On farmland it also calls
//!   `FUN_0041C996`, **the field brush**.
//!
//! # `DAT_00553D2C` is a top row, and the panel's **bottom edge is pinned**
//!
//! Every y on this screen is `R * 16 + k` where `R` is `DAT_00553D2C`, and the
//! shell table's `0x04` row said so and stopped, refusing to invent a y. That
//! restraint was right and is now unnecessary, because the box arithmetic
//! closes:
//!
//! `top + height = 464` for **every** value of `R`, because the `R` terms
//! cancel. So `R` is the panel's top row in 16-pixel cells, the panel **grows
//! upward** as its content grows, and `DAT_005651C8` (`C`) is two extra cells
//! of head-room granted when the tile belongs to a county so the county's name
//! can be printed above the heading. [`Layout`] is that, and [`Layout::box_at`]
//! is the arithmetic.
//!
//! > *"`FUN_0041B032` draws no `Ui_DrawBox`: it paints over the campaign map
//! > and its two halves place their own lines."*
//!
//! **False.** Both halves open with `Ui_DrawBox`, at `0x0041B1D9` and
//! `0x0041C1B0`. The comment reads like somebody looked at the dispatcher's own
//! 79 bytes — which contain no drawing at all — and concluded the callees did
//! not either. `window: None` follows from it and is wrong the same way.
//!
//! # `L2.eng` 31/21, *"Morale"*, is dead text — and a `[V]` rested on it
//!
//! `docs/armies.md` §1 and `screens/map/mod.rs` both list *"31/21 Morale"* among
//! *"the army info panel's own field labels, drawn by `UnitPanel_Draw` right
//! next to the offsets"*, and `armies.md` then rests the **[V]** on unit
//! `+0x166 morale` on it.
//!
//! **Nothing in `Lords2.exe` ever draws group 31 index 21.** Group 31 has
//! exactly two consumers — this painter and `FUN_0041B081` — and their
//! reachable index sets are `{0, 2, 5, 8, 9, 12…16, 20, 22, 23, 24, 25, 27…31}`
//! and `{17, 18}`. `+0x166` is written by `Merchant_SpawnAll` and read by
//! nothing on this panel. So 31/21 is dead text exactly like 31/26, which
//! `armies.md` §3.4 already flags, and **the `[V]` on the morale field needs a
//! different source.** [`DEAD_LABELS`] carries all ten.
//!
//! `g_infoUnitButtons` (`0x004DC560`) is **kind 1** — left *press* — and
//! `g_infoFieldBrush` (`0x004DC4D0`) is **kind 3** — left *release*. A real
//! behavioural difference between the two halves of one screen, reproduced.
//!
//! ```text
//! 0x004DC560  in the field                0x004DC5A8  garrisoned
//!   (48,352)  Panel_MoveButton  0x004371CE   (48,352)  FUN_004374C4  LEAVE THE CASTLE
//!   (112,352) Panel_DisbandButton 0x0043733A (112,352) Panel_DisbandButton
//!   (176,352) Panel_SplitButton 0x004378B3   (176,352) Panel_SplitButton
//! ```
//!
//! **`FUN_004374C4` is unnamed and it is a real action.** It finds a free tile
//! near the castle with `Map_FindFreeTileNear`, moves the army out, clears both
//! halves of the garrison link and starts a battle if the county was besieged —
//! or destroys the army if no tile is free. It is *leave the castle*, and a
//! name like `Army_LeaveCastle` is earned.
//!
//! The panel's **geometry, its layout ladder and its input arms** are here, and
//! the unit half draws every value it has state for. The tile half's eighty-odd
//! `L2.eng` group 30 descriptions are a table this module carries the shape of
//! and not the contents: [`TileKind`] is the ladder, and the strings come out
//! of the player's own `L2.eng`. `Icon_tmp.pl8` **is** loaded by
//! `crate::shell::ShellAssets` and until now no frame of it was drawn anywhere;
//! [`ICON`] is what changes that.
//!
//! A player right-clicked a field and got *"the screen that left clicking should
//! bring … but the text for that field isn't filled in."* The panel had its box,
//! its brush and its county name, and not one word of `TileInfo_Draw`
//! (`0x0041C208`) for a `0x20` tile. [`draw_farmland`] is that arm, whole:
//!
//! * **the heading and the mode** — `DAT_004D2EC8`, sixteen bytes a terrain
//! value, gives `(heading, body, icon, mode)` and the painter draws
//!   `Eng_DrawString(30, heading)` then `Eng_DrawString(30, mode)` after it, both
//!   in `&g_fontHeading`: *"Farmland - Wheat."* [`FARM_TILE_INFO`] is the
//!   table, checked against the player's `Lords2.exe` by
//!   `tests/screens_info.rs`;
//! * **a body, or a report, by mode** — `0x13` *Wheat* runs
//!   `TileInfo_DrawGrain` (`0x0041CB3A`) and `0x15` *Cattle* runs
//!   `TileInfo_DrawHerd` (`0x0041D299`), and **neither draws the table's
//!   description**: 30/35 … 30/39 and 30/44 … 30/47 are read into `local_1c`
//!   and never reach a draw call. `0x12` *Fallow* swaps its body for 30/58 or
//!   30/34 on `g_optAdvancedFarming`; the rest wrap the table's body at
//!   `(0x68, row*16 + 100)`;
//! * **the icon**, `Icon_tmp.pl8` frame `icon` at `(0x28, row*16 + 0x60)`;
//! * and first, from `FUN_0041BEFE`, **the inset well** every tile panel sits
//!   in, `Ui_DrawInsetRect(0x20, row*16 + 0x38, 400, (0x18 - row) * 16)`.
//!
//! Groups 30, 77 and 22 are this arm's vocabulary and every word is drawn from
//! the player's own `L2.eng`, with our transcription only where the file has
//! none (`CLAUDE.md` rule 6).
//!
//! **The four weather and event figures are drawn** — `docs/decisions.md`
//! correction C199. County `+0x278` and
//! `+0x274` are the grain and herd a random event took or gave, `+0x24C` and
//! `+0x270` what the weather did (advanced farming only), and all four are
//! **imported** in `docs/stored-fields.json` — this comment claimed they were
//! excluded, which was true when it was written and two corrections out of
//! date by the time a player met the panel. They are `grain_event_change`,
//! `herd_event_change`, `grain_weather_change` and `herd_weather_change`, the
//! same four `Panel_JobGrain` and `Panel_JobCattle` draw on the job page.

mod types;
pub use types::*;
mod constants;
pub use constants::*;
mod screen;
pub use screen::*;
mod painters;
pub use painters::*;

use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

fn at_screen_edge(x: i32, y: i32) -> bool {
    x <= 0 || y <= 0 || x >= l2_view::canvas::WIDTH as i32 - 1 || y >= l2_view::canvas::HEIGHT as i32 - 1
}

pub struct InfoScreen {
    target: Target,
    status: String,
    press: Press,
}

/// **`DAT_004DD640` as a table, with the kind byte its one record carries.**
///
/// `Widget_Test` kind **4**, read out of `+0x0F` of `0x004DD640`.
///
/// `docs/arms.json` filed it `left-press`
/// wrong *kind*: kind 4 also shows the pressed picture and accepts a double
/// click as a press. The repeat is inert — `FUN_00438ACC` assigns the same
/// garrison every time — and that is a property of the handler, not of the
/// record.
///
/// **`FUN_00438A91` — the tile half's one widget**, and the `arm!` is its
/// marker.
fn garrison_widgets() -> [Widget; 1] {
    [Widget::new(GARRISON_WIDGET, crate::arm!("0x00438A91/info-garrison-widget", Repeat))]
}

