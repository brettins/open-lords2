//! A 5 x 7 bitmap font, and drawing a string into the indexed canvas.
//!
//! **This font is ours, not the game's.** Every glyph below is hand-authored
//! here, and it is the right thing for a debug overlay, a diagnostic line and
//! `screens/index.rs`, which is ours on purpose and says so.
//!
//! # It is the wrong thing for a screen, and this header used to say otherwise
//!
//! Until the draw-call audit it read: *"the original's glyphs live in
//! `Font_c2.pl8` and friends, and that file is an open question … so the
//! interface draws its own letters **until the real font is decoded**."*
//!
//! **The real font has been decoded.** `crates/l2-game/src/shell/font.rs` draws
//! with `Fntl2_14.pl8` and `Fntl2_22.pl8` through the 128-byte
//! character-to-frame table at `0x004D71D0` that `Glyph_Draw` (`0x00402A14`)
//! indexes, and the mapping is self-checking: the frames it sends `g`, `j`,
//! `p`, `q` and `y` to are exactly the frames in that file which are four
//! pixels taller than their neighbours.
//!
//! **`Font_c2.pl8` was never the file the game draws from**, which is why
//! waiting on it was waiting on the wrong thing twice over: `docs/audit.md`
//! records that `font_c2` does **not** appear among `Lords2.exe`'s strings at
//! all, that it shares 103 of its 108 frame records with `Fntl2_9.pl8`, and
//! that the RLE puzzle this header cited as the open question was **resolved**
//! (`docs/formats/pl8-failures.md` §5).
//!
//! That stale sentence outlived the thing it described, and the audit measured
//! what it cost: **of the marks our screen modules put on the canvas, a third
//! are still drawn in this 5 x 7 font** — whole screens of them, in
//! `castle.rs`, `diplomacy.rs`, `siege.rs`, `job.rs`, `menu.rs` and
//! `menubar.rs`, which draw *nothing at all* through the game's own artwork.
//! Those screens fetch the right `L2.eng` strings and then draw them here.
//! That is the *"placeholder shit everywhere"* a player reported, and no test
//! could see it, because every one of those screens draws the right words at
//! the right coordinate.
//!
//! **So: a caption a player is meant to read belongs in `shell::Pen`, not
//! here.** `tools/draws/screendraws.js` counts the split per module and
//! `docs/draws.md` §7 says why the count is the instrument. This font stays for
//! the things that are honestly ours — and `docs/decisions.md` CNEW-fontstale
//! records that a document promising *"until X"* keeps promising it long after
//! X, which is an argument for a counted number over a written intention.
//!
//! The shape is deliberately the same as everything else in this crate: the
//! font writes palette indices into a `Vec<u8>`, so a test asserts on pixels
//! and never on a window.

use crate::canvas::Canvas;

pub const GLYPH_W: i32 = 5;
pub const GLYPH_H: i32 = 7;
/// One blank column between glyphs.
pub const ADVANCE: i32 = GLYPH_W + 1;
/// One blank row between lines, plus one for descenders that we do not have.
pub const LINE: i32 = GLYPH_H + 2;

/// Rows top to bottom; within a row, bit 4 is the leftmost of five columns.
type Glyph = [u8; 7];

/// What an unmapped character draws: a hollow box, so a missing glyph is
/// visible rather than silently blank.
const MISSING: Glyph = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];

const BLANK: Glyph = [0; 7];

/// `(character, glyph)`, printable ASCII. Lowercase is not listed: it draws
/// the uppercase shape, which is a legibility compromise this font makes on
/// purpose rather than an omission.
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
    ('@', [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110]),
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

/// The glyph for a character. Lowercase folds to uppercase; anything else
/// draws [`MISSING`].
pub fn glyph(c: char) -> Glyph {
    let c = c.to_ascii_uppercase();
    for (g, bits) in GLYPHS {
        if *g == c {
            return *bits;
        }
    }
    MISSING
}

/// Width in pixels of a string as [`draw`] would lay it out, with no trailing
/// advance after the last glyph.
pub fn width(s: &str) -> i32 {
    let n = s.chars().count() as i32;
    if n == 0 {
        0
    } else {
        n * ADVANCE - 1
    }
}

/// Draw a string with its top-left at `(x, y)`. Returns the x the next glyph
/// would occupy, so callers can chain runs in different colours.
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

/// Draw right-aligned so that the string *ends* at `x`. Numbers in a panel
/// line up on their last digit, which is the whole reason this exists.
pub fn draw_right(canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
    draw(canvas, x - width(s), y, s, colour)
}

/// Draw centred on `x`.
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
        // 'I' is 14 pixels; the exact count is what makes this a real check.
        let expected = glyph('I').iter().map(|r| r.count_ones() as usize).sum::<usize>();
        draw(&mut c, 1, 1, "I", 9);
        assert_eq!(c.count(9), expected);
        assert_eq!(c.at(1, 1), 0, "the glyph's top-left corner is blank for 'I'");
        assert_eq!(c.at(3, 1), 9, "and its crossbar is not");
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
        // Right-aligned: nothing may be painted at or past the anchor.
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
