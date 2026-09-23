//! A network game's notices, drawn over every screen: a peer is late
//! (`docs/netcode.md` §4 *Stalling*), the game has stopped (§6), and the
//! heartbeat. The stopped box and the in-battle wait box are ours [D]: `L2.eng`
//! has no group for either (CNEW-battle-wait-box).

use crate::screen::Ctx;
use crate::shell::{font, Pen};
use l2_view::Canvas;

const BOX: (i32, i32, i32) = (160, 200, 20);

/// `FUN_0042476B` (`0x0042476B`, 00420000.c:1396) `(10, 0x1E)` from `Battle_Frame` (`004b0000.c:6786`):
/// `Pl8_DrawFrame(&g_panelsSheet, 0x105, 10, 30)`. [V]
pub const WAIT_GLYPH: (usize, i32, i32) = (0x105, 10, 30);

/// `FUN_0041A844` (`0x0041A844`, 00410000.c:3868): `FUN_0040437D(2, 0x1DC, 8, 3, c)` then
/// `Ui_DrawRectOutline(1, 0x1DB, 10, 5, 0x3F)`. [V]
pub const HEART_BAR: (i32, i32, i32, i32) = (2, 476, 8, 3);
pub const HEART_RIM: (i32, i32, i32, i32, u8) = (1, 475, 10, 5, 0x3F);
/// `local_8 = 0xFA` when `DAT_0057C998` ("Gtime") moved, else `0xF9`. [V]
pub const HEART_LIVE: u8 = 0xFA;
pub const HEART_IDLE: u8 = 0xF9;

/// `FUN_0041A844`'s statics: `DAT_004E59A0` the countdown, `DAT_004E59C0` the
/// Gtime last seen. Per seat; never in the digest (C210).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Beat {
    count: u8,
    seen: u32,
    pub live: bool,
    /// This frame resampled: the only frame `0x0041A844` draws on. [V]
    pub drawn: bool,
}

impl Beat {
    /// One frame: `DAT_004E59A0 = '\f'` resamples every twelfth. [V]
    pub fn frame(&mut self, gtime: u32) {
        self.count = self.count.saturating_sub(1);
        self.drawn = self.count == 0;
        if self.drawn {
            self.live = self.seen < gtime;
            self.count = 12;
            self.seen = gtime;
        }
    }
}

pub fn draw(ctx: &Ctx, canvas: &mut Canvas) {
    let link = &ctx.game.net;
    // `g_multiplayer != 0 && g_appPhase == 3` (00410000.c:3873): `lockstep` is set
    // once a session runs a world. [I]
    if link.lockstep && link.beat.drawn {
        heartbeat(ctx, canvas);
    }
    let in_battle = ctx.game.battle.is_some();
    let lines: Vec<String> = match (&link.halted, link.stalled) {
        (Some(why), _) => {
            let mut v = vec!["THE GAME HAS STOPPED".to_string()];
            v.extend(wrap(why, 36));
            v
        }
        // `FUN_0042476B`'s `g_battlePhase == 0` guard. [V] Its 1,600 ms expiry
        // (`0x640 < now - DAT_00553C6C`) is not ours: `stalled` ends with the stall. [I]
        (None, true) if !in_battle => return wait_glyph(ctx, canvas),
        // [D] 0x0042476B draws nothing in battle; ours says the game is waiting
        // (CNEW-battle-wait-box).
        (None, true) => vec!["WAITING FOR THE OTHER PLAYERS...".to_string()],
        (None, false) => return,
    };
    let pen = pen(ctx);
    let (x, y, cols) = BOX;
    pen.window(canvas, x, y, cols, lines.len() as i32 + 2, 1);
    for (i, line) in lines.iter().enumerate() {
        pen.body(canvas, x + 16, y + 16 + 14 * i as i32, line, font::TEXT);
    }
}

fn pen<'a>(ctx: &'a Ctx) -> Pen<'a> {
    Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    }
}

fn wait_glyph(ctx: &Ctx, canvas: &mut Canvas) {
    let (frame, x, y) = WAIT_GLYPH;
    let drawn = ctx.assets.chrome.as_ref().is_some_and(|c| c.draw_panel_frame(canvas, frame, x, y));
    if !drawn {
        pen(ctx).outline(canvas, x, y, 48, 48, HEART_RIM.4);
    }
}

fn heartbeat(ctx: &Ctx, canvas: &mut Canvas) {
    let (x, y, w, h) = HEART_BAR;
    let colour = if ctx.game.net.beat.live { HEART_LIVE } else { HEART_IDLE };
    canvas.fill_rect(x, y, w, h, colour);
    let (x, y, w, h, rim) = HEART_RIM;
    pen(ctx).outline(canvas, x, y, w, h, rim);
}

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    for word in text.split_whitespace() {
        let last = out.last_mut().expect("one line");
        if !last.is_empty() && last.len() + 1 + word.len() > width {
            out.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Assets, Game};

    fn drawn(game: &mut Game) -> Canvas {
        let assets = Assets::placeholder();
        let mut canvas = Canvas::screen();
        canvas.clear(0);
        draw(&Ctx { game, assets: &assets }, &mut canvas);
        canvas
    }

    /// Ablation: `self.seen = gtime` or `self.count = 12` in `Beat::frame`.
    #[test]
    fn the_heartbeat_resamples_gtime_every_twelfth_frame() {
        let mut b = Beat::default();
        b.frame(1);
        assert!(b.live, "Gtime moved since the last sample");
        for _ in 0..11 {
            b.frame(1);
            assert!(b.live, "held between samples");
        }
        b.frame(1);
        assert!(!b.live, "twelve frames on, Gtime stood still");
    }

    /// Ablation: `link.lockstep` or `link.beat.drawn` in `draw`, or `HEART_LIVE` for `HEART_IDLE`.
    #[test]
    fn the_heartbeat_is_drawn_bottom_left_in_a_lockstep_game_only() {
        let mut game = Game::new(1);
        assert_eq!(drawn(&mut game).at(2, 476), 0, "no heartbeat outside a network game");
        game.net.lockstep = true;
        game.net.beat.frame(1);
        let c = drawn(&mut game);
        assert_eq!((c.at(2, 476), c.at(9, 478)), (HEART_LIVE, HEART_LIVE));
        assert_eq!((c.at(1, 475), c.at(10, 479)), (0x3F, 0x3F), "the rim");
        game.net.beat.frame(1);
        assert_eq!(drawn(&mut game).at(2, 476), 0, "drawn on the resample frame only");
    }

    /// Ablation: the `!in_battle` arm; the glyph gives way to our box in a battle.
    #[test]
    fn a_stall_on_the_map_draws_the_wait_glyph_at_ten_thirty() {
        let mut game = Game::new(1);
        game.net.stalled = true;
        let c = drawn(&mut game);
        assert_eq!((c.at(10, 30), c.at(57, 77)), (0x3F, 0x3F), "48 x 48 at (10, 30)");
        assert_eq!(c.at(BOX.0 as usize + 8, BOX.1 as usize + 8), 0, "not our box");
    }
}
