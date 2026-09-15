#![allow(unused_imports)]
use super::*;

use l2_formats::DecodedFrame;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// `g_glyphWidths` (`0x004D71F0`), transcribed — **all 224 bytes of it.**
///
/// **It used to be 128 bytes, and the original's is 224.[V]**
/// `Ui_DrawText` (`0x00402637`) looks up every character above `0x1F` as
/// `g_glyphWidths[c - 0x20]`, with `c` a byte and no bound, so the index runs
/// to `0xDF`; `Glyph_Draw` (`0x00402A14`) and the measure `FUN_004014F0`
/// (`0x004D71D0[c]`, the same bytes) do the same. The 96 bytes past 128 were
/// read out of the image and eleven are not zero:
///
/// * `0xA0 … 0xA7` → frames 83, 91, 95, 99, 103, 103, 0, 14. Each of the first
///   four is the frame one before the one `0x85`, `0x8D`, `0x95` and `0x97` use,
///   and `0xA6` and `0xA7` reuse `'a'` and `'o'` — **[I]** code page 437's
///   *á í ó ú ñ Ñ ª º*, which is what the neighbours `0x80 … 0x9A` are;
/// * `0xDF`, `0xE0`, `0xE1` → frame 105.
///
/// Nothing else holds the tail: no absolute address in the file points into
/// `0x004D71F1 … 0x004D72CF`, and `0x004D72D0` begins a different table. The
/// highest frame the table asks for is therefore **105**, not 104, which every
/// shipped face still holds (`Fntl2_22.pl8`, the smallest, has 106).
pub const GLYPH_MAP: [u8; 224] = [
    0, 63, 64, 0, 0, 65, 0, 74, 67, 68, 66, 70, 78, 69, 79, 77, //
    62, 53, 54, 55, 56, 57, 58, 59, 60, 61, 72, 73, 0, 71, 0, 75, //
    0, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, //
    42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 0, 99, 0, 0, 0, //
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, //
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 0, 0, 0, 0, 0, //
    103, 99, 88, 86, 83, 85, 1, 103, 90, 87, 89, 91, 94, 93, 27, 27, //
    31, 105, 105, 98, 95, 97, 102, 101, 25, 41, 47, 0, 0, 0, 0, 0, //
    84, 92, 96, 100, 104, 104, 1, 15, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 106, //
    106, 106, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// **The one per-face branch in `Glyph_Draw`: three index ranges drawn one
/// pixel higher, in `&g_fontBody` only.** **[V]**, from the instruction bytes.
///
/// ```text
/// 0x00402A8C  mov  eax, [g_blitRowCount] ; add [g_drawY], eax    ; y += record[0x0D]
/// 0x00402A97  cmp  dword [ebp+8], 0x005AF8F0                     ; font == &g_fontBody
/// 0x00402A9E  jne  0x00402B0E                                    ; any other face: no raise
/// 0x00402AA9  cmp  eax, 0x61 ; jl  …    0x00402AB7  cmp eax, 0x6D ; jg  …
/// 0x00402AC0  dec  dword [0x00591528]                            ; g_drawY -= 1
/// 0x00402ACB  cmp  eax, 0x73 ; jl  …    0x00402AD9  cmp eax, 0x77 ; jg  …
/// 0x00402AE2  dec  dword [0x00591528]
/// 0x00402AED  cmp  eax, 0x80 ; jl  …    0x00402AFD  cmp eax, 0x84 ; jg  …
/// 0x00402B08  dec  dword [0x00591528]
/// ```
///
/// `eax` is `Glyph_Draw`'s second argument zero-extended, and that argument is
/// the **index** — `Ui_DrawText` has already subtracted `0x20`. So the ranges
/// are inclusive index ranges, and in characters they are `0x81 … 0x8D`,
/// `0x93 … 0x97` and `0xA0 … 0xA4`. `0x005AF8F0` is `g_fontBody`, which
/// `Res_LoadStatic` fills from `Fntl2_14.pl8`; no other face is compared, so
/// `Fntl2_22.pl8`, `Fntl2_9.pl8`, `Font_10.pl8` and `Fnt_8.pl8` never raise.
pub const ACCENT_RAISE: [(u8, u8); 3] = [(0x61, 0x6D), (0x73, 0x77), (0x80, 0x84)];

pub fn accent_raised(c: char) -> bool {
    let code = c as u32;
    if code < GLYPH_MAP_BASE as u32 {
        return false;
    }
    let index = code - GLYPH_MAP_BASE as u32;
    ACCENT_RAISE.iter().any(|&(lo, hi)| index >= lo as u32 && index <= hi as u32)
}

fn in_table(c: char) -> bool {
    let code = c as u32;
    code >= GLYPH_MAP_BASE as u32 && ((code - GLYPH_MAP_BASE as u32) as usize) < GLYPH_MAP.len()
}

impl Font {
    pub fn new(bytes: Vec<u8>, line: i32) -> Result<Font, String> {
        let sheet = Sheet::new(bytes).map_err(|e| e.to_string())?;
        Ok(Font { sheet, line, raises_accents: false })
    }

    /// This font, as `&g_fontBody`: [`ACCENT_RAISE`]'s characters one pixel
    /// higher. `Glyph_Draw` (`0x00402A14`) decides by the font *pointer*, so it
    /// is a property of which global a file was loaded into and not of the
    /// file — `crate::shell::ShellAssets::load` gives it to `Fntl2_14.pl8`.
    pub fn raising_accents(mut self) -> Font {
        self.raises_accents = true;
        self
    }

    pub fn raises_accents(&self) -> bool {
        self.raises_accents
    }

    fn lift(&self, c: char) -> i32 {
        if self.raises_accents && accent_raised(c) {
            -1
        } else {
            0
        }
    }

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

    /// How wide a string **measures** — `FUN_004014F0`, which is what
    /// centring uses, and not what drawing advances. **[V]**
    ///
    /// Four for a space, the glyph's frame width plus one for a glyph, and
    /// **nothing for any other character whose table entry is zero** — `'@'`
    /// above all, the blank sign column. [`Font::draw`] advances four over
    /// `'@'` (`Ui_DrawText`, `0x00402637`) and this charges it nothing, so the
    /// two disagree by four per `'@'`
    /// trailing space either: the four pixels `Ui_DrawText` adds at the end go
    /// into `g_penAdvance`, not into the measure.
    pub fn width(&self, s: &str) -> i32 {
        s.chars()
            .map(|c| match self.glyph(c) {
                Some(f) => f.width as i32 + 1,
                None if c == ' ' || !in_table(c) => SPACE_ADVANCE,
                None => 0,
            })
            .sum()
    }

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

    pub fn draw(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, style: &Style) -> i32 {
        let mut pen = x;
        for c in s.chars() {
            let Some(frame) = self.glyph(c) else {
                pen += SPACE_ADVANCE;
                continue;
            };
            let y = y + self.lift(c);
            if let Some((up, down)) = style.shadow {
                Font::blit_mask(canvas, &frame, pen, y - 1, up);
                Font::blit_mask(canvas, &frame, pen, y + 1, down);
            }
            Font::blit_mask(canvas, &frame, pen, y, style.colour_of(c));
            pen += frame.width as i32 + 1;
        }
        pen - x
    }

    /// Draw `s` the way `Ui_DrawText` does under `g_dropShadow` — each glyph
    /// once at `(x + 1, y + 1)` in [`DROP_SHADOW_COLOUR`], then once at `(x, y)`
    /// in `colour`. Returns the pen advance, like [`Font::draw`]. **[V]**, see
    /// [`DROP_SHADOW_COLOUR`] for the arm and the eight painters that take it.
    pub fn draw_dropped(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
        let mut pen = x;
        for c in s.chars() {
            let Some(frame) = self.glyph(c) else {
                pen += SPACE_ADVANCE;
                continue;
            };
            let y = y + self.lift(c);
            Font::blit_mask(canvas, &frame, pen + 1, y + 1, DROP_SHADOW_COLOUR);
            Font::blit_mask(canvas, &frame, pen, y, colour);
            pen += frame.width as i32 + 1;
        }
        pen - x
    }

    /// `FUN_004025D7`: centred inside `width`, and **never left of `x`** — the
    /// original clamps the offset at zero,
    /// starts at the box's left edge and runs out of it
    /// centred off the other side.
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

