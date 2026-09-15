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
//! Three of that panel's four rows read `if (flag == 0) "No" else "Yes"`. The
//! fourth, *"Fight humans only?"*, reads `if (g_optFightHumansOnly == 0) "Yes"
//! else "No"` — the opposite. `[V]`, and it is the binary's, not a slip here:
//!
//! `FUN_004A6A30` auto-resolves a battle the local player is not in **when the
//! byte is 0**, so the byte is really *"fight everything"* and the panel is
//! printing its negation. [`Setting::FightHumansOnly`] keeps the byte and turns
//! the sense round in exactly one place.
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
//! Group 18 is `Yes` / `No` / `Cancel` and group 19 is `On` / `Off` / `Cancel`,
//! and **the Display panel uses both** — *Animations* is On/Off and *Full
//! screen* is Yes/No. That is not tidy and it is what the painter does. `[V]`
//!
//! [`Page::Quirks`] has no `g_screenId`, no painter and no `L2.eng` group,
//! because the original has no such page and could not: to it these are not
//! settings. It is marked on screen, in our own font,
//! [`crate::screens::index`] is. `docs/decisions.md` C62.
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
//! **All twelve widget records carry kind 5** at `+0x0F`, so a row's picture
//! goes down on the press and its `Opt_Toggle*` runs **twenty frames later**,
//! out of `Widget_Test`'s countdown. Ours acted on the click, which is the
//! gauntlets' defect on a screen that had no `docs/arms.json` record to catch
//! it. [`Row::kind`] declares it and [`Press`] does the rest; the handlers are
//! in [`toggle`] and `OptionsScreen::fire`.

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


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Advanced,
    Sound,
    Display,
    Help,
    Quirks,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    Behavioural(l2_kingdom::Quirk),
    Presentation(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct QuirkRow {
    pub entry: &'static str,
    pub home: Home,
}

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

pub fn quirk_reproduced(row: QuirkRow, game: &crate::Game) -> bool {
    match row.home {
        Home::Behavioural(q) => game.kingdom.options.quirks.reproduces(q),
        Home::Presentation(field) => !game.presentation_quirks.is_fixed(field),
    }
}

pub fn set_quirk(row: QuirkRow, reproduced: bool, game: &mut crate::Game) {
    match row.home {
        Home::Behavioural(q) => game.kingdom.options.quirks.set_reproduced(q, reproduced),
        Home::Presentation(field) => game.presentation_quirks.set_fixed(field, !reproduced),
    }
}

pub fn quirk_group(game: &crate::Game) -> l2_net::Group {
    let rows = quirk_rows();
    if rows.is_empty() {
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

pub fn set_all_quirks(reproduced: bool, game: &mut crate::Game) {
    for row in quirk_rows() {
        set_quirk(row, reproduced, game);
    }
}

pub fn quirk_tally(game: &crate::Game) -> (usize, usize) {
    let rows = quirk_rows();
    let on = rows.iter().filter(|r| quirk_reproduced(**r, game)).count();
    (on, rows.len())
}

fn tri(group: l2_net::Group) -> u8 {
    match group {
        l2_net::Group::AllReproduced => 1,
        l2_net::Group::AllFixed => 0,
        l2_net::Group::Mixed => 2,
    }
}

fn summary_of(row: QuirkRow) -> &'static str {
    match row.home {
        Home::Behavioural(q) => q.summary(),
        Home::Presentation(field) => field,
    }
}

