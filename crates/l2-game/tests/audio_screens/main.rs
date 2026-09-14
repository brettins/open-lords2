//! **The sounds a screen makes when it opens**, which is where nearly all of
//! the original's non-battlefield audio lives.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_screens
//! ```
//!
//! `tests/audio_wiring.rs` covers the music policy and the narrator's message
//! window. This file covers the class that `docs/audio-triggers.md` had counted
//! and nobody had wired: **`Sound_PlayFile(name, 1, 0)` and
//! `Sound_RestartSlot(n)` in the function that sets `g_screenId`.**
//!
//! # Why the screen arriving is the trigger and not a stand-in for it
//!
//! The temptation is to read `Director`'s screen-stack diff as an approximation
//! — *the original plays a sound, we notice a screen*. It is not.
//! `Panel_OpenRation` (`0x0043A846`) is **four statements** and two of them are
//! sounds:
//!
//! ```c
//! g_screenId = 0x19; Panel_Ration();
//! if (rationAchieved == 0)                                Sound_PlayFile("S021_02.wav", 1, 0);
//! else if (herd && !herdEaten && !grainEaten)             Sound_PlayFile("S021_01.wav", 1, 0);
//! ```
//!
//! The sound *is* the screen opening, with a condition on the county attached.
//! `Sidebar_Button`'s supplies arm, `Panel_SplitButton`, `Map_ZoomOut`,
//! `Panel_JobDetail` and `TileInfo_Draw` are all that shape.
//! mechanism reaches fourteen sites.
//!
//! Everything below drives real [`Event`]s through [`Machine::handle`] and then
//! runs [`audio::Director::listen`], the way `main.rs` does. Nothing constructs
//! a stack, for the reason `audio_wiring.rs` opens with.

mod screens;
pub use screens::*;
mod audio_behavior;
pub use audio_behavior::*;

use l2_game::audio::{self, names, Audio};
use l2_game::game::Assets;
use l2_game::screen::{Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::info::Target;
use l2_game::screens::setup::SetupPage;
use l2_game::input::{Event, Key};
use l2_game::screen::Ctx;
use l2_game::Game;

fn send(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

const APP_ROOT: ScreenId = ScreenId::Setup(SetupPage::Title);

pub(crate) fn world() -> Game {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(14);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].county_count = 1;
    g.player = 1;
    g
}

/// An audio layer that indexes and decodes the player's install with no device.
fn headless() -> Option<Audio> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Audio::headless(&platform.vfs))
}

/// Run the director over the machine as it stands, as the event loop does.
fn listen(d: &mut audio::Director, a: &mut Audio, m: &Machine, g: &Game) {
    d.listen(a, m, g);
}

