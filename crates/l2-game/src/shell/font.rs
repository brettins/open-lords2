//! The original's fonts, drawn the way the original draws them.
//!
//! `l2_view::text` is *our* 5 × 7 font and says so. It is the right thing for a
//! debug overlay and the wrong thing for a screen whose whole claim is that it
//! shows what the game showed, so this module draws with `Fntl2_14.pl8` and
//! `Fntl2_22.pl8` — the two fonts `docs/screens-county.md` §4 calls *"every
//! panel body line"* and *"every panel heading"*.
//!
//! # A font is a PL8 plus one 128-byte table
//!
//! **[V]** `Glyph_Draw` (`0x00402A14`) is four lines of arithmetic:
//!
//! ```text
//! frame = g_glyphWidths[ch] - 1        // ch = c - 0x20; 0 means no glyph
//! y    += frameRecord[0x0D]            // the glyph's own vertical offset
//! advance = frame.width + 1
//! ```
//!
//! and `FUN_004014F0`, which measures a string, indexes the *same* table from
//! `0x004D71D0` with the raw character — `0x004D71F0 - 0x20`, the same bytes.
//! A space is never looked up at all: it advances 4 and draws nothing.
//!
//! The table is therefore a **character-to-frame map**, not the widths its
//! name in `docs/symbols.md` suggests; the widths come from the PL8's own frame
//! records. The name is kept because it is the one in `symbols.json`.
//!
//! The mapping is self-checking, which is what makes it **[V]** rather than
//! **[D]**: it sends `'a'` to frame 0 and `'A'` to frame 26 in `Fntl2_14.pl8`,
//! and the frames it sends `'g'`, `'j'`, `'p'`, `'q'`, `'y'` to are exactly the
//! frames in that file that are four pixels taller than their neighbours.
//! Descenders land on the descending letters. Nothing else would.
//!
//! # Every string is drawn three times — except when it is drawn once
//!
//! `Ui_DrawText` (`0x00402637`) draws each glyph at `y - 1` in one shadow
//! colour, at `y + 1` in another, and then at `y` in the real one. Text on
//! these screens is embossed, not flat, and a reimplementation that draws it
//! once looks subtly wrong everywhere at the same time.
//!
//! The two shadow colours are `0x10` and `0x1F` — **except** on the setup
//! screen (`g_screenId == 0x1F`) and the conquest screen (`0x1C`), where the
//! same function uses `0x36` and `0x2C` instead. Those two screens run under
//! `gateway.256` rather than the campaign palette, so the indices that read as
//! shadow are different ones. That branch is at the top of `Ui_DrawText` and is
//! the only thing in it that knows which screen it is on.
//!
//! **[D]** and then two globals switch it off again:
//!
//! * `DAT_005AEA40` — when non-zero the whole emboss is skipped and the glyph
//!   is drawn once, at `y`, in its own colour. The front end sets it around
//!   every menu item, every button caption and every body line, and clears it
//!   for the heading. So on those pages **the heading is embossed and nothing
//!   else is**, which is not what a reimplementation would guess.
//! * `DAT_0058FE2C` — when non-zero, characters `0x41 … 0x5A`, which is `A`
//!   through `Z` and nothing else, are drawn in **colour 1** instead of the
//!   caller's. The setup pages set it around their heading and the conquest
//!   screen sets it around everything it draws. It is a drop-capital effect
//!   applied to every capital in the line.
//!
//! [`Style`] carries both, because a font that cannot say "flat, capitals in
//! colour 1" cannot draw the game's own title screen.

use l2_formats::DecodedFrame;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// `g_glyphWidths` (`0x004D71F0`), transcribed.
///
/// One byte per character from `0x20`, giving `frame + 1`; zero means the
/// character has no glyph and advances [`SPACE_ADVANCE`]. This is the
/// executable's data, not ours, and it is a constant here for the same reason
/// `l2_view::chrome::MINIMAP_REALM_RAMP` is: it is a constant of the game, and
/// this crate should not have to open `Lords2.exe` to draw a letter.
/// `tests/install.rs` reads the same bytes back out of the user's own copy.
pub const GLYPH_MAP: [u8; 128] = [
    0, 63, 64, 0, 0, 65, 0, 74, 67, 68, 66, 70, 78, 69, 79, 77, //
    62, 53, 54, 55, 56, 57, 58, 59, 60, 61, 72, 73, 0, 71, 0, 75, //
    0, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, //
    42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 0, 99, 0, 0, 0, //
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, //
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 0, 0, 0, 0, 0, //
    103, 99, 88, 86, 83, 85, 1, 103, 90, 87, 89, 91, 94, 93, 27, 27, //
    31, 105, 105, 98, 95, 97, 102, 101, 25, 41, 47, 0, 0, 0, 0, 0,
];

/// Where that table lives, so a test can go and read it.
pub const GLYPH_MAP_VA: u32 = 0x004D_71F0;

/// The character the table starts at.
pub const GLYPH_MAP_BASE: u8 = 0x20;

/// What a character with no glyph advances. `FUN_004014F0` special-cases
/// `' '` before the table lookup and adds 4; `Glyph_Draw` adds nothing at all
/// for a zero entry, which is what makes `'@'` an invisible sign column that
/// still occupies its place in a column of numbers.
pub const SPACE_ADVANCE: i32 = 4;

/// The two shadow colours `Ui_DrawText` embosses with, on every screen but two.
pub const SHADOW: (u8, u8) = (0x10, 0x1F);

/// The pair it uses instead when `g_screenId` is `0x1C` or `0x1F` — the
/// conquest screen and the setup pages, which run under `gateway.256`.
pub const SHADOW_GATEWAY: (u8, u8) = (0x36, 0x2C);

/// The colour the setup and conquest painters pass for ordinary text.
pub const TEXT: u8 = 0x3F;
/// The colour they pass for the highlighted item of a menu.
pub const HIGHLIGHT: u8 = 0xF9;
/// The colour they pass for an item that is present but not available.
pub const DISABLED: u8 = 0x20;

/// How a string is drawn: the colour, whether it is embossed, and whether its
/// capitals get `Ui_DrawText`'s drop-capital colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub colour: u8,
    /// The emboss pair, or `None` for flat — `DAT_005AEA40 != 0`.
    pub shadow: Option<(u8, u8)>,
    /// The colour `A` … `Z` are drawn in instead — `DAT_0058FE2C != 0`, which
    /// always means colour 1.
    pub caps: Option<u8>,
}

impl Style {
    /// Embossed the ordinary way, no drop capitals.
    pub fn new(colour: u8) -> Style {
        Style { colour, shadow: Some(SHADOW), caps: None }
    }

    /// The colour one character is actually drawn in.
    fn colour_of(&self, c: char) -> u8 {
        match self.caps {
            Some(k) if c.is_ascii_uppercase() => k,
            _ => self.colour,
        }
    }
}

/// One of the original's fonts.
pub struct Font {
    sheet: Sheet,
    /// The height the game lays lines out on. Not read out of the file — the
    /// painters position every line absolutely — but useful for a shell that
    /// has to place a line the original never drew.
    pub line: i32,
}

/// The file names of the two fonts the management screens use.
pub const BODY: &str = "Fntl2_14.pl8";
pub const HEADING: &str = "Fntl2_22.pl8";
/// `Fntl2_9.pl8`, used by the county strip and nothing else.
pub const SMALL: &str = "Fntl2_9.pl8";

impl Font {
    pub fn new(bytes: Vec<u8>, line: i32) -> Result<Font, String> {
        let sheet = Sheet::new(bytes).map_err(|e| e.to_string())?;
        Ok(Font { sheet, line })
    }

    /// The frame for a character.
    ///
    /// **`Glyph_Draw`'s `y += frameRecord[0x0D]` is already in the frame.** The
    /// decoder reserves those rows at the top of the canvas and puts the
    /// rectangle below them, so a glyph canvas is positioned by its own top
    /// edge and this function has nothing left to add. It used to add the count
    /// a second time, which cost every `0x0D = 3` glyph in `Fntl2_14.pl8` —
    /// `a c e m n o s u x z` and the descenders — three pixels of drop, while
    /// `b d f h i k l t` and `?` stayed put because their count is zero. That
    /// is the exact split a player reported off a screenshot.
    ///
    /// `None` for a character the map sends nowhere: a space, or one of the
    /// punctuation marks the font simply does not have.
    fn glyph(&self, c: char) -> Option<DecodedFrame> {
        let code = c as u32;
        if code < GLYPH_MAP_BASE as u32 {
            return None;
        }
        let slot = (code - GLYPH_MAP_BASE as u32) as usize;
        let entry = *GLYPH_MAP.get(slot)?;
        if entry == 0 {
            return None;
        }
        self.sheet.frame(entry as usize - 1)
    }

    /// How wide a string draws. `FUN_004014F0`: four for a space, else the
    /// glyph's frame width plus one, and **no trailing space** — the four
    /// pixels `Ui_DrawText` adds go into `g_penAdvance`, not into the measure
    /// that centring uses.
    pub fn width(&self, s: &str) -> i32 {
        s.chars()
            .map(|c| match self.glyph(c) {
                Some(f) => f.width as i32 + 1,
                None => SPACE_ADVANCE,
            })
            .sum()
    }

    /// The height of the tallest glyph in a string, for laying out a line the
    /// original placed by hand. The frame's own height already counts the rows
    /// reserved above the rectangle, so there is nothing to add to it.
    pub fn height(&self, s: &str) -> i32 {
        s.chars()
            .filter_map(|c| self.glyph(c).map(|f| f.height as i32))
            .max()
            .unwrap_or(self.line)
    }

    /// One glyph, as a mask: every pixel the frame paints is written in
    /// `colour`, because the font sheets carry a shape and the caller carries
    /// the colour (`Glyph_Draw` blits through `DAT_0057D3BC`).
    fn blit_mask(canvas: &mut Canvas, frame: &DecodedFrame, x: i32, y: i32, colour: u8) {
        for row in 0..frame.height as i32 {
            for col in 0..frame.width as i32 {
                let i = (row * frame.width as i32 + col) as usize;
                if !frame.opaque[i] {
                    continue;
                }
                let (px, py) = (x + col, y + row);
                if px < 0 || py < 0 || px >= canvas.width as i32 || py >= canvas.height as i32 {
                    continue;
                }
                canvas.set(px as usize, py as usize, colour);
            }
        }
    }

    /// Draw `s` at `(x, y)`. Returns the pen advance, which is how the original
    /// lays a value out after a label without either knowing the other's width.
    pub fn draw(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, style: &Style) -> i32 {
        let mut pen = x;
        for c in s.chars() {
            let Some(frame) = self.glyph(c) else {
                pen += SPACE_ADVANCE;
                continue;
            };
            // The order the original draws in: above, below, then the real one
            // on top of both. The glyph's own vertical offset is inside the
            // frame — see `glyph` — so all three share `y` and nothing else.
            if let Some((up, down)) = style.shadow {
                Font::blit_mask(canvas, &frame, pen, y - 1, up);
                Font::blit_mask(canvas, &frame, pen, y + 1, down);
            }
            Font::blit_mask(canvas, &frame, pen, y, style.colour_of(c));
            pen += frame.width as i32 + 1;
        }
        pen - x
    }

    /// `FUN_004025D7`: centred inside `width`, and **never left of `x`** — the
    /// original clamps the offset at zero, so a string too long for its box
    /// starts at the box's left edge and runs out of it rather than being
    /// centred off the other side.
    ///
    pub fn draw_centred(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        width: i32,
        s: &str,
        style: &Style,
    ) -> i32 {
        let offset = ((width - self.width(s)) / 2).max(0);
        self.draw(canvas, x + offset, y, s, style)
    }

    /// Right-aligned so the last pixel lands on `right`.
    pub fn draw_right(
        &self,
        canvas: &mut Canvas,
        right: i32,
        y: i32,
        s: &str,
        style: &Style,
    ) -> i32 {
        self.draw(canvas, right - self.width(s), y, s, style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_sends_letters_where_the_font_files_say_it_does() {
        // 'a' is frame 0 and 'A' is frame 26 - the two anchors that make the
        // descender check below meaningful.
        assert_eq!(GLYPH_MAP[('a' as usize) - 0x20], 1);
        assert_eq!(GLYPH_MAP[('A' as usize) - 0x20], 27);
        assert_eq!(GLYPH_MAP[('z' as usize) - 0x20], 26);
        assert_eq!(GLYPH_MAP[('Z' as usize) - 0x20], 52);
        // '0' is the *last* digit frame, not the first: '1'..'9' are 53..61 and
        // '0' is 62, which is how the file lays its digit strip out.
        assert_eq!(GLYPH_MAP[('1' as usize) - 0x20], 53);
        assert_eq!(GLYPH_MAP[('9' as usize) - 0x20], 61);
        assert_eq!(GLYPH_MAP[('0' as usize) - 0x20], 62);
    }

    #[test]
    fn a_space_and_an_at_sign_have_no_glyph() {
        assert_eq!(GLYPH_MAP[0], 0, "' '");
        assert_eq!(GLYPH_MAP[('@' as usize) - 0x20], 0, "'@' - the blank sign column");
        assert_eq!(GLYPH_MAP[('$' as usize) - 0x20], 0);
    }

    #[test]
    fn the_map_covers_every_character_from_0x20() {
        assert_eq!(GLYPH_MAP.len(), 128);
        assert_eq!(GLYPH_MAP_BASE, 0x20);
        // The highest frame any character asks for. Fntl2_14.pl8 has 108
        // frames and Fntl2_22.pl8 has 106, so nothing here can be out of range
        // in the body font and only the accented tail could be in the heading.
        assert_eq!(GLYPH_MAP.iter().copied().max().unwrap(), 105);
    }
}
