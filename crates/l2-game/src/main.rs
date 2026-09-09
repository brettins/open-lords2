//! Lords of the Realm II — the game.
//!
//! ```text
//! l2-game <game dir> [--mods <dir>]
//! ```
//!
//! **This is the only file in the workspace that names `winit` or `pixels`.**
//! Everything a screen does is done to a [`Canvas`] of palette indices and
//! driven by `l2_game::input::Event`, so the whole interface can be exercised
//! with no window at all — which is what `tests/` does.
//!
//! # Simulation time is not frame time
//!
//! `docs/plan.md` is explicit, and the battle viewer had to learn it the hard
//! way: pacing with `WaitUntil` decides **when to draw**, never what a tick
//! contains. Without a throttle the loop repaints as fast as the machine can
//! manage — about ten thousand frames a second — which is more than the surface
//! can present and makes wgpu reject the submission outright; the picture was a
//! blank white window until it was throttled.
//!
//! So: the clock is consulted in exactly one place, [`TICK`], and it decides
//! how often [`l2_game::Machine::update`] is called. `update` is not told how
//! much time passed and cannot ask, which is what keeps the simulation
//! reproducible (`docs/netcode.md`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use l2_game::game::Assets;
use l2_game::input::{window, Event as GameEvent, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::setup::SetupPage;
use l2_game::{scenario, Game};
use l2_mods::Platform;
use l2_view::Canvas;

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Window, WindowId};

const CANVAS_W: u32 = l2_view::canvas::WIDTH as u32;
const CANVAS_H: u32 = l2_view::canvas::HEIGHT as u32;

/// One fixed simulation tick. The only clock in the application.
const TICK: Duration = Duration::from_millis(16);

/// How long after a left press a second one is a **double** click.
///
/// The original never measures this: Windows does, against the user's own
/// `GetDoubleClickTime()`, and hands the game `WM_LBUTTONDBLCLK` instead of the
/// second `WM_LBUTTONDOWN`. `winit` has no such event, so this file measures it
/// — and it is this file's business alone, because it is a *clock*, and
/// `docs/netcode.md` allows one only above [`l2_game::input`]. 500 ms is the
/// Windows default that the original was therefore compiled against.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// And how far apart the two presses may be. Windows uses
/// `SM_CXDOUBLECLK` / `SM_CYDOUBLECLK`, four *window* pixels by default; ours is
/// in canvas pixels because that is the only coordinate a screen ever sees, and
/// four of them is generous at any whole scale.
const DOUBLE_CLICK_SLOP: i32 = 4;

struct App {
    game: Game,
    assets: Assets,
    machine: Machine,
    canvas: Canvas,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    next_tick: Instant,
    /// Where the pointer was last reported. A click carries no position of its
    /// own in `winit`, and asking the window again would be a second source of
    /// truth that could disagree with what the screen was last told.
    last_cursor: (i32, i32),
    /// When and where the left button last went down, for [`DOUBLE_CLICK`].
    /// `None` once a double click has been reported, so three clicks are a
    /// double and then a single rather than two doubles — which is what
    /// Windows itself does.
    last_press: Option<(Instant, (i32, i32))>,
}

impl App {
    fn present(&mut self) {
        let Some(pixels) = self.pixels.as_mut() else { return };
        // Which palette. Most screens run under the campaign's; the front end,
        // the merchant, the armoury, castle building and the ratings each read
        // a `.256` of their own, and a canvas of indices means nothing without
        // knowing which one. The top screen names it.
        let palette = self
            .machine
            .palette_name()
            .and_then(|n| self.assets.shell.palette(n))
            .unwrap_or(&self.assets.palette);
        self.canvas.to_rgba(palette, pixels.frame_mut());
        if let Err(e) = pixels.render() {
            eprintln!("render failed: {e}");
        }
    }

    fn redraw(&mut self) {
        let App { game, assets, machine, canvas, window, .. } = self;
        let ctx = Ctx { game, assets };
        machine.draw(&ctx, canvas);
        let title = machine.title(&ctx);
        if let Some(w) = window {
            w.set_title(&title);
        }
    }

    /// Window coordinates to canvas pixels.
    ///
    /// The only floating-point arithmetic in the application, and it stops
    /// here: what a screen receives is an integer pixel. A float that reached
    /// the simulation would be a float that differed between machines.
    ///
    /// The arithmetic itself is [`l2_game::input::window::to_canvas`], which is
    /// a pure function of the window's size and is tested there against the
    /// window size the scrolling bug was reported from. It lives in the library
    /// rather than here because **a transform that can only be checked by
    /// opening a window is a transform that stops being checked**, and because
    /// this is the one piece of arithmetic every click in the game passes
    /// through.
    ///
    /// `pixels` has an inverse of its own, `window_pos_to_pixel`. We do not use
    /// it: it answers `Err` for a position outside the picture, and *that
    /// position is exactly the one that matters* — the cursor pushed into the
    /// letterbox border, which the original would have read as the edge of the
    /// screen.
    fn to_canvas(&self, x: f64, y: f64) -> (i32, i32) {
        match self.window.as_ref() {
            Some(w) => {
                let size = w.inner_size();
                window::to_canvas(size.width.max(1), size.height.max(1), x, y)
            }
            None => (x as i32, y as i32),
        }
    }

    /// **F5 — resize the window to an exact multiple of 640 × 480.**
    ///
    /// The key is the original's, not ours: with `g_optFullScreen` clear the
    /// game draws the caption *"(F5 key re-sizes window to 640x480)"*, so it
    /// shipped a key that snaps the window back to a whole scale. Ours snaps to
    /// the **largest whole scale that still fits the window the player has**,
    /// which is 1× when the window is small and 2× or 3× on a modern display;
    /// the original had only 1× to snap to. That widening is ours and this is
    /// where it is written down.
    ///
    /// Integer scaling stays. A 1996 sprite game at a fractional scale gets
    /// pixels of two different widths in the same row, which is visible on
    /// every diagonal in the tile art; borders are the honest cost of not
    /// doing that, and this key is how the player makes them go away.
    fn snap_to_whole_scale(&mut self) {
        let Some(w) = self.window.clone() else { return };
        let size = w.inner_size();
        let (ww, wh) = window::snapped(size.width.max(1), size.height.max(1));
        let want = winit::dpi::PhysicalSize::new(ww, wh);
        if want == size {
            return;
        }
        let _ = w.request_inner_size(want);
    }

    fn deliver(&mut self, event: GameEvent) {
        let App { game, assets, machine, .. } = self;
        let mut ctx = Ctx { game, assets };
        machine.handle(event, &mut ctx);
    }

    /// One fixed simulation tick.
    fn tick(&mut self) {
        let App { game, assets, machine, .. } = self;
        let mut ctx = Ctx { game, assets };
        machine.update(&mut ctx);
    }
}

fn translate(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Space) => Key::Space,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        WinitKey::Character(s) => Key::letter(s.chars().next()?),
        _ => return None,
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Lords of the Realm II")
            .with_inner_size(winit::dpi::LogicalSize::new(CANVAS_W as f64, CANVAS_H as f64));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        let surface =
            SurfaceTexture::new(size.width.max(1), size.height.max(1), Arc::clone(&window));
        let pixels = Pixels::new(CANVAS_W, CANVAS_H, surface).expect("create pixels");
        self.window = Some(window);
        self.pixels = Some(pixels);
        self.redraw();
    }

    /// The fixed tick. Nothing below this line learns how long it waited.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.machine.should_quit() {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if now < self.next_tick {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
            return;
        }
        self.next_tick = now + TICK;
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));

        self.tick();

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
                self.machine.mark_dirty();
            }
            WindowEvent::RedrawRequested => self.present(),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                // F5 never reaches a screen: it is about the window, and the
                // window is this file's business alone.
                if event.logical_key == WinitKey::Named(NamedKey::F5) {
                    self.snap_to_whole_scale();
                } else if let Some(key) = translate(&event.logical_key) {
                    self.deliver(GameEvent::KeyDown(key));
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
                // `WM_LBUTTONDBLCLK` *replaces* the second `WM_LBUTTONDOWN`, so
                // this is one event or the other and never both.
                let now = Instant::now();
                let doubled = self.last_press.is_some_and(|(t, (px, py))| {
                    now.duration_since(t) <= DOUBLE_CLICK
                        && (x - px).abs() <= DOUBLE_CLICK_SLOP
                        && (y - py).abs() <= DOUBLE_CLICK_SLOP
                });
                if doubled {
                    self.last_press = None;
                    self.deliver(GameEvent::DoubleClick { x, y });
                } else {
                    self.last_press = Some((now, (x, y)));
                    self.deliver(GameEvent::Click { x, y });
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_cursor;
                self.deliver(GameEvent::Release { x, y });
            }
            // The original acts on the right button's **release** everywhere,
            // never its press — see `input::Event::RightClick`.
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
        if self.machine.should_quit() {
            event_loop.exit();
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: l2-game <game dir> [--mods <dir>]");
    eprintln!();
    eprintln!("  <game dir>   a Lords of the Realm II install: Lords2.exe, L2_maps.dat,");
    eprintln!("               lastturn.sav and the tile sets. Never written to.");
    std::process::exit(2)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir: Option<PathBuf> = None;
    let mut mods: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mods" => mods = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "-h" | "--help" => usage(),
            other => dir = Some(PathBuf::from(other)),
        }
    }
    let Some(dir) = dir else { usage() };

    // The overlay, which is also how the core ruleset is loaded. `docs/plan.md`
    // revision 3 puts mods behind the game, with one exception: loading the
    // ruleset at start-up is framework work, and this is it.
    let mut builder = Platform::builder().base(&dir);
    if let Some(m) = &mods {
        let found = l2_mods::discover(m)?;
        let ids: Vec<String> = found.iter().map(|f| f.id.clone()).collect();
        builder = builder.mods_dir(m).enable(ids);
    }
    let platform = builder.build()?;
    for m in &platform.load_order {
        println!("mod: {} {}", m.id, m.version);
    }
    let tables = platform.kingdom_tables()?;

    let assets = Assets::load(&platform.vfs)?;
    let game = scenario::load(&platform.vfs, tables)?;
    println!(
        "{} counties, {} owned by realm {}, {} {}",
        game.kingdom.county_count,
        game.owned_by(game.player),
        game.player,
        l2_game::screens::map::season_name(game.kingdom.season),
        game.kingdom.year
    );

    let mut app = App {
        game,
        assets,
        // The front end, as the original has it: `g_screenId` 0x1F, page 1.
        // `screens::menu` is the two-item placeholder it replaces; it is still
        // there, and `tests/machine.rs` still drives it, but the application
        // no longer starts on it.
        machine: Machine::new(ScreenId::Setup(SetupPage::Title)),
        canvas: Canvas::screen(),
        window: None,
        pixels: None,
        next_tick: Instant::now(),
        last_cursor: (0, 0),
        last_press: None,
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    Ok(())
}
