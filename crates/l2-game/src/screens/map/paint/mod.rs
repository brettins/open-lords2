
mod units;
pub use units::*;
mod ui;
pub use ui::*;

use super::*;

pub(super) const CLOCK_X: i32 = 0x168;
pub(super) const CLOCK_Y: i32 = 6;
pub(super) const SEASON_X: i32 = 0x16C;
/// `L2.eng` group 29: `"No Season"`, `"Spring"`, `"Summer"`, `"Autumn"`,
/// `"Winter"` — indexed by `g_season` with no adjustment.
pub const SEASON_GROUP: usize = 29;
/// `Ui_DrawCount(gold, 0, 500, 6, …)` — the treasury, and its group 8 noun
/// index. 0/1 is *"Crown."* / *"Crowns."*.
pub(super) const GOLD_X: i32 = 500;
pub(super) const GOLD_NOUN: usize = 0;

