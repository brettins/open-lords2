//! `Hotspot_Test` (`0x0040E3EE`) kind **2** on the skirmish page: the down
//! edge, then the handler again every 320 ms while the button stays down on
//! the same record. `docs/input.md` §2 — the kind byte is `+0x0F` of the
//! 24-byte record and five of the twenty-two at `0x004DCF68` hold 2.
//!
//! The rectangles are not repeated here: [`SetupScreen::hotspots`] is the one
//! table, and [`held_kind`] says which of its arms the pulse belongs to.
#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use crate::input::{Event, Rect};
use crate::press::Widget;
use crate::screen::{Ctx, Transition};

impl SetupScreen {
    /// The five kind-2 records, in [`SetupScreen::hotspots`] order, which is
    /// the widget table's own.
    pub(crate) fn held_table(&self) -> Vec<(Widget, SkirmishArm)> {
        self.hotspots()
            .into_iter()
            .filter_map(|(r, a)| match a {
                Action::Skirmish(arm) => held_kind(arm).map(|k| (Widget::new(r, k), arm)),
                _ => None,
            })
            .collect()
    }

    /// `Hotspot_Test` re-hit-tests every frame, so a pointer that leaves the
    /// record stops the pulse and a release ends it:
    /// [`crate::press::Press::event`] is both.
    pub(crate) fn press_event(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let table = self.held_table();
        let widgets: Vec<Widget> = table.iter().map(|&(w, _)| w).collect();
        let fired = self.press.event(&widgets, event);
        debug_assert!(
            fired.is_none() || matches!(event, Event::Click { .. } | Event::DoubleClick { .. }),
            "no kind-2 record fires on a release",
        );
        // `Hotspot_Test`'s kind-2 arm calls the handler on
        // `g_mouseLeftPressed || g_mouseLeftDoubleClick`
        // (`00400000.c:7880-7888`), so the second press of a double click acts;
        // the first is [`SetupScreen::handle`]'s own `Event::Click` arm.
        match (event, fired) {
            (Event::DoubleClick { .. }, Some(i)) => match table.get(i) {
                Some(&(_, arm)) => self.act(Action::Skirmish(arm), ctx),
                None => Transition::Stay,
            },
            _ => Transition::Stay,
        }
    }

    /// One tick of the pulse. The table is rebuilt because
    /// `SkirmishArm::OpenFiles` raises `g_setupPage` to 13, whose hotspots are
    /// not these; an index past the end is that page change and ends the hold.
    pub(crate) fn tick_held(&mut self, ctx: &mut Ctx) -> Transition {
        let mut transition = Transition::Stay;
        for i in self.press.tick() {
            let arm = self.held_table().get(i).map(|&(_, a)| a);
            match arm {
                Some(arm) => transition = self.act(Action::Skirmish(arm), ctx),
                None => self.press.release(),
            }
        }
        transition
    }
}
