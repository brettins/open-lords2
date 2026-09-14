
mod units;
pub use units::*;
mod ui;
pub use ui::*;

use super::*;

/// `Ui_DrawYear(g_year, 0x168, 6, 3)` — where the year starts.
/// one of the bar's three readings sits on.
pub(super) const CLOCK_X: i32 = 0x168;
pub(super) const CLOCK_Y: i32 = 6;
/// `Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, …)` — the season's
/// **base**, which the year's own width is added to. Four pixels right of
/// [`CLOCK_X`], and that four is the original's, not a rounding of ours.
pub(super) const SEASON_X: i32 = 0x16C;
/// `L2.eng` group 29: `"No Season"`, `"Spring"`, `"Summer"`, `"Autumn"`,
/// `"Winter"` — indexed by `g_season` with no adjustment.
pub const SEASON_GROUP: usize = 29;
/// `Ui_DrawCount(gold, 0, 500, 6, …)` — the treasury, and its group 8 noun
/// index. 0/1 is *"Crown."* / *"Crowns."*.
pub(super) const GOLD_X: i32 = 500;
pub(super) const GOLD_NOUN: usize = 0;

