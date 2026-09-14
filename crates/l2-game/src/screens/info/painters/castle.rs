#![allow(unused_imports)]
use super::*;
use super::tiles::*;
use super::reports::*;
use super::*;
use super::types::*;
use super::constants::*;
use super::screen::*;
use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

/// **`TileInfo_Draw` (`0x0041C208`) for a `0x80` castle tile, and
/// `TileInfo_DrawCastle` (`0x0041DA2F`) under it.**
///
/// The outer painter's four literals:
///
/// ```c
/// else if (g_pickedTileGraphic < 0x1a) {
///   if      (county.castleDegraded == 1) local_20 = 0xe;   /* under construction */
///   else if (county.castleDegraded == 2) local_20 = 0xf;   /* under repair       */
///   else                                 local_20 = 8;     /* a castle           */
///   local_1c = g_pickedTileGraphic + 7;  local_8 = 0x1c;  local_c = 0;
/// }
/// ```
///
/// — heading 30/8, 30/14 or 30/15, body 30/`graphic + 7` (so the bare plot's
/// `0x14` takes 30/27 and a royal castle's `0x19` takes 30/32), and
/// `Icon_tmp.pl8` frame `0x1C`. **The body goes through the same
/// `FUN_0040328E(30, local_1c, 0x68, row*16 + 100, 0x130, …)` every other
/// non-farmland arm uses**, both sides of the owner test being one call.
///
/// Then `TileInfo_DrawCastle`, which is two arms and a shared tail:
///
/// ```c
/// DAT_00568474 = (county.garrisonUnit != 0);
/// if (county.castleDegraded == 0) {
///   if (county.field_0x1c2 != 0) return;                 /* ruined: nothing at all */
///   71/0x10 (0x68, R+0x88) + Ui_DrawNumber(taxBonus[type], ' ', " %", pen + 0x68)
///   71/0x0B (0x68, R+0x98) + Ui_DrawNumber(barracks[type], ' ', " ", pen + 0x68) + 71/0x0C
///   if (garrison) {
///     owner == local ? Ui_DrawNumber(unit.menTotal, ' ', " ", 0x68, R+0xa8) + 71/0x0D
///                    : 71/0x13 (0x68, R+0xa8)
///     71/0x0E (0x68, R+0xc4); Widget_Draw(8, 0x20, &g_tilePanelWidgets, 1)
///   }
/// } else {
///   if (county.owner == g_localPlayer) {
///     Ui_DrawCount(labour[3].workers, 0x26, 0x68, R+0x88)
///     Castle_DrawStatusBlock(county, 8, 0x30, R)
///   }
///   if (garrison) { 71/0x0E (0x68, R+0x104); Widget_Draw(…) }
/// }
/// ```
///
/// **Note what the two arms do not share.** The intact arm's tax and barracks
/// lines are `Castle_DrawStatusBlock`'s first two written out again at a
/// different y, and the degraded arm reaches the block itself —
/// building its first castle is the one that shows the stone and wood owed and
/// the seasons left.
/// heading above it says *"Castle."* and the block below is empty
/// the original's, not a gap of ours.
///
/// The tax and barracks words come from
/// [`super::job::castle_word`]'s run, one word low at type 0 — `docs/bugs.md`'s
/// *"Barracks for 2500 troops."* on a county with no castle is reproduced here
/// too, because it is the same two table reads.
pub(super) fn draw_castle(
    ctx: &Ctx,
    pen: &Pen,
    canvas: &mut Canvas,
    l: Layout,
    tile: usize,
    pressed: bool,
    ink: &l2_view::ink::Ink,
) {
    let k = &ctx.game.kingdom;
    let a = pen.assets;
    let map = &k.campaign.map;
    let graphic = map.terrain[tile] as usize;
    let Some(c) = k.counties.get(map.county[tile] as usize) else { return };
    let say = |canvas: &mut Canvas, group: usize, index: usize, x: i32, y: i32| {
        pen.body(canvas, x, y, &words(a, group, index), font::TEXT)
    };

    // The heading, in `&g_fontHeading` like every other arm's.
    let heading = match c.castle_degraded {
        1 => CASTLE_HEADING_BUILDING,
        2 => CASTLE_HEADING_REPAIR,
        _ => CASTLE_HEADING,
    };
    pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &words(a, TILE_GROUP, heading), font::TEXT);
    let body = words(a, TILE_GROUP, graphic + 7);
    pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), TILE_BODY_WRAP, &body, font::TEXT);
    // `Sprite_WGenSprite(0x1C, 0x28, row*16 + 0x60)`. The original draws it
    // *after* `TileInfo_DrawCastle` returns, which matters only in that the
    // ruined arm below returns early and the icon is still drawn.
    if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(CASTLE_ICON)) {
        canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
    }

    // `DAT_00568474`, and the widget it counts.
    let garrison = k.campaign.units.get(c.garrison_unit).filter(|_| c.garrison_unit != 0);
    let widget = |canvas: &mut Canvas, y: i32| {
        say(canvas, CASTLE_GROUP, VIEW_THESE_TROOPS, BODY_X, y);
        // `System.pl8` frame 25 is the tick, which is what the record's `+4`
        // carries; our own button is the fallback for an install with no
        // artwork. `Widget_Draw` adds one to it while the press timer at
        // `+0x0D` runs.
        let frame = if pressed { 26 } else { 25 };
        if !pen.system_frame(canvas, frame, GARRISON_WIDGET.x, GARRISON_WIDGET.y) {
            crate::widget::frame(canvas, GARRISON_WIDGET, ink.highlight);
        }
    };

    if c.castle_degraded == 0 {
        if c.castle_ruined {
            return;
        }
        let t = &k.tables;
        let type_index = usize::from(c.castle_type);
        let at = say(canvas, CASTLE_GROUP, CASTLE_TAX_BONUS, BODY_X, l.y(0x88));
        let bonus = super::super::job::castle_word(t, super::super::job::CASTLE_TAX_BONUS_BASE + type_index);
        pen.number_in(Face::Body, canvas, at, l.y(0x88), bonus, ' ', " %", font::TEXT);
        let at = say(canvas, CASTLE_GROUP, CASTLE_BARRACKS, BODY_X, l.y(0x98));
        let cap = super::super::job::castle_word(t, super::super::job::CASTLE_BARRACKS_BASE + type_index);
        let at = pen.number_in(Face::Body, canvas, at, l.y(0x98), cap, ' ', " ", font::TEXT);
        say(canvas, CASTLE_GROUP, CASTLE_TROOPS, at, l.y(0x98));
        if let Some(u) = garrison {
            // **No ownership gate on the widget** — somebody else's garrison
            // gets 71/19 instead of the count and the same button under it.
            if u.owner == ctx.game.player {
                let at =
                    pen.number_in(Face::Body, canvas, BODY_X, l.y(0xA8), u.men, ' ', " ", font::TEXT);
                say(canvas, CASTLE_GROUP, CASTLE_STATIONED, at, l.y(0xA8));
            } else {
                say(canvas, CASTLE_GROUP, CASTLE_ENEMY_BARRACKED, BODY_X, l.y(0xA8));
            }
            widget(canvas, l.y(0xC4));
        }
        return;
    }
    if c.owner == ctx.game.player {
        pen.count(
            canvas,
            BODY_X,
            l.y(0x88),
            c.labour[l2_kingdom::tables::JOB_CASTLE_BUILDING],
            0x26,
            font::TEXT,
        );
        super::super::job::castle_status_block(pen, ctx, canvas, c, 8, 0x30, l.row);
    }
    if garrison.is_some() {
        widget(canvas, l.y(0x104));
    }
}

