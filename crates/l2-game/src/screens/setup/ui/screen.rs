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
        self.read_prefs(ctx);
        let t = if self.page == SetupPage::Load {
            self.load_update(ctx)
        } else {
            self.update_page(ctx)
        };
        self.keep_prefs(ctx);
        t
    }

    fn take_clicks(&mut self) -> u8 {
        self.load.as_mut().map_or(0, |l| l.take_clicks())
    }

    fn take_redraw(&mut self) -> bool {
        let boxed = self.load.as_mut().is_some_and(|l| l.take_redraw());
        let pulse = self.press.take_redraw() | boxed;
        core::mem::take(&mut self.clock_redraw) || pulse
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        self.read_prefs(ctx);
        // arm: 0x004BA9C8/setup-load-name key
        let t = if self.page == SetupPage::Load {
            self.load_event(event, ctx)
        } else {
            self.handle_page(event, ctx)
        };
        self.keep_prefs(ctx);
        t
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.read_prefs(ctx);
        self.draw_page(ctx, canvas);
    }
}

impl SetupScreen {
    fn update_page(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.map_read {
            self.read_map(ctx);
        }
        if self.page == SetupPage::Shield {
            self.name.tick();
        }
        // `Skirmish_Setup` (`0x0042B7F7`) runs when page 12 opens, and the
        // `.skr` scan `FUN_0042D389` (`00420000.c:5204-5205`) with it.
        if matches!(self.page, SetupPage::Skirmish | SetupPage::SkirmishMulti) {
            if self.troops_file.is_none() {
                self.skirmish_files = ctx.assets.shell.skirmish_files.clone();
                self.load_troops(ctx);
            }
            self.tick_banner();
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

    fn handle_page(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
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
            match self.press_event(event, ctx) {
                Transition::Stay => {}
                t => return t,
            }
        }
        // Screen_FrameInput page 0xD, `00420000.c:7473-7482`: right release
        // anywhere, or Ui_OkButtonClicked (0x0040E7E4, left release in 24x24). [V]
        // Screen_FrameInput 0x0042FF10. [D] :7474 returns to DAT_00553E5C, the opener
        // (FUN_0043DDF4 00430000.c:8245); its other opener is the multiplayer page 0xC, redesigned.
        if self.page == SetupPage::SkirmishFile {
            let (ox, oy) = SKIRMISH_FILE_OK;
            match event {
                Event::RightClick { .. } => return self.go(SetupPage::Skirmish),
                Event::Release { x, y } if Rect::new(ox, oy, 24, 24).contains(x, y) => {
                    return self.go(SetupPage::Skirmish)
                }
                _ => {}
            }
        }
        match event {
            // arm: ours/setup-key-up key
            Event::KeyDown(Key::Up) => self.selected = (self.selected + n - 1) % n,
            // arm: ours/setup-key-down key
            Event::KeyDown(Key::Down) => self.selected = (self.selected + 1) % n,
            // arm: ours/setup-key-activate key
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(ctx),
            // [D] developer page, not the original's: C242. App_WndProc 0x004B29BE binds no Ctrl key [I].
            // arm: ours/setup-key-index key
            Event::KeyDown(Key::CtrlChar('D')) => return Transition::Push(ScreenId::Index),
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

    fn draw_page(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
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
        // `FUN_0041E61D` (`00410000.c:5119-5126`): pages 11 and up take
        // `skirmish.256` either way, and `DAT_0056899C` picks between
        // `skirmish.pl8` and `skircust.pl8`. The palette does not change.
        let background = base.background_custom(self.skirmish.custom);
        if !shell::background(canvas, a, background) {
            canvas.clear(ctx.assets.ink.background);
            let line =
                format!("SETUP PAGE {} - NO {}", base.number(), background.to_uppercase());
            l2_view::text::draw(canvas, 4, 4, &line, ctx.assets.ink.dim);
        }
        self.paint(ctx, canvas, &pen, &head, base);
        if base == SetupPage::Load {
            self.draw_load(ctx, canvas);
        }
        if self.page == SetupPage::Dropdown {
            self.paint_dropdown(canvas, &pen, ctx);
        }
    }
}

