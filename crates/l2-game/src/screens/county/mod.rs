//! The county panels — **four of them**, not one.
//!
//! This screen used to be one invented full-screen page listing twenty-two
//! fields with two arrow rows bolted underneath. It was not what the original
//! draws, and nobody had looked. `docs/screens-county.md` is the reading; this
//! is the rebuild.
//!
//! # What the original does
//!
//! The county's numbers live in the **county strip**, the 162 × 94 plate at
//! (478, 156) in the campaign map's right-hand column, and the strip is a
//! **2 × 2 hotspot**: `CountyStrip_Click` (`0x00438CEB`) opens the population
//! panel from its top-left quadrant, happiness from top-right, tax from
//! bottom-left and rations from bottom-right,
//! any of them. Each is a floating `Ui_DrawBox` window over whatever was
//! underneath, and each has **two ways out**: the 24 × 24 picture in its
//! bottom-right corner, and the **right mouse button**
//! anywhere. That picture is a cursor arrow pointing into a black hole — a
//! close button whose artwork is the instruction, and not the tick this file
//! used to call it. `docs/screens-county.md` §2.6 and §2.7.
//!
//! So this screen draws the strip at the original's own coordinates, over the
//! original's own `Misc_cty.pl8` plates, with the original's own quadrants
//! live; and it draws one of the four panels at the original's own rectangle,
//! with the original's own rows in the original's own order, using the
//! original's `Panels.pl8` box kit and `System2.pl8` buttons.
//!
//! # The painters, address by address
//!
//! Transcribed from the decompilation, coordinates resolved to decimal in the
//! trailing comment. Every one of the four opens with `FUN_004050C0()`, which
//! is `Map_DrawFrame()` behind a 16 ms throttle — **the campaign map beneath
//! the inset, repainted every frame**. It is excluded from the counts below
//! and from `tools/audit/draws-A.json`; our equivalent is
//! [`Screen::is_overlay`] plus the map screen sitting under us on the stack.
//!
//! ```text
//! Panel_Population():                                           0x004110B1
//!   FUN_004050C0()                       -> Map_DrawFrame()      [excluded]
//!   Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)        (16, 48) 448 x 368
//!   Eng_DrawString(73, 0, 0x14, 0x38, heading)  "Population in"    (20, 56)
//!   Eng_DrawString(100, slot*20 + county, pen + 0x16, 0x38, heading)
//!   Ui_HistoryGraph(0x20, 0x54, 0)         (32, 84) 402 x 155, mode 0 -> peak
//!   Ui_DrawYear(DAT_00553228, 0x20, 0xF0, 3)     first year in the window
//!   Ui_DrawText("-", pen + 0x20, 0xF0, body)                     (32, 240)
//!   Ui_DrawYear(g_year, 0x58, 0xF0, 3)                           (88, 240)
//!   Eng_DrawString(73, 8, 0xB0, 0xF0, body) "Greatest population"(176, 240)
//!   Ui_DrawNumber(peak, '@', "", pen + 0xB0, 0xF0, body)   the graph's return
//!   Eng_DrawString(73, 9, pen + 0xB0, 0xF0, body)              "people."
//!   Eng_DrawString(73, 1, 0x30, 0x10A, heading) "Last season"    (48, 266)
//!   Ui_DrawNumber(county.popLast, '@', "", 0x150, 0x10A, heading)(336, 266)
//!   Eng_DrawString(73, 2, 0x30, 0x12A, body)    "Births"         (48, 298)
//!   Ui_DrawDelta(county.births,  0, "", "", 0x150, 0x12A, body, 0x3F, 0xF9)
//!   Eng_DrawString(73, 3, 0x30, 0x13A, body)    "Deaths"         (48, 314)
//!   Ui_DrawDelta(-county.deaths, 0, ..., 0x150, 0x13A, ...)
//!   Eng_DrawString(73, 4, 0x30, 0x14A, body)    "Army"           (48, 330)
//!   Ui_DrawDelta(county.popArmy, 0, ..., 0x150, 0x14A, ...)
//!   if emigrants == 0:
//!     Eng_DrawString(73, 10, 0x30, 0x15A, body) "No emigration."  (48, 346)
//!   else:
//!     Eng_DrawString(73, 5, 0x30, 0x15A, body)  "Emigrants to"
//!     Eng_DrawString(100, slot*20 + emigrantDestination, pen + 0x30, 0x15A)
//!     Ui_DrawDelta(-county.emigrants, 0, ..., 0x150, 0x15A, ...)
//!   if immigrants == 0:
//!     Eng_DrawString(73, 11, 0x30, 0x16A, body) "No immigration." (48, 362)
//!   else:
//!     Eng_DrawString(73, 6, 0x30, 0x16A, body)  "Total immigrants"
//!     Ui_DrawDelta(county.immigrants, 0, ..., 0x150, 0x16A, ...)
//!   Eng_DrawString(73, 7, 0x30, 0x182, heading) "This Season"     (48, 386)
//!   Ui_DrawNumber(county.population, '@', "", 0x150, 0x182, heading)
//!   Ui_OkButton(0x1B4, 0x184, 0)                                (436, 388)
//! ```
//!
//! ```text
//! Panel_Happiness():                                            0x004116FB
//!   FUN_004050C0()                                               [excluded]
//!   Ui_DrawBox(0x10, 0x30, 0x1C, 0x18)        (16, 48) 448 x 384
//!   Eng_DrawString(85, 0, 0x14, 0x38, heading)  "Happiness in"     (20, 56)
//!   Eng_DrawString(100, slot*20 + county, pen + 0x16, 0x38, heading)
//!   Ui_HistoryGraph(0x20, 0x54, 1)                (32, 84), mode 1
//!   Ui_DrawYear(DAT_00553228, 0x20, 0xF0, 3)                     (32, 240)
//!   Ui_DrawText("-", pen + 0x20, 0xF0, body)
//!   Ui_DrawYear(g_year, 0x58, 0xF0, 3)                           (88, 240)
//!   Eng_DrawString(85, 8, 0xB0, 0xF1, body)  "Average happiness" (176, 241)
//!   Ui_DrawNumber(county.happinessAvg, '@', "", pen + 0xB0, 0xF1, body)
//!   Pl8_DrawFrame(Misc_cty, 0x17, pen + 0xB0, 0xEF)      the face, 20 x 18
//!   Eng_DrawString(85, 1, 0x30, 0x10C, heading) "Last season"    (48, 268)
//!   Ui_DrawNumber(county.happinessLast, '@', "", 0x150, 0x10C, heading)
//!   Pl8_DrawFrame(Misc_cty, 0x17, pen + 0x150, 0x10F)            the face
//!   Eng_DrawString(85, 2, 0x30, 0x12A, body) "From taxes"        (48, 298)
//!   Ui_DrawDelta(county.shownTax,    0, ..., 0x150, 0x12A, ...)
//!   Eng_DrawString(85, 3, 0x30, 0x13A, body) "From ration"       (48, 314)
//!   Ui_DrawDelta(county.shownRation, 0, ..., 0x150, 0x13A, ...)
//!   Eng_DrawString(85, 4, 0x30, 0x14A, body) "From health"       (48, 330)
//!   Ui_DrawDelta(county.shownHealth, 0, ..., 0x150, 0x14A, ...)
//!   Eng_DrawString(85, 5, 0x30, 0x15A, body) "From army"         (48, 346)
//!   Ui_DrawDelta(county.shownArmy,   0, ..., 0x150, 0x15A, ...)
//!   Eng_DrawString(85, 6, 0x30, 0x16A, body) "From ale"          (48, 362)
//!   Ui_DrawDelta(county.shownAle,    0, ..., 0x150, 0x16A, ...)
//!   Eng_DrawString(85, 9, 0x30, 0x17A, body) "From events"       (48, 378)
//!   Ui_DrawDelta(county.shownEvents, 0, ..., 0x150, 0x17A, ...)
//!   Eng_DrawString(85, 7, 0x30, 0x192, heading) "This Season"    (48, 402)
//!   Ui_DrawNumber(county.happiness, '@', "", 0x150, 0x192, heading)
//!   Pl8_DrawFrame(Misc_cty, 0x17, pen + 0x150, 0x195)            the face
//!   Ui_OkButton(0x1B4, 0x194, 0)                                (436, 404)
//! ```
//!
//! ```text
//! Panel_Tax():                                                  0x0041152F
//!   FUN_004050C0()                                               [excluded]
//!   Ui_DrawBox(0x50, 0x90, 0x14, 0x09)       (80, 144) 320 x 144
//!   Pl8_DrawFrame(Misc_cty, 0x3E, 0x140, 0xA0)      the vignette (320, 160)
//!   Eng_DrawString(86, 1, 0x60, 0xA8, body)     "Tax rate"       (96, 168)
//!   Ui_DrawNumber(county.taxRate, ' ', "%", 0x100, 0xA8, body)  (256, 168)
//!   Eng_DrawString(86, 2, 0x60, 0xC8, body)     "People pay"     (96, 200)
//!   Ui_DrawCount(county.taxShown, 0, pen + 0x60, 0xC8, body) "Crown/Crowns."
//!   Eng_DrawString(86, 3, 0x60, 0xE8, body)     "This county"    (96, 232)
//!   Ui_DrawHappinessDelta(realm.taxHapEmpire + county.dHapTaxLocal,
//!                         0xF0, 0xE8, body, 0x3F, 0xF9)         (240, 232)
//!   Eng_DrawString(86, 4, 0x60, 0x100, body)    "Other counties" (96, 256)
//!   Ui_DrawHappinessDelta(county.taxHapOther, 0xF0, 0x100, ...)  (240, 256)
//!   Ui_OkButton(0x174, 0x104, 0)                                (372, 260)
//!   [Screen_DrawWidgets 0x15] Widget_Draw(0, 0, &g_taxWidgets, 2)
//! ```
//!
//! **`L2.eng` 86/0 *"Tax in"* is drawn by nothing.** The painter's four
//! `Eng_DrawString` sites pass 1, 2, 3 and 4, and a grep of the whole corpus for
//! `(0x56,` finds exactly those four. It is the same shape as 31/21 *"Morale"*
//! and as the court's 70/1 *"Arms"*: a caption the original left in the file.
//! [`g86::TITLE_NEVER_DRAWN`] names it so nobody goes looking, and **we used to
//! draw it** — a "TAX IN" heading of ours at (96, 152) that the game has never
//! put on that panel.
//!
//! ```text
//! Panel_Ration():                                               0x00411B72
//!   rows = g_optArmiesEat ? 0x11 : 0x0F
//!   FUN_004050C0()                                               [excluded]
//!   Ui_DrawBox(0x80, 0x60, 0x12, rows)      (128, 96) 288 x 240 or 288 x 272
//!   Ui_DrawCentred(87, 0, 0x80, 0x68, 0x120, heading)  "Ration" (128, 104) w288
//!   Eng_DrawString(87, 1, 0x90, 0x88, body)     "Wanted:"       (144, 136)
//!   Eng_DrawString(21, county.rationWanted, 0xF0, 0x88, body)   (240, 136)
//!   Eng_DrawString(87, 2, 0x90, 0xA1, body)     "Achieved:"     (144, 161)
//!   Eng_DrawString(21, county.rationAchieved, 0xF0, 0xA1, body, colour)
//!                                     colour 0xF9 when it differs from wanted
//!   Ui_DrawHappinessDelta(county.dHapRation, 0x154, 0xA1, ...)  (340, 161)
//!   Eng_DrawString(87, 3, 0x90, 0xBA, body)     "Health:"       (144, 186)
//!   Eng_DrawString(20, county.healthBand, 0xF0, 0xBA, body)     (240, 186)
//!   Ui_DrawHappinessDelta(county.dHapHealth, 0x154, 0xBA, ...)  (340, 186)
//!   Pl8_DrawFrame(Misc_cty, 0x21, 0x90,  0xDC)   grain, 36 x 27 (144, 220)
//!   Pl8_DrawFrame(Misc_cty, 0x26, 0x172, 0xDC)   cattle, 37 x 24(370, 220)
//!   Pl8_DrawFrame(Misc_cty, 0x21, 0xE0,  0x100)  grain          (224, 256)
//!   Pl8_DrawFrame(Misc_cty, 0x26, 0x11C, 0x100)  cattle         (284, 256)
//!   Pl8_DrawFrame(Misc_cty, 0x2A, 0x158, 0x100)  the third, 23 x 19 (344, 256)
//!   Pl8_DrawFrame(Misc_cty, 0x18, 0x90,  0x11A)  8 x 18         (144, 282)
//!   Eng_DrawString(87, 5, 0xA0, 0x11E, body)     "Fed"          (160, 286)
//!   Eng_DrawString(87, 4, 0x90, 0x134, body)     "Eaten"        (144, 308)
//!   Ui_DrawNumberRight(county.grainEaten, ' ', "", 0xD0,  0x134, 0x40, body)
//!   Ui_DrawNumberRight(county +0x170,     ' ', "", 0xD0,  0x11E, 0x40, body)
//!   Ui_DrawNumberRight(county.herdEaten,  ' ', "", 0x10A, 0x134, 0x40, body)
//!   Ui_DrawNumberRight(county +0x174,     ' ', "", 0x10A, 0x11E, 0x40, body)
//!   Ui_DrawNumberRight(county +0x16C,     ' ', "", 0x144, 0x11E, 0x40, body)
//!   Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)      (388, 308) or (388, 340)
//!   if g_optArmiesEat:
//!     Ui_DrawNumber(county.friendly + county.enemy, ' ', "", 0x88, 0x150, body)
//!     Eng_DrawString(87, 8, pen + 0x88, 0x150, body)
//!                              "men foraging in the county."    (136, 336)
//!   [Screen_DrawWidgets 0x19] Widget_Draw(0, 0, &g_rationWidgets, 2)
//!                             Panel_RationSlider()
//! ```
//!
//! # Two things this file used to get wrong about alignment.
//! same mistake
//!
//! * **`Ui_DrawNumber` and `Ui_DrawDelta` draw *left*-aligned from their `x`.**
//!   `Ui_NumberToBuffer` formats into a buffer and `Ui_DrawText` puts it at `x`;
//!   nothing measures it. `docs/screens-county.md` §5.1 says *"values
//!   right-anchored from x = 336"* and that is wrong — 336 is where the digits
//!   **start**. The `'@'` lead is a blank glyph that reserves one character's
//!   width for a sign, which is how a `+7` and a `7` line up; it is not
//!   right-alignment.
//! * **`Ui_DrawNumberRight` centres.** Its whole body after building the string
//!   is `FUN_004025D7`, the same helper `Ui_DrawCentred` calls, and that is
//!   `x + max(0, (width - textWidth) / 2)`. The ration panel's five calls pass
//!   width `0x40`, and the proof is in the artwork: the
//!   three "Fed" columns start at x 208, 266 and 324, so their centres are 240,
//!   298 and 356 — and the three icons above them are drawn at 224 (36 wide),
//!   284 (37 wide) and 344 (23 wide), whose centres are 242, 302 and 355. The
//!   numbers sit under their pictures. Right-aligned they would *end* at 208,
//!   266 and 324 and stand left of every icon. `docs/screens-county.md` §5.4
//!   calls them right-aligned; they are not.
//!
//! # What is still ours, and says so
//!
//! * **The history graph, and it is a whole picture.** `Ui_HistoryGraph`
//!   (`0x004156A7`); here is all of it.
//!
//!   ```text
//!   Ui_HistoryGraph(x, y, mode) -> peak:                       0x004156A7
//!     File_ReadChunk("graphs.pl8", scratch, 200000, 0)
//!     Ui_DrawInsetRect(x, y, 0x192, 0x9B)              402 x 155 recess
//!     Sprite_WGenSprite(mode == 0 ? 0 : 1, x + 1, y + 1)
//!                              Graphs.pl8 frame 0 or 1, both 402 x 153
//!     a     = Table_Lookup(g_historyLength, 0x004D29F8, 10, 0x16)
//!     pitch = *(int*)(0x004D2AC0 + a * 4)
//!     peak  = max over g_historyLength turns from g_historyHead, wrapping 400
//!     div   = Table_Lookup(peak, 0x004D2A48, 0x0F, 100)
//!     for i in 0 .. g_historyLength:                 one bar per turn
//!       Sprite_WGenHSprite(i & 1 ? a : a + 1,
//!                          x + 1 + pitch * i + pitch / 2,
//!                          y + 0x9A - value / div)
//!   ```
//!
//!   The two tables are read out of `Lords2.exe` and they agree with the
//!   artwork, which is the check that makes this more than a transcription:
//!   `0x004D29F8` is the (threshold, value) ladder `<10 -> 2, <20 -> 4, <30 ->
//!   6, <40 -> 8, <50 -> 10, <60 -> 12, <80 -> 14, <100 -> 16, <130 -> 18,
//!   <200 -> 20`, default `22`, and `0x004D2AC0` indexed by that answer gives
//!   pitches `40, 20, 12, 10, 8, 6, 5, 4, 3, 2, 1`. `Graphs.pl8` has **24
//!   frames**: 0 and 1 are the two 402 × 153 backgrounds, and 2 … 23 are bar
//!   sprites 100 pixels tall in eleven equal-width pairs — 48, 26, 16, 12, 10,
//!   8, 6, 5, 4, 3, 2 — one pair per pitch class, alternating so neighbouring
//!   bars are distinguishable. `0x004D2A48` is the vertical divisor ladder,
//!   fifteen pairs from `<101 -> 1` to `<5000 -> 50`, default 100.
//!
//!   `l2-kingdom` keeps no history array (`docs/screens-county.md` §7 has the
//!   layout: 400 turns × 16 counties × 8 bytes at `0x0056D8C0`), and
//!   `Graphs.pl8` is not one of the sheets `l2-view` loads, so the rectangle is
//!   an empty recess that says so. It is a stub and it is meant to look like
//!   one, and what is missing is now written down.
//! * **The two years under the graph.** `Ui_DrawYear(DAT_00553228, …)` prints
//! the year at the head of the history window;
//!   such year, and drawing only the right-hand one would be worse than
//!   drawing neither. Both are recorded missing.
//! *(**Fixed.** This list used to carry a fourth entry: "what is behind the
//! panels — the original has the campaign map there; the map screen is another
//! file, so this one paints a flat ground." It no longer does. The panels are
//! [overlays](crate::screen::Screen::is_overlay) and the machine paints the map
//! screen beneath them, which is the same correction the village needed —
//! `docs/decisions.md` C22.)*
//! *(**Fixed.** This list used to carry: "the strings on the panels — ours,
//! transcribed from the `L2.eng` group each row names", and, above it, "the
//! font on the four panels — the panels still use our own 5 × 7 font at the
//! original's coordinates". Both are gone. All four panels now draw through
//! [`Pen`](crate::shell::Pen), which is `Fntl2_14.pl8` and `Fntl2_22.pl8` read
//! through `g_glyphWidths`, and every string is fetched from the install's own
//! `L2.eng` at the group and index the painter passes. The transcriptions stay
//! as the **fallback** for a machine with no game, which is what
//! [`draw_strip`] has always done and what the tests run against.)*
//! *(**Fixed.** This list used to carry a fifth entry: "the bottom strip reads
//! BACK TO MAP; the original's is End Turn, which is the map screen's
//! business." It was a rectangle of ours drawn over — and hit-tested ahead of —
//! a live control of the game's, at (478, 460), which is `g_sidebarButtons`
//! record 5 to the pixel. It is gone, and the whole right-hand column is now
//! passed down to the map screen the way `Screen_FrameInput`'s six guards pass
//! it.)*
//!
//! # What is not here at all
//!
//! Labour, sowing, ale and field types, because **none of them is on a county
//! panel in the original either**. Peasants are moved by rubber-band drag on
//! the village screen (0x02), ale is bought from the merchant (0x08), and
//! fields are painted on the map. `docs/screens-county.md` §6.4 and §6.5.

mod layout;
pub use layout::*;
mod draw;
pub use draw::*;
mod strip;
pub use strip::*;

use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};

use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen, TRAILING};
use crate::widget;

/// The four panels, in the order Up and Down cycle them.
///
/// **The order is ours**; the original has no ordering because it has no
/// keyboard route in. Tax and Ration lead because they are the two that take
/// orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Tax,
    Ration,
    Population,
    Happiness,
}

pub const PANELS: [Panel; 4] = [Panel::Tax, Panel::Ration, Panel::Population, Panel::Happiness];

// ----------------------------------------------------- the `L2.eng` strings
//
// One constant per `(group, index)` the four painters pass, carrying our own
// transcription as the fallback for a machine with no game.

/// One `L2.eng` reference: the group, the index the painter passes, and our
/// transcription of it.
///
/// **State the limit where the constant lives**, because a check on existence
/// A coordinator check that every `(group, index)`
/// below resolves in the player's own `L2.eng` proves that the file has *a*
/// string there — it would have passed on the armoury's group 16. What makes
/// these right is the second half, done here: every index below was read out of
/// the shipped `L2.eng` and matched by its **words** to the row the painter
/// draws it on, and the four groups are complete (73 has twelve strings and the
/// painter draws all twelve; 85 has ten and draws all ten; 86 has five and
/// draws four; 87 has twelve and draws seven).
#[derive(Clone, Copy)]
pub struct Line {
    pub group: usize,
    pub index: usize,
    /// Upper case because our fallback font has no lower case.
    pub ours: &'static str,
}

const fn line(group: usize, index: usize, ours: &'static str) -> Line {
    Line { group, index, ours }
}

/// `L2.eng` group 8 index 0/1 — *"Crown."* / *"Crowns."*, which the tax panel's
/// `Ui_DrawCount(taxShown, 0, …)` picks between.
const CROWN_NOUN: usize = 0;
/// `L2.eng` group 20, indexed by county `+0x09` — the five health words.
const GROUP_HEALTH_BANDS: usize = 20;
/// `L2.eng` group 21, indexed by county `+0x15D`/`+0x15E` — the six ration
/// levels, *None* … *Triple*.
const GROUP_RATION_LEVELS: usize = 21;

/// `L2.eng` group 73 — the population panel, `Panel_Population`.
mod g73 {
    use super::{line, Line};
    pub const GROUP: usize = 73;
    pub const TITLE: Line = line(GROUP, 0, "POPULATION IN");
    pub const LAST: Line = line(GROUP, 1, "LAST SEASON");
    pub const BIRTHS: Line = line(GROUP, 2, "BIRTHS");
    pub const DEATHS: Line = line(GROUP, 3, "DEATHS");
    pub const ARMY: Line = line(GROUP, 4, "ARMY");
    pub const EMIGRANTS: Line = line(GROUP, 5, "EMIGRANTS TO");
    pub const IMMIGRANTS: Line = line(GROUP, 6, "TOTAL IMMIGRANTS");
    pub const THIS: Line = line(GROUP, 7, "THIS SEASON");
    pub const GREATEST: Line = line(GROUP, 8, "GREATEST POPULATION");
    /// Drawn **after** the graph's peak: *"Greatest population" 456 "people."*
    #[allow(dead_code, reason = "named because it is not drawn; the graph has no peak here")]
    pub const PEOPLE: Line = line(GROUP, 9, "PEOPLE.");
    pub const NO_EMIGRATION: Line = line(GROUP, 10, "NO EMIGRATION.");
    pub const NO_IMMIGRATION: Line = line(GROUP, 11, "NO IMMIGRATION.");
}

/// `L2.eng` group 85 — the happiness panel, `Panel_Happiness`.
mod g85 {
    use super::{line, Line};
    pub const GROUP: usize = 85;
    pub const TITLE: Line = line(GROUP, 0, "HAPPINESS IN");
    pub const LAST: Line = line(GROUP, 1, "LAST SEASON");
    pub const FROM_TAXES: Line = line(GROUP, 2, "FROM TAXES");
    pub const FROM_RATION: Line = line(GROUP, 3, "FROM RATION");
    pub const FROM_HEALTH: Line = line(GROUP, 4, "FROM HEALTH");
    pub const FROM_ARMY: Line = line(GROUP, 5, "FROM ARMY");
    pub const FROM_ALE: Line = line(GROUP, 6, "FROM ALE");
    pub const THIS: Line = line(GROUP, 7, "THIS SEASON");
    pub const AVERAGE: Line = line(GROUP, 8, "AVERAGE HAPPINESS");
    pub const FROM_EVENTS: Line = line(GROUP, 9, "FROM EVENTS");
}

/// `L2.eng` group 86 — the tax panel, `Panel_Tax`.
mod g86 {
    use super::{line, Line};
    pub const GROUP: usize = 86;
    /// **Index 0, *"Tax in"*, is drawn by nothing.** The painter's four
    /// `Eng_DrawString` sites pass 1, 2, 3 and 4, and those four are the only
    /// literal `(0x56,` in the whole corpus. It is the tax panel's counterpart
    /// of the court's *"Arms"* and of 31/21 *"Morale"*: a caption left in the
    /// file. Named so the next reader does not go looking, and **never passed
    /// to [`Pen::eng`](crate::shell::Pen::eng)** — drawing it is what this file
    /// used to do wrong.
    #[allow(dead_code, reason = "named precisely so that it is never drawn")]
    pub const TITLE_NEVER_DRAWN: Line = line(GROUP, 0, "TAX IN");
    pub const RATE: Line = line(GROUP, 1, "TAX RATE");
    pub const PEOPLE_PAY: Line = line(GROUP, 2, "PEOPLE PAY");
    pub const THIS_COUNTY: Line = line(GROUP, 3, "THIS COUNTY");
    pub const OTHER_COUNTIES: Line = line(GROUP, 4, "OTHER COUNTIES");
}

/// `L2.eng` group 87 — the ration panel, `Panel_Ration`, and these strings are
/// the **fallback** now: the panel draws the player's own file through [`eng`]
/// and falls back to these only where the install has nothing.
///
/// The reason is countable.
/// stylistic: `Panel_Ration` (`0x00411B72`) is the **only consumer of group 87
/// in the whole binary** — enumerated, not assumed — and it draws seven of the
/// group's twelve strings. **A group with one consumer *is* that screen's
/// vocabulary**, so reading the painter without reading the group is reading
/// half the function; this panel had its *numbers* read out of the
/// decompilation and its *words* written by us.
///
/// These constants stay because an install with no `L2.eng` still has to draw
/// something, and because they say what each index *is* without a lookup.
mod g87 {
    use super::{line, Line};
    pub const GROUP: usize = 87;
    pub const TITLE: Line = line(GROUP, 0, "RATION");
    pub const WANTED: Line = line(GROUP, 1, "WANTED:");
    pub const ACHIEVED: Line = line(GROUP, 2, "ACHIEVED:");
    pub const HEALTH: Line = line(GROUP, 3, "HEALTH:");
    pub const EATEN: Line = line(GROUP, 4, "EATEN");
    pub const FED: Line = line(GROUP, 5, "FED");
    /// The *Armies Eat* line, drawn only when that option is on.
    pub const FORAGING: Line = line(GROUP, 8, "MEN FORAGING IN THE COUNTY.");
    /// **Indices 6, 7, 9, 10 and 11 — *"Feeds"*, *"Feeds"*, *"growing"*,
    /// *"harvested"*, *"planted"* — have no literal call site anywhere in the
    /// corpus.** Whether some caller reaches them through a computed group is
    /// not established; `Msg_DrawWindow` takes its group from a table. Recorded
    ///
    #[allow(dead_code, reason = "an inventory of absences, asserted in tests")]
    pub const UNDRAWN: [usize; 5] = [6, 7, 9, 10, 11];
}

/// `L2.eng` group 61 — the county strip's two captions, which are **not** the
/// tax panel's group and used to be filed as though they were.
mod g61 {
    use super::{line, Line};
    pub const GROUP: usize = 61;
    pub const STRIP_TAX: Line = line(GROUP, 0, "TAX");
    pub const STRIP_RATION: Line = line(GROUP, 1, "RATION");
}

pub struct CountyScreen {
    county: u8,
    panel: Panel,
    /// The ration slider is being dragged: the left button went down inside it
    /// and has not come up. `Ration_SliderClick` fires on `g_mouseLeftDown &&
    /// g_mouseInputChanged`, so a
    /// screen driven by discrete events needs to remember the first half.
    slider_held: bool,
    /// The two arrows' press timer and auto-repeat counter — `+0x0D` and
    /// `+0x0E` of `g_taxWidgets` / `g_rationWidgets`. See [`CountyScreen::arrows`].
    press: Press,
}

/// Geometry tests. The canvas tests that read numbers back off the pixels live
/// in `tests/screens_county.rs`, because they need the shipped install.
///
/// **Nine mutations were checked against these and those**, each turning
/// exactly one test red and no others. The last three are this audit's, and
/// every literal in the assertion is pinned from the decompilation
/// computed from the constant it is about — which is the trap `docs/agents.md`
/// records: *ablating a constant while computing your probe from that same
/// constant tests nothing at all*.
///
/// | mutation | test that went red |
/// |---|---|
/// | `MAX_TAX_RATE` 50 → 51 | `the_tax_rate_stops_at_the_originals_own_ceiling_of_fifty` |
/// | `delta_value`'s zero guard removed | `a_zero_delta_row_draws_its_label_and_no_number` |
/// | the value column right-anchored again | `a_zero_delta_row_draws_its_label_and_no_number` |
/// | `SLIDER_CTRL_Y` 220 → 216 | `the_split_sliders_controls_are_four_pixels_below_its_knob` |
/// | `JobScreen::ok_button`'s blacksmith arm removed | `the_blacksmith_is_the_one_job_that_is_not_this_window` |
/// | `HOT_X_LEFT_END` 548 → 560 | `the_strip_quadrants_fit_the_plate_and_leave_the_thermometer_unclickable` |
/// | `Chrome::load` preferring `System2.pl8` | `the_ration_split_slider_sets_the_field_the_original_sets` |
/// | the population panel's first row y 266 → 267 | `the_population_panel_opens_from_its_own_quadrant_and_lays_out_where_it_should` |
/// | the strip's population x 508 → 509 | `the_county_strip_shows_the_saves_numbers_where_the_original_puts_them` |
#[cfg(test)]
mod tests {
    use super::*;

    fn inside(outer: Rect, inner: Rect) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.w <= outer.x + outer.w
            && inner.y + inner.h <= outer.y + outer.h
    }

    /// The four windows are the four `Ui_DrawBox` calls, and each holds its own
    /// corner picture. A window that ran off the screen, or a corner outside
    /// it, would mean a cell count or a button coordinate was misread.
    #[test]
    fn every_panel_window_is_on_screen_and_holds_its_own_ok_button() {
        for p in PANELS {
            let w = p.window();
            assert!(w.x >= 0 && w.y >= 0, "{p:?} starts on screen");
            assert!(w.x + w.w <= 640 && w.y + w.h <= 480, "{p:?} ends on screen: {w:?}");
            assert!(inside(w, p.ok_button()), "{p:?}: the corner is outside its window {w:?}");
        }
    }

    /// The two order panels' arrows sit inside their own windows, on one line,
    /// with the up arrow to the *left* of the down arrow; the two panels that
    /// only report have none.
    ///
    /// The two pairs are spaced differently and that is the original's, not a
    /// slip: `g_taxWidgets` puts them at x 192 and 224 — a frame is 24 wide, so
    /// eight pixels apart — and `g_rationWidgets` at 344 and 368, flush.
    #[test]
    fn only_the_two_order_panels_have_arrows_and_both_pairs_are_inside_them() {
        for p in PANELS {
            match p {
                Panel::Tax | Panel::Ration => {
                    let up = p.increase_button().expect("an up arrow");
                    let down = p.decrease_button().expect("a down arrow");
                    assert!(inside(p.window(), up), "{p:?} up arrow {up:?} escapes its window");
                    assert!(inside(p.window(), down), "{p:?} down arrow escapes its window");
                    assert!(up.x < down.x, "{p:?}: up is the left of the pair");
                    assert_eq!(up.y, down.y, "{p:?}: and they are on one line");
                    assert!(
                        down.x - up.x >= system::ARROW,
                        "{p:?}: the pair may touch but must not overlap"
                    );
                }
                _ => {
                    assert!(p.increase_button().is_none(), "{p:?} takes no orders");
                    assert!(p.decrease_button().is_none(), "{p:?} takes no orders");
                }
            }
        }
        assert_eq!(
            Panel::Tax.decrease_button().unwrap().x - Panel::Tax.increase_button().unwrap().x,
            32,
            "g_taxWidgets records 0 and 1 are at x 192 and 224"
        );
        assert_eq!(
            Panel::Ration.decrease_button().unwrap().x
                - Panel::Ration.increase_button().unwrap().x,
            24,
            "g_rationWidgets records 0 and 1 are at x 344 and 368, flush"
        );
    }

    /// `CountyStrip_Click`'s four quadrants do not overlap, all four sit inside
    /// the 162 x 94 plate, and the gap between the two columns is exactly where
/// the health thermometer is drawn — so the gap exists.
    #[test]
    fn the_strip_quadrants_fit_the_plate_and_leave_the_thermometer_unclickable() {
        let rects: Vec<Rect> = PANELS.iter().map(|p| p.strip_hotspot()).collect();
        for (i, a) in rects.iter().enumerate() {
            assert!(inside(STRIP, *a), "quadrant {i} {a:?} escapes the plate {STRIP:?}");
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                let over =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!over, "quadrants {i} and {j} overlap: {a:?} {b:?}");
            }
        }
        for x in THERMOMETER.0..THERMOMETER.0 + THERMOMETER_W {
            for r in &rects {
                assert!(!r.contains(x, 200), "thermometer column {x} is clickable in {r:?}");
            }
        }
        // And the four between them cover both columns and both rows, so no
        // quadrant is a degenerate strip.
        for r in &rects {
            assert!(r.w > 40 && r.h > 20, "{r:?} is too small to click");
        }
    }

    /// The slider closes: the caps meet the track with no gap and no overlap,
    /// and the knob's travel is the track's width.
    #[test]
    fn the_split_sliders_caps_track_and_knob_travel_agree() {
        assert_eq!(SLIDER_CAP_LEFT_X + system::SLIDER_CAP, SLIDER_TRACK_X, "left cap meets track");
        assert_eq!(SLIDER_TRACK_X + SLIDER_TRACK_W, SLIDER_CAP_RIGHT_X, "track meets right cap");
        let centre = |v: i32| SLIDER_KNOB_ORIGIN + v + system::SLIDER_KNOB_W / 2;
        assert_eq!(centre(0), SLIDER_TRACK_X + 1, "at 0 the knob centres on the track's start");
        assert_eq!(
            centre(MAX_RATION_SPLIT),
            SLIDER_TRACK_X + SLIDER_TRACK_W + 1,
            "at 100 it centres on the track's end"
        );
        assert!(inside(Panel::Ration.window(), split_track()), "the track is inside the panel");
    }

    /// No panel window reaches the county strip, which is what lets the click
    /// handler test the strip's quadrants before the panel's own widgets.
    #[test]
    fn no_panel_window_reaches_the_county_strip() {
        for p in PANELS {
            let w = p.window();
            assert!(
                w.x + w.w <= STRIP.x,
                "{p:?} at {w:?} overlaps the strip column at x = {}",
                STRIP.x
            );
        }
    }

    /// **The split slider's two y's**, which were one until this audit.
    ///
    /// `Panel_RationSlider` draws the knob at `0xD8` and the caps at `0xDC`;
    /// `Ration_SliderClick` (`0x0043A379`) hit-tests `(200, 0xDC, 24, 24)`,
    /// `(0x145, 0xDC, 24, 24)` and `(0xE0, 0xDC, 0x66, 24)`. The literals here
/// are pinned from the decompilation
    /// constants, which is the difference between a test and a restatement.
    #[test]
    fn the_split_sliders_controls_are_four_pixels_below_its_knob() {
        assert_eq!(SLIDER_KNOB_Y, 0xD8, "Pl8_DrawFrame(System, 0x4C, …, 0xD8)");
        assert_eq!(SLIDER_CTRL_Y, 0xDC, "Rect_Contains(200, 0xDC, 0x18, 0x18)");
        for r in [split_down_button(), split_up_button(), split_track()] {
            assert_eq!(r.y, 0xDC, "every hit box in Ration_SliderClick is at 0xDC");
            assert_eq!(r.h, 0x18);
        }
        assert_eq!(split_down_button().x, 200);
        assert_eq!(split_up_button().x, 0x145, "0x145, one pixel right of the cap");
        assert_eq!(split_track().x, 0xE0);
        assert_eq!(split_track().w, 0x66);
    }

    /// **`Armies Eat` makes the ration panel two cells taller and moves its
    /// corner picture 32 pixels down.** `Panel_Ration` opens
    /// `rows = g_optArmiesEat ? 2 : 0`, draws `Ui_DrawBox(0x80, 0x60, 0x12,
    /// rows + 0xF)` and ends with `Ui_OkButton(0x184, (rows + 0xF) * 0x10 +
    /// 0x44, 0)`.
    #[test]
    fn armies_eat_is_the_one_option_that_changes_a_panels_shape() {
        assert_eq!(Panel::Ration.box_cells_for(false), (0x80, 0x60, 0x12, 0x0F));
        assert_eq!(Panel::Ration.box_cells_for(true), (0x80, 0x60, 0x12, 0x11));
        assert_eq!(Panel::Ration.ok_button_for(false).y, 0x0F * 0x10 + 0x44);
        assert_eq!(Panel::Ration.ok_button_for(true).y, 0x11 * 0x10 + 0x44);
        // And no other panel notices.
        for p in [Panel::Population, Panel::Happiness, Panel::Tax] {
            assert_eq!(p.box_cells_for(false), p.box_cells_for(true), "{p:?}");
            assert_eq!(p.ok_button_for(false), p.ok_button_for(true), "{p:?}");
        }
    }

    /// **`Ui_DrawNumberRight` centres, and the artwork is the proof.**
    ///
    /// The three "Fed" columns are drawn with width `0x40` from x 208, 266 and
    /// 324; the three icons above them are `Misc_cty` frames `0x21` (36 wide),
    /// `0x26` (37) and `0x2A` (23) at x 224, 284 and 344. Centred, each number
    /// lands within a few pixels of its own picture. Right-aligned — which is
    /// what `docs/symbols.json` calls the function and what
    /// `docs/screens-county.md` §5.4 repeats — every number would *end* left of
    /// every icon, which is the assertion below.
    #[test]
    fn the_ration_panels_numbers_centre_under_their_icons() {
        const ICON_X: [i32; 3] = [224, 284, 344];
        const ICON_W: [i32; 3] = [36, 37, 23];
        for i in 0..3 {
            let centre = FOOD_COL_X[i] + FOOD_COL_W / 2;
            let icon_centre = ICON_X[i] + ICON_W[i] / 2;
            assert!(
                (centre - icon_centre).abs() <= 4,
                "column {i}: number centres at {centre}, icon at {icon_centre}"
            );
            // The falsification: right-alignment would put the number's right
            // edge at FOOD_COL_X[i], left of the icon's own left edge.
            assert!(FOOD_COL_X[i] < ICON_X[i], "column {i} would be left of its icon");
        }
    }

    /// The five `L2.eng` groups these panels draw, and the indices that are
    /// **not** drawn — because a constant that names an absence is the only
    /// thing that stops the next reader re-discovering it.
    #[test]
    fn the_groups_are_the_painters_literal_arguments() {
        assert_eq!((g73::GROUP, g85::GROUP, g86::GROUP, g87::GROUP), (73, 85, 86, 87));
        assert_eq!(g61::GROUP, 61, "the strip's captions are group 61, not the tax panel's");
        assert_eq!((GROUP_HEALTH_BANDS, GROUP_RATION_LEVELS), (20, 21));
        assert_eq!(CROWN_NOUN, 0, "L2.eng 8/0 and 8/1, Crown. / Crowns.");

        // 86/0 "Tax in" has no call site in the whole corpus; the painter's
        // four are 1, 2, 3 and 4.
        assert_eq!(g86::TITLE_NEVER_DRAWN.index, 0);
        for l in [g86::RATE, g86::PEOPLE_PAY, g86::THIS_COUNTY, g86::OTHER_COUNTIES] {
            assert_ne!(l.index, g86::TITLE_NEVER_DRAWN.index);
        }
        // 73/9 "people." belongs to the graph's peak, which needs a history
        // array we do not have; it is named and not drawn.
        assert_eq!(g73::PEOPLE.index, 9);
        // And the five ration strings nothing reaches by a literal group.
        for i in g87::UNDRAWN {
            for l in [g87::TITLE, g87::WANTED, g87::ACHIEVED, g87::HEALTH, g87::EATEN, g87::FED] {
                assert_ne!(l.index, i, "87/{i} is in UNDRAWN and also drawn");
            }
            assert_ne!(g87::FORAGING.index, i);
        }
    }
}

