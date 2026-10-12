//! The first page of *Multiple players*: Host a game, Join a game, Back (C255);
//! C263's Continue a saved game.

use l2_view::Canvas;

use super::draw::{self, Heading, Look, Pens, BIG_H, BIG_W, BUTTON_H, BUTTON_W, LEFT, WIDTH};
use super::Item;
use crate::input::Rect;
use crate::screen::Ctx;
use crate::shell::font;

/// `L2.eng` group 11 index 3, *"Multiple players"*, page 1's own item.
pub(super) const HEADING: (usize, usize) = (11, 3);
const HOST_Y: i32 = 0x7C;
const JOIN_Y: i32 = 0xBC;
const RESUME_Y: i32 = 0xF8;
const BACK_Y: i32 = 0x130;
const CAPTION: i32 = BIG_H + 4;
const HOST_CAPTION: &str = "You choose the map; friends join with a code or your address.";
const JOIN_CAPTION: &str = "Type the code the host gave you.";

pub(super) const ITEMS: [Item; 4] = [Item::Host, Item::Join, Item::Resume, Item::Back];

pub(super) fn rects() -> Vec<Rect> {
    let big_x = LEFT + (WIDTH - BIG_W) / 2;
    vec![
        Rect::new(big_x, HOST_Y, BIG_W, BIG_H),
        Rect::new(big_x, JOIN_Y, BIG_W, BIG_H),
        Rect::new(LEFT + (WIDTH - BUTTON_W) / 2, RESUME_Y, BUTTON_W, BUTTON_H),
        Rect::new(LEFT + (WIDTH - BUTTON_W) / 2, BACK_Y, BUTTON_W, BUTTON_H),
    ]
}

pub(super) fn draw(ctx: &Ctx, canvas: &mut Canvas, pens: &Pens, selected: usize) {
    draw::backdrop(ctx, canvas, pens, Heading::Eng(HEADING.0, HEADING.1));
    let rects = rects();
    for (i, (item, r)) in ITEMS.iter().zip(&rects).enumerate() {
        let look = if i == selected { Look::Selected } else { Look::Plain };
        if r.h == BIG_H {
            draw::big_button(canvas, pens, *r, item.label(), look);
        } else {
            draw::button(canvas, &pens.body, *r, item.label(), look);
        }
    }
    for (r, caption) in rects.iter().zip([HOST_CAPTION, JOIN_CAPTION]) {
        pens.body.body_centred(canvas, LEFT, r.y + CAPTION, WIDTH, caption, font::TEXT);
    }
}
