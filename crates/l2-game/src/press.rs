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
//! *sprite index*, and the painters own it. [`Press::pressed`] is what a
//! painter asks.
//!
//! # Determinism
//!
//! `docs/netcode.md`: an auto-repeat is a timer, and a timer that feeds the
//! simulation is a lockstep surface. **This one does not feed it.** It sits
//! entirely on the input side: it consumes ticks and produces *discrete fires*,
//! and a fire becomes an ordinary command exactly as a click does. Two peers
//! running at different frame rates therefore produce different *numbers* of
//! commands, which is correct — a player who holds an arrow longer steps the
//! number further — and never a different *result* from the same commands.
//!
//! Nothing here reads a clock. It counts [`TICK_MS`] ticks, because
//! `crate::input`'s rule is that no screen may be told how much time passed.

use crate::input::{Event, Rect};

/// One fixed simulation tick, in milliseconds.
///
/// Duplicated from `crate::battlefield` deliberately rather than shared: this
/// module must not depend on the battle, and the number is the machine's, not
/// either module's. If they ever disagree the test below says so.
pub const TICK_MS: u32 = 16;

/// **The auto-repeat gate table at `0x004D2748`**, read out of `Lords2.exe`.
///
/// `Widget_Test`'s repeat is not a rate and not a formula. It is a **48-byte
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

/// The counter clamps here, and the clamped branch **skips the table** — so
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
/// them is the obvious mistake, which is why [`Kind::Held`] is a separate
/// branch of [`Press::tick`] rather than a parameter on the ramp.
pub const HELD_PULSE_MS: u32 = 320;

/// **The kind byte at `+0x0F` of the original's 24-byte input record, as a
/// type.**
///
/// This is the whole of what decides press, release, hold or repeat, and until
/// it existed here a screen answered a gesture by hand-rolling its own
/// press/release bookkeeping — which is how `docs/arms.json` came to mark
/// nineteen arms `reproduced` under a kind none of them actually had. A screen
/// now **declares** the kind, in a [`Widget`] table, and [`Press::event`]
/// decides when the handler runs.
///
/// The five are the original's own and there is no sixth: `Widget_Test`
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
    /// `WM_LBUTTONDBLCLK` *instead of* the second `WM_LBUTTONDOWN`. That is a
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
    /// It carries no memory of a press: the tester simply hit-tests the box and
    /// reads the released flag, so a release inside a kind-3 box fires it
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

/// **One record of a screen's input table** — a rectangle and the kind of
/// gesture it answers.
///
/// The original's record carries the geometry, the sprite frame, the handler
/// pointer, three counters and the kind, in 24 bytes. Ours carries the geometry
/// and the kind: the handler is the arm of the `match` the screen writes around
/// the index this returns, the counters live in [`Press`] because only one
/// widget can be down at a time, and the frame belongs to the painter.
///
/// **The hit box is a square in the original and a rectangle here.**
/// `Widget_Test` reads `+0x06` as the side of a square — it uses `table[3]` for
/// both axes, which is why every widget record's width equals its height — and
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
/// `step` is the counter at `+0x0E` **after** its increment, exactly as the
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

/// **A held button**, for one widget table.
///
/// Ours in shape and the original's in behaviour. `Widget_Test` keeps this
/// state *per record*, in the record; a screen module here keeps one of these
/// and names the widget it is holding, because our screens hit-test their own
/// rectangles rather than walking a table.
///
/// Drive it with the three things a screen already hears —
/// [`Press::press`] from `Event::Click`, [`Press::pointer`] from
/// `Event::Pointer`, [`Press::release`] from `Event::Release` — and call
/// [`Press::tick`] once per `Screen::update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    /// Which widget is down, if any. An index into whatever the screen calls
    /// its buttons.
    held: Option<usize>,
    /// **How the held widget repeats.** [`Kind::Repeat`] walks
    /// [`REPEAT_GATE`]; [`Kind::Held`] is a flat [`HELD_PULSE_MS`] pulse off a
    /// different clock in a different tester. Nothing else repeats, and a
    /// widget of another kind never becomes `held`.
    held_kind: Kind,
    /// `+0x0E`, the repeat counter, in 30 ms steps.
    step: u8,
    /// Milliseconds accumulated toward the next step. Reset rather than
    /// decremented, because `FUN_004B20ED` sets its timestamp to *now* on a
    /// step and leaves it alone otherwise.
    since_step: u32,
    /// `+0x0D`, the press timer, in frames. Non-zero means the pressed frame is
    /// showing.
    frames: u8,
    /// Kind 5 only: the widget whose handler runs when [`Press::frames`]
    /// reaches zero.
    pending: Option<usize>,
    /// Which widget the press timer belongs to.
    ///
    /// The original does not need this: the timer lives *in* the record, so
    /// "which button is drawn down" and "which button's timer is running" are
    /// the same question by construction. A screen that owns its rectangles has
    /// to say which one, and it is not the held one — a kind-4 button stays
    /// down for three frames **after** the release, which is the whole reason
    /// `rec[0x0D]` is set to 3 rather than to 1.
    showing: Option<usize>,
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
    /// rather than wraps so that "some clicks happened" can never round to
    /// "none".
    clicks: u8,
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
            frames: 0,
            pending: None,
            showing: None,
            clicks: 0,
        }
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
    /// all, which is why this is called from [`Press::press`] and
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
            // **`g_mouseLeftDoubleClick` is a press for kinds 2 and 4 only.**
            // `Widget_Test`'s kind-4 arm tests
            // `g_mouseLeftPressed || g_mouseLeftDoubleClick` and `Hotspot_Test`'s
            // kind-2 arm the same; kinds 1, 3 and 5 do not — well, kind 5 does,
            // and it is the one below. Kinds 1 and 3 read one flag each. Since
            // Windows sends the double click *instead of* the second press,
            // that is the difference between a spinner that steps twice on a
            // fast double click and a button that steps once.
            Event::DoubleClick { x, y } => {
                let i = table.iter().position(|w| w.rect.contains(x, y))?;
                match table[i].kind {
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
                    Kind::Press | Kind::Release => None,
                }
            }
            Event::Release { x, y } => {
                self.release();
                let i = table.iter().position(|w| w.rect.contains(x, y))?;
                (table[i].kind == Kind::Release).then_some(i)
            }
            // The original re-runs the hit test every frame, so a pointer that
            // has walked off the button simply stops matching and the record's
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
    pub fn press(&mut self, widget: usize) -> bool {
        // sfx: Widget_Test#1
        self.click();
        self.held = Some(widget);
        self.held_kind = Kind::Repeat;
        self.step = 0;
        self.since_step = 0;
        self.frames = PRESS_FRAMES;
        self.pending = None;
        self.showing = Some(widget);
        true
    }

    /// **A kind-2 press.** Fires immediately and then every
    /// [`HELD_PULSE_MS`] while the button stays down on it.
    ///
    /// **No pressed frame.** `Hotspot_Test` sets the record's `+0x0D` exactly as
    /// `Widget_Test` does, and nothing ever draws a hotspot record — the whole
    /// difference between the two testers is that one owns the visible buttons.
    /// So this leaves [`Press::pressed`] empty, which is what
    /// [`Kind::has_pressed_frame`] says out loud.
    pub fn press_held(&mut self, widget: usize) -> bool {
        self.held = Some(widget);
        self.held_kind = Kind::Held;
        self.step = 0;
        self.since_step = 0;
        self.frames = 0;
        self.pending = None;
        self.showing = None;
        true
    }

    /// **A kind-5 press.** Does *not* fire. Puts the pressed frame up and
    /// arms the handler for [`DELAYED_FRAMES`] ticks' time; [`Press::tick`]
    /// returns the widget on the tick it expires.
    pub fn press_delayed(&mut self, widget: usize) {
        // sfx: Widget_Test#2
        self.click();
        self.held = None;
        self.held_kind = Kind::Repeat;
        self.step = 0;
        self.since_step = 0;
        self.frames = DELAYED_FRAMES;
        self.pending = Some(widget);
        self.showing = Some(widget);
    }

    /// The pointer moved. `over` is the widget under it, or `None`.
    ///
    /// The original does not track this: it re-runs the hit test every frame,
    /// so a pointer that has walked off the button simply stops matching and
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

    /// One fixed tick. Returns the widget whose handler should run, if any.
    ///
    /// At most one fire per tick, which is what the original gets too: its
    /// counter advances at most once per frame.
    pub fn tick(&mut self) -> Option<usize> {
        // The countdown loop at the top of `Widget_Test`, which runs whether or
        // not anything is under the pointer.
        if self.frames > 0 {
            self.frames -= 1;
            if self.frames == 0 {
                self.showing = None;
                if let Some(w) = self.pending.take() {
                    return Some(w);
                }
            }
        }
        let held = self.held?;
        // **The flat pulse, which is a different tester on a different clock.**
        // `Hotspot_Test`'s kind-2 arm consults `DAT_0057D3C8` — one of
        // `Tick_Pulses`' eight dividers, every fourth 80 ms pulse — and there is
        // no table, no ramp and no pressed picture in it.
        if self.held_kind == Kind::Held {
            self.since_step += TICK_MS;
            if self.since_step < HELD_PULSE_MS {
                return None;
            }
            self.since_step = 0;
            return Some(held);
        }
        // `rec[0x0D] = 3` — the hold keeps the pressed frame up.
        self.frames = PRESS_FRAMES;
        self.showing = Some(held);
        self.since_step += TICK_MS;
        if self.since_step < REPEAT_STEP_MS {
            return None;
        }
        self.since_step = 0;
        self.step = self.step.saturating_add(1);
        let fires = fires_on_step(self.step);
        if self.step > REPEAT_CLAMP {
            self.step = REPEAT_CLAMP;
        }
        fires.then_some(held)
    }

    /// **Is the pressed frame showing?** A painter adds one to its sprite index
    /// when this is true, which is all `Widget_Draw` does.
    pub fn pressed(&self) -> Option<usize> {
        if self.frames == 0 {
            return None;
        }
        self.showing
    }

    /// Whether anything is waiting — a kind-5 press mid-delay. A screen that
    /// must not accept a second press while one is running asks this.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
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
        assert_eq!((0..200).filter_map(|_| p.tick()).count(), 0, "no repeat after the release");
    }

    /// The whole gesture, in ticks, with the numbers a player would feel.
    #[test]
    fn holding_a_button_repeats_slowly_then_quickly() {
        let mut p = Press::new();
        assert!(p.press(3));
        // 16 ms ticks against a 30 ms step, so a step every other tick.
        let mut fires = Vec::new();
        for t in 1..=120 {
            if p.tick() == Some(3) {
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
    /// original re-hit-tests every frame and simply stops matching.
    #[test]
    fn sliding_off_the_button_stops_it_repeating() {
        let mut p = Press::new();
        p.press(1);
        for _ in 0..40 {
            p.tick();
        }
        p.pointer(Some(2));
        assert_eq!((0..200).filter_map(|_| p.tick()).count(), 0);
    }

    /// **Kind 5: the press does not fire, and twenty ticks later it does.**
    /// This is the gauntlet.
    #[test]
    fn a_delayed_button_shows_the_pressed_frame_first_and_acts_afterwards() {
        let mut p = Press::new();
        p.press_delayed(0);
        assert_eq!(p.pressed(), Some(0), "the gauntlet goes down at once");
        assert!(p.busy());
        for t in 1..DELAYED_FRAMES as u32 {
            assert_eq!(p.tick(), None, "nothing has happened yet at tick {t}");
            assert_eq!(p.pressed(), Some(0), "and it is still down");
        }
        assert_eq!(p.tick(), Some(0), "the handler runs on the twentieth tick");
        assert_eq!(p.pressed(), None, "and the button comes back up");
        assert!(!p.busy());
    }

    /// A kind-4 press shows the pressed frame too — for three frames, not
    /// twenty — and it comes back up on its own after the release.
    #[test]
    fn a_repeating_button_shows_the_pressed_frame_for_three_frames() {
        let mut p = Press::new();
        p.press(7);
        assert_eq!(p.pressed(), Some(7));
        p.release();
        p.tick();
        p.tick();
        assert_eq!(p.pressed(), Some(7), "still down two frames after the release");
        p.tick();
        assert_eq!(p.pressed(), None, "and up on the third");
    }
}
