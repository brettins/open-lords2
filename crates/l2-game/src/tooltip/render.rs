#![allow(unused_imports)]
use super::*;
use super::lookup::*;
use l2_view::Canvas;
use crate::screen::{Ctx, ScreenId};
use crate::shell::{font, Pen};
use crate::Game;

/// **Our transcription of group 220**, for an install whose `L2.eng` cannot be
/// read. `CLAUDE.md` rule 6: the player's own file is drawn, and this is the
/// fallback. `tests/tooltips.rs` holds it against the file, string for string.
pub const TEXT: [&str; COUNT] = [
    "Null tool tip",
    "Kingdom view. Click on a county.",
    "Labour, red if needed, purple if idle.",
    "Ration status",
    "Overall happiness",
    "Overview map",
    "View population report",
    "View happiness report",
    "Set tax rate",
    "Set rations",
    "Adjust labor allocation",
    "Create an army",
    "Go to treasury",
    "Send supplies",
    "End your turn",
    "Cattle, and change next season",
    "Wheat, and change next season",
    "Seasons left to reclaim a field",
    "Wood produced next season",
    "Stone produced next season",
    "Iron produced next season",
    "Weapons produced. Click for smithy.",
    "Seasons left to build castle",
    "Battle overview. Click to go to area.",
    "Selected troops. Click to deselect.",
    "Overall troop levels",
    "Pause the battle",
    "Retreat from field",
    "Lower castle drawbridge",
    "Mop up enemy troops",
    "Autocalculate battle or siege results",
    "Return census map to empire mode",
    "Build or update a castle",
    "Diplomatic initiatives",
    "The health of the county",
];

/// `FUN_0040328E(0xDC, id, …)`'s string: the player's `L2.eng`, and ours only
/// where the file gave nothing.
pub fn words(shell: &crate::shell::ShellAssets, id: u8) -> String {
    let s = shell.text(GROUP, id as usize);
    if s.is_empty() {
        TEXT.get(id as usize).copied().unwrap_or("").to_string()
    } else {
        s.to_string()
    }
}


/// `FUN_004015B9(c, &g_fontBody)`.
fn glyph_width(ctx: &Ctx, c: char) -> i32 {
    let s = c.to_string();
    match &ctx.assets.shell.body {
        Some(f) => f.width(&s),
        None => l2_view::text::width(&s),
    }
}

pub fn layout(ctx: &Ctx, tip: Shown) -> (crate::input::Rect, Vec<String>) {
    let text = words(&ctx.assets.shell, tip.id);
    let measured = crate::message::break_lines(&text, MEASURE_WIDTH, |c| glyph_width(ctx, c));
    let pen = pen(ctx);
    let widest = measured
        .iter()
        .map(|l| {
            let mut scratch = Canvas::new(1, 1);
            pen.body(&mut scratch, 0, 0, l, INK)
        })
        .max()
        .unwrap_or(0);
    let (w, h) = box_size(measured.len(), widest);
    let lines = crate::message::break_lines(&text, DRAW_WIDTH, |c| glyph_width(ctx, c));
    (crate::input::Rect::new(tip.x, tip.y, w, h), lines)
}

fn pen<'a>(ctx: &'a Ctx) -> Pen<'a> {
    // `DAT_005AEA40 = 1` around both passes: flat text, no emboss.
    Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: None,
        caps: None,
    }
}

/// **`FUN_00476E95`'s draw half**, over whatever the frame painted. Nothing
/// when the option is off or no tip is up.
pub fn draw(ctx: &Ctx, tips: &Tooltips, canvas: &mut Canvas) {
    if !ctx.game.prefs.tool_tips {
        return;
    }
    let Some(tip) = tips.shown() else { return };
    let pen = pen(ctx);
    let text = words(&ctx.assets.shell, tip.id);
    let (tx, ty) = (tip.x + 4, tip.y + 4);
    let measured = crate::message::break_lines(&text, MEASURE_WIDTH, |c| glyph_width(ctx, c));
    for (k, line) in measured.iter().enumerate() {
        pen.body(canvas, tx, ty + LINE * k as i32, line, font::TEXT);
    }
    let (rect, lines) = layout(ctx, tip);
    canvas.fill_rect(rect.x, rect.y, rect.w, rect.h, FILL);
    for (k, line) in lines.iter().enumerate() {
        pen.body(canvas, tx, ty + LINE * k as i32, line, font::TEXT);
    }
    outline(canvas, rect.x, rect.y, rect.w, rect.h, INK);
}

/// **`FUN_00403CF4` (`0x00403CF4`)** — a one-pixel rectangle, its origin and
/// extent clipped to the screen first.
fn outline(canvas: &mut Canvas, mut x: i32, mut y: i32, mut w: i32, mut h: i32, c: u8) {
    let (sw, sh) = (canvas.width as i32, canvas.height as i32);
    if x < 1 {
        x = 0;
    }
    if sw <= w + x {
        w = sw - x;
    }
    if y < 1 {
        y = 0;
    }
    if sh <= h + y {
        h = sh - y;
    }
    canvas.fill_rect(x, y, w, 1, c);
    canvas.fill_rect(x, y + h - 1, w, 1, c);
    canvas.fill_rect(x, y, 1, h, c);
    canvas.fill_rect(x + w - 1, y, 1, h, c);
}

