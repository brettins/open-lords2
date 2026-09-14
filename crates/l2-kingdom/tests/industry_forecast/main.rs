//! **The industry rows' forecast, by the road the game travels.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test industry_forecast
//! ```
//!
//! `l2_kingdom::county::Industry::next_season` is county `+0x2A8 + c*0x18` —
//! `Industry_LabourEstimate`'s (`0x0044F318`) tail, the four bytes the sidebar's
//! industry rows draw a `Ui_DrawDelta` of. It is written by
//! [`l2_kingdom::industry::preview`]. The four divisors, the four job slots, the flat efficiency and the
//! [`l2_kingdom::field::refresh_estimates`], which is the estimate round the
//! season pass runs.
//!
//! # Why the road and not a hand-built county
//!
//! `docs/decisions.md` C30 and `docs/agents.md`'s *a test that drives the
//! picture from the wrong field passes for ever*: **a field is only tested if
//! something a test reads was written by something the game runs.** The village
//! screen had eleven green tests over four industry records that no importer
//! had ever filled. So this loads the England turn-one fixture through
//! `l2_scenario`, ends a season with [`l2_kingdom::Kingdom::advance_season`],
//! and reads the number back out of the counties the importer built.
//!
//! # Why every constant here is a literal
//!
//! `docs/agents.md`, *how to ablate wrongly*, one: **ablating a constant while
//! computing your probe from that same constant tests nothing at all.** So the
//! expected value below is not `pct(workers / t.commodity[c].divisor, …)` —
//! nothing in this file reads `Tables`, `efficiency_ramp`, `resource_limit` or
//! `pct`. The four divisors, the four job slots, the flat efficiency and the
//! unlimited limit are typed out as the numbers the decompilation has, and the
//! arithmetic is written out longhand.

mod forecast;
pub use forecast::*;

use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_scenario::Scenario;
use l2_testkit::england;

/// `Industry_Produce`'s divisor column, in commodity order — wood, iron,
/// weapons, stone. `docs/kingdom.md` §7.4, and `l2_kingdom::industry`'s module
/// table. **Weapons is 4 and stone is 2**, which is the whole reason a divisor
/// column exists.
const DIVISOR: [i32; 4] = [1, 1, 4, 2];

/// The labour ladder slot each industry draws its workers from — 6 wood
/// cutting, 4 iron mining, 7 blacksmith, 5 stone quarrying. County
/// `+0xC4 + job*0x0C`.
const JOB: [usize; 4] = [6, 4, 7, 5];

/// `if (g_optAdvancedFarming == 0) return 80;` — `FUN_0044F248`'s first
/// statement, and **the whole of the ramp in the shipped game**: England turn
/// one has the option off, so every industry sits at a flat 80 whatever its
/// staffing or its base.
const FLAT_EFFICIENCY: i32 = 80;

/// `FUN_0044EF4E`'s literal for wood, iron and stone once the three flags hold.
const UNLIMITED: i32 = 999;

