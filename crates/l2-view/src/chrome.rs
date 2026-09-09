//! The original's interface artwork: `Panels.pl8`, `Misc_cty.pl8` and the
//! minimap rasters in `MAPnn.PL8`.
//!
//! Everything here is decompiled — `docs/screens.md` §3 and §4 — rather than
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
//!   layouts, plus the realm banners and the minimap furniture.
//! * **`MAPnn.PL8`** holds two 128 × 128 rasters per map slot, four slots to a
//!   file: a county id per pixel and a shading mask. The *colour* of the
//!   minimap is not in the file at all — it comes from an 8-bytes-per-realm
//!   ramp in `Lords2.exe` at `0x004D2900`, which is read out of the executable
//!   here rather than invented.

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
    /// picture** that also carries the ring, rather than the same one inside
    /// one. Its border is blue like all the others; its geometry is its own.
    pub const CASTLE_PLAIN: usize = 0x40;
    pub const CASTLE_RINGED: usize = 0x4E;

    /// The shortfall icons, drawn when a job is **below** its wanted floor —
    /// the other end of the same test, and the only two the strip has.
    pub const SHORTFALL_GRAIN: usize = 0x23;
    pub const SHORTFALL_CATTLE: usize = 0x29;

    /// **The three palette entries the ring is made of**, in `Base01.256`:
    /// `rgb(194, 230, 255)`, `rgb(157, 202, 234)` and `rgb(0, 0, 121)`.
    ///
    /// Named rather than described, because *"blue outline"* is a memory and
    /// this is a measurement: of frame `0x55`'s 124 border pixels, 124 are one
    /// of these three. `crates/l2-view/tests/install.rs` asserts it against the
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

// ------------------------------------------------------------------ minimap

/// The two 128 × 128 rasters `Minimap_Load` reads for one map slot.
pub struct Minimap {
    /// The county id under each minimap pixel. `Minimap_Click` reads this.
    pub counties: Vec<u8>,
    /// The shading mask. Only 0, 10..=13 and 63 occur in the shipped files:
    /// 10..=13 are land inside a county and get recoloured per owner, 63 is
    /// drawn as-is, and 0 is transparent.
    pub shades: Vec<u8>,
}

impl Minimap {
    /// `Minimap_Load`: `"map01.pl8" + (slot >> 2) * 0x10`, i.e. four map slots
    /// per file. The 11 files the game ships cover exactly the 44 used slots.
    pub fn file_for_slot(slot: usize) -> String {
        format!("Map{:02}.pl8", (slot >> 2) + 1)
    }

    /// The two frame indices for a slot: `(slot & 3) * 5` and `+ 1`.
    pub fn frames_for_slot(slot: usize) -> (usize, usize) {
        let base = (slot & 3) * 5;
        (base, base + 1)
    }

    pub fn load(bytes: &[u8], slot: usize) -> Result<Minimap, String> {
        let pl8 = Pl8::parse(bytes).map_err(|e| e.to_string())?;
        let (c, s) = Minimap::frames_for_slot(slot);
        let counties = pl8.decode(c).map_err(|e| e.to_string())?;
        let shades = pl8.decode(s).map_err(|e| e.to_string())?;
        let want = (MINIMAP_DIM * MINIMAP_DIM) as usize;
        if counties.indices.len() != want || shades.indices.len() != want {
            return Err(format!(
                "slot {slot}: frames {c}/{s} are {}x{} and {}x{}, not 128x128",
                counties.width, counties.height, shades.width, shades.height
            ));
        }
        Ok(Minimap { counties: counties.indices, shades: shades.indices })
    }

    /// The county at a minimap pixel, indexed as `Minimap_Click` does.
    pub fn county_at(&self, x: i32, y: i32) -> u8 {
        let (dx, dy) = (x - MINIMAP_HIT_X, y - MINIMAP_HIT_Y);
        if dx < 0 || dy < 0 || dx >= MINIMAP_DIM || dy >= MINIMAP_DIM {
            return 0;
        }
        self.counties[(dy * MINIMAP_DIM + dx) as usize]
    }
}

/// The realm colour ramp `Minimap_DrawOverlay` indexes, read out of
/// `Lords2.exe` at `0x004D2900`: eight bytes per realm colour, of which the
/// first four are used — `ramp[colour * 8 + (shade - 10)]`.
///
/// **This is the executable's data, not ours**, and it is transcribed here
/// rather than looked up at run time because the value is a constant of the
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

/// The index `Minimap_DrawOverlay` writes for the selected county's brightest
/// shade, in place of the ramp entry.
pub const MINIMAP_SELECTED: u8 = 0x20;

/// The shades the overlay recolours. Anything else in the raster is left alone.
pub const MINIMAP_SHADE_LO: u8 = 10;
pub const MINIMAP_SHADE_HI: u8 = 13;

/// Clamp a realm's raw colour byte the way `FUN_004171EE` does before using it
/// as a frame index: 0 becomes 1, anything above 5 becomes 5.
///
/// The clamp belongs here and **not** on the load path. Realm `+0x0A` is stored
/// raw so that a misread offset reads back as zero and fails a test; clamping
/// on load would turn every realm into a plausible-looking colour 1.
pub fn realm_colour(raw: u8) -> u8 {
    raw.clamp(1, 5)
}

// -------------------------------------------------------------------- Chrome

/// The interface sheets, loaded once.
pub struct Chrome {
    panels: Sheet,
    misc_cty: Sheet,
    system: Option<Sheet>,
}

impl Chrome {
    /// `Panels.pl8` is preload entry 10 and is always resident; `Misc_cty.pl8`
    /// is loaded with the campaign map's tile sets.
    ///
    /// The button sheet is **`System.pl8`, not `System2.pl8`** — see [`system`].
    /// It is optional where the other two are not, because a caller that only
    /// draws boxes should not be stopped by a missing button sheet.
    pub fn load<F>(mut read: F) -> Result<Chrome, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let panels = Sheet::new(read("Panels.pl8")?).map_err(|e| format!("Panels.pl8: {e}"))?;
        let misc_cty =
            Sheet::new(read("Misc_cty.pl8")?).map_err(|e| format!("Misc_cty.pl8: {e}"))?;
        let system = read("System.pl8")
            .or_else(|_| read("System2.pl8"))
            .ok()
            .and_then(|b| Sheet::new(b).ok());
        Ok(Chrome { panels, misc_cty, system })
    }

    pub fn panels(&self) -> &Sheet {
        &self.panels
    }

    pub fn misc_cty(&self) -> &Sheet {
        &self.misc_cty
    }

    pub fn system(&self) -> Option<&Sheet> {
        self.system.as_ref()
    }

    /// One `System2.pl8` frame. Returns false when the sheet is absent or the
    /// frame will not decode, so the caller can draw its own button rather
    /// than draw nothing.
    pub fn draw_system(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        match self.system.as_ref() {
            Some(s) => {
                let f = match s.frame(frame) {
                    Some(f) => f,
                    None => return false,
                };
                canvas.blit(&f, x, y);
                true
            }
            None => false,
        }
    }

    fn blit(&self, canvas: &mut Canvas, sheet: &Sheet, frame: usize, x: i32, y: i32) -> bool {
        match sheet.frame(frame) {
            Some(f) => {
                canvas.blit(&f, x, y);
                true
            }
            None => false,
        }
    }

    /// One `Misc_cty` frame at a position. Returns false if the frame is
    /// missing, so a caller can fall back rather than draw nothing silently.
    pub fn draw_misc(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        self.blit(canvas, &self.misc_cty, frame, x, y)
    }

    pub fn draw_panel_frame(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        self.blit(canvas, &self.panels, frame, x, y)
    }

    /// `Ui_DrawTileStrip`: `Panels.pl8` frames 196 + (c mod 8) at 24-pixel
    /// steps. `Screen_DrawMenuBar` draws 25 cells from x 0 and then 2 more from
    /// x 592, which covers 0 … 639 with an 8-pixel overlap.
    pub fn draw_strip(&self, canvas: &mut Canvas, x: i32, y: i32, cells: i32) {
        for c in 0..cells {
            let frame = panels::STRIP + (c as usize % panels::STRIP_LEN);
            self.draw_panel_frame(canvas, frame, x + c * panels::STRIP_CELL, y);
        }
    }

    /// The 640 × 24 menu bar background, exactly as `Screen_DrawMenuBar` lays
    /// it out. The text on top of it is the caller's business.
    pub fn draw_menu_bar_background(&self, canvas: &mut Canvas) {
        self.draw_strip(canvas, 0, 0, 25);
        self.draw_strip(canvas, 0x250, 0, 2);
    }

    /// `Ui_DrawBox`: a bordered box `cols` × `rows` cells of 16 pixels.
    ///
    /// `set` picks the border: 0 is frames 0..51, 1 adds `0xCC` for the second
    /// set. The interior is the 12 × 12 texture, tiled from the box's own
    /// origin so two boxes side by side do not seam.
    ///
    /// **The offset applies to the border and not to the interior.** `set` is
    /// `Ui_DrawBoxBorder`'s first argument and `Ui_DrawBoxInterior` has no such
    /// argument at all — `FUN_004093E0` is literally
    /// `Ui_DrawBoxBorder(1, …); Ui_DrawBoxInterior(x + 0x10, y + 0x10, …)`.
    /// Adding 0xCC to an interior index sends `0x34 + n` past 255 and into the
    /// five 13 × 16 banner frames at the end of the file, which is what it did
    /// the first time anybody drew a box in set 1: the custom-game screen's
    /// twelve option boxes came out full of shields.
    pub fn draw_box(&self, canvas: &mut Canvas, x: i32, y: i32, cols: i32, rows: i32, set: usize) {
        let base = if set == 0 { 0 } else { panels::SET_B };
        let cell = panels::CELL;
        for r in 0..rows {
            for c in 0..cols {
                let (px, py) = (x + c * cell, y + r * cell);
                let interior = r > 0 && r < rows - 1 && c > 0 && c < cols - 1;
                let frame = if r == 0 && c == 0 {
                    panels::CORNER_TL
                } else if r == 0 && c == cols - 1 {
                    panels::CORNER_TR
                } else if r == rows - 1 && c == 0 {
                    panels::CORNER_BL
                } else if r == rows - 1 && c == cols - 1 {
                    panels::CORNER_BR
                } else if r == 0 {
                    panels::EDGE_TOP + (c as usize - 1) % panels::EDGE_LEN
                } else if r == rows - 1 {
                    panels::EDGE_BOTTOM + (c as usize - 1) % panels::EDGE_LEN
                } else if c == 0 {
                    panels::EDGE_LEFT + (r as usize - 1) % panels::EDGE_LEN
                } else if c == cols - 1 {
                    panels::EDGE_RIGHT + (r as usize - 1) % panels::EDGE_LEN
                } else {
                    panels::TEXTURE
                        + (c as usize - 1) % panels::TEXTURE_DIM
                        + ((r as usize - 1) % panels::TEXTURE_DIM) * panels::TEXTURE_DIM
                };
                let frame = if interior { frame } else { frame + base };
                self.draw_panel_frame(canvas, frame, px, py);
            }
        }
    }

    /// The campaign right column. `own` picks between the two middle layouts,
    /// which is the only thing `Panel_DrawCounty` varies about the frames
    /// themselves. Returns how many of the frames were actually drawn.
    pub fn draw_right_panel(&self, canvas: &mut Canvas, own: bool) -> usize {
        use misc_cty::*;
        let x = crate::campaign::PANEL_X;
        let mut drawn = 0;
        let mut go = |f: usize, y: i32| {
            if self.draw_misc(canvas, f, x, y) {
                drawn += 1;
            }
        };
        go(PANEL_TOP, PANEL_TOP_Y);
        if own {
            go(PANEL_OWN_A, PANEL_MIDDLE_Y);
            go(PANEL_OWN_B, PANEL_OWN_B_Y);
            go(PANEL_OWN_C, PANEL_OWN_C_Y);
        } else {
            go(PANEL_FOREIGN, PANEL_MIDDLE_Y);
        }
        go(PANEL_STATUS, PANEL_STATUS_Y);
        go(PANEL_END_TURN, PANEL_END_TURN_Y);
        drawn
    }

    /// The realm banners in the menu bar: `Misc_cty` frame `colour + 0x55` at
    /// `x = 270 + 16i, y = 4`, one per realm that is still in play.
    pub fn draw_banner(&self, canvas: &mut Canvas, slot: i32, colour: u8) -> bool {
        self.draw_misc(canvas, misc_cty::BANNER + colour as usize, 270 + slot * 16, 4)
    }
}

/// Draw the minimap: the shading raster recoloured per county owner, exactly as
/// `Minimap_DrawOverlay` does it.
///
/// `owner` maps a county id to its realm colour. A shade outside 10..=13 is
/// left as the raster holds it, which is how the sea (63) and the transparent
/// surround (0) survive; the selected county's brightest shade becomes
/// [`MINIMAP_SELECTED`].
pub fn draw_minimap(
    canvas: &mut Canvas,
    minimap: &Minimap,
    selected: u8,
    owner: &dyn Fn(u8) -> u8,
) {
    for y in 0..MINIMAP_DIM {
        for x in 0..MINIMAP_DIM {
            let i = (y * MINIMAP_DIM + x) as usize;
            let shade = minimap.shades[i];
            let county = minimap.counties[i];
            let (px, py) = (MINIMAP_X + x, MINIMAP_Y + y);
            if shade == 0 {
                // Index 0 is transparent: the blitter leaves the panel behind.
                continue;
            }
            if !(MINIMAP_SHADE_LO..=MINIMAP_SHADE_HI).contains(&shade) {
                canvas.set(px as usize, py as usize, shade);
                continue;
            }
            let step = (shade - MINIMAP_SHADE_LO) as usize;
            let ink = if step == 0 && county != 0 && county == selected {
                MINIMAP_SELECTED
            } else {
                let colour = owner(county).min(MINIMAP_REALM_RAMP.len() as u8 - 1);
                MINIMAP_REALM_RAMP[colour as usize][step]
            };
            canvas.set(px as usize, py as usize, ink);
        }
    }
}

/// The rectangle a minimap click is tested against — `Minimap_Click`'s, not the
/// picture's.
pub const fn minimap_hit_area() -> Clip {
    Clip::new(
        MINIMAP_HIT_X,
        MINIMAP_HIT_Y,
        MINIMAP_HIT_X + MINIMAP_DIM,
        MINIMAP_HIT_Y + MINIMAP_DIM,
    )
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
        // ...and the hit rectangle is offset from it, which is the original's
        // own inconsistency rather than ours.
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

    /// The overlay leaves everything that is not a county shade alone, and
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
        draw_minimap(&mut c, &m, 0, &|_| 2);

        fn at(c: &Canvas, i: i32) -> u8 {
            c.at((MINIMAP_X + i) as usize, MINIMAP_Y as usize)
        }
        assert_eq!([at(&c, 0), at(&c, 1), at(&c, 2), at(&c, 3)], MINIMAP_REALM_RAMP[2]);
        assert_eq!(at(&c, 4), 63, "the sea is drawn as the raster holds it");
        assert_eq!(at(&c, 5), 200, "index 0 is transparent");

        // The selection replaces only the brightest shade.
        draw_minimap(&mut c, &m, 3, &|_| 2);
        assert_eq!(at(&c, 0), MINIMAP_SELECTED);
        assert_eq!(at(&c, 1), MINIMAP_REALM_RAMP[2][1], "the other three are untouched");
    }

    /// `FUN_004171EE`'s clamp, and the reason it lives here rather than on the
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
