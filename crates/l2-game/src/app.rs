#![allow(unused_imports)]
use super::*;

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
use winit::window::{CursorIcon, Window, WindowId};

/// `ctrl` is the window procedure's `DAT_004DF3A8` — `0x004B29BE` latches
/// `VK_CONTROL` on key-down and clears it on key-up, and its digit arm calls a
/// different function depending on it. Only the digits carry the modifier,
/// because only the digits are dispatched on it.
pub(super) fn translate(key: &WinitKey, ctrl: bool) -> Option<Key> {
    Some(match key {
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Space) => Key::Space,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        // The four the window procedure dispatches into the edit buffer and
        // nothing else dispatches at all: `VK_HOME`, `VK_END`, `VK_INSERT`,
        // `VK_DELETE`. See `l2_game::text`.
        WinitKey::Named(NamedKey::Home) => Key::Home,
        WinitKey::Named(NamedKey::End) => Key::End,
        WinitKey::Named(NamedKey::Insert) => Key::Insert,
        WinitKey::Named(NamedKey::Delete) => Key::Delete,
        WinitKey::Character(s) if ctrl => Key::ctrl_letter(s.chars().next()?),
        WinitKey::Character(s) => Key::letter(s.chars().next()?),
        _ => return None,
    })
}

fn usage() -> ! {
    eprintln!("usage: l2-game <game dir> [--mods <dir>] [--no-sound]");
    eprintln!();
    eprintln!("  <game dir>   a Lords of the Realm II install: Lords2.exe, L2_maps.dat,");
    eprintln!("               the fonts, the artwork and the tile sets. Never written to.");
    eprintln!("  --no-sound   do not open an audio device. The game already runs");
    eprintln!("               silent on a machine that has none; this is for a");
    eprintln!("               machine that has one and would rather it stayed quiet.");
    std::process::exit(2)
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
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    Ok(())
}

