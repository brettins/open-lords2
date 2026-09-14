#![allow(unused_imports)]
use super::*;
use super::minimap::*;
use l2_formats::{DecodedFrame, Palette, Pl8};
use crate::canvas::{Canvas, Clip};
use crate::sheet::Sheet;

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
        // `Misc_bat.PL8` is the battle's tenant of `g_miscCtySheet`; optional
        // for the same reason `System.pl8` is.
        let misc_bat = read("Misc_bat.PL8").ok().and_then(|b| Sheet::new(b).ok());
        Ok(Chrome { panels, misc_cty, misc_bat, system })
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
/// missing, so a caller can fall back.
    pub fn draw_misc(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        self.blit(canvas, &self.misc_cty, frame, x, y)
    }

    /// One `Misc_bat.PL8` frame — the battlefield's half of `g_miscCtySheet`.
    /// False when the install does not ship the file, so the caller keeps its
    /// own chrome. Frame names in [`misc_bat`].
    pub fn draw_misc_bat(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        match self.misc_bat.as_ref() {
            Some(s) => self.blit(canvas, s, frame, x, y),
            None => false,
        }
    }

    pub fn misc_bat(&self) -> Option<&Sheet> {
        self.misc_bat.as_ref()
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

    /// The 640 × 24 menu bar background
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
    /// # Set 2 is a box with **no top rail**, and it is the menu drop-down's
    ///
    /// `Ui_DrawBoxBorder`'s style is not only a frame offset. Style 2 changes
    /// the *shape*, in two places and nowhere else — read back off `0x00409934`:
    ///
    /// ```c
    /// if (r == 0 && c == 0)         frame = style == 2 ? 0x1C : 0;  /* left edge, not a corner */
    /// if (r == 0 && c == cols - 1)  frame = style == 2 ? 0x28 : 1;  /* right edge, not a corner */
    /// if (r == 0 && style != 2)     frame = 4 + (c - 1) % 12;       /* the top rail: skipped */
    /// ```
    ///
    /// and its one caller, `FUN_00409429`, completes the picture by filling the
    /// interior **from the box's own `y`**, one row taller than `Ui_DrawBox`:
    ///
    /// ```c
    /// Ui_DrawBoxBorder(2, x, y, cols, rows);
    /// Ui_DrawBoxInterior(x + 0x10, y, cols - 2, rows - 1);   /* not y + 0x10, not rows - 2 */
    /// ```
    ///
    /// So the drop-down's parchment reaches the menu bar it hangs from and its
    /// left and right edges run all the way up. We drew set 1's artwork for it —
    /// a **closed** box with a full 16-pixel top rail at `y = 24` — and the
    /// first caption's origin is `y = 38`, fourteen pixels down. A player read
    /// the result off the screen exactly:
    ///
    /// > *"the text in the menus at the top when I open the menu is slightly
    /// > high — it's clipping into the 'fold' of the scroll at the top, and
    /// > there's a bit of empty space from the last bit of text to the bottom
    /// > 'fold' of the scroll"*
    ///
    /// **Both halves are this one difference, and neither is a row-pitch
    /// error.** The item pitch is a data column in `g_menuBarItems`' item tables
    /// — 0, 20, 40, … — which `screens::menubar` already carries, and every
    /// caption is where `Eng_DrawString(group, index, x + 0x10, item.y + y +
    /// 0x20, …)` puts it. What moved was the plate around them: a rail that
    /// should not exist ate the top of the block, and the sixteen pixels it
    /// occupied are what make the space under the last row look unbalanced.
    ///
    /// `screens::menubar` was asking for set 2 all along and saying so in its
    /// header — *"our `Pen::window` only models two of the original's three
/// border sets, so set 2 draws with set 1's artwork. Recorded
    /// faked."* The record was accurate, it was in the file, and it did not
    /// cause the work to happen: `docs/agents.md`, *a correct explanation
    /// sitting directly above the omission it describes*.
    pub fn draw_box(&self, canvas: &mut Canvas, x: i32, y: i32, cols: i32, rows: i32, set: usize) {
        let base = if set == 0 { 0 } else { panels::SET_B };
        let open_top = set == 2;
        let cell = panels::CELL;
        for r in 0..rows {
            for c in 0..cols {
                let (px, py) = (x + c * cell, y + r * cell);
                // **Where the interior starts, which is what set 2 moves.**
                // `Ui_DrawBox` insets it a cell in both directions;
                // `FUN_00409429` insets it horizontally only and makes it one
                // row taller, so row 0's middle cells are parchment, not rail.
                let interior = if open_top {
                    r < rows - 1 && c > 0 && c < cols - 1
                } else {
                    r > 0 && r < rows - 1 && c > 0 && c < cols - 1
                };
                if interior {
                    // The texture's own row index: the interior's first row,
                    // not the box's second.
                    let tr = if open_top { r as usize } else { r as usize - 1 };
                    let frame = panels::TEXTURE
                        + (c as usize - 1) % panels::TEXTURE_DIM
                        + (tr % panels::TEXTURE_DIM) * panels::TEXTURE_DIM;
                    self.draw_panel_frame(canvas, frame, px, py);
                    continue;
                }
                let frame = if r == 0 && c == 0 {
                    if open_top {
                        panels::EDGE_LEFT
                    } else {
                        panels::CORNER_TL
                    }
                } else if r == 0 && c == cols - 1 {
                    if open_top {
                        panels::EDGE_RIGHT
                    } else {
                        panels::CORNER_TR
                    }
                } else if r == rows - 1 && c == 0 {
                    panels::CORNER_BL
                } else if r == rows - 1 && c == cols - 1 {
                    panels::CORNER_BR
                } else if r == 0 {
                    // `if (r == 0 && style != 2)` — style 2 has no top rail at
                    // all, and the cell belongs to the interior above.
                    if open_top {
                        continue;
                    }
                    panels::EDGE_TOP + (c as usize - 1) % panels::EDGE_LEN
                } else if r == rows - 1 {
                    panels::EDGE_BOTTOM + (c as usize - 1) % panels::EDGE_LEN
                } else if c == 0 {
                    panels::EDGE_LEFT + (r as usize - 1) % panels::EDGE_LEN
                } else if c == cols - 1 {
                    panels::EDGE_RIGHT + (r as usize - 1) % panels::EDGE_LEN
                } else {
                    continue;
                };
                self.draw_panel_frame(canvas, frame + base, px, py);
            }
        }
    }

    /// The campaign right column. `own` picks between the two middle layouts,
    /// which is the only thing `Panel_DrawCounty` varies about the frames
/// themselves. Returns how many of the frames were drawn.
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

    /// The 29 × 123 strip beside the minimap, at `Minimap_Draw`'s
    /// `(x + 0x83, y + 7)` = **(611, 32)**.
    ///
    /// Frame `0x5C` is the four buttons — peasant, food, heart, magnifier — and
    /// frame `0x5B` replaces it while an overlay is up: the six-swatch colour
    /// bar of [`MINIMAP_RATING_RAMP`] with a tick at the good end and a cross at
    /// the bad one, over a single button. That the strip loses three of its four
    /// buttons is not decoration: `Minimap_ModeButton` ignores buttons 1…3 once
    /// a mode is on, and only button 4 — now the only one drawn — turns it off.
    pub fn draw_minimap_side(&self, canvas: &mut Canvas, mode: MinimapMode) -> bool {
        let frame = if mode.is_rating() {
            misc_cty::MINIMAP_SIDE_ACTIVE
        } else {
            misc_cty::MINIMAP_SIDE
        };
        self.draw_misc(canvas, frame, MINIMAP_SIDE_X, MINIMAP_SIDE_Y)
    }

    /// The mode badge in the minimap's top-left corner, at `Minimap_Draw`'s
    /// `(x + 5, y + 5)` = **(485, 30)**. Nothing is drawn in mode 0.
    pub fn draw_minimap_badge(&self, canvas: &mut Canvas, mode: MinimapMode) -> bool {
        match mode.badge_frame() {
            Some(f) => self.draw_misc(canvas, f, MINIMAP_BADGE_X, MINIMAP_BADGE_Y),
            None => false,
        }
    }
}

