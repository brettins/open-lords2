#![allow(unused_imports)]
use super::*;
use super::screen_impl::*;
use super::*;
use super::helpers::*;
use super::tests::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

impl SaveLoadScreen {
    pub fn new(mode: Mode) -> SaveLoadScreen {
        let entries = saves::list();
        let mut screen = SaveLoadScreen {
            mode,
            entries,
            top: 0,
            selected: None,
            name: begin_name(""),
            status: Status::Idle,
            press: Press::new(),
            working: 0,
        };
        if mode == Mode::Load {
            screen.select(0);
        }
        screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn name(&self) -> String {
        self.name.text()
    }

    pub fn name_field(&self) -> &crate::text::TextField {
        &self.name
    }

    pub fn row_rect(i: usize) -> Rect {
        let (col, row) = (i % COLS, i / COLS);
        let x = LIST.0 + col as i32 * COL_W;
        let right = (INTERIOR.0 + INTERIOR.2).min(x + COL_W - 8);
        Rect::new(x, LIST.1 + row as i32 * ROW_H, right - x, ROW_H)
    }

    pub(crate) fn max_top(&self) -> usize {
        let over = self.entries.len().saturating_sub(PAGE);
        over.div_ceil(SCROLL_STEP) * SCROLL_STEP
    }

    pub(crate) fn scroll(&mut self, by: i32) {
        let max = self.max_top() as i32;
        self.top = (self.top as i32 + by).clamp(0, max) as usize;
    }

    pub(crate) fn at(&self, x: i32, y: i32) -> Option<usize> {
        (0..PAGE)
            .find(|&i| Self::row_rect(i).contains(x, y))
            .map(|i| self.top + i)
            .filter(|&i| i < self.entries.len())
    }

    pub(super) fn select(&mut self, i: usize) {
        let Some(entry) = self.entries.get(i) else { return };
        self.selected = Some(i);
        self.name = begin_name(&entry.name);
    }

    /// **`DAT_005CD41C = 100`**, which is the whole of the thumb up's handler
    /// and of Enter's: arm the latch, and let [`WORK_FRAMES`] run.
    ///
    /// the handler's: the tick that takes the latch up plays `S040_02.wav` on
    /// `g_screenId == '6'` — the save box — and `S040_01.wav` on anything else,
    /// which here is the load box. `[V]`, two `if`s and not an `if`/`else`.
    ///
    // sfx: FUN_004ad9f0#1,FUN_004ad9f0#2
    pub(super) fn begin(&mut self, ctx: &mut Ctx) {
        self.working = WORK_FRAMES;
        self.status = Status::Working;
        let line = match self.mode {
            Mode::Save => crate::audio::names::speech::SAVE_GAME,
            Mode::Load => crate::audio::names::speech::LOAD_GAME,
        };
        ctx.game.spoken = (ctx.game.spoken.0.wrapping_add(1), line);
    }

    pub fn working(&self) -> u8 {
        self.working
    }

    pub(crate) fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // `FUN_004342F3`.
            0 => {
                self.begin(ctx);
                Transition::Stay
            }
            // `SaveLoad_Cancel` (`0x00434308`): `g_screenId = g_screenIdSaved`.
            1 => Transition::Pop,
            2 => {
                self.scroll(-(SCROLL_STEP as i32));
                Transition::Stay
            }
            _ => {
                self.scroll(SCROLL_STEP as i32);
                Transition::Stay
            }
        }
    }

    /// The load or the save itself — what `SaveLoad_Tick` does when
    /// `DAT_0057D3C4` reaches zero. Everything that can go wrong comes back as
    /// a [`Status`] and the screen stays open; only success closes it.
    pub(super) fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        match self.mode {
            Mode::Load => {
                // **The path comes from the edit buffer, not from the
                // highlighted row.** `SaveLoad_Tick` (`0x004AD9F0`) is
                // `Str_Copy(0x4EA130, 0x4EAD60, 0xC); Path_AddExtension(…)` —
                // twelve bytes of the *typed* name — and clicking a row is what
                // puts a name into the buffer. So one road, and the mouse joins
                // it upstream. Ours read `selected` and ignored what was typed.
                let name = self.name.text().trim().to_string();
                let Some(i) = self.entries.iter().position(|e| e.name == name) else {
                    self.status = Status::Failed(if name.is_empty() {
                        "NO SAVED GAME IS SELECTED".into()
                    } else {
                        format!("{name:?} IS NOT A SAVED GAME")
                    });
                    return Transition::Stay;
                };
                let tables = ctx.game.kingdom.tables;
                match saves::read_path(&self.entries[i].path, tables) {
                    Ok(game) => {
                        let tips = ctx.game.tips;
                        *ctx.game = game;
                        ctx.game.tips = tips;
                        Transition::Pop
                    }
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
            Mode::Save => {
                // `Menu_SaveGame` (`0x00433F49`) is reachable from the
                // battlefield — `Screen_FrameInput`'s `0x29` arm opens with
                // `Menu_OpenDropdown` and neither the opener nor the handler
                // tests `g_battlePhase` — and the original's `.sav` is a memory
                // dump, so the battle goes into it. Ours is a versioned format
                // that does not encode `crate::battlefield::LiveBattle`, and
                // `crate::save::decode` puts `battle: None` back.
                //
                // arm: ours/save-refuses-mid-battle left-press
                if ctx.game.battle.is_some() {
                    self.status = Status::Failed(BATTLE_REFUSAL.into());
                    return Transition::Stay;
                }
                let name = self.name.text().trim().to_string();
                if !saves::is_valid_name(&name) {
                    self.status = Status::Failed(format!("{name:?} IS NOT A SAVE NAME"));
                    return Transition::Stay;
                }
                match saves::write(&name, ctx.game) {
                    Ok(_) => Transition::Pop,
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
        }
    }

    pub(super) fn edit(&mut self, event: Event, ctx: &Ctx) -> bool {
        // arm: 0x004BA9C8/saveload-name key
        let m = crate::text::FontMetrics::of(&ctx.assets.shell);
        if !self.name.event(event, &m) {
            return false;
        }
        self.status = Status::Idle;
        true
    }
}

