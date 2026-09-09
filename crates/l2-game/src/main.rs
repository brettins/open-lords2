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
use l2_game::input::{Event as GameEvent, Key};
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
    fn to_canvas(&self, x: f64, y: f64) -> (i32, i32) {
        match self.pixels.as_ref() {
            Some(p) => {
                let (px, py) = p
                    .window_pos_to_pixel((x as f32, y as f32))
                    .unwrap_or_else(|pos| p.clamp_pixel_pos(pos));
                (px as i32, py as i32)
            }
            None => (x as i32, y as i32),
        }
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
                if let Some(key) = translate(&event.logical_key) {
                    self.deliver(GameEvent::KeyDown(key));
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let (x, y) = self.to_canvas(position.x, position.y);
                self.last_cursor = (x, y);
                self.deliver(GameEvent::Pointer { x, y });
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_cursor;
                self.deliver(GameEvent::Click { x, y });
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_cursor;
                self.deliver(GameEvent::Release { x, y });
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
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    Ok(())
}
