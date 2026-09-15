//! `tests/audio_install.rs` checks that the *tables* name files that ship and
//! that the *mixer* turns a file into samples. Both were green, both still are,
//! and **no music has ever played**: `audio::scene` asked whether the screen at
//! the bottom of the stack is a setup page, and the front end is *pushed
//! under* the campaign, so it answered `FrontEnd` 
//! for every state a running game can be in. `docs/decisions.md` C116.

mod routing;
pub use routing::*;
mod ui_and_speech;
pub use ui_and_speech::*;
mod audio_controls_and_feedback;
pub use audio_controls_and_feedback::*;

use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

const APP_ROOT: ScreenId = ScreenId::Setup(SetupPage::Title);

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

pub(crate) fn world() -> Game {
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(14);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].county_count = 1;
    g.player = 1;
    g
}

