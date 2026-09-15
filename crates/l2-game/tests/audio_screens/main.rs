//! `Panel_OpenRation` (`0x0043A846`) is **four statements** and two of them are
//! sounds:

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

fn headless() -> Option<Audio> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Audio::headless(&platform.vfs))
}

fn listen(d: &mut audio::Director, a: &mut Audio, m: &Machine, g: &Game) {
    d.listen(a, m, g);
}

