#![allow(unused_imports)]
use super::*;
use super::types::*;
use l2_view::Canvas;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

pub struct NoblesScreen;

impl NoblesScreen {
    pub fn new() -> NoblesScreen {
        NoblesScreen
    }
}

impl Default for NoblesScreen {
    fn default() -> NoblesScreen {
        NoblesScreen::new()
    }
}

impl Screen for NoblesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Nobles
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The standings — screen 0x20".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(PALETTE)
    }

    /// `grtnoble.pl8` is a raw 640 × 480 page read straight into the display
    /// buffer, so this covers whatever opened it.
    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // `Screen_FrameInput`'s epilogue, which runs on every screen but
            // `0x12`: a press inside the minimap raster selects that county,
            // recentres the map and sets `g_screenId = 0`.
            // arm: 0x0042FF10/minimap-under-the-standings left-press
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // **The seven tabs** — `Hotspot_Test(0, 0, &g_nobleTabs, 7)`, all
            // kind 1, all `FUN_0043524E`, which is
            // `g_nobleCategory = g_uiHotspotId; g_redrawRequest = 2;
            // FUN_004B3994(g_uiHotspotId);`.
            //
            // The category is on the [`crate::game::Game`] and not on this
            // screen for the reason `docs/audio.json` gives for the castle
            // chooser's silence: a selection a screen keeps to itself is
            // invisible to [`crate::audio::Director`], so the spoken name
            // could not be reproduced. The original's is a global too.
            // arm: 0x0043524E/standings-category left-press
            Event::Click { x, y } if TABS.iter().any(|t| t.contains(x, y)) => {
                let hit = TABS.iter().position(|t| t.contains(x, y)).unwrap_or(0);
                ctx.game.nobles_category = hit as u8;
                ctx.game.nobles_spoken = ctx.game.nobles_spoken.wrapping_add(1);
                Transition::Stay
            }
            // `Ui_OkButtonClicked()` — a left release in the 24 x 24 corner
            // box. The original goes to the map; we pop, which lands on the
            // court the button was pressed on. See the module docs.
            // arm: 0x0042FF10/standings-ok left-release
            Event::Click { x, y } if OK.contains(x, y) => Transition::Pop,
            // `g_mouseRightReleased`, anywhere, tested before the OK.
            // arm: 0x0042FF10/standings-right right-release
            Event::RightClick { .. } => Transition::Pop,
            // **Ours**: the arm has no keyboard test.
            // arm: ours/standings-keyboard-close key
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

        let category = (ctx.game.nobles_category as usize).min(CATEGORIES - 1);
        let s = rank(&ctx.game.kingdom.realms, category, ctx.game.kingdom.year);

        // The five columns, in the original's own seating order.
        for (slot, &realm) in COLUMN_REALM.iter().enumerate() {
            let Some(r) = ctx.game.kingdom.realms.get(realm as usize) else { continue };
            if !r.in_play {
                continue;
            }
            let x = COLUMN_X[slot];
            let top = (100 - s.pct[realm as usize]) * PIXELS_PER_PERCENT;
            // `Sprite_WGenSprite(shieldIndex - 1, …)` out of `flags.pl8`. A
            // shield of 0 would index -1 in the original; ours skips, and a
            // realm in play always has one.
            let frame = (r.shield_index as usize).wrapping_sub(1);
            if let Some(f) = a.sheet(FLAGS).and_then(|sheet| sheet.frame(frame)) {
                canvas.blit(&f, x, top + FLAG_TOP);
            }
            for (dx, hue) in POLE {
                let y0 = top + POLE_TOP;
                canvas.fill_rect(x + dx, y0, 1, POLE_BOTTOM - y0 + 1, hue);
            }
        }

        // The marker under the tab being looked at — **the category's, not the
        // leader's**. See the module docs.
        if let Some(f) = a.sheet(FLAGS).and_then(|sheet| sheet.frame(MARKER_FRAME)) {
            canvas.blit(&f, TAB_X[category], MARKER_Y);
        }

        // *"Most counties,"* and then either the leader's name or
        // *"undecided."*, two pixels past where the label ended.
        let x = pen.eng(canvas, GROUP, category, LINE_AT.0, LINE_AT.1, font::TEXT);
        if s.decided() {
            let name = super::super::message::lord_name(ctx, s.leader);
            pen.body(canvas, x + NAME_DX, LINE_AT.1, &name, font::TEXT);
        } else {
            pen.eng(canvas, GROUP, UNDECIDED, x + NAME_DX, LINE_AT.1, font::TEXT);
        }

        pen.ok_button(canvas, OK.x, OK.y, OK_MODE);
    }
}

