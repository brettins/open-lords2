#![allow(unused_imports)]
use super::*;
use super::castle::*;
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

/// **`TileInfo_Draw` (`0x0041C208`)'s six wordless arms** — road, sea, the
/// occupied and the ruined dwelling plot, mountain, woodland, and the
/// scrubland fall-through under all of them.
///
/// Each is three literals and nothing else: the heading through
/// `Eng_DrawString(0x1E, local_20, 0x28, row * 0x10 + 0x40, &g_fontHeading)`,
/// the body through `FUN_0040328E(0x1E, local_1c, 0x68, row * 0x10 + 100,
/// 0x130, …)` — the owner test above it picks between two *identical* calls —
/// and the icon through `Sprite_WGenSprite(local_8, 0x28, row * 0x10 + 0x60)`.
///
/// Rule 6: every word comes out of the player's `L2.eng` group 30.
pub fn tile_ladder_row(kind: TileKind) -> Option<(usize, usize, usize)> {
    if kind == TileKind::Scrubland {
        return Some(SCRUBLAND_INFO);
    }
    TILE_LADDER.iter().find(|row| row.0 == kind).map(|&(_, h, b, f)| (h, b, f))
}

pub fn draw_plain_tile(pen: &Pen, canvas: &mut Canvas, l: Layout, kind: TileKind) {
    let Some((heading, body, frame)) = tile_ladder_row(kind) else { return };
    let a = pen.assets;
    pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &words(a, TILE_GROUP, heading), font::TEXT);
    pen.body_wrapped(
        canvas,
        BODY_X,
        l.y(BODY_DY),
        TILE_BODY_WRAP,
        &words(a, TILE_GROUP, body),
        font::TEXT,
    );
    if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
        canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
    }
}

/// **`TileInfo_Draw` (`0x0041C208`) for a `0x80` tile below graphic `0x0D` —
/// the mine, the quarry, the blacksmith and the lumber mill.**
///
/// 1. the heading, [`SITE_INFO`]`.0` in `&g_fontHeading` at [`HEADING_X`];
/// 2. the body, [`SITE_INFO`]`.1` plus a **size** band — [`SITE_BAND_EDGES`] on
///    last season's output, or [`SITE_BAND_DESTROYED`] when the record is
///    knocked out. The brief that opened this arm called it a fertility band;
///    group 22 is never touched here and 53…57 run *"A small mine."* to *"A
///    destroyed mine."*
/// 3. the working-or-idle line at [`SITE_STATUS_DY`], a **separate** tail block
/// keyed on `industry.enabled` —
///    *destroyed* at once whenever an army has just trampled it, which is the
///    original's and not a bug of ours.
pub fn draw_resource_site(
    ctx: &Ctx,
    pen: &Pen,
    canvas: &mut Canvas,
    l: Layout,
    tile: usize,
    c: l2_kingdom::tables::Commodity,
) {
    let k = &ctx.game.kingdom;
    let a = pen.assets;
    let (heading, base, frame) = SITE_INFO[c.index()];
    let Some(county) = k.counties.get(k.campaign.map.county[tile] as usize) else { return };
    let site = &county.industry[c.index()];

    pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &words(a, TILE_GROUP, heading), font::TEXT);
    let body = base + site_band(site);
    pen.body_wrapped(
        canvas,
        BODY_X,
        l.y(BODY_DY),
        TILE_BODY_WRAP,
        &words(a, TILE_GROUP, body),
        font::TEXT,
    );
    let state = if site.enabled { SITE_OPERATIONAL } else { SITE_SHUT_DOWN };
    pen.body(canvas, BODY_X, l.y(SITE_STATUS_DY), &words(a, TILE_GROUP, state), font::TEXT);
    if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
        canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
    }
}

pub fn site_band(site: &l2_kingdom::county::Industry) -> usize {
    if site.disabled_seasons != 0 {
        return SITE_BAND_DESTROYED;
    }
    let made = site.output;
    if made < SITE_BAND_EDGES[0] {
        SITE_BAND_SMALL
    } else if made < SITE_BAND_EDGES[1] {
        SITE_BAND_MEDIUM
    } else if made < SITE_BAND_EDGES[2] {
        SITE_BAND_LARGE
    } else {
        SITE_BAND_VERY_LARGE
    }
}

/// **`TileInfo_Draw` (`0x0041C208`) for a `0x20` tile**, in the painter's own
/// order: the heading and its mode in `&g_fontHeading`, then the body or one of
/// the two reports, then the icon.
///
/// `draw` puts the county's name over it (`Ui_DrawCentred(100, …)`, the same
/// block for every tile with head-room) and the brush under it
/// (`FUN_0041C996`).
pub fn draw_farmland(ctx: &Ctx, pen: &Pen, canvas: &mut Canvas, l: Layout, tile: usize) {
    let k = &ctx.game.kingdom;
    let terrain = k.campaign.map.terrain[tile] as usize;
    let Some(&[heading, body, frame, mode]) = FARM_TILE_INFO.get(terrain) else { return };
    let a = pen.assets;
    let county = k.counties.get(k.campaign.map.county[tile] as usize);
    let mine = county.is_some_and(|c| c.owner == ctx.game.player);

    let x = pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &words(a, TILE_GROUP, heading), font::TEXT);
    if mode != 0 {
        pen.heading(canvas, x, l.y(HEADING_DY), &words(a, TILE_GROUP, mode), font::TEXT);
    }

    match (mode, county) {
        (mode::WHEAT, Some(c)) if mine => draw_grain_report(ctx, pen, canvas, l, c),
        (mode::CATTLE, Some(c)) if mine => draw_herd_report(ctx, pen, canvas, l, c),
        (mode::WHEAT | mode::CATTLE, _) => {}
        (mode::FALLOW, _) => {
            let index = if k.options.advanced_farming { FALLOW_BODY_ADVANCED } else { FALLOW_BODY_PLAIN };
            let s = words(a, TILE_GROUP, index);
            if mine {
                pen.body_wrapped(canvas, 0x48, l.y(0xB0), 0x150, &s, font::TEXT);
            } else {
                pen.body_wrapped(canvas, BODY_X, l.y(0x60), TILE_BODY_WRAP, &s, font::TEXT);
            }
        }
        _ => {
            let s = words(a, TILE_GROUP, body);
            pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), TILE_BODY_WRAP, &s, font::TEXT);
        }
    }

    if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
        canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
    }
}

pub(crate) fn workers_colour(c: &l2_kingdom::County, job: usize) -> u8 {
    if c.labour[job] < c.labour_wanted[job] {
        WORKERS_SHORT
    } else if c.labour_useful[job] < c.labour[job] {
        WORKERS_IDLE
    } else {
        font::TEXT
    }
}

