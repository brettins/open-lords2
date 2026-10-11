#![allow(unused_imports)]
use super::*;
use super::events::*;
use super::app::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use l2_game::audio::Audio;
use l2_game::game::Assets;
use l2_game::input::{window, Event as GameEvent};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::setup::SetupPage;
use l2_game::{scenario, Game};
use l2_view::Canvas;
use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Cursor, CursorIcon, CustomCursor, Window, WindowId};

impl App {
    pub(super) fn present(&mut self) {
        if self.machine.take_dirty() {
            self.redraw();
        }
        let Some(pixels) = self.pixels.as_mut() else { return };
        self.machine.present(&self.assets, &self.canvas, pixels.frame_mut());
        if let Err(e) = pixels.render() {
            eprintln!("render failed: {e}");
        }
    }

    pub(crate) fn redraw(&mut self) {
        self.assets.quirks = self.game.presentation_quirks;
        self.sample_wall_clock();
        let App { game, assets, machine, canvas, window, .. } = self;
        let ctx = Ctx { game, assets };
        machine.draw(&ctx, canvas);
        let title = machine.title(&ctx);
        l2_game::crash::note_screen(&format!("{:?}", machine.top_id()));
        if let Some(w) = window {
            w.set_title(&title);
        }
        self.apply_pointer();
    }

    /// **`Cursor_Set` (`0x004B1CF3`)**, the binary's only caller of
    /// `SetCursor`, driven from where its only caller `Battle_Frame`
    /// (`0x004B99C0`) drives it: the frame. The choice is
    /// [`l2_game::screen::Machine::pointer`] and is entirely in the library;
    /// what is here is the one thing that cannot be, the window call.
    ///
    /// **The pictures are the original's** — the seven `RT_GROUP_CURSOR`
    /// resources `App_InitWindow` (`0x004B2258`) loads from the player's own
    /// `Lords2.exe`, at [`App::cursor_scale`], which is the scale the canvas
    /// itself is drawn at. A system cursor keeps the size the desktop gives it
    /// however far the 640 × 480 picture is blown up, so against a 3× canvas
    /// the pointer is a third of the size the original drew — the town
    /// square's question mark, resource 110, being where a player noticed it.
    fn apply_pointer(&mut self) {
        let want = self.machine.pointer(&self.game);
        if want == self.pointer {
            return;
        }
        self.pointer = want;
        let Some(w) = &self.window else { return };
        use l2_game::cursor::Pointer;
        if let Some((_, c)) = self.cursors.iter().find(|(id, _)| *id == want.resource()) {
            w.set_cursor(Cursor::Custom(c.clone()));
            return;
        }
        w.set_cursor(match want {
            Pointer::Arrow | Pointer::ArrowAlt => CursorIcon::Default,
            Pointer::Question => CursorIcon::Help,
            Pointer::Cross | Pointer::CrossTarget => CursorIcon::Crosshair,
            Pointer::Ring => CursorIcon::Pointer,
            Pointer::Peasant => CursorIcon::Grabbing,
            Pointer::Scythe => CursorIcon::Move,
        });
    }

    pub(crate) fn build_cursors(&mut self, event_loop: &ActiveEventLoop) {
        let Some(w) = &self.window else { return };
        let size = w.inner_size();
        let scale = window::scale(size.width.max(1), size.height.max(1));
        if self.pictures.is_empty() || (scale == self.cursor_scale && !self.cursors.is_empty()) {
            return;
        }
        self.cursor_scale = scale;
        self.cursors = self
            .pictures
            .iter()
            .filter_map(|p| {
                let p = p.scaled(scale);
                let src =
                    CustomCursor::from_rgba(p.rgba, p.width, p.height, p.hot_x, p.hot_y).ok()?;
                Some((p.id, event_loop.create_custom_cursor(src)))
            })
            .collect();
        let want = self.pointer.resource();
        if let (Some(w), Some((_, c))) =
            (&self.window, self.cursors.iter().find(|(id, _)| *id == want))
        {
            w.set_cursor(Cursor::Custom(c.clone()));
        }
    }

    /// `docs/netcode.md` D-5 — *no wall clock, no scheduler* — is why it cannot
    /// live any lower: `l2-game` the library has no `SystemTime` anywhere
    /// simulation path physically has nothing to read. The same discipline
    /// [`l2_game::clock::Ticker`] holds for the monotonic clock
    /// (`docs/decisions.md` C193), for the same reason.
    fn sample_wall_clock(&mut self) {
        self.assets.wall_clock = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64);
    }

    pub(super) fn to_canvas(&self, x: f64, y: f64) -> (i32, i32) {
        match self.window.as_ref() {
            Some(w) => {
                let size = w.inner_size();
                window::to_canvas(size.width.max(1), size.height.max(1), x, y)
            }
            None => (x as i32, y as i32),
        }
    }

    pub(super) fn snap_to_whole_scale(&mut self) {
        let Some(w) = self.window.clone() else { return };
        let size = w.inner_size();
        let (ww, wh) = window::snapped(size.width.max(1), size.height.max(1));
        let want = winit::dpi::PhysicalSize::new(ww, wh);
        if want == size {
            return;
        }
        let _ = w.request_inner_size(want);
    }

    pub(crate) fn deliver(&mut self, event: GameEvent) {
        let App { game, assets, machine, .. } = self;
        let mut ctx = Ctx { game, assets };
        machine.handle(event, &mut ctx);
    }

    pub(super) fn tick(&mut self) {
        self.sample_wall_clock();
        let App { game, assets, machine, .. } = self;
        let mut ctx = Ctx { game, assets };
        machine.update(&mut ctx);
        self.autosave();
        self.listen();
    }

    /// **`Save_RotateAndWrite` (`0x0049A453`)**, which is
    /// [`l2_game::saves::run_pending`] and nothing else.
    fn autosave(&mut self) {
        if let Some(Err(e)) = l2_game::saves::run_pending(&mut self.machine, &self.game) {
            eprintln!("autosave: {e}");
        }
    }

    /// Outside any tick: `docs/netcode.md` D-5. On Start the lobby's world replaces ours.
    pub(super) fn pump_net(&mut self) {
        let tables = self.game.kingdom.tables;
        let game = &mut self.game;
        // The lobby page shows the roster and the router's answer as they come (C253).
        let shown = |l: &l2_game::net::Link| {
            (l.phase.clone(), l.roster.clone(), l.me, l.hosting, l.reach.clone(), l.port.clone())
        };
        let before = shown(&game.net);
        let started = l2_game::net::service(&mut self.net, game, &self.assets, tables);
        if shown(&game.net) != before {
            self.machine.mark_dirty();
        }
        if let Some(world) = started {
            self.game = world;
            self.machine.mark_dirty();
            // `[I]` the replay's seed; the world carries its own from `Start`.
            let seed = self.game.turns_played as u64;
            let transport = self.net.take().and_then(|d| d.into_transport());
            self.live = transport.and_then(|t| l2_game::net::Live::new(t, &mut self.game, seed));
            let home = l2_game::net::replay::home();
            if let (Some(live), Some(dir)) = (self.live.as_mut(), home) {
                live.record_to(dir);
            }
        }
    }

    fn listen(&mut self) {
        self.director.listen(&mut self.audio, &self.machine, &self.game);
    }
}

