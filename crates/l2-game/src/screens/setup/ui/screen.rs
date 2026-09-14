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

    /// `Game_NewGame`'s `Save_RotateAndWrite()`. Raised by
/// [`SetupScreen::new_game`] once a world has been built.
    fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    /// `Map_LoadPlanes`'s effect on the option block, once, on the first tick.
    ///
    /// The original loads the planes the moment the custom page is opened and
    /// again on every change of scenario, and `g_playerStartCount` falls out of
    /// that load. The constructor has no [`Ctx`] and so no `L2_maps.dat`, so
    /// the first read waits for the first tick — which is also what makes a
    /// screen built in a test with no install work: it never gets a slot, and
    /// the seat count stays at its five.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.map_read {
            self.read_map(ctx);
        }
        // `Edit_DrawCaret` counts its own frames; ours counts ticks, because
        // nothing under the renderer may read a clock. `crate::text`.
        if self.page == SetupPage::Shield {
            self.name.tick();
        }
        // **The clock on page 1 — ours, and this is the only thing that makes
        // it move.** The reading comes in on `Assets`; the minute it falls in
        // is compared with the minute already on the screen, and only a change
        // asks for a repaint. Nothing here reads a clock, and on any page but
        // the title
        if self.page == SetupPage::Title {
            let minute = ctx.assets.wall_clock.map(crate::wallclock::minute);
            if minute != self.clock_minute {
                self.clock_minute = minute;
                self.clock_redraw = true;
            }
        }
        Transition::Stay
    }

    /// The minute turning, and nothing else — see [`SetupScreen::clock_minute`].
    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.clock_redraw)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let n = self.count().max(1);
        // **The name field gets first refusal on page 4, and only there.**
        //
        // `Screen_HandleInput`'s page-4 arm is the one that sets `g_editActive`
        // (`0x005AEB78`), and that flag is what decides whether a keystroke
        // reaches the buffer at all — so on every other page of the front end
        // the keys below keep the meaning they have here today. On page 4 they
        // do not: `Space` is a space in a name, and `I` is the letter I.
        //
        // The commit is `Edit_Commit(&g_options, 0x1F)`, which the original
// runs **every frame** while the page is up.
        // Doing it per keystroke is the same thing at the only moments the
        // buffer can have changed.
        if self.page == SetupPage::Shield {
            // arm: 0x004BA9C8/setup-name key
            let metrics = text::FontMetrics::of(&ctx.assets.shell);
            if self.name.event(event, &metrics) {
                self.saved_name = self.name.commit(text::PLAYER_NAME_LEN);
                return Transition::Stay;
            }
        }
        // **Everything below this line is ours.** The front end has no keyboard
        // at all in the original: not one of `Screen_HandleInput`'s thirteen
        // `g_setupPage` arms tests a key, and the window procedure has no
        // `g_screenId == 0x1F` case. Its whole interface is `Hotspot_Test` and
// `Widget_Test`. That is recorded — a menu a person
        // cannot drive from the keyboard is worse, not more faithful — and the
        // records are `ours/setup-*` in `docs/arms.json`.
        match event {
            // arm: ours/setup-key-up key
            Event::KeyDown(Key::Up) => self.selected = (self.selected + n - 1) % n,
            // arm: ours/setup-key-down key
            Event::KeyDown(Key::Down) => self.selected = (self.selected + 1) % n,
            // arm: ours/setup-key-activate key
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(ctx),
            // Ours: the demo's index of every screen. `screens::index` says
            // why it exists and marks itself as not the game's.
            //
            // arm: ours/setup-key-index key
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            // arm: ours/setup-key-escape key
            Event::KeyDown(Key::Escape) => {
                // Whatever the page is, Escape is its own way back — the
                // original's Back button where there is one, and out of the
                // front end where there is not.
                return match self.page {
                    // Pop, not Quit. Popping the last screen quits anyway —
                    // `Machine::apply` — so this is the right answer both when
                    // the front end is the root and when it was opened from
                    // somewhere else, without the screen having to know which.
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
                    // The one kind-3 record on the title page fires on the
                    // release, below; its press only selects.
                    if self.page == SetupPage::Title && action == Action::Item(2) {
                        return Transition::Stay;
                    }
                    return self.act(action, ctx);
                }
            }
            // `Hotspot_Test` kind 3: `DAT_004DCB48` record 2, hotspot id 4.
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
        // Page 9 is drawn over whatever was underneath it, so the background
        // and the page beneath are painted first and only then the open list.
        let base = if self.page == SetupPage::Dropdown { self.under } else { self.page };
        if !shell::background(canvas, a, base.background()) {
            // No install, or a partial one. Say so in our own font — never in
            // the original's — so that an empty page can never be mistaken for
            // a page the game drew empty.
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

