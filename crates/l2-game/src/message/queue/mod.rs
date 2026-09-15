#![allow(unused_imports)]

mod actions;
pub use actions::*;

use super::*;
use super::layout::*;
use super::tests::*;
use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};
use crate::game::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    Open,
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

    pub fn post(&mut self, letter: Letter, local_player: u8) -> bool {
        self.enqueue(Record::from(letter), local_player)
    }

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

    pub fn clamp_tip_timer(&mut self) {
        if self.open.is_some_and(|r| r.category == category::TIP) && self.timer > 999 {
            self.timer = TIP_TIMER;
        }
    }

    pub fn open(&self) -> Option<&Record> {
        self.open.as_ref()
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn timer(&self) -> i32 {
        self.timer
    }

    pub fn just_opened(&self) -> bool {
        self.timer == TIMER_START && self.open.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.open.is_none() && self.ring.iter().all(Record::is_empty)
    }

    pub fn queued(&self) -> usize {
        self.ring.iter().filter(|r| !r.is_empty()).count()
    }

    pub fn waiting(&self) -> Vec<Record> {
        (0..RING)
            .map(|i| self.ring[(self.tail + i) % RING])
            .filter(|r| !r.is_empty())
            .collect()
    }

    pub fn restore(&mut self, record: Record) {
        self.ring[self.head] = record;
        self.head += 1;
        if self.head > 0x31 {
            self.head = 0;
        }
        self.pending = true;
    }

    pub fn reopen(&mut self, record: Record, timer: i32) {
        self.open = Some(record);
        self.timer = timer;
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismissal {
    Closed,
    GameOver(Outcome),
    Nothing,
}

