//! **Saving a game and loading it back**
//! real reason.
//!
//! ```text
//! cargo test -p l2-game --test save                       # all of it but one
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test save
//! ```
//!
//! # Why comparing the two structs is not enough
//!
//! `Game` derives `PartialEq`, so `assert_eq!(saved, loaded)` is one line and
//! proves something — but it is the *weak* form of the claim, because it only
//! ever compares the loader against the writer. A field that both of them
//! ignore round-trips perfectly and is gone.
//!
//! So the test that carries the weight is [`ten_seasons_from_a_reloaded_game_are_the_same_ten`]:
//! it takes one game, saves it, loads the save, and then **plays both forward
//! ten seasons**, requiring the `l2_net::Canonical` digest of the two kingdoms
//! to match after every single one. That digest is the same number a lockstep
//! peer exchanges every tick (`docs/netcode.md` §5)
//! generator's state, or the history ring's head, or one county's dryness, does
//! not survive the first season that reads it — the digests part company and
//! the test names the season they parted on.
//!
//! A save format is only worth having if resuming is indistinguishable from
//! never having stopped. That is exactly what this measures.
//!
//! # And the files
//!
//! **Every test that touches a save has a directory of its own**, and [`Saves`]
//! is the only way this file gets one. It is never the default directory — a
//! test that wrote into `%APPDATA%` would destroy the player's own saves on the
//! machine it ran on — and it is never *shared*, because a shared one is what
//! made `the_save_screen_writes_a_file_and_the_load_screen_reads_it_back` fail
//! about one run in a hundred. See [`Saves`].
//!
//! Nothing here writes a `.sav` anywhere near the repository
//! is `.l2sav` in any case — `l2_game::save::EXTENSION` says why.

mod roundtrip;
pub use roundtrip::*;
mod isolation;
pub use isolation::*;
mod ui;
pub use ui::*;
mod autosave;
pub use autosave::*;

use std::path::PathBuf;

use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

