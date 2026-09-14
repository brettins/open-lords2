#![allow(unused_imports)]
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
fn draw_castle(
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
        let bonus = super::job::castle_word(t, super::job::CASTLE_TAX_BONUS_BASE + type_index);
        pen.number_in(Face::Body, canvas, at, l.y(0x88), bonus, ' ', " %", font::TEXT);
        let at = say(canvas, CASTLE_GROUP, CASTLE_BARRACKS, BODY_X, l.y(0x98));
        let cap = super::job::castle_word(t, super::job::CASTLE_BARRACKS_BASE + type_index);
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
        super::job::castle_status_block(pen, ctx, canvas, c, 8, 0x30, l.row);
    }
    if garrison.is_some() {
        widget(canvas, l.y(0x104));
    }
}

/// **`TileInfo_Draw` (`0x0041C208`)'s six wordless arms** — road, sea, the
/// occupied and the ruined dwelling plot, mountain, woodland, and the
/// scrubland fall-through under all of them.
///
/// Each is three literals and nothing else: the heading through
/// `Eng_DrawString(0x1E, local_20, 0x28, row * 0x10 + 0x40, &g_fontHeading)`,
/// the body through `FUN_0040328E(0x1E, local_1c, 0x68, row * 0x10 + 100,
/// 0x130, …)` — the owner test above it picks between two *identical* calls —
/// and the icon through `Sprite_WGenSprite(local_8, 0x28, row * 0x10 + 0x60)`.
/// `local_c` is zero in all six.
///
/// Rule 6: every word comes out of the player's `L2.eng` group 30.
/// [`TILE_LADDER`]'s row for a kind, with [`SCRUBLAND_INFO`] as the ladder's
/// own fall-through. `None` for the four kinds that have their own painter.
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
/// Three things, and the middle one is the whole finding:
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
///
/// The icon is the ladder's shared `Sprite_WGenSprite(local_8, 0x28, R*0x10 +
/// 0x60)`. No ownership gate anywhere in the arm: a rival's mine says its size
/// and its state.
///
/// The four `Sound_RestartSlot` calls that sit among these literals were
/// already built — `docs/audio.json` `TileInfo_Draw#1…#4`, fired on the panel
/// *opening* by [`crate::audio`].
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

/// `TileInfo_Draw`'s band arithmetic, whole — see [`SITE_BAND_EDGES`].
///
/// [`l2_kingdom::county::Industry::output`] is the original's
/// `total − totalSnapshot`: the same difference, held as a field
/// recomputed. `disabledSeasons` **bypasses** the buckets,
/// destroyed site is never also large.
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
    // `g_localPlayer == g_pickedCountyOwner`.
    let mine = county.is_some_and(|c| c.owner == ctx.game.player);

    // `g_penAdvance = 0; Eng_DrawString(30, local_20, 0x28, row*16 + 0x40,
    // &g_fontHeading); if (local_c) Eng_DrawString(30, local_c, g_penAdvance +
    // 0x28, …)` — the mode follows the heading on the same line.
    let x = pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &words(a, TILE_GROUP, heading), font::TEXT);
    if mode != 0 {
        pen.heading(canvas, x, l.y(HEADING_DY), &words(a, TILE_GROUP, mode), font::TEXT);
    }

    match (mode, county) {
        (mode::WHEAT, Some(c)) if mine => draw_grain_report(ctx, pen, canvas, l, c),
        (mode::CATTLE, Some(c)) if mine => draw_herd_report(ctx, pen, canvas, l, c),
        // Somebody else's wheat or cattle: the heading and the icon, and the
        // table's description is **not** drawn — `local_1c` goes unread.
        (mode::WHEAT | mode::CATTLE, _) => {}
        (mode::FALLOW, _) => {
            let index = if k.options.advanced_farming { FALLOW_BODY_ADVANCED } else { FALLOW_BODY_PLAIN };
            let s = words(a, TILE_GROUP, index);
            // Yours sits lower and wider, clear of the brush's caption.
            if mine {
                pen.body_wrapped(canvas, 0x48, l.y(0xB0), 0x150, &s, font::TEXT);
            } else {
                pen.body_wrapped(canvas, BODY_X, l.y(0x60), TILE_BODY_WRAP, &s, font::TEXT);
            }
        }
        // Barren, blighted and being reclaimed — both owner arms are the same
        // call.
        _ => {
            let s = words(a, TILE_GROUP, body);
            pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), TILE_BODY_WRAP, &s, font::TEXT);
        }
    }

    // `File_ReadChunk("icon_tmp.pl8"); Sprite_WGenSprite(local_8, 0x28, row*16 + 0x60)`.
    if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
        canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
    }
}

/// The worker count's colour — both reports' three-way ladder on their own job.
fn workers_colour(c: &l2_kingdom::County, job: usize) -> u8 {
    if c.labour[job] < c.labour_wanted[job] {
        WORKERS_SHORT
    } else if c.labour_useful[job] < c.labour[job] {
        WORKERS_IDLE
    } else {
        font::TEXT
    }
}

/// `if (v == 0) Ui_DrawNumber(0, '@', " ", 0x108, y) else Ui_DrawDelta(v, 0, " ",
/// " ", 0x108, y, body, 0x3F, 0xF9)` — the five signed lines of both reports.
///
/// The zero is drawn, not skipped: mode 0 would draw nothing, and the painters
/// test for it first and print `0` instead. Every one of the ten string
/// arguments (`&DAT_004D4240` … `&DAT_004D4278`) is a single space, read out
/// of the image.
fn report_value(pen: &Pen, canvas: &mut Canvas, y: i32, value: i32) {
    const X: i32 = 0x108;
    let body = crate::shell::Face::Body;
    if value == 0 {
        pen.number_in(body, canvas, X, y, 0, '@', " ", font::TEXT);
        return;
    }
    // `Ui_DrawText(prefix, x, y, font, sign ? colourNeg : colourPos)`, then the
    // number at `x + g_penAdvance` with `'-'` or `'+'` in the lead.
    let colour = if value < 0 { REPORT_NEG } else { font::TEXT };
    let next = pen.body(canvas, X, y, " ", colour);
    let lead = if value < 0 { '-' } else { '+' };
    pen.number_in(body, canvas, next, y, value.abs(), lead, " ", colour);
}

/// **The weather's line**, which `TileInfo_DrawGrain` and `TileInfo_DrawHerd`
/// both write out in full at `row * 0x10 + 0xA4`, advanced farming only:
///
/// ```c
/// g_penAdvance = 0;
/// if (v < 1) {
///   if (v < 0) { Ui_DrawCount(-v, noun, 0x28, y); Eng_DrawString(77, 0x11, pen + 0x28, y); }
///   else       { Eng_DrawString(77, 0x12, 0x28, y); }
/// } else       { Ui_DrawCount(v, noun, 0x28, y);  Eng_DrawString(77, 0x10, pen + 0x28, y); }
/// ```
///
/// The same three arms `job::weather_line` draws at `0xC0` from column `0x40`.
fn weather_line(pen: &Pen, canvas: &mut Canvas, y: i32, v: i32, noun: usize) {
    const X: i32 = 0x28;
    let a = pen.assets;
    let say = |canvas: &mut Canvas, x: i32, index: usize| {
        pen.body(canvas, x, y, &words(a, REPORT_GROUP, index), font::TEXT)
    };
    if v == 0 {
        say(canvas, X, 0x12);
        return;
    }
    let shown = if v < 0 { v.wrapping_neg() } else { v };
    let at = pen.count(canvas, X, y, shown, noun, font::TEXT);
    say(canvas, at, if v < 0 { 0x11 } else { 0x10 });
}

/// **`TileInfo_DrawGrain` (`0x0041CB3A`)** — the wheat field's report, owner only.
///
/// # The store line and the weather line
///
/// ```c
/// if (county.field_0x278 == 0) Eng_DrawString(77, 0x18, 0x28, row*16 + 0x94);   /* no outside factors */
/// else { Ui_DrawCount(county.field_0x278, 2, 0x28, …);  /* then 77/0x19 rats (0x87) or 77/0x1A surplus (0x8B) */ }
/// if (g_optAdvancedFarming == 1) { … the +0x24C line at row*16 + 0xa4 … }
/// ```
///
/// `+0x278` is `grain_event_change` and `+0x24C` is `grain_weather_change`;
/// both are **imported** (`docs/stored-fields.json`), and the module doc's
/// claim that they were excluded was two corrections out of date. The words
/// are group 77's — 0x19/0x1A after the figure, and 0x10/0x11/0x12 on the
/// weather line — and [`weather_line`] is the shape `Panel_JobGrain` and
/// `Panel_JobCattle` share with this painter at a different y.
fn draw_grain_report(ctx: &Ctx, pen: &Pen, canvas: &mut Canvas, l: Layout, c: &l2_kingdom::County) {
    let k = &ctx.game.kingdom;
    let a = pen.assets;
    let say = |canvas: &mut Canvas, x: i32, y: i32, index: usize| {
        pen.body(canvas, x, y, &words(a, REPORT_GROUP, index), font::TEXT)
    };
    // `Ui_DrawCount(labour[0].workers, 0x20, 0x68, row*16 + 0x68, body, colour)` —
    // *"Farmers"* — and the store, `Ui_DrawCount(grain, 2, 0x128, …)`, *"Sacks"*.
    pen.count(canvas, 0x68, l.y(0x68), c.labour[0], 0x20, workers_colour(c, 0));
    pen.count(canvas, 0x128, l.y(0x68), c.grain, 2, font::TEXT);
    if k.options.advanced_farming {
        let band = ((c.fertility + 100) / 0x1D).clamp(0, 6) as usize;
        pen.body(canvas, 0x68, l.y(0x78), &words(a, FERTILITY_GROUP, band), font::TEXT);
    }
    // `+0x278` — what last season's random event did to the store. *Rats*
    // (`0x87`) and *Grain found* (`0x8B`) are the only two ids that write it,
    // and any other id leaves the figure with no words after it.
    if c.grain_event_change == 0 {
        say(canvas, 0x28, l.y(0x94), 0x18);
    } else {
        let at = pen.count(canvas, 0x28, l.y(0x94), c.grain_event_change, 2, font::TEXT);
        match c.event_id {
            0x87 => {
                say(canvas, at, l.y(0x94), 0x19);
            }
            0x8B => {
                say(canvas, at, l.y(0x94), 0x1A);
            }
            _ => {}
        }
    }
    // `if (g_optAdvancedFarming == 1)` — the `+0x24C` weather line.
    if k.options.advanced_farming {
        weather_line(pen, canvas, l.y(0xA4), c.grain_weather_change, 2);
    }

    if k.season_next == 1 {
        // Facing Spring: the seed and what it will yield.
        let x = pen.count(canvas, 0x28, l.y(0xC0), c.grain_sown_expected, 2, font::TEXT);
        say(canvas, x, l.y(0xC0), 1);
        let yielding = c.grain_sown_expected * k.tables.grain.yield_per_sack;
        let x = pen.count(canvas, 0x28, l.y(0xD0), yielding, 2, font::TEXT);
        say(canvas, x, l.y(0xD0), 2);
    } else {
        let seasons = match k.season_next {
            2 => 3,
            3 => 2,
            _ => 1,
        };
        let crop = if k.season_next == 4 { c.crop[2] } else { c.grain_grown_expected };
        let x = pen.count(canvas, 0x28, l.y(0xC0), crop, 2, font::TEXT);
        let x = say(canvas, x, l.y(0xC0), 3);
        pen.count(canvas, x, l.y(0xC0), seasons, 0x42, font::TEXT);
        let x = say(canvas, 0x28, l.y(0xD0), 0);
        let x = pen.count(canvas, x, l.y(0xD0), c.crop[0], 2, font::TEXT);
        say(canvas, x, l.y(0xD0), 4);
    }
    say(canvas, 0x28, l.y(0xE0), 0x1B);
    report_value(pen, canvas, l.y(0xE0), -c.grain_eaten);
    say(canvas, 0x28, l.y(0xF0), 0x1C);
    report_value(pen, canvas, l.y(0xF0), c.grain_change_expected);
}

/// **`TileInfo_DrawHerd` (`0x0041D299`)** — the pasture's report, owner only.
///
/// The event line has [`draw_grain_report`]'s shape: `+0x274`
/// (`herd_event_change`) is written by `Herd_SeasonTick` only from
/// `eventHerdPct`, whose setters are *Mad cows* (`0x88`), *Wolves* (`0x89`),
/// *Bad cattle* (`0x8C`) and *Cow bonanza* (`0x8D`) — *No bull*'s 99 zeroes it
/// — so any other id leaves the figure with no words after it. The weather
/// line is `+0x270`, `herd_weather_change`.
fn draw_herd_report(ctx: &Ctx, pen: &Pen, canvas: &mut Canvas, l: Layout, c: &l2_kingdom::County) {
    let a = pen.assets;
    let say = |canvas: &mut Canvas, x: i32, y: i32, index: usize| {
        pen.body(canvas, x, y, &words(a, REPORT_GROUP, index), font::TEXT)
    };
    // *"Dairy maids"* and *"Animals"*.
    pen.count(canvas, 0x68, l.y(0x68), c.labour[1], 0x22, workers_colour(c, 1));
    pen.count(canvas, 0x128, l.y(0x68), c.herd, 4, font::TEXT);
    if c.fields_cattle != 0 {
        let crowding = match c.herd_crowding {
            10 => 8,
            20 => 9,
            30 => 10,
            _ => 11,
        };
        say(canvas, 0x68, l.y(0x78), crowding);
    }
    // `+0x274` — what last season's random event did to the herd.
    if c.herd_event_change == 0 {
        say(canvas, 0x28, l.y(0x94), 0x13);
    } else {
        let at = pen.count(canvas, 0x28, l.y(0x94), c.herd_event_change, 4, font::TEXT);
        let word = match c.event_id {
            0x88 => Some(0x14),
            0x89 => Some(0x15),
            0x8C => Some(0x16),
            0x8D => Some(0x17),
            _ => None,
        };
        if let Some(index) = word {
            say(canvas, at, l.y(0x94), index);
        }
    }
    // `if (g_optAdvancedFarming == 1)` — the `+0x270` weather line.
    if ctx.game.kingdom.options.advanced_farming {
        weather_line(pen, canvas, l.y(0xA4), c.herd_weather_change, 4);
    }

    say(canvas, 0x28, l.y(0xC0), 5);
    pen.count(canvas, 0x108, l.y(0xC0), c.herd_births_expected, 4, font::TEXT);
    say(canvas, 0x28, l.y(0xD0), 6);
    pen.count(canvas, 0x108, l.y(0xD0), c.herd_deaths_expected, 4, font::TEXT);
    // *"Change due to farming"* is computed inline and never stored.
    say(canvas, 0x28, l.y(0xE0), 7);
    report_value(pen, canvas, l.y(0xE0), c.herd_births_expected - c.herd_deaths_expected);
    say(canvas, 0x28, l.y(0xF0), 0x1B);
    report_value(pen, canvas, l.y(0xF0), -c.herd_eaten);
    // *"Overall change"* — county `+0x258`, `herd_change_expected`
    // (`docs/records.json` `herdOverallChange`, C128).
    say(canvas, 0x28, l.y(0x100), 0x1C);
    report_value(pen, canvas, l.y(0x100), c.herd_change_expected);
}

