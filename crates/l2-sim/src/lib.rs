
pub mod ai;
pub mod battle;
pub mod castle;
pub mod cue;
pub mod facing;
pub mod figure;
pub mod fire;
pub mod formation;
pub mod melee;
pub mod missile;
pub mod movement;
pub mod pathfind;
pub mod proving;
pub mod runner;
pub mod siege;
pub mod terrain;
pub mod troop;
pub mod unit;

pub use ai::{Ai, AiField, Action, World};
pub use battle::Battle;
pub use castle::{CastleSheet, CastleSheets, CastleTables};
pub use cue::Cues;
pub use facing::{facing_from_delta, FACINGS, FACING_DELTA};
pub use figure::{Figure, Motion, Role, Side, State, SIDE_A, SIDE_B};
pub use missile::{MissileStats, WeaponClass};
pub use movement::{move_delay, ticks_per_cell, CellEntry, Progress};
pub use pathfind::{Grid, Outcome, Pos};
pub use runner::{BattleRunner, Conclusion, End, Fighter, Muster};
pub use siege::{CastleDamage, SiegeState, WallBlow};
pub use terrain::Battlefield;
pub use troop::{Troop, TroopStats, TroopTable, ALL_TROOPS};
pub use unit::{BattleUnit, Units, MAX_UNITS};

pub const MAX_FIGURES: usize = 80;
