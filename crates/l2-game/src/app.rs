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

pub(crate) fn usage() -> ! {
    eprintln!("usage: l2-game <game dir> [--mods <dir>] [--no-sound]");
    eprintln!();
    eprintln!("  <game dir>   a Lords of the Realm II install: Lords2.exe, L2_maps.dat,");
    eprintln!("               the fonts, the artwork and the tile sets. Never written to.");
    eprintln!("  --no-sound   do not open an audio device. The game already runs");
    eprintln!("               silent on a machine that has none; this is for a");
    eprintln!("               machine that has one and would rather it stayed quiet.");
    std::process::exit(2)
}
