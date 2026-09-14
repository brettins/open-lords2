//! **Is anything driving the audio layer?**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_wiring
//! ```
//!
//! `tests/audio_install.rs` checks that the *tables* name files that ship and
//! that the *mixer* turns a file into samples. Both were green, both still are,
//! and **no music has ever played**: `audio::scene` asked whether the screen at
//! the bottom of the stack is a setup page, and the front end is *pushed
//! under* the campaign, so it answered `FrontEnd` 
//! for every state a running game can be in. `docs/decisions.md` C116.
//!
//! The test that covered it did this:
//!
//! ```ignore
//! let machine = Machine::new(ScreenId::Campaign);
//! assert_eq!(scene(&machine, &game), Scene::Campaign { .. });
//! ```
//!
//! — a machine with one screen on it, which the application cannot produce.
//! The assertion was right, the subject was a fixture, and that is
//! `docs/agents.md`'s *a test that drives the picture from the wrong field*
//! with a stack instead of a field.
//!
//! So everything here starts from **the root `main.rs` builds** and reaches
//! every other state through [`Machine::handle`] with real events. Nothing
//! constructs a stack.

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

/// **The screen `main.rs` roots the machine on.** If this stops being what the
/// application starts from, every test below is testing a state nobody reaches
/// — which is the whole defect this file exists for — so it is named once,
/// here, and `the_root_is_the_one_the_application_starts_on` checks it against
/// `main.rs` itself.
const APP_ROOT: ScreenId = ScreenId::Setup(SetupPage::Title);

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

/// A world with a realm holding one county of fourteen, which is what the
/// England start is and what `Scroll1` is the answer to.
pub(crate) fn world() -> Game {
    let mut g = Game::new(5);
    // **Tip screens: No.** A tip posted twenty frames in queues ahead of the
    // messages these tests post and speaks first, which is right; the tips'
    // own narration is asserted in `tests/tips.rs`.
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(14);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].county_count = 1;
    g.player = 1;
    g
}

