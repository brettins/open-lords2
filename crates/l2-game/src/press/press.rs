#![allow(unused_imports)]
use super::*;
use super::kind::*;
use crate::input::{Event, Rect};

/// **One press timer per record, not one per table.** `+0x0D` is a byte of
/// each 24-byte record, and the countdown loop at the top of `Widget_Test`
/// decrements every record's timer on every call, so two gauntlets pressed a
/// few frames apart are both down and **both** act, each twenty frames after
/// its own press. This struct held one timer and one pending widget until
/// that was measured; the second press overwrote the first, and the first
/// button did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    held: Option<usize>,
    held_kind: Kind,
    /// `+0x0E`, the repeat counter, in 30 ms steps.
    step: u8,
/// Milliseconds accumulated toward the next step. Reset
    /// decremented, because `FUN_004B20ED` sets its timestamp to *now* on a
    /// step and leaves it alone otherwise.
    since_step: u32,
    /// **`+0x0D` of every record**, the press timer, in frames. Non-zero means
    /// that record's pressed frame is showing.
    timers: [u8; MAX_WIDGETS],
    delayed: u32,
    /// `Widget_Test` (`0x0040DA1E`) calls `Sound_RestartSlot(1)` — `click3.wav`
    /// — from inside the hit test, at two sites: the kind-4 arm and the kind-5
    /// arm. This module *is* that hit test, so the count belongs here and
    /// nowhere else; what it must not do is reach the audio layer, because
    /// `docs/netcode.md` D-3 makes [`crate::audio::Audio`] unreachable from a
    /// screen on purpose. So this is an **outbox**: write-only from here,
    /// drained upward by [`crate::screen::Machine::handle`], and nothing that
    /// happens to it can be read back by the thing that filled it.
    clicks: u8,
    redraw: bool,
    /// `g_mouseLeftDown`, bit 0 of `DAT_004EABC2`: `App_WndProc`
    /// (`0x004B29BE`) sets it on `0x201` and clears it on `0x202`, and `0x203`
    /// sets bit 0 of `DAT_004EADA1` instead (`004b0000.c:2115-2124`).
    down: bool,
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
            down: false,
        }
    }

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

    fn slot(widget: usize) -> usize {
        assert!(
            widget < MAX_WIDGETS,
            "widget {widget} is past press::MAX_WIDGETS ({MAX_WIDGETS}); raise it rather than \
             letting a table time the wrong button",
        );
        widget
    }

    pub fn take_clicks(&mut self) -> u8 {
        core::mem::take(&mut self.clicks)
    }

    pub(crate) fn click(&mut self) {
        self.clicks = self.clicks.saturating_add(1);
    }

    pub fn event(&mut self, table: &[Widget], event: Event) -> Option<usize> {
        match event {
            Event::Click { x, y } => {
                // `App_WndProc` (`0x004B29BE`) answers `0x201` with
                // `DAT_004EABC2 |= 1` before any hit test
                // (`004b0000.c:2115-2117`), and `Hotspot_Test`'s kind-2 arm
                // (`00400000.c:7877-7879`) asks `g_mouseLeftDown` alone — so a
                // press begun on the background and dragged onto a record
                // holds it. Set here and not only in `press`/`press_held`,
                // which the `?` below skips. `[V]`
                self.down = true;
                let i = table.iter().position(|w| w.rect.contains(x, y))?;
                match table[i].kind {
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
            Event::Pointer { x, y } => {
                let over = table.iter().position(|w| w.rect.contains(x, y));
                self.pointer(over.map(|i| (i, table[i].kind)));
                None
            }
            Event::PointerLeft => {
                self.pointer(None);
                None
            }
            _ => None,
        }
    }

    /// **It touches no other record.** A kind-5 button pressed a moment
    /// earlier keeps counting down and still acts: `Widget_Test`'s kind-4 arm
    /// writes `+0x0C`, `+0x0D` and `+0x0E` of its own record and nothing else.
    pub fn press(&mut self, widget: usize) -> bool {
        let w = Press::slot(widget);
        // sfx: Widget_Test#1
        self.click();
        self.down = true;
        self.held = Some(w);
        self.held_kind = crate::arm!("0x0040DA1E/widget-auto-repeat", Repeat);
        self.step = 0;
        self.since_step = 0;
        self.timers[w] = PRESS_FRAMES;
        self.delayed &= !(1 << w);
        true
    }

/// **No pressed frame.** `Hotspot_Test` sets the record's `+0x0D`.
    pub fn press_held(&mut self, widget: usize) -> bool {
        let w = Press::slot(widget);
        self.down = true;
        self.held = Some(w);
        self.held_kind = Kind::Held;
        self.step = 0;
        self.since_step = 0;
        true
    }

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
        self.down = true;
        self.held = None;
        self.step = 0;
        self.since_step = 0;
        self.timers[w] = DELAYED_FRAMES;
        self.delayed |= 1 << w;
    }

    /// **The press has no memory of where it began.** `Hotspot_Test`'s kind-2
    /// arm (`00400000.c:7878-7891`) and `Widget_Test`'s kind-4 arm
    /// (`00400000.c:7513-7526`) are both reached only by the loop that
    /// hit-tests the record under the pointer, and both then ask
    /// `g_mouseLeftDown` alone: with the button down, leaving a record suspends
    /// it, re-entering resumes it, and the pointer dragged onto another record
    /// of the same kind holds **that** one.
    pub fn pointer(&mut self, over: Option<(usize, Kind)>) {
        if !self.down {
            return;
        }
        match over {
            Some((i, kind)) if matches!(kind, Kind::Held | Kind::Repeat) => {
                if self.held == Some(i) {
                    return;
                }
                self.held = Some(i);
                self.held_kind = kind;
                // `Widget_Test`'s countdown loop zeroes `+0x0E` of every record
                // whose `+0x0D` is `0` (`00400000.c:7486-7488`), so a kind-4
                // record entered with the button down ramps from step 0. The
                // kind-2 divider `DAT_0058FEB0` is zeroed at the press (`7882`)
                // and nowhere else, so kind 2 keeps its count.
                if kind == Kind::Repeat {
                    self.step = 0;
                    self.since_step = 0;
                }
            }
            _ => {
                self.held = None;
                self.step = 0;
            }
        }
    }

    pub fn release(&mut self) {
        self.down = false;
        self.held = None;
        self.step = 0;
    }

    pub fn tick(&mut self) -> Fired {
        let mut fired = Fired::default();
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

    fn hold(&mut self, mut fired: Fired) -> Fired {
        // `Hotspot_Test`'s kind-2 arm consults `DAT_0057D3C8` — one of
        // `Tick_Pulses`' (`0x004BBC80`) eight dividers, every fourth 80 ms pulse
        // (`004b0000.c:7763`) — and there is no table, no ramp and no pressed
        // picture in it. The divider is a global counted in `Tick_Pulses`, not
        // in the record, so it runs on while the pointer is off the record and
        // only `self.held` decides whether the pulse reaches a handler.
        if self.held_kind == Kind::Held {
            if !self.down {
                return fired;
            }
            self.since_step += TICK_MS;
            if self.since_step >= HELD_PULSE_MS {
                self.since_step = 0;
                fired.held = self.held;
            }
            return fired;
        }
        let Some(held) = self.held else { return fired };
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

    pub fn is_pressed(&self, widget: usize) -> bool {
        self.timers.get(widget).is_some_and(|&t| t != 0)
    }

    pub fn any_pressed(&self) -> bool {
        self.timers.iter().any(|&t| t != 0)
    }

    pub fn busy(&self) -> bool {
        self.delayed != 0
    }
}

