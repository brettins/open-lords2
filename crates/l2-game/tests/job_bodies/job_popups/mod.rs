#![allow(unused_imports)]

mod dirty;
pub use dirty::*;
mod grain_and_cattle;
pub use grain_and_cattle::*;
mod industry_and_building;
pub use industry_and_building::*;

use super::*;
use super::events_and_letters::*;
use super::tile_panel::*;
use l2_game::game::Assets;
use l2_game::message::category;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::job::JobScreen;
use l2_game::shell::font::{Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::event::{EventKind, RealmPurse};
use l2_kingdom::field::FieldType;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::tables::{
    Commodity, Season, Tables, Weather, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING,
    JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
    JOB_WOOD_CUTTING,
};
use l2_mods::Platform;
use l2_view::Canvas;



const JOB_BLACKSMITH: usize = 7;
const ARMOUR: usize = 5;
/// `DAT_004D29C8[5]` — group 8's singular index for armour, whose plural at 29
/// is *"Armour"* again.
const ARMOUR_NOUN: usize = 28;

