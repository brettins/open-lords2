#![allow(unused_imports)]
use super::*;
use super::score_part::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

fn lord_name(ctx: &Ctx, realm: u8) -> String {
    match ctx.game.player_names.get(realm as usize).map(|n| n.as_str()) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => format!("REALM {realm}"),
    }
}

fn shield_frame(ctx: &Ctx, realm: u8) -> usize {
    let index = ctx.game.kingdom.realms.get(realm as usize).map_or(0, |r| r.shield_index);
    SHIELD_FRAME0 + index as usize
}

impl Screen for RatingsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Ratings
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Battle Master ratings — screen 0x2E".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(PALETTE)
    }

    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // arm: 0x0042FF10/minimap-under-the-ratings left-press
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // arm: 0x0042FF10/ratings-any-press left-press
            Event::Click { .. } | Event::RightClick { .. } => Transition::Pop,
            // arm: ours/ratings-keyboard-close key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        if !crate::shell::background(canvas, a, BACKGROUND) {
            canvas.clear(ink.background);
        }
        pen.ok_button(canvas, OK.x, OK.y, 0);
        pen.eng_heading_centred(
            canvas,
            GROUP,
            HEADING,
            HEADING_AT.0,
            HEADING_AT.1,
            HEADING_AT.2,
            font::TEXT,
        );

        let (mine, theirs) = score(&self.ratings);
        let blocks = [
            (self.ratings.mine, self.ratings.theirs, self.ratings.realms.0, mine),
            (self.ratings.theirs, self.ratings.mine, self.ratings.realms.1, theirs),
        ];
        for (b, &((before, after), (their_before, their_after), realm, points)) in
            blocks.iter().enumerate()
        {
            let top = BLOCK_Y[b];
            pen.misc_frame(canvas, shield_frame(ctx, realm), SHIELD_X, top);
            let name = lord_name(ctx, realm);
            // **[`Pen::body`] returns an absolute x, not a width** — see its
            // own doc comment and `docs/decisions.md` C61. These three lines
            // added it to `NAME_AT.0` a second time
            // loaded put *"Scored"*
            let x = pen.body(canvas, NAME_AT.0, top + NAME_AT.1, &name, font::TEXT);
            let x = pen.eng(canvas, GROUP, SCORED, x, top + NAME_AT.1, font::TEXT);
            // `Ui_DrawNumber(score, ' ', &DAT_004D43B4 | &DAT_004D43C4, …,
            // &g_fontHeading, 0x3F)`, one space each. The lead and suffix were
            // already right; the **face** was not. **[V]**
            pen.number_in(
                Face::Heading,
                canvas,
                x + SCORE_DX,
                top + NAME_AT.1 - SCORE_DY,
                points,
                ' ',
                " ",
                font::TEXT,
            );

            // Row 1 has no label — `L2.eng` 37/1 is drawn by nothing.
            pen.eng(canvas, GROUP, KILLED, LABEL_X, top + ROW_DY[1], font::TEXT);
            pen.eng(canvas, GROUP, KILLS, LABEL_X, top + ROW_DY[2], font::TEXT);

            for c in 0..COLUMNS {
                let x = c as i32 * COL_PITCH + COL_X0;
                let hue = if after.troops[c] == 0 { font::TEXT } else { 0x20 };
                // Lead `' '`, suffix `&DAT_004D43B8` … `&DAT_004D43D0` — six
                // addresses, each holding a single space. Read out of the image
                //: the suffix is inside what `FUN_004025D7`
                // `docs/decisions.md` C140. **[V]**
                pen.number_centred(
                    canvas,
                    x,
                    top + ROW_DY[0],
                    COL_W,
                    before.troops[c],
                    ' ',
                    " ",
                    hue,
                );
                pen.number_centred(
                    canvas,
                    x,
                    top + ROW_DY[1],
                    COL_W,
                    before.troops[c] - after.troops[c],
                    ' ',
                    " ",
                    hue,
                );
                pen.number_centred(
                    canvas,
                    x,
                    top + ROW_DY[2],
                    COL_W,
                    their_before.troops[c] - their_after.troops[c],
                    ' ',
                    " ",
                    0xF9,
                );
            }
        }

        if ctx.game.prefs.debug_overlay {
            l2_view::text::draw(
                canvas,
                4,
                470,
                "NO SKIRMISH MODE: THESE ARE A BATTLE'S NUMBERS AND NOTHING FILLS THEM YET",
                ink.dim,
            );
        }
    }
}

