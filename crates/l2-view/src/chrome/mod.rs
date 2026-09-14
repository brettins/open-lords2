//! The original's interface artwork: `Panels.pl8`, `Misc_cty.pl8` and the
//! minimap rasters in `MAPnn.PL8`.
//!
//! Everything here is decompiled — `docs/screens.md` §3 and §4 —
//! designed. Where a rectangle is invented it says so at the point of use.
//!
//! # Three sheets, three jobs
//!
//! * **`Panels.pl8`** is a framed-box kit, and its 262 frames decompose exactly:
//!   four corners, four twelve-frame edges (52 = `0x34`), a 144-frame 12 × 12
//!   interior texture (196 = `0xC4`), eight 24 × 24 strip frames (204 = `0xCC`),
//!   then a second complete border set. Every boundary lands on the offset the
//!   drawing code adds — `Ui_DrawBoxBorder` adds `0xCC` for the second set and
//!   `Ui_DrawBoxInterior` indexes `52 + c%12 + (r%12)*12`.
//! * **`Misc_cty.pl8`** is the campaign right column: seven 162-pixel-wide
//!   frames that tile `y` 24 … 480 with no gap under either of the two middle
//! layouts, plus the realm banners and the minimap furniture.
//! * **`MAPnn.PL8`** holds two 128 × 128 rasters per map slot, four slots to a
//!   file: a county id per pixel and a shading mask. The *colour* of the
//!   minimap is not in the file at all — it comes from an 8-bytes-per-realm
//!   ramp in `Lords2.exe` at `0x004D2900`, which is read out of the executable
//!   here.

mod minimap;
pub use minimap::*;
mod chrome;
pub use chrome::*;

use l2_formats::{DecodedFrame, Palette, Pl8};

use crate::canvas::{Canvas, Clip};
use crate::sheet::Sheet;

// ---------------------------------------------------------------- Panels.pl8

/// Frame indices inside `Panels.pl8`, recovered from `Ui_DrawBoxBorder` and
/// `Ui_DrawBoxInterior` and confirmed against the file's own frame table.
pub mod panels {
    /// Corners, in the order the drawing code tests for them.
    pub const CORNER_TL: usize = 0;
    pub const CORNER_TR: usize = 1;
    pub const CORNER_BR: usize = 2;
    pub const CORNER_BL: usize = 3;
    /// Each edge is twelve frames, cycled `(n - 1) % 12`.
    pub const EDGE_TOP: usize = 4;
    pub const EDGE_BOTTOM: usize = 0x10;
    pub const EDGE_LEFT: usize = 0x1C;
    pub const EDGE_RIGHT: usize = 0x28;
    pub const EDGE_LEN: usize = 12;
    /// The interior texture: a 12 × 12 grid of 16 × 16 frames.
    pub const TEXTURE: usize = 0x34;
    pub const TEXTURE_DIM: usize = 12;
    /// Eight 24 × 24 frames, tiled to make the menu bar.
    pub const STRIP: usize = 0xC4;
    pub const STRIP_LEN: usize = 8;
    /// The second complete border set. `Ui_DrawBoxBorder` reaches it by adding
    /// this to every index above.
    pub const SET_B: usize = 0xCC;
    /// One cell of every box this kit builds.
    pub const CELL: i32 = 16;
    /// One cell of the 24 × 24 strip.
    pub const STRIP_CELL: i32 = 24;
}

// -------------------------------------------------------------- Misc_bat.pl8

/// Frame indices inside `Misc_bat.pl8`, the battlefield's right column.
///
/// `g_miscCtySheet` (`0x005530C8`) holds this file, not `Misc_cty.pl8`, while a
/// battle is up — `DAT_0053F050` picks — so these indices and `misc_cty`'s are
/// two vocabularies over one slot. Each frame's own header carries the position
/// it is drawn at, and every one below agrees with its call site. **[V]**
pub mod misc_bat {
    /// The column under the overview panel, 160 × 228 at (480, 184).
    pub const COLUMN: usize = 0;
    /// The button strip, 160 × 32 at (480, 448).
    pub const BUTTONS: usize = 1;
    /// The two-count plate, 160 × 36 at (480, 412).
    pub const COUNTS: usize = 2;
    /// The lit pause button, 30 × 30, over slot 0 of the strip.
    pub const PAUSE_LIT: usize = 5;
    /// The lit retreat button, 30 × 30, over slot 1.
    pub const RETREAT_LIT: usize = 6;
    /// `shieldIndex + SHIELD` — six 28 × 35 plates, frames 7 … 12. A
    /// `shieldIndex` of 0 is clamped to 6 by the writer, so frame 6 is never
    /// reached this way and stays [`RETREAT_LIT`].
    pub const SHIELD: usize = 6;
}

// -------------------------------------------------------------- Misc_cty.pl8

/// Frame indices inside `Misc_cty.pl8` used by the campaign screen.
pub mod misc_cty {
    /// The right column, top to bottom. `PANEL_TOP` holds the minimap.
    pub const PANEL_TOP: usize = 54;
    /// The middle, when the selected county belongs to the local player.
    pub const PANEL_OWN_A: usize = 55;
    pub const PANEL_OWN_B: usize = 66;
    pub const PANEL_OWN_C: usize = 56;
    /// The middle, when it does not: one frame covering the same 274 pixels.
    pub const PANEL_FOREIGN: usize = 58;
    /// The last two strips; `PANEL_END_TURN` is the End Turn button.
    pub const PANEL_STATUS: usize = 57;
    pub const PANEL_END_TURN: usize = 59;
    /// `realmColour + 0x55`, and the colour byte is clamped to 1..=5, so the
    /// five that can be drawn are 86..=90 — the five 13 × 16 frames.
    pub const BANNER: usize = 0x55;
    /// The vertical strip beside the minimap; the second is used when an
    /// overlay mode is active.
    pub const MINIMAP_SIDE: usize = 0x5C;
    pub const MINIMAP_SIDE_ACTIVE: usize = 0x5B;

    /// The farm/industry slider's thumb, and **the same thumb inside a
    /// two-pixel blue ring** when the county has idle townsfolk.
    ///
    /// `CountyStrip_Draw`'s last branch: `labour[8].workers == 0` draws `0x3D`
    /// at `(share / 2 + 0x214, 0x106)` and anything else draws `0x55` at
    /// `(share / 2 + 0x212, 0x104)` — two pixels up and left, for a frame four
    /// pixels wider and four taller.
    pub const SPLIT_THUMB: usize = 0x3D;
    pub const SPLIT_THUMB_IDLE: usize = 0x55;

    /// **Every frame that carries the blue ring**, plain first.
    ///
    /// Five of the county strip's drawers pick between a plain icon and a
    /// ringed one on the same question — `labour[slot].useful <
    /// labour[slot].workers`, *more people on this job than it can use* — and
    /// the ringed frame is always drawn two pixels up and two left of the plain
    /// one, because it is the plain one inside a ring.
    ///
    /// | plain | ringed | what |
    /// |---|---|---|
    /// | `0x21` | `0x4B` | the sheaf — grain farming, slot 0 |
    /// | `0x26` | `0x4C` | the cow — cattle farming, slot 1 |
    /// | `0x3F` | `0x4D` | field reclamation, slot 2 |
    /// | `0x30 + n` | `0x4F + n` | the county's industry, slot 7 |
    ///
    /// **The last row is six pairs, not four.** `FUN_004106C4` indexes both
    /// frames by a county byte at `+0x290` this project has not named, and the
    /// ringed run in the file is `0x4F` … `0x54` — six frames, matching
    /// `0x30` … `0x35` — so `n` reaches 5. That is a reading of how many frames
    /// exist, not of what the byte means, and none of the six is drawn yet.
    ///
    /// **The castle is deliberately not here.** It has a ringed frame too, and
    /// it is the one that is *not* its plain twin plus a ring — see
    /// [`CASTLE_PLAIN`].
    pub const RINGED_PAIRS: [(usize, usize); 9] = [
        (0x21, 0x4B),
        (0x26, 0x4C),
        (0x3F, 0x4D),
        (0x30, 0x4F),
        (0x31, 0x50),
        (0x32, 0x51),
        (0x33, 0x52),
        (0x34, 0x53),
        (0x35, 0x54),
    ];

    /// The castle's cell on the strip, and the odd one out.
    ///
    /// `CountyStrip_DrawCastleIcon` draws `0x40` at `(0x25B, y + 300)` and
    /// `0x4E` at `(0x255, y + 0x129)` — six pixels left and three up, not the
    /// two-and-two every other pair uses, because `0x4E` is 32 × 34 against
    /// `0x40`'s 23 × 26. So the ringed castle is a **different, larger
/// picture** that also carries the ring
    /// one. Its border is blue like all the others; its geometry is its own.
    pub const CASTLE_PLAIN: usize = 0x40;
    pub const CASTLE_RINGED: usize = 0x4E;

    /// **The three industry rows that have no state at all.**
    ///
    /// `FUN_00410502` (iron), `FUN_00410598` (stone) and `FUN_0041062E` (wood)
    /// are the same nine lines three times over: one `Pl8_DrawFrame` of a fixed
    /// frame, one `Ui_DrawDelta`, and the row counter. **They have no ringed
    /// twin and no shortfall frame** — only the blacksmith and the castle, of
    /// the five industry rows, react to their staffing.
    ///
    /// The `x` differs by six pixels between the iron row and the other two and
    /// that is the original's, not a slip: iron is drawn at 600 and stone and
    /// wood at `0x252` = 594.
    ///
    /// **`docs/draws-map.md` §2 names these three painters in the wrong
    /// order** — it has `0x00410502` as stone, `0x00410598` as wood and
    /// `0x0041062E` as iron. `CountyStrip_Draw`'s dispatch settles it: the
    /// right-hand list holds *labour slots*, and slot 4 (iron mining) calls
    /// `0x00410502`, slot 5 (stone quarrying) calls `0x00410598`, slot 6 (wood
    /// cutting) calls `0x0041062E`. `Unit_TrampleTile` agrees from the other
    /// side. `docs/decisions.md` C135.
    pub const INDUSTRY_IRON: usize = 0x2C;
    pub const INDUSTRY_STONE: usize = 0x2D;
    pub const INDUSTRY_WOOD: usize = 0x2E;
    /// The x each of the three is drawn at, in the same order.
    pub const INDUSTRY_X: [i32; 3] = [600, 0x252, 0x252];

    /// **The castle row's three materials icons.**
    /// `CountyStrip_DrawCastleIcon` picks one when the castle is still owed
    /// anything, and puts `L2.eng` 71/18 — *"Needed"* — under it:
    ///
    /// ```c
    /// if (stoneOwed && woodOwed)      frame 0x1B at (0x23C, y + 0x131);
    /// else if (stoneOwed == 0)        frame 0x1A at (0x23C, y + 0x137);
    /// else                            frame 0x19 at (0x23C, y + 0x137);
    /// ```
    ///
    /// So `0x19` is the stone the county still owes, `0x1A` the wood, and
    /// `0x1B` the pair — and the pair sits **six pixels higher**, because it is
    /// the taller picture.
    pub const CASTLE_NEEDS_STONE: usize = 0x19;
    pub const CASTLE_NEEDS_WOOD: usize = 0x1A;
    pub const CASTLE_NEEDS_BOTH: usize = 0x1B;

    /// The shortfall icons, drawn when a job is **below** its wanted floor —
    /// the other end of the same test, and the only two the strip has.
    pub const SHORTFALL_GRAIN: usize = 0x23;
    pub const SHORTFALL_CATTLE: usize = 0x29;

    /// **The three palette entries the ring is made of**, in `Base01.256`:
    /// `rgb(194, 230, 255)`, `rgb(157, 202, 234)` and `rgb(0, 0, 121)`.
    ///
/// Named, because *"blue outline"* is a memory and
    /// this is a measurement: of frame `0x55`'s 124 border pixels, 124 are one
    /// of these three. `crates/l2-view/tests/install/main.rs` asserts it against the
    /// player's own file.
    pub const RING_COLOURS: [u8; 3] = [64, 65, 95];
}

// -------------------------------------------------------------- System2.pl8

/// Frame indices inside the button sheet — **`System.pl8`, not `System2.pl8`**.
///
/// `Res_LoadButtons` (`0x00499A1C`) loads `system2.pl8` for index 0 and
/// `system.pl8` for index 1 into the same 56,600-byte buffer, and the kingdom
/// screens ask for index 1 (`0x004BA3xx`, guarded on the in-game state being
/// 3). The two files are the same size and the same 84-frame layout, and the
/// difference is not cosmetic: **69 of `System2.pl8`'s 84 frames are entirely
/// index 0**, including every frame named below except [`OK`], while none of
/// `System.pl8`'s are. Loading `System2.pl8` draws the panels' arrows and
/// slider as nothing at all, which is how this was found.
///
/// Every button is a **normal/pressed pair**: `Widget_Draw` adds 1 to the frame
/// while the record's press timer at `+0x0D` is running, so the odd frame of
/// each pair is the pressed state. `docs/screens-county.md` §4.2.
pub mod system {
    /// The tax panel's up arrow, from `g_taxWidgets` (`0x004DD790`) record 0.
    pub const ARROW_UP: usize = 0x15;
    /// The tax panel's down arrow, from record 1. Note the *up* arrow is the
    /// left of the two and the *down* arrow the right — the original's order,
    /// not a transcription slip.
    pub const ARROW_DOWN: usize = 0x17;
    /// Both arrow frames are 24 × 24, which is also the widget's hit box.
    pub const ARROW: i32 = 24;
    /// `Ui_OkButton(x, y, 0)` — the picture that closes a panel.
    ///
    /// **Not a tick.** The frame decodes to a cursor arrow pointing into a
    /// small black hole: a close button whose artwork is the instruction. It is
    /// a live hotspot, and the right button closes the panel as well, so the
    /// game offers two ways out. Of `System.pl8`'s 84 frames this one is the
    /// **fourth darkest**, 15.3% near-black against a median frame's 1.2%,
    /// which is what `l2-view`'s install test asserts so that the label cannot
    /// drift back. `docs/screens-county.md` §2.7. The name is ours and is kept,
    /// because renaming a constant every reader knows costs more than the word
    /// "OK" misleads.
    pub const OK: usize = 0x33;
    /// `Ui_OkButton(x, y, 1)` — the same picture with a raised bevel round it.
    pub const OK_ALT: usize = 0x10;
    pub const OK_DIM: i32 = 24;
    /// The ration slider, from `Panel_RationSlider` (`0x00411FDE`).
    pub const SLIDER_CAP_LEFT: usize = 0x4A;
    pub const SLIDER_CAP_RIGHT: usize = 0x4B;
    pub const SLIDER_KNOB: usize = 0x4C;
    pub const SLIDER_CAP: i32 = 24;
    /// The knob is 10 × 32, so at value 0 (drawn at track − 4) its centre lands
    /// on the track's first pixel and at 100 on its last.
    pub const SLIDER_KNOB_W: i32 = 10;
}

/// Where each right-column frame goes, from `Screen_DrawCampaign` and
/// `Panel_DrawCounty`. `y` is the frame's top; the heights come from the file.
///
/// The whole column is `x` 478 … 639, `y` 24 … 479, and it tiles exactly:
/// 24 + 132 = 156, + 94 = 250, + 52 = 302, + 128 = 430, + 30 = 460, + 20 = 480,
/// with 156 + 274 = 430 for the foreign layout.
pub const PANEL_TOP_Y: i32 = 24;
pub const PANEL_MIDDLE_Y: i32 = 156;
pub const PANEL_OWN_B_Y: i32 = 250;
pub const PANEL_OWN_C_Y: i32 = 302;
pub const PANEL_STATUS_Y: i32 = 430;
pub const PANEL_END_TURN_Y: i32 = 460;

/// `Minimap_Draw(0x1E0, 0x19)` blits the picture at `(x - 2, y + 3)`.
pub const MINIMAP_X: i32 = 478;
pub const MINIMAP_Y: i32 = 28;
/// `Minimap_Click` hit-tests a *different* rectangle — `(480, 25)`, 128 × 128.
/// The disagreement is the original's; both constants are literals in the two
/// functions. See `docs/screens.md` §3.2.
pub const MINIMAP_HIT_X: i32 = 480;
pub const MINIMAP_HIT_Y: i32 = 25;
pub const MINIMAP_DIM: i32 = 128;
/// `Minimap_Draw` puts the 29 × 123 mode strip at `(x + 0x83, y + 7)` and the
/// mode badge at `(x + 5, y + 5)`, both off the hit rectangle's origin.
pub const MINIMAP_SIDE_X: i32 = MINIMAP_HIT_X + 0x83;
pub const MINIMAP_SIDE_Y: i32 = MINIMAP_HIT_Y + 7;
pub const MINIMAP_BADGE_X: i32 = MINIMAP_HIT_X + 5;
pub const MINIMAP_BADGE_Y: i32 = MINIMAP_HIT_Y + 5;

// ------------------------------------------------------------------ minimap

/// The realm colour ramp `Minimap_DrawOverlay` indexes, read out of
/// `Lords2.exe` at `0x004D2900`: eight bytes per realm colour, of which the
/// first four are used — `ramp[colour * 8 + (shade - 10)]`.
///
/// **This is the executable's data, not ours**, and it is transcribed here
///
/// game and `l2-view` may not open files. `tests/install.rs` reads the same 48
/// bytes back out of the user's own `Lords2.exe` and fails if they differ.
pub const MINIMAP_REALM_RAMP: [[u8; 4]; 6] = [
    [0x0A, 0x0B, 0x0C, 0x0D], // 0 — unowned: the shades unchanged
    [0x01, 0x0E, 0x0F, 0xF9], // 1
    [0x03, 0xF2, 0xF3, 0xFB], // 2
    [0x38, 0x35, 0x32, 0x2F], // 3
    [0x05, 0xF4, 0xF5, 0xFD], // 4
    [0x04, 0xFC, 0xF1, 0xF0], // 5
];
/// Where that table lives, so a test can go and read it.
pub const MINIMAP_REALM_RAMP_VA: u32 = 0x004D_2900;

/// The **rating** ramp the three statistic overlays index, read out of
/// `Lords2.exe` at `0x004D28F8`: six bytes, **worst first**.
///
/// ```text
/// 004d28f8  0f 15 f3 09 f1 05 | 05 05      <- this table, then two bytes of slack
/// 004d2900  0a 0b 0c 0d 20 0b 0c 0d        <- MINIMAP_REALM_RAMP row 0
/// ```
///
/// It is a *different* table from the realm ramp, not an extension of it: the
/// two are adjacent and the eight bytes between `0x004D28F8` and the realm
/// ramp's first row are this table plus two bytes nothing indexes.
///
/// **The direction is verified from the artwork.** `Misc_cty.pl8` frame 91 —
/// the strip the original swaps in beside the minimap while an overlay is up —
/// carries a six-swatch colour bar with a tick against the top swatch and a
/// cross against the bottom one, and reading its pixels down column 5 gives
/// `0x05, 0xF1, 0x09, 0xF3, 0x15, 0x0F`: **exactly this table reversed**. So
/// index 0 is the red at the bottom of the bar (bad) and index 5 the purple at
/// the top (good), which is also what the three ratings mean — band 0 is
/// "unhappy", "short of food", "short of workers".
pub const MINIMAP_RATING_RAMP: [u8; 6] = [0x0F, 0x15, 0xF3, 0x09, 0xF1, 0x05];
/// Where that table lives, so a test can go and read it.
pub const MINIMAP_RATING_RAMP_VA: u32 = 0x004D_28F8;

/// The index `Minimap_DrawOverlay` writes for the selected county's brightest
/// shade, in place of the ramp entry.
pub const MINIMAP_SELECTED: u8 = 0x20;

/// The shades the overlay recolours. Anything else in the raster is left alone.
pub const MINIMAP_SHADE_LO: u8 = 10;
pub const MINIMAP_SHADE_HI: u8 = 13;

/// **`g_realmColour` — the two palette pens a realm writes its own text in.**
/// **[V]**
///
/// A ten-byte table at `0x004DC1D0`, five `(pen, highlight)` pairs, and the
/// binary indexes it **from two bytes lower** so that the shield is 1-based:
/// `(&g_realmColour)[shieldIndex * 2]` with `g_realmColour` at `0x004DC1CE`.
/// Here it is indexed by `shield - 1`; [`realm_pen`] does the conversion.
///
/// | shield | pen | | highlight | |
/// |---|---|---|---|---|
/// | 1 | `0x0E` | rgb(170, 0, 0) — red | `0x0F` | rgb(215, 0, 0) |
/// | 2 | `0xFB` | rgb(255, 255, 0) — yellow | `0x0D` | rgb(194, 202, 113) |
/// | 3 | `0x3A` | rgb(36, 36, 36) — near black | `0x20` | rgb(255, 255, 255) |
/// | 4 | `0x05` | rgb(130, 0, 130) — magenta | `0xFD` | rgb(255, 0, 255) |
/// | 5 | `0x04` | rgb(0, 0, 130) — blue | `0xF0` | rgb(0, 146, 255) |
///
/// # It is derived from the shield, and this reproduces the derivation
///
/// The pen is stored in the save, at realm `+0x08`, and **nothing computes it
/// at draw time** — `CountyStrip_Draw` reads the byte.
/// written from this table, from the shield, by the two functions that hand a
/// realm its colour: `Realms_AssignLords` (`0x0049CAAA`) at new game and
/// `FUN_0042BA40` when a custom battle invents an opponent. Both do
///
/// ```c
/// g_realms[n].field_0x8 = (&g_realmColour)[g_realms[n].shieldIndex * 2];
/// g_realms[n].field_0x9 = (&DAT_004dc1cf)[g_realms[n].shieldIndex * 2];
/// ```
///
/// and `RefsTo` finds no other reader of the table and no other writer of the
/// field. So deriving the pen from the shield cannot get out of step with the
/// shield, which storing a second copy of it could. Checked against the user's
/// own saves: over the eleven fixture `.sav` files, realm `+0x08` equals this
/// table at `+0x0A` for **every realm in every one**, including the six where
/// realm 1 flies shield 5 — which is what separates "keyed by the shield" from
/// "keyed by the realm id".
///
/// # Why this matters more than a colour
///
/// The shield is what the **human picks**, and the AI lords take the slots that
/// are left (`Realms_AssignLords` walks 1 … 5 and takes the first unused one).
/// So no lord has a fixed colour and no realm id has one either: a player who
/// takes red makes the realm-1 slot red *in that game*. Anything that colours a
/// realm has to go through the shield, and a fixed table keyed by realm id will
/// look right in the game it was written against and wrong in the next one.
pub const REALM_PEN: [[u8; 2]; 5] =
    [[0x0E, 0x0F], [0xFB, 0x0D], [0x3A, 0x20], [0x05, 0xFD], [0x04, 0xF0]];

/// Where shield 1's pair sits, so a test can read the ten bytes back out of the
/// user's own executable. The binary's own base is this **less two**.
pub const REALM_PEN_VA: u32 = 0x004D_C1D0;

/// The pen a realm writes its own text in, or `None` outside 1 … 5.
///
/// **This does not clamp, and [`realm_colour`] does.** The clamp there is
/// `FUN_004171EE`'s, applied before using the byte as a *frame index*, where
/// A pen has an honest answer for "we do
/// not know this realm's colour", and it is not red: a zero shield that clamped
/// up to 1 would render as a plausible wrong colour and survive a canvas diff,
/// which is the failure that hid this bug. The caller falls back
/// visibly instead.
///
/// The original has no such case — it would index two bytes below the table and
/// read `0x04`, `0x02` out of `g_lordChoice`'s tail — and it cannot reach it,
/// because the lines that use the pen are drawn only for `owner != 0` and every
/// realm in play has a shield.
pub fn realm_pen(shield: u8) -> Option<u8> {
    REALM_PEN.get(shield.checked_sub(1)? as usize).map(|p| p[0])
}

/// The brighter of the pair, realm `+0x09`. Nothing in this tree draws with it
/// yet; it is here because it is the other half of the record and a reader who
/// finds one will want the other.
pub fn realm_pen_highlight(shield: u8) -> Option<u8> {
    REALM_PEN.get(shield.checked_sub(1)? as usize).map(|p| p[1])
}

// -------------------------------------------------------------------- Chrome

/// The interface sheets, loaded once.
pub struct Chrome {
    panels: Sheet,
    misc_cty: Sheet,
    /// `Misc_bat.PL8`, the other tenant of `g_miscCtySheet`. Optional because
    /// only the battle screens draw it.
    misc_bat: Option<Sheet>,
    system: Option<Sheet>,
}

/// A palette-resolved colour for a raster the chrome does not supply. Only used
/// where our own drawing has to put something down; the original's own artwork
/// never goes through it.
pub fn nearest(palette: &Palette, rgb: [u8; 3]) -> u8 {
    crate::ink::nearest(palette, rgb)
}

/// A frame's size without decoding it, for a caller laying out around it.
pub fn frame_size(sheet: &Sheet, index: usize) -> Option<(u16, u16)> {
    sheet.frame(index).map(|f: DecodedFrame| (f.width, f.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `Panels.pl8` index arithmetic, asserted as the chain of boundaries
    /// that must meet: 4 corners + 4 × 12 edges = 52, + 144 texture frames =
    /// 196, + 8 strip frames = 204 — and 204 is exactly the offset
    /// `Ui_DrawBoxBorder` adds to reach the second border set.
    #[test]
    fn the_panels_kit_partitions_its_frames_with_no_gap_and_no_overlap() {
        assert_eq!(panels::EDGE_TOP, panels::CORNER_BL + 1);
        assert_eq!(panels::EDGE_BOTTOM, panels::EDGE_TOP + panels::EDGE_LEN);
        assert_eq!(panels::EDGE_LEFT, panels::EDGE_BOTTOM + panels::EDGE_LEN);
        assert_eq!(panels::EDGE_RIGHT, panels::EDGE_LEFT + panels::EDGE_LEN);
        assert_eq!(panels::TEXTURE, panels::EDGE_RIGHT + panels::EDGE_LEN);
        assert_eq!(panels::TEXTURE, 4 + 4 * panels::EDGE_LEN);
        assert_eq!(panels::STRIP, panels::TEXTURE + panels::TEXTURE_DIM * panels::TEXTURE_DIM);
        assert_eq!(panels::SET_B, panels::STRIP + panels::STRIP_LEN);
        assert_eq!(panels::SET_B, 0xCC, "the offset Ui_DrawBoxBorder adds");
    }

    /// The right column tiles y 24..480 with no gap under either layout, and
    /// reaches the right edge of the screen. Heights are the file's; if they
    /// are ever read back wrong this stops adding up.
    #[test]
    fn the_right_panel_frames_tile_the_column_exactly() {
        // Heights of Misc_cty frames 54, 55, 66, 56, 58, 57, 59.
        let (top, own_a, own_b, own_c, foreign, status, end) = (132, 94, 52, 128, 274, 30, 20);
        assert_eq!(PANEL_TOP_Y + top, PANEL_MIDDLE_Y);
        assert_eq!(PANEL_MIDDLE_Y + own_a, PANEL_OWN_B_Y);
        assert_eq!(PANEL_OWN_B_Y + own_b, PANEL_OWN_C_Y);
        assert_eq!(PANEL_OWN_C_Y + own_c, PANEL_STATUS_Y);
        assert_eq!(PANEL_MIDDLE_Y + foreign, PANEL_STATUS_Y, "the foreign layout ends level");
        assert_eq!(PANEL_STATUS_Y + status, PANEL_END_TURN_Y);
        assert_eq!(PANEL_END_TURN_Y + end, 480, "and the column reaches the bottom");
        assert_eq!(crate::campaign::PANEL_X + crate::campaign::PANEL_W, 640);
    }

    /// The minimap exactly fills the bottom of the panel's top frame: it is
    /// drawn at y 28 and 28 + 128 = 156, which is where the next frame starts.
    #[test]
    fn the_minimap_sits_flush_with_the_bottom_of_the_panels_top_frame() {
        assert_eq!(MINIMAP_Y + MINIMAP_DIM, PANEL_MIDDLE_Y);
        assert_eq!(MINIMAP_X, crate::campaign::PANEL_X);
        //...and the hit rectangle is offset from it
// own inconsistency.
        assert_ne!((MINIMAP_X, MINIMAP_Y), (MINIMAP_HIT_X, MINIMAP_HIT_Y));
        assert_eq!(MINIMAP_HIT_X - MINIMAP_X, 2);
        assert_eq!(MINIMAP_Y - MINIMAP_HIT_Y, 3);
    }

    /// `Minimap_Load`'s file and frame arithmetic, checked against the two ends
    /// of the range: slot 0 is `Map01.pl8` frames 0/1 and slot 59 is
    /// `Map15.pl8` frames 15/16.
    #[test]
    fn a_map_slot_names_its_file_and_its_two_frames() {
        assert_eq!(Minimap::file_for_slot(0), "Map01.pl8");
        assert_eq!(Minimap::frames_for_slot(0), (0, 1));
        assert_eq!(Minimap::file_for_slot(3), "Map01.pl8");
        assert_eq!(Minimap::frames_for_slot(3), (15, 16));
        assert_eq!(Minimap::file_for_slot(4), "Map02.pl8");
        assert_eq!(Minimap::file_for_slot(59), "Map15.pl8");
        assert_eq!(Minimap::frames_for_slot(59), (15, 16));
        // The 16 empty slots 24..39 fall in map07..map10, which is exactly the
        // set of MAPnn files the game does not ship.
        for slot in 24..40 {
            let n = (slot >> 2) + 1;
            assert!((7..=10).contains(&n), "slot {slot} -> Map{n:02}");
        }
    }

    /// The overlay leaves everything,
    /// marks the selected county with the original's own 0x20.
    #[test]
    fn the_minimap_overlay_recolours_only_the_four_county_shades() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![0; n] };
        // Four shades of county 3, one sea pixel, one transparent pixel.
        for (i, s) in [10u8, 11, 12, 13, 63, 0].iter().enumerate() {
            m.shades[i] = *s;
            m.counties[i] = 3;
        }
        let mut c = Canvas::screen();
        c.clear(200);
        draw_minimap(&mut c, &m, 0, &MinimapTint::Owner(&|_| 2));

        fn at(c: &Canvas, i: i32) -> u8 {
            c.at((MINIMAP_X + i) as usize, MINIMAP_Y as usize)
        }
        assert_eq!([at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)], MINIMAP_REALM_RAMP[2]);
        assert_eq!(at(&c, 4), 63, "the sea is drawn as the raster holds it");
        assert_eq!(at(&c, 5), 200, "index 0 is transparent");

        // The selection replaces only the brightest shade.
        draw_minimap(&mut c, &m, 3, &MinimapTint::Owner(&|_| 2));
        assert_eq!(at(&c, 0), MINIMAP_SELECTED);
        assert_eq!(at(&c, 1), MINIMAP_REALM_RAMP[2][1], "the other three are untouched");
    }

    /// **A statistic overlay is one flat colour per county, not a four-step
    /// shade**, and a band of 6 leaves the raster alone.
    ///
    /// The same synthetic raster as above, drawn three ways, with the pixels
/// counted.
    #[test]
    fn a_rating_overlay_paints_one_ramp_colour_over_all_four_shades() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![0; n] };
        for (i, s) in [10u8, 11, 12, 13, 63, 0].iter().enumerate() {
            m.shades[i] = *s;
            m.counties[i] = 3;
        }
        fn at(c: &Canvas, i: i32) -> u8 {
            c.at((MINIMAP_X + i) as usize, MINIMAP_Y as usize)
        }
        let mut c = Canvas::screen();

        // Band 0 — the worst rating — is one colour across all four shades,
        // where the ownership ramp gives four different ones.
        c.clear(200);
        draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| Some(0)));
        assert_eq!(
            [at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)],
            [MINIMAP_RATING_RAMP[0]; 4]
        );
        assert_eq!(at(&c, 4), 63, "the sea is still the raster's");
        assert_eq!(at(&c, 5), 200, "index 0 is still transparent");

        // Band 5 — the best — is a different colour, so the modes are
        // distinguishable by pixel and not only by intent.
        c.clear(200);
        draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| Some(5)));
        assert_eq!([at(&c, 0), at(&c, 3)], [MINIMAP_RATING_RAMP[5]; 2]);
        assert_ne!(MINIMAP_RATING_RAMP[0], MINIMAP_RATING_RAMP[5]);

        // Band 6, and "not the player's county", both leave the raster's own
        // shade — which is exactly the unowned row of the realm ramp.
        for absent in [Some(6u8), None] {
            c.clear(200);
            draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| absent));
            assert_eq!(
                [at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)],
                MINIMAP_REALM_RAMP[0],
                "band {absent:?} must leave the picture alone"
            );
        }

        // The selected county's brightest shade is still 0x20 in a rating mode:
        // the original tests the selection before it dispatches on the mode.
        c.clear(200);
        draw_minimap(&mut c, &m, 3, &MinimapTint::Rating(&|_| Some(0)));
        assert_eq!(at(&c, 0), MINIMAP_SELECTED);
        assert_eq!(at(&c, 1), MINIMAP_RATING_RAMP[0]);
    }

    /// Every band any of the three ratings can produce is either a valid index
    /// into the six-entry ramp or the "draw nothing" 6 — **checked over the
    /// whole `u8` range**, because a six-entry table in this subsystem has run
    /// off its end once already (`docs/decisions.md` C50).
    #[test]
    fn no_rating_band_can_index_past_the_ramp() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![10; n] };
        m.counties[0] = 3;
        let mut c = Canvas::screen();
        for band in 0..=255u8 {
            c.clear(200);
            // The point is that this does not panic, and that anything outside
            // 0..=5 falls through to the raster's own pixel.
            draw_minimap(&mut c, &m, 0, &MinimapTint::Rating(&|_| Some(band)));
            let px = c.at(MINIMAP_X as usize, MINIMAP_Y as usize);
            if (band as usize) < MINIMAP_RATING_RAMP.len() {
                assert_eq!(px, MINIMAP_RATING_RAMP[band as usize]);
            } else {
                assert_eq!(px, 10, "band {band} must leave the raster's shade");
            }
        }
    }

    /// The mode buttons are not radio buttons, and the badge frames are not in
    /// order. Both are the original's.
    #[test]
    fn the_minimap_mode_buttons_map_to_modes_and_badges() {
        assert_eq!(MinimapMode::from_button(0), Some(MinimapMode::Labour));
        assert_eq!(MinimapMode::from_button(1), Some(MinimapMode::Food));
        assert_eq!(MinimapMode::from_button(2), Some(MinimapMode::Happiness));
        assert_eq!(MinimapMode::from_button(3), None, "button 4 is not a mode");
        assert_eq!(MinimapMode::default(), MinimapMode::Owner);
        assert!(!MinimapMode::Owner.is_rating());
        assert_eq!(MinimapMode::Owner.badge_frame(), None);
        assert_eq!(MinimapMode::Labour.badge_frame(), Some(0x5D));
        assert_eq!(MinimapMode::Food.badge_frame(), Some(0x5F));
        assert_eq!(MinimapMode::Happiness.badge_frame(), Some(0x5E));
    }

/// `FUN_004171EE`'s clamp, and the reason it lives here
    /// load path: colour 0 would index `Misc_cty` frame 85, which is 13 x 37
    /// and does not fit the 24-pixel menu bar, while 1..=5 land on the five
    /// 13 x 16 frames that do.
    #[test]
    fn a_realm_colour_is_clamped_to_the_five_frames_that_fit_the_bar() {
        assert_eq!(realm_colour(0), 1, "0 is not a colour");
        for c in 1..=5u8 {
            assert_eq!(realm_colour(c), c);
        }
        assert_eq!(realm_colour(6), 5);
        assert_eq!(realm_colour(255), 5);
        // Every clamped value indexes one of the banner frames 86..=90.
        for raw in 0..=255u8 {
            let f = misc_cty::BANNER + realm_colour(raw) as usize;
            assert!((86..=90).contains(&f), "raw {raw} -> frame {f}");
        }
    }

    #[test]
    fn a_minimap_click_lands_on_the_county_under_it() {
        let n = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        let mut m = Minimap { counties: vec![0; n], shades: vec![0; n] };
        m.counties[(7 * MINIMAP_DIM + 5) as usize] = 9;
        assert_eq!(m.county_at(MINIMAP_HIT_X + 5, MINIMAP_HIT_Y + 7), 9);
        assert_eq!(m.county_at(MINIMAP_HIT_X + 6, MINIMAP_HIT_Y + 7), 0);
        assert_eq!(m.county_at(MINIMAP_HIT_X - 1, MINIMAP_HIT_Y), 0, "outside is nothing");
        assert_eq!(m.county_at(MINIMAP_HIT_X, MINIMAP_HIT_Y + MINIMAP_DIM), 0);
        assert!(minimap_hit_area().contains(MINIMAP_HIT_X + 5, MINIMAP_HIT_Y + 7));
        assert!(!minimap_hit_area().contains(MINIMAP_HIT_X + MINIMAP_DIM, MINIMAP_HIT_Y));
    }
}

