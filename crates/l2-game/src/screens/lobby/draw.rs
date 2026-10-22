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
    let text = match heading {
        Heading::Eng(g, i) => ctx.assets.shell.text(g, i).to_string(),
        Heading::Text(s) => s.to_string(),
    };
    storybook(canvas, pens, x, HEAD_Y, cols * 16, &text, font::TEXT);
}

/// `[D]` the heading face is a drop capital, never a reading face (owner, C257):
/// the first letter in it, the rest in the body face on the capital's foot.
pub(super) fn storybook(canvas: &mut Canvas, pens: &Pens, x: i32, y: i32, width: i32, s: &str, ink: u8) {
    let assets = pens.body.assets;
    let (Some(head), Some(body), Some(first)) = (&assets.heading, &assets.body, s.chars().next()) else {
        pens.body.body_centred(canvas, x, y, width, s, ink);
        return;
    };
    let cap = first.to_string();
    let rest = &s[first.len_utf8()..];
    let (cap_w, rest_w) = (head.width(&cap) + 1, body.width(rest));
    let x0 = x + ((width - cap_w - rest_w) / 2).max(0);
    pens.head.heading(canvas, x0, y, &cap, ink);
    let foot = y + head.height(&cap) - body.height(rest).max(1);
    pens.body.body(canvas, x0 + cap_w, foot, rest, ink);
}

/// `[D]` one code symbol, body face, centred in its cell (owner, C257).
pub(super) fn symbol(canvas: &mut Canvas, pens: &Pens, r: Rect, c: char, ink: u8) {
    let h = pens.body.assets.body.as_ref().map_or(7, |f| f.height(&c.to_string()));
    pens.body.body_centred(canvas, r.x, r.y + (r.h - h) / 2, r.w, &c.to_string(), ink);
}

pub(super) fn button(canvas: &mut Canvas, pen: &Pen, r: Rect, label: &str, look: Look) {
    shell::button_recess(canvas, r.x, r.y, r.w, r.h);
    pen.body_centred(canvas, r.x, r.y + ITEM_TEXT, r.w, label, look.ink());
}

pub(super) fn big_button(canvas: &mut Canvas, pens: &Pens, r: Rect, label: &str, look: Look) {
    shell::button_recess(canvas, r.x, r.y, r.w, r.h);
    storybook(canvas, pens, r.x, r.y + 8, r.w, label, look.ink());
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

/// `[V]` the realm pen `Realms_AssignLords` (`0x0049CAAA`) writes from the shield,
/// `g_realmColour` table at `0x004DC1D0`; same indices under `Custom.256`. C266.
pub(super) fn plaque(canvas: &mut Canvas, pen: &Pen, shield: u8, x: i32, y: i32) {
    match l2_view::chrome::realm_pen(shield) {
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
