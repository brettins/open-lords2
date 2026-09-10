//! **The options panels** — `g_screenId` `0x39`, `0x42`, `0x43` and `0x31` —
//! and a fifth page that is ours.
//!
//! # There is no options *screen*
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
//! (*"the two values, and the F5 note that only shows in windowed mode"*), which
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
//! `g_soundOptWidgetCount` and `g_displayOptWidgetCount` rather than a literal,
//! which is the shape that hid `g_sendSuppliesWidgets`' cut sheep row. **It is
//! not hiding anything here.** `xref.js touches` finds exactly three functions
//! per count — the painter, which writes it, and `Screen_DrawWidgets` and
//! `Screen_HandleInput`, which only read it — and the tables decoded out of
//! `.data` are exactly as long as the value written:
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
//! nothing. Recorded here rather than guessed at, and not reproduced.
//!
//! Reading `FUN_0043441C` for that settles something else, which belongs to
//! screen `0x45` rather than here and is written down because nothing else
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
//! settings. It is marked on screen, in our own font, exactly as
//! [`crate::screens::index`] is. `docs/decisions.md` C62.
//!
//! # The hit boxes
//!
//! `crates/l2-game/src/screens/map.rs`'s header records three wrong-screen bugs
//! that reached a player in one evening, all of them near-misses falling through
//! to county selection. **A page of checkboxes is a page of small hotspots and
//! has the same exposure**, so this module takes the other side of every
//! judgement:
//!
//! * only the 24 × 24 widget boxes and the close button are hot. A row's *label*
//!   is not a hotspot, because the original's is not;
//! * **every** click inside or outside the window is consumed
//!   ([`Transition::Stay`]), never [`Transition::Pass`]. A player has already
//!   reported that *"clicking anywhere inside a window used to close it"*, and
//!   the fall-through version of that fault is worse: it acts on the map behind;
//! * closing is the close button, `Escape`, or a right-click — the original's
//!   own three (`Screen_FrameInput` tests `g_mouseRightReleased` before
//!   `Ui_OkButtonClicked()` on all five ids, and `L2.eng` group 12 index 0 is
//!   *"Click Right to Exit"*).

use l2_view::Canvas;

use crate::game::PRESENTATION;
use crate::input::{Event, Key, Rect};
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

/// `L2.eng` group 50 — *"Advanced options."* and its four rows. **Verified
/// against the words**, not against the indices existing: index 0 is
/// *"Advanced options."*, 1 *"Advanced farming"*, 2 *"Army foraging"*, 3
/// *"Exploration"*, 4 *"Fight humans only?"*, and the group holds exactly five
/// strings.
pub const GROUP_ADVANCED: usize = 50;
/// `L2.eng` group 51 — *"Sounds"*, *"Music"*, *"Sound effects"*, *"Speech"*.
/// Exactly four strings. Verified against the words.
pub const GROUP_SOUND: usize = 51;
/// `L2.eng` group 52 — *"Display options"*, *"Animations"*, *"Full screen"*,
/// *"(F5 key re-sizes window to 640x480)"*. Exactly four. Verified against the
/// words.
pub const GROUP_DISPLAY: usize = 52;
/// `L2.eng` group 45 — *"Help Options"*, *"Tip screens"*, *"Tool tips"*,
/// *"Start game help"*. Exactly four. Verified against the words.
pub const GROUP_HELP: usize = 45;

/// `L2.eng` group 18 — **`Yes` / `No` / `Cancel`**, three strings. Index 2 is
/// drawn by none of these four painters.
pub const GROUP_YES_NO: usize = 18;
/// `L2.eng` group 19 — **`On` / `Off` / `Cancel`**, three strings, and again
/// index 2 is unused here.
pub const GROUP_ON_OFF: usize = 19;

/// Group 52 index 3, the windowed-mode hint, and the one row of these four
/// panels that is drawn conditionally.
pub const DISPLAY_F5_NOTE: usize = 3;
/// The F5 note's own baseline — `Eng_DrawString(52, 3, 0x48, 0x108, body)`,
/// which is 24 pixels left of every other row on the panel.
pub const DISPLAY_F5_NOTE_AT: (i32, i32) = (0x48, 0x108);

/// A row's state word comes from one of two `L2.eng` groups, and one panel uses
/// both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    /// Group 18 — `Yes` / `No`.
    YesNo,
    /// Group 19 — `On` / `Off`.
    OnOff,
}

impl Words {
    pub fn group(self) -> usize {
        match self {
            Words::YesNo => GROUP_YES_NO,
            Words::OnOff => GROUP_ON_OFF,
        }
    }

    /// Index 0 is the affirmative and index 1 the negative in both groups.
    pub fn index(self, on: bool) -> usize {
        usize::from(!on)
    }
}

/// What a row switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// `g_optAdvancedFarming` (`0x0053F25C`), `Opt_ToggleAdvancedFarming`
    /// (`0x00434556`).
    AdvancedFarming,
    /// `g_optArmiesEat` (`0x0053F260`), `Opt_ToggleArmyForaging`
    /// (`0x004345D0`).
    ArmyForaging,
    /// `g_optExploration` (`0x0053F264`), `Opt_ToggleExploration`
    /// (`0x00434693`).
    Exploration,
    /// `g_optFightHumansOnly` (`0x0053F284`), `Opt_ToggleFightHumansOnly`
    /// (`0x0043470D`). **Stored inverted** — the byte is 0 when the option
    /// displays *Yes* — and kept as the byte so the sense cannot drift from the
    /// binary's.
    FightHumansOnly,
    /// `g_optMusic` (`0x0053F218`), `Opt_ToggleMusic` (`0x004349A4`).
    Music,
    /// `g_optSoundEffects` (`0x0053F214`), `Opt_ToggleSoundEffects`
    /// (`0x00434A29`).
    SoundEffects,
    /// `g_optSpeech` (`0x0053F20C`), `Opt_ToggleSpeech` (`0x00434A9A`).
    Speech,
    /// `g_optAnimations` (`0x0053F248`), `Opt_ToggleAnimations`
    /// (`0x00434AD5`).
    Animations,
    /// `g_optFullScreen` (`0x0053F228`), `Opt_ToggleFullScreen`
    /// (`0x00434B10`). **Not honoured** — see [`Row::supported`].
    FullScreen,
    /// `g_optTipScreens` (`0x0053F24C`), `Opt_ToggleTipScreens`
    /// (`0x00434787`).
    TipScreens,
    /// `g_optToolTips` (`0x0053F250`), `Opt_ToggleToolTips` (`0x004347C7`).
    ToolTips,
    /// `Opt_GameHelpContents` (`0x00434942`) — `WinHelpA(hwnd, "l2help.hlp",
    /// HELP_CONTENTS, 1)`. **A button, not a toggle**, and not honoured.
    StartGameHelp,
}

/// One row of one panel.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    /// The `L2.eng` index within the page's group. Index 0 is the heading, so a
    /// row is never 0.
    pub label: usize,
    /// The label's baseline, from the painter.
    pub label_at: (i32, i32),
    /// The state word's x. Its y is the label's.
    pub state_x: i32,
    /// The 24 × 24 widget's top-left, from the widget table in `.data`.
    pub widget_at: (i32, i32),
    pub words: Words,
    pub setting: Setting,
}

impl Row {
    /// The widget's hit box — 24 × 24 at the record's own corner, and **the only
    /// hot rectangle on the row**.
    pub fn hit(&self) -> Rect {
        Rect::new(self.widget_at.0, self.widget_at.1, WIDGET, WIDGET)
    }

    /// Whether this engine can honour the row.
    ///
    /// Two cannot, and they are drawn anyway. *Full screen* is a DirectDraw
    /// mode switch in a program that is a `winit` window — and in the original
    /// it is already a permanent *Yes* on any desktop that is not 8bpp, because
    /// `Display_Init` (`0x0042E5C0`) forces it. *Start game help* opens
    /// `l2help.hlp` through `WinHelpA`, a Windows 3.1 help file and a Windows
    /// API that has not shipped since Vista.
    ///
    /// **Drawn rather than dropped**, in the disabled colour with the reason
    /// under the panel: a reproduction that silently loses two of its eleven
    /// rows is a reproduction nobody can check against a screenshot.
    pub fn supported(&self) -> bool {
        !matches!(self.setting, Setting::FullScreen | Setting::StartGameHelp)
    }
}

/// `Ui_OkButton`'s corner picture, and every options widget, is 24 × 24.
const WIDGET: i32 = 24;

/// Screen `0x39` — `Screen_AdvancedOptions` (`0x00414F68`), `L2.eng` group 50.
///
/// **Four rows, not three.** `docs/bugs.md` §6.4 says *"the original ships three
/// behaviour switches"* and names Advanced Farming, Foraging and Exploration.
/// Group 50 holds **five** strings — a heading and four rows — and the widget
/// table `g_advancedOptWidgets` holds **four** records. *"Fight humans only?"*
/// is the fourth, it is on this panel, and it changes a rule
/// (`FUN_004A6A30` auto-resolves a battle the local player is not in when the
/// byte is 0). `[V]`, both counts.
const ADVANCED: &[Row] = &[
    Row {
        label: 1,
        label_at: (0x60, 0xA0),
        state_x: 0x140,
        widget_at: (280, 156),
        words: Words::YesNo,
        setting: Setting::AdvancedFarming,
    },
    Row {
        label: 2,
        label_at: (0x60, 0xC0),
        state_x: 0x140,
        widget_at: (280, 188),
        words: Words::YesNo,
        setting: Setting::ArmyForaging,
    },
    Row {
        label: 3,
        label_at: (0x60, 0xE0),
        state_x: 0x140,
        widget_at: (280, 220),
        words: Words::YesNo,
        setting: Setting::Exploration,
    },
    Row {
        label: 4,
        label_at: (0x60, 0x100),
        state_x: 0x140,
        widget_at: (280, 252),
        words: Words::YesNo,
        setting: Setting::FightHumansOnly,
    },
];

/// Screen `0x42` — `Screen_SoundOptions` (`0x0041515C`), `L2.eng` group 51.
///
/// **Three flags and no volumes.** `Ui_OpenSlider` has exactly two call sites in
/// the whole executable — game speed and scroll speed — so group 12's
/// *"Adjusting music level"*, *"Adjusting sound level"* and *"Number of
/// samples"* are unreachable strings, and the three percentages behind them
/// (`g_options+0x3C`, `+0x40`, `+0x44`) are written to the preferences file on
/// every exit and read by one function whose only caller passes the mode that
/// draws none of them. Inventing a mixer here would be inventing a surface.
/// `[V]`
const SOUND: &[Row] = &[
    Row {
        label: 1,
        label_at: (0x60, 0xA0),
        state_x: 0x140,
        widget_at: (280, 156),
        words: Words::OnOff,
        setting: Setting::Music,
    },
    Row {
        label: 2,
        label_at: (0x60, 0xC0),
        state_x: 0x140,
        widget_at: (280, 188),
        words: Words::OnOff,
        setting: Setting::SoundEffects,
    },
    Row {
        label: 3,
        label_at: (0x60, 0xE0),
        state_x: 0x140,
        widget_at: (280, 220),
        words: Words::OnOff,
        setting: Setting::Speech,
    },
];

/// Screen `0x43` — `Screen_DisplayOptions` (`0x004152EA`), `L2.eng` group 52.
///
/// Two rows **and two different word pairs**: *Animations* is On/Off (group 19)
/// and *Full screen* is Yes/No (group 18), on the same panel. Group 52 index 3,
/// *"(F5 key re-sizes window to 640x480)"*, is drawn only while the game is
/// windowed. `[V]`
const DISPLAY: &[Row] = &[
    Row {
        label: 1,
        label_at: (0x60, 0xD0),
        state_x: 0x140,
        widget_at: (280, 204),
        words: Words::OnOff,
        setting: Setting::Animations,
    },
    Row {
        label: 2,
        label_at: (0x60, 0xF0),
        state_x: 0x140,
        widget_at: (280, 236),
        words: Words::YesNo,
        setting: Setting::FullScreen,
    },
];

/// Screen `0x31` — `Screen_HelpOptions` (`0x004154EA`), `L2.eng` group 45.
///
/// The third row is a **button, not a toggle**: its widget's callback is
/// `Opt_GameHelpContents` (`0x00434942`), and its widget sits at a different x
/// from the two above it because it is not in the state column.
const HELP: &[Row] = &[
    Row {
        label: 1,
        label_at: (0x80, 0xC0),
        state_x: 0x120,
        widget_at: (240, 188),
        words: Words::YesNo,
        setting: Setting::TipScreens,
    },
    Row {
        label: 2,
        label_at: (0x80, 0xE0),
        state_x: 0x120,
        widget_at: (240, 220),
        words: Words::YesNo,
        setting: Setting::ToolTips,
    },
    Row {
        label: 3,
        label_at: (0x80, 0x100),
        state_x: 0x120,
        widget_at: (288, 252),
        words: Words::YesNo,
        setting: Setting::StartGameHelp,
    },
];

impl Page {
    /// `g_screenId`, or `None` for the page that is ours.
    pub fn screen_id(self) -> Option<u8> {
        match self {
            Page::Advanced => Some(0x39),
            Page::Sound => Some(0x42),
            Page::Display => Some(0x43),
            Page::Help => Some(0x31),
            Page::Quirks => None,
        }
    }

    /// The painter, for anyone going back to the binary.
    pub fn painter(self) -> Option<u32> {
        match self {
            Page::Advanced => Some(0x0041_4F68),
            Page::Sound => Some(0x0041_515C),
            Page::Display => Some(0x0041_52EA),
            Page::Help => Some(0x0041_54EA),
            Page::Quirks => None,
        }
    }

    /// The `L2.eng` group whose index 0 is the heading.
    pub fn group(self) -> Option<usize> {
        match self {
            Page::Advanced => Some(GROUP_ADVANCED),
            Page::Sound => Some(GROUP_SOUND),
            Page::Display => Some(GROUP_DISPLAY),
            Page::Help => Some(GROUP_HELP),
            Page::Quirks => None,
        }
    }

    /// `Ui_DrawBox(x, y, cols, rows)` in cells of sixteen pixels, and the border
    /// set — all four panels use `FUN_004093E0`, which is set 1.
    pub fn window(self) -> (i32, i32, i32, i32, usize) {
        match self {
            Page::Advanced => (0x30, 0x60, 0x18, 0x0D, 1),
            Page::Sound => (0x30, 0x60, 0x18, 0x0C, 1),
            Page::Display => (0x30, 0x90, 0x18, 0x0A, 1),
            Page::Help => (0x60, 0x80, 0x16, 0x0B, 1),
            // Ours, and the widest of the five because it lists a lot of rows.
            Page::Quirks => (0x10, 0x20, 0x26, 0x1A, 1),
        }
    }

    /// The heading's baseline.
    pub fn heading_at(self) -> (i32, i32) {
        match self {
            Page::Advanced => (0x40, 0x74),
            Page::Sound => (0x40, 0x74),
            Page::Display => (0x40, 0xA4),
            Page::Help => (0x80, 0x94),
            Page::Quirks => (0x20, 0x44),
        }
    }

    /// `Ui_OkButton(x, y, mode)` — the corner picture that closes the panel.
    pub fn close_at(self) -> (i32, i32) {
        match self {
            Page::Advanced => (0x188, 0x100),
            Page::Sound => (0x188, 0xF0),
            Page::Display => (0x188, 0x100),
            Page::Help => (0x194, 0x106),
            Page::Quirks => (0x228, 0x1B0),
        }
    }

    /// The close button's hit box.
    pub fn close_hit(self) -> Rect {
        let (x, y) = self.close_at();
        Rect::new(x, y, WIDGET, WIDGET)
    }

    /// The rows the painter draws, in the painter's order. Empty for the quirks
    /// page, which is built from the catalogue instead.
    pub fn rows(self) -> &'static [Row] {
        match self {
            Page::Advanced => ADVANCED,
            Page::Sound => SOUND,
            Page::Display => DISPLAY,
            Page::Help => HELP,
            Page::Quirks => &[],
        }
    }

    /// Whether this page is the original's or ours.
    pub fn is_ours(self) -> bool {
        self == Page::Quirks
    }

    /// The five, in the order the demo index offers them.
    pub const ALL: [Page; 5] =
        [Page::Advanced, Page::Sound, Page::Display, Page::Help, Page::Quirks];
}

// ---------------------------------------------------------------------------
// Reading and writing a setting
// ---------------------------------------------------------------------------

/// What a row's state word says right now.
///
/// The four *rule* settings come off [`l2_kingdom::kingdom::Options`], which is
/// the world's; the sound and interface settings are the machine's and come off
/// [`Prefs`]. That split is not cosmetic — the first four are in the save body
/// and in the lockstep digest and the rest are not, which is the same line
/// `docs/bugs.md` §6.3a draws between the two quirk sets.
pub fn value(setting: Setting, ctx: &Ctx) -> bool {
    let o = &ctx.game.kingdom.options;
    match setting {
        Setting::AdvancedFarming => o.advanced_farming,
        Setting::ArmyForaging => o.armies_eat,
        Setting::Exploration => o.exploration,
        // Inverted in the original: the byte is 0 when the option displays
        // *Yes*. `l2_kingdom::battle::settlement` tests the byte against 0
        // exactly as `FUN_004A6A30` does, and this is the only place the sense
        // is turned round for a reader.
        Setting::FightHumansOnly => o.fight_humans_only_byte == 0,
        Setting::Music => ctx.game.prefs.music,
        Setting::SoundEffects => ctx.game.prefs.effects,
        Setting::Speech => ctx.game.prefs.speech,
        Setting::Animations => ctx.game.prefs.animations,
        Setting::TipScreens => ctx.game.prefs.tip_screens,
        Setting::ToolTips => ctx.game.prefs.tool_tips,
        // Neither is honoured; both report the original's default so the panel
        // reads as the original's does.
        Setting::FullScreen => true,
        Setting::StartGameHelp => true,
    }
}

/// Flip a row, the way its `Opt_Toggle*` does: `x = (x != 1)`.
pub fn toggle(setting: Setting, ctx: &mut Ctx) {
    let on = {
        let read = Ctx { game: ctx.game, assets: ctx.assets };
        value(setting, &read)
    };
    let o = &mut ctx.game.kingdom.options;
    match setting {
        Setting::AdvancedFarming => o.advanced_farming = !on,
        Setting::ArmyForaging => o.armies_eat = !on,
        Setting::Exploration => o.exploration = !on,
        Setting::FightHumansOnly => o.fight_humans_only_byte = u8::from(on),
        Setting::Music => ctx.game.prefs.music = !on,
        Setting::SoundEffects => ctx.game.prefs.effects = !on,
        Setting::Speech => ctx.game.prefs.speech = !on,
        Setting::Animations => ctx.game.prefs.animations = !on,
        Setting::TipScreens => ctx.game.prefs.tip_screens = !on,
        Setting::ToolTips => ctx.game.prefs.tool_tips = !on,
        Setting::FullScreen | Setting::StartGameHelp => {}
    }
}

// ---------------------------------------------------------------------------
// The quirk list, which spans two homes
// ---------------------------------------------------------------------------

/// Where a quirk's flag actually lives.
///
/// **Two homes, and the split is not an implementation detail a player should
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

// ---------------------------------------------------------------------------
// The screen
// ---------------------------------------------------------------------------

/// Row pitch on the quirks page. Ours, not the original's — it has no such page.
const QUIRK_ROW_H: i32 = 16;
/// Where the quirks page's list begins, and where its check boxes sit.
const QUIRK_LIST_X: i32 = 0x30;
const QUIRK_LIST_Y: i32 = 0x70;
const QUIRK_BOX_X: i32 = 0x20;
/// The parent check box, above the list.
const QUIRK_PARENT: (i32, i32) = (0x20, 0x54);
/// A check box is 12 x 12 — smaller than the original's 24 x 24 widget, because
/// this page lists a dozen rows and the original's never lists more than four.
const QUIRK_BOX: i32 = 12;

/// One of the five options panels.
pub struct OptionsScreen {
    page: Page,
    /// Which quirk row the pointer is over, for the highlight. Purely
    /// presentational: nothing branches on it but the draw.
    hover: Option<usize>,
    /// Whether the pointer is over the parent check box.
    hover_parent: bool,
}

impl OptionsScreen {
    pub fn new(page: Page) -> OptionsScreen {
        OptionsScreen { page, hover: None, hover_parent: false }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    /// The parent check box's hit box.
    pub fn parent_hit() -> Rect {
        Rect::new(QUIRK_PARENT.0, QUIRK_PARENT.1, QUIRK_BOX, QUIRK_BOX)
    }

    /// One quirk row's check box.
    pub fn quirk_hit(index: usize) -> Rect {
        Rect::new(QUIRK_BOX_X, QUIRK_LIST_Y + index as i32 * QUIRK_ROW_H, QUIRK_BOX, QUIRK_BOX)
    }

    /// Which quirk row's box a point is in, if any.
    ///
    /// **Boxes only, and no nearest-match.** `map.rs`'s header records what a
    /// near-miss costs; on a page of a dozen twelve-pixel boxes the answer to
    /// "close to two of them" has to be *neither*.
    pub fn quirk_at(count: usize, x: i32, y: i32) -> Option<usize> {
        (0..count).find(|i| OptionsScreen::quirk_hit(*i).contains(x, y))
    }
}

impl Screen for OptionsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Options(self.page)
    }

    fn title(&self, ctx: &Ctx) -> String {
        match (self.page.group(), self.page.screen_id()) {
            (Some(g), Some(id)) => format!("{} — screen 0x{id:02X}", ctx.assets.shell.text(g, 0)),
            _ => "The original game's bugs — ours".to_string(),
        }
    }

    /// Every one of these is a `Ui_DrawBox` window over whatever opened it. The
    /// four painters clear nothing.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // The original's ways out, and no fourth. `Screen_FrameInput` tests
            // `g_mouseRightReleased` before `Ui_OkButtonClicked()` on all five
            // of these ids, and `L2.eng` group 12 index 0 says it in English:
            // *"Click Right to Exit"*.
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,

            Event::Pointer { x, y } => {
                if self.page.is_ours() {
                    let count = quirk_rows().len();
                    self.hover = OptionsScreen::quirk_at(count, x, y);
                    self.hover_parent = OptionsScreen::parent_hit().contains(x, y);
                } else {
                    self.hover = None;
                    self.hover_parent = false;
                }
                Transition::Stay
            }

            Event::Click { x, y } => {
                if self.page.close_hit().contains(x, y) {
                    return Transition::Pop;
                }
                if self.page.is_ours() {
                    if OptionsScreen::parent_hit().contains(x, y) {
                        // The parent is tri-state to *read* and two-state to
                        // *click*: it is meaningless to click a control into
                        // "mixed", so a click from Mixed goes to all-fixed —
                        // the direction the player asked for by name, *"turn
                        // off original game's bugs"*.
                        let reproduced =
                            quirk_group(ctx.game) == l2_net::Group::AllFixed;
                        set_all_quirks(reproduced, ctx.game);
                        return Transition::Stay;
                    }
                    let rows = quirk_rows();
                    if let Some(i) = OptionsScreen::quirk_at(rows.len(), x, y) {
                        let now = quirk_reproduced(rows[i], ctx.game);
                        set_quirk(rows[i], !now, ctx.game);
                    }
                    return Transition::Stay;
                }
                for row in self.page.rows() {
                    if row.hit().contains(x, y) {
                        if row.supported() {
                            toggle(row.setting, ctx);
                        }
                        return Transition::Stay;
                    }
                }
                // **Consumed, never passed.** A click that missed every box does
                // nothing at all; letting it fall through would land it on the
                // map underneath, which is the exact fault `map.rs`'s header
                // records three of — and a player has separately reported that
                // clicking anywhere inside a window used to close it.
                Transition::Stay
            }

            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let pen = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            // The four options painters set neither text flag, so this is the
            // ordinary emboss with no drop capitals — the same pen every other
            // management panel uses.
            shadow: Some(font::SHADOW),
            caps: None,
        };

        let (bx, by, cols, rows, set) = self.page.window();
        pen.window(canvas, bx, by, cols, rows, set);

        if self.page.is_ours() {
            self.draw_quirks(ctx, canvas, &pen);
        } else {
            self.draw_panel(ctx, canvas, &pen);
        }
        let _ = (bx, by, cols, rows);

        let (cx, cy) = self.page.close_at();
        let drawn = ctx
            .assets
            .chrome
            .as_ref()
            .is_some_and(|c| c.draw_system(canvas, l2_view::chrome::system::OK, cx, cy));
        if !drawn {
            shell::button_recess(canvas, cx, cy, WIDGET, WIDGET);
        }
    }
}

impl OptionsScreen {
    /// One of the original's four, drawn as its painter draws it.
    fn draw_panel(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen) {
        let a = &ctx.assets.shell;
        let group = self.page.group().expect("one of the original's four");
        let (hx, hy) = self.page.heading_at();
        let heading = a.text(group, 0).to_string();
        pen.heading(canvas, hx, hy, &heading, font::TEXT);

        let mut unsupported = false;
        for row in self.page.rows() {
            let label = a.text(group, row.label).to_string();
            let colour = if row.supported() { font::TEXT } else { font::DISABLED };
            pen.body(canvas, row.label_at.0, row.label_at.1, &label, colour);

            // The state word, out of group 18 or 19, at its own x. *Start game
            // help* is a button and has no state, so it prints none.
            if row.setting != Setting::StartGameHelp {
                let on = value(row.setting, ctx);
                let word = a.text(row.words.group(), row.words.index(on)).to_string();
                pen.body(canvas, row.state_x, row.label_at.1, &word, colour);
            }

            let (wx, wy) = row.widget_at;
            let drawn = ctx
                .assets
                .chrome
                .as_ref()
                .is_some_and(|c| c.draw_system(canvas, l2_view::chrome::system::OK_ALT, wx, wy));
            if !drawn {
                shell::button_recess(canvas, wx, wy, WIDGET, WIDGET);
            }
            unsupported |= !row.supported();
        }

        // Group 52 index 3 — *"(F5 key re-sizes window to 640x480)"* — is drawn
        // only while the game is windowed (`if (g_optFullScreen == 0)`). We are
        // always windowed, so it is always drawn.
        //
        // **The original draws it in `0x3F` like every other row**; it is dimmed
        // here because this engine has no F5 resize, and that is a divergence
        // rather than a transcription. `Eng_DrawString(52, 3, 0x48, 0x108,
        // &g_fontBody, 0x3F)`.
        if self.page == Page::Display {
            let note = a.text(group, DISPLAY_F5_NOTE).to_string();
            pen.body(
                canvas,
                DISPLAY_F5_NOTE_AT.0,
                DISPLAY_F5_NOTE_AT.1,
                &note,
                font::DISABLED,
            );
        }

        // Ours, in our own font, so it cannot be mistaken for the game's words.
        if unsupported {
            let (x, y, _, rows, _) = self.page.window();
            // On one line so that `crates/l2-game/tests/draws.rs`' caption
            // scanner can see it: it reads the first quoted run on the line the
            // call is on, and a `rustfmt`-split literal is invisible to it.
            l2_view::text::draw(canvas, x, y + rows * 16 + 6, "GREYED: THIS ENGINE IS A WINDOW, AND L2HELP.HLP IS WIN3.1", ctx.assets.ink.dim);
        }
    }

    /// **Ours.** The quirks page.
    fn draw_quirks(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen) {
        let ink = &ctx.assets.ink;
        let (hx, hy) = self.page.heading_at();
        // Our words, in the game's font, because the game has none for this.
        pen.heading(canvas, hx, hy, "The original game's bugs", font::TEXT);

        let group = quirk_group(ctx.game);
        let (on, total) = quirk_tally(ctx.game);
        check_box(canvas, QUIRK_PARENT.0, QUIRK_PARENT.1, tri(group), ink, self.hover_parent);
        let parent = match group {
            l2_net::Group::AllReproduced => {
                format!("Reproducing all of them  ({on} of {total})")
            }
            l2_net::Group::AllFixed => format!("All turned off  ({on} of {total})"),
            l2_net::Group::Mixed => format!("Some of them  ({on} of {total})"),
        };
        pen.body(canvas, QUIRK_PARENT.0 + 20, QUIRK_PARENT.1 - 2, &parent, font::TEXT);

        for (i, row) in quirk_rows().iter().enumerate() {
            let y = QUIRK_LIST_Y + i as i32 * QUIRK_ROW_H;
            let reproduced = quirk_reproduced(*row, ctx.game);
            check_box(canvas, QUIRK_BOX_X, y, u8::from(reproduced), ink, self.hover == Some(i));
            let colour = if reproduced { font::TEXT } else { font::DISABLED };
            let line = format!("{}  {}", row.entry, summary_of(*row));
            pen.body(canvas, QUIRK_LIST_X, y - 2, &line, colour);
        }

        // The mark. This page is **ours**, and it says so on itself in our own
        // font — the rule `crates/l2-game/src/screens/index.rs` already follows.
        l2_view::text::draw(canvas, QUIRK_PARENT.0, 0x1C4, "OURS: THE ORIGINAL HAS NO SUCH PAGE. SEE DOCS/BUGS.MD", ink.dim);
    }
}

/// The parent's three states as the box's three fills.
fn tri(group: l2_net::Group) -> u8 {
    match group {
        l2_net::Group::AllReproduced => 1,
        l2_net::Group::AllFixed => 0,
        l2_net::Group::Mixed => 2,
    }
}

/// A check box: empty, ticked, or **half filled for the mixed state**.
///
/// Three appearances, because the parent has three states, and a two-state box
/// that showed "mixed" as either of the others would be a control that lies
/// about what is underneath it.
fn check_box(canvas: &mut Canvas, x: i32, y: i32, state: u8, ink: &l2_view::Ink, hover: bool) {
    let edge = if hover { ink.highlight } else { ink.dim };
    canvas.fill_rect(x, y, QUIRK_BOX, QUIRK_BOX, edge);
    canvas.fill_rect(x + 1, y + 1, QUIRK_BOX - 2, QUIRK_BOX - 2, ink.background);
    match state {
        // Reproduced: a full mark.
        1 => canvas.fill_rect(x + 3, y + 3, QUIRK_BOX - 6, QUIRK_BOX - 6, ink.highlight),
        // Mixed: a half-height bar, which is neither of the other two at a
        // glance and is what a tri-state box looks like everywhere else.
        2 => canvas.fill_rect(x + 3, y + QUIRK_BOX / 2 - 1, QUIRK_BOX - 6, 2, ink.highlight),
        _ => {}
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
