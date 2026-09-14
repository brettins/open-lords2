//! **The options panels** — `g_screenId` `0x39`, `0x42`, `0x43` and `0x31` —
//! and a fifth page that is ours.
//!
//! #
//!
//! The premise this module was written against was *"find the options screen in
//! the binary and reproduce it"*. There isn't one. The Options drop-down
//! (`L2.eng` group 2) opens **three** separate modal panels, a fourth hangs off
//! the Help menu, and two more entries open the shared value spinner — six
//! controls behind one menu, with three `g_screenId` values between them:
//!
//! | menu item | screen | painter | `L2.eng` |
//! |---|---|---|---|
//! | Options ▸ Advanced | `0x39` | `Screen_AdvancedOptions` `0x00414F68` | 50 |
//! | Options ▸ Sounds | `0x42` | `Screen_SoundOptions` `0x0041515C` | 51 |
//! | Options ▸ Display | `0x43` | `Screen_DisplayOptions` `0x004152EA` | 52 |
//! | Options ▸ Game Speed | `0x21` | `Screen_SliderBox` `0x0040CD58` | 12/1 |
//! | Options ▸ Scroll Speed | `0x21` | the same | 12/2 |
//! | Help ▸ Game Help | `0x31` | `Screen_HelpOptions` `0x004154EA` | 45 |
//!
//! All four panels were rows of [`crate::screens::shells::SHELLS`] until now —
//! the original's window, the original's words, and a click that closed them
//! again. **[`Shell::unfinished`] said exactly what was missing in each case**
//! (*"the two values"), which
//! is what a shell is for; this module is those four sentences answered.
//!
//! [`Shell::unfinished`]: crate::screens::shells::Shell::unfinished
//!
//! # The four painters, address by address
//!
//! They are one shape four times over: `FUN_004093E0` (which is
//! `Ui_DrawBoxBorder(1, …)` plus `Ui_DrawBoxInterior` inset one cell), a
//! heading, N label rows, N state words out of group 18 or 19, and
//! `Ui_OkButton`. Every coordinate below is a literal in the painter.
//!
//! ```text
//! Screen_HelpOptions():                                         0x004154EA
//!   g_helpOptWidgetCount = 3
//!   FUN_004093E0(0x60, 0x80, 0x16, 0x0B)     border set 1 at (96, 128) 352x176
//!   Eng_DrawString(45, 3, 0x80, 0x100, body)    "Start game help"  (128, 256)
//!   Ui_OkButton(0x194, 0x106, 0)                                   (404, 262)
//!   Eng_DrawString(45, 0, 0x80, 0x94,  heading) "Help Options"     (128, 148)
//!   Eng_DrawString(45, 1, 0x80, 0xC0,  body)    "Tip screens"      (128, 192)
//!   Eng_DrawString(45, 2, 0x80, 0xE0,  body)    "Tool tips"        (128, 224)
//!   Eng_DrawString(18, tipScreens ? 0 : 1, 0x120, 0xC0, body)      (288, 192)
//!   Eng_DrawString(18, toolTips   ? 0 : 1, 0x120, 0xE0, body)      (288, 224)
//!
//! Screen_AdvancedOptions():                                     0x00414F68
//!   FUN_004093E0(0x30, 0x60, 0x18, 0x0D)      set 1 at (48, 96)     384x208
//!   Eng_DrawString(50, 0, 0x40, 0x74,  heading) "Advanced options." (64, 116)
//!   Eng_DrawString(50, 1, 0x60, 0xA0,  body)   "Advanced farming"   (96, 160)
//!   Eng_DrawString(50, 2, 0x60, 0xC0,  body)   "Army foraging"      (96, 192)
//!   Eng_DrawString(50, 3, 0x60, 0xE0,  body)   "Exploration"        (96, 224)
//!   Eng_DrawString(50, 4, 0x60, 0x100, body)   "Fight humans only?" (96, 256)
//!   Eng_DrawString(18, advancedFarming ? 0 : 1, 0x140, 0xA0, body) (320, 160)
//!   Eng_DrawString(18, armiesEat       ? 0 : 1, 0x140, 0xC0, body) (320, 192)
//!   Eng_DrawString(18, exploration     ? 0 : 1, 0x140, 0xE0, body) (320, 224)
//!   Eng_DrawString(18, fightHumansOnly ? 1 : 0, 0x140, 0x100, body)  <- INVERTED
//!   g_advancedOptWidgetCount = 4
//!   Ui_OkButton(0x188, 0x100, 0)                                  (392, 256)
//!
//! Screen_SoundOptions():                                        0x0041515C
//!   FUN_004093E0(0x30, 0x60, 0x18, 0x0C)      set 1 at (48, 96)     384x192
//!   Eng_DrawString(51, 0, 0x40, 0x74, heading) "Sounds"             (64, 116)
//!   Eng_DrawString(51, 1, 0x60, 0xA0, body)    "Music"              (96, 160)
//!   Eng_DrawString(51, 2, 0x60, 0xC0, body)    "Sound effects"      (96, 192)
//!   Eng_DrawString(51, 3, 0x60, 0xE0, body)    "Speech"             (96, 224)
//!   Eng_DrawString(19, music   ? 0 : 1, 0x140, 0xA0, body)         (320, 160)
//!   Eng_DrawString(19, effects ? 0 : 1, 0x140, 0xC0, body)         (320, 192)
//!   Eng_DrawString(19, speech  ? 0 : 1, 0x140, 0xE0, body)         (320, 224)
//!   g_soundOptWidgetCount = 3
//!   Ui_OkButton(0x188, 0xF0, 0)                                    (392, 240)
//!
//! Screen_DisplayOptions():                                      0x004152EA
//!   FUN_004093E0(0x30, 0x90, 0x18, 0x0A)      set 1 at (48, 144)    384x160
//!   Eng_DrawString(52, 2, 0x60, 0xF0, body)    "Full screen"        (96, 240)
//!   Eng_DrawString(18, fullScreen == 1 ? 0 : 1, 0x140, 0xF0, body) (320, 240)
//!   if (!fullScreen)
//!     Eng_DrawString(52, 3, 0x48, 0x108, body) "(F5 key re-sizes…)" (72, 264)
//!   g_displayOptWidgetCount = 2
//!   Ui_OkButton(0x188, 0x100, 0)                                   (392, 256)
//!   Eng_DrawString(52, 0, 0x40, 0xA4, heading) "Display options"    (64, 164)
//!   Eng_DrawString(52, 1, 0x60, 0xD0, body)    "Animations"         (96, 208)
//!   Eng_DrawString(19, animations ? 0 : 1, 0x140, 0xD0, body)      (320, 208)
//! ```
//!
//! The Display painter draws its rows **out of order** — the second row and its
//! state word first, then the F5 note, then the widget count and the OK button,
//! and only then the heading and the first row. Nothing overlaps, so the order
//! does not show; it is transcribed as it is so that a future reader comparing
//! this listing with `fn.js` does not think a line is missing.
//!
//! # The state word on row four of the Advanced panel is **inverted**
//!
//! Three of that panel's four rows read `if (flag == 0) "No" else "Yes"`. The
//! fourth, *"Fight humans only?"*, reads `if (g_optFightHumansOnly == 0) "Yes"
//! else "No"` — the opposite. `[V]`, and it is the binary's, not a slip here:
//! `FUN_004A6A30` auto-resolves a battle the local player is not in **when the
//! byte is 0**, so the byte is really *"fight everything"* and the panel is
//! printing its negation. [`Setting::FightHumansOnly`] keeps the byte and turns
//! the sense round in exactly one place.
//!
//! # Every widget count is a variable, and all four are honest
//!
//! `Widget_Draw` is passed `g_helpOptWidgetCount`, `g_advancedOptWidgetCount`,
//! `g_soundOptWidgetCount` and `g_displayOptWidgetCount`
//! which is the shape that hid `g_sendSuppliesWidgets`' cut sheep row. **It is
//! not hiding anything here.** `xref.js touches` finds exactly three functions
//! per count — the painter, which writes it, and `Screen_DrawWidgets` and
//! `Screen_HandleInput` — and the tables decoded out of
//! `.data` are
//!
//! ```text
//!   g_advancedOptWidgets 0x004DDC10  4 records, then g_soundOptWidgets    count 4
//!   g_soundOptWidgets    0x004DDC70  3 records, then g_displayOptWidgets  count 3
//!   g_displayOptWidgets  0x004DDCB8  2 records, then g_helpOptWidgets     count 2
//!   g_helpOptWidgets     0x004DDCE8  3 records, then an unnamed table     count 3
//! ```
//!
//! The one thing that turned up is **next door**: `0x004DDD30`, immediately
//! after `g_helpOptWidgets`' three records and before `g_saveLoadWidgets` at
//! `0x004DDD78`, holds **three more 24-byte records** — `(376, 176)`,
//! `(376, 216)` and `(376, 256)`, frame 19, kind 4, handlers `0x0043441C`,
//! `0x004344BA` and `0x004344D6`. The last two are one line each,
//! `g_confirmAnswer = 1` and `g_confirmAnswer = 0`; the first is the callback
//! `Menu_Quit` hands to `Ui_OpenConfirm(0, …)`, *"Exit the game?"*. **No
//! `Widget_Draw` or `Widget_Test` call site anywhere in the image names
//! `0x004DDD30`** — `grep` over the whole decompilation finds the address only
//! in `.data` — so those three buttons exist, cost 72 bytes, and are drawn by
//! nothing. Recorded here
//!
//! Reading `FUN_0043441C` for that settles something else, which belongs to
//! screen `0x45`
//! records it: **answering *yes* to *"Exit the game?"* does not exit.** While
//! `DAT_0053F644` is under 3 it increments it, sets `g_screenId = 0x45` and
//! checks that `lom.256` is on disk — the Lords of Magic advertisement — and
//! only sets `g_quitRequest` if the file is *missing*. `Screen_FrameInput`'s
//! whole `0x45` arm is *any mouse release sets `g_quitRequest = 1`*, so **the
//! advertisement is the last thing the game shows and a click on it is the
//! exit.** `[V]` In multiplayer (`DAT_00568464`) it leaves the net game and
//! goes to `0x1F` page 1 instead, and *no* to the box restores
//! `g_screenIdSaved`.
//!
//! # The rows, and where every number comes from
//!
//! Each panel is a `Ui_DrawBox`, a heading, N label rows, and one **24 × 24
//! widget** per row taken from a 24-byte widget table in `.data`, hit-tested at
//! offset (0, 0) so the record's `x`/`y` are absolute screen pixels. The state
//! word is a second string drawn at a second x. `[V]` — the tables were read out
//! of `.data` and their callbacks match the painters' rows one for one:
//!
//! | screen | widget table | rows |
//! |---|---|---|
//! | `0x39` | `g_advancedOptWidgets` `0x004DDC10` | 4 |
//! | `0x42` | `g_soundOptWidgets` `0x004DDC70` | 3 |
//! | `0x43` | `g_displayOptWidgets` `0x004DDCB8` | 2 |
//! | `0x31` | `g_helpOptWidgets` `0x004DDCE8` | 3 |
//!
//! **Two independent readings agree on every coordinate.** The label positions
//! in [`Page::rows`] are the ones `shells.rs` already carried, read from the
//! painters months ago; the widget boxes come from a fresh read of the four
//! widget tables. They were derived separately and they line up.
//!
//! # The state words come from two different groups on one panel
//!
//! Group 18 is `Yes` / `No` / `Cancel` and group 19 is `On` / `Off` / `Cancel`,
//! and **the Display panel uses both** — *Animations* is On/Off and *Full
//! screen* is Yes/No. That is not tidy and it is what the painter does. `[V]`
//!
//! # Two rows are drawn and cannot be driven, and they say so
//!
//! *Full screen* and *Start game help* are the original's, and neither is ours
//! to honour: this engine is a `winit` window, and `l2help.hlp` is a Windows 3.1
//! help file. They are drawn because the panel has them — a panel that quietly
//! lost two rows would be a panel nobody could check against a screenshot — and
//! they are drawn in the disabled colour with the reason on the page. See
//! [`Row::supported`].
//!
//! # The quirks page is **ours**
//!
//! [`Page::Quirks`] has no `g_screenId`, no painter and no `L2.eng` group,
//! because the original has no such page and could not: to it these are not
//! settings. It is marked on screen, in our own font,
//! [`crate::screens::index`] is. `docs/decisions.md` C62.
//!
//! # The hit boxes
//!
//! `crates/l2-game/src/screens/map/mod.rs`'s header records three wrong-screen bugs
//! that reached a player in one evening, all of them near-misses falling through
//! to county selection. **A page of checkboxes is a page of small hotspots and
//! has the same exposure**, so this module takes the other side of every
//! judgement:
//!
//! * only the 24 × 24 widget boxes and the close button are hot. A row's *label*
//! is not a hotspot,
//! * **every** click inside or outside the window is consumed
//!   ([`Transition::Stay`]), never [`Transition::Pass`]. A player has already
//!   reported that *"clicking anywhere inside a window used to close it"*, and
//!   the fall-through version of that fault is worse: it acts on the map behind;
//! * closing is the close button **on the release**, or a right release — the
//!   original's own two (`Screen_FrameInput` tests `g_mouseRightReleased`
//!   before `Ui_OkButtonClicked()` on all four ids, and `L2.eng` group 12 index
//!   0 is *"Click Right to Exit"*) — and `Escape`, which is ours.
//!
//! # Every row is a delayed press
//!
//! **All twelve widget records carry kind 5** at `+0x0F`, so a row's picture
//! goes down on the press and its `Opt_Toggle*` runs **twenty frames later**,
//! out of `Widget_Test`'s countdown. Ours acted on the click, which is the
//! gauntlets' defect on a screen that had no `docs/arms.json` record to catch
//! it. [`Row::kind`] declares it and [`Press`] does the rest; the handlers are
//! in [`toggle`] and `OptionsScreen::fire`.
//!
//! **What does the flip change?** Three rows reach nothing in this engine —
//! *Exploration*, *Animations* and *Tool tips* flip a field no rule and no
//! painter reads — and the table on [`toggle`] says which is which, so that a
//! row that looks honoured and is not can be found by reading one place.

mod page;
pub use page::*;
mod words;
pub use words::*;
mod screen;
pub use screen::*;

use l2_view::Canvas;

use crate::game::PRESENTATION;
use crate::input::{Event, Key, Rect};
use crate::press::{Kind, Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

// ---------------------------------------------------------------------------
// The pages
// ---------------------------------------------------------------------------

/// Which options panel. Four of the original's and one of ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    /// `0x39` — *"Advanced options."* The three rule switches plus *Fight
    /// humans only?*.
    Advanced,
    /// `0x42` — *"Sounds"*.
    Sound,
    /// `0x43` — *"Display options"*.
    Display,
    /// `0x31` — *"Help Options"*.
    Help,
    /// **Ours.** The quirks page; see the module documentation.
    Quirks,
}

// ---------------------------------------------------------------------------
// The quirk list, which spans two homes
// ---------------------------------------------------------------------------

/// Where a quirk's flag
///
/// **Two homes
/// ever meet.** `docs/bugs.md` §6.3a: *if flipping it can change a number in a
/// saved game it is behavioural; if it can only change which pixels are painted
/// from the same numbers it is presentation.* A behavioural quirk is on
/// [`l2_kingdom::kingdom::Options`], in the save body and the lockstep digest; a
/// presentation quirk is on [`Assets`], where the simulation cannot see it at
/// all. See [`QuirkRow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    /// A rule variation. `l2_net::Quirk`, on `Options`.
    Behavioural(l2_kingdom::Quirk),
    /// Only pixels. A field of [`crate::game::Quirks`], named by the string in
    /// [`crate::game::PRESENTATION`].
    Presentation(&'static str),
}

/// One switchable defect, as the quirks page shows it.
#[derive(Debug, Clone, Copy)]
pub struct QuirkRow {
    /// Its `docs/bugs.md` entry — `"B1"`, `"B11a"`.
    pub entry: &'static str,
    pub home: Home,
}

/// **Every switchable quirk, in one list, across both homes.**
///
/// Behavioural first in `l2_net::Quirk::ALL`'s order, then presentation in
/// `crate::game::PRESENTATION`'s. Both are fixed slices walked by index —
/// nothing here is a map and nothing is iterated in hash order
/// (`docs/netcode.md` D-4). The order is stable, so the page's rows do not move
/// under a player between one frame and the next.
pub fn quirk_rows() -> Vec<QuirkRow> {
    let mut rows: Vec<QuirkRow> = l2_kingdom::Quirk::ALL
        .iter()
        .map(|q| QuirkRow { entry: q.entry(), home: Home::Behavioural(*q) })
        .collect();
    rows.extend(
        PRESENTATION
            .iter()
            .map(|(field, entry)| QuirkRow { entry, home: Home::Presentation(field) }),
    );
    rows
}

/// Whether this defect is currently reproduced.
pub fn quirk_reproduced(row: QuirkRow, game: &crate::Game) -> bool {
    match row.home {
        Home::Behavioural(q) => game.kingdom.options.quirks.reproduces(q),
        Home::Presentation(field) => !game.presentation_quirks.is_fixed(field),
    }
}

/// Reproduce this defect, or do not.
pub fn set_quirk(row: QuirkRow, reproduced: bool, game: &mut crate::Game) {
    match row.home {
        Home::Behavioural(q) => game.kingdom.options.quirks.set_reproduced(q, reproduced),
        Home::Presentation(field) => game.presentation_quirks.set_fixed(field, !reproduced),
    }
}

/// **The group switch's state, across both homes.**
///
/// The user asked for *"a group checkbox for 'turn off original game's bugs'
/// which toggles them all at once, while letting people still check/uncheck
/// individuals"*, and a control that can say *all*, *none* or *some* is
/// therefore tri-state. `l2_net::Group` is the same three values for the
/// behavioural half alone; this is the answer over both halves, which is the
/// only one a player should ever be shown — the split between the two homes
/// exists for the netcode's sake and is not something the interface may leak.
pub fn quirk_group(game: &crate::Game) -> l2_net::Group {
    let rows = quirk_rows();
    if rows.is_empty() {
        // No switchable quirk exists at all. "All reproduced" is the honest
        // answer to an empty set: nothing has been turned off.
        return l2_net::Group::AllReproduced;
    }
    let mut any_on = false;
    let mut any_off = false;
    for row in &rows {
        if quirk_reproduced(*row, game) {
            any_on = true;
        } else {
            any_off = true;
        }
    }
    match (any_on, any_off) {
        (true, false) => l2_net::Group::AllReproduced,
        (false, true) => l2_net::Group::AllFixed,
        _ => l2_net::Group::Mixed,
    }
}

/// Set every quirk in both homes at once. What the parent checkbox does.
///
/// It does **not** remember what the children were: a parent that restored a
/// previous mixture would be a fourth state the checkbox cannot show.
pub fn set_all_quirks(reproduced: bool, game: &mut crate::Game) {
    for row in quirk_rows() {
        set_quirk(row, reproduced, game);
    }
}

/// How many quirks are reproduced, of how many there are, across both homes.
pub fn quirk_tally(game: &crate::Game) -> (usize, usize) {
    let rows = quirk_rows();
    let on = rows.iter().filter(|r| quirk_reproduced(**r, game)).count();
    (on, rows.len())
}

/// The parent's three states as the box's three fills.
fn tri(group: l2_net::Group) -> u8 {
    match group {
        l2_net::Group::AllReproduced => 1,
        l2_net::Group::AllFixed => 0,
        l2_net::Group::Mixed => 2,
    }
}

/// One line about a quirk, for the page.
fn summary_of(row: QuirkRow) -> &'static str {
    match row.home {
        Home::Behavioural(q) => q.summary(),
        // A presentation quirk's own field name is the best short label there is
        // until `PRESENTATION` carries one; the entry id is printed beside it.
        Home::Presentation(field) => field,
    }
}

