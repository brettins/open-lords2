
use l2_view::Canvas;

use crate::shell::font::Style;
use crate::shell::Pen;

pub const ID: &str = env!("L2_BUILD_ID");

const X: i32 = 4;
const BOTTOM_MARGIN: i32 = 2;

/// the 51-screen draw audit's headline finding was that six modules wrongly drew
/// in the debug font (`docs/decisions.md` C107), and this is the one element on
/// the page that has no business drawing in the game's.
pub fn draw(canvas: &mut Canvas, pen: &Pen) {
    let s = format!("BUILD {ID}");
    let y = top_edge(pen, &s);
    match &pen.assets.small {
        Some(f) => {
            f.draw(canvas, X, y, &s, &Style { colour: pen.ink.dim, shadow: None, caps: None });
        }
        None => {
            l2_view::text::draw(canvas, X, y, &s, pen.ink.dim);
        }
    }
}

pub fn top_edge(pen: &Pen, s: &str) -> i32 {
    let h = match &pen.assets.small {
        Some(f) => f.height(s),
        None => l2_view::text::GLYPH_H,
    };
    l2_view::canvas::HEIGHT as i32 - BOTTOM_MARGIN - h
}
