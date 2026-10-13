//! C259: on a sharp canvas the body face's text is drawn a second time, in
//! `Fntl2_22.pl8` at 2x on the 3x plane, spaced by ink. The 1x draw still
//! happens beneath, so layout, measures and draw audits are unchanged.

use super::*;
use l2_formats::DecodedFrame;
use l2_view::sharp::SCALE;
use l2_view::Canvas;

const S: i32 = SCALE as i32;
/// C259: body's 1 px gap, tripled; 2 px when 3 would outrun the 1x run.
pub const GAP: i32 = 3;
pub const GAP_TIGHT: i32 = 2;
pub const WORD: i32 = 10;

#[derive(Clone, Copy)]
pub(super) enum Anchor {
    Left,
    Mid,
    Right,
}

#[derive(Clone, Copy)]
pub(super) enum Ink<'a> {
    Style(&'a Style),
    Dropped(u8),
}

/// C259's lifts in heading rows, as the owner set them on the tuner.
pub fn lift(c: char) -> i32 {
    match c {
        '.' => 3,
        '?' => 2,
        '!' | '\'' => 1,
        ':' => -1,
        '0'..='9' => 1,
        _ => 0,
    }
}

struct Placed {
    frame: DecodedFrame,
    from: i32,
    left: i32,
    dx: i32,
    c: char,
}

fn ink_cols(frame: &DecodedFrame, from: i32) -> Option<(i32, i32)> {
    let w = frame.width as i32;
    let cols = (0..w).filter(|&col| (from..frame.height as i32).any(|row| frame.opaque[(row * w + col) as usize]));
    let v: Vec<i32> = cols.collect();
    Some((*v.first()?, *v.last()?))
}

fn layout(face: &Font, s: &str, gap: i32) -> (Vec<Placed>, i32) {
    let (mut out, mut pen, mut end) = (Vec::new(), 0, 0);
    for c in s.chars() {
        let Some(frame) = face.glyph(c) else {
            pen += WORD;
            continue;
        };
        let from = face.overhang(c, &frame);
        let (left, right) = ink_cols(&frame, from).unwrap_or((0, frame.width as i32 - 1));
        out.push(Placed { frame, from, left, dx: pen, c });
        end = pen + (right - left + 1) * 2;
        pen = end + gap;
    }
    (out, end.max(pen - gap).max(0))
}

fn blit2x(canvas: &mut Canvas, p: &Placed, x: i32, y: i32, colour: u8) {
    let w = p.frame.width as i32;
    for row in p.from..p.frame.height as i32 {
        for col in 0..w {
            if !p.frame.opaque[(row * w + col) as usize] {
                continue;
            }
            let (px, py) = (x + (col - p.left) * 2, y + row * 2);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                canvas.sharp_set(px + dx, py + dy, colour);
            }
        }
    }
}

pub(super) fn draw(body: &Font, face: &Font, canvas: &mut Canvas, x: i32, y: i32, s: &str, ink: Ink, anchor: Anchor) -> i32 {
    let adv1: i32 = s.chars().map(|c| body.glyph(c).map_or(SPACE_ADVANCE, |f| f.width as i32 + 1)).sum();
    let (mut placed, mut w3) = layout(face, s, GAP);
    if w3 > adv1 * S {
        (placed, w3) = layout(face, s, GAP_TIGHT);
    }
    let sx = x * S
        + match anchor {
            Anchor::Left => 0,
            Anchor::Mid => (adv1 * S - w3) / 2,
            Anchor::Right => adv1 * S - w3,
        };
    let (x0, x1) = (x.min(sx / S) - 2, (x + adv1).max((sx + w3) / S + 1) + 2);
    let (y0, y1) = (y - 4, y + body.line.max(16) + 8);
    canvas.sharp_seed(x0, y0, x1, y1);
    let advance = match ink {
        Ink::Style(style) => body.draw_plain(canvas, x, y, s, style),
        Ink::Dropped(colour) => body.dropped_plain(canvas, x, y, s, colour),
    };
    canvas.sharp_claim(x0, y0, x1, y1);
    for p in &placed {
        let (gx, gy) = (sx + p.dx, y * S - lift(p.c) * 2);
        match ink {
            Ink::Style(style) => {
                if let Some((up, down)) = style.shadow {
                    blit2x(canvas, p, gx, gy - 2, up);
                    blit2x(canvas, p, gx, gy + 2, down);
                }
                blit2x(canvas, p, gx, gy, style.colour_of(p.c));
            }
            Ink::Dropped(colour) => {
                blit2x(canvas, p, gx + 2, gy + 2, DROP_SHADOW_COLOUR);
                blit2x(canvas, p, gx, gy, colour);
            }
        }
    }
    advance
}
