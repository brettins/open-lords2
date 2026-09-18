//! **The standings** — `Screen_GreatestNoble` (`0x0041593B`), `g_screenId`
//! `0x20`.
//!
//! ```text
//! Screen_GreatestNoble(firstFrame):                             0x0041593B
//!   Restore_WorkingDir(); DAT_0053F050 = 0            campaign sprite mode
//!   if (firstFrame == 1) FUN_004B11CE()
//!   File_ReadChunk("grtnoble.256", palette, 0x300, 0)
//!   FUN_00408FCB("grtnoble.pl8", 0x1E0)     a raw 640 x 480 page, not a sheet
//!   File_ReadChunk("flags.pl8", g_spriteBank, 160000, 0)  the sheet, into RAM
//!   leader = FUN_00415BDC()                        the ranking — see [`rank`]
//!   for slot in 1..6:
//!     realm = g_nobleColumnRealm[slot]                          0x004D2B58
//!     if (g_realms[realm].strength == 0) continue
//!     x   = g_nobleColumnX[slot]                                0x004D2B40
//!     top = (100 - g_nobleBarPct[realm]) * 2                    0x00522C60
//!     Sprite_WGenSprite(realm.shieldIndex - 1, x, top + 0x2D)   flags.pl8
//!     for (i, hue) in [(0x17, 0x10), (0x18, 0x12), (0x19, 0x14), (0x1A, 0x12)]:
//!       FUN_00403A8F(x + i, top + 0x89, x + i, 0x158, hue)      the pole
//!   Sprite_WGenSprite(5, g_nobleTabX[g_nobleCategory], 0x173)   the marker
//!   g_penAdvance = 0
//!   Eng_DrawString(35, g_nobleCategory, 0x148, 0x1BE, body)   "Most counties,"
//!   if (!allLevel && !tiedAtTop)
//!        Ui_DrawText(g_playerNames[leader], pen + 0x14A, 0x1BE, body)
//!   else Eng_DrawString(35, 7, pen + 0x14A, 0x1BE, body)          "undecided."
//!   Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x1C, 1)   (612, 452)
//!   Gfx_MarkAllDirty(); Gfx_Present(1); Palette_Set(grtnoble.256)
//! ```
//!
//! `docs/draws.md`'s sheet note said frame 5 was *"for the leader"*. It is
//! not: `Sprite_WGenSprite(5, …)` is indexed by `g_nobleCategory` through
//! [`TAB_X`], not by the leading realm, and its x values are the seven tab
//! columns. Frames 0…4 are the five 51 × 92 banners; frame **5 is 23 × 60**,
//! read from the shipped file — the marker that sits under the tab you are
//! looking at. `C194`.
//!
//! # The metric, category by category — `FUN_00415E42` (`0x00415E42`)
//!
//! | # | `L2.eng` 35 | realm field | ours |
//! |---|---|---|---|
//! | 0 | *"Most counties,"* | `+0x29` | [`Realm::county_count`] |
//! | 1 | *"Most castles,"* | `+0x4C`, **as a byte** | `score_inputs[`[`SCORE_INPUT_CASTLES`]`]` |
//! | 2 | *"Most troops,"* | `+0x54` | [`Realm::total_men`] |
//! | 3 | *"Most crowns,"* | `+0x118` | [`Realm::gold`] |
//! | 4 | *"Happiest people,"* | `+0x0C` | [`Realm::mean_happiness`] |
//! | 5 | *"Most people,"* | `+0x10` | [`Realm::population_total`] |
//! | 6 | *"Greatest noble,"* | `6 - (+0x2B)`, or **2 flat before 1270** | [`Realm::rank`] |
//!
//! Two of those are worth saying out loud. **Category 6 is the overall
//! standing and it is deliberately dead for the first two years**: `g_year <
//! 0x4F6` returns a flat 2 for every realm, which makes every bar equal, which
//! makes the line read *"Greatest noble, undecided."* until 1270. And
//! **category 1 reads a byte**: `+0x4C` is the finished-castle count
//! `Castle_BuildTick` (`0x004508DE`) writes
//! a 256th castle would read as none. Reproduced, in [`value`].
//!
//! # The ranking — `FUN_00415BDC` (`0x00415BDC`)
//!
//! ```c
//! leader = 1; best = 0;
//! for (r = 1; r < 6; r++) { v = FUN_00415E42(r); if (best <= v) { leader = r; best = v; } }
//! for (r = 1; r < 6; r++) pct[r] = clamp(PctOf(FUN_00415E42(r), best), 0, 100);
//! allLevel = 1;
//! for (r = 1; r < 6; r++) if (realm r is in play) { first ? remember : differ -> allLevel = 0 }
//! if (!allLevel) { tiedAtTop = any in-play r != leader with pct[r] == pct[leader]; }
//! else           { pct[r] = 50 for every in-play realm; }
//! ```
//!
//! `Screen_HandleInput`'s `0x20` arm
//! has. The table is at `0x004DC890`, seven 24-byte records read out of
//! `Lords2.exe`:
//!
//! ```text
//! x0 46  y0 428  x1  83  y1 463  handler FUN_0043524E  kind 1  id 0
//! x0 84            x1 121                                      id 1
//! …                                                            …
//! x0 274           x1 312                                      id 6
//! ```
//!
//! `FUN_0043524E` is three statements — `g_nobleCategory = g_uiHotspotId;
//! g_redrawRequest = 2; FUN_004B3994(g_uiHotspotId);` — so a tab changes the
//! category, repaints, and **speaks its own name**: `S035_01.wav` …
//! `S035_07.wav`, the files named after the `L2.eng` group this screen draws.
//!
//! `Screen_FrameInput`'s `0x20` arm is the county panels' shape exactly: a
//! turn ending under it force-closes it, otherwise a right release anywhere or
//! `Ui_OkButtonClicked` (`0x0040E7E4`) in the corner box sets `g_screenId = 0`.
//!
//! `[V]`, both exits write the same byte.
//!
//! That is `docs/decisions.md` C190's `Transition::Goto(Campaign)` — *go to
//! screen X, unwinding the stack*. C190 built it and does not forbid it; what
//! it warns against is `Goto` where the byte means "come back where you were",
//! and this arm names a screen instead. Ours popped onto the court, which was
//! filed as deliberate and is now the behaviour the original does not have.
//!
//! The way in is `FUN_004351C4` (`0x004351C4`), the court's one widget:
//!
//! ```c
//! g_screenId = 0x20; g_redrawRequest = 1;
//! if (g_multiplayer == 0) FUN_00435211();          /* Score_RankAndRefreshAll */
//! else                    Net_SendCommand(0x3B, 0);
//! FUN_004B3994(g_nobleCategory);
//! ```
//!
//! **The recount is the interesting half.** `FUN_00435211`
//! (`Score_RankAndRefreshAll`, `0x00435211`) is `Score_RankRealms()` then
//! `Realm_UpdateTotals(r)` for r in 1..=5 — the totals five of the six score
//! inputs are read from, rebuilt for every realm, before the page is drawn.

mod types;
pub use types::*;
mod view;
pub use view::*;

use l2_view::Canvas;

use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 35 — eight strings, which is the whole group, and this
/// screen is its **only consumer in the binary**. `CLAUDE.md` rule 6.
pub const GROUP: usize = 35;

/// The seven categories are group 35 indices 0…6, in this order.
pub const COUNTIES: usize = 0;
pub const CASTLES: usize = 1;
pub const TROOPS: usize = 2;
pub const CROWNS: usize = 3;
pub const HAPPINESS: usize = 4;
pub const PEOPLE: usize = 5;
pub const GREATEST_NOBLE: usize = 6;
pub const UNDECIDED: usize = 7;

pub const CATEGORIES: usize = 7;

pub const BACKGROUND: &str = "Grtnoble.pl8";
pub const PALETTE: &str = "Grtnoble.256";
pub const FLAGS: &str = "Flags.pl8";
pub const MARKER_FRAME: usize = 5;

/// `g_nobleColumnX` (`0x004D2B40`) — entries 1…5, the x of each **column**.
pub const COLUMN_X: [i32; 5] = [39, 167, 294, 424, 552];
/// `g_nobleColumnRealm` (`0x004D2B58`) — which realm stands in each column,
/// and **it is not the realm order**: realm 1 is placed in the middle.
pub const COLUMN_REALM: [u8; 5] = [5, 3, 1, 2, 4];

/// `g_nobleTabX` (`0x004D2B20`) — the marker's x per category. 38 apart, and
/// eight pixels right of each tab's own [`TABS`] left edge.
pub const TAB_X: [i32; CATEGORIES] = [54, 92, 130, 168, 206, 244, 282];
pub const MARKER_Y: i32 = 0x173;

pub const FLAG_TOP: i32 = 0x2D;
pub const POLE_TOP: i32 = 0x89;
pub const POLE_BOTTOM: i32 = 0x158;
/// Four one-pixel `FUN_00403A8F` lines: `(dx, colour)`. A pole, lit from the
/// left. **`FUN_00403A8F` is not one of the audit's 26 primitives**, which is
/// why a bar chart of up to twenty lines counted as zero draw calls.
pub const POLE: [(i32, u8); 4] = [(0x17, 0x10), (0x18, 0x12), (0x19, 0x14), (0x1A, 0x12)];
pub const PIXELS_PER_PERCENT: i32 = 2;
pub const LEVEL_BAR_PCT: i32 = 50;

pub const LINE_AT: (i32, i32) = (0x148, 0x1BE);
pub const NAME_DX: i32 = 0x14A - 0x148;

pub const OK: Rect = Rect::new(640 - 0x1C, 480 - 0x1C, 24, 24);
pub const OK_MODE: usize = 1;

/// `g_nobleTabs` (`0x004DC890`) — seven `Hotspot_Test` records, `{x0, y0, x1,
/// y1}`, all kind 1, all handler `FUN_0043524E`, ids 0…6. Read out of
/// `Lords2.exe` at a 24-byte stride.
pub const TABS: [Rect; CATEGORIES] = [
    tab(46, 83),
    tab(84, 121),
    tab(122, 159),
    tab(160, 197),
    tab(198, 235),
    tab(236, 273),
    tab(274, 312),
];
pub const TAB_Y0: i32 = 428;
pub const TAB_Y1: i32 = 463;

const fn tab(x0: i32, x1: i32) -> Rect {
    Rect::new(x0, TAB_Y0, x1 - x0, TAB_Y1 - TAB_Y0)
}

/// **`g_year < 0x4F6`** — before 1270, `FUN_00415E42` returns a flat 2 for
/// every realm in the *Greatest noble* category, so the overall standing is
/// *"undecided."* for the first two years of every game.
pub const GREATEST_NOBLE_YEAR: i32 = 0x4F6;
pub const GREATEST_NOBLE_EARLY: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standings {
    /// `g_nobleBarPct` (`0x00522C60`) — indexed by realm, 0…100. Index 0 is
    /// never written and never read, as the original's is not.
    pub pct: [i32; l2_kingdom::MAX_REALMS],
    pub leader: u8,
    /// `DAT_00522C78` — every in-play realm scores the same.
    pub all_level: bool,
    /// `DAT_00522C94` — somebody other than the leader matches its bar.
    pub tied_at_top: bool,
}

/// **`FUN_00435211`, `Score_RankAndRefreshAll` (`0x00435211`)** — the recount
/// the court's button runs before it opens this page.
///
/// Every field it writes — `+0x2B` the rank, `+0x50` the score, `+0x29`,
/// `+0x10`, `+0x14`, `+0x0C`, `+0x58`, `+0x60`, `+0x2C`, `+0x54` — is on
/// [`l2_kingdom::Realm`]
/// **opening a scoreboard changes the world**, and it changes it only where
/// the totals had gone stale since the last AI turn: run twice over an
/// unchanged kingdom it is idempotent, and run after a county has changed
/// hands mid-turn it is not.
///
/// The original says the same thing in the only way it can: in a network game
/// `Court_OpenGreatestNoble` does **not** call this, it sends
/// `Net_SendCommand(0x3B, 0)`, whose deferred action `NetAct_RankRealms`
/// (`0x00448422`) runs the identical pair on every peer. We have no network
/// game and take the single-player arm; when one exists this call site is one
/// of the places that has to become a command.
pub fn recount(game: &mut crate::game::Game) {
    let t = game.kingdom.tables;
    l2_kingdom::ai::rank_realms(&t, &mut game.kingdom.realms);
    for realm in 1..l2_kingdom::MAX_REALMS as u8 {
        let (armies, total_men) = game.kingdom.campaign.units.realm_totals(realm);
        let l2_kingdom::Kingdom { realms, counties, county_count, .. } = &mut game.kingdom;
        l2_kingdom::ai::update_realm_totals(
            &mut realms[realm as usize],
            counties,
            *county_count,
            realm,
            armies,
            total_men,
        );
    }
}

