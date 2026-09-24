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
        WinitKey::Named(NamedKey::Home) => Key::Home,
        WinitKey::Named(NamedKey::End) => Key::End,
        WinitKey::Named(NamedKey::Insert) => Key::Insert,
        WinitKey::Named(NamedKey::Delete) => Key::Delete,
        WinitKey::Named(NamedKey::F2) => Key::F2,
        WinitKey::Named(NamedKey::F3) => Key::F3,
        WinitKey::Named(NamedKey::F4) => Key::F4,
        WinitKey::Named(NamedKey::F12) => Key::F12,
        WinitKey::Character(s) if ctrl => Key::ctrl_letter(s.chars().next()?),
        WinitKey::Character(s) => Key::letter(s.chars().next()?),
        _ => return None,
    })
}

pub(crate) fn usage() -> ! {
    eprintln!("usage: l2-game <game dir> [--mods <dir>] [--no-sound] [--verify-replay <file>]");
    eprintln!();
    eprintln!("  <game dir>   a Lords of the Realm II install: Lords2.exe, L2_maps.dat,");
    eprintln!("               the fonts, the artwork and the tile sets. Never written to.");
    eprintln!("  --no-sound   do not open an audio device. The game already runs");
    eprintln!("               silent on a machine that has none; this is for a");
    eprintln!("               machine that has one and would rather it stayed quiet.");
    eprintln!("  --verify-replay <file>");
    eprintln!("               replay a recorded network game (.l2nr) and check every");
    eprintln!("               tick's checksum; exits 0 when bit-exact, 1 otherwise.");
    std::process::exit(2)
}

impl super::App {
    /// [D] ours, C238: `Options_Save` on change, not only at exit (`0x0040EBFE`).
    pub(super) fn persist_prefs(&mut self) {
        if self.game.prefs == self.saved_prefs {
            return;
        }
        self.saved_prefs = self.game.prefs;
        if let Err(e) = l2_game::prefs::save(&self.game.prefs) {
            eprintln!("preferences: {e}");
        }
    }

    /// `Display_Init` (`0x0042E5C0`) forces full screen; borderless here, and
    /// `pixels` scales 640 x 480 by F5's whole-number rule (`input::window`).
    pub(super) fn fullscreen(&self) -> Option<winit::window::Fullscreen> {
        self.game.prefs.full_screen.then_some(winit::window::Fullscreen::Borderless(None))
    }

    // C238; App_WndProc 0x004B29BE WM_SYSKEYUP case 0xd is an empty break (004b0000.c:2083).
    // arm: ours/windowed-toggle key
    pub(super) fn toggle_full_screen(&mut self) {
        self.game.prefs.full_screen = !self.game.prefs.full_screen;
        if let Some(w) = &self.window {
            w.set_fullscreen(self.fullscreen());
        }
        self.persist_prefs();
    }
}

pub(crate) fn platform(
    dir: &std::path::Path,
    mods: Option<&std::path::Path>,
) -> Result<Platform, Box<dyn std::error::Error>> {
    let mut builder = Platform::builder().base(dir);
    if let Some(m) = mods {
        let found = l2_mods::discover(m)?;
        let ids: Vec<String> = found.iter().map(|f| f.id.clone()).collect();
        builder = builder.mods_dir(m).enable(ids);
    }
    Ok(builder.build()?)
}

/// `docs/netcode.md` §6 *The replay harness*, in a fresh process.
pub(crate) fn verify_replay(dir: PathBuf, mods: Option<PathBuf>, path: PathBuf) -> ! {
    let run = move || -> Result<String, String> {
        let platform = platform(&dir, mods.as_deref()).map_err(|e| e.to_string())?;
        let tables = platform.kingdom_tables().map_err(|e| e.to_string())?;
        let assets = Assets::load(&platform.vfs).map_err(|e| e.to_string())?;
        l2_game::net::replay::verify_file(&assets, &path, tables)
    };
    // A debug build's `Game` moves overflow the main thread's 1 MB stack.
    let worker = std::thread::Builder::new().stack_size(256 << 20).spawn(run);
    let verdict = match worker.map(|w| w.join()) {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => Err("the replay panicked".to_string()),
        Err(e) => Err(e.to_string()),
    };
    match verdict {
        Ok(said) => {
            println!("{said}");
            std::process::exit(0)
        }
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(1)
        }
    }
}
