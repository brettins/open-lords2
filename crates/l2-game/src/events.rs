#![allow(unused_imports)]
use super::*;
use super::app_frame::*;
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Lords of the Realm II")
            .with_inner_size(winit::dpi::LogicalSize::new(CANVAS_W as f64, CANVAS_H as f64))
            .with_fullscreen(self.fullscreen());
        // [D] ours: app_id / WM_CLASS = `saves::APP_DIR`, so a desktop entry and a
        // compositor rule can name the window. The original has one WndClass.
        #[cfg(target_os = "linux")]
        let attrs = winit::platform::wayland::WindowAttributesExtWayland::with_name(
            attrs,
            l2_game::saves::APP_DIR,
            l2_game::saves::APP_DIR,
        );
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        let surface =
            SurfaceTexture::new(size.width.max(1), size.height.max(1), Arc::clone(&window));
        let pixels = Pixels::new(CANVAS_W, CANVAS_H, surface).expect("create pixels");
        self.window = Some(window);
        self.pixels = Some(pixels);
        self.build_cursors(event_loop);
        self.redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.machine.should_quit() {
            event_loop.exit();
            return;
        }
        self.pump_net();
        let owed = self.ticker.due(self.started.elapsed().as_nanos() as u64);
        if let Some(next) = self.ticker.next_ns() {
            let deadline = self.started + Duration::from_nanos(next);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
        if owed == 0 {
            return;
        }

        for _ in 0..owed {
            if let Some(live) = &mut self.live {
                let before = (self.game.net.stalled, self.game.net.halted.is_some(), self.game.net.beat.live, self.game.net.notice.is_some());
                live.pump(&mut self.game);
                if before != (self.game.net.stalled, self.game.net.halted.is_some(), self.game.net.beat.live, self.game.net.notice.is_some()) {
                    self.machine.mark_dirty();
                }
            }
            self.tick();
            self.end_session();
        }
        self.persist_prefs();

        if self.machine.take_dirty() {
            self.redraw();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(p) = self.pixels.as_mut() {
                    if let Err(e) = p.resize_surface(size.width.max(1), size.height.max(1)) {
                        eprintln!("resize failed: {e}");
                    }
                }
                self.build_cursors(event_loop);
                self.machine.mark_dirty();
            }
            WindowEvent::RedrawRequested => self.present(),
            WindowEvent::ModifiersChanged(mods) => {
                self.ctrl = mods.state().control_key();
                self.alt = mods.state().alt_key();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if event.logical_key == WinitKey::Named(NamedKey::F5) {
                    // App_WndProc 0x004B29BE case 0x74: `if (g_optFullScreen == 0)`.
                    if !self.game.prefs.full_screen {
                        self.snap_to_whole_scale();
                    }
                } else if self.alt && event.logical_key == WinitKey::Named(NamedKey::Enter) {
                    self.toggle_full_screen();
                } else {
                    if let Some(key) = translate(&event.logical_key, self.ctrl) {
                        self.deliver(GameEvent::KeyDown(key));
                    }
                    if !self.ctrl {
                        for c in event.text.iter().flat_map(|t| t.chars()) {
                            self.deliver(GameEvent::Text(c));
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = self.to_canvas(position.x, position.y);
                self.last_cursor = (x, y);
                self.deliver(GameEvent::Pointer { x, y });
            }
            WindowEvent::CursorLeft { .. } => self.deliver(GameEvent::PointerLeft),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_cursor;
                let now = Instant::now();
                let doubled = self.last_press.is_some_and(|(t, (px, py))| {
                    now.duration_since(t) <= DOUBLE_CLICK
                        && (x - px).abs() <= DOUBLE_CLICK_SLOP
                        && (y - py).abs() <= DOUBLE_CLICK_SLOP
                });
                let e = if doubled {
                    self.last_press = None;
                    self.left.double_clicked(x, y)
                } else {
                    self.last_press = Some((now, (x, y)));
                    self.left.pressed(x, y)
                };
                self.deliver(e);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_cursor;
                if let Some(e) = self.left.released(x, y) {
                    self.deliver(e);
                }
            }
            // The right button's **down** edge — `g_mouseRightPressed`
            // (`0x004EABE0`), read five times in the image and by exactly one
            // arm we build: `Screen_FrameInput`'s minimap epilogue
            // (`0x0042FF10`). See `input::Event::RightPress`.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let (x, y) = self.last_cursor;
                self.deliver(GameEvent::RightPress { x, y });
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Right,
                ..
            } => {
                let (x, y) = self.last_cursor;
                self.deliver(GameEvent::RightClick { x, y });
            }
            _ => {}
        }
        self.persist_prefs();
        if self.machine.should_quit() {
            event_loop.exit();
        }
    }
}

