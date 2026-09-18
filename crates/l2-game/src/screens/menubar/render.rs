#![allow(unused_imports)]
use super::*;
use super::items::*;
use super::dropdown::*;
use super::tests_part::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::options::Page;
use crate::screens::saveload::Mode;
use crate::shell::font;

pub fn draw_titles(ctx: &Ctx, canvas: &mut Canvas, open: Option<usize>) {
    let ink = &ctx.assets.ink;
    let t = titles(ctx);
    for (i, r) in t.iter().enumerate() {
        let caption = title_text(ctx, i);
        let lit = open == Some(i);
        if lit {
            // `g_spriteWidth = (textWidth + 4) / 16 + 2; g_spriteHeight = 0x12;
            // FUN_004B414A(x - 2, y - 3, 0x3F)` — the plate the open title
            // stands on, two left and three up, in the strip's own black.
            //
            // **`g_spriteWidth` is in sixteen-pixel units**, so the plate is
            // rounded out to a whole number of cells and is always at least 32
            // pixels wider than the word. This drew `w + 4`
            // until `FUN_004B414A`'s body was read.
            let cells = (r.w + 4) / PLATE_CELL + 2;
            canvas.fill_rect(
                r.x + TITLE_PLATE_DX,
                r.y + TITLE_PLATE_DY,
                cells * PLATE_CELL,
                TITLE_PLATE_H,
                font::TEXT,
            );
        }
        let colour = if lit { PICKED_INK } else { font::TEXT };
        match ctx.assets.shell.body.as_ref() {
            Some(f) => {
                f.draw(canvas, r.x, r.y, &caption, &font::Style::new(colour));
            }
            None => {
                let c = if lit { ink.background } else { ink.text };
                text::draw(canvas, r.x, r.y + 2, &caption, c);
            }
        }
    }
}

