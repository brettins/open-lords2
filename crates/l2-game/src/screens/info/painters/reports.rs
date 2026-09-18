#![allow(unused_imports)]
use super::*;
use super::castle::*;
use super::tiles::*;
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
    let colour = if value < 0 { REPORT_NEG } else { font::TEXT };
    let next = pen.body(canvas, X, y, " ", colour);
    let lead = if value < 0 { '-' } else { '+' };
    pen.number_in(body, canvas, next, y, value.abs(), lead, " ", colour);
}

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
pub(super) fn draw_grain_report(ctx: &Ctx, pen: &Pen, canvas: &mut Canvas, l: Layout, c: &l2_kingdom::County) {
    let k = &ctx.game.kingdom;
    let a = pen.assets;
    let say = |canvas: &mut Canvas, x: i32, y: i32, index: usize| {
        pen.body(canvas, x, y, &words(a, REPORT_GROUP, index), font::TEXT)
    };
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
pub(super) fn draw_herd_report(ctx: &Ctx, pen: &Pen, canvas: &mut Canvas, l: Layout, c: &l2_kingdom::County) {
    let a = pen.assets;
    let say = |canvas: &mut Canvas, x: i32, y: i32, index: usize| {
        pen.body(canvas, x, y, &words(a, REPORT_GROUP, index), font::TEXT)
    };
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
    say(canvas, 0x28, l.y(0xE0), 7);
    report_value(pen, canvas, l.y(0xE0), c.herd_births_expected - c.herd_deaths_expected);
    say(canvas, 0x28, l.y(0xF0), 0x1B);
    report_value(pen, canvas, l.y(0xF0), -c.herd_eaten);
    // *"Overall change"* — county `+0x258`, `herd_change_expected`
    // (`docs/records.json` `herdOverallChange`, C128).
    say(canvas, 0x28, l.y(0x100), 0x1C);
    report_value(pen, canvas, l.y(0x100), c.herd_change_expected);
}


