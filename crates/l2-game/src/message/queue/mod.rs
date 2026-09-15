#![allow(unused_imports)]

mod actions;
pub use actions::*;

use super::*;
use super::layout::*;
use super::tests::*;
use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};
use crate::game::Game;

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
/// so a tip that is enqueued on a screen the pump does not run on
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
    pub(super) fn close(&mut self) {
        self.open = None;
        self.timer = 0;
    }

    /// **`Msg_DismissUnlessQuestion`'s own test (`0x00476710`)**: every
    /// category but the four questions. The dismissal itself is
    /// [`crate::message::dismiss_unless_question`], which has to reach
    /// `Msg_Dismiss` and so cannot live on the ring.
    pub fn dismissed_by_map_click(&self) -> bool {
        self.open.is_some_and(|r| !r.is_question())
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

