//! Reading the England turn-one fixture, against a real install.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-game --test scenario
//! ```
//!
//! **No window is opened and no process is started**; every check reads bytes.
//!
//! # Two gates, not one
//!
//! The save comes from `l2_testkit::england_turn1` — a *named* fixture with a
//! fingerprint — and the map, the tile sets and `Lords2.exe` come from the
//! install. They are different things and were being fetched from the same
//! place: this file used to mount the install and read its `lastturn.sav`,
//! which is the rolling autosave the game rewrites every turn somebody plays.
//!
//! The numbers asserted here are `docs/kingdom.md` §9's — the eight independent
//! predictions it landed against this position — with the county count
//! corrected to **five** owned, one for each of realms 1 to 5. Which realm gets
//! which county is rolled per game and is no longer asserted; see
//! `l2_testkit::ENGLAND_TURN1_COUNTIES`.

mod england_fixture;
pub use england_fixture::*;
mod turn_execution;
pub use turn_execution::*;
mod save_loading;
pub use save_loading::*;

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::scenario;
use l2_kingdom::tables::{Tables, Weather};
use l2_mods::Platform;

fn platform(dir: &PathBuf) -> Platform {
    Platform::builder().base(dir).build().expect("the install mounts")
}

/// The England turn-one fixture, loaded as a `Game`.
macro_rules! game {
    () => {{
        let save = l2_testkit::england!();
        scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads")
    }};
}

/// The install, for the assets that are not the save.
macro_rules! install {
    () => {
        l2_testkit::install!()
    };
}

