//! **Holding a button down**, which the original does in exactly one place and
//! we did in none.
//!
//! A player reported it as two separate things and they are one thing:
//!
//! > *"Clicking yes/no (gauntlet thumbs up and down) is instant, whereas the
//! > game waited on mouse-up, and the gauntlet would go down slightly when
//! > clicked."*
//! >
//! > *"Holding on a button doesn't seem to make it go up faster. I recall you
//! > could click an up arrow and after a few seconds the number would go up
//! > fast."*
//!
//! Both are the **kind byte** of the original's widget record, and
//! `docs/input.md` is the whole model. The short version, because this module
//! is one half of it:
//!
//! `Widget_Test` (`0x0040DA1E`) walks a table of 24-byte records and reads a
//! kind at `+0x0F`. Kind **4** — every `+`/`−` in the game — fires on the
//! **press**, shows the pressed frame for three frames, and **auto-repeats with
//! acceleration** while the button stays down on it. Kind **5** — the yes/no
//! box's two gauntlets, the divide screen's confirm, diplomacy's send — fires
//! **twenty frames after the press**, with the pressed frame up the whole time.
//! That delay is what reads as *"the game waited on mouse-up"*.
//!
//! # What is here and what is not
//!
//! Here: the acceleration table, the schedule it produces, and the two state
//! machines. **Not** here: the artwork. The pressed frame is `base + 1` on the
//! same sheet — `Widget_Draw` (`0x0040CFD2`) adds one to the frame at `+0x04`
//! whenever the press timer at `+0x0D` is non-zero — so it is a fact about a
//! *sprite index*, and the painters own it. [`Press::is_pressed`] is what a
//! painter asks, one record at a time.
//!
//! # Determinism
//!
//! `docs/netcode.md`: an auto-repeat is a timer, and a timer that feeds the
//! simulation is a lockstep surface. **This one does not feed it.** It sits
//! entirely on the input side: it consumes ticks and produces *discrete fires*,
//! and a fire becomes an ordinary command. Two peers
//! running at different frame rates therefore produce different *numbers* of
//! commands, which is correct — a player who holds an arrow longer steps the
//! number further — and never a different *result* from the same commands.
//!
//! Nothing here reads a clock. It counts [`TICK_MS`] ticks, because
//! `crate::input`'s rule is that no screen may be told how much time passed.

use crate::input::{Event, Rect};

/// One fixed simulation tick, in milliseconds.
///
/// Duplicated from `crate::battlefield` deliberately: this
/// module must not depend on the battle, and the number is the machine's, not
/// either module's. If they ever disagree the test below says so.
pub const TICK_MS: u32 = 16;

/// **The auto-repeat gate table at `0x004D2748`**, read out of `Lords2.exe`.
///
/// `Widget_Test`'s repeat is a **48-byte
/// hand-authored ramp**, indexed by how many 30 ms steps the button has been
/// held for, and the button fires on a step whose entry is non-zero:
///
/// ```text
/// idx:  0  1  2  3  4  5  6  7 | 8  9 10 11 12 13 14 15 16 17 18 19 20 21 22 23
/// val:  8  8  8  8  8  8  8  8 | 1  0  0  0  0  0  1  0  0  0  0  1  0  0  0  1
/// idx: 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
/// val:  0  0  1  0  0  1  0  0  1  0  1  0  1  0  0  1  1  1  1  1  1  1  1  0
/// ```
///
/// **The first eight entries are never read** — the code returns before
/// touching the table while the counter is under 8 — and neither is entry 47,
/// because the counter clamps *to* 47 on the step that would have made it 48
/// and that branch fires without consulting the table at all. They are kept in
/// the constant because the constant is a copy of the bytes, and a copy with
/// the dead entries edited out is a copy somebody has already interpreted.
///
/// The wobble at 36 → 39 is in the game. It is not smoothed here.
pub const REPEAT_GATE: [u8; 48] = [
    8, 8, 8, 8, 8, 8, 8, 8, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0,
    0, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0,
];

/// Address of [`REPEAT_GATE`] in `Lords2.exe`, for the check that pins it.
pub const REPEAT_GATE_ADDR: u32 = 0x004D_2748;

/// Steps below this fire nothing: `if (rec[0x0E] < 8) return 0;`
pub const REPEAT_FIRST_STEP: u8 = 8;

/// The counter clamps here, and the clamped branch **skips the table** —
/// from this step onward the button fires on every step, which is the *"and
/// then it goes fast"* the player remembers.
pub const REPEAT_CLAMP: u8 = 0x2F;

/// **30 ms**, the gate `FUN_004B20ED` puts on the repeat counter.
///
/// It is a `timeGetTime()` difference tested against `0x1E`, evaluated once per
/// frame in the whole-game frame function and stored in `DAT_004EA128`, and the
/// timestamp is **not advanced** when the difference is under 30. So the
/// counter advances at most once per 30 ms and at most once per frame,
/// whichever is slower.
pub const REPEAT_STEP_MS: u32 = 30;

/// How long the pressed frame stays up for a kind-4 button after the press:
/// `rec[0x0D] = 3`, refreshed to 3 on every frame the button is still held over
/// it, so in practice it is *three frames after the release*.
pub const PRESS_FRAMES: u8 = 3;

/// **Twenty**, and this is the number the player felt. `Widget_Test`'s kind-5
/// branch sets `rec[0x0D] = 0x14` and returns **without calling the handler**;
/// the handler runs in the next call's countdown loop, on the frame the timer
/// reaches zero.
pub const DELAYED_FRAMES: u8 = 20;

/// **320 ms**, `Hotspot_Test`'s kind-2 repeat, which is a flat pulse and not a
/// ramp: `DAT_0057D3C8` is one of `Tick_Pulses`' eight dividers — every four of
/// the 80 ms pulses. The two repeats are different mechanisms and conflating
/// so [`Kind::Held`] is a separate
/// branch of [`Press::tick`].
pub const HELD_PULSE_MS: u32 = 320;

/// **The kind byte at `+0x0F` of the original's 24-byte input record, as a
/// type.**
///
/// This is the whole of what decides press, release, hold or repeat, and until
/// it existed here a screen answered a gesture by hand-rolling its own
/// press/release bookkeeping — which is how `docs/arms.json` came to mark
/// nineteen arms `reproduced` under a kind none of them had. A screen
/// now **declares** the kind, in a [`Widget`] table, and [`Press::event`]
/// decides when the handler runs.
///
/// The five are the original's own: `Widget_Test`
/// (`0x0040DA1E`) tests `+0x0F` against 4 and 5 and ignores every other value,
/// `Hotspot_Test` (`0x0040E3EE`) against 1, 3 and 2 in that order. **The
/// numbers do not overlap between the two testers** — `Hotspot_Test`'s 3 is a
/// release and `Widget_Test` has no 3 that fires at all — so the tester is part
/// of the question, which is what [`Kind::from_record`] takes.
///
/// `docs/input.md` is the model in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// `Hotspot_Test` kind **1** — fires on the down edge, draws nothing.
    /// The majority of the interface.
    ///
    /// It reads `g_mouseLeftPressed` **and not** `g_mouseLeftDoubleClick`, so
    /// the second click of a double click does not fire it: Windows sends
    /// `WM_LBUTTONDBLCLK`
    /// behaviour, not an oversight, and [`Press::event`] keeps it.
    Press,
    /// `Hotspot_Test` kind **2** — the down edge, then a **flat**
    /// [`HELD_PULSE_MS`] pulse for as long as the button stays down on it.
    ///
    /// `if (kind != 2 || !(down || pressed || doubleClick)) skip;` then
    /// `if (pressed || doubleClick) fire; else if (DAT_0057D3C8) fire;`.
    Held,
    /// `Hotspot_Test` kind **3** — fires on `g_mouseLeftReleased`, the **up**
    /// edge.
    ///
/// It carries no memory of a press: the tester hit-tests the box and
    /// reads the released flag,
    /// whether or not the press that preceded it happened there.
    Release,
    /// `Widget_Test` kind **4** — the down edge, the pressed picture for
    /// [`PRESS_FRAMES`] frames, and an **accelerating** auto-repeat off
    /// [`REPEAT_GATE`].
    ///
    /// Every `+`/`−`, every `<`/`>`, every up/down arrow in the game.
    Repeat,
    /// `Widget_Test` kind **5** — the down edge puts the pressed picture up and
    /// the handler runs [`DELAYED_FRAMES`] frames **later**.
    ///
    /// Every yes/no gauntlet, every options checkbox, diplomacy's send. *"The
    /// game waited on mouse-up, and the gauntlet would go down slightly when
    /// clicked."*
    Delayed,
}

impl Kind {
    /// **The word `docs/arms.json` files this kind under**, so the marker beside
    /// an arm and the type the code answers it with cannot drift apart by
    /// somebody editing one of them.
    pub const fn gesture(self) -> &'static str {
        match self {
            Kind::Press => "left-press",
            Kind::Held => "left-press-held",
            Kind::Release => "left-release",
            Kind::Repeat => "left-press-repeat",
            Kind::Delayed => "left-press-delayed",
        }
    }

    /// The kind a record's `+0x0F` byte means. `widget` says which tester walks
    /// the table, because the two use the same record and different numbers.
    pub const fn from_record(widget: bool, byte: u8) -> Option<Kind> {
        Some(match (widget, byte) {
            (false, 1) => Kind::Press,
            (false, 2) => Kind::Held,
            (false, 3) => Kind::Release,
            (true, 4) => Kind::Repeat,
            (true, 5) => Kind::Delayed,
            _ => return None,
        })
    }

    /// **Does a widget of this kind show the pressed picture?**
    ///
    /// Only the two `Widget_Test` kinds: `Widget_Draw` (`0x0040CFD2`) adds one
    /// to the frame at `+0x04` for kinds 4 and 5 and for nothing else, and
    /// `Hotspot_Test`'s records are never drawn at all.
    pub const fn has_pressed_frame(self) -> bool {
        matches!(self, Kind::Repeat | Kind::Delayed)
    }
}

/// **The marker and the declaration, as one token.**
///
/// `arm!("0x00437AFB/divide-confirm", Delayed)` *is* `Kind::Delayed` — it
/// expands to nothing else — and it is also the `docs/arms.json` marker for
/// that arm. `crates/l2-game/tests/arms.rs` reads the id out of the first
/// argument and the gesture out of the second, through [`Kind::gesture`],
/// the word the marker claims and the kind the widget is answered with are
/// the same identifier and cannot drift apart.
///
/// That check used to compare a comment's word with a record's word and never
/// look at the `Kind` beside it: every options row was once declared
/// `Kind::Press` under a `left-press-delayed` comment and `arms.rs` stayed
/// green. **So a comment marker may not name the three kinds only this module
/// answers** — `left-press-repeat`, `left-press-delayed`, `left-press-held` —
/// and `arms.rs` refuses one that does. A plain press or a release can still
/// be marked by comment, because hand-rolled tests answer those.
#[macro_export]
macro_rules! arm {
    ($id:literal, $kind:ident $(,)?) => {
        $crate::press::Kind::$kind
    };
}

/// **One record of a screen's input table** — a rectangle and the kind of
/// gesture it answers.
///
/// The original's record carries the geometry, the sprite frame, the handler
/// pointer, three counters and the kind, in 24 bytes. Ours carries the geometry
/// and the kind: the handler is the arm of the `match` the screen writes around
/// the index this returns, the counters live in [`Press`] — indexed by the same
/// index, one press timer per record — and the frame belongs to the painter.
///
/// **The hit box is a square in the original and a rectangle here.**
/// `Widget_Test` reads `+0x06` as the side of a square — it uses `table[3]` for
/// both axes, so every widget record's width equals its height — and
/// `Hotspot_Test` reads the same four shorts as `{x0, y0, x1, y1}`. A [`Rect`]
/// expresses both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Widget {
    pub rect: Rect,
    pub kind: Kind,
}

impl Widget {
    pub const fn new(rect: Rect, kind: Kind) -> Widget {
        Widget { rect, kind }
    }
}

/// Does the button fire on this step of the hold?
///
/// `step` is the counter at `+0x0E` **after** its increment.
/// original tests it.
pub fn fires_on_step(step: u8) -> bool {
    if step >= REPEAT_CLAMP + 1 {
        // The clamp branch: `rec[0x0E] = 0x2F` and fall straight through to the
        // call. It does not look at the table.
        return true;
    }
    if step < REPEAT_FIRST_STEP {
        return false;
    }
    REPEAT_GATE[step as usize] != 0
}

/// **How many records one [`Press`] can time.** An index into a screen's
/// table must be below this.
///
/// The largest table a screen of ours hands to [`Press`] is the army-division
/// screen's eighteen (`g_splitWidgets`). The bound is a bitmask's width
/// ([`Fired`]), and it is asserted on every press,
/// bigger table fails its first test
pub const MAX_WIDGETS: usize = 32;

/// **The handlers one tick owes a screen**, in the order `Widget_Test` calls
/// them.
///
/// First every kind-5 record whose countdown reached zero, in record order —
/// the loop at the top of `Widget_Test` (`0x0040DA1E`) walks the whole table
/// and **does not return after a fire**, so two can expire in one call. Then
/// the held record's repeat, which is the hit test below that loop.
///
/// In the original two kind-5 records cannot expire on the same frame, because
/// the hit test returns on its first hit and so arms one record per frame. Ours
/// can receive two `Event::Click`s between ticks, and would lose the second if
/// this were an `Option`.
#[must_use = "a fire that is not run is a button that did nothing"]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fired {
    /// Bit `i` set: record `i`'s twenty frames ran out on this tick.
    delayed: u32,
    /// The held record's auto-repeat step or flat pulse.
    held: Option<usize>,
}

impl Iterator for Fired {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.delayed != 0 {
            let i = self.delayed.trailing_zeros() as usize;
            self.delayed &= self.delayed - 1;
            return Some(i);
        }
        self.held.take()
    }
}

/// **One widget table's input state**: the held button, and a press timer for
/// every record.
///
/// Ours in shape and the original's in behaviour. `Widget_Test` keeps this
/// state *per record*, in the record; a screen module here keeps one of these,
/// indexed by the same index as the table it hands to [`Press::event`],
/// because our screens hit-test their own rectangles
/// table.
///
/// **One press timer per record, not one per table.** `+0x0D` is a byte of
/// each 24-byte record, and the countdown loop at the top of `Widget_Test`
/// decrements every record's timer on every call, so two gauntlets pressed a
/// few frames apart are both down and **both** act, each twenty frames after
/// its own press. This struct held one timer and one pending widget until
/// that was measured; the second press overwrote the first, and the first
/// button did nothing.
///
/// Drive it with the three things a screen already hears —
/// [`Press::press`] from `Event::Click`, [`Press::pointer`] from
/// `Event::Pointer`, [`Press::release`] from `Event::Release` — and call
/// [`Press::tick`] once per `Screen::update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    /// Which widget is down, if any. An index into whatever the screen calls
    /// its buttons. Only one can be: there is one left button.
    held: Option<usize>,
    /// **How the held widget repeats.** [`Kind::Repeat`] walks
    /// [`REPEAT_GATE`]; [`Kind::Held`] is a flat [`HELD_PULSE_MS`] pulse off a
    /// different clock in a different tester. Nothing else repeats, and a
    /// widget of another kind never becomes `held`.
    held_kind: Kind,
    /// `+0x0E`, the repeat counter, in 30 ms steps.
    step: u8,
/// Milliseconds accumulated toward the next step. Reset
    /// decremented, because `FUN_004B20ED` sets its timestamp to *now* on a
    /// step and leaves it alone otherwise.
    since_step: u32,
    /// **`+0x0D` of every record**, the press timer, in frames. Non-zero means
    /// that record's pressed frame is showing.
    ///
    /// Indexed by widget, which is what makes *"which button is drawn down"*
    /// and *"which button's timer is running"* the same question — as they are
    /// in the original, where the timer lives in the record. A kind-4 button
    /// stays down for three frames **after** the release, which is the whole
/// reason `rec[0x0D]` is set to 3, so the held widget is
    /// not the answer to either.
    timers: [u8; MAX_WIDGETS],
    /// **Bit `i` set: record `i` is kind 5 and its countdown is running**, so
    /// its handler runs when its timer reaches zero.
    ///
    /// The original tests `rec[0x0F] == 5` at that moment. Ours has no table in
    /// [`Press::tick`], so the kind is noted on the press instead; a screen's
    /// table does not change kind between a press and its twentieth frame.
    delayed: u32,
    /// **How many times this table has played the click**, drained by
    /// [`Press::take_clicks`].
    ///
    /// `Widget_Test` (`0x0040DA1E`) calls `Sound_RestartSlot(1)` — `click3.wav`
    /// — from inside the hit test, at two sites: the kind-4 arm and the kind-5
    /// arm. This module *is* that hit test, so the count belongs here and
    /// nowhere else; what it must not do is reach the audio layer, because
    /// `docs/netcode.md` D-3 makes [`crate::audio::Audio`] unreachable from a
    /// screen on purpose. So this is an **outbox**: write-only from here,
    /// drained upward by [`crate::screen::Machine::handle`], and nothing that
    /// happens to it can be read back by the thing that filled it.
    ///
    /// A `u8` and not a `u32` because it is emptied on every event; it saturates
/// so that "some clicks happened" can never round to
    /// "none".
    clicks: u8,
    /// **A tick changed what the table's screen shows**, drained by
    /// [`Press::take_redraw`].
    ///
    /// Set when [`Press::tick`] fires a handler or brings a pressed picture back
    /// up. An event repaints on its own — `Machine::handle` marks the machine
    /// dirty for every one —
    /// the delayed fire arrive with **no** event, and until this existed a held
    /// arrow stepped the number on every pulse and the screen showed it only
    /// when the button came up.
    redraw: bool,
}

impl Default for Press {
    fn default() -> Press {
        Press::new()
    }
}

impl Press {
    pub const fn new() -> Press {
        Press {
            held: None,
            held_kind: Kind::Repeat,
            step: 0,
            since_step: 0,
            timers: [0; MAX_WIDGETS],
            delayed: 0,
            clicks: 0,
            redraw: false,
        }
    }

    /// **Whether a tick changed what the screen shows**, and forget it.
    ///
    /// The original repaints on the frame a handler runs, by one of two roads,
    /// and both are `[V]`:
    ///
    /// * **the handler paints.** `Tax_IncreaseCounty` (`0x0043AA83`) ends
    ///   `Panel_Tax()`; `FUN_00436372`, the gift stepper, and `SaveLoad_Scroll`
    ///   (`0x00434346`) set `g_redrawRequest = 2`, which `Battle_Frame` turns
    ///   into `Screen_Draw` on the same frame;
    /// * **the frame loop paints.** `SplitScreen_ToParent` (`0x00437D65`),
    ///   `FUN_0043B27A` and `FUN_00435339` change a number and paint nothing,
    ///   and `Battle_Frame` calls `Screen_DrawWidgets` (`0x004BA26E`) every
    ///   frame, whose `0x11`, `0x18` and `0x0C` arms run
    ///   `Screen_SplitArmyRows`, `FUN_0041AEA2` and `Trade_DrawPanel` — the
    /// number and the widgets — before `Widget_Draw`.
    ///
    /// Either way the number a held arrow steps is on screen on the frame it
    /// stepped. Ours repaints when the machine is dirty,
    /// one of these hands this up through [`crate::screen::Screen::take_redraw`].
    pub fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.redraw)
    }

    /// **`DAT_00591554` — the repeat counter the handler is called with.**
    ///
    /// `Widget_Test` publishes `0` on the press and `rec[0x0E]` — the step,
    /// clamped at `0x2F` — on every repeat fire, just before the call. Only one
    /// family of handlers reads it: the trade panel's up and down,
    /// `FUN_00435339` and `FUN_0043543D`, which step by one below `0x2C` and by
    /// **ten** from there. So a held arrow on that panel walks one at a time
    /// for 1.3 s and then ten at a time. `[V]`
    pub fn repeat_step(&self) -> u8 {
        if self.held.is_some() {
            self.step
        } else {
            0
        }
    }

    /// The record's slot, refusing an index past [`MAX_WIDGETS`] loudly.
    fn slot(widget: usize) -> usize {
        assert!(
            widget < MAX_WIDGETS,
            "widget {widget} is past press::MAX_WIDGETS ({MAX_WIDGETS}); raise it rather than \
             letting a table time the wrong button",
        );
        widget
    }

    /// **Take the clicks this table owes the audio layer**, and forget them.
    ///
    /// The shape of [`crate::screen::Screen::take_redraw`], and for the same
    /// reason: the thing that produced it must not be able to observe what was
    /// done with it. See the field.
    pub fn take_clicks(&mut self) -> u8 {
        core::mem::take(&mut self.clicks)
    }

    /// **`Sound_RestartSlot(1)`, inside the hit test.**
    ///
    /// The two sites are `Widget_Test`'s kind-4 arm and its kind-5 arm, and
    /// both are guarded by `g_mouseLeftPressed || g_mouseLeftDoubleClick` — the
    /// *initial press*. Neither the auto-repeat's later pulses nor kind 5's
    /// delayed fire go past this line, and `Hotspot_Test` has no such line at
/// all, so this is called from [`Press::press`] and
    /// [`Press::press_delayed`] and **not** from [`Press::press_held`] or
    /// [`Press::tick`].
    fn click(&mut self) {
        self.clicks = self.clicks.saturating_add(1);
    }

    /// **Answer one event against a screen's table**, with each widget's own
    /// kind deciding what happens.
    ///
    /// The return is *"run this widget's handler now"*, an index into `table`.
    /// It is `None` for a [`Kind::Delayed`] press — that one fires out of
    /// [`Press::tick`], twenty ticks later — and for every event that misses.
    ///
    /// This is the two hit-testers, and it is the reason the module exists: a
    /// screen names its rectangles and their kinds and stops keeping
    /// press/release state of its own.
    pub fn event(&mut self, table: &[Widget], event: Event) -> Option<usize> {
        match event {
            Event::Click { x, y } => {
                let i = table.iter().position(|w| w.rect.contains(x, y))?;
                match table[i].kind {
                    // `Hotspot_Test` kind 1: publish and call, nothing else.
                    Kind::Press => Some(i),
                    Kind::Held => {
                        self.press_held(i);
                        Some(i)
                    }
                    Kind::Repeat => {
                        self.press(i);
                        Some(i)
                    }
                    Kind::Delayed => {
                        self.press_delayed(i);
                        None
                    }
                    // The press is not this widget's gesture. It is not a miss
                    // either — the release below is what it waits for.
                    Kind::Release => None,
                }
            }
            // **`g_mouseLeftDoubleClick` is a press for kinds 2, 4 and 5, and
            // not for 1 or 3.** `Widget_Test` (`0x0040DA1E`) guards both of its
            // arms with `g_mouseLeftPressed || g_mouseLeftDoubleClick`, and
            // `Hotspot_Test` (`0x0040E3EE`) its kind-2 arm; kind 1 reads
            // `g_mouseLeftPressed` alone and kind 3 `g_mouseLeftReleased`
            // alone. Since Windows sends the double click
            // second press, that is the difference between a spinner that steps
            // twice on a fast double click and a box that answers once. `[V]`
            //
            // **And a double click does not hold the button down.**
            // `App_WndProc` (`0x004B29BE`) handles `0x203` by setting bit 0 of
            // `DAT_004EADA1` and nothing else — `0x201` is the only message that
            // sets the down bit — so `g_mouseLeftDown` stays 0 for as long as
            // the second press is held, and `Widget_Test`'s hold branch returns
            // at `if (g_mouseLeftDown == 0) return 0;`. A double click on a
            // spinner steps once and **does not auto-repeat**, however long the
            // button stays down after it. `[V]`
            Event::DoubleClick { x, y } => {
                let i = table.iter().position(|w| w.rect.contains(x, y))?;
                match table[i].kind {
                    Kind::Held => {
                        self.press_held(i);
                        self.release();
                        Some(i)
                    }
                    Kind::Repeat => {
                        self.press(i);
                        self.release();
                        Some(i)
                    }
                    Kind::Delayed => {
                        self.press_delayed(i);
                        None
                    }
                    Kind::Press | Kind::Release => None,
                }
            }
            Event::Release { x, y } => {
                self.release();
                let i = table.iter().position(|w| w.rect.contains(x, y))?;
                (table[i].kind == Kind::Release).then_some(i)
            }
            // The original re-runs the hit test every frame,
// has walked off the button stops matching and the record's
            // timer is never refreshed. Ours says it.
            Event::Pointer { x, y } => {
                self.pointer(table.iter().position(|w| w.rect.contains(x, y)));
                None
            }
            Event::PointerLeft => {
                self.pointer(None);
                None
            }
            _ => None,
        }
    }

    /// **A kind-4 press.** Fires immediately — the return is *"run the handler
    /// now"* — and starts the hold.
    ///
    /// **It touches no other record.** A kind-5 button pressed a moment
    /// earlier keeps counting down and still acts: `Widget_Test`'s kind-4 arm
    /// writes `+0x0C`, `+0x0D` and `+0x0E` of its own record and nothing else.
    /// This used to clear the one pending delayed press, so pressing a spinner
    /// straight after the supplies thumb cancelled the dispatch.
    pub fn press(&mut self, widget: usize) -> bool {
        let w = Press::slot(widget);
        // sfx: Widget_Test#1
        self.click();
        self.held = Some(w);
        // `Widget_Test`'s hold branch: `rec[0x0E]++` every 30 ms step while
        // this record stays under a held button, firing on `REPEAT_GATE`'s
        // ramp. Every kind-4 widget in the game repeats through this one line.
        self.held_kind = crate::arm!("0x0040DA1E/widget-auto-repeat", Repeat);
        self.step = 0;
        self.since_step = 0;
        self.timers[w] = PRESS_FRAMES;
        self.delayed &= !(1 << w);
        true
    }

    /// **A kind-2 press.** Fires immediately and then every
    /// [`HELD_PULSE_MS`] while the button stays down on it.
    ///
/// **No pressed frame.** `Hotspot_Test` sets the record's `+0x0D`.
    /// `Widget_Test` does, and nothing ever draws a hotspot record — the whole
    /// difference between the two testers is that one owns the visible buttons.
    /// So this leaves [`Press::is_pressed`] false for it, which is what
    /// [`Kind::has_pressed_frame`] says out loud.
    pub fn press_held(&mut self, widget: usize) -> bool {
        let w = Press::slot(widget);
        self.held = Some(w);
        self.held_kind = Kind::Held;
        self.step = 0;
        self.since_step = 0;
        true
    }

    /// **A kind-5 press.** Does *not* fire. Puts this record's pressed frame
    /// up and arms its handler for [`DELAYED_FRAMES`] ticks' time;
    /// [`Press::tick`] returns the widget on the tick it expires.
    ///
    /// **Every other record's countdown carries on**,
    /// pressed before the first has acted does not cancel it. **The same
    /// record pressed again restarts its own**: the kind-5 arm is
    /// `rec[0x0D] = 0x14` whatever the timer held,
    /// within twenty frames — a double click, usually — acts once, twenty
    /// frames after the second press. `[V]`, `Widget_Test` (`0x0040DA1E`).
    pub fn press_delayed(&mut self, widget: usize) {
        let w = Press::slot(widget);
        // sfx: Widget_Test#2
        self.click();
        self.held = None;
        self.step = 0;
        self.since_step = 0;
        self.timers[w] = DELAYED_FRAMES;
        self.delayed |= 1 << w;
    }

    /// The pointer moved. `over` is the widget under it, or `None`.
    ///
    /// The original does not track this: it re-runs the hit test every frame,
///
    /// the record's timer is never refreshed. The effect is the same and this
    /// is how a screen that owns its own rectangles says it.
    pub fn pointer(&mut self, over: Option<usize>) {
        if self.held.is_some() && self.held != over {
            self.held = None;
            self.step = 0;
        }
    }

    /// The button came up. Ends the hold; the pressed frame runs out on its
    /// own, which is what `rec[0x0D] = 3` buys.
    pub fn release(&mut self) {
        self.held = None;
        self.step = 0;
    }

    /// One fixed tick. Returns every widget whose handler should run, in the
    /// order `Widget_Test` runs them — see [`Fired`].
    ///
    /// A screen runs them in turn and stops at the first that leaves the
    /// screen, because a table nobody walks any more fires nothing: the
    /// original's handler writes `g_screenId`, and the next frame's dispatcher
    /// no longer calls `Widget_Test` on that table. Its remaining timers are
/// not lost — ours freezes with the screen underneath.
    /// record's bytes sit untouched in `.data` until the screen comes back.
    pub fn tick(&mut self) -> Fired {
        let mut fired = Fired::default();
        // **The countdown loop at the top of `Widget_Test`**, over every
        // record, whether or not anything is under the pointer:
        //
        //   if (rec[0x0D] != 0 && --rec[0x0D] == 0 && rec[0x0F] == 5) call it;
        //
        // and it goes on to the next record after a call.
        let mut came_up = false;
        for (i, t) in self.timers.iter_mut().enumerate() {
            if *t == 0 {
                continue;
            }
            *t -= 1;
            if *t == 0 {
                came_up = true;
                if self.delayed & (1 << i) != 0 {
                    self.delayed &= !(1 << i);
                    fired.delayed |= 1 << i;
                }
            }
        }
        let fired = self.hold(fired);
        if came_up || fired != Fired::default() {
            self.redraw = true;
        }
        fired
    }

    /// The hit-test half of [`Press::tick`]: the held record's repeat or pulse.
    fn hold(&mut self, mut fired: Fired) -> Fired {
        let Some(held) = self.held else { return fired };
        // **The flat pulse, which is a different tester on a different clock.**
        // `Hotspot_Test`'s kind-2 arm consults `DAT_0057D3C8` — one of
        // `Tick_Pulses`' eight dividers, every fourth 80 ms pulse — and there is
        // no table, no ramp and no pressed picture in it.
        if self.held_kind == Kind::Held {
            self.since_step += TICK_MS;
            if self.since_step >= HELD_PULSE_MS {
                self.since_step = 0;
                fired.held = Some(held);
            }
            return fired;
        }
        // `rec[0x0D] = 3` — the hold keeps the pressed frame up.
        self.timers[held] = PRESS_FRAMES;
        self.since_step += TICK_MS;
        if self.since_step < REPEAT_STEP_MS {
            return fired;
        }
        self.since_step = 0;
        self.step = self.step.saturating_add(1);
        let fires = fires_on_step(self.step);
        if self.step > REPEAT_CLAMP {
            self.step = REPEAT_CLAMP;
        }
        if fires {
            fired.held = Some(held);
        }
        fired
    }

    /// **Is this record's pressed frame showing?** A painter adds one to that
    /// record's sprite index when this is true, which is all `Widget_Draw`
    /// does — for every record, so two buttons can be down at once.
    ///
    /// This replaced a `pressed() -> Option<usize>`, which could name one
    /// button and so drew the first of two pressed gauntlets back up while its
    /// handler was still waiting.
    pub fn is_pressed(&self, widget: usize) -> bool {
        self.timers.get(widget).is_some_and(|&t| t != 0)
    }

    /// Whether any record of the table is drawn down.
    pub fn any_pressed(&self) -> bool {
        self.timers.iter().any(|&t| t != 0)
    }

    /// Whether anything is waiting — a kind-5 press mid-delay, on any record.
    pub fn busy(&self) -> bool {
        self.delayed != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The schedule, spelled out**, because the table is easy to copy and
    /// hard to read.
    ///
    /// These are the steps the button fires on, and they are the acceleration:
    /// six steps between the first two, then five, four, three, three, three,
    /// two, two, three, and then every step. At 30 ms a step that is a first
    /// repeat 240 ms after the press and full speed from 1.44 s — *"click an up
    /// arrow and after a few seconds the number would go up fast."*
    #[test]
    fn the_repeat_accelerates_on_the_schedule_the_table_encodes() {
        let firing: Vec<u8> = (0..=REPEAT_CLAMP).filter(|&s| fires_on_step(s)).collect();
        assert_eq!(
            firing,
            vec![8, 14, 19, 23, 26, 29, 32, 34, 36, 39, 40, 41, 42, 43, 44, 45, 46],
            "the ramp"
        );
        // Nothing at all for the first seven steps: 210 ms of holding before
        // the button does anything a second time.
        assert!((0..8).all(|s| !fires_on_step(s)));
        // And past the clamp, every step.
        assert!((REPEAT_CLAMP + 1..=u8::MAX).all(fires_on_step));
    }

    /// A press fires once and once only, however long you wait, unless the
    /// button is *held*.
    #[test]
    fn a_press_and_release_fires_exactly_once() {
        let mut p = Press::new();
        assert!(p.press(3));
        p.release();
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0, "no repeat after the release");
    }

    /// The whole gesture, in ticks, with the numbers a player would feel.
    #[test]
    fn holding_a_button_repeats_slowly_then_quickly() {
        let mut p = Press::new();
        assert!(p.press(3));
        // 16 ms ticks against a 30 ms step,
        let mut fires = Vec::new();
        for t in 1..=120 {
            if p.tick().any(|w| w == 3) {
                fires.push(t);
            }
        }
        assert_eq!(fires[0], 16, "the first repeat is step 8, and a step is two ticks");
        assert_eq!(&fires[..4], &[16, 28, 38, 46], "steps 8, 14, 19, 23");
        // By the end of the run it is firing every other tick, which is as fast
        // as a 30 ms gate can go.
        let tail = &fires[fires.len() - 5..];
        assert!(
            tail.windows(2).all(|w| w[1] - w[0] == 2),
            "past the clamp every step fires: {tail:?}",
        );
    }

    /// **The pointer leaving the button stops the repeat**, because the
/// original re-hit-tests every frame and stops matching.
    #[test]
    fn sliding_off_the_button_stops_it_repeating() {
        let mut p = Press::new();
        p.press(1);
        for _ in 0..40 {
            let _ = p.tick();
        }
        p.pointer(Some(2));
        assert_eq!((0..200).map(|_| p.tick().count()).sum::<usize>(), 0);
    }

    /// **Kind 5: the press does not fire, and twenty ticks later it does.**
    /// This is the gauntlet.
    #[test]
    fn a_delayed_button_shows_the_pressed_frame_first_and_acts_afterwards() {
        let mut p = Press::new();
        p.press_delayed(0);
        assert!(p.is_pressed(0), "the gauntlet goes down at once");
        assert!(p.busy());
        for t in 1..DELAYED_FRAMES as u32 {
            assert_eq!(p.tick().next(), None, "nothing has happened yet at tick {t}");
            assert!(p.is_pressed(0), "and it is still down");
        }
        assert_eq!(p.tick().collect::<Vec<_>>(), vec![0], "the handler runs on the twentieth tick");
        assert!(!p.is_pressed(0), "and the button comes back up");
        assert!(!p.busy());
    }

    /// A kind-4 press shows the pressed frame too — for three frames, not
    /// twenty — and it comes back up on its own after the release.
    #[test]
    fn a_repeating_button_shows_the_pressed_frame_for_three_frames() {
        let mut p = Press::new();
        p.press(7);
        assert!(p.is_pressed(7));
        p.release();
        let _ = p.tick();
        let _ = p.tick();
        assert!(p.is_pressed(7), "still down two frames after the release");
        let _ = p.tick();
        assert!(!p.is_pressed(7), "and up on the third");
    }

    /// The tick each widget fires on, over `ticks` ticks, with an optional
    /// press of a kind-5 widget injected before a given tick.
    fn run(p: &mut Press, ticks: u32, presses: &[(u32, usize)]) -> Vec<(u32, usize)> {
        let mut out = Vec::new();
        for t in 1..=ticks {
            for &(at, w) in presses {
                if at == t {
                    p.press_delayed(w);
                }
            }
            out.extend(p.tick().map(|w| (t, w)));
        }
        out
    }

    /// **Two gauntlets pressed five ticks apart both act, each on its own
    /// twentieth tick** — `+0x0D` is a byte of each record, and the countdown
    /// loop walks every record.
    ///
    /// **Ablation, run:** make `press_delayed` zero every other record's timer
    /// — the one-pending-press model this replaced — and this goes red with
    /// only the second widget firing.
    #[test]
    fn two_delayed_presses_five_ticks_apart_both_fire_each_on_its_own_twentieth_tick() {
        let mut p = Press::new();
        // Pressed before ticks 1 and 6, so their twentieth ticks are 20 and 25.
        let fired = run(&mut p, 40, &[(1, 0), (6, 1)]);
        assert_eq!(fired, vec![(20, 0), (25, 1)]);
    }

    /// **Both are drawn down while both wait**, which is `Widget_Draw` reading
    /// each record's own timer.
    #[test]
    fn two_waiting_gauntlets_are_both_drawn_down() {
        let mut p = Press::new();
        p.press_delayed(0);
        for _ in 0..5 {
            let _ = p.tick();
        }
        p.press_delayed(1);
        assert!(p.is_pressed(0) && p.is_pressed(1));
    }

    /// **The same gauntlet pressed twice acts once, twenty ticks after the
    /// second press**: the kind-5 arm writes `rec[0x0D] = 0x14` whatever the
    /// timer held.
    #[test]
    fn the_same_delayed_widget_pressed_twice_restarts_and_acts_once() {
        let mut p = Press::new();
        let fired = run(&mut p, 60, &[(1, 2), (8, 2)]);
        assert_eq!(fired, vec![(27, 2)]);
    }

    /// **A spinner pressed while a thumb is waiting does not cancel the thumb.**
    /// `Widget_Test`'s kind-4 arm writes its own record's bytes and no other.
    ///
    /// **Ablation, run:** zero `self.delayed` in `Press::press` and this goes
    /// red — the thumb never acts.
    #[test]
    fn a_kind_four_press_does_not_cancel_a_waiting_kind_five_press() {
        let mut p = Press::new();
        p.press_delayed(6);
        let _ = p.tick();
        assert!(p.press(0), "the spinner fires on its press");
        p.release();
        let fired: Vec<usize> = (0..40).flat_map(|_| p.tick().collect::<Vec<_>>()).collect();
        assert_eq!(fired, vec![6], "and the thumb still acts");
    }

    /// **The countdown and the repeat can fall on one tick**, and both come
    /// back — the delayed fire first, because the countdown loop runs before the
    /// hit test.
    #[test]
    fn a_countdown_and_a_repeat_on_one_tick_both_fire_countdown_first() {
        // The thumb before tick 1 acts on tick 20. The spinner pressed before
        // tick 5 and held makes its first repeat on the sixteenth tick after
        // its press — step 8 of 30 ms steps at 16 ms a tick — which is also
        // tick 20.
        let mut p = Press::new();
        let mut on = Vec::new();
        for t in 1..=20u32 {
            if t == 1 {
                p.press_delayed(5);
            }
            if t == 5 {
                assert!(p.press(0));
            }
            on.push(p.tick().collect::<Vec<_>>());
        }
        assert_eq!(on[19], vec![5, 0], "tick 20: {:?}", on);
        assert!(on[..19].iter().all(Vec::is_empty), "{on:?}");
    }

    /// **`DAT_00591554` is 0 on the press and the clamped counter on each repeat
    /// fire** — the value the trade arrows read to decide between one and ten.
    /// The two numbers are pinned from `Widget_Test`'s body, not from the
    /// constants: the first repeat is step 8 and the clamp writes `0x2F`.
    #[test]
    fn the_published_repeat_step_is_zero_on_the_press_and_the_counter_on_a_repeat() {
        let mut p = Press::new();
        p.press(0);
        assert_eq!(p.repeat_step(), 0, "published as 0 before the press's call");
        let mut seen = Vec::new();
        for _ in 0..200 {
            if p.tick().next().is_some() {
                seen.push(p.repeat_step());
            }
        }
        assert_eq!(seen.first(), Some(&8), "the first repeat is step 8");
        assert_eq!(seen.last(), Some(&0x2F), "and the counter clamps at 0x2F");
        p.release();
        assert_eq!(p.repeat_step(), 0);
    }

    /// Every record past the table's end is refused, loudly.
    #[test]
    #[should_panic(expected = "MAX_WIDGETS")]
    fn a_widget_past_the_bound_is_refused() {
        Press::new().press_delayed(MAX_WIDGETS);
    }
}
