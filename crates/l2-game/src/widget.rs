//! The few pieces of chrome every screen draws: a framed panel, a button, a
//! label with a number after it.
//!
//! These are drawing helpers that happen to know about [`Rect`], which is why
//! they live here rather than in `l2-view`: a rectangle is only interesting
//! because something can be clicked in it, and clicking is this crate's
//! business. Everything below writes palette indices through the ordinary
//! canvas primitives, so it is all assertable without a window.

use l2_view::text;
use l2_view::{Canvas, Ink};

use crate::input::Rect;

/// A one-pixel outline, drawn inside `rect`.
pub fn frame(canvas: &mut Canvas, rect: Rect, colour: u8) {
    canvas.fill_rect(rect.x, rect.y, rect.w, 1, colour);
    canvas.fill_rect(rect.x, rect.y + rect.h - 1, rect.w, 1, colour);
    canvas.fill_rect(rect.x, rect.y, 1, rect.h, colour);
    canvas.fill_rect(rect.x + rect.w - 1, rect.y, 1, rect.h, colour);
}

/// A filled, framed panel.
pub fn panel(canvas: &mut Canvas, ink: &Ink, rect: Rect) {
    canvas.fill_rect(rect.x, rect.y, rect.w, rect.h, ink.panel);
    frame(canvas, rect, ink.border);
}

/// A button. `focused` is the keyboard selection or the pointer hovering it —
/// the interface does not distinguish, because the player does not either.
pub fn button(canvas: &mut Canvas, ink: &Ink, rect: Rect, label: &str, focused: bool) {
    canvas.fill_rect(rect.x, rect.y, rect.w, rect.h, ink.panel);
    frame(canvas, rect, if focused { ink.highlight } else { ink.border });
    let colour = if focused { ink.highlight } else { ink.text };
    let y = rect.y + (rect.h - text::GLYPH_H) / 2;
    text::draw_centred(canvas, rect.centre_x(), y, label, colour);
}

/// `label` in the dim colour, `value` in the bright one, with the value's last
/// character landing on `right`. Numbers in a column then line up on their
/// units digit.
pub fn stat(canvas: &mut Canvas, ink: &Ink, x: i32, y: i32, right: i32, label: &str, value: &str) {
    text::draw(canvas, x, y, label, ink.dim);
    text::draw_right(canvas, right, y, value, ink.text);
}

/// The same, with the value coloured by which way it moved.
pub fn stat_delta(
    canvas: &mut Canvas,
    ink: &Ink,
    x: i32,
    y: i32,
    right: i32,
    label: &str,
    value: &str,
    delta: i32,
) {
    text::draw(canvas, x, y, label, ink.dim);
    let colour = match delta {
        0 => ink.text,
        d if d > 0 => ink.good,
        _ => ink.bad,
    };
    text::draw_right(canvas, right, y, value, colour);
}

/// A signed number with an explicit sign, which is how the original's happiness
/// panel reads: `+5`, `-2`, `0`.
pub fn signed(n: i32) -> String {
    if n > 0 {
        format!("+{n}")
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_formats::Palette;

    fn ink() -> Ink {
        // A palette where every named colour resolves to a different index, so
        // a test can tell them apart.
        let mut bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            bytes[i * 3] = (i % 64) as u8;
            bytes[i * 3 + 1] = ((i * 7) % 64) as u8;
            bytes[i * 3 + 2] = ((i * 13) % 64) as u8;
        }
        Ink::for_palette(&Palette::from_bytes(&bytes).unwrap())
    }

    #[test]
    fn a_frame_paints_its_edges_and_leaves_its_middle_alone() {
        let mut c = Canvas::new(20, 10);
        frame(&mut c, Rect::new(2, 2, 6, 4), 5);
        assert_eq!(c.count(5), 2 * 6 + 2 * (4 - 2), "two rows and two short columns");
        assert_eq!(c.at(2, 2), 5);
        assert_eq!(c.at(7, 5), 5);
        assert_eq!(c.at(4, 3), 0, "the interior is untouched");
    }

    #[test]
    fn a_focused_button_is_drawn_differently_from_an_unfocused_one() {
        let ink = ink();
        let r = Rect::new(0, 0, 60, 14);
        let mut a = Canvas::new(64, 16);
        let mut b = Canvas::new(64, 16);
        button(&mut a, &ink, r, "GO", false);
        button(&mut b, &ink, r, "GO", true);
        assert!(a.diff_count(&b) > 0, "focus must be visible");
        assert!(b.count(ink.highlight) > 0);
        assert_eq!(a.count(ink.highlight), 0);
    }

    #[test]
    fn a_delta_is_coloured_by_its_sign() {
        let ink = ink();
        let mut up = Canvas::new(120, 10);
        let mut down = Canvas::new(120, 10);
        stat_delta(&mut up, &ink, 0, 0, 110, "POP", "435", 18);
        stat_delta(&mut down, &ink, 0, 0, 110, "POP", "435", -18);
        assert!(up.count(ink.good) > 0 && up.count(ink.bad) == 0);
        assert!(down.count(ink.bad) > 0 && down.count(ink.good) == 0);
    }

    #[test]
    fn signed_numbers_carry_their_sign_except_zero() {
        assert_eq!(signed(5), "+5");
        assert_eq!(signed(0), "0");
        assert_eq!(signed(-5), "-5");
    }
}
