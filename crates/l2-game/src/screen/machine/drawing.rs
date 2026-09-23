#![allow(unused_imports)]
use super::*;
use super::state::*;
use super::turns::*;
use super::frame::*;
use super::super::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

impl Machine {
    pub fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let from = self.base();
        for screen in &mut self.stack[from..] {
            screen.draw(ctx, canvas);
        }
        self.draw_turn_timer(ctx, canvas);
        // `FUN_00476E95` comes after `FUN_0041A639` in `Battle_Frame`.
        crate::tooltip::draw(ctx, &self.tooltips, canvas);
        crate::net::draw_status(ctx, canvas);
    }

    fn base(&self) -> usize {
        for i in (0..self.stack.len()).rev() {
            if !self.stack[i].is_overlay() {
                return i;
            }
        }
        0
    }

    /// The original has one display palette and only a *painter* writes it —
    /// `Screen_Armoury` ends with `Palette_Set(armoury.256)`,
    /// `Screen_DrawBattlefield` (`0x004233F7`) sets `T32_bat1.256` — and nothing
    /// that draws over a page touches it.
    ///
    /// `Tip_Show` (`0x00476DA9`) saves `g_screenId`, writes `0x27` and posts a
    /// message; `FUN_00476E21` puts the byte back; `Msg_DrawWindow`
    /// (`0x0047309E`) has no `Palette_Set` anywhere in its 10,915 bytes.
    ///
    /// window over the armoury is in the armoury's colours. `[V]`
    ///
    /// This used to ask the top screen alone, and an overlay that names no
    /// palette — the tip host, the message scroll, the menu bar, the options
    /// pages — handed the page beneath it the campaign palette. A player saw the
    /// raise-army and castle screens *"color reversed"* behind their first tip
    /// until he dismissed it. `docs/decisions.md` C178.
    pub fn palette_name(&self) -> Option<&'static str> {
        for screen in self.stack.iter().rev() {
            if let Some(name) = screen.palette() {
                return Some(name);
            }
            if !screen.is_overlay() {
                return None;
            }
        }
        None
    }

    pub fn present(&self, assets: &crate::game::Assets, canvas: &Canvas, rgba: &mut [u8]) {
        let live = self.live_palette();
        let palette = live.as_ref().unwrap_or_else(|| {
            self.palette_name()
                .and_then(|n| assets.shell.palette(n))
                .unwrap_or(&assets.palette)
        });
        // `FUN_004B0CB4` never touches the framebuffer — it rewrites the display
        // palette and lets the unchanged plane of indices resolve darker. See
        // `l2_view::fade` and [`Screen::fade`].
        let faded = self.fade().map(|phase| l2_view::fade::at(palette, phase));
        canvas.to_rgba(faded.as_ref().unwrap_or(palette), rgba);
    }

    pub fn live_palette(&self) -> Option<l2_formats::Palette> {
        self.stack.last().and_then(|s| s.live_palette())
    }

    pub fn fade(&self) -> Option<u8> {
        self.stack.last().and_then(|s| s.fade())
    }

    pub fn title(&self, ctx: &Ctx) -> String {
        self.stack.last().map(|s| s.title(ctx)).unwrap_or_default()
    }

    pub(super) fn apply(&mut self, t: Transition) {
        let depth = self.stack.len().saturating_sub(1);
        self.apply_at(depth, t);
    }

    pub(crate) fn apply_at(&mut self, depth: usize, t: Transition) {
        match t {
            Transition::Stay | Transition::Pass => {}
            Transition::Reveal => self.stack.truncate(depth + 1),
            Transition::Push(id) => {
                self.stack.truncate(depth + 1);
                self.stack.push(id.build());
            }
            Transition::Pop => {
                self.stack.truncate(depth);
                if self.stack.is_empty() {
                    self.quit = true;
                }
            }
            Transition::Replace(id) => {
                self.stack.truncate(depth);
                self.stack.push(id.build());
            }
            Transition::Quit => {
                self.stack.clear();
                self.quit = true;
            }
            Transition::Goto(id) => match self.stack.iter().position(|s| s.id() == id) {
                Some(at) => self.stack.truncate(at + 1),
                None => {
                    self.stack.clear();
                    self.stack.push(id.build());
                }
            },
            // `[D]`: the loader `FUN_004976A1` (`0x004976A1`) writes the screen
            // itself — `g_screenId = 0` (`00490000.c:3098`), then `0x29` inside
            // `FUN_00477C89` (`00470000.c:3194`) on `g_battlePhase == 2` — so
            // the original arrives with one screen and no stack under it, and we
            // have one. Truncating to `depth` is what keeps the campaign map
            // under a battlefield raised here.
            Transition::Resume(id) => match self.stack.iter().position(|s| s.id() == id) {
                Some(at) => self.stack.truncate(at + 1),
                None => {
                    self.stack.truncate(depth);
                    self.stack.push(id.build());
                }
            },
        }
    }
}


