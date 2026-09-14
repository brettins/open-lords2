//! Ending a turn: the phase machine goes round, the AI finishes, and the same
//! kingdom ended twice produces the same numbers.
//!
//! No install and no window. The kingdoms here are built by hand so that each
//! assertion is about *one* thing the spine does — the tests that run the
//! England turn-one scenario are in `tests/scenario.rs`.

mod spine;
pub use spine::*;
mod military;
pub use military::*;

use l2_kingdom::phase::Phase;
use l2_kingdom::realm::AI_STEP_DONE;
use l2_kingdom::tables::Tables;
use l2_game::turn;
use l2_game::Game;

use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::units_tick::Contact;
use l2_kingdom::merchant::MerchantRoutes;
use l2_kingdom::unit::{Unit, UnitKind};

