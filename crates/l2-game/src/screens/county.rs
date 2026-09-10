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
//! bottom-left and rations from bottom-right, and there is no other way into
//! any of them. Each is a floating `Ui_DrawBox` window over whatever was
//! underneath, and each has **two ways out**: the 24 × 24 picture in its
//! bottom-right corner, which is a live hotspot, and the **right mouse button**
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
//! # Two things this file used to get wrong about alignment, and both are the
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
//!   width `0x40`, and the proof is in the artwork rather than in the C: the
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
//!   (`0x004156A7`) was never read until this audit; here is all of it.
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
//!   one, and what is missing is now written down rather than merely absent.
//! * **The two years under the graph.** `Ui_DrawYear(DAT_00553228, …)` prints
//!   the year at the head of the history window; with no history there is no
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

use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};

use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
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

// ----------------------------------------------------------------- geometry
//
// Every rectangle below is `Ui_DrawBox(x, y, cols, rows)` out of the panel's
// own painter, in cells of 16 pixels. `docs/screens-county.md` §5.

/// `Panel_Population` (`0x004110B1`): `Ui_DrawBox(0x10, 0x30, 0x1C, 0x17)`.
const POPULATION_BOX: (i32, i32, i32, i32) = (16, 48, 28, 23);
/// `Panel_Happiness` (`0x004116FB`): `Ui_DrawBox(0x10, 0x30, 0x1C, 0x18)`.
const HAPPINESS_BOX: (i32, i32, i32, i32) = (16, 48, 28, 24);
/// `Panel_Tax` (`0x0041152F`): `Ui_DrawBox(0x50, 0x90, 0x14, 0x09)`.
const TAX_BOX: (i32, i32, i32, i32) = (80, 144, 20, 9);
/// `Panel_Ration` (`0x00411B72`): `Ui_DrawBox(0x80, 0x60, 0x12, 0x0F)`. The
/// height is 17 cells with *Armies Eat* on; we always draw the short one,
/// because we never draw the foraging line it makes room for.
const RATION_BOX: (i32, i32, i32, i32) = (128, 96, 18, 15);

/// `Ui_HistoryGraph(0x20, 0x54, mode)` opens with
/// `Ui_DrawInsetRect(x, y, 0x192, 0x9B)` — 402 × 155 at (32, 84) — and both
/// graph panels call it with the same origin.
const GRAPH: Rect = Rect::new(32, 84, 402, 155);

/// The county strip's own rectangle: `Misc_cty` frame 55 is 162 × 94 and
/// `Screen_DrawCampaign` draws it at (478, 156).
pub const STRIP: Rect = Rect::new(478, chrome::PANEL_MIDDLE_Y, 162, 94);

/// `CountyStrip_Click`'s outer guard, made half-open: the original tests
/// `mx > 487 && mx < 630` and `my > 181 && my < 241`.
const HOT_X0: i32 = 488;
const HOT_X1: i32 = 630;
const HOT_Y0: i32 = 182;
const HOT_Y1: i32 = 241;
/// Its two splits. `548 … 567` is a dead band 19 pixels wide, and the health
/// thermometer — 14 wide, drawn at x = 552 — sits inside it with two pixels
/// spare on each side. The gap exists for the bar.
const HOT_X_LEFT_END: i32 = 548;
const HOT_X_RIGHT_START: i32 = 568;
const HOT_Y_SPLIT: i32 = 212;

/// `Pl8_DrawFrameHere(g_miscCtySheet, healthBand + 0x46, 0x228, 0xB5)`.
const THERMOMETER: (i32, i32) = (552, 181);
const THERMOMETER_FRAME: usize = 0x46;
const THERMOMETER_W: i32 = 14;

/// The county strip's three farming rows, by labour slot, as
/// (plain, below the floor, **above the ceiling — the blue ring**).
///
/// **`[V]`.** Every drawer in `CountyStrip_Draw`'s left-hand row loop asks the
/// same question first: `if (county.labour[slot].useful <
/// county.labour[slot].workers)` — *more people on this job than it can use* —
/// and draws the ringed frame two pixels up and left of where the plain one
/// goes. That is the signal the player described as *"a blue outline for idle
/// peasants (eg too many on dairy)"*, and dairy is the second row.
///
/// A `None` in the middle column is a job whose drawer has no shortfall frame
/// at all: `FUN_004103C5` tests only the ceiling. The frame numbers themselves
/// are [`misc_cty::RINGED_PAIRS`](l2_view::chrome::misc_cty::RINGED_PAIRS).
///
/// | slot | drawer | job |
/// |---|---|---|
/// | 0 | `FUN_0041023A` | grain farming |
/// | 1 | `FUN_004100AF` | cattle farming — the dairy |
/// | 2 | `FUN_004103C5` | field reclamation |
const PRODUCE_ICONS: [(usize, Option<usize>, usize); 3] = [
    (misc_cty::RINGED_PAIRS[0].0, Some(misc_cty::SHORTFALL_GRAIN), misc_cty::RINGED_PAIRS[0].1),
    (misc_cty::RINGED_PAIRS[1].0, Some(misc_cty::SHORTFALL_CATTLE), misc_cty::RINGED_PAIRS[1].1),
    (misc_cty::RINGED_PAIRS[2].0, None, misc_cty::RINGED_PAIRS[2].1),
];

/// Both graph panels put their labels at x = 48 (`0x30`) and **start** their
/// values at x = 336 (`0x150`).
///
/// **Not right-anchored.** `Ui_DrawNumber` and `Ui_DrawDelta` both end in a
/// plain `Ui_DrawText(buffer, x, …)` and nothing measures the string; the `'@'`
/// lead is a zero-width glyph that reserves the sign column so a `+7` and a `7`
/// line up, which is what makes the column *look* right-anchored in a
/// screenshot. `docs/screens-county.md` §5.1 says *"values right-anchored from
/// x = 336"* and it is wrong.
const LABEL_X: i32 = 48;
const VALUE_LEFT: i32 = 336;

/// `Panel_Ration`'s three food columns: `Ui_DrawNumberRight(…, x, y, 0x40, …)`,
/// which **centres** the number in those 64 pixels. Grain, cattle and the third
/// source, under the icon row at (224, 256), (284, 256) and (344, 256).
const FOOD_COL_X: [i32; 3] = [0xD0, 0x10A, 0x144];
const FOOD_COL_W: i32 = 0x40;

impl Panel {
    /// The panel's window in pixels — the rectangle `Ui_DrawBox` covers.
    pub fn window(self) -> Rect {
        let (x, y, cols, rows) = self.box_cells();
        Rect::new(x, y, cols * 16, rows * 16)
    }

    fn box_cells(self) -> (i32, i32, i32, i32) {
        self.box_cells_for(false)
    }

    /// The same with *Armies Eat* known, which is the one option that changes a
    /// panel's shape: `Panel_Ration` opens
    /// `rows = g_optArmiesEat == 1 ? 2 : 0; Ui_DrawBox(0x80, 0x60, 0x12, rows + 0xF)`,
    /// making room for the foraging line at y = 336. Everything else ignores it.
    fn box_cells_for(self, armies_eat: bool) -> (i32, i32, i32, i32) {
        match self {
            Panel::Population => POPULATION_BOX,
            Panel::Happiness => HAPPINESS_BOX,
            Panel::Tax => TAX_BOX,
            Panel::Ration => {
                let (x, y, cols, rows) = RATION_BOX;
                (x, y, cols, if armies_eat { rows + 2 } else { rows })
            }
        }
    }

    /// The graph area, for the two panels that have one.
    pub fn graph_rect(self) -> Option<Rect> {
        matches!(self, Panel::Population | Panel::Happiness).then_some(GRAPH)
    }

    /// `Ui_OkButton(x, y, 0)` — the 24 × 24 picture in the panel's own corner,
    /// and the box `Ui_OkButtonClicked` (`0x0040E7E4`) hit-tests on a left
    /// release.
    ///
    /// **It is not a tick.** `System.pl8` frame `0x33` decodes to a cursor
    /// arrow pointing into a small black hole: a close button whose artwork is
    /// the instruction. It is a real target *and* the right button closes the
    /// panel from anywhere (`docs/screens-county.md` §2.6), so the game offers
    /// two ways out and so do we. §2.7; the name `OK` is ours and is kept.
    pub fn ok_button(self) -> Rect {
        self.ok_button_for(false)
    }

    /// The same, with the ration panel's *Armies Eat* height applied — its
    /// corner is `Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)` and `rows` is the
    /// box's own, so turning the option on moves the button 32 pixels down.
    pub fn ok_button_for(self, armies_eat: bool) -> Rect {
        let (x, y) = match self {
            // Ui_OkButton(0x1B4, 0x184, 0)
            Panel::Population => (436, 388),
            // Ui_OkButton(0x1B4, 0x194, 0)
            Panel::Happiness => (436, 404),
            // Ui_OkButton(0x174, 0x104, 0)
            Panel::Tax => (372, 260),
            // Ui_OkButton(0x184, rows * 0x10 + 0x44, 0)
            Panel::Ration => {
                let (_, _, _, rows) = self.box_cells_for(armies_eat);
                (388, rows * 0x10 + 0x44)
            }
        };
        Rect::new(x, y, system::OK_DIM, system::OK_DIM)
    }

    /// The **up** arrow: record 0 of `g_taxWidgets` (`0x004DD790`) or of
    /// `g_rationWidgets` (`0x004DD7C0`). It is the left of the pair, which is
    /// the original's arrangement and not a transcription slip.
    pub fn increase_button(self) -> Option<Rect> {
        match self {
            Panel::Tax => Some(Rect::new(192, 162, system::ARROW, system::ARROW)),
            Panel::Ration => Some(Rect::new(344, 130, system::ARROW, system::ARROW)),
            _ => None,
        }
    }

    /// The **down** arrow: record 1 of the same table, 24 pixels to its right.
    pub fn decrease_button(self) -> Option<Rect> {
        match self {
            Panel::Tax => Some(Rect::new(224, 162, system::ARROW, system::ARROW)),
            Panel::Ration => Some(Rect::new(368, 130, system::ARROW, system::ARROW)),
            _ => None,
        }
    }

    /// Which quadrant of the county strip opens this panel.
    pub fn strip_hotspot(self) -> Rect {
        let (x, w) = match self {
            Panel::Population | Panel::Tax => (HOT_X0, HOT_X_LEFT_END - HOT_X0),
            Panel::Happiness | Panel::Ration => (HOT_X_RIGHT_START, HOT_X1 - HOT_X_RIGHT_START),
        };
        let (y, h) = match self {
            Panel::Population | Panel::Happiness => (HOT_Y0, HOT_Y_SPLIT - HOT_Y0),
            Panel::Tax | Panel::Ration => (HOT_Y_SPLIT, HOT_Y1 - HOT_Y_SPLIT),
        };
        Rect::new(x, y, w, h)
    }
}

// ------------------------------------------------- the produce rows, and
//                                                    the popup they open
//
// `FUN_0040FEC1` (`0x0040FEC1`) lays the 162 x 128 plate at y = 302 out into two
// columns, `CountyStrip_Draw` picks the two row pitches, and
// `CountyStrip_JobClick` (`0x00438E3B`) turns a press into a job popup. The
// three are one fact and live together here.

/// `CountyStrip_JobClick`'s hit box: `x 0x1DE … 0x27F, y 0x12E … 0x1AD`, and the
/// column split at `0x230`.
const JOBS_Y0: i32 = 0x12E;
const JOBS_Y1: i32 = 0x1AE;
const JOBS_SPLIT_X: i32 = 0x230;

/// **`FUN_0040FEC1`'s left-hand list**, verbatim and in its own order: cattle,
/// then grain, then reclamation — each entry the **labour slot** the row is
/// about, which is also what `CountyStrip_JobClick` writes into `g_jobPanelJob`
/// plus one.
pub fn farm_rows(c: &l2_kingdom::county::County) -> Vec<usize> {
    let mut rows = Vec::with_capacity(3);
    if c.fields_cattle != 0 || c.herd != 0 {
        rows.push(1);
    }
    if c.fields_grain != 0 || c.grain != 0 {
        rows.push(0);
    }
    if c.fields_reclaiming != 0 {
        rows.push(2);
    }
    rows
}

/// **`FUN_0040FEC1`'s right-hand list**, and its order is not the industry
/// array's: wood, iron, stone, weapons, then the castle.
///
/// Each of the four is gated on `industry[n].enabled` — county `+0x297 + n*0x18`,
/// the byte `Industry_ToggleFromMap` (`0x0043D309`) XORs when you click the
/// building on the campaign map — and the castle row on `castleDegraded`.
pub fn industry_rows(c: &l2_kingdom::county::County) -> Vec<usize> {
    let mut rows = Vec::with_capacity(5);
    for (industry, slot) in [(0usize, 6usize), (1, 4), (3, 5), (2, 7)] {
        if c.industry[industry].enabled {
            rows.push(slot);
        }
    }
    if c.castle_degraded != 0 {
        rows.push(3);
    }
    rows
}

/// `DAT_0053E970` and `DAT_0053F04C`, the two row pitches, which
/// `CountyStrip_Draw` picks from the two counts.
///
/// **They are not the same rule.** The farm column has two cases and the
/// industry column three, because the industry column can hold five rows:
///
/// ```c
/// DAT_0053E970 = farmRows     < 3 ? 0x3C : 0x2D;
/// DAT_0053F04C = industryRows < 3 ? 0x3C : industryRows < 4 ? 0x2D : 0x1E;
/// ```
pub fn farm_pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else {
        0x2D
    }
}

pub fn industry_pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else if rows < 4 {
        0x2D
    } else {
        0x1E
    }
}

/// **`CountyStrip_JobClick` (`0x00438E3B`)** — which labour slot a press in the
/// sidebar's lower plate opens the job popup for, or `None`.
///
/// The caller supplies the ownership gate, which is the function's own first
/// line: `if (g_counties[g_selectedCounty].owner == g_localPlayer)`. Everything
/// else is here, including the two ways it refuses — a row index past the end of
/// its list returns 0 rather than falling through to the other column.
pub fn job_row_at(c: &l2_kingdom::county::County, x: i32, y: i32) -> Option<usize> {
    if !(478..640).contains(&x) || !(JOBS_Y0..JOBS_Y1).contains(&y) {
        return None;
    }
    let (rows, pitch) = if x < JOBS_SPLIT_X {
        let rows = farm_rows(c);
        let pitch = farm_pitch(rows.len());
        (rows, pitch)
    } else {
        let rows = industry_rows(c);
        let pitch = industry_pitch(rows.len());
        (rows, pitch)
    };
    let n = ((y - JOBS_Y0) / pitch) as usize;
    rows.get(n).copied()
}

/// `CountyStrip_Click` (`0x00438CEB`) itself: which panel a pixel opens, or
/// `None` for the outer guard and for the thermometer's dead band.
///
/// **This is the whole navigation into the four panels** — §2.3 — so it lives
/// here as one function rather than as four rectangles the caller loops over,
/// and the campaign map and the county screen both call it.
pub fn panel_at(x: i32, y: i32) -> Option<Panel> {
    PANELS.into_iter().find(|p| p.strip_hotspot().contains(x, y))
}

// -------------------------------------------------- the ration split slider
//
// `Panel_RationSlider` (`0x00411FDE`) draws it and `Ration_SliderClick`
// (`0x0043A379`) hit-tests it. The caps are drawn at 200 and 324 and are 24
// wide, the track runs 224 … 323, and the knob is drawn at `220 + value`.

/// **Two y's, not one, and this file used to have one.** `Panel_RationSlider`
/// draws the knob at `0xD8` = 216 and both caps at `0xDC` = 220, and
/// `Ration_SliderClick` hit-tests all three boxes at `0xDC` — so a single
/// constant of 216 put every control four pixels above where the game has it,
/// drawn *and* clickable.
const SLIDER_KNOB_Y: i32 = 216;
const SLIDER_CTRL_Y: i32 = 220;
const SLIDER_CAP_LEFT_X: i32 = 200;
const SLIDER_CAP_RIGHT_X: i32 = 324;
const SLIDER_TRACK_X: i32 = 224;
const SLIDER_TRACK_W: i32 = 100;
const SLIDER_KNOB_ORIGIN: i32 = 220;
/// The three track colours, which are the original's own literal arguments:
/// `FUN_00403A8F(…, 0x10)` for the top line, `FUN_0040437D(…, 0x3F)` for the
/// 100 × 4 fill, `FUN_00403A8F(…, 0x1F)` for the bottom. They used to be
/// [`Ink`](l2_view::Ink) values of ours.
const TRACK_TOP: u8 = 0x10;
const TRACK_FILL: u8 = 0x3F;
const TRACK_BOTTOM: u8 = 0x1F;

/// `(200, 0xDC, 0x18, 0x18)` — steps the split down by one.
pub fn split_down_button() -> Rect {
    Rect::new(SLIDER_CAP_LEFT_X, SLIDER_CTRL_Y, system::SLIDER_CAP, system::SLIDER_CAP)
}

/// `(0x145, 0xDC, 0x18, 0x18)` — up by one. 0x145 is 325, one pixel right of
/// where the cap is drawn; that off-by-one is the original's.
pub fn split_up_button() -> Rect {
    Rect::new(SLIDER_CAP_RIGHT_X + 1, SLIDER_CTRL_Y, system::SLIDER_CAP, system::SLIDER_CAP)
}

/// `(0xE0, 0xDC, 0x66, 0x18)` — a click here jumps the split to `mouseX - 224`.
pub fn split_track() -> Rect {
    Rect::new(SLIDER_TRACK_X, SLIDER_CTRL_Y, 102, system::SLIDER_CAP)
}

// ----------------------------------------------------- the `L2.eng` strings
//
// One constant per `(group, index)` the four painters pass, carrying our own
// transcription as the fallback for a machine with no game.

/// One `L2.eng` reference: the group, the index the painter passes, and our
/// transcription of it.
///
/// **State the limit where the constant lives**, because a check on existence
/// is not a check on meaning. A coordinator check that every `(group, index)`
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

/// `L2.eng` group 87 — the ration panel, `Panel_Ration`.
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
    /// rather than guessed.
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

// ------------------------------------------- the sheet frames the panels draw
//
// `Misc_cty.pl8` in campaign mode — `Pen::misc_frame`, which is
// `Pl8_DrawFrame(g_miscCtySheet, …)`.

/// `Ui_DrawHappinessDelta`'s face, 20 × 18, and the three plain happiness
/// numbers on `Panel_Happiness` each get one too.
const FRAME_FACE: usize = 0x17;
/// `Panel_Ration`: an 8 × 18 glyph at (144, 282), left of *"Fed"*.
const FRAME_FED_MARK: usize = 0x18;
/// `Panel_Ration`: the grain sack, 36 × 27. Drawn twice — at the slider's left
/// end and over the first Fed/Eaten column.
const FRAME_GRAIN: usize = 0x21;
/// `Panel_Ration`: cattle, 37 × 24. Drawn twice, the same way.
const FRAME_CATTLE: usize = 0x26;
/// `Panel_Ration`: the third food column's icon, 23 × 19. `Misc_cty` frame
/// `0x2A` has not been decoded to a picture here; `docs/screens-county.md` §5.4
/// calls it sheep, which is **[D]** and not checked.
const FRAME_THIRD_FOOD: usize = 0x2A;
/// `Panel_Tax`: the vignette at (320, 160).
const FRAME_TAX_VIGNETTE: usize = 0x3E;
/// `Ui_DrawHappinessDelta` puts its closing bracket at `pen + 0x14`, which is
/// exactly [`FRAME_FACE`]'s width — so the face's box is 20 pixels wide whether
/// or not the sheet is there, and the bracket lands in the same place.
const FACE_W: i32 = 0x14;

// ---------------------------------------------------------------- the screen

pub struct CountyScreen {
    county: u8,
    panel: Panel,
    /// The ration slider is being dragged: the left button went down inside it
    /// and has not come up. `Ration_SliderClick` fires on `g_mouseLeftDown &&
    /// g_mouseInputChanged`, which is a held button and a moved pointer, so a
    /// screen driven by discrete events needs to remember the first half.
    slider_held: bool,
}

impl CountyScreen {
    /// Opens on the panel the strip quadrant that was clicked names, which is
    /// the only way the original opens any of them ([`panel_at`]).
    pub fn new(county: u8, panel: Panel) -> CountyScreen {
        CountyScreen { county, panel, slider_held: false }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    pub fn panel(&self) -> Panel {
        self.panel
    }

    pub fn open(&mut self, panel: Panel) {
        self.panel = panel;
    }

    fn panel_index(&self) -> usize {
        PANELS.iter().position(|&p| p == self.panel).unwrap_or(0)
    }

    /// Move the open panel's own value by `step`. Silently refused for a county
    /// the player does not hold, and there is nothing to move on the two panels
    /// that only report.
    fn adjust(&self, ctx: &mut Ctx, step: i32) {
        let id = self.county;
        let Some(c) = ctx.game.kingdom.counties.get(id as usize) else { return };
        match self.panel {
            Panel::Tax => {
                let next = (c.tax_rate + step).clamp(0, MAX_TAX_RATE);
                ctx.game.set_tax_rate(id, next);
            }
            Panel::Ration => {
                let next = (c.ration_wanted + step).clamp(0, RATION_LEVEL_COUNT as i32 - 1);
                ctx.game.set_ration(id, next);
            }
            Panel::Population | Panel::Happiness => {}
        }
    }

    /// **`Ration_SliderClick` (`0x0043A379`) — and it is a drag, not a click.**
    ///
    /// ```c
    /// if (counties[sel].owner != g_localPlayer) return 0;
    /// if (g_mouseLeftReleased) return 0;                    /* the release does nothing */
    /// if (!g_mouseLeftDoubleClick && !(g_mouseLeftDown && g_mouseInputChanged)) return 0;
    /// ```
    ///
    /// So it fires **while the button is held and the pointer has moved**, and
    /// the release is explicitly ignored. `held` is that condition; the caller
    /// passes it for both a press and a drag, which is what makes the thumb
    /// follow the cursor instead of jumping once per click.
    ///
    /// The two gestures do not end the same way, and `g_uiHotspotArg` is what
    /// tells them apart: **1 for a jump on the track, 0 for an arrow.** A track
    /// jump that changes nothing springs back; an arrow keeps walking. The rule
    /// is [`l2_kingdom::Kingdom::set_ration_split`] and the flag is `sweep`
    /// there.
    ///
    /// One more thing the original does that reads oddly and is deliberate:
    /// the arrows only step on a **press** (`g_mouseLeftPressed ||
    /// g_mouseLeftDoubleClick`), so holding the button down on an arrow and
    /// wiggling does not repeat — but holding it on the **track** does, because
    /// the track branch reads `mouseX` every frame.
    fn split_click(&self, ctx: &mut Ctx, x: i32, y: i32, pressed: bool) -> bool {
        if self.panel != Panel::Ration {
            return false;
        }
        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return false };
        let current = c.ration_split;
        // The track is read on every frame of the drag; the arrows step only on
        // the press that started it.
        let (next, sweep) = if split_track().contains(x, y) {
            (x - SLIDER_TRACK_X, true)
        } else if split_down_button().contains(x, y) {
            if !pressed {
                return true;
            }
            (current - 1, false)
        } else if split_up_button().contains(x, y) {
            if !pressed {
                return true;
            }
            (current + 1, false)
        } else {
            return false;
        };
        let next = next.clamp(0, MAX_RATION_SPLIT);
        // `if (rationSplit == local_10) return 1;` — the arm consumes the input
        // and does not re-run the food pass for a value that has not moved.
        if next != current {
            ctx.game.set_ration_split(self.county, next, sweep);
        }
        true
    }
}

impl Screen for CountyScreen {
    fn id(&self) -> ScreenId {
        ScreenId::County(self.county, self.panel)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("County {}", self.county)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **The whole right-hand column is live under an open panel**, and the
        // arm says so rather than leaving it to be inferred. `0x14`'s, verbatim:
        //
        // ```c
        // if (FUN_0043292D() == 0 && FUN_00432967() == 0 &&      /* modes, sidebar  */
        //     CountyStrip_Click() == 0 && FUN_00439122() == 0 && /* strip, split    */
        //     CountyStrip_JobClick() == 0 && FUN_00439079() == 0) {
        //   if (!rightReleased) { if (Ui_OkButtonClicked()) { g_screenId = 0; } }
        //   else                                            { g_screenId = 0; }
        // }
        // ```
        //
        // Six guards, every one of them hit-testing `x >= 0x1DE`, and every one
        // of them ahead of the panel's own two ways out. `0x15` and `0x16` are
        // the same six; `0x19` inserts `Ration_SliderClick` after the second,
        // which is why the split *below* is tested here and the column is not.
        //
        // Two things follow. Clicking the strip while a panel is open
        // **switches** panel rather than closing it — which is how a player goes
        // from population to tax without a trip via the map — and the five
        // sidebar buttons, the minimap, its four mode icons, the farm/industry
        // slider and the produce rows all keep working with a panel up. None of
        // that was reproduced: this screen tested the four strip quadrants
        // itself and swallowed everything else in the column.
        //
        // [`Transition::Pass`] is the whole of it, and it is the same mechanism
        // the village already used for the same six guards
        // (`docs/decisions.md` C59). The strip quadrants go down with the rest,
        // so the map's own `CountyStrip_Click` answers and its `Push` lands at
        // the map's depth — which is `g_screenId = 0x14` and not a second panel
        // stacked on the first.
        //
        // The six guards themselves are the map screen's and are recorded there;
        // the arm here is that *this screen's ladder runs them first*, which is
        // one predicate shared with the village — so one record, and it lives on
        // [`crate::screens::belongs_to_the_right_column`].
        if crate::screens::belongs_to_the_right_column(event) {
            return Transition::Pass;
        }
        match event {
            // **The slider is dragged.** `Ration_SliderClick` returns 0 on the
            // release and fires on `g_mouseLeftDown && g_mouseInputChanged` —
            // held and moved — so the thumb follows the cursor for as long as
            // the button is down. Ours was reachable only from a press, which
            // is the fourth place our input model differs from the original's
            // by *category* rather than by coordinate.
            //
            // The release ends the drag and does nothing else, exactly as the
            // first line of the original's ladder says.
            Event::Release { x, y } => {
                self.slider_held = false;
                // `Ui_OkButtonClicked` (`0x0040E7E4`) — the 24 x 24 corner
                // picture the panel's own `Ui_OkButton` call stashed, **on the
                // release**: its first statement is
                // `if (g_mouseLeftReleased == 0) return 0;`. Ours tested it on
                // the press, which is one of four such arms and the reason
                // `docs/arms.json` now records a gesture KIND rather than only
                // an arm's existence.
                //
                // **The BACK TO MAP button that used to be tested here is gone.**
                // It was ours, it was drawn at (478, 460), and that is the
                // original's **End Turn** strip to the pixel — record 5 of
                // `g_sidebarButtons`. So a rectangle of ours sat on top of a
                // live control of the game's. The column now passes down and
                // the strip ends the turn.
                // arm: 0x0040E7E4/panel-corner-closes left-release
                if self
                    .panel
                    .ok_button_for(ctx.game.kingdom.options.armies_eat)
                    .contains(x, y)
                {
                    return Transition::Pop;
                }
                return Transition::Stay;
            }
            Event::Pointer { x, y } if self.slider_held => {
                self.split_click(ctx, x, y, false);
                return Transition::Stay;
            }
            // **`Ui_DrawBox` panels are dismissed by the right button**, and the
            // game says so in its own words: `Screen_SliderBox` prints `L2.eng`
            // group 12 index 0, *"Click Right to Exit"*, under its caption.
            // `Screen_FrameInput`'s arm for each of `0x14`, `0x15`, `0x16` and
            // `0x19` is the same shape — the strip, the sidebar and the ration
            // slider are tested first, so a click on those *switches* panel
            // rather than closing it, and only then does a right release set
            // `g_screenId = 0`.
            //
            // It closes from *anywhere*, the strip included: every guard in
            // that chain tests a left press or a left release, so none of them
            // consumes a right one.
            //
            // arm: 0x0042FF10/panel-right-closes right-release
            Event::RightClick { .. } => return Transition::Pop,
            // **Ours, and counted.** The original has no keyboard route out of a
            // panel and none into another one: its only `VK_ESCAPE` handler
            // quits the game, and the four panels are four screen ids with no
            // ordering between them at all. Kept because a keyboard player has
            // nothing else, and recorded in `docs/arms.json` as an invention
            // rather than left as a comment admitting a choice — which is what
            // `docs/decisions.md` C61 found nine of.
            // arm: ours/county-panel-keyboard key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => return Transition::Pop,
            Event::KeyDown(Key::Up) => {
                self.panel = PANELS[(self.panel_index() + PANELS.len() - 1) % PANELS.len()];
            }
            Event::KeyDown(Key::Down) => {
                self.panel = PANELS[(self.panel_index() + 1) % PANELS.len()];
            }
            Event::KeyDown(Key::Left) => self.adjust(ctx, -1),
            Event::KeyDown(Key::Right) => self.adjust(ctx, 1),
            Event::Click { x, y } => {
                // `Ui_OkButtonClicked` (`0x0040E7E4`) — the 24 × 24 corner
                // picture the panel's own `Ui_OkButton` call stashed.
                //
                // **The BACK TO MAP button that used to be tested here is gone.**
                // It was ours, it was drawn at (478, 460), and that is the
                // original's **End Turn** strip to the pixel — record 5 of
                // `g_sidebarButtons`. So a rectangle of ours sat on top of a
                // live control of the game's, which is the same defect a player
                // reported about the five sidebar icons a fortnight ago. The
                // column now passes down and the strip ends the turn.
                // The strip's four quadrants are in the column and went down
                // with it, so nothing is tested for them here.
                // `Ration_SliderClick` (`0x0043A379`) — `0x19`'s own extra
                // guard, and the reason the ration panel's arm is one line
                // longer than the other three.
                // arm: 0x0043A379/ration-split-slider left-press
                if self.split_click(ctx, x, y, true) {
                    self.slider_held = true;
                    return Transition::Stay;
                }
                // `Screen_HandleInput`'s widget tables: `g_taxWidgets`
                // (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`), two
                // records each, up then down.
                // arm: 0x004BA9C8/tax-and-ration-arrows left-press
                if self.panel.increase_button().is_some_and(|r| r.contains(x, y)) {
                    self.adjust(ctx, 1);
                } else if self.panel.decrease_button().is_some_and(|r| r.contains(x, y)) {
                    self.adjust(ctx, -1);
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    /// **These are four windows over the campaign map**, which is what this
    /// module's own header has said all along — *"each is a floating
    /// `Ui_DrawBox` window over whatever was underneath"* — while `draw` went
    /// on clearing the screen and painting a flat ground where the map should
    /// be. The village's correction (`docs/decisions.md` C22) is the same
    /// mistake one screen along, and this is the other half of it.
    fn is_overlay(&self) -> bool {
        true
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        // **No clear.** The campaign map is the screen underneath on the stack.

        // **The seven column plates are no longer repainted here, and that is a
        // one-line change with a visible consequence.**
        //
        // This used to call `draw_right_column`, which puts `Misc_cty` frames 54
        // … 59 down over the whole column, y 24 … 480 — including the **minimap**
        // at (480, 25), the four mode icons beside it and the End Turn caption,
        // none of which this screen redraws. So *opening a county panel blanked
        // the minimap.* That was harmless while the column was dead under a
        // panel; now that every control in it is live it would be a control a
        // player can click and cannot see.
        //
        // The original does not repaint it either: `Screen_Draw`'s `0x14`,
        // `0x15`, `0x16` and `0x19` arms are `Panel_Population` and its three
        // siblings, and not one of them calls `CountyStrip_Draw` or
        // `Minimap_Draw`. The column is simply still there from the last frame —
        // §3.1's *"there is no screen clear anywhere in this engine"*.
        //
        // The **strip** is still drawn, and only because it is the one part of
        // the column that carries the focus outline (ours) and because the plate
        // it lands on is identical to the one the map screen drew a moment ago.
        draw_strip(ctx, canvas, self.county, Some(self.panel));
        self.draw_panel(ctx, canvas);

        // **The bottom strip is not ours to draw.** It is the campaign map's
        // End Turn button, the map screen is underneath on the stack and draws
        // it, and the column now passes clicks down to it. A BACK TO MAP
        // rectangle of ours used to be painted over it — and hit-tested ahead
        // of it — from right here.
    }
}

// -------------------------------------------------------- drawing the strip
//
// A free function, not a method, because **the campaign map draws it too**.
// `Screen_DrawCampaign` puts `CountyStrip_Draw` in the sidebar of the map
// itself; until now our map screen drew a box of our own numbers over the jobs
// plate below it and left this plate empty, which is the thing a player looks
// at every turn and the one place the original's own layout was going spare.

/// The one line of text the strip's font draws.
///
/// `CountyStrip_Draw` uses **`Fntl2_9.pl8`**, and it is the only caller of that
/// font in the whole game (`docs/screens-county.md` §2.1). We draw it where the
/// install has it and fall back to our own 5 × 7 font where it does not, so the
/// *layout* is the original's on every machine and the *letters* are only ours
/// on a machine with no game.
///
/// `colour` is a resolved palette index rather than one of `shell::font`'s
/// constants, because one of the rules here is a colour: the achieved ration is
/// **red when it differs from the wanted one**. That rule is the original's; the
/// index we spell red with is ours, out of [`Ink`].
fn strip_text(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    match ctx.assets.shell.small.as_ref() {
        Some(f) => {
            // `CountyStrip_Draw` sets `DAT_005AEA40 = 1` for the whole numeric
            // block and clears it after, and that global switches
            // `Ui_DrawText`'s emboss **off**. The strip's numbers are flat.
            let style = crate::shell::font::Style { colour, shadow: None, caps: None };
            f.draw(canvas, x, y, s, &style);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// The same, centred in `width` from `x` — `Ui_DrawCentred`, which clamps the
/// offset at zero rather than letting a long string start left of its box.
///
/// Public under a longer name because the End Turn caption is drawn in this
/// font too (`Screen_DrawEndTurn`), and it is the map screen that draws it.
pub fn strip_centred_at(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    s: &str,
    colour: u8,
) {
    strip_centred(ctx, canvas, x, y, width, s, colour)
}

fn strip_centred(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, width: i32, s: &str, colour: u8) {
    let w = match ctx.assets.shell.small.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    };
    strip_text(ctx, canvas, x + ((width - w) / 2).max(0), y, s, colour);
}

/// One line in the **body** font (`Fntl2_14.pl8`), centred in `width` from `x`.
///
/// The strip's own font is the 9-pixel one, but the county's name and the
/// three "sovereign land of …" lines are drawn with `g_fontBody`, embossed —
/// `DAT_005AEA40` is only set for the numeric block between them.
/// **This helper really does right-align, and it is OURS.** It is named after
/// `Ui_DrawNumberRight`, which does not: that function centres (C119), and the
/// resemblance is the name only. Kept because the produce rows were laid out
/// against it and changing the anchoring is a separate, visible decision.
///
/// **Flat, not embossed.** Each produce row sets `DAT_005AEA40 = 1` around its
/// number and clears it after — the same switch the strip's own figures are
/// drawn under — and that global turns `Ui_DrawText`'s emboss off.
/// **`Ui_DrawDelta` (`0x00402E0C`) — the produce rows' signed forecast.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative number."*
/// This is the routine that would have. The original, in full:
///
/// ```c
/// if (value == 0 && mode == 0) return;                     /* nothing at all */
/// Ui_DrawText(prefix, x, y, font, value < 0 ? colourNeg : colourPos);
/// if      (mode == 2) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else if (value < 0) Ui_DrawNumber(-value, '-', suffix, x + g_penAdvance, …, colourNeg);
/// else if (value < 1) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else                Ui_DrawNumber( value, '+', suffix, x + g_penAdvance, …, colourPos);
/// ```
///
/// Four things in it are worth having exactly, and three of them are the sort a
/// reimplementation drops without noticing:
///
/// * **The minus is a lead *character*, not a mark.** `Ui_DrawNumber` writes it
///   over `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves
///   free for a sign, so sign and digits go out in one `Ui_DrawText`. There is
///   no separate glyph to place or to lose.
/// * **A positive value carries an explicit `'+'`.** Only the *sign* tells the
///   player which way a forecast runs; the row has no other cue.
/// * **`mode == 0` and a value of zero draw nothing whatever.** All eight
///   produce rows pass mode 0. That is why an absent delta has read as a quiet
///   row rather than as an obvious hole — a county with nothing happening looks
///   the same either way.
/// * **The colour is the sign too**: `0xFA` positive, `0xF9` negative, at every
///   one of the eight call sites.
///
/// The prefix and the suffix are a single space at all eight — read out of
/// `Lords2.exe` at `0x004D3D40 … 0x004D3D84`, where the only one that is not
/// `" "` is the tax rate's `"%"`. They are drawn as two separate strings, so
/// [`TRAILING`](crate::shell::TRAILING)'s four pixels fall between
/// the prefix and the number and **not** between the number and its suffix.
/// Concatenating the three into one string would lose those four pixels, which
/// is the whole reason this is not a `format!`.
///
/// **Ours:** the font. The original uses `g_font10` — preload entry 5,
/// `font_10` — and this workspace loads `fntl2_9`, `fntl2_14` and `fntl2_22`
/// and not that one. The 9-pixel font is the closest we have and the row is a
/// pixel short because of it; loading `font_10` is its own job.
fn strip_delta(ctx: &Ctx, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    // `if ((value != 0) || (mode != 0))` — every produce row passes mode 0.
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { DELTA_POS };
    let lead = if value < 0 { '-' } else { '+' };
    // `Ui_DrawText(prefix, x, y, font, colour)`, then the number at
    // `x + g_penAdvance` — which is the prefix's width plus `Ui_DrawText`'s own
    // four trailing pixels, not the prefix's width alone.
    let prefix = " ";
    let advance = match ctx.assets.shell.small.as_ref() {
        Some(f) => f.width(prefix),
        None => text::width(prefix),
    } + crate::shell::TRAILING;
    strip_text(ctx, canvas, x, y, prefix, colour);
    // `Ui_DrawNumber(|value|, lead, suffix, …)` — one string, lead in slot 0.
    strip_text(ctx, canvas, x + advance, y, &format!("{lead}{} ", value.abs()), colour);
}

/// `Ui_DrawDelta`'s `colourPos`, the eighth argument at all eight produce-row
/// call sites.
const DELTA_POS: u8 = 0xFA;

/// `Ui_DrawDelta`'s `colourNeg`, the ninth. It is the same index
/// [`font::HIGHLIGHT`](crate::shell::font::HIGHLIGHT) carries and they are kept
/// apart on purpose: that one is *"this is the thing you are looking at"* and
/// this one is *"this number is negative"*, and a rename of either must not
/// drag the other.
const DELTA_NEG: u8 = 0xF9;

/// **`Ui_DrawNumberRight` (`0x004030C6`) centres.** It is not right-aligned and
/// it never was: its tail is `FUN_004025D7(buf, x, y, width, font, colour)`,
/// whose whole body is
///
/// ```c
/// Ui_DrawText(str, x + max(0, (width - Ui_TextWidth(str, font)) / 2), y, font, colour);
/// ```
///
/// The name is the original's shape rather than ours — `docs/symbols.json`'s
/// comment said *"Ui_DrawNumber, right-aligned inside width"* and that comment
/// is corrected on this branch. Two draw audits found it independently in the
/// same week, which is the usual sign that a name has been believed instead of
/// read.
///
/// It lands here: the produce rows' stock figure was anchored at x = 540 and
/// belongs centred between 480 and 540. This helper used to be `body_right` and
/// used to do that, which is the same defect the row's missing delta was
/// reported alongside — *"grain not shown as a negative"* and *"grain in the
/// wrong place"* would have looked like one complaint.
fn body_centred_in(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, w: i32, s: &str, colour: u8) {
    let style = crate::shell::font::Style { colour, shadow: None, caps: None };
    body_centred_styled(ctx, canvas, x, y, w, s, style);
}

/// Centred in `width` from `x`, with the emboss pair chosen by the caller — because
/// `CountyStrip_Draw` uses **two different ones** in the same plate. See
/// [`crate::shell::font::SHADOW_GREY`].
fn body_centred_styled(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    w: i32,
    s: &str,
    style: crate::shell::font::Style,
) {
    match ctx.assets.shell.body.as_ref() {
        Some(f) => {
            f.draw_centred(canvas, x, y, w, s, &style);
        }
        None => {
            text::draw_centred(canvas, x + w / 2, y, s, style.colour);
        }
    }
}

/// The county's name: `L2.eng` group 100, index `scenarioIndex * 20 + countyId`
/// — and `g_scenarioIndex` *is* the map slot ([`crate::game::Game::map_slot`]).
///
/// Falls back to `COUNTY n` for an install with no `L2.eng`, which is also what
/// the tests run against.
pub fn county_name(ctx: &Ctx, id: u8) -> String {
    let index = ctx.game.map_slot * 20 + id as usize;
    let name = ctx.assets.shell.text(100, index);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// One `L2.eng` string with a fallback, for the two the strip needs by name.
fn eng(ctx: &Ctx, group: usize, index: usize, fallback: &str) -> String {
    let s = ctx.assets.shell.text(group, index);
    if s.is_empty() {
        fallback.to_string()
    } else {
        s.to_string()
    }
}

/// The ration level's name — `L2.eng` group 21, which is what `CountyStrip_Draw`
/// indexes with county `+0x15D`.
fn ration_label(ctx: &Ctx, level: i32) -> String {
    let level = level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
    eng(ctx, GROUP_RATION_LEVELS, level as usize, ration_name(level))
}

/// **The county strip: what `CountyStrip_Draw` (`0x0040F7D3`) puts in the
/// 162 × 94 plate at (478, 156), at its own coordinates.**
///
/// `focus` outlines one quadrant. That outline is ours — the original's
/// quadrants are invisible because it is a mouse game — and it is drawn only
/// when a panel is actually open, so the map's sidebar carries none.
pub fn draw_strip(ctx: &Ctx, canvas: &mut Canvas, county: u8, focus: Option<Panel>) {
    let ink = &ctx.assets.ink;
    let Some(c) = ctx.game.kingdom.counties.get(county as usize) else { return };
    let mine = ctx.game.is_players(county);
    // **The strip's ink is the original's literal `0x3F`, which is black.**
    //
    // Every string `CountyStrip_Draw` writes — the county's name, its
    // population, its happiness, the tax rate, both group-61 captions — passes
    // colour `0x3F` to `Ui_DrawText`, and `0x3F` in `Base01.256` is
    // `rgb(0, 0, 0)`. The one exception is the achieved ration when it is not
    // the wanted one, which is `0xF9`.
    //
    // These were `ink.text`, which resolves to *white*, and a player reported
    // it: *"the text should be black not white over the happiness."* He is
    // right, and the reason [`Ink`](l2_view::Ink) is not the answer here is
    // that this text is written on the **original's own plate** — `Misc_cty`
    // frame `0x37` — so the index is a reading of the binary and not a choice
    // of ours. With no chrome loaded there is no plate to be black on, and the
    // fallback is our own ink.
    let strip_ink =
        if ctx.assets.chrome.is_some() { crate::shell::font::TEXT } else { ink.text };
    let strip_bad =
        if ctx.assets.chrome.is_some() { crate::shell::font::HIGHLIGHT } else { ink.bad };

    // `Ui_DrawCentred(100, scenarioIndex*0x14 + county, 0x1E0, 0xA5, 0xA0,
    // &g_fontBody, 0x3F)` — the county's name, centred across 160 pixels at
    // (480, 165), and **in the 14-pixel body font, not the strip's 9-pixel
    // one**. `docs/screens-county.md` §2.1 says "all of it in the 9-pixel
    // font"; the name is the exception, and `DAT_005AEA40` is set to 1 only
    // *after* it, so the name is embossed and the numbers below it are not.
    //
    // The unowned plate is 162 × 274 rather than 162 × 94 and puts the name
    // fifteen pixels lower, at `0xB4`; that is the original's own difference,
    // not a rounding of ours.
    //
    // **The emboss is the parchment pair on both plates, and over the cloudy
    // one that is a bug of the original's that we reproduce.** `Ui_DrawText`
    // picks its emboss from `g_screenId` and two globals, never from the
    // caller, and `CountyStrip_Draw` sets neither of them around this call —
    // so the name is drawn with `0x10`/`0x1F`, a dark olive over a pale
    // parchment yellow, whether it is standing on the parchment plate
    // (`Misc_cty` frame `0x37`) or on the cloudy one (frame `0x3A`). On the
    // cloudy plate the highlight is a colour that is not in the picture and
    // the name reads as though it were fading into paper that is not there.
    //
    // A player reported it and asked for it to be reproduced *and* switchable:
    // [`crate::game::Quirks::grey_county_name`] is the switch and it is off by
    // default. `docs/bugs.md` B64.
    let name = county_name(ctx, county);
    let name_y = if mine { 165 } else { 180 };
    let name_style = if ctx.assets.quirks.grey_county_name && !mine {
        crate::shell::font::Style {
            colour: strip_ink,
            shadow: Some(crate::shell::font::SHADOW_GREY),
            caps: None,
        }
    } else {
        crate::shell::font::Style::new(strip_ink)
    };
    body_centred_styled(ctx, canvas, 480, name_y, 160, &name, name_style);

    if !mine {
        // **`owner != 0` — and unclaimed land is a third case, not a second.**
        //
        // `CountyStrip_Draw`'s else-arm draws the cloudy plate and the name for
        // *any* county that is not yours, and then:
        //
        // ```c
        // if (g_counties[g_selectedCounty].owner != 0) {
        //   DAT_0058fe9c = 1;                                   /* the grey emboss */
        //   colour = g_realms[owner].field_0x8;
        //   Ui_DrawCentred(0xf, 0, 0x1e0, 0xf0,  0xa0, &g_fontBody, colour);
        //   Ui_DrawCentred(0xf, 1, 0x1e0, 0x104, 0xa0, &g_fontBody, colour);
        //   FUN_004025d7(&g_playerNames + owner * 0x2c, 0x1e0, 0x118, 0xa0, &g_fontBody, colour);
        //   DAT_0058fe9c = 0;
        // }
        // ```
        //
        // So a county nobody holds gets the plate and its name and **nothing
        // else** — no banner, no owner line. `L2.eng` group 15 is two strings,
        // `"Sovereign land"` and `"of"`, and the third line is a lord's name
        // out of `g_playerNames`; there is no wording in the file for an
        // unowned county because the original never needs one.
        //
        // We drew `SOVEREIGN LAND / OF / UNCLAIMED` here, which is a sentence
        // the original cannot produce. A player reported it in one line:
        // *"Unclaimed lands have no 'sovereign land of'."*
        if c.owner == 0 {
            return;
        }
        // The three lines carry the **grey** emboss — `DAT_0058FE9C = 1` —
        // and not the parchment one the name above them uses. Two emboss pairs
        // in one plate is the original's own arrangement, and ours had
        // collapsed them into one.
        //
        // **All three take the same pen**, and that is worth stating because it
        // is the natural place to expect a difference. `CountyStrip_Draw`
        // computes `colour` once and passes it to all of them — the banner, the
        // "of", and the lord's name — so the grey is the *emboss* and the
        // realm's colour is the *pen*, on every line:
        //
        // ```c
        // colour = g_realms[owner].field_0x8;
        // Ui_DrawCentred(0xf, 0, 0x1e0, 0xf0,  0xa0, &g_fontBody, colour);
        // Ui_DrawCentred(0xf, 1, 0x1e0, 0x104, 0xa0, &g_fontBody, colour);
        // FUN_004025d7(&g_playerNames + owner * 0x2c, 0x1e0, 0x118, 0xa0, &g_fontBody, colour);
        // ```
        //
        // **The third line is a name.** It read `REALM 3` because nothing in
        // this workspace filled `g_playerNames`; the front end fills it at
        // *Start* — the local player's from what was typed on setup page 4, an
        // AI lord's from `L2.eng` group 7 — so the line says *SOVEREIGN LAND /
        // OF / THE BARON* the way the original's does.
        //
        // **The fallback is not decoration.** A world that did not come through
        // the front end — a `.sav` imported by `l2-scenario`, a kingdom a test
        // built — has no names in it, and an empty third line under two full
        // ones looks like a drawing fault rather than like missing data.
        let owner = match ctx.game.player_names[c.owner as usize].as_str() {
            n if n.is_empty() => format!("REALM {}", c.owner),
            n => n,
        };
        // **The pen is keyed by the realm's shield, not by its id**, and that
        // was the bug a player reported as *"the sovereign land text has the
        // wrong colours … the counties seem to have the right colours … but the
        // text doesn't match that"*. This drew from `Ink::realm` — a table of
        // our own, indexed by the **realm number** — while the minimap tint,
        // the menu-bar banner and the campaign flag all go through the shield.
        // Two keys and three tables for one fact.
        //
        // The shield is what the *human picks*; the AI lords take the slots
        // left over. So a realm id has no colour of its own, and ten of the
        // twenty-five realms across this project's eleven save fixtures fly a
        // shield that is not their id — which is how the wrong key was caught.
        // `docs/decisions.md` C112.
        let shield = ctx.game.kingdom.realms.get(c.owner as usize).map_or(0, |r| r.shield_index);
        // The fallback is `Ink`'s and is **visibly** ours: a world with no
        // shields is a placeholder world, and it should not borrow one of the
        // game's five real colours to look finished. See
        // `l2_view::chrome::realm_pen` on why this does not clamp to 1.
        let colour = l2_view::chrome::realm_pen(shield)
            .unwrap_or_else(|| ink.realm.get(c.owner as usize).copied().unwrap_or(ink.text));
        let style = crate::shell::font::Style {
            colour,
            shadow: Some(crate::shell::font::SHADOW_GREY),
            caps: None,
        };
        let banner = eng(ctx, 15, 0, "SOVEREIGN LAND");
        body_centred_styled(ctx, canvas, 480, 240, 160, &banner, style);
        body_centred_styled(ctx, canvas, 480, 260, 160, &eng(ctx, 15, 1, "OF"), style);
        body_centred_styled(ctx, canvas, 480, 280, 160, &owner, style);
        return;
    }

    // `Ui_DrawNumber(pop, ' ', " ", 0x1FC, 0xBD, &g_fontSmall, 0x3F)` and the
    // identical call for happiness at `0x25A`.
    //
    // **Both are left origins.** `Ui_DrawNumber` takes no anchoring argument —
    // the two calls differ only in their value and their x — so the happiness
    // figure starts at 602 rather than ending there. We right-anchored it,
    // which put a two-digit number on top of the plate's heart and would have
    // put a three-digit one further left still. See `docs/decisions.md` C42.
    strip_text(ctx, canvas, 508, 189, &c.population.to_string(), strip_ink);
    strip_text(ctx, canvas, 602, 189, &c.happiness.to_string(), strip_ink);
    // Group 61, centred in 76 pixels at (0x1E0, 0xD5) and (0x234, 0xD5), and
    // **in colour 0x3F, the same as the numbers** — the captions are not dimmed
    // in the original and ours were unreadable against the plate.
    strip_centred(ctx, canvas, 480, 213, 76, &line_text(ctx, g61::STRIP_TAX), strip_ink);
    strip_centred(ctx, canvas, 564, 213, 76, &line_text(ctx, g61::STRIP_RATION), strip_ink);
    // (0x1FA, 0xE2), and the ration level centred in 76 at (0x234, 0xE2).
    strip_text(ctx, canvas, 506, 226, &format!("{}%", c.tax_rate), strip_ink);
    // "Red when it differs from rationWanted" is the original's own rule, and
    // the colour it picks is `0xF9` rather than `0x3F`.
    let colour = if c.ration_achieved == c.ration_wanted { strip_ink } else { strip_bad };
    strip_centred(ctx, canvas, 564, 226, 76, &ration_label(ctx, c.ration_achieved), colour);

    // Pl8_DrawFrameHere(g_miscCtySheet, band + 0x46, 0x228, 0xB5) — the
    // five-level health thermometer, in the dead band the hotspot leaves.
    let band = c.health_band.min(4);
    let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
        ch.draw_misc(canvas, THERMOMETER_FRAME + band as usize, THERMOMETER.0, THERMOMETER.1)
    });
    if !drawn {
        // OURS: a five-segment bar where the thermometer goes.
        for i in 0..5i32 {
            let lit = 4 - i <= band as i32;
            let y = THERMOMETER.1 + i * 12;
            let colour = if lit { ink.good } else { ink.border };
            canvas.fill_rect(THERMOMETER.0, y, THERMOMETER_W, 10, colour);
        }
    }

    // `Pl8_DrawFrame(g_miscCtySheet, 0x3D, share / 2 + 0x214, 0x106)` — the
    // farm/industry split's thumb, on the 162 × 52 plate at (478, 250) that
    // `CountyStrip_Draw` paints as its last act. It is drawn *here* rather than
    // by the map screen because the county panels repaint the whole sidebar
    // over the map, and a thumb only the map drew would vanish whenever a panel
    // was open.
    //
    // # The blue outline, which is a **frame**
    //
    // `CountyStrip_Draw`'s last branch, verbatim:
    //
    // ```c
    // if (county.labour[8].workers == 0)
    //     Pl8_DrawFrame(g_miscCtySheet, 0x3D, share / 2 + 0x214, 0x106);
    // else
    //     Pl8_DrawFrame(g_miscCtySheet, 0x55, share / 2 + 0x212, 0x104);
    // ```
    //
    // Slot 8 is *Idle townsfolk*, so **the thumb changes the moment anybody in
    // the county has nothing to do** — which is exactly what the player
    // remembered: *"the peasant slider I think had a blue outline if there
    // were idle peasants as well."* This module used to guess the castle job;
    // it is the idle pool.
    //
    // And the outline is measurable rather than described. Frame `0x3D` is
    // 9 × 33 and frame `0x55` is 13 × 37 — four wider and four taller — drawn
    // two pixels left and two pixels up, so it is *the same thumb inside a
    // two-pixel ring*. Every one of the ring's 124 pixels is one of three
    // palette entries, and all three are blue: `95` = `rgb(0, 0, 121)`,
    // `65` = `rgb(157, 202, 234)` and `64` = `rgb(194, 230, 255)`.
    // `crates/l2-view/tests/install.rs` asserts that against the player's own
    // `Misc_cty.pl8`.
    let share = c.industry_share.clamp(0, 100);
    let idle = c.labour[JOB_IDLE_TOWNSFOLK] != 0;
    let (frame, tx, ty) = if idle {
        (misc_cty::SPLIT_THUMB_IDLE, share / 2 + 530, 260)
    } else {
        (misc_cty::SPLIT_THUMB, share / 2 + 532, 262)
    };
    let thumb = ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, tx, ty));
    if !thumb {
        // OURS, for an install with no `Misc_cty.pl8`: the ring is drawn as a
        // ring, because that is what it is.
        canvas.fill_rect(share / 2 + 532, 262, 9, 33, ink.highlight);
        if idle {
            widget::frame(canvas, Rect::new(share / 2 + 530, 260, 13, 37), ink.realm[2]);
        }
    }

    draw_produce_rows(ctx, canvas, c, strip_ink);

    // OURS: the original's quadrants are invisible. A one-pixel outline is how
    // a keyboard player sees which of the four is open.
    if let Some(p) = focus {
        widget::frame(canvas, p.strip_hotspot(), ink.highlight);
    }
}

/// **The produce rows, and the blue outline that is the point of them.**
///
/// The plate below the slider — `Misc_cty` frame `0x38` at (478, 302) — carries
/// one row per thing the county makes. `FUN_0040FEC1` decides *which* rows,
/// into two lists, and `CountyStrip_Draw` then walks them; this is the **left**
/// list, the three farming rows, which is where the dairy is.
///
/// ```c
/// if (fieldsCattle || herd)     rows[n++] = 1;   // FUN_004100AF
/// if (fieldsGrain  || grain)    rows[n++] = 0;   // FUN_0041023A
/// if (fieldsReclaiming)         rows[n++] = 2;   // FUN_004103C5
/// DAT_0053E970 = n < 3 ? 0x3C : 0x2D;            // the row pitch
/// ```
///
/// Each drawer opens with the same three-way choice, and it is the one the
/// player asked about:
///
/// ```c
/// if (labour[slot].useful  < labour[slot].workers) ringed frame, two px up-left
/// else if (labour[slot].workers < labour[slot].wanted) shortfall frame
/// else                                                 plain frame
/// ```
///
/// So **the county's cow gets a blue ring around it the moment more people are
/// milking than the herd can use** — *"there's no blue outline for idle
/// peasants (eg too many on dairy)"*, exactly. The ceiling is
/// [`l2_kingdom::county::County::labour_useful`], the same word
/// `Village_RebuildIcons` uses to decide which peasants in the village are
/// drawn sitting down, and the floor is the one `Panel_JobDetail` already
/// colours the count red below.
///
/// # What is not here
///
/// * **The right-hand list** — wood, iron, stone, weapons and the castle. Three
///   of the five have no state at all (frames `0x2C`, `0x2D`, `0x2E`, drawn
///   flat), and the two that do — the blacksmith and the castle — pick their
///   frame from county `+0x290`, an unnamed byte, and from `+0x1B0`. Neither is
///   settled, so neither is drawn.
/// * **The seasonal deltas** — and a player found the hole before this comment
///   was rewritten: *"Sidebar doesn't show grain being planted as a negative
///   number."* He is right, and he is describing **Spring**.
///
///   Every drawer follows its icon with a `Ui_DrawDelta` (`0x00402E0C`), which
///   is a *signed* number: `value < 0` draws `Ui_DrawNumber(-value, '-', …)` in
///   `colourNeg` (`0xF9`), `value > 0` gets a `'+'` lead in `colourPos`
///   (`0xFA`), and zero gets `'@'`, the blank glyph that keeps a zero
///   column-aligned. The minus is **not a separate mark** — it overwrites
///   `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves free
///   for a sign, and the whole string goes out in one `Ui_DrawText`. With
///   `mode == 0`, which is what all eight rows pass, a value of zero draws
///   **nothing at all**.
///
///   **It is not "the change since last season".** The tooltip layer says so in
///   the game's own words — `L2.eng` group 220 index 15 is *"Cattle, and change
///   next season"* and 16 is *"Wheat, and change next season"* — and the code
///   agrees: `County_RefreshEstimates(county, g_seasonNext)`.
///
///   **The grain row's value is county `+0x22C`, and this workspace never
///   computes it.** `Grain_LabourEstimate` (`0x0044D374`) writes it in a tail
///   *after* the search loop [`l2_kingdom::land::grain_labour_estimate`]
///   reproduces:
///
///   ```c
///   staff = county.labour[0].workers;                    /* the real staffing */
///   county.field_0x230 = Grain_Sow(county, staff, county.grain);
///   if (season == 4) county.crop[2]      = Grain_Harvest(county, staff, county.crop[1]);
///   if (season == 2 || season == 3) county.field_0x2FC = Grain_Grow(county, staff, county.crop[1]);
///
///   if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
///   else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
///   else                  county.field_0x22C = -county.grainEaten;
///   ```
///
///   So in **Spring** the row is `−(sown) − eaten`, which cannot be anything but
///   negative — the player's sentence, exactly. Our port returns
///   `GrainEstimate { wanted, useful }` and stops at the loop, so all four of
///   those writes are missing, and **the sign question never arises because the
///   number never arrives.** It cannot be recovered from the estimate either:
///   the loop calls `Grain_Sow(county, workers, grain − grainEaten)` and the
///   tail calls `Grain_Sow(county, staff, grain)` — a different third argument.
///   `crate::field`'s module docs already say the estimate round runs twice
///   "for … the panel forecasts, which the estimates fill from whatever the
///   allocator last decided"; these are those forecasts.
///
///   The four industry rows read a different quantity again — commodity `c`'s
///   i32 at county `0x2A8 + c * 0x18`, which `docs/records.json` gives to
///   `Industry[c + 1]`'s unnamed head word, and the stone row reads `0x2F0`,
///   one whole record past the end of a four-record array. Reported, not
///   guessed at. `docs/draws-map.md` §5.10.
fn draw_produce_rows(
    ctx: &Ctx,
    canvas: &mut Canvas,
    c: &l2_kingdom::county::County,
    strip_ink: u8,
) {
    // `FUN_0040FEC1`, in its own order: cattle, then grain, then reclamation —
    // and the same list `job_row_at` hit-tests, so the picture and the target
    // cannot drift apart.
    let rows = farm_rows(c);
    // `DAT_0053E970`: three rows or more and they close up.
    let pitch = farm_pitch(rows.len());

    for (n, &slot) in rows.iter().enumerate() {
        let y = pitch * n as i32;
        let (plain, short, ringed) = PRODUCE_ICONS[slot];
        let state = if c.labour_useful[slot] < c.labour[slot] {
            // The ringed frame, and its own position — two pixels up and left
            // of the plain one, because it is the plain one plus a ring.
            Some(ringed)
        } else if c.labour[slot] < c.labour_wanted[slot] {
            short
        } else {
            None
        };
        let (frame, x, dy) = match (slot, state) {
            (1, Some(f)) => (f, 482, 0x131),
            (1, None) => (plain, 484, 0x133),
            (0, Some(f)) if f == ringed => (f, 484, 0x12D),
            (0, Some(f)) => (f, 485, 0x12E),
            (0, None) => (plain, 486, 0x12F),
            (_, Some(f)) => (f, 491, 0x130),
            (_, None) => (plain, 493, 0x132),
        };
        let drawn =
            ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, x, y + dy));
        if !drawn {
            // OURS, with no `Misc_cty.pl8`: a label and, when it applies, the
            // ring — because the ring is the thing being said.
            let ink = &ctx.assets.ink;
            let name = ["GRAIN", "DAIRY", "RECLAIM"][slot];
            text::draw(canvas, x, y + dy + 8, name, ink.dim);
            if state == Some(ringed) {
                widget::frame(canvas, Rect::new(x - 2, y + dy - 2, 44, 32), ink.realm[2]);
            }
        }
        // **The row's forecast for next season.** `Ui_DrawDelta(value, 0, " ",
        // " ", 0x204, pitch*row + dy, &g_font10, 0xFA, 0xF9)` — the same `x` at
        // all three farm rows, and `dy` `0x139` for the two that have a stock
        // and `0x133` for reclamation, which has a countdown instead.
        //
        // **Only the cattle row draws one.** Its value is
        // `Herd_LabourEstimate`'s tail — `(births − deaths) − herdEaten` — and
        // [`l2_kingdom::land::herd_preview`] is that tail, ported, written on
        // every season tick and carried in the save. Grain's is county `+0x22C`
        // and reclamation's `+0x20C`; neither is computed anywhere in this
        // workspace, so neither row can draw one yet, and the doc comment above
        // says what it would take. **C123.**
        let (delta, delta_dy) = match slot {
            1 => (Some(c.herd_change_expected), 0x139),
            0 => (Some(c.grain_change_expected), 0x139),
            _ => (None, 0x133),
        };
        if let Some(v) = delta {
            strip_delta(ctx, canvas, v, 0x204, y + delta_dy);
        }
        // `Ui_DrawNumberRight(store, ' ', …, 0x1E0, y + 0x14D, 0x3C,
        // &g_fontBody, 0x3F)` — the store itself.
        //
        // **`Ui_DrawNumberRight` centres.** Its tail is `FUN_004025D7`, which
        // computes `x + (width − textWidth) / 2`; the name and the
        // `docs/symbols.json` comment both said right-aligned and both were
        // wrong, found independently by two draw audits. So this is centred in
        // sixty pixels from x = 480, not anchored at 540.
        // Reclamation's number is a field this project has not named, so that
        // row carries none.
        let store = match slot {
            0 => Some(c.grain),
            1 => Some(c.herd),
            _ => None,
        };
        if let Some(v) = store {
            body_centred_in(ctx, canvas, 480, y + 0x14D, 0x3C, &v.to_string(), strip_ink);
        }
    }
}

impl CountyScreen {
    /// The pen every panel draws through: `Fntl2_14.pl8` and `Fntl2_22.pl8`,
    /// embossed with [`font::SHADOW`] — which is what `Ui_DrawText` picks for
    /// every screen id but `0x1C` and `0x1F`.
    fn pen<'a>(&self, ctx: &'a Ctx) -> Pen<'a> {
        Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        }
    }

    fn draw_panel(&self, ctx: &Ctx, canvas: &mut Canvas) {
        let pen = self.pen(ctx);
        let ink = &ctx.assets.ink;
        let armies_eat = ctx.game.kingdom.options.armies_eat;
        let (bx, by, cols, rows) = self.panel.box_cells_for(armies_eat);
        // `Ui_DrawBox(x, y, cols, rows)`, border set 0. `Pen::window` falls back
        // to a flat plate of ours where the install has no `Panels.pl8`.
        pen.window(canvas, bx, by, cols, rows, 0);

        let Some(c) = ctx.game.kingdom.counties.get(self.county as usize) else { return };
        match self.panel {
            Panel::Population => {
                // `Eng_DrawString(73, 0, 0x14, 0x38, heading)` and then the
                // county's name — group 100 — immediately after it, which this
                // panel used to omit entirely.
                let x = pen.heading(canvas, 20, 56, &line_text(ctx, g73::TITLE), font::TEXT);
                pen.heading(canvas, x + 2, 56, &county_name(ctx, self.county), font::TEXT);

                self.draw_graph_stub(ctx, canvas, "POPULATION");

                // `Eng_DrawString(73, 8, 0xB0, 0xF0)` — and the two things that
                // follow it, the graph's peak and 73/9 *"people."*, need the
                // history array to have a peak at all. See the module docs.
                pen.body(canvas, 176, 240, &line_text(ctx, g73::GREATEST), font::TEXT);

                heading_row(&pen, canvas, 266, &line_text(ctx, g73::LAST), c.pop_last);
                let rows: [(i32, Line, i32); 3] = [
                    (298, g73::BIRTHS, c.births),
                    (314, g73::DEATHS, -c.deaths),
                    (330, g73::ARMY, c.army),
                ];
                for (y, label, value) in rows {
                    delta_row(&pen, canvas, y, &line_text(ctx, label), value);
                }
                if c.emigrants == 0 {
                    pen.body(canvas, LABEL_X, 346, &line_text(ctx, g73::NO_EMIGRATION), font::TEXT);
                } else {
                    // The original draws the destination county's **name**
                    // between the label and the number: `Eng_DrawString(100,
                    // slot * 20 + emigrantDestination, pen + 0x30, 0x15A)`.
                    let x =
                        pen.body(canvas, LABEL_X, 346, &line_text(ctx, g73::EMIGRANTS), font::TEXT);
                    pen.body(canvas, x, 346, &county_name(ctx, c.emigrant_destination), font::TEXT);
                    delta_value(&pen, canvas, 346, -c.emigrants);
                }
                if c.immigrants == 0 {
                    let s = line_text(ctx, g73::NO_IMMIGRATION);
                    pen.body(canvas, LABEL_X, 362, &s, font::TEXT);
                } else {
                    delta_row(&pen, canvas, 362, &line_text(ctx, g73::IMMIGRANTS), c.immigrants);
                }
                heading_row(&pen, canvas, 386, &line_text(ctx, g73::THIS), c.population);
            }
            Panel::Happiness => {
                let x = pen.heading(canvas, 20, 56, &line_text(ctx, g85::TITLE), font::TEXT);
                pen.heading(canvas, x + 2, 56, &county_name(ctx, self.county), font::TEXT);

                self.draw_graph_stub(ctx, canvas, "HAPPINESS");

                // 85/8 at (176, 241), the number after it, then the face.
                let x = pen.body(canvas, 176, 241, &line_text(ctx, g85::AVERAGE), font::TEXT);
                let x = pen.body(canvas, x, 241, &c.happiness_avg.to_string(), font::TEXT);
                pen.misc_frame(canvas, FRAME_FACE, x, 239);

                heading_row_face(&pen, canvas, 268, &line_text(ctx, g85::LAST), c.happiness_last);
                let rows: [(i32, Line, i32); 6] = [
                    (298, g85::FROM_TAXES, c.shown_tax),
                    (314, g85::FROM_RATION, c.shown_ration),
                    (330, g85::FROM_HEALTH, c.shown_health),
                    (346, g85::FROM_ARMY, c.shown_army),
                    (362, g85::FROM_ALE, c.shown_ale),
                    (378, g85::FROM_EVENTS, c.shown_events),
                ];
                for (y, label, value) in rows {
                    delta_row(&pen, canvas, y, &line_text(ctx, label), value);
                }
                heading_row_face(&pen, canvas, 402, &line_text(ctx, g85::THIS), c.happiness);
            }
            Panel::Tax => {
                // **No title.** `L2.eng` 86/0 *"Tax in"* is drawn by nothing —
                // see [`g86::TITLE_NEVER_DRAWN`] and the module docs. A "TAX IN"
                // heading of ours used to stand here at (96, 152).
                pen.misc_frame(canvas, FRAME_TAX_VIGNETTE, 320, 160);
                pen.body(canvas, 96, 168, &line_text(ctx, g86::RATE), font::TEXT);
                // `Ui_DrawNumber(taxRate, ' ', "%", 0x100, 0xA8, body)` — the
                // lead is a real space and the suffix is the per-cent sign.
                pen.body(canvas, 256, 168, &format!(" {}%", c.tax_rate), font::TEXT);

                let x = pen.body(canvas, 96, 200, &line_text(ctx, g86::PEOPLE_PAY), font::TEXT);
                // `Ui_DrawCount(taxShown, 0, …)` — the number and then group 8's
                // *"Crown."* / *"Crowns."*. We used to write "CROWNS" ourselves.
                pen.count(canvas, x, 200, c.tax_shown, CROWN_NOUN, true, font::TEXT);

                pen.body(canvas, 96, 232, &line_text(ctx, g86::THIS_COUNTY), font::TEXT);
                let empire = ctx
                    .game
                    .kingdom
                    .realms
                    .get(c.owner as usize)
                    .map_or(0i32, |r| r.tax_hap_empire as i32);
                happiness_delta(&pen, canvas, 240, 232, c.d_hap_tax_local + empire);
                pen.body(canvas, 96, 256, &line_text(ctx, g86::OTHER_COUNTIES), font::TEXT);
                happiness_delta(&pen, canvas, 240, 256, c.tax_hap_other);
            }
            Panel::Ration => {
                // `Ui_DrawCentred(87, 0, 0x80, 0x68, 0x120, heading, 0x3F)`.
                pen.heading_centred(canvas, 128, 104, 288, &line_text(ctx, g87::TITLE), font::TEXT);
                pen.body(canvas, 144, 136, &line_text(ctx, g87::WANTED), font::TEXT);
                pen.body(canvas, 240, 136, &ration_label(ctx, c.ration_wanted), font::TEXT);

                pen.body(canvas, 144, 161, &line_text(ctx, g87::ACHIEVED), font::TEXT);
                // **Red when it differs from wanted** — the painter's own
                // `local_8`, `0x3F` or `0xF9`.
                let colour = if c.ration_achieved == c.ration_wanted {
                    font::TEXT
                } else {
                    font::HIGHLIGHT
                };
                pen.body(canvas, 240, 161, &ration_label(ctx, c.ration_achieved), colour);
                happiness_delta(&pen, canvas, 340, 161, c.d_hap_ration);

                pen.body(canvas, 144, 186, &line_text(ctx, g87::HEALTH), font::TEXT);
                pen.body(canvas, 240, 186, &health_label(ctx, c.health_band), font::TEXT);
                happiness_delta(&pen, canvas, 340, 186, c.d_hap_health);

                // The slider's two flanking icons, then the three food columns'
                // icons, then the mark left of "Fed". Six `Pl8_DrawFrame`s the
                // panel used to draw none of.
                pen.misc_frame(canvas, FRAME_GRAIN, 144, 220);
                pen.misc_frame(canvas, FRAME_CATTLE, 370, 220);
                for (frame, x) in
                    [(FRAME_GRAIN, 224), (FRAME_CATTLE, 284), (FRAME_THIRD_FOOD, 344)]
                {
                    pen.misc_frame(canvas, frame, x, 256);
                }
                pen.misc_frame(canvas, FRAME_FED_MARK, 144, 282);

                self.draw_split_slider(ctx, canvas, c.ration_split);

                // **The Fed row, which used to say "NOT SIMULATED".** Its three
                // fields are county `+0x170`, `+0x174` and `+0x16C`, and this
                // module had them down as *"not in l2-kingdom at all, so there is
                // nothing to put here"*. They are `grainEaten * foodPerSack`,
                // `herdEaten * foodPerHead` and `herd * dairyPerHead` — products
                // of three fields that were always there. The absence was
                // recorded honestly, in a comment, beside the words on the
                // screen, and read as a conclusion rather than as a question.
                //
                // **Three numbers, not two, and the third explains the panel**:
                // the standing herd feeds five people a head without being
                // slaughtered, so a county with more dairy than mouths eats
                // nothing at all and its slider has nothing to divide. That
                // number is what says so.
                //
                // Drawn through [`Pen`] rather than the 5 x 7 font, and centred
                // rather than right-aligned: `Ui_DrawNumberRight` **centres**
                // (C110's neighbour, and the symbol's name is a false claim).
                let (by_grain, by_meat, by_dairy) =
                    l2_kingdom::ration::people_fed(&ctx.game.kingdom.tables, c);
                let w = FOOD_COL_W;
                pen.body(canvas, 160, 286, &line_text(ctx, g87::FED), font::TEXT);
                pen.number_centred(canvas, FOOD_COL_X[0], 286, w, by_grain, font::TEXT);
                pen.number_centred(canvas, FOOD_COL_X[1], 286, w, by_meat, font::TEXT);
                pen.number_centred(canvas, FOOD_COL_X[2], 286, w, by_dairy, font::TEXT);
                pen.body(canvas, 144, 308, &line_text(ctx, g87::EATEN), font::TEXT);
                pen.number_centred(canvas, FOOD_COL_X[0], 308, w, c.grain_eaten, font::TEXT);
                pen.number_centred(canvas, FOOD_COL_X[1], 308, w, c.herd_eaten, font::TEXT);

                if armies_eat {
                    // `Ui_DrawNumber(+0x19C + +0x198, ' ', "", 0x88, 0x150)`
                    // then 87/8.
                    let men = c.friendly_troops + c.enemy_troops;
                    let x = pen.body(canvas, 136, 336, &format!(" {men} "), font::TEXT);
                    pen.body(canvas, x, 336, &line_text(ctx, g87::FORAGING), font::TEXT);
                }
            }
        }

        self.draw_buttons(ctx, canvas, armies_eat);
    }

    /// The corner picture, and the two arrows the two order panels have.
    fn draw_buttons(&self, ctx: &Ctx, canvas: &mut Canvas, armies_eat: bool) {
        let pen = self.pen(ctx);
        let ink = &ctx.assets.ink;
        let ok = self.panel.ok_button_for(armies_eat);
        pen.ok_button(canvas, ok.x, ok.y, 0);
        let live = ctx.game.is_players(self.county);
        for (rect, frame, label) in [
            (self.panel.increase_button(), system::ARROW_UP, "+"),
            (self.panel.decrease_button(), system::ARROW_DOWN, "-"),
        ] {
            let Some(r) = rect else { continue };
            // `Widget_Draw` picks record `+0x04` **plus one** while the press
            // timer at `+0x0D` runs, so each arrow has a pressed picture — frames
            // 0x16 and 0x18 — that we never show. Recorded, not built: nothing
            // in this engine carries a widget press timer.
            if !pen.system_frame(canvas, frame, r.x, r.y) {
                widget::button(canvas, ink, r, label, live);
            }
        }
    }

    /// `Panel_RationSlider` (`0x00411FDE`), which is drawn from
    /// `Screen_DrawWidgets`'s `0x19` arm rather than from `Panel_Ration`:
    ///
    /// ```text
    ///   Ui_DrawBoxInterior(0xDC, 0xD8, 8, 2)         (220, 216) 128 x 32
    ///   Pl8_DrawFrame(System, 0x4A, 200,   0xDC)     left cap   (200, 220)
    ///   Pl8_DrawFrame(System, 0x4B, 0x144, 0xDC)     right cap  (324, 220)
    ///   FUN_00403A8F(0xE0, 0xE5, 0x143, 0xE5, 0x10)  line  (224,229)-(323,229)
    ///   FUN_0040437D(0xE0, 0xE6, 100, 4, 0x3F)       fill  (224,230) 100 x 4
    ///   FUN_00403A8F(0xE0, 0xEA, 0x143, 0xEA, 0x1F)  line  (224,234)-(323,234)
    ///   Pl8_DrawFrame(System, 0x4C, 0xDC + split, 0xD8)  the knob at y 216
    /// ```
    ///
    /// **The caps sit at y = 220 and the knob at y = 216**, and this file used
    /// to draw both at 216 — which also put all three hit boxes four pixels
    /// high, because `Ration_SliderClick` (`0x0043A379`) tests
    /// `(200, 0xDC, 24, 24)`, `(0x145, 0xDC, 24, 24)` and
    /// `(0xE0, 0xDC, 0x66, 24)`, every one of them at **0xDC = 220**.
    fn draw_split_slider(&self, ctx: &Ctx, canvas: &mut Canvas, split: i32) {
        let pen = self.pen(ctx);
        let ink = &ctx.assets.ink;
        // The parchment well the whole control sits in, which we drew none of.
        pen.box_interior(canvas, 220, SLIDER_KNOB_Y, 8, 2);
        // The three track lines. The colours are the original's literals.
        canvas.fill_rect(SLIDER_TRACK_X, 229, SLIDER_TRACK_W, 1, TRACK_TOP);
        canvas.fill_rect(SLIDER_TRACK_X, 230, SLIDER_TRACK_W, 4, TRACK_FILL);
        canvas.fill_rect(SLIDER_TRACK_X, 234, SLIDER_TRACK_W, 1, TRACK_BOTTOM);
        let caps = {
            let l = pen.system_frame(
                canvas,
                system::SLIDER_CAP_LEFT,
                SLIDER_CAP_LEFT_X,
                SLIDER_CTRL_Y,
            );
            let r = pen.system_frame(
                canvas,
                system::SLIDER_CAP_RIGHT,
                SLIDER_CAP_RIGHT_X,
                SLIDER_CTRL_Y,
            );
            l && r
        };
        if !caps {
            widget::button(canvas, ink, split_down_button(), "<", false);
            widget::button(canvas, ink, split_up_button(), ">", false);
        }
        let knob_x = SLIDER_KNOB_ORIGIN + split.clamp(0, MAX_RATION_SPLIT);
        if !pen.system_frame(canvas, system::SLIDER_KNOB, knob_x, SLIDER_KNOB_Y) {
            canvas.fill_rect(knob_x, SLIDER_KNOB_Y, system::SLIDER_KNOB_W, 32, ink.highlight);
        }
    }

    /// **A stub, and it looks like one.** [`Ui_HistoryGraph`'s whole
    /// picture](self#what-is-still-ours-and-says-so) needs `g_countyHistory`
    /// and `Graphs.pl8`; `l2-kingdom` has no history array and `l2-view` does
    /// not load that sheet, so there is nothing to plot.
    ///
    /// The recess itself is real — `Ui_DrawInsetRect(0x20, 0x54, 0x192, 0x9B)`
    /// is the graph's own first call and is drawn here at its own coordinates.
    /// The two lines inside it are ours and say so.
    fn draw_graph_stub(&self, ctx: &Ctx, canvas: &mut Canvas, what: &str) {
        let ink = &ctx.assets.ink;
        let Some(r) = self.panel.graph_rect() else { return };
        canvas.fill_rect(r.x, r.y, r.w, r.h, ink.background);
        self.pen(ctx).inset(canvas, r);
        let mid = r.y + r.h / 2;
        // OURS, both of them: a diagnostic, in our own 5 x 7 font, so that a
        // screenshot cannot be mistaken for the original's graph.
        text::draw_centred(canvas, r.centre_x(), mid - 10, &format!("{what} HISTORY"), ink.dim);
        text::draw_centred(canvas, r.centre_x(), mid + 2, "NOT SIMULATED", ink.bad);
    }
}

/// One `L2.eng` string, from the install if it has one and from our own
/// transcription if it does not.
///
/// # `Ui_DrawNumberRight` centres, and its name is a false claim
///
/// Recorded here because it is what this panel's five numbers depend on and two
/// branches found it independently within a day. It is `Ui_NumberToBuffer`
/// followed by `FUN_004025D7`, which is
/// `Ui_DrawText(s, x + max(0, (width - textWidth) / 2), y, …)` — and
/// `Ui_DrawCentred` calls **the same function**. One alignment, two names, one
/// of them true. This module right-aligned these columns because the symbol said
/// *right*.
///
/// **The symbol's entry is `[V]` and its comment says the opposite of its
/// body**, so the verification carried the error: a wrong name with a wrong
/// verified comment is believed twice — once for the name and once for the tier
/// — with nothing left to contradict it. `[V]` records that somebody read it,
/// not that somebody read it correctly. Twenty call sites in the original
/// inherit it and **eighteen beyond this panel are unaudited.**
fn line_text(ctx: &Ctx, l: Line) -> String {
    eng(ctx, l.group, l.index, l.ours)
}

fn ration_name(level: i32) -> &'static str {
    RATION_NAMES
        .get(level.clamp(0, RATION_LEVEL_COUNT as i32 - 1) as usize)
        .copied()
        .unwrap_or("?")
}

/// `Eng_DrawString(20, healthBand, …)` — the five health words, with
/// [`HEALTH_BAND_NAMES`] as the fallback.
fn health_label(ctx: &Ctx, band: u8) -> String {
    let ours = HEALTH_BAND_NAMES.get(band as usize).copied().unwrap_or("?");
    eng(ctx, GROUP_HEALTH_BANDS, band as usize, ours)
}

/// A `Ui_DrawNumber(v, '@', "", 0x150, y, heading)` row: the label in the left
/// column and the value **left-aligned from x = 336**, both in the 22-pixel
/// font, which is what the three plain rows on the two graph panels are.
fn heading_row(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.heading(canvas, LABEL_X, y, label, font::TEXT);
    pen.heading(canvas, VALUE_LEFT, y, &value.to_string(), font::TEXT);
}

/// The same with `Pl8_DrawFrame(Misc_cty, 0x17, pen + 0x150, y + 3)` after it —
/// the happiness panel's *Last season* and *This Season* rows both carry the
/// face, three pixels below the text's own line.
fn heading_row_face(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.heading(canvas, LABEL_X, y, label, font::TEXT);
    let x = pen.heading(canvas, VALUE_LEFT, y, &value.to_string(), font::TEXT);
    pen.misc_frame(canvas, FRAME_FACE, x, y + 3);
}

/// `Ui_DrawDelta(value, 0, "", "", 0x150, y, body, 0x3F, 0xF9)` — the label and
/// the value.
fn delta_row(pen: &Pen, canvas: &mut Canvas, y: i32, label: &str, value: i32) {
    pen.body(canvas, LABEL_X, y, label, font::TEXT);
    delta_value(pen, canvas, y, value);
}

/// The value half on its own, for the emigration row, which puts a county name
/// between the label and the number.
///
/// Three things it is easy to get wrong and all three are the original's:
/// **a zero draws nothing at all** — mode 0 with `value == 0` returns before the
/// first `Ui_DrawText` — the digits are **left-aligned from `x`** rather than
/// right-anchored to it, and the empty prefix still advances the pen by
/// [`TRAILING`], so they start four pixels right of it.
fn delta_value(pen: &Pen, canvas: &mut Canvas, y: i32, value: i32) {
    if value == 0 {
        return;
    }
    let colour = if value < 0 { font::HIGHLIGHT } else { font::TEXT };
    let sign = if value < 0 { '-' } else { '+' };
    pen.body(canvas, VALUE_LEFT + TRAILING, y, &format!("{sign}{}", value.abs()), colour);
}

/// `Ui_DrawHappinessDelta` (`0x0041AC95`), whole:
///
/// ```text
///   Ui_DrawText("(", x, y, font, 0x3F)              pen = w("(") + 4
///   pen -= 4
///   Ui_DrawDelta(value, 1, "", "", x + pen, y, font, colourPos, colourNeg)
///   Pl8_DrawFrame(g_miscCtySheet, 0x17, x + pen, y - 2)
///   Ui_DrawText(")", x + pen + 0x14, y, font, 0x3F)
/// ```
///
/// So it is `( ±n ☺ )`, the bracket sits at `x`, and the closing bracket is
/// [`FACE_W`] = 20 pixels past the face — which is exactly the face's own width
/// in `Misc_cty.pl8`, so the gap is the picture's and not a guess. **Mode 1
/// never suppresses a zero**, unlike the panel rows: a zero draws `0` with the
/// blank sign column.
fn happiness_delta(pen: &Pen, canvas: &mut Canvas, x: i32, y: i32, value: i32) {
    let after_bracket = pen.body(canvas, x, y, "(", font::TEXT) - TRAILING;
    let colour = if value < 0 { font::HIGHLIGHT } else { font::TEXT };
    let sign = match value.signum() {
        -1 => "-",
        1 => "+",
        _ => "",
    };
    let face_x =
        pen.body(canvas, after_bracket + TRAILING, y, &format!("{sign}{}", value.abs()), colour);
    pen.misc_frame(canvas, FRAME_FACE, face_x, y - 2);
    pen.body(canvas, face_x + FACE_W, y, ")", font::TEXT);
}

/// Geometry tests. The canvas tests that read numbers back off the pixels live
/// in `tests/screens.rs`, because they need the shipped install.
///
/// **Nine mutations were checked against these and those**, each turning
/// exactly one test red and no others. The last three are this audit's, and
/// every literal in the assertion is pinned from the decompilation rather than
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
    /// the health thermometer is drawn — which is why the gap exists.
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
    /// are pinned from the decompilation rather than computed from the
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
