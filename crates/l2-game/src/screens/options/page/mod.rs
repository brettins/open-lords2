#![allow(unused_imports)]

mod handlers;
pub use handlers::*;

use super::*;
use super::words::*;
use super::screen::*;
use l2_view::Canvas;
use crate::game::PRESENTATION;
use crate::input::{Event, Key, Rect};
use crate::press::{Kind, Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

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

/// `L2.eng` group 260 — message `0x104`, *"Cannot change display."* — which
/// `Opt_ToggleFullScreen` enqueues when the flag is 1 and the desktop is not
/// 8bpp. `docs/formats/eng.md` §5 names that function as its consumer.
pub const FULL_SCREEN_REFUSAL: u16 = 0x104;

/// Group 52 index 3, the windowed-mode hint, and the one row of these four
/// panels that is drawn conditionally.
pub const DISPLAY_F5_NOTE: usize = 3;
/// The F5 note's own baseline — `Eng_DrawString(52, 3, 0x48, 0x108, body)`,
/// which is 24 pixels left of every other row on the panel.
pub const DISPLAY_F5_NOTE_AT: (i32, i32) = (0x48, 0x108);

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
    /// **The kind byte at `+0x0F` of the row's widget record**, and every one
    /// of the twelve is **5**: `Widget_Test` shows the pressed picture on the
    /// press and runs the `Opt_Toggle*` handler **twenty frames later**, out
    /// of its countdown loop. `node tools/oracle/kinds.js` lists all twelve
    /// under `widget 5`, and `crates/l2-game/tests/arms/main.rs` reads the byte out
    /// of the player's own `Lords2.exe`.
    ///
    /// Ours acted on the click until this field existed — the same defect a
    /// player reported of the yes/no gauntlets (*"clicking yes/no is instant
    /// whereas the game waited"*), on a screen nobody had inventoried.
    pub kind: Kind,
    pub words: Words,
    pub setting: Setting,
}

impl Row {
    /// The widget's hit box — 24 × 24 at the record's own corner, and **the only
    /// hot rectangle on the row**.
    pub fn hit(&self) -> Rect {
        Rect::new(self.widget_at.0, self.widget_at.1, WIDGET, WIDGET)
    }

    /// The row as a record of the table [`Press::event`] walks.
    pub fn widget(&self) -> Widget {
        Widget::new(self.hit(), self.kind)
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
    /// **, in the disabled colour with the reason
    /// under the panel: a reproduction that silently loses two of its eleven
    /// rows is a reproduction nobody can check against a screenshot.
    pub fn supported(&self) -> bool {
        !matches!(self.setting, Setting::FullScreen | Setting::StartGameHelp)
    }
}

/// `Ui_OkButton`'s corner picture, and every options widget, is 24 × 24.
pub(super) const WIDGET: i32 = 24;

/// **`System.pl8` frame 25**, the `+0x04` base frame of all twelve widget
/// records (`node tools/oracle/kinds.js`: `f25` on every `Opt_*` row), and
/// `Widget_Draw` (`0x0040CFD2`) draws `base + 1` while the press timer runs.
///
/// Ours drew `Ui_OkButton`'s mode-1 picture, frame `0x10`, which is no record's
/// frame: it was chosen to look like a button
/// table.
pub const WIDGET_FRAME: usize = 25;

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
        kind: crate::arm!("0x00434556/opt-advanced-farming", Delayed),
        words: Words::YesNo,
        setting: Setting::AdvancedFarming,
    },
    Row {
        label: 2,
        label_at: (0x60, 0xC0),
        state_x: 0x140,
        widget_at: (280, 188),
        kind: crate::arm!("0x004345D0/opt-army-foraging", Delayed),
        words: Words::YesNo,
        setting: Setting::ArmyForaging,
    },
    Row {
        label: 3,
        label_at: (0x60, 0xE0),
        state_x: 0x140,
        widget_at: (280, 220),
        kind: crate::arm!("0x00434693/opt-exploration", Delayed),
        words: Words::YesNo,
        setting: Setting::Exploration,
    },
    Row {
        label: 4,
        label_at: (0x60, 0x100),
        state_x: 0x140,
        widget_at: (280, 252),
        kind: crate::arm!("0x0043470D/opt-fight-humans-only", Delayed),
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
        kind: crate::arm!("0x004349A4/opt-music", Delayed),
        words: Words::OnOff,
        setting: Setting::Music,
    },
    Row {
        label: 2,
        label_at: (0x60, 0xC0),
        state_x: 0x140,
        widget_at: (280, 188),
        kind: crate::arm!("0x00434A29/opt-sound-effects", Delayed),
        words: Words::OnOff,
        setting: Setting::SoundEffects,
    },
    Row {
        label: 3,
        label_at: (0x60, 0xE0),
        state_x: 0x140,
        widget_at: (280, 220),
        kind: crate::arm!("0x00434A9A/opt-speech", Delayed),
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
        kind: crate::arm!("0x00434AD5/opt-animations", Delayed),
        words: Words::OnOff,
        setting: Setting::Animations,
    },
    Row {
        label: 2,
        label_at: (0x60, 0xF0),
        state_x: 0x140,
        widget_at: (280, 236),
        kind: crate::arm!("0x00434B10/opt-full-screen", Delayed),
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
        kind: crate::arm!("0x00434787/opt-tip-screens", Delayed),
        words: Words::YesNo,
        setting: Setting::TipScreens,
    },
    Row {
        label: 2,
        label_at: (0x80, 0xE0),
        state_x: 0x120,
        widget_at: (240, 220),
        kind: crate::arm!("0x004347C7/opt-tool-tips", Delayed),
        words: Words::YesNo,
        setting: Setting::ToolTips,
    },
    Row {
        label: 3,
        label_at: (0x80, 0x100),
        state_x: 0x120,
        widget_at: (288, 252),
        // No `arm!`: `docs/arms.json` files `Opt_GameHelpContents` `missing`,
        // and a marker says an arm is in the tree. The kind is still the
        // record's.
        kind: Kind::Delayed,
        words: Words::YesNo,
        setting: Setting::StartGameHelp,
    },
];

