#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! The rule here was `next_tick = Instant::now() + TICK` read *after* the wait,
//! which carried every overshoot forward and made a 16 ms tick 16.4 ms of wall
//! clock — two percent slow, compounding, and audible the moment a film's sound
//! track (played out by the device in real time) ran against a picture stepped
//! by tick count. `docs/decisions.md` C193.

mod app_frame;
mod events;

mod app;
mod launch;
mod os;
use app::*;

use std::sync::Arc;
use std::time::{Duration, Instant};

use l2_game::audio::Audio;
use l2_game::game::Assets;
use l2_game::screen::{Machine, ScreenId};
use l2_game::screens::setup::SetupPage;
use l2_game::{scenario, Game};
use l2_view::Canvas;

use pixels::Pixels;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{CustomCursor, Window};

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
    alt: bool,
    /// What `prefs.ini` last held; `persist_prefs` writes on a difference.
    saved_prefs: l2_game::game::Prefs,
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
    /// The lobby's sockets; `l2_game::net::service` pumps them between ticks (D-5).
    net: Option<l2_game::net::Driver>,
    /// The lockstep session after `Start`, pumped before each tick (D-5).
    live: Option<l2_game::net::Live>,
}

fn main() {
    launch::panic_hook();
    if let Err(e) = run() {
        launch::fatal(&e.to_string());
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let launch::Args { dir, mods, sound, verify, net } = launch::args();
    let prefs = l2_game::prefs::load();
    let dir = launch::install_dir(dir, &prefs);
    if let Some(path) = verify {
        // [D] headless: a replay panic goes to stderr, not the crash box, C240.
        drop(std::panic::take_hook());
        app::verify_replay(dir, mods, path);
    }
    let platform = app::platform(&dir, mods.as_deref())?;
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
    // `App_WinMain`'s `Options_Load()` (`00400000.c:8221`), after `L2.eng` and
    // before `App_InitWindow`: `Options_Load` `0x004AE1B8`. `l2_game::prefs`.
    let mut game = game;
    game.prefs = prefs;
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
    match &net {
        Some(flags) => {
            launch::open_lobby(&mut game, flags);
            machine.push(ScreenId::Lobby);
        }
        None => l2_game::movie::start_up(&mut machine),
    }

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
        alt: false,
        saved_prefs: prefs,
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
        net: None,
        live: None,
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    // `FUN_0049767D`'s `Options_Save()` (`00490000.c:3084`), reached only from
    // `0x0040EBFE` after the message loop ends.
    if let Err(e) = l2_game::prefs::save(&app.game.prefs) {
        eprintln!("preferences: {e}");
    }
    Ok(())
}

