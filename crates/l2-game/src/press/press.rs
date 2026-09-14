#![allow(unused_imports)]
use super::*;
use super::kind::*;
use crate::input::{Event, Rect};

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
    pub(crate) fn click(&mut self) {
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

