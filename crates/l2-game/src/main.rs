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
//! `docs/plan.md` is explicit
//! way: pacing with `WaitUntil` decides **when to draw**, never what a tick
//! contains. Without a throttle the loop repaints as fast as the machine can
//! manage — about ten thousand frames a second — which is more than the surface
//! can present and makes wgpu reject the submission outright; the picture was a
//! blank white window until it was throttled.
//!
//! So: the clock is consulted in exactly one place, [`App::about_to_wait`],
//! and it decides how often [`l2_game::Machine::update`] is called. `update` is
//! not told how much time passed and cannot ask, which is what keeps the
//! simulation reproducible (`docs/netcode.md`).
//!
//! **How many ticks are owed is [`l2_game::clock::Ticker`], not this file.**
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

// One fixed simulation tick is `l2_game::TICK_MS`, and **when** one falls due
// is `l2_game::clock::Ticker` — in the library, where a test can run it. This
// file supplies the reading it works from and nothing else.

/// How long after a left press a second one is a **double** click.
///
/// The original never measures this: Windows does, against the user's own
/// `GetDoubleClickTime()`
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
    /// What the monotonic readings handed to [`App::ticker`] are measured from.
    /// One `Instant`, taken once, so that a reading is a `u64` of nanoseconds
    /// and the scheduling arithmetic is integer and testable.
    started: Instant,
    ticker: l2_game::clock::Ticker,
    /// Where the pointer was last reported. A click carries no position of its
    /// own in `winit`, and asking the window again would be a second source of
    /// truth that could disagree with what the screen was last told.
    last_cursor: (i32, i32),
    /// **Sound, and it is deliberately only here.**
    ///
    /// `Audio` is not in [`Ctx`], so no screen can reach it, ask it anything,
    /// or wait on it. Everything audible is *derived* from the world once a
    /// tick by [`App::listen`], which means a sound cannot change what the
    /// simulation does in either value or timing — the property
    /// `docs/netcode.md`'s lockstep argument rests on, held by the type
    /// system
    audio: Audio,
    /// **What decides what is audible**
    /// notice that something has *become* true. It lives in the library so that
    /// a test runs this code —
    /// `crates/l2-game/tests/audio_wiring/main.rs`.
    director: l2_game::audio::Director,
    /// `DAT_004DF3A8` — whether Control is held. The window procedure keeps the
    /// same latch and its digit arm dispatches on it: with Control, store a
    /// battle control group; without, recall one.
    ctrl: bool,
    /// When and where the left button last went down, for [`DOUBLE_CLICK`].
    /// `None` once a double click has been reported, so three clicks are a
    /// double and then a single — which is what
    /// Windows itself does.
    last_press: Option<(Instant, (i32, i32))>,
    /// **`DAT_004EABC2`'s left bit**
    /// derives from it. In the library so a test can drive it without a window;
    /// see [`l2_game::input::LeftButton`].
    left: l2_game::input::LeftButton,
    /// The pointer the window is showing, so `set_cursor` is called on a change
    /// and not on every frame. `Battle_Frame` re-chooses every frame and hands
    /// `Cursor_Set` the answer regardless; `SetCursor` on an unchanged
    /// `HCURSOR` is free and `winit`'s is not.
    pointer: l2_game::cursor::Pointer,
    /// **The original's own pointer pictures**, read out of the player's
    /// `Lords2.exe` the way `App_InitWindow` (`0x004B2258`) reads them —
    /// `RT_GROUP_CURSOR` 102, 103, 104, 105, 110, 111, 113. Empty when the
    /// executable is absent or unreadable, and then the system cursors below
    /// stand in.
    pictures: Vec<l2_formats::cursors::Picture>,
    /// [`App::pictures`] built for the window's current whole scale, by
    /// resource id, and the scale they were built at.
    cursors: Vec<(u16, CustomCursor)>,
    cursor_scale: u32,
}

impl App {
    fn present(&mut self) {
        // **The canvas and the palette must be the same frame.**
        // `Machine::present` reads the palette off the *live* stack — it has to,
        // because `Screen::fade` and a film's `live_palette` change with no
        // redraw at all — and `RedrawRequested` arrives after
        // `request_redraw`
        // while the canvas still holds the old one. Presenting then is one
        // frame of the defect `tests/overlay_palette.rs` records, in reverse:
        // the old page's indices under the new page's colours. Redrawing
        // whatever went dirty first is the whole guard.
        if self.machine.take_dirty() {
            self.redraw();
        }
        let Some(pixels) = self.pixels.as_mut() else { return };
        // Which palette
        // campaign's; the front end, the merchant, the armoury, castle building,
        // the battlefield and the ratings each read a `.256` of their own
        // window drawn over one of those runs under it; a film runs under its
        // own, which changes as it plays. `Machine::present` is
        // the whole decision, in the library, where a test can see its colours.
        self.machine.present(&self.assets, &self.canvas, pixels.frame_mut());
        if let Err(e) = pixels.render() {
            eprintln!("render failed: {e}");
        }
    }

    fn redraw(&mut self) {
        // **The one place the presentation quirks cross from the session into
        // the assets.** `Ctx` hands a screen `&Assets`, so the quirks page
        // cannot write them where the drawing code reads them; the authority
        // is `Game::presentation_quirks` and this is its projection. Done here
        //
        // a click, a key, a future command replay - is on screen the next
        // frame without every writer having to remember.
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
    ///
    /// With no executable to read, the fall-back is the nearest system cursor
    /// to a kind, which is a stand-in and not a reproduction.
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

    /// Build every picture at the window's current whole scale. Called on the
    /// first window and again whenever the scale changes, because a
    /// `CustomCursor` is a fixed bitmap and the canvas's scale is not.
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
        // The pointer on screen is still one of the old bitmaps, and
        // `apply_pointer` only acts on a change of kind: re-set it here.
        let want = self.pointer.resource();
        if let (Some(w), Some((_, c))) =
            (&self.window, self.cursors.iter().find(|(id, _)| *id == want))
        {
            w.set_cursor(Cursor::Custom(c.clone()));
        }
    }

    /// **The only wall clock in the program, and it is in the shell.**
    ///
    /// `l2_game::wallclock` draws the title screen's MST clock — ours, not the
    /// original's; see that module — and it is arithmetic on a number, with no
    /// `SystemTime` of its own. This is where the number comes from, projected
    /// into [`Assets`] the way the presentation quirks are projected in
    /// [`Self::redraw`], because `Ctx` hands a screen `&Assets` and that is the
    /// only channel into a painter.
    ///
    /// `docs/netcode.md` D-5 — *no wall clock, no scheduler* — is why it cannot
    /// live any lower: `l2-game` the library has no `SystemTime` anywhere
    /// simulation path physically has nothing to read. The same discipline
    /// [`l2_game::clock::Ticker`] holds for the monotonic clock
    /// (`docs/decisions.md` C193), for the same reason.
    ///
    /// A clock before 1970 — or one the machine cannot read — leaves the field
    /// `None` and the screen simply draws no clock.
    fn sample_wall_clock(&mut self) {
        self.assets.wall_clock = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64);
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
    ///
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
        // Sampled here as well as in [`Self::redraw`], and it has to be: the
        // redraw only happens when something is already dirty, so a sample
        // taken there alone would stop the clock at the minute the page opened. The tick is what
        // notices the minute turning; nothing under the shell may notice it.
        self.sample_wall_clock();
        let App { game, assets, machine, .. } = self;
        let mut ctx = Ctx { game, assets };
        machine.update(&mut ctx);
        self.autosave();
        self.listen();
    }

    /// **`Save_RotateAndWrite` (`0x0049A453`)**, which is
    /// [`l2_game::saves::run_pending`] and nothing else.
    ///
    /// The body is in the library for [`Self::listen`]'s reason, stated below
    /// it. What is here is the only thing that cannot be: a failed write is
    /// **said out loud and does not stop the game**. A full disk at the turn
    /// boundary must not end the session, and a silent failure would leave the
    /// player believing there is a turn to go back to.
    fn autosave(&mut self) {
        if let Some(Err(e)) = l2_game::saves::run_pending(&mut self.machine, &self.game) {
            eprintln!("autosave: {e}");
        }
    }

    /// **Everything audible**, which is [`l2_game::audio::Director::listen`]
    /// and nothing else.
    ///
    /// The body used to be here. Being in a binary meant no test could call it,
    /// so the only test available was one that re-typed the same lines beside
    /// its own assertions — a test that passes with this file deleted. See the
    /// `Director` doc comment for the whole of that argument.
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

    /// The fixed tick. Nothing below this line learns how long it waited.
    ///
    /// **The deadline comes from the deadline before it**, not from the moment
    /// the wait returned: that is [`l2_game::clock::Ticker`]'s whole job, and
    /// the reason a tick is 16 ms of wall clock
    /// the timer overshot. A wake that came back late runs the ticks it owes,
    /// up to `clock::MAX_CATCH_UP`, so that time the machine spent elsewhere is
    /// repaid to the simulation instead of being lost from it.
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
                // A whole scale is what both the canvas and the pointer are
                // drawn at, so a resize can change the cursor bitmaps.
                self.build_cursors(event_loop);
                self.machine.mark_dirty();
            }
            WindowEvent::RedrawRequested => self.present(),
            WindowEvent::ModifiersChanged(mods) => {
                // `WM_KEYDOWN` / `WM_KEYUP` on `VK_CONTROL` in the original.
                self.ctrl = mods.state().control_key();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                // F5 never reaches a screen: it is about the window
                // window is this file's business alone.
                if event.logical_key == WinitKey::Named(NamedKey::F5) {
                    self.snap_to_whole_scale();
                } else {
                    if let Some(key) = translate(&event.logical_key, self.ctrl) {
                        self.deliver(GameEvent::KeyDown(key));
                    }
                    // **And `WM_CHAR` after `WM_KEYDOWN`, as Windows sends
                    // them.** The original's window procedure handles the two
                    // messages in separate arms: virtual keys drive the
                    // hotkeys, characters drive `Edit_TypeChar`. `winit` gives
                    // us the character in `text`, already shifted and already
                    // through the layout, which is what `WM_CHAR` carries.
                    //
                    // Control-held keys produce no `WM_CHAR` worth having —
                    // `Ctrl+A` is `0x01` — and the original's control arm is a
                    // `WM_KEYDOWN` one, so they are suppressed here.
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
                // `WM_LBUTTONDBLCLK` *replaces* the second `WM_LBUTTONDOWN`, so
                // this is one event or the other and never both.
                let now = Instant::now();
                let doubled = self.last_press.is_some_and(|(t, (px, py))| {
                    now.duration_since(t) <= DOUBLE_CLICK
                        && (x - px).abs() <= DOUBLE_CLICK_SLOP
                        && (y - py).abs() <= DOUBLE_CLICK_SLOP
                });
                // `App_WndProc`'s three arms, in [`l2_game::input::LeftButton`]:
                // `0x201` sets the down bit and `0x203` does not, which is what
                // decides whether the matching `0x202` is an edge.
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
                // **`None` after a double click.** `WM_LBUTTONUP` clears a bit
                // `WM_LBUTTONDBLCLK` never set, so the frame poll sees no change
                // and raises no `g_mouseLeftReleased`. We delivered one anyway
                // until this branch existed.
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
            // Everything else the right button does is on its **release** —
            // see `input::Event::RightClick`.
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
    // **A fresh world, not the install's autosave.**
    //
    // This used to be `scenario::load`, which reads `lastturn.sav` out of the
    // game directory — the *original program's* rolling autosave. So what our
    // engine started on was whatever the person last played in Lords of the
    // Realm II, and for a while that made the campaign look correct: a player
    // reported starting on the right map in the right season, and he was seeing
    // his own saved game from another program. His `Autumn 1269` was the proof,
    // because no fresh start can be in 1269 — `Game_NewGame`'s single
    // `Season_Advance` lands in Winter 1268.
    //
    // **The feature was correct only because of what was in a file our code did
    // not own.** `docs/environment.md` has warned about that file for weeks and
    // nine *tests* were fixed by naming fixtures explicitly; the application was
    // never looked at. When a class of bug is fixed across the tests, ask
    // whether the product has the same bug.
    //
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

    // Sound is opened through the same vfs every other asset comes through, so
    // the install is found once and a mod layer can replace a `.wav` for free.
    // `Audio::open` cannot fail: no device, no files, or a device that refuses
    // a stream all end at the same silent object
    // it did before sound existed.
    let audio = if sound { Audio::open(&platform.vfs) } else { Audio::silent() };
    println!(
        "sound: {} ({} wav files found)",
        if audio.is_live() { "on" } else { "off" },
        audio.file_count()
    );

    // The front end, as the original has it: `g_screenId` 0x1F, page 1.
    // `screens::menu` is the two-item placeholder it replaces; it is still
    // there, and `tests/machine.rs` still drives it, but the application no
    // longer starts on it.
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
