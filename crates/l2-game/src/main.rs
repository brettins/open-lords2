//! The rule here was `next_tick = Instant::now() + TICK` read *after* the wait,
//! which carried every overshoot forward and made a 16 ms tick 16.4 ms of wall
//! clock — two percent slow, compounding, and audible the moment a film's sound
//! track (played out by the device in real time) ran against a picture stepped
//! by tick count. `docs/decisions.md` C193.

mod app;
pub use app::*;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use l2_game::audio::Audio;
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
use winit::window::{Cursor, CursorIcon, CustomCursor, Window, WindowId};

const CANVAS_W: u32 = l2_view::canvas::WIDTH as u32;
const CANVAS_H: u32 = l2_view::canvas::HEIGHT as u32;


const DOUBLE_CLICK: Duration = Duration::from_millis(500);

const DOUBLE_CLICK_SLOP: i32 = 4;

struct App {
    game: Game,
    assets: Assets,
    machine: Machine,
    canvas: Canvas,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    started: Instant,
    ticker: l2_game::clock::Ticker,
    last_cursor: (i32, i32),
    audio: Audio,
    director: l2_game::audio::Director,
    /// `DAT_004DF3A8` — whether Control is held. The window procedure keeps the
    /// same latch and its digit arm dispatches on it: with Control, store a
    /// battle control group; without, recall one.
    ctrl: bool,
    last_press: Option<(Instant, (i32, i32))>,
    /// **`DAT_004EABC2`'s left bit**
    /// derives from it. In the library so a test can drive it without a window;
    /// see [`l2_game::input::LeftButton`].
    left: l2_game::input::LeftButton,
    pointer: l2_game::cursor::Pointer,
    /// **The original's own pointer pictures**, read out of the player's
    /// `Lords2.exe` the way `App_InitWindow` (`0x004B2258`) reads them —
    /// `RT_GROUP_CURSOR` 102, 103, 104, 105, 110, 111, 113. Empty when the
    /// executable is absent or unreadable, and then the system cursors below
    /// stand in.
    pictures: Vec<l2_formats::cursors::Picture>,
    cursors: Vec<(u16, CustomCursor)>,
    cursor_scale: u32,
}

impl App {
    fn present(&mut self) {
        if self.machine.take_dirty() {
            self.redraw();
        }
        let Some(pixels) = self.pixels.as_mut() else { return };
        self.machine.present(&self.assets, &self.canvas, pixels.frame_mut());
        if let Err(e) = pixels.render() {
            eprintln!("render failed: {e}");
        }
    }

    fn redraw(&mut self) {
        self.assets.quirks = self.game.presentation_quirks;
        self.sample_wall_clock();
        let App { game, assets, machine, canvas, window, .. } = self;
        let ctx = Ctx { game, assets };
        machine.draw(&ctx, canvas);
        let title = machine.title(&ctx);
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

    fn build_cursors(&mut self, event_loop: &ActiveEventLoop) {
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

    fn to_canvas(&self, x: f64, y: f64) -> (i32, i32) {
        match self.window.as_ref() {
            Some(w) => {
                let size = w.inner_size();
                window::to_canvas(size.width.max(1), size.height.max(1), x, y)
            }
            None => (x as i32, y as i32),
        }
    }

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

    fn tick(&mut self) {
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

    fn listen(&mut self) {
        self.director.listen(&mut self.audio, &self.machine, &self.game);
    }
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
        self.build_cursors(event_loop);
        self.redraw();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.machine.should_quit() {
            event_loop.exit();
            return;
        }
        let owed = self.ticker.due(self.started.elapsed().as_nanos() as u64);
        if let Some(next) = self.ticker.next_ns() {
            let deadline = self.started + Duration::from_nanos(next);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
        if owed == 0 {
            return;
        }

        for _ in 0..owed {
            self.tick();
        }

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
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                if event.logical_key == WinitKey::Named(NamedKey::F5) {
                    self.snap_to_whole_scale();
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
        if self.machine.should_quit() {
            event_loop.exit();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir: Option<PathBuf> = None;
    let mut mods: Option<PathBuf> = None;
    let mut sound = true;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mods" => mods = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--no-sound" => sound = false,
            "-h" | "--help" => usage(),
            other => dir = Some(PathBuf::from(other)),
        }
    }
    let Some(dir) = dir else { usage() };

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
    // The front end is up from the first frame and `Setup`'s own *Start* builds
    // the real game
    // until then. `docs/decisions.md` C117.
    let game = scenario::new_game(
        &assets,
        0,
        &l2_game::setup::SetupOptions::new().commit(1, l2_net::Quirks::default()),
        1,
        // Red, which is `FUN_004978AD`'s seed and what page 4 shows before
        // anybody clicks. This world is only what stands behind the title page.
        1,
        scenario::SEED,
        tables,
    )?;
    println!(
        "{} counties, {} owned by realm {}, {} {} - a NEW world; the front end starts the real one",
        game.kingdom.county_count,
        game.owned_by(game.player),
        game.player,
        l2_game::screens::map::season_name(game.kingdom.season),
        game.kingdom.year
    );

    let audio = if sound { Audio::open(&platform.vfs) } else { Audio::silent() };
    println!(
        "sound: {} ({} wav files found)",
        if audio.is_live() { "on" } else { "off" },
        audio.file_count()
    );

    let mut machine = Machine::new(ScreenId::Setup(SetupPage::Title));
    // **And over it, the intro** — `App_WinMain`'s `FUN_004B3571(0)`, which
    // chains to the Impressions logo and the credits before the title page is
    // seen. A missing film goes straight to the title page.
    l2_game::movie::start_up(&mut machine);

    let mut app = App {
        game,
        assets,
        audio,
        director: l2_game::audio::Director::new(),
        machine,
        canvas: Canvas::screen(),
        window: None,
        pixels: None,
        started: Instant::now(),
        ticker: l2_game::clock::Ticker::new(),
        last_cursor: (0, 0),
        ctrl: false,
        last_press: None,
        left: l2_game::input::LeftButton::new(),
        pointer: l2_game::cursor::Pointer::Arrow,
        // `App_InitWindow` (`0x004B2258`) loads the seven cursors from the
        // executable's own resources. Ours are read through the same overlay
        // every other asset comes through, and a missing or unreadable
        // `Lords2.exe` simply leaves the system cursors in place.
        pictures: platform
            .vfs
            .read(scenario::EXECUTABLE)
            .ok()
            .and_then(|b| l2_formats::cursors::read(&b).ok())
            .unwrap_or_default(),
        cursors: Vec::new(),
        cursor_scale: 0,
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    Ok(())
}
