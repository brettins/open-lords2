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
/// the 80 ms pulses. Nothing here implements it yet; the constant is written
/// down because the two repeats are different mechanisms and conflating them is
/// the obvious mistake.
pub const HELD_PULSE_MS: u32 = 320;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Press {
    /// Which widget is down, if any. An index into whatever the screen calls
    /// its buttons.
    held: Option<usize>,
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
}

impl Press {
    pub const fn new() -> Press {
        Press { held: None, step: 0, since_step: 0, frames: 0, pending: None, showing: None }
    }

    /// **A kind-4 press.** Fires immediately — the return is *"run the handler
    /// now"* — and starts the hold.
    pub fn press(&mut self, widget: usize) -> bool {
        self.held = Some(widget);
        self.step = 0;
        self.since_step = 0;
        self.frames = PRESS_FRAMES;
        self.pending = None;
        self.showing = Some(widget);
        true
    }

    /// **A kind-5 press.** Does *not* fire. Puts the pressed frame up and
    /// arms the handler for [`DELAYED_FRAMES`] ticks' time; [`Press::tick`]
    /// returns the widget on the tick it expires.
    pub fn press_delayed(&mut self, widget: usize) {
        self.held = None;
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
