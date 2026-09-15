//! **The message window, its ring, and everything that dismisses or answers
//! one.** `Msg_Enqueue` (`0x00472BC5`), `Msg_Pump` (`0x00472E46`),
//! `Msg_DrawWindow` (`0x0047309E`), `Msg_Dismiss` (`0x00476768`) and the input
//! arm `Msg_HandleInput` (`0x0047685D`).
//!
//! The original's message system is four functions and one 24-byte record, and
//! the record is the join: the *rules* fill one in and push it, and the
//! *window* reads it back. `l2_kingdom::diplomacy::Letter` is already that
//! record — it was written from `Msg_Enqueue`'s eight parameters — so [`Record`]
//! is `Letter` plus the one field `Letter` did not need (`+0x13`, the second
//! realm a heading names) and a `From` impl carries them across.
//!
//! ```c
//! /* Msg_Enqueue, 0x00472BC5 — all three isHuman branches compute this */
//! enqueue = (to == 0) || (to == g_localPlayer);
//! ```
//!
//! **The one thing the window does write into the world is the outcome**, and
//! that is the original's own arrangement: `Msg_DrawWindow`'s
//! category-`0x0E` arm is the only writer of `DAT_0053F0C4` during play. It is
//! reproduced in [`show`], which is *the frame the window opens*, and
//! `docs/plan.md`'s sentence about the win is exactly that split — **displayed**
//! sets the outcome, **dismissed** acts on it.
//!
//! `Msg_DrawWindow` dispatches on the record's category byte at `+0x11` into
//! twenty arms. They are enumerated in [`category`], and [`Shape`] is the
//! taxonomy a painter needs: which of them draw a portrait, which draw a county
//! name, which carry a question and which are answered by a click somewhere
//! else entirely.
//!
//! * **the draw creates the hotspot.** `Ui_OkButton` stashes its own position
//!   into `DAT_0055CE78`/`DAT_0057C8A0`, `Msg_DrawWindow`'s last two statements
//!   copy those into `DAT_005681DC`/`DAT_0056950C`, and `Msg_HandleInput`
//!   hit-tests a **48 × 48** box round *that* copy — twice the size of the 24 ×
//!   24 picture. A category that draws no OK button therefore has no click
//!   target at all, and category `0x04` is one.
//!
//! And one that is not in `Msg_DrawWindow` at all: **`Map_Click`'s entire body
//! is guarded on `g_messageGroup == 0`**, and its `else` is
//! `Msg_DismissUnlessQuestion` (`0x00476710`). With a message up, a left click
//! on the campaign map dismisses it and does *nothing else* — no county
//! selected, no village opened, no army ordered. See
//! [`MessageQueue::dismiss_unless_question`].

pub mod help;
mod layout;
pub use layout::*;
mod queue;
pub use queue::*;
mod tests;
pub use tests::*;

use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};

use crate::game::Game;

pub const RING: usize = 50;

pub const TIMER_START: i32 = 2000;

pub const MULTIPLAYER_TIMEOUT: i32 = 0x641;

pub const TIP_TIMER: i32 = 100;

/// The `L2.eng` groups a *rule* names.
pub mod group {
    /// **`0xC2` — `L2.eng` 194, *"Foiled again."***, the AI's lament on losing
    /// ground, raised by `FUN_0049B42B`.
    ///
    /// It is here because it is the one group `Msg_Dismiss` (`0x00476768`)
    /// tests: `if (g_messageGroup != 0xc2) Sound_StopOneShot();`. Closing any
    /// other window cuts the narrator off mid-sentence; closing this one lets
    /// him finish. `[V]` on the branch; `[I]` on why, which is presumably that
    /// the line is short and self-contained.
    pub const FOILED_AGAIN: u16 = 0xC2;
}

/// The message categories, `Msg_Enqueue`'s `+0x11` byte, as `Msg_DrawWindow`
/// dispatches on them.
pub mod category {
    /// A plain notice. The heading is the group's own label, or a county name
    /// when `+0x12` is set, or a lord's name when `+0x13` is.
    pub const NOTICE: u8 = 0x00;
    pub const LETTER: u8 = 0x01;
    /// A county notice with the portrait panel — `L2.eng` 109/1 over the county
    /// name.
    pub const COUNTY_PORTRAIT: u8 = 0x02;
    pub const COUNTY_NOTICE: u8 = 0x03;
    pub const TIP: u8 = 0x04;
    pub const PARAGRAPHS_FIRST: u8 = 0x05;
    pub const PARAGRAPHS_LAST: u8 = 0x09;
    /// **The pay-for-help prompt.** Price, and a yes/no pair answered by
    /// `Diplo_PayHelpClicked` (`0x004367FF`).
    pub const PAY_PROMPT: u8 = 0x0A;
    /// **The *"Accept alliance ?"* prompt**, answered by `FUN_00436872`
    /// (`0x00436872`).
    pub const ALLIANCE_PROMPT: u8 = 0x0B;
    /// A diplomatic letter — `Msg_DrawDiplomacy` (`0x00475E07`), eleven arms
    /// over groups 245..=255, three of which carry their own question.
    pub const DIPLOMACY: u8 = 0x0C;
    pub const CAPTURE: u8 = 0x0D;
    /// **The ending.** The only writer of `DAT_0053F0C4` during play.
    pub const ENDING: u8 = 0x0E;
    pub const EVENT: u8 = 0x0F;
    pub const COUNTY_TALL: u8 = 0x10;
    /// **The garrison prompt** — *"Cannot garrison castle."*, group 166,
    /// answered by `FUN_004376BB` (`0x004376BB`), which opens the divide screen.
    pub const GARRISON_PROMPT: u8 = 0x11;
    /// Castle building progress, group 163, with four sub-arms.
    pub const CASTLE: u8 = 0x12;
    /// **The help window**, groups `0x123 ..= 0x127`, whose geometry comes out
    /// of the table at `g_helpWindowGeom` (`0x004D6EB8`).
    pub const HELP: u8 = 0x13;
    /// A letter from *"Beyond"* — `Msg_DrawBeyondLetter` (`0x00476488`), which
    /// is `Msg_DrawDiplomacy`'s layout with no reply widget, so it cannot be
    /// answered.
    pub const BEYOND: u8 = 0x14;
}

/// `L2.eng` group 109 — index 0 is *"From "*, index 1 the county-notice
/// heading. `Msg_DrawWindow` draws `Eng_DrawString(0x6D, 0, …)` before every
/// lord's name.
pub const GROUP_FROM: usize = 109;
/// `L2.eng` group 100 — the county names, indexed `scenario * 20 + county`.
pub const GROUP_COUNTY: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Notice,
    Letter,
    CountyPortrait,
    CountyNotice,
    Tip,
    Paragraphs(usize),
    Prompt,
    DiplomaticLetter,
    Capture,
    Ending,
    Event,
    Garrison,
    Castle,
    Help,
    Unhandled,
}

impl Shape {
    pub fn of(category: u8) -> Shape {
        match category {
            category::NOTICE => Shape::Notice,
            category::LETTER => Shape::Letter,
            category::COUNTY_PORTRAIT => Shape::CountyPortrait,
            category::COUNTY_NOTICE | category::COUNTY_TALL => Shape::CountyNotice,
            category::TIP => Shape::Tip,
            c @ category::PARAGRAPHS_FIRST..=category::PARAGRAPHS_LAST => {
                Shape::Paragraphs(c as usize - 4)
            }
            category::PAY_PROMPT | category::ALLIANCE_PROMPT => Shape::Prompt,
            category::DIPLOMACY | category::BEYOND => Shape::DiplomaticLetter,
            category::CAPTURE => Shape::Capture,
            category::ENDING => Shape::Ending,
            category::EVENT => Shape::Event,
            category::GARRISON_PROMPT => Shape::Garrison,
            category::CASTLE => Shape::Castle,
            category::HELP => Shape::Help,
            _ => Shape::Unhandled,
        }
    }

    pub fn has_ok_button(self) -> bool {
        !matches!(self, Shape::Tip | Shape::Unhandled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// `0x004DDB50` → `FUN_004376BB`. *"Cannot garrison castle."* — yes opens
    /// the divide screen seeded from the army, no returns to the map.
    Garrison,
    /// `0x004DDA90` → `Diplo_PayHelpClicked` (`0x004367FF`).
    PayForHelp,
    /// `0x004DDAC0` → `FUN_00436872`. Reached from category `0x0B` *and* from
/// the diplomatic letter of group `0xF8`, so one table serves two
    /// categories.
    AcceptAlliance,
    /// `0x004DDAF0` → `FUN_004368FD` — the ally's answer to *"help me"*.
    AnswerHelpRequest,
    /// `0x004DDB20` → `FUN_0043695D` — the ally's answer to *"attack them"*.
    AnswerAttackRequest,
}

impl Prompt {
    pub fn widgets(self) -> [(i32, i32); 2] {
        match self {
            Prompt::Garrison => [(320, 312), (356, 316)],
            Prompt::PayForHelp | Prompt::AcceptAlliance => [(352, 184), (388, 188)],
            Prompt::AnswerHelpRequest | Prompt::AnswerAttackRequest => [(80, 280), (116, 284)],
        }
    }

    /// The widget hit box: a square of side `+0x06`, which is 32 in all five.
    pub const SIDE: i32 = 32;

    pub const FRAME_YES: usize = 29;
    pub const FRAME_NO: usize = 31;

    pub fn hit(self, x: i32, y: i32) -> Option<bool> {
        let w = self.widgets();
        let inside = |(wx, wy): (i32, i32)| {
            x >= wx && x < wx + Prompt::SIDE && y >= wy && y < wy + Prompt::SIDE
        };
        if inside(w[0]) {
            Some(true)
        } else if inside(w[1]) {
            Some(false)
        } else {
            None
        }
    }
}

/// The window's geometry: `FUN_004093E0(x, y, cols, rows)`, in the original's
/// own units — `x` and `y` in pixels, `cols` and `rows` in 16-pixel cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Frame {
    const fn new(x: i32, y: i32, w: i32, h: i32) -> Frame {
        Frame { x, y, w, h }
    }

    pub fn ok_button(self) -> (i32, i32) {
        (self.x + self.w - 0x30, self.y + self.h - 0x30)
    }

    /// `Ui_OkButtonClicked` (`0x0040E7E4`), which every *other* screen uses,
    /// tests the 24 × 24 box instead. The message scroll is the easier target on
    /// purpose.
    pub fn ok_hitbox(self) -> crate::input::Rect {
        let (x, y) = self.ok_button();
        crate::input::Rect::new(x - 0xC, y - 0xC, 0x30, 0x30)
    }
}

/// `g_messageQueue` (`0x00568480`), `g_messageQueueHead`, `g_messageQueueTail`,
/// `g_messagePending`, `g_messageGroup` and `g_messageTimer`, in one place
/// because they are one mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageQueue {
    ring: [Record; RING],
    head: usize,
    pub(super) tail: usize,
    pending: bool,
    open: Option<Record>,
    timer: i32,
}

impl Default for MessageQueue {
    fn default() -> MessageQueue {
        MessageQueue::new()
    }
}

pub const PUMP_SCREENS: [u8; 4] = [0x00, 0x27, 0x0F, 0x29];

pub const PUMP_JOB: usize = 8;

