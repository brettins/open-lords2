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
//! **The ring is display.** That is not a modelling preference; it is forced by
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
//! that is the original's own arrangement rather than a leak: `Msg_DrawWindow`'s
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

/// The message categories, `Msg_Enqueue`'s `+0x11` byte, as `Msg_DrawWindow`
/// dispatches on them.
///
/// Twenty arms. The numbering is not a table — `0x05 ..= 0x09` is a *range*
/// whose value is the paragraph count — which is why this is constants and a
/// [`Shape`] rather than an enum over the byte.
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

/// One 24-byte ring record — `Msg_Enqueue`'s eight parameters, in memory order.
///
/// | offset | field | |
/// |---|---|---|
/// | `+0x00` | [`Record::to`] | recipient realm; **0 is everybody** |
/// | `+0x04` | [`Record::from`] | sender realm; 0 is the game itself |
/// | `+0x08` | [`Record::group`] | the `L2.eng` group, and **0 means empty slot** |
/// | `+0x0C` | [`Record::variant`] | which string of the group, drawn as `variant + 1` |
/// | `+0x11` | [`Record::category`] | which of [`category`]'s twenty layouts |
/// | `+0x12` | [`Record::county`] | a county id where the heading needs one |
/// | `+0x13` | [`Record::spare`] | a **second realm**, whose name replaces the heading |
/// | `+0x14` | [`Record::payload`] | a number the layout may print |
///
/// `+0x10` is a hole; the record is 24 bytes and eight fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Record {
    pub to: u8,
    pub from: u8,
    pub group: u16,
    pub variant: u8,
    pub category: u8,
    pub county: u8,
    /// `+0x13`. **Not spare at all** in categories `0x00` and `0x01`: a non-zero
    /// value there is a realm index and `Ui_DrawText(g_playerNames + spare *
    /// 0x2C, …)` puts that lord's name where the county name or the group's own
    /// label would have gone. `County_ChangeOwner`'s group `0x72` sets it to the
    /// *previous* owner, which is what makes *"X has taken Y from Z"* one
    /// string.
    pub spare: u8,
    pub payload: i32,
}

impl Record {
    /// An empty slot. `Msg_Pump` tests `group == 0` and nothing else.
    pub fn is_empty(&self) -> bool {
        self.group == 0
    }

    /// The `L2.eng` index of the body string: `variant + 1`, past the group's
    /// own label at index 0.
    pub fn body_index(&self) -> usize {
        self.variant as usize + 1
    }

    /// Which layout `Msg_DrawWindow` picks for this record.
    pub fn shape(&self) -> Shape {
        Shape::of(self.category)
    }

    /// Whether this record carries a question — the four categories
    /// `Msg_DismissUnlessQuestion` (`0x00476710`) refuses to close, and the four
    /// `Msg_HandleInput` runs a `Widget_Test` for.
    ///
    /// **The two lists are the same four and that is checkable rather than
    /// assumed**: `0x11`, `0x0A`, `0x0B`, `0x0C`. See
    /// [`MessageQueue::dismiss_unless_question`] and
    /// [`Record::answer_widgets`].
    pub fn is_question(&self) -> bool {
        matches!(
            self.category,
            category::GARRISON_PROMPT
                | category::PAY_PROMPT
                | category::ALLIANCE_PROMPT
                | category::DIPLOMACY
        )
    }

    /// The yes/no pair this record draws, or `None`.
    ///
    /// Category `0x0C` is the awkward one and it is awkward in the original
    /// too: `Msg_DrawDiplomacy` draws a widget for **three** of its eleven
    /// groups and `Msg_HandleInput` tests exactly those three, so
    /// [`Record::is_question`] is true for the whole category and this is false
    /// for eight of its groups. A right-click still closes those eight; there is
    /// simply nothing to click.
    pub fn answer_widgets(&self) -> Option<Prompt> {
        match self.category {
            category::GARRISON_PROMPT => Some(Prompt::Garrison),
            category::PAY_PROMPT => Some(Prompt::PayForHelp),
            category::ALLIANCE_PROMPT => Some(Prompt::AcceptAlliance),
            category::DIPLOMACY => match self.group {
                0xF8 => Some(Prompt::AcceptAlliance),
                0xFA => Some(Prompt::AnswerHelpRequest),
                0xFB => Some(Prompt::AnswerAttackRequest),
                _ => None,
            },
            _ => None,
        }
    }
}

impl From<Letter> for Record {
    fn from(l: Letter) -> Record {
        Record {
            to: l.to,
            from: l.from,
            group: l.group,
            variant: l.variant,
            category: l.category,
            county: l.county,
            spare: 0,
            payload: l.payload,
        }
    }
}

impl From<Ending> for Record {
    fn from(e: Ending) -> Record {
        Record {
            to: e.to,
            from: e.from,
            group: e.group,
            // **Not zero.** `Ending::variant` was added for this: groups 194
            // and 195 hold sixteen lord-flavoured lines apiece and the ending
            // chain picks one. Dropping it here is how every lord in the game
            // would come to say the Knight's first line.
            variant: e.variant,
            category: e.category,
            county: 0,
            spare: 0,
            payload: 0,
        }
    }
}

impl Record {
    /// Back to the ending the victory rules read. They consult `group` and
    /// `from` and nothing else — see [`l2_kingdom::victory::outcome_of`].
    pub fn as_ending(&self) -> Ending {
        Ending {
            group: self.group,
            from: self.from,
            to: self.to,
            category: self.category,
            variant: self.variant,
        }
    }
}

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
    /// timed one — and [`Shape::Unhandled`] does not because there is no arm.
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
    /// the diplomatic letter of group `0xF8`, which is why one table serves two
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

/// Where each category's window goes, read off `Msg_DrawWindow`'s
/// `FUN_004093E0` calls one arm at a time.
///
/// Three categories are not here because their geometry is not a constant:
/// [`category::TIP`] follows the cursor, [`category::HELP`] and the two
/// letter categories index tables in `.rdata`, and the paragraph stack computes
/// its height from how many paragraphs it drew.
pub fn frame_of(record: &Record) -> Option<Frame> {
    let f = match record.category {
        category::NOTICE => Frame::new(0x20, 0xA0, 0x1A0, 0xE0),
        category::COUNTY_TALL => Frame::new(0x20, 0xA0, 0x1A0, 0xF0),
        category::GARRISON_PROMPT | category::CASTLE => Frame::new(0x20, 0x90, 0x1A0, 0xF0),
        category::LETTER => Frame::new(0x10, 0x80, 0x1C0, 0x100),
        category::COUNTY_PORTRAIT => Frame::new(0x10, 0x90, 0x1C0, 0x100),
        category::COUNTY_NOTICE => Frame::new(0x10, 0xA0, 0x1C0, 0xF0),
        category::PAY_PROMPT | category::ALLIANCE_PROMPT => Frame::new(0x10, 0x80, 0x1C0, 0xF0),
        category::CAPTURE => Frame::new(0x20, 0xA0, 0x1A0, 0xC0),
        category::ENDING => Frame::new(0x10, 0x80, 0x1C0, 0xE0),
        // **The one arm whose height is a rule rather than a constant.** The
        // event window is 0xC0 tall for an event id below 0x12E and 0xE0 for one
        // at or above it, so the taller box is exactly the events with a number
        // line under the body.
        category::EVENT => Frame::new(0x20, 0xA0, 0x1A0, 0xC0),
        _ => return None,
    };
    Some(f)
}

/// What [`MessageQueue::advance`] did to the timer this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Nothing; the window is still up.
    Open,
    /// The timer ran out and the window closed itself. Only two things reach
    /// this: a [`category::TIP`], always, and any message in a **network** game
    /// after 399 ticks.
    TimedOut,
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
    tail: usize,
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

impl MessageQueue {
    pub fn new() -> MessageQueue {
        MessageQueue {
            ring: [Record::default(); RING],
            head: 0,
            tail: 0,
            pending: false,
            open: None,
            timer: 0,
        }
    }

    /// `Msg_Reset` (`0x00472AE0`) — both cursors, the flag, the timer and all
    /// fifty slots.
    ///
    /// **`FUN_00472B40`, the slot clear it calls fifty times, takes a slot
    /// index and then ignores it for six of its seven fields**, clearing
    /// `ring[g_messageQueueTail]` instead. Only `+0x08`, the group, lands on the
    /// slot it was asked about. That is a real defect and it happens to be
    /// harmless in both call sites — `Msg_Reset` zeroes `tail` before the loop
    /// and `group == 0` *is* the emptiness test, so every slot ends up empty
    /// however much stale payload it still holds; `Msg_Pump` passes the tail
    /// itself, so there the argument and the global agree. `docs/bugs.md`
    /// B95.
    pub fn reset(&mut self) {
        *self = MessageQueue::new();
    }

    /// `Msg_Enqueue` (`0x00472BC5`) — **the peer filter and the ring write.**
    ///
    /// Returns whether the record was kept. The filter is the module header's:
    /// `to == 0 || to == local_player`, computed identically down all three of
    /// the original's `isHuman` branches.
    ///
    /// The ring is **not** checked for space. Fifty-one messages in one turn
    /// overwrite the oldest and the tail is left pointing into the middle of
    /// them; the original does the same and nothing in the binary counts.
    ///
    /// arm-note: this is a rule, not an input arm. `docs/arms.json` records the
    /// arms that dismiss and answer, not the 144 call sites that fill the ring.
    pub fn enqueue(&mut self, record: Record, local_player: u8) -> bool {
        if !(record.to == 0 || record.to == local_player) {
            return false;
        }
        self.ring[self.head] = record;
        self.head += 1;
        if self.head > 0x31 {
            self.head = 0;
        }
        self.pending = true;
        true
    }

    /// [`MessageQueue::enqueue`] for a rule-layer letter.
    pub fn post(&mut self, letter: Letter, local_player: u8) -> bool {
        self.enqueue(Record::from(letter), local_player)
    }

    /// **`Msg_Pump`'s pull half** — the `g_messageTimer < 1` branch.
    ///
    /// One call moves one slot. A slot whose `group` is 0 is *skipped*, not
    /// searched past: the tail advances by one and the function returns, so a
    /// gap in the ring costs one frame each. That is the original's loop-free
    /// shape and it is why [`MessageQueue::drain`] has to call this in a loop.
    ///
    /// Returns whether a window opened.
    pub fn pull(&mut self) -> bool {
        if self.timer >= 1 {
            return false;
        }
        self.timer = 0;
        let slot = self.ring[self.tail];
        if slot.is_empty() {
            if self.tail == self.head {
                return false;
            }
            self.tail += 1;
            if self.tail >= RING {
                self.tail = 0;
            }
            return false;
        }
        self.ring[self.tail] = Record::default();
        self.tail += 1;
        if self.tail > 0x31 {
            self.tail = 0;
        }
        self.open = Some(slot);
        self.timer = TIMER_START;
        true
    }

    /// **`Msg_Pump`'s countdown half.** Returns [`Tick::TimedOut`] when the
    /// window closed itself.
    ///
    /// The three branches, in the original's own order:
    ///
    /// ```c
    /// g_messageTimer--;
    /// if (g_multiplayer == 0 || g_messageCategory == 0x0E) {
    ///     if (g_messageTimer < 1) g_messageTimer = 1;      /* never expires */
    /// } else if (g_messageCategory == 4) {
    ///     if (g_messageTimer < 1) Msg_Dismiss();           /* the tip */
    /// } else if (g_messageTimer < 0x641) Msg_Dismiss();    /* 399 ticks */
    /// ```
    ///
    /// So **in single player nothing times out**, an *ending* never times out
    /// even in a network game, and everything else in a network game is gone
    /// after 399 ticks whether it was read or not.
    pub fn advance(&mut self, multiplayer: bool) -> Tick {
        let Some(open) = self.open else { return Tick::Open };
        self.timer -= 1;
        if !multiplayer || open.category == category::ENDING {
            if self.timer < 1 {
                self.timer = 1;
            }
            Tick::Open
        } else if open.category == category::TIP {
            if self.timer < 1 {
                self.close();
                return Tick::TimedOut;
            }
            Tick::Open
        } else if self.timer < MULTIPLAYER_TIMEOUT {
            self.close();
            Tick::TimedOut
        } else {
            Tick::Open
        }
    }

    /// `Msg_DrawWindow`'s category-`0x04` arm: *"if the timer is above 999, make
    /// it 100"*. It runs every frame the tip is drawn, so the clamp bites once
    /// and the tip then counts 100 down to nothing.
    ///
    /// Kept out of [`MessageQueue::pull`] deliberately — the original really
    /// does start every message at 2000 and shorten this one from the *draw*,
    /// which is why a tip that is enqueued on a screen the pump does not run on
    /// keeps its full 2000.
    pub fn clamp_tip_timer(&mut self) {
        if self.open.is_some_and(|r| r.category == category::TIP) && self.timer > 999 {
            self.timer = TIP_TIMER;
        }
    }

    /// The record on screen, or `None`.
    pub fn open(&self) -> Option<&Record> {
        self.open.as_ref()
    }

    /// `g_messageGroup != 0` — the test twelve functions in the binary make.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// `g_messageTimer`.
    pub fn timer(&self) -> i32 {
        self.timer
    }

    /// **Whether this is the frame the window opened**, which is what
    /// `Msg_DrawWindow`'s `g_messageTimer == 2000` arms mean: the shield sound,
    /// the portrait load, and the ending's whole outcome ladder.
    pub fn just_opened(&self) -> bool {
        self.timer == TIMER_START && self.open.is_some()
    }

    /// Whether the ring holds anything at all, open window included.
    pub fn is_empty(&self) -> bool {
        self.open.is_none() && self.ring.iter().all(Record::is_empty)
    }

    /// How many filled slots the ring holds. For tests and for the census; the
    /// original counts nothing.
    pub fn queued(&self) -> usize {
        self.ring.iter().filter(|r| !r.is_empty()).count()
    }

    /// **The records still waiting, oldest first** — the ring walked from the
    /// tail, skipping the gaps `Msg_Pump` would step over one frame at a time.
    ///
    /// For [`crate::save`], which stores a queue and not an implementation of
    /// one, and for tests. Nothing in the game reads it: the original walks the
    /// ring one slot per frame and has no reason to know how long it is.
    pub fn waiting(&self) -> Vec<Record> {
        (0..RING)
            .map(|i| self.ring[(self.tail + i) % RING])
            .filter(|r| !r.is_empty())
            .collect()
    }

    /// Put a record back on the ring at the head, **without the peer filter**.
    /// [`crate::save`] only; a record in a file was filtered when it was first
    /// enqueued.
    pub fn restore(&mut self, record: Record) {
        self.ring[self.head] = record;
        self.head += 1;
        if self.head > 0x31 {
            self.head = 0;
        }
        self.pending = true;
    }

    /// Put the window back up with the timer it had. [`crate::save`] only.
    pub fn reopen(&mut self, record: Record, timer: i32) {
        self.open = Some(record);
        self.timer = timer;
    }

    /// Close the window with none of `Msg_Dismiss`'s side effects — the two
    /// lines of it that are pure state.
    fn close(&mut self) {
        self.open = None;
        self.timer = 0;
    }

    /// **`Msg_DismissUnlessQuestion` (`0x00476710`)** — the whole function:
    ///
    /// ```c
    /// if (category != 0x11 && category != 10 && category != 0x0B && category != 0x0C)
    ///     Msg_Dismiss();
    /// ```
    ///
    /// One caller, and it is the arm nobody had looked for: **`Map_Click`'s
    /// entire body is `if (g_messageGroup == 0) { … } else { this }`**. So with
    /// a message up, a left click on the campaign map closes it and the map does
    /// nothing else at all — no pick, no county selection, no village. A
    /// *question* survives the click, which is what stops a stray click on the
    /// map from silently declining an alliance.
    ///
    /// Returns whether it closed. The marker for this arm is on its CALLER, in
    /// `screens/map.rs`: the gesture is a click on the campaign map and this is
    /// only the four-line helper it reaches.
    pub fn dismiss_unless_question(&mut self) -> bool {
        match self.open {
            Some(r) if !r.is_question() => {
                self.close();
                true
            }
            _ => false,
        }
    }
}

/// What `Msg_Dismiss` asks the caller to do after it has closed the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismissal {
    /// Nothing; the window closed.
    Closed,
    /// **The game is over.** `Msg_Dismiss`'s last three lines:
    /// `if (outcome == 10 || outcome == 11) { Campaign_EnterConquest();
    /// g_screenId = 0x1C; }`. A game ends when the message that set the outcome
    /// is dismissed, not when the outcome is written.
    GameOver(Outcome),
    /// The window was not open.
    Nothing,
}

/// **`Msg_Dismiss` (`0x00476768`).**
///
/// Free function rather than a method because the ending arm reads
/// [`crate::victory::Campaign`] and can enter the conquest screen, which is the
/// whole `Game`'s business and not the ring's.
///
/// What is deliberately **not** here: the arm for groups `0x9E`, `0x9F`, `0xEE`
/// and `0xEF`, which drops into screen `0x26` and sets `g_quitRequest = 3`.
/// Those four groups are the demo's *"Congratulations"* / *"Defeat"* posters and
/// **nothing in the binary enqueues any of them** — `docs/formats/eng.md` marks
/// all four dead. Building it would be a `dead-reproduced` arm, which
/// `crates/l2-game/tests/arms.rs` asserts stays empty. `docs/arms.json` records
/// it as `dead`.
pub fn dismiss(game: &mut Game) -> Dismissal {
    if !game.messages.is_open() {
        return Dismissal::Nothing;
    }
    game.messages.close();
    let outcome = game.campaign.outcome;
    if outcome.is_over() {
        // `Campaign_EnterConquest` (`0x00497879`), then `g_screenId = 0x1C`.
        game.campaign.enter_conquest_screen();
        return Dismissal::GameOver(outcome);
    }
    Dismissal::Closed
}

/// **`Msg_DrawWindow`'s side effects on the frame the window opens** — the arms
/// that are not drawing at all, and that a reading for text and voice lookups
/// walks straight past.
///
/// Three of them, and each is in a different category's branch:
///
/// * **`0x0B`** opens `if (g_realms[g_localPlayer].ally != 0) { Msg_Dismiss();
///   return; }` — an alliance offer that arrives when you already have an ally
///   closes itself unseen. `Msg_DrawDiplomacy` has the same guard for its own
///   alliance arm (group `0xF8`).
/// * **`0x0E`** runs the outcome ladder — `l2_kingdom::victory::outcome_of` —
///   and, with no opponents left, **enqueues group 225 onto the ring it is being
///   drawn from**. That is `docs/plan.md`'s mainline win.
/// * **`0x04`** clamps the timer; see [`MessageQueue::clamp_tip_timer`].
///
/// Returns whether the window is still up. It is called from the message
/// screen's `update` rather than its `draw`, because [`crate::screen::Screen`]
/// hands `draw` a `&Ctx` on purpose and this changes the world — see
/// `crates/l2-game/src/screen.rs`, *Draw cannot mutate*. The original runs it in
/// the draw; the effect is identical because the original's draw and input both
/// run once per frame, and the difference is recorded here rather than hidden.
pub fn show(game: &mut Game) -> bool {
    let Some(record) = game.messages.open().copied() else { return false };
    // arm: 0x0047309E/tip-timer-clamp
    game.messages.clamp_tip_timer();
    if !game.messages.just_opened() {
        return true;
    }
    match record.category {
        // arm: 0x0047309E/alliance-offer-lapses
        category::ALLIANCE_PROMPT => {
            let ally = game.kingdom.realms.get(game.player as usize).map_or(0, |r| r.ally);
            if ally != 0 {
                dismiss(game);
                return false;
            }
        }
        // arm: 0x0047309E/ending-sets-outcome
        category::ENDING => {
            let step = victory::outcome_of(
                record.as_ending(),
                game.player,
                game.campaign.ranking,
                game.kingdom.options.quirks,
            );
            match step {
                OutcomeStep::Set(o) => game.campaign.outcome = o,
                OutcomeStep::EnqueueVictory => {
                    game.campaign.outcome = Outcome::InPlay;
                    let victory = victory::victory_message(game.player);
                    let player = game.player;
                    game.messages.enqueue(Record::from(victory), player);
                }
            }
        }
        // The diplomatic letter carries the same alliance guard for group 0xF8
        // only — `Msg_DrawDiplomacy`'s `iVar2 == 3` arm.
        // arm: 0x00475E07/letter-alliance-lapses
        category::DIPLOMACY if record.group == 0xF8 => {
            let ally = game.kingdom.realms.get(game.player as usize).map_or(0, |r| r.ally);
            if ally != 0 {
                dismiss(game);
                return false;
            }
        }
        _ => {}
    }
    true
}

/// **Show and dismiss every queued message at once**, and return the outcome.
///
/// This is what a *headless* turn does in place of the frame loop: it is
/// `Msg_Pump` + [`show`] + [`dismiss`] run to exhaustion, with no window and no
/// click. [`crate::turn::end_turn`] is the headless door — `docs/agents.md`,
/// *name the branch* — and it cannot raise a screen, so a game driven through it
/// still ends, and ends by the same ladder an interactive game does.
///
/// **It is not a second implementation of the rules.** Every step goes through
/// the same [`show`] and [`dismiss`] the message screen calls, which is the
/// point: an ending settled headlessly and an ending settled by a person
/// pressing the corner button cannot disagree, because there is one ladder.
///
/// It stops at the first message that ends the game, because in the original
/// dismissing that message enters screen `0x1C` and nothing behind it in the
/// ring is ever shown.
pub fn drain(game: &mut Game) -> Outcome {
    // Fifty slots and one enqueue-while-draining (group 225), so a hundred
    // iterations is a hard bound that a full ring cannot reach. A `while true`
    // here would be one ring-corruption away from hanging the turn.
    for _ in 0..(RING * 2) {
        if !game.messages.is_open() && !game.messages.pull() {
            if game.messages.is_empty() {
                break;
            }
            continue;
        }
        if !show(game) {
            continue;
        }
        if let Dismissal::GameOver(o) = dismiss(game) {
            return o;
        }
    }
    game.campaign.outcome
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
/// constant is kept so the list is the binary's rather than ours.
pub const PUMP_SCREENS: [u8; 4] = [0x00, 0x27, 0x0F, 0x29];

/// The job panel only pumps for job **8** — `g_jobPanelJob == 8`, the ninth
/// labour slot.
pub const PUMP_JOB: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(to: u8, group: u16) -> Record {
        Record { to, group, ..Record::default() }
    }

    /// **The filter that makes this display state and not simulation state.**
    #[test]
    fn a_message_addressed_to_another_realm_never_enters_this_peers_ring() {
        let mut q = MessageQueue::new();
        assert!(q.enqueue(notice(0, 0x92), 1), "realm 0 is everybody");
        assert!(q.enqueue(notice(1, 0x92), 1), "and me");
        assert!(!q.enqueue(notice(3, 0x92), 1), "and not realm 3");
        assert_eq!(q.queued(), 2);
    }

    #[test]
    fn a_pulled_message_opens_a_window_and_empties_its_slot() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        assert!(!q.is_open());
        assert!(q.pull());
        assert_eq!(q.open().map(|r| r.group), Some(0x92));
        assert_eq!(q.timer(), TIMER_START);
        assert!(q.just_opened());
        assert_eq!(q.queued(), 0, "the slot was cleared");
    }

    /// `Msg_Pump` only pulls when the timer has run out, and in single player it
    /// never does. So a second message waits behind the first.
    #[test]
    fn a_second_message_waits_behind_the_first() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.enqueue(notice(0, 0x93), 1);
        assert!(q.pull());
        assert_eq!(q.open().map(|r| r.group), Some(0x92));
        assert!(!q.pull(), "the timer is at 2000");
        q.close();
        assert!(q.pull());
        assert_eq!(q.open().map(|r| r.group), Some(0x93));
    }

    /// **Single player never times out.** Two thousand ticks and the message is
    /// still there, clamped at 1.
    #[test]
    fn in_single_player_a_message_waits_for_ever() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.pull();
        for _ in 0..(TIMER_START + 500) {
            assert_eq!(q.advance(false), Tick::Open);
        }
        assert!(q.is_open());
        assert_eq!(q.timer(), 1, "clamped, not expired");
    }

    /// **In a network game it goes after 399 ticks** — `2000 - 0x641`.
    #[test]
    fn in_a_network_game_a_message_expires_after_three_hundred_and_ninety_nine_ticks() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.pull();
        let mut ticks = 0;
        while q.is_open() {
            assert_eq!(ticks < 1000, true, "it must expire");
            if q.advance(true) == Tick::TimedOut {
                break;
            }
            ticks += 1;
        }
        assert_eq!(ticks + 1, TIMER_START - MULTIPLAYER_TIMEOUT + 1);
        assert!(!q.is_open());
    }

    /// …but an **ending** does not, on either kind of game. It is the one
    /// category the multiplayer timeout exempts.
    #[test]
    fn an_ending_never_times_out_even_in_a_network_game() {
        let mut q = MessageQueue::new();
        q.enqueue(
            Record { group: 225, category: category::ENDING, ..Record::default() },
            1,
        );
        q.pull();
        for _ in 0..TIMER_START * 2 {
            assert_eq!(q.advance(true), Tick::Open);
        }
        assert!(q.is_open());
    }

    /// The tip is the other exemption, in the other direction: it goes even in
    /// single player, because the draw shortens its timer to 100.
    #[test]
    fn a_tip_closes_itself_and_has_no_button_to_close_it_with() {
        let mut q = MessageQueue::new();
        q.enqueue(
            Record { group: 0xE6, category: category::TIP, ..Record::default() },
            1,
        );
        q.pull();
        assert!(!Shape::of(category::TIP).has_ok_button());
        q.clamp_tip_timer();
        assert_eq!(q.timer(), TIP_TIMER);
        // The tip's own branch is only reached in a network game; in single
        // player the first branch clamps it to 1 and it stays up. That is the
        // original, and it is why `Msg_Pump`'s ladder tests multiplayer FIRST.
        for _ in 0..TIP_TIMER {
            q.advance(false);
        }
        assert!(q.is_open(), "single player: clamped to 1, like everything else");
    }

    /// **A gap in the ring costs one call, not a search.** `Msg_Pump` advances
    /// the tail by one and returns when it lands on an empty slot.
    #[test]
    fn an_empty_slot_costs_one_pull_each() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.enqueue(notice(0, 0x93), 1);
        q.pull();
        q.close();
        q.pull();
        q.close();
        // Both gone; the tail is at 2 and the head at 2, so the next pull sees
        // an empty slot with tail == head and stops.
        assert!(!q.pull());
        assert!(!q.is_open());
    }

    /// The four question categories and the four widget tests are the same four.
    #[test]
    fn the_categories_that_survive_a_map_click_are_the_ones_with_an_answer() {
        for c in 0u8..=0x14 {
            let r = Record { group: 1, category: c, ..Record::default() };
            let questioned = r.is_question();
            let has_widget = matches!(
                c,
                category::GARRISON_PROMPT
                    | category::PAY_PROMPT
                    | category::ALLIANCE_PROMPT
                    | category::DIPLOMACY
            );
            assert_eq!(questioned, has_widget, "category {c:#x}");
        }
    }

    #[test]
    fn a_map_click_closes_a_notice_and_leaves_a_question_standing() {
        let mut q = MessageQueue::new();
        q.enqueue(notice(0, 0x92), 1);
        q.pull();
        assert!(q.dismiss_unless_question());
        assert!(!q.is_open());

        q.enqueue(
            Record { group: 180, category: category::ALLIANCE_PROMPT, ..Record::default() },
            1,
        );
        q.pull();
        assert!(!q.dismiss_unless_question());
        assert!(q.is_open(), "an alliance offer is not closed by a stray click");
    }

    /// The OK hit box is **twice** the picture, and off-centre by nothing:
    /// twelve pixels of slop on each side.
    #[test]
    fn the_ok_hitbox_is_forty_eight_square_round_a_twenty_four_square_button() {
        let f = frame_of(&Record { group: 1, category: category::NOTICE, ..Record::default() })
            .expect("category 0 has a constant frame");
        let (bx, by) = f.ok_button();
        assert_eq!((bx, by), (0x20 + 0x1A0 - 0x30, 0xA0 + 0xE0 - 0x30));
        let hit = f.ok_hitbox();
        assert_eq!((hit.w, hit.h), (0x30, 0x30));
        assert!(hit.contains(bx, by));
        assert!(hit.contains(bx - 12, by - 12), "twelve pixels above and left");
        assert!(hit.contains(bx + 35, by + 35), "and beyond the picture's corner");
        assert!(!hit.contains(bx - 13, by));
    }

    /// The five prompts are five distinct tables and three distinct positions,
    /// and every one is a 36-pixel step with a 4-pixel drop.
    #[test]
    fn every_prompt_is_a_thumb_up_and_a_thumb_down_thirty_six_pixels_apart() {
        for p in [
            Prompt::Garrison,
            Prompt::PayForHelp,
            Prompt::AcceptAlliance,
            Prompt::AnswerHelpRequest,
            Prompt::AnswerAttackRequest,
        ] {
            let [yes, no] = p.widgets();
            assert_eq!(no.0 - yes.0, 36, "{p:?}");
            assert_eq!(no.1 - yes.1, 4, "{p:?}");
            assert_eq!(p.hit(yes.0 + 1, yes.1 + 1), Some(true), "{p:?}");
            assert_eq!(p.hit(no.0 + 1, no.1 + 1), Some(false), "{p:?}");
            assert_eq!(p.hit(0, 0), None, "{p:?}");
        }
    }

    /// The paragraph categories are a *range* and the count is in the byte.
    #[test]
    fn the_paragraph_categories_carry_their_own_count() {
        assert_eq!(Shape::of(5), Shape::Paragraphs(1));
        assert_eq!(Shape::of(9), Shape::Paragraphs(5));
        assert_eq!(Shape::of(4), Shape::Tip);
        assert_eq!(Shape::of(10), Shape::Prompt);
    }

    /// A category nobody enqueues draws nothing **and cannot be left with the
    /// left button**, because every `Ui_OkButton` call is inside an arm.
    #[test]
    fn an_unhandled_category_has_no_button() {
        assert_eq!(Shape::of(0x15), Shape::Unhandled);
        assert!(!Shape::of(0x15).has_ok_button());
        assert!(frame_of(&Record { group: 1, category: 0x15, ..Record::default() }).is_none());
    }
}
