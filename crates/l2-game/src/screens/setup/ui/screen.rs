#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::input::*;
use super::*;
use super::helpers::*;
use super::constants::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

impl Screen for SetupScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Setup(self.page)
    }

    fn title(&self, ctx: &Ctx) -> String {
        let name = ctx.assets.shell.text(GROUP, 0);
        if name.is_empty() {
            format!("Lords of the Realm II — setup page {}", self.page.number())
        } else {
            format!("{name} — setup page {}", self.page.number())
        }
    }

    fn palette(&self) -> Option<&'static str> {
        Some(self.page.palette())
    }

    fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.map_read {
            self.read_map(ctx);
        }
        if self.page == SetupPage::Shield {
            self.name.tick();
        }
        if self.page == SetupPage::Title {
            let minute = ctx.assets.wall_clock.map(crate::wallclock::minute);
            if minute != self.clock_minute {
                self.clock_minute = minute;
                self.clock_redraw = true;
            }
        }
        self.tick_held(ctx)
    }

    fn take_redraw(&mut self) -> bool {
        let pulse = self.press.take_redraw();
        core::mem::take(&mut self.clock_redraw) || pulse
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let n = self.count().max(1);
        // `Screen_HandleInput`'s page-4 arm is the one that sets `g_editActive`
        // (`0x005AEB78`), and that flag is what decides whether a keystroke
        // reaches the buffer at all — so on every other page of the front end
        // the keys below keep the meaning they have here today. On page 4 they
        // do not: `Space` is a space in a name, and `I` is the letter I.
        if self.page == SetupPage::Shield {
            // arm: 0x004BA9C8/setup-name key
            let metrics = text::FontMetrics::of(&ctx.assets.shell);
            if self.name.event(event, &metrics) {
                self.saved_name = self.name.commit(text::PLAYER_NAME_LEN);
                return Transition::Stay;
            }
        }
        // `Hotspot_Test` (`0x0040E3EE`) walks its table on every frame of
        // input: the press, the move off the record and the release are one
        // call. [`SetupScreen::press_event`] is that call, and the five kind-2
        // records are the only ones it holds.
        if matches!(
            event,
            Event::Click { .. }
                | Event::DoubleClick { .. }
                | Event::Release { .. }
                | Event::Pointer { .. }
                | Event::PointerLeft
        ) {
            self.press_event(event);
        }
        match event {
            // arm: ours/setup-key-up key
            Event::KeyDown(Key::Up) => self.selected = (self.selected + n - 1) % n,
            // arm: ours/setup-key-down key
            Event::KeyDown(Key::Down) => self.selected = (self.selected + 1) % n,
            // arm: ours/setup-key-activate key
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(ctx),
            // arm: ours/setup-key-index key
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            // arm: ours/setup-key-escape key
            Event::KeyDown(Key::Escape) => {
                return match self.page {
                    SetupPage::Title => Transition::Pop,
                    SetupPage::Dropdown => {
                        self.page = self.under;
                        Transition::Stay
                    }
                    _ => self.go(SetupPage::Title),
                };
            }
            Event::Pointer { x, y } => {
                if let Some((i, _)) = self.at(x, y) {
                    self.selected = i;
                }
            }
            Event::Click { x, y } => {
                if let Some((i, action)) = self.at(x, y) {
                    self.selected = i;
                    if self.page == SetupPage::Title && action == Action::Item(2) {
                        return Transition::Stay;
                    }
                    return self.act(action, ctx);
                }
            }
            // `Hotspot_Test` kind 3: `DAT_004DCB48` record 2, hotspot id 4.
            //
            // arm: 0x00432B05/lords-of-magic left-release
            Event::Release { x, y } if self.page == SetupPage::Title => {
                if let Some((i, action @ Action::Item(2))) = self.at(x, y) {
                    self.selected = i;
                    return self.act(action, ctx);
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        // **[D]** The front end's own text flags. `DAT_005AEA40` is set around
        // every menu item, button caption and body line and cleared for the
        // heading, so on these pages *only the heading is embossed*; and
        // `DAT_0058FE2C` is set around the heading alone, which draws its
        // capitals in colour 1. So there are two pens here, not one: `head`
        // for the heading and `pen` — flat, no drop capitals — for everything
        // else. A reimplementation that embossed the lot would be wrong on
        // every page of the front end at once.
        let head = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW_GATEWAY),
            caps: Some(1),
        };
        let pen = head.flat();
        let base = if self.page == SetupPage::Dropdown { self.under } else { self.page };
        if !shell::background(canvas, a, base.background()) {
            canvas.clear(ctx.assets.ink.background);
            let line = format!(
                "SETUP PAGE {} - NO {}",
                base.number(),
                base.background().to_uppercase()
            );
            l2_view::text::draw(canvas, 4, 4, &line, ctx.assets.ink.dim);
        }
        self.paint(ctx, canvas, &pen, &head, base);
        if self.page == SetupPage::Dropdown {
            self.paint_dropdown(canvas, &pen, ctx);
        }
    }
}

