//! **The real font has been decoded.** `crates/l2-game/src/shell/font/mod.rs` draws
//! with `Fntl2_14.pl8` and `Fntl2_22.pl8` through the 128-byte
//! character-to-frame table at `0x004D71D0` that `Glyph_Draw` (`0x00402A14`)
//! indexes, and the mapping is self-checking: the frames it sends `g`, `j`,
//! `p`, `q` and `y` to are exactly the frames in that file which are four
//! pixels taller than their neighbours.
//!
//! Those screens fetch the right `L2.eng` strings and then draw them here.
//!
//! **So: a caption a player is meant to read belongs in `shell::Pen`, not
//! here.** `tools/draws/screendraws.js` counts the split per module and
//! `docs/draws.md` §7 says why the count is the instrument. This font stays for
//! the things that are honestly ours — and `docs/decisions.md` C107
//! records that a document promising *"until X"* keeps promising it long after
//! X, which is an argument for a counted number over a written intention.

use crate::canvas::Canvas;

pub const GLYPH_W: i32 = 5;
pub const GLYPH_H: i32 = 7;
pub const ADVANCE: i32 = GLYPH_W + 1;
pub const LINE: i32 = GLYPH_H + 2;

type Glyph = [u8; 7];

const MISSING: Glyph = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];

const BLANK: Glyph = [0; 7];

const GLYPHS: &[(char, Glyph)] = &[
    (' ', BLANK),
    ('!', [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]),
    ('"', [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
    ('#', [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010]),
    ('$', [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100]),
    ('%', [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011]),
    ('&', [0b01000, 0b10100, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101]),
    ('\'', [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
    ('(', [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
    (')', [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
    ('*', [0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000]),
    ('+', [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
    (',', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000]),
    ('-', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
    ('.', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100]),
    ('/', [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000]),
    ('0', [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
    ('1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('2', [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
    ('3', [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
    ('4', [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
    ('5', [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
    ('6', [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
    ('7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    ('8', [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
    ('9', [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
    (':', [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000]),
    (';', [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b01000]),
    ('<', [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
    ('=', [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000]),
    ('>', [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000]),
    ('?', [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100]),
    // The original's fonts map it to nothing (`g_glyphWidths[0x20] == 0`) and
    // `Ui_DrawText` (`0x00402637`) advances four pixels over it and paints
    // nothing, so `Ui_DrawCount` (`0x0041AB67`) opens every count with one to
    // hold a sign column. This font used to draw a real at-sign here, which
    // meant a `Pen` could not pass the original's own lead without printing
    // `@1000 Crowns.` on an install with no `Fntl2_*.pl8`. It keeps its
    // column — `width` and `draw` still advance it — and loses its picture.
    ('@', BLANK),
    ('A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('B', [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110]),
    ('C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
    ('D', [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
    ('E', [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b11111]),
    ('F', [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b10000]),
    ('G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111]),
    ('H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('J', [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
    ('K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
    ('L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
    ('M', [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
    ('N', [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
    ('O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
    ('R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
    ('S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
    ('T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('V', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
    ('W', [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
    ('X', [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
    ('Y', [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
    ('[', [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110]),
    ('\\', [0b10000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00001]),
    (']', [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110]),
    ('^', [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000]),
    ('_', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]),
    ('|', [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
];

pub fn glyph(c: char) -> Glyph {
    let c = c.to_ascii_uppercase();
    for (g, bits) in GLYPHS {
        if *g == c {
            return *bits;
        }
    }
    MISSING
}

pub fn width(s: &str) -> i32 {
    let n = s.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * ADVANCE - 1
    }
}

pub fn draw(canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
    let mut cx = x;
    for c in s.chars() {
        let g = glyph(c);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..GLYPH_W {
                if bits & (1 << (GLYPH_W - 1 - col)) != 0 {
                    let px = cx + col;
                    let py = y + row as i32;
                    if px >= 0 && py >= 0 {
                        canvas.set(px as usize, py as usize, colour);
                    }
                }
            }
        }
        cx += ADVANCE;
    }
    cx
}

pub fn draw_right(canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
    draw(canvas, x - width(s), y, s, colour)
}

pub fn draw_centred(canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
    draw(canvas, x - width(s) / 2, y, s, colour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_fits_the_five_pixel_cell() {
        for (c, g) in GLYPHS {
            for (row, bits) in g.iter().enumerate() {
                assert_eq!(
                    bits & !0b11111,
                    0,
                    "{c:?} row {row} sets a bit outside the 5-pixel cell"
                );
            }
        }
    }

    #[test]
    fn the_table_holds_no_duplicate_characters() {
        let mut seen: Vec<char> = GLYPHS.iter().map(|(c, _)| *c).collect();
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n, "a duplicated character would shadow one of the two");
    }

    #[test]
    fn a_drawn_string_paints_the_glyph_and_nothing_else() {
        let mut c = Canvas::new(20, 10);
        let expected = glyph('I').iter().map(|r| r.count_ones() as usize).sum::<usize>();
        draw(&mut c, 1, 1, "I", 9);
        assert_eq!(c.count(9), expected);
        assert_eq!(c.at(1, 1), 0, "the glyph's top-left corner is blank for 'I'");
        assert_eq!(c.at(3, 1), 9, "and its crossbar is not");
    }

    /// **`'@'` is the game's blank sign column: it holds one cell and paints
    /// nothing.** `Ui_DrawCount` (`0x0041AB67`) opens every count with it, so a
    /// `Pen` that passes the original's lead must not print an at-sign on an
    /// install with no `Fntl2_*.pl8`.
    #[test]
    fn the_at_sign_holds_a_column_and_paints_nothing() {
        assert_eq!(glyph('@'), BLANK);
        let mut with_lead = Canvas::new(40, 10);
        let end = draw(&mut with_lead, 0, 0, "@1", 5);
        assert_eq!(end, 2 * ADVANCE, "the lead still advances one cell");
        let mut shifted = Canvas::new(40, 10);
        draw(&mut shifted, ADVANCE, 0, "1", 5);
        assert!(shifted.count(5) > 0, "the probe digit draws something");
        assert_eq!(with_lead.pixels, shifted.pixels, "'@' painted, or moved the digit");
    }

    #[test]
    fn lowercase_draws_the_uppercase_shape_and_unknowns_draw_a_box() {
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph('\u{80}'), MISSING);
        assert_ne!(glyph('~'), BLANK, "an unmapped printable is visible, not invisible");
    }

    #[test]
    fn text_is_laid_out_left_right_and_centred_around_the_same_width() {
        let mut c = Canvas::new(100, 20);
        assert_eq!(width(""), 0);
        assert_eq!(width("AB"), 2 * ADVANCE - 1);

        draw_right(&mut c, 50, 0, "AB", 3);
        for y in 0..20 {
            for x in 50..100 {
                assert_eq!(c.at(x, y), 0, "painted at {x},{y}, past the right anchor");
            }
        }
        let mut d = Canvas::new(100, 20);
        draw_centred(&mut d, 50, 0, "AB", 3);
        assert_eq!(d.count(3), c.count(3), "the same string, the same ink");
    }

    #[test]
    fn drawing_clips_rather_than_panicking_at_the_edges() {
        let mut c = Canvas::new(8, 8);
        draw(&mut c, -20, -20, "HELLO", 4);
        draw(&mut c, 200, 200, "HELLO", 4);
        assert_eq!(c.count(4), 0);
        draw(&mut c, -2, 0, "H", 4);
        assert!(c.count(4) > 0, "a partly visible glyph still paints its visible half");
    }
}
