#![allow(unused_imports)]
use super::*;
use super::turns::*;
use super::frame::*;
use super::drawing::*;
use super::super::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

impl Machine {
    pub fn new(root: ScreenId) -> Machine {
        Machine {
            stack: vec![root.build()],
            quit: false,
            dirty: true,
            clicks: 0,
            pointer: (0, 0),
            pointer_changed: false,
            tooltips: crate::tooltip::Tooltips::new(),
            tooltip_screens: Vec::new(),
            tool_tips_seen: None,
            autosave: false,
            tip_seat: None,
            clock_seen: None,
        }
    }

    pub fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    pub fn tooltips(&self) -> &crate::tooltip::Tooltips {
        &self.tooltips
    }

    pub fn clicks(&self) -> u32 {
        self.clicks
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub fn ids(&self) -> Vec<ScreenId> {
        self.stack.iter().map(|s| s.id()).collect()
    }

    pub fn top_id(&self) -> Option<ScreenId> {
        self.stack.last().map(|s| s.id())
    }

    pub fn top_screen_id(&self) -> Option<ScreenId> {
        self.stack.iter().rev().map(|s| s.id()).find(|id| *id != ScreenId::Message)
    }

    pub fn top_screen_byte(&self, game: &Game) -> Option<u8> {
        let s = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message)?;
        s.mode_screen_id().or_else(|| crate::tip::screen_byte(s.id(), game))
    }

    /// `g_screenId` (`0x004EAC50`) for the trace: the tip table, then `turn_clock::screen_byte`
    /// where the tip table has no row. C246.
    pub fn traced_screen_byte(&self, game: &Game) -> Option<u8> {
        let s = self.stack.iter().rev().find(|s| s.id() != ScreenId::Message)?;
        s.mode_screen_id()
            .or_else(|| crate::tip::screen_byte(s.id(), game))
            .or_else(|| crate::turn_clock::screen_byte(s.id()))
    }

    /// **The pointer this frame** — `Battle_Frame`'s two-way choice
    /// (`0x004B99C0`): the battlefield ids run the hover ladder, every other
    /// screen is one lookup in `g_cursorByScreen` (`0x004E3098`). See
    /// [`crate::cursor`].
    pub fn pointer(&self, game: &Game) -> crate::cursor::Pointer {
        let byte = self.top_screen_byte(game).unwrap_or(0);
        if (0x28..0x2B).contains(&byte) {
            return match game.battle.as_ref() {
                Some(b) => b.cursor().into(),
                None => crate::cursor::Pointer::Arrow,
            };
        }
        crate::cursor::by_screen(byte)
    }

    pub fn take_dirty(&mut self) -> bool {
        core::mem::replace(&mut self.dirty, false)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn push(&mut self, id: ScreenId) {
        self.stack.push(id.build());
        self.dirty = true;
    }

    pub fn handle(&mut self, event: Event, ctx: &mut Ctx) {
        // Chosen because the original answers it with nothing at all: the
        // window procedure (`0x004B29BE`) has `WM_KEYDOWN` arms for Backspace,
        // Enter, Control, Escape, End, Home, the arrows, Insert, Delete, the
        // digits, two keypad keys and F1 … F9, F11 and F12 — no letter — and the
        // `WM_CHAR` it sends for Ctrl+D is `0x04`, which `Edit_TypeChar`
        // (`0x00401A20`) rejects. F5 is `main.rs`'s window snap and is not
        // delivered here. Taken before the stack so that a text field cannot
        // swallow it and a screen that reads [`Key::CtrlChar`] — only the
        // battlefield does, and only for the nine digits — never meets it.
        //
        // arm: ours/debug-overlay-toggle key
        if event == Event::KeyDown(crate::input::Key::CtrlChar('D')) {
            ctx.game.prefs.debug_overlay = !ctx.game.prefs.debug_overlay;
            self.dirty = true;
            return;
        }
        // [V] App_WndProc 0x004B29BE `case 0x1b: if (g_appPhase == 3) Menu_Quit();`
        // — ungated by g_screenId, so in a game Escape raises the exit box over
        // any screen and closes none. Menu_Quit 0x004343F8 = Ui_OpenConfirm(0).
        // g_appPhase == 3 is a game in play, a skirmish battlefield too (Battle_Start callers,
        // 00430000.c:7890); Ui_OpenConfirm 00400000.c:8019 has no guard for a box already up.
        // arm: 0x004B29BE/escape-quits-outside-the-game key
        let leaving = self.ids().contains(&ScreenId::Leave);
        if event == Event::KeyDown(crate::input::Key::Escape)
            && !leaving
            && self.ids().iter().any(|id| matches!(id, ScreenId::Campaign | ScreenId::Battlefield))
        {
            // C257: FUN_0043441C's network arm leaves the game; ours asks first.
            let ask = if ctx.game.net.lockstep {
                ScreenId::Leave
            } else {
                ScreenId::Confirm(crate::screens::confirm::Ask::Quit)
            };
            self.push(ask);
            self.trace_screen(ctx.game);
            return;
        }
        if let Event::KeyDown(k) = event {
            // g_screenId: the court (0x09) is in turn_clock::screen_byte only.
            let screen = self.traced_screen_byte(ctx.game);
            crate::debug_keys::key_down(ctx.game, k, screen);
        }
        // `FUN_004B191E`'s `g_mouseInputChanged`: the position moved, or either
        // button went down or up. A key does not set it.
        match event {
            Event::Pointer { x, y } => {
                if (x, y) != self.pointer {
                    self.pointer = (x, y);
                    self.pointer_changed = true;
                }
            }
            Event::Click { x, y }
            | Event::Release { x, y }
            | Event::DoubleClick { x, y }
            | Event::RightClick { x, y }
            | Event::RightPress { x, y } => {
                self.pointer = (x, y);
                self.pointer_changed = true;
            }
            Event::KeyDown(_) | Event::Text(_) | Event::PointerLeft => {}
        }
        for depth in (0..self.stack.len()).rev() {
            let t = self.stack[depth].handle(event, ctx);
            let clicked = self.stack[depth].take_clicks();
            self.clicks = self.clicks.wrapping_add(clicked as u32);
            self.autosave |= self.stack[depth].take_autosave();
            if t == Transition::Pass {
                continue;
            }
            self.apply_at(depth, t);
            break;
        }
        self.trace_screen(ctx.game);
        self.dirty = true;
    }

}
