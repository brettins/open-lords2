//! **The tool tips** — `FUN_00476E95` (`0x00476E95`), the frame function that
//! puts a one-line box beside a resting pointer, and the six functions it calls.
//!
//! `L2.eng` group **220** is its vocabulary and it has no other consumer: *"Null
//! tool tip"* at index 0 and thirty-four tips after it. The Help Options panel's
//! *"Tool tips"* row (`Opt_ToggleToolTips`, `0x004347C7`) flips `g_optToolTips`,
//! and until this module that flag was read by nothing here.
//!
//! # The functions, `[V]`, read out of the decompilation
//!
//! ```c
//! void FUN_00476E95(void) {                            /* once a frame, Battle_Frame */
//!   if (g_optToolTips) {
//!     if (tipId == 0) FUN_00477131();                  /* wait for a rest, then show */
//!     else            FUN_004770B8();                  /* any mouse change hides it */
//!     if (tipId) {
//!       DAT_005AEA40 = 1; DAT_005CD4F8 = 0; g_penAdvance = 0;       /* flat text */
//!       FUN_0040328E(0xDC, tipId, x + 4, y + 4, 0xB4, 200, 0, 0, body, 0x3F);
//!       w = 0xC - (0xB0 - widest) / 16;                /* in 16-pixel units */
//!       h = DAT_005CD4F8 < 0x11 ? 0x16 : 0x28;         /* one line, or more */
//!       FUN_004B414A(x, y, 0x20);                      /* fill w*16 x h */
//!       FUN_0040328E(0xDC, tipId, x + 4, y + 4, 0xB0, 200, 0, 0, body, 0x3F);
//!       DAT_005AEA40 = 0;
//!       FUN_00403CF4(x, y, w << 4, h, 0x3F);           /* the outline, clipped */
//!     }
//!   }
//! }
//! void FUN_00477131(void) {
//!   now = timeGetTime();
//!   if (!g_mouseInputChanged && 999 < (int)(now - stamp)) {
//!     stamp = now;
//!     tipId = FUN_004772B6();                          /* may be 0 */
//!     x = g_mouseX < 0x141 ? g_mouseX + 0x1E : g_mouseX - 0xDC;
//!     y = g_mouseY < 0xF1  ? g_mouseY + 0x1E : g_mouseY - 0x1E;
//!     FUN_00477249();                                  /* clamp to 0 … 0x1B8 */
//!     …save the 192 x 40 backdrop…
//!   } else if (g_mouseInputChanged) stamp = now;
//! }
//! void FUN_004770B8(void) { if (g_mouseInputChanged) { tipId = 0; …restore… } }
//! void FUN_0047703A(void) { if (tipId) { tipId = 0; …restore… } }  /* Screen_Draw */
//! int  FUN_004772B6(void) {
//!   switch (DAT_004D6FB8[g_screenId]) { case 1: return FUN_00477320();
//!                                      case 2: return FUN_004777AA(); }
//!   return 0;
//! }
//! ```
//!
//! # What that means for a player
//!
//! * **The rest is a second of wall clock, not a frame count.** `timeGetTime`,
//!   strictly more than 999 ms since the stamp. Ours counts [`crate::TICK_MS`]
//!   ticks, so it is **63 ticks** (1,008 ms) — the same conversion
//!   [`crate::audio`] makes for the tip narration's own `999 <` test.
//! * **The stamp is written on every frame the mouse changes while no tip is
//!   up**, and on the frame a tip is resolved. It is *not* written on the frame
//!   that hides a tip. So a tip that has been up for more than a second and is
//!   nudged for exactly one frame comes straight back at the new place on the
//!   next still frame, with no rest at all; a movement of two frames or more
//!   re-arms the full second. Reproduced.
//! * **"The mouse changed" is `g_mouseInputChanged`**, which the frame poll
//!   (`FUN_004B191E`) sets when the position moved *or* either button changed.
//!   A key press does not touch it.
//! * **The lookup is not a widget table.** It is a per-screen byte,
//!   `DAT_004D6FB8[g_screenId]` ([`SCREENS`]), choosing one of two pointer
//!   ladders: [`campaign_tip`] for the thirty-five screens that sit on the
//!   campaign map's sidebar, [`battle_tip`] for the battlefield, `0x29`. The
//!   campaign ladder reads live state — the minimap mode, whether the selected
//!   county is the player's, and the two produce-row lists — at the moment the
//!   tip is resolved, and the id it answers is the group-220 index.
//! * **The box is placed once**, 30 pixels right of and below the pointer, or
//!   220 left / 30 above past the screen's middle (`x 321`, `y 241`), then
//!   clamped to `0 … 440`. It does not follow the pointer; the pointer moving
//!   is what takes it away.
//! * **A repaint takes it away too.** `Screen_Draw` opens with `FUN_0047703A`,
//!   and so do the siege-preparation, battle-prompt, battle-result and outcome
//!   painters and `Smk_PlayThenClose`. It comes back by the ordinary rule — at
//!   once if its stamp is already a second old.
//! * **Nothing suppresses it but the table.** A tip screen up is `g_screenId
//!   0x27`, whose byte is 0; the battlefield's drag (`0x2A`) and outcome
//!   (`0x2B`) are 0; the message scroll changes no screen id, so tips go on
//!   showing over the sidebar under a message.
//!
//! # Where it lives here
//!
//! [`Tooltips`] is the state and [`Tooltips::frame`] is `FUN_00476E95`'s input
//! half; [`crate::screen::Machine`] owns it, feeds it the pointer and the screen
//! byte, and draws [`draw`] last — `Battle_Frame` calls `FUN_00476E95` after the
//! turn timer, the tip ladder and the message pump.
//!
//! **It is on the machine and not on [`crate::Game`]** because its clock is a
//! frame counter and `Game` is compared whole by the save round trips: a game
//! that had been looked at would not equal the same game reloaded. Two writes
//! from outside the layer therefore arrive as projections, each written down at
//! its reader in [`crate::screen::Machine`]: `Opt_ToggleToolTips`' stamp reset,
//! and `Map_InitMode`'s.
//!
//! # What was not established
//!
//! * `g_selectedCounty`'s produce-row lists (`DAT_0053F690`, `DAT_00553FD0`)
//!   are filled by `FUN_0040FEC1` when the county strip is painted. That they
//!   are always the selected county's when a tip resolves is `[I]`; ours
//!   computes them from the selected county at resolve time.
//! * `FUN_0047703A`'s callers are painters. Ours drops the tip when the set of
//!   screens on the stack changes, which is when a painter would run; a
//!   repaint that changes no screen (a toggle's `g_redrawRequest = 2`) is not
//!   modelled, and every such repaint found is preceded by a click, which has
//!   already hidden the tip. `[I]`.
//! * The first pass is drawn at a wrap of 180 and the box sized from it; the
//!   text is then drawn again at 176. A string that wraps differently at the two
//!   widths would get a box of the wrong height. Both passes are drawn here
//!   exactly as the original draws them, so whatever it showed, this shows; the
//!   original then left anything outside the saved 192 x 40 backdrop on the
//!   screen after the tip went, and ours repaints and does not.

use l2_view::Canvas;

use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};
use crate::Game;

/// `L2.eng` group 220, the tips' only consumer is this layer.
pub const GROUP: usize = 220;
/// Index 0 *"Null tool tip"* and thirty-four tips.
pub const COUNT: usize = 35;

/// `FUN_00477131`'s `999 < (int)(now - stamp)`, in milliseconds.
pub const REST_MS: u32 = 999;

/// **`DAT_004D6FB8`**, one byte per `g_screenId` `0x00 … 0x43`: `1` the
/// campaign ladder, `2` the battlefield ladder, `0` no tips. `[V]`, read out of
/// `Lords2.exe` and held against the player's own copy by
/// `tests/tooltips.rs`.
pub const SCREENS: [u8; 0x44] = [
    1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, // 0x00
    1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 1, 1, 0, // 0x10
    0, 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, // 0x20
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, // 0x30
    0, 0, 1, 1, // 0x40
];

/// The two values [`SCREENS`] holds besides zero.
pub mod ladder {
    /// `FUN_00477320`, [`super::campaign_tip`].
    pub const CAMPAIGN: u8 = 1;
    /// `FUN_004777AA`, [`super::battle_tip`].
    pub const BATTLE: u8 = 2;
}

/// `DAT_004D6FB8[g_screenId]`, zero for a byte past the table.
pub fn ladder_of(screen: Option<u8>) -> u8 {
    screen.and_then(|b| SCREENS.get(b as usize).copied()).unwrap_or(0)
}

/// **The original's `g_screenId` for each of our screens**, as far as the
/// tooltip table needs it.
///
/// Exhaustive with no wildcard, so a new screen is a compile error here until
/// somebody says which byte it is. `None` is a screen that is **ours** and has
/// no byte at all. The bytes are the ones [`ScreenId`]'s own documentation
/// names; the four county panels are `0x14` population, `0x15` tax, `0x16`
/// happiness and `0x19` rations (`docs/screens-county.md` §1).
pub fn screen_byte(id: ScreenId, game: &Game, mode: Option<u8>) -> Option<u8> {
    use crate::screens::county::Panel;
    use ScreenId as S;
    if let Some(b) = mode {
        return Some(b);
    }
    match id {
        S::Campaign => Some(0x00),
        S::Village(_) => Some(0x02),
        S::Info(_) => Some(0x04),
        S::Merchant(_) => Some(0x08),
        S::Court => Some(0x09),
        S::Armoury(_) => Some(0x0A),
        S::Diplomacy => Some(0x0B),
        S::Trade(..) => Some(0x0C),
        S::Rack(..) => Some(0x0D),
        S::Job(..) => Some(0x0F),
        S::Divide(_) => Some(0x11),
        S::BattlePrompt => Some(0x12),
        S::BattleResult => Some(0x13),
        S::County(_, Panel::Population) => Some(0x14),
        S::County(_, Panel::Tax) => Some(0x15),
        S::County(_, Panel::Happiness) => Some(0x16),
        S::RaiseArmy(_) => Some(0x17),
        S::Supplies(_) => Some(0x18),
        S::County(_, Panel::Ration) => Some(0x19),
        S::DiploCompose(..) => Some(0x1A),
        S::Castle(_) => Some(0x1B),
        S::Conquest => Some(0x1C),
        S::Nobles => Some(0x20),
        S::Siege(_) => Some(0x1D),
        // The front end's menu is `0x1F` page 1 (`screens::menu`).
        S::Menu | S::Setup(_) => Some(0x1F),
        S::About => Some(0x25),
        // A film: `Smk_Play` parks `g_screenId` at `0x22`.
        S::Movie(_) => Some(0x22),
        S::Tip => Some(0x27),
        S::Battlefield => Some(game.battle.as_ref().map_or(0x29, |b| b.screen_id())),
        S::Ratings => Some(0x2E),
        S::MenuBar(_) => Some(0x32),
        S::SaveLoad(mode) => Some(mode.screen_id()),
        S::Options(page) => page.screen_id(),
        // The message scroll is not a screen id in the original, and the
        // caller looks through it; the demo index is ours.
        S::Message | S::Index => None,
    }
}

/// What [`campaign_tip`] reads besides the pointer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sidebar {
    /// `g_minimapMode`, 0 … 3.
    pub minimap_mode: u8,
    /// `g_counties[g_selectedCounty].owner == g_localPlayer`.
    pub owned: bool,
    /// `DAT_0053F690`, `FUN_0040FEC1`'s farm list — see
    /// [`crate::screens::county::farm_rows`].
    pub farm: Vec<usize>,
    /// `DAT_00553FD0`, its industry list —
    /// [`crate::screens::county::industry_rows`].
    pub industry: Vec<usize>,
}

impl Sidebar {
    /// The selected county's, as `CountyStrip_Draw` last painted them.
    pub fn of(game: &Game, minimap_mode: u8) -> Sidebar {
        let k = &game.kingdom;
        let c = k.counties.get(game.selected as usize);
        Sidebar {
            minimap_mode,
            owned: c.is_some_and(|c| c.owner == game.player),
            farm: c.map(crate::screens::county::farm_rows).unwrap_or_default(),
            industry: c.map(crate::screens::county::industry_rows).unwrap_or_default(),
        }
    }
}

/// **`FUN_00477320` (`0x00477320`)** — the campaign sidebar's tip under the
/// pointer, or 0. The ladder is the original's, comparison for comparison.
///
/// It partitions the 162-pixel column right of `x 478` from `y 24` down:
/// the minimap and its four mode buttons, the county strip's two picture
/// buttons and the health heart between them, the labour slider, the produce
/// rows, the five sidebar buttons and End Turn. **Twenty-six ids**: 1 … 22 and
/// 31 … 34.
// arm: 0x00477320/campaign-sidebar-tips hover
pub fn campaign_tip(s: &Sidebar, x: i32, y: i32) -> u8 {
    if y < 0x18 || x < 0x1DE {
        return 0;
    }
    if y < 0x9B {
        return if x < 0x261 {
            1
        } else if s.minimap_mode < 1 {
            if y < 0x3E {
                2
            } else if y < 0x5E {
                3
            } else if y < 0x7D {
                4
            } else {
                5
            }
        } else if y < 0x7D {
            match s.minimap_mode {
                1 => 2,
                2 => 3,
                _ => 4,
            }
        } else {
            0x1F
        };
    }
    if y < 0xFA {
        if !s.owned {
            return 0;
        }
        let middle = x >= 0x223 && x < 0x23B;
        return match (y < 0xD2, x < 0x223, middle) {
            (_, _, true) => 0x22,
            (true, true, _) => 6,
            (true, false, _) => 7,
            (false, true, _) => 8,
            (false, false, _) => 9,
        };
    }
    if y < 300 {
        return if s.owned { 10 } else { 0 };
    }
    if y < 0x1AE {
        if !s.owned {
            return 0;
        }
        // `(g_mouseY - 0x130) / pitch`, C division: y 300 … 303 is row 0.
        if x < 0x230 {
            let pitch = crate::screens::county::farm_pitch(s.farm.len());
            let row = (y - 0x130) / pitch;
            return match s.farm.get(row as usize) {
                Some(1) => 0x0F,
                Some(0) => 0x10,
                Some(2) => 0x11,
                _ => 0,
            };
        }
        let pitch = crate::screens::county::industry_pitch(s.industry.len());
        let row = (y - 0x130) / pitch;
        return match s.industry.get(row as usize) {
            Some(6) => 0x12,
            Some(5) => 0x13,
            Some(4) => 0x14,
            Some(7) => 0x15,
            Some(3) => 0x16,
            _ => 0,
        };
    }
    if y < 0x1CC {
        return if x < 0x200 {
            0x0B
        } else if x < 0x221 {
            0x0C
        } else if x < 0x241 {
            0x0D
        } else if x < 0x260 {
            0x20
        } else {
            0x21
        };
    }
    0x0E
}

/// **`FUN_004777AA` (`0x004777AA`)** — the battlefield's tip under the
/// pointer, or 0: the overview, the selected troops, the troop levels, and the
/// five buttons along the bottom. **Eight ids**, 23 … 30.
// arm: 0x004777AA/battle-hud-tips hover
pub fn battle_tip(x: i32, y: i32) -> u8 {
    if y < 0x18 || x < 0x1DE {
        0
    } else if y < 0xB8 {
        0x17
    } else if y < 0x19C {
        0x18
    } else if y < 0x1C1 {
        0x19
    } else if x < 0x200 {
        0x1A
    } else if x < 0x220 {
        0x1B
    } else if x < 0x240 {
        0x1C
    } else if x < 0x260 {
        0x1D
    } else {
        0x1E
    }
}

/// **Where the box goes** — `FUN_00477131`'s two offsets and `FUN_00477249`'s
/// clamp, for a pointer at `(mx, my)`.
pub fn place(mx: i32, my: i32) -> (i32, i32) {
    let mut x = if mx < 0x141 { mx + 0x1E } else { mx - 0xDC };
    let mut y = if my < 0xF1 { my + 0x1E } else { my - 0x1E };
    // FUN_00477249, four independent tests.
    if x < 0 {
        x = 0;
    }
    if x > 0x1B7 {
        x = 0x1B8;
    }
    if y < 0 {
        y = 0;
    }
    if y > 0x1B7 {
        y = 0x1B8;
    }
    (x, y)
}

/// The wrap of the first pass, which the box is sized from.
pub const MEASURE_WIDTH: i32 = 0xB4;
/// The wrap of the second pass, which is the one left on screen.
pub const DRAW_WIDTH: i32 = 0xB0;
/// `FUN_004B414A(x, y, 0x20)`.
pub const FILL: u8 = 0x20;
/// The text and `FUN_00403CF4`'s outline.
pub const INK: u8 = 0x3F;
/// `FUN_0040328E`'s line step in the body font.
pub const LINE: i32 = 0x10;

/// **The box's size**, from the first pass: how many lines it wrapped to, and
/// the widest line's `g_penAdvance` (its advance plus `Ui_DrawText`'s trailing
/// four).
///
/// `0xC - (0xB0 - widest) / 16` units of sixteen pixels — C division, toward
/// zero — and 22 pixels tall for one line (`DAT_005CD4F8 < 0x11`), 40 for more.
pub fn box_size(lines: usize, widest: i32) -> (i32, i32) {
    let units = 0xC - (0xB0 - widest) / 16;
    let h = if (lines as i32) * LINE < 0x11 { 0x16 } else { 0x28 };
    (units * 16, h)
}

/// A tip on screen: which, and where its box's corner is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// The group-220 index, never 0.
    pub id: u8,
    pub x: i32,
    pub y: i32,
}

/// **The layer's state** — `DAT_004EAC00` the tip, `DAT_004EB268`/`70` its
/// corner, `_DAT_004EA830` the stamp, and our tick count standing in for
/// `timeGetTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tooltips {
    now: u64,
    /// `None` is the stamp at 0 against a clock that has run since boot:
    /// *long ago*. `Opt_ToggleToolTips` and `Map_InitMode` both write that.
    stamp: Option<u64>,
    shown: Option<Shown>,
}

impl Tooltips {
    pub fn new() -> Tooltips {
        Tooltips::default()
    }

    /// The tip on screen, if there is one.
    pub fn shown(&self) -> Option<Shown> {
        self.shown
    }

    /// **`_DAT_004EA830 = 0`** — `Opt_ToggleToolTips` (`0x004347C7`) and
    /// `Map_InitMode` (`0x00498270`). The next still frame shows a tip at once.
    pub fn rearm(&mut self) {
        self.stamp = None;
    }

    /// **`FUN_0047703A` (`0x0047703A`)** — the tip goes, the stamp stays.
    /// Returns whether one was up.
    // arm: 0x0047703A/tip-dropped-by-a-repaint frame
    pub fn drop_tip(&mut self) -> bool {
        self.shown.take().is_some()
    }

    /// **One frame of `FUN_00476E95`'s input half.** `changed` is
    /// `g_mouseInputChanged`, `pointer` is `(g_mouseX, g_mouseY)`, and
    /// `resolve` is `FUN_004772B6`, asked only on the frame a tip is due.
    ///
    /// Returns whether what is on screen changed.
    // arm: 0x00476E95/tool-tip-rest hover
    pub fn frame(
        &mut self,
        enabled: bool,
        changed: bool,
        pointer: (i32, i32),
        resolve: impl FnOnce(i32, i32) -> u8,
    ) -> bool {
        self.now += 1;
        if !enabled {
            return false;
        }
        if self.shown.is_some() {
            // FUN_004770B8: the stamp is not touched.
            return changed && self.shown.take().is_some();
        }
        // FUN_00477131.
        let rested = self
            .stamp
            .is_none_or(|s| (self.now - s) * crate::TICK_MS as u64 > REST_MS as u64);
        if changed {
            self.stamp = Some(self.now);
            return false;
        }
        if !rested {
            return false;
        }
        self.stamp = Some(self.now);
        let (mx, my) = pointer;
        let id = resolve(mx, my);
        if id == 0 {
            return false;
        }
        let (x, y) = place(mx, my);
        self.shown = Some(Shown { id, x, y });
        true
    }
}

// ------------------------------------------------------------------ the words

/// **Our transcription of group 220**, for an install whose `L2.eng` cannot be
/// read. `CLAUDE.md` rule 6: the player's own file is drawn, and this is the
/// fallback. `tests/tooltips.rs` holds it against the file, string for string.
pub const TEXT: [&str; COUNT] = [
    "Null tool tip",
    "Kingdom view. Click on a county.",
    "Labour, red if needed, purple if idle.",
    "Ration status",
    "Overall happiness",
    "Overview map",
    "View population report",
    "View happiness report",
    "Set tax rate",
    "Set rations",
    "Adjust labor allocation",
    "Create an army",
    "Go to treasury",
    "Send supplies",
    "End your turn",
    "Cattle, and change next season",
    "Wheat, and change next season",
    "Seasons left to reclaim a field",
    "Wood produced next season",
    "Stone produced next season",
    "Iron produced next season",
    "Weapons produced. Click for smithy.",
    "Seasons left to build castle",
    "Battle overview. Click to go to area.",
    "Selected troops. Click to deselect.",
    "Overall troop levels",
    "Pause the battle",
    "Retreat from field",
    "Lower castle drawbridge",
    "Mop up enemy troops",
    "Autocalculate battle or siege results",
    "Return census map to empire mode",
    "Build or update a castle",
    "Diplomatic initiatives",
    "The health of the county",
];

/// `FUN_0040328E(0xDC, id, …)`'s string: the player's `L2.eng`, and ours only
/// where the file gave nothing.
pub fn words(shell: &crate::shell::ShellAssets, id: u8) -> String {
    let s = shell.text(GROUP, id as usize);
    if s.is_empty() {
        TEXT.get(id as usize).copied().unwrap_or("").to_string()
    } else {
        s.to_string()
    }
}

// ------------------------------------------------------------------ the draw

/// `FUN_004015B9(c, &g_fontBody)`.
fn glyph_width(ctx: &Ctx, c: char) -> i32 {
    let s = c.to_string();
    match &ctx.assets.shell.body {
        Some(f) => f.width(&s),
        None => l2_view::text::width(&s),
    }
}

/// **The box as it is drawn**, and the lines left in it: the rectangle and the
/// second pass's wrap. The first pass is drawn by [`draw`] and not returned,
/// because the fill covers it.
pub fn layout(ctx: &Ctx, tip: Shown) -> (crate::input::Rect, Vec<String>) {
    let text = words(&ctx.assets.shell, tip.id);
    let measured = crate::message::break_lines(&text, MEASURE_WIDTH, |c| glyph_width(ctx, c));
    let pen = pen(ctx);
    let widest = measured
        .iter()
        .map(|l| {
            let mut scratch = Canvas::new(1, 1);
            pen.body(&mut scratch, 0, 0, l, INK)
        })
        .max()
        .unwrap_or(0);
    let (w, h) = box_size(measured.len(), widest);
    let lines = crate::message::break_lines(&text, DRAW_WIDTH, |c| glyph_width(ctx, c));
    (crate::input::Rect::new(tip.x, tip.y, w, h), lines)
}

fn pen<'a>(ctx: &'a Ctx) -> Pen<'a> {
    // `DAT_005AEA40 = 1` around both passes: flat text, no emboss.
    Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: None,
        caps: None,
    }
}

/// **`FUN_00476E95`'s draw half**, over whatever the frame painted. Nothing
/// when the option is off or no tip is up.
pub fn draw(ctx: &Ctx, tips: &Tooltips, canvas: &mut Canvas) {
    if !ctx.game.prefs.tool_tips {
        return;
    }
    let Some(tip) = tips.shown() else { return };
    let pen = pen(ctx);
    let text = words(&ctx.assets.shell, tip.id);
    let (tx, ty) = (tip.x + 4, tip.y + 4);
    // First pass, wrapped at 0xB4: what the box is measured from.
    let measured = crate::message::break_lines(&text, MEASURE_WIDTH, |c| glyph_width(ctx, c));
    for (k, line) in measured.iter().enumerate() {
        pen.body(canvas, tx, ty + LINE * k as i32, line, font::TEXT);
    }
    let (rect, lines) = layout(ctx, tip);
    canvas.fill_rect(rect.x, rect.y, rect.w, rect.h, FILL);
    // Second pass, wrapped at 0xB0.
    for (k, line) in lines.iter().enumerate() {
        pen.body(canvas, tx, ty + LINE * k as i32, line, font::TEXT);
    }
    outline(canvas, rect.x, rect.y, rect.w, rect.h, INK);
}

/// **`FUN_00403CF4` (`0x00403CF4`)** — a one-pixel rectangle, its origin and
/// extent clipped to the screen first.
fn outline(canvas: &mut Canvas, mut x: i32, mut y: i32, mut w: i32, mut h: i32, c: u8) {
    let (sw, sh) = (canvas.width as i32, canvas.height as i32);
    if x < 1 {
        x = 0;
    }
    if sw <= w + x {
        w = sw - x;
    }
    if y < 1 {
        y = 0;
    }
    if sh <= h + y {
        h = sh - y;
    }
    canvas.fill_rect(x, y, w, 1, c);
    canvas.fill_rect(x, y + h - 1, w, 1, c);
    canvas.fill_rect(x, y, 1, h, c);
    canvas.fill_rect(x + w - 1, y, 1, h, c);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DAT_004D6FB8`'s shape, which the resolver depends on: the four county
    /// panels agree, and the tip's own screen answers nothing.
    #[test]
    fn the_table_answers_the_screens_it_is_about() {
        for b in [0x14, 0x15, 0x16, 0x19, 0x00, 0x10, 0x02] {
            assert_eq!(SCREENS[b], ladder::CAMPAIGN, "screen {b:#04x}");
        }
        assert_eq!(SCREENS[0x29], ladder::BATTLE);
        for b in [0x08, 0x0A, 0x0B, 0x17, 0x1B, 0x1F, 0x27, 0x2A, 0x2B] {
            assert_eq!(SCREENS[b], 0, "screen {b:#04x}");
        }
        assert_eq!(SCREENS.iter().filter(|&&v| v == ladder::CAMPAIGN).count(), 35);
    }
}
