//! Screen `0x2B`, `Screen_BattleOutcome` (`0x00423241`), drawn over the field.

use l2_view::Canvas;

use super::*;
use crate::battlefield::{LiveBattle, Mode};
use crate::screen::Ctx;
use crate::shell::{font, Pen};

/// ```text
///   choice_owner == 0 || !g_optAnimations:
///   FUN_004093E0(0x10, 0x90, 0x1C, 0x0A); Ui_OkButton(0x1A0, 0x100, 0)
///   Eng_DrawString(0x52, pair*2, 0x30, 0xA8); FUN_0040328E(0x52, pair*2+1, 0x30, 0xE0, 0x180, 100, 0, 0)
///   else: FUN_0047703A(); FUN_004B1310(); FUN_004093E0(0x10, 0x30, 0x1C, 0x16)
///   Ui_DrawInsetRect(0x27, 0x48, 0x192, 0xC2); Ui_OkButton(0x1A0, 0x160, 0)
///   Eng_DrawString(0x52, pair*2, 0x30, 0x138); FUN_0040328E(…, 0x30, 0x158, 0x180, 100, 0x20, 0x1A0)
/// ```
/// [V] `00420000.c` Screen_BattleOutcome. [I] `FUN_0047703A` is unread; `FUN_0040328E`'s
/// `0x20, 0x1A0` are not modelled. The neutral arm is pair 6 through [`outcome_banner`].
pub(super) fn draw_outcome(p: &Pen, canvas: &mut Canvas, ctx: &Ctx, live: &LiveBattle) {
    if live.mode != Mode::Outcome {
        return;
    }
    let pair = outcome_banner(ctx.game, live);
    let head = ctx.assets.shell.text(GROUP_BANNER, pair * 2).to_string();
    let body = ctx.assets.shell.text(GROUP_BANNER, pair * 2 + 1).to_string();
    if ctx.game.prefs.animations && live.choice_owner != 0 {
        let palette =
            ctx.assets.shell.palette(l2_view::scene::TILE_PALETTE).unwrap_or(&ctx.assets.palette);
        canvas.remap(&l2_view::canvas::shade_table(palette));
        p.window(canvas, 0x10, 0x30, 0x1C, 0x16, BOX_SET);
        p.inset(canvas, crate::input::Rect::new(0x27, 0x48, 0x192, 0xC2));
        p.ok_button(canvas, 0x1A0, 0x160, 0);
        p.heading(canvas, 0x30, 0x138, &head, font::TEXT);
        p.body_wrapped(canvas, 0x30, 0x158, 0x180, &body, font::TEXT);
        return;
    }
    p.window(canvas, OUTCOME_BOX.x, OUTCOME_BOX.y, OUTCOME_COLS, OUTCOME_ROWS, BOX_SET);
    p.ok_button(canvas, OUTCOME_OK.0, OUTCOME_OK.1, 0);
    let head = if head.is_empty() { "THE BATTLE IS OVER.".into() } else { head };
    p.heading(canvas, OUTCOME_HEAD.0, OUTCOME_HEAD.1, &head, font::TEXT);
    if !body.is_empty() {
        p.body_wrapped(canvas, OUTCOME_BODY.0, OUTCOME_BODY.1, OUTCOME_BODY.2, &body, font::TEXT);
    }
}
