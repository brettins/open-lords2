//! The Modern scheme's keys (C272, `docs/battle-controls.md` §3, §4, §6). `[I]` ours: the original
//! has one scheme, `Screen_FrameInput`'s `0x29` arms, which Classic still runs untouched. Selection is
//! a list of battle-unit ids on the seat: presentation, never sent, `figure.selected` never written.
//! Every order is one canonical [`BattleOp::Command`].

#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::commands::*;
use super::field::*;
use super::groups::*;
use super::super::*;
use super::super::view::*;
use tests::*;
use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::game::BattleControls;
use crate::input::{Key, Rect};

/// Shift + digit on a US layout: what the window reports for `1` … `9`. `[I]`; rebinding is P5.
const SHIFTED_DIGITS: &str = "!@#$%^&*(";

impl LiveBattle {
    /// `docs/battle-controls.md` §6. `shift` is the key as the window reported it.
    pub fn m_key(&mut self, key: Key, shift: bool) -> bool {
        let prev = self.last_group.take();
        let slot = |c: char| group_slot(c as u8);
        let acted = match key {
            Key::Char(c @ '1'..='9') => self.recall_modern(slot(c), prev),
            Key::Char(c) if shift && SHIFTED_DIGITS.contains(c) => {
                let slot = SHIFTED_DIGITS.find(c);
                let add = self.live_sel();
                slot.is_some_and(|s| {
                    let group = &mut self.mgroups[s];
                    for u in add {
                        if !group.contains(&u) {
                            group.push(u);
                        }
                    }
                    true
                })
            }
            Key::CtrlChar(c @ '1'..='9') => slot(c).is_some_and(|s| {
                self.mgroups[s] = self.live_sel();
                true
            }),
            Key::Char('A') => {
                self.armed = !self.live_sel().is_empty();
                self.armed
            }
            Key::Char('S') => self.issue(Verb::Stop, (0, 0), None, false),
            Key::Char('H') => self.issue(Verb::Hold, (0, 0), None, false),
            Key::Char('L') => self.issue(Verb::Line, (0, 0), None, false),
            Key::Char('C') | Key::Char('V') => self.issue(Verb::Column, (0, 0), None, false),
            Key::Char('M') => self.issue(Verb::Merge, (0, 0), None, false),
            Key::Char('G') => self.press_charge(),
            Key::Space => self.press_pause(),
            Key::F2 => self.select_all(),
            Key::Char('+') | Key::Char('=') => self.change_speed(true),
            Key::Char('-') | Key::Char('_') => self.change_speed(false),
            Key::Left | Key::Right | Key::Up | Key::Down => self.pan(key),
            _ => false,
        };
        self.redraw |= acted;
        acted
    }

    /// `+` / `-`: ½×, 1×, 2×, single player only (a shared world has one speed). `[I]` ours, C272.
    fn change_speed(&mut self, faster: bool) -> bool {
        if self.deferred {
            return false;
        }
        self.speed = (self.speed + if faster { 1 } else { -1 }).clamp(-1, 1);
        true
    }

    /// Sim steps this frame: Classic and a shared world always one.
    pub fn ticks_this_frame(&mut self) -> u32 {
        if !self.is_modern() || self.deferred {
            return 1;
        }
        self.speed_beat = !self.speed_beat;
        match self.speed {
            1 => 2,
            -1 => u32::from(self.speed_beat),
            _ => 1,
        }
    }

    /// A group key: select; the same key again with nothing between centres the camera.
    fn recall_modern(&mut self, slot: Option<usize>, prev: Option<usize>) -> bool {
        let Some(slot) = slot else { return false };
        let live = |u: &u8| self.runner.units.get(*u as usize).owner == self.owner;
        let group: Vec<u8> = self.mgroups[slot].iter().copied().filter(live).collect();
        if group.is_empty() {
            return false;
        }
        self.sel = group;
        self.last_group = Some(slot);
        if prev == Some(slot) {
            self.look_at_selection();
        }
        true
    }

    /// Arrows pan one cell a press (the key repeat does the rest).
    fn pan(&mut self, key: Key) -> bool {
        let (dx, dy) = match key {
            Key::Left => (-1, 0),
            Key::Right => (1, 0),
            Key::Up => (0, -1),
            _ => (0, 1),
        };
        self.cam = (self.cam.0 + dx, self.cam.1 + dy);
        self.clamp_cam();
        true
    }
}
