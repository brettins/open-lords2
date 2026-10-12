//! The lobby pages' furniture: page 7's backdrop and window, the setup pages'
//! recessed buttons (`FUN_00403EE4`), a plate with a caret. `[D]` ours (C255).

use l2_view::Canvas;

use crate::input::Rect;
use crate::screen::Ctx;
use crate::screens::setup::SetupPage;
use crate::shell::{self, font, Pen};
use crate::text::{FontMetrics, TextField};

/// `Ui_DrawBoxStyle1` (`0x004093E0`) at (0x60, 0x40), 0x1C by 0x14 cells: x 96..544, y 64..384.
pub(super) const LOBBY_WINDOW: (i32, i32, i32, i32) = (0x60, 0x40, 0x1C, 0x14);
pub(super) const LEFT: i32 = 0x80;
pub(super) const WIDTH: i32 = 0x180;
pub(super) const HEAD_Y: i32 = 0x50;
/// `FUN_00403EE4(x, y, 192, 24)`, the setup pages' item, label 5 below the top.
pub(super) const BUTTON_W: i32 = 0xC0;
pub(super) const BUTTON_H: i32 = 0x18;
pub(super) const ITEM_TEXT: i32 = 5;
pub(super) const BIG_W: i32 = 0x120;
pub(super) const BIG_H: i32 = 0x28;
/// `[D]` `Custom.256` index 0xFA is (0, 63, 0), the palette's one pure green.
pub(super) const TICK: u8 = 0xFA;
/// `Panels2.pl8` frames 0xCD, 0xCF, ... 0xD5: page 4's five shields (`FUN_0041F1DD`).
const BOX_SHEET: &str = "Panels2.pl8";
pub(super) const SHIELD_W: i32 = 60;
pub(super) const SHIELD_H: i32 = 65;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    Plain,
    Selected,
    Disabled,
}

impl Look {
    /// `DAT_00553114`'s pick as the setup pages paint it: 0xF9 over 0x3F.
    pub(super) fn ink(self) -> u8 {
        match self {
            Look::Plain => font::TEXT,
            Look::Selected => font::HIGHLIGHT,
            Look::Disabled => font::DISABLED,
        }
    }
}

pub(super) struct Pens<'a> {
    pub head: Pen<'a>,
    pub body: Pen<'a>,
}

pub(super) fn pens<'a>(ctx: &'a Ctx) -> Pens<'a> {
    let head = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW_GATEWAY),
        caps: Some(1),
    };
    Pens { head, body: head.flat() }
}

pub(super) enum Heading<'a> {
    Eng(usize, usize),
    Text(&'a str),
}

pub(super) fn backdrop(ctx: &Ctx, canvas: &mut Canvas, pens: &Pens, heading: Heading) {
    if !shell::background(canvas, &ctx.assets.shell, SetupPage::Custom.background()) {
        canvas.clear(ctx.assets.ink.background);
    }
    let (x, y, cols, rows) = LOBBY_WINDOW;
    pens.body.window(canvas, x, y, cols, rows, 1);
    match heading {
        Heading::Eng(g, i) => pens.head.eng_heading_centred(canvas, g, i, x, HEAD_Y, cols * 16, font::TEXT),
        Heading::Text(s) => {
            pens.head.heading_centred(canvas, x, HEAD_Y, cols * 16, s, font::TEXT);
        }
    }
}

pub(super) fn button(canvas: &mut Canvas, pen: &Pen, r: Rect, label: &str, look: Look) {
    shell::button_recess(canvas, r.x, r.y, r.w, r.h);
    pen.body_centred(canvas, r.x, r.y + ITEM_TEXT, r.w, label, look.ink());
}

pub(super) fn big_button(canvas: &mut Canvas, pens: &Pens, r: Rect, label: &str, look: Look) {
    shell::button_recess(canvas, r.x, r.y, r.w, r.h);
    pens.head.heading_centred(canvas, r.x, r.y + 8, r.w, label, look.ink());
}

/// Page 4's name plate (`FUN_0041F321`): a recess, the text, the caret when `field` has focus.
pub(super) fn plate(canvas: &mut Canvas, pen: &Pen, r: Rect, field: &TextField, focused: bool) {
    shell::button_recess(canvas, r.x, r.y, r.w, r.h);
    let (x, y) = (r.x + 6, r.y + ITEM_TEXT);
    pen.body(canvas, x, y, &field.text(), font::TEXT);
    if focused {
        let ink = if pen.assets.body.is_some() { font::TEXT } else { pen.ink.text };
        field.draw_caret(canvas, x, y, ink, &FontMetrics::of(pen.assets));
    }
}

/// `FUN_0041F1DD`'s shield frame; `false` without the sheet.
pub(super) fn shield(canvas: &mut Canvas, pen: &Pen, shield: u8, x: i32, y: i32) -> bool {
    let frame = shield.clamp(1, 5) as usize * 2 + 0xCB;
    match pen.assets.sheet(BOX_SHEET).and_then(|s| s.frame(frame)) {
        Some(f) => {
            canvas.blit(&f, x, y);
            true
        }
        None => false,
    }
}

/// `[D]` the frame's commonest opaque index, the field colour under the live
/// palette: no realm-colour table exists in the data we read (C255).
pub(super) fn shield_ink(pen: &Pen, shield: u8) -> Option<u8> {
    let frame = shield.clamp(1, 5) as usize * 2 + 0xCB;
    let f = pen.assets.sheet(BOX_SHEET)?.frame(frame)?;
    let mut counts = [0u32; 256];
    for (i, &on) in f.opaque.iter().enumerate() {
        if on {
            counts[f.indices[i] as usize] += 1;
        }
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(_, &n)| n)?;
    (*n > 0).then_some(best as u8)
}

pub(super) fn plaque(canvas: &mut Canvas, pen: &Pen, shield: u8, x: i32, y: i32) {
    match shield_ink(pen, shield) {
        Some(ink) => {
            canvas.fill_rect(x, y, 14, 16, ink);
            pen.outline(canvas, x, y, 14, 16, font::TEXT);
        }
        None => {
            pen.body(canvas, x, y, &shield.to_string(), font::TEXT);
        }
    }
}

pub(super) fn tick(canvas: &mut Canvas, x: i32, y: i32) {
    for i in 0..4 {
        canvas.fill_rect(x + i, y + 6 + i, 2, 3, TICK);
    }
    for i in 0..9 {
        canvas.fill_rect(x + 4 + i, y + 9 - i, 2, 3, TICK);
    }
}
