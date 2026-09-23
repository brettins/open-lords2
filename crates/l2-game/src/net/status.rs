//! A network game's two notices, drawn over every screen: a peer is late
//! (`docs/netcode.md` §4 *Stalling*), and the game has stopped (§6). Ours:
//! `L2.eng` has no group for either, as it has none for the lobby.

use crate::screen::Ctx;
use crate::shell::{font, Pen};
use l2_view::Canvas;

const BOX: (i32, i32, i32) = (160, 200, 20);

pub fn draw(ctx: &Ctx, canvas: &mut Canvas) {
    let link = &ctx.game.net;
    let lines: Vec<String> = match (&link.halted, link.stalled) {
        (Some(why), _) => {
            let mut v = vec!["THE GAME HAS STOPPED".to_string()];
            v.extend(wrap(why, 36));
            v
        }
        (None, true) => vec!["WAITING FOR THE OTHER PLAYERS...".to_string()],
        (None, false) => return,
    };
    let pen = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };
    let (x, y, cols) = BOX;
    pen.window(canvas, x, y, cols, lines.len() as i32 + 2, 1);
    for (i, line) in lines.iter().enumerate() {
        pen.body(canvas, x + 16, y + 16 + 14 * i as i32, line, font::TEXT);
    }
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
