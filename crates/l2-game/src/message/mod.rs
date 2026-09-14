//! **The message window, its ring, and everything that dismisses or answers
//! one.** `Msg_Enqueue` (`0x00472BC5`), `Msg_Pump` (`0x00472E46`),
//! `Msg_DrawWindow` (`0x0047309E`), `Msg_Dismiss` (`0x00476768`) and the input
//! arm `Msg_HandleInput` (`0x0047685D`).
//!
//! # Why this is one module and not two
//!
//! The original's message system is four functions and one 24-byte record, and
//! the record is the join: the *rules* fill one in and push it, and the
//! *window* reads it back. `l2_kingdom::diplomacy::Letter` is already that
//! record — it was written from `Msg_Enqueue`'s eight parameters — so [`Record`]
//! is `Letter` plus the one field `Letter` did not need (`+0x13`, the second
//! realm a heading names) and a `From` impl carries them across.
//!
//! # Which half is simulation and which is display
//!
//! **The ring is display.** It is forced by
//! the binary, and it is the answer `docs/netcode.md` needs:
//!
//! ```c
//! /* Msg_Enqueue, 0x00472BC5 — all three isHuman branches compute this */
//! enqueue = (to == 0) || (to == g_localPlayer);
//! ```
//!
//! A message addressed to realm 3 is **never put in realm 1's ring**. So two
//! peers of one game hold different rings by construction, and a ring inside
//! `Canonical::hash_of(kingdom)` would desync every network game on the first
//! letter an AI wrote to somebody. The rules therefore keep producing
//! [`l2_kingdom::diplomacy::Letter`] values, which are the same on every peer,
//! and this queue — on [`crate::Game`], beside the levy and the live battle — is
//! where one peer's copy of them lands.
//!
//! **The one thing the window does write into the world is the outcome**, and
//! that is the original's own arrangement: `Msg_DrawWindow`'s
//! category-`0x0E` arm is the only writer of `DAT_0053F0C4` during play. It is
//! reproduced in [`show`], which is *the frame the window opens*, and
//! `docs/plan.md`'s sentence about the win is exactly that split — **displayed**
//! sets the outcome, **dismissed** acts on it.
//!
//! # The categories
//!
//! `Msg_DrawWindow` dispatches on the record's category byte at `+0x11` into
//! twenty arms. They are enumerated in [`category`], and [`Shape`] is the
//! taxonomy a painter needs: which of them draw a portrait, which draw a county
//! name, which carry a question and which are answered by a click somewhere
//! else entirely.
//!
//! # Three arms the dispatcher gives no hint of
//!
//! `docs/agents.md` records that input hides in at least three places. All three
//! are here:
//!
//! * **the draw creates the hotspot.** `Ui_OkButton` stashes its own position
//!   into `DAT_0055CE78`/`DAT_0057C8A0`, `Msg_DrawWindow`'s last two statements
//!   copy those into `DAT_005681DC`/`DAT_0056950C`, and `Msg_HandleInput`
//!   hit-tests a **48 × 48** box round *that* copy — twice the size of the 24 ×
//!   24 picture. A category that draws no OK button therefore has no click
//!   target at all, and category `0x04` is one.
//! * **the draw dismisses.** Category `0x0B` opens `if (my ally != 0)
//!   { Msg_Dismiss(); return; }`, so an alliance offer that arrived while
//!   another one was being accepted closes itself without ever being seen.
//! * **the draw enqueues.** Category `0x0E` with no opponents left pushes group
//!   225 onto the ring it is being drawn from.
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

/// The ring is 50 slots. `Msg_Reset` clears `0..0x32` and both cursors wrap at
/// `0x31`.
pub const RING: usize = 50;

/// `g_messageTimer` when a record is pulled off the ring — and the value every
/// *"first frame"* arm of `Msg_DrawWindow` compares against.
pub const TIMER_START: i32 = 2000;

/// **What a message costs a player in a network game.** `Msg_Pump` dismisses an
/// ordinary message once the timer falls below `0x641`, which is 399 ticks after
/// it opened; in single player the same branch clamps the timer to 1 instead, so
/// **a message never times out and waits for a click.**
pub const MULTIPLAYER_TIMEOUT: i32 = 0x641;

/// `Msg_DrawWindow`'s category-`0x04` arm clamps a timer above 999 down to this,
/// so the floating tip lives 100 ticks and then `Msg_Pump`'s category-`0x04`
/// branch dismisses it. It is the one category with no way to dismiss it by
/// hand.
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
///
/// Twenty arms. `0x05 ..= 0x09` is a *range*
/// whose value is the paragraph count — so this is constants and a
/// [`Shape`].
pub mod category {
    /// A plain notice. The heading is the group's own label, or a county name
    /// when `+0x12` is set, or a lord's name when `+0x13` is.
    pub const NOTICE: u8 = 0x00;
    /// A lord's letter: shield, portrait, *"From <name>"*, wrapped body.
    pub const LETTER: u8 = 0x01;
    /// A county notice with the portrait panel — `L2.eng` 109/1 over the county
    /// name.
    pub const COUNTY_PORTRAIT: u8 = 0x02;
    /// County name, then the group's label, then the body.
    pub const COUNTY_NOTICE: u8 = 0x03;
    /// **The floating tip.** Placed at the cursor, no OK button, and dismissed
    /// only by its own timer.
    pub const TIP: u8 = 0x04;
    /// `0x05 ..= 0x09` — a stack of `category - 4` wrapped paragraphs.
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
    /// A county was captured: a Smacker plays when animations are on.
    pub const CAPTURE: u8 = 0x0D;
    /// **The ending.** The only writer of `DAT_0053F0C4` during play.
    pub const ENDING: u8 = 0x0E;
    /// A county event, with an event-specific number line under the body.
    pub const EVENT: u8 = 0x0F;
    /// County name, group label, body — the taller cousin of [`COUNTY_NOTICE`].
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

/// Which of `Msg_DrawWindow`'s twenty arms a category takes, as a value a
/// painter and a test can both read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// [`category::NOTICE`].
    Notice,
    /// [`category::LETTER`] — shield, portrait, *"From"*.
    Letter,
    /// [`category::COUNTY_PORTRAIT`].
    CountyPortrait,
    /// [`category::COUNTY_NOTICE`] and [`category::COUNTY_TALL`].
    CountyNotice,
    /// [`category::TIP`] — at the cursor, no button.
    Tip,
    /// `0x05 ..= 0x09`: `n` wrapped paragraphs, `n = category - 4`.
    Paragraphs(usize),
    /// [`category::PAY_PROMPT`], [`category::ALLIANCE_PROMPT`] — a lord's letter
    /// with a yes/no pair.
    Prompt,
    /// [`category::DIPLOMACY`] and [`category::BEYOND`].
    DiplomaticLetter,
    /// [`category::CAPTURE`].
    Capture,
    /// [`category::ENDING`].
    Ending,
    /// [`category::EVENT`].
    Event,
    /// [`category::GARRISON_PROMPT`].
    Garrison,
    /// [`category::CASTLE`].
    Castle,
    /// [`category::HELP`].
    Help,
    /// A category byte `Msg_DrawWindow` has no arm for. **The window is drawn
    /// blank and the OK button is not drawn either**, because every arm that
    /// draws one is inside a branch — so an unknown category is a message that
    /// cannot be dismissed with the left button. Nothing enqueues one; it is
    /// here because the byte can hold it.
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

    /// Whether this layout draws `Ui_OkButton`, and therefore whether a left
    /// click can dismiss the message at all.
    ///
    /// **Every arm but two draws one.** [`Shape::Tip`] does not — it is the
    /// timed one — and [`Shape::Unhandled`] does not
    pub fn has_ok_button(self) -> bool {
        !matches!(self, Shape::Tip | Shape::Unhandled)
    }
}

/// The five two-button prompts in the game, named by what pressing *yes* does.
///
/// Each is one 24-byte widget pair in `.rdata`, hotspot id 1 on the left
/// (`System.pl8` frame 29, a mailed thumb up) and 0 on the right (frame 31,
/// thumb down), decoded with `tools/oracle/widgets.js`. The addresses are the
/// tables, not the handlers.
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
    /// `(x, y)` of the **yes** widget and of the **no** widget, absolute — every
    /// one of the five is drawn `Widget_Draw(0, 0, table, 2)`, so the table's
    /// coordinates are screen coordinates.
    ///
    /// The pair is offset by `(36, 4)` in all five tables: the no button is four
    /// pixels *lower*, which is the original's own arrangement and not a
    /// transcription slip — both were read out of the executable.
    pub fn widgets(self) -> [(i32, i32); 2] {
        match self {
            Prompt::Garrison => [(320, 312), (356, 316)],
            Prompt::PayForHelp | Prompt::AcceptAlliance => [(352, 184), (388, 188)],
            Prompt::AnswerHelpRequest | Prompt::AnswerAttackRequest => [(80, 280), (116, 284)],
        }
    }

    /// The widget hit box: a square of side `+0x06`, which is 32 in all five.
    pub const SIDE: i32 = 32;

    /// `System.pl8` frames — 29 yes, 31 no.
    pub const FRAME_YES: usize = 29;
    pub const FRAME_NO: usize = 31;

    /// Which widget, if either, a click at `(x, y)` is in. `true` is *yes*,
    /// hotspot id 1.
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

    /// `Ui_OkButton(x + w - 0x30, y + h - 0x30, 0)` — every arm that draws one
    /// draws it here.
    pub fn ok_button(self) -> (i32, i32) {
        (self.x + self.w - 0x30, self.y + self.h - 0x30)
    }

    /// **The click target, which is not the picture.** `Msg_HandleInput` tests
    /// `Rect_Contains(okX - 0xC, okY - 0xC, 0x30, 0x30)` — a 48 × 48 box round
    /// the 24 × 24 button, so twelve pixels of slop on every side.
    /// `Ui_OkButtonClicked` (`0x0040E7E4`), which every *other* screen uses,
    /// tests the 24 × 24 box instead. The message scroll is the easier target on
    /// purpose.
    pub fn ok_hitbox(self) -> crate::input::Rect {
        let (x, y) = self.ok_button();
        crate::input::Rect::new(x - 0xC, y - 0xC, 0x30, 0x30)
    }
}

/// **The 50-slot ring, the open window, and its timer.**
///
/// `g_messageQueue` (`0x00568480`), `g_messageQueueHead`, `g_messageQueueTail`,
/// `g_messagePending`, `g_messageGroup` and `g_messageTimer`, in one place
/// because they are one mechanism.
///
/// not-encoded: **per-peer display state**, and the module header is the whole
/// argument — `Msg_Enqueue` filters on `g_localPlayer`, so this differs between
/// two peers of one game on purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageQueue {
    ring: [Record; RING],
    /// `g_messageQueueHead` — where the next enqueue lands.
    head: usize,
    /// `g_messageQueueTail` — where the next pull comes from.
    pub(super) tail: usize,
    /// `g_messagePending`. Set by an enqueue and **never cleared by anything**
    /// in the binary except `Msg_Reset`; carried so the field is not silently
    /// dropped, and read by nothing here for the same reason.
    pending: bool,
    /// `g_messageGroup` and the rest of the unpacked record, or `None` when no
    /// window is up. The original spreads it over six globals and tests
    /// `g_messageGroup != 0` for *"is one open"*.
    open: Option<Record>,
    /// `g_messageTimer`.
    timer: i32,
}

impl Default for MessageQueue {
    fn default() -> MessageQueue {
        MessageQueue::new()
    }
}

/// **Which screens `Msg_Pump` runs on**, and what happens on the rest.
///
/// ```c
/// if (g_screenId == 0x00 || g_screenId == 0x27 ||
///     (g_screenId == 0x0F && g_jobPanelJob == 8) || g_screenId == 0x29) { … }
/// else if (g_messageGroup != 0) Msg_Dismiss();
/// ```
///
/// So the window lives on the campaign map, screen `0x27`, the *ninth* job
/// panel and the battlefield — and **opening any other screen while one is up
/// closes it**. That is an arm in its own right and it is why walking into the
/// village makes a message disappear.
///
/// `0x27` is not built here and is not in `screens/shells.rs` either; the
/// constant is kept so the list is the binary's.
pub const PUMP_SCREENS: [u8; 4] = [0x00, 0x27, 0x0F, 0x29];

/// The job panel only pumps for job **8** — `g_jobPanelJob == 8`, the ninth
/// labour slot.
pub const PUMP_JOB: usize = 8;

