#![allow(unused_imports)]
use super::*;
use super::screen::*;
use super::common::*;
use super::blacksmith::*;
use super::castle::*;
use l2_kingdom::county::County;
use l2_kingdom::tables::{
    Commodity, Tables, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_COUNT, JOB_FIELD_RECLAMATION,
    JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_NAMES, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
};
use l2_view::chrome::system;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{count_noun, font, Face, Pen};

/// The weather's line, which `Panel_JobGrain` and `Panel_JobCattle` both write
/// out in full at `y = 0xC0`, advanced farming only:
///
/// ```c
/// g_penAdvance = 0;
/// if (v < 1) {
///   if (v < 0) { Ui_DrawCount(-v, noun, 0x40, 0xc0); Eng_DrawString(77, 0x11, pen + 0x40, 0xc0); }
///   else       { Eng_DrawString(77, 0x12, 0x40, 0xc0); }
/// } else       { Ui_DrawCount(v, noun, 0x40, 0xc0);  Eng_DrawString(77, 0x10, pen + 0x40, 0xc0); }
/// ```
fn weather_line(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, v: i32, noun: usize) {
    const Y: i32 = 0xC0;
    if v < 0 {
        let at = count(pen, ctx, canvas, v.wrapping_neg(), noun, 0x40, Y);
        say(pen, ctx, canvas, FORECAST_GROUP, 0x11, at, Y);
    } else if v == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x12, 0x40, Y);
    } else {
        let at = count(pen, ctx, canvas, v, noun, 0x40, Y);
        say(pen, ctx, canvas, FORECAST_GROUP, 0x10, at, Y);
    }
}

/// **`Panel_JobGrain` (`0x00413590`)**, 27 call sites. The store, the fertility
/// band, what last season's event and weather did to the store, then either
/// what will be sown (facing Spring) or what is growing and when it comes in,
/// and the two signed rows.
///
/// ```text
/// Ui_DrawCount(grain, 2, 0x130, 0x88)
/// [adv] Eng_DrawString(22, (fertility + 100) / 0x1D, 0x80, 0x98)
/// +0x278 == 0 ? 77/0x18 at (0x40, 0xB0)
///             : Ui_DrawCount(+0x278, 2, 0x40, 0xB0) + 77/0x19 (event 0x87) or 77/0x1A (0x8B)
/// [adv] the weather line on +0x24C
/// g_seasonNext == 1:
///   Ui_DrawCount(+0x230, 2, 0x40, 0xD8)                + 77/1
///   Ui_DrawCount(+0x230 * g_grainYieldPerSack, 2, 0x40, 0xE8) + 77/2
/// otherwise:
///   Ui_DrawCount(season 4 ? crop[2] : +0x2FC, 2, 0x40, 0xD8) + 77/3
///     + Ui_DrawCount(season 2 ? 3 : season 3 ? 2 : 1, 0x42, pen + 0x40, 0xD8)
///   77/0 at (0x40, 0xE8) + Ui_DrawCount(crop[0], 2, pen + 0x40, 0xE8) + 77/4
/// 77/0x1B at (0x40, 0x108); grainEaten == 0 ? Ui_DrawNumber(0, '@', " ", 0x130)
///                                           : Ui_DrawDelta(-grainEaten, 0, " ", " ", 0x128)
/// 77/0x1C at (0x40, 0x118); +0x22C == 0 ? the same zero : Ui_DrawDelta(+0x22C, …)
/// ```
fn grain(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County) {
    let k = &ctx.game.kingdom;
    let advanced = k.options.advanced_farming;
    count(pen, ctx, canvas, c.grain, NOUN_SACK, 0x130, 0x88);
    if advanced {
        // `(fertility + 100) / 0x1D`, C division; `+0x208` is -100…100, so the
        // band is 0…6 and group 22 has exactly seven strings.
        let band = (c.fertility + 100) / 0x1D;
        say(pen, ctx, canvas, FERTILITY_GROUP, band.max(0) as usize, 0x80, 0x98);
    }

    // `+0x278` — what last season's random event did to the store.
    const EVENT_Y: i32 = 0xB0;
    if c.grain_event_change == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x18, 0x40, EVENT_Y);
    } else {
        let at = count(pen, ctx, canvas, c.grain_event_change, NOUN_SACK, 0x40, EVENT_Y);
        // `county.eventId`, the county's own `+0x1AA`: *Rats* and *Grain
        // found*. Any other id leaves the number with no words after it.
        match c.event_id {
            0x87 => {
                say(pen, ctx, canvas, FORECAST_GROUP, 0x19, at, EVENT_Y);
            }
            0x8B => {
                say(pen, ctx, canvas, FORECAST_GROUP, 0x1A, at, EVENT_Y);
            }
            _ => {}
        }
    }
    if advanced {
        weather_line(pen, ctx, canvas, c.grain_weather_change, NOUN_SACK);
    }

    if k.season_next == 1 {
        let at = count(pen, ctx, canvas, c.grain_sown_expected, NOUN_SACK, 0x40, 0xD8);
        say(pen, ctx, canvas, FORECAST_GROUP, 1, at, 0xD8);
        // `+0x230 * g_grainYieldPerSack`, an i32 product in the original.
        let yielded = c.grain_sown_expected.wrapping_mul(k.tables.grain.yield_per_sack);
        let at = count(pen, ctx, canvas, yielded, NOUN_SACK, 0x40, 0xE8);
        say(pen, ctx, canvas, FORECAST_GROUP, 2, at, 0xE8);
    } else {
        let seasons = match k.season_next {
            2 => 3,
            3 => 2,
            _ => 1,
        };
        let crop = if k.season_next == 4 { c.crop[2] } else { c.grain_grown_expected };
        let at = count(pen, ctx, canvas, crop, NOUN_SACK, 0x40, 0xD8);
        let at = say(pen, ctx, canvas, FORECAST_GROUP, 3, at, 0xD8);
        count(pen, ctx, canvas, seasons, NOUN_SEASON, at, 0xD8);
        let at = say(pen, ctx, canvas, FORECAST_GROUP, 0, 0x40, 0xE8);
        let at = count(pen, ctx, canvas, c.crop[0], NOUN_SACK, at, 0xE8);
        say(pen, ctx, canvas, FORECAST_GROUP, 4, at, 0xE8);
    }

    say(pen, ctx, canvas, FORECAST_GROUP, 0x1B, 0x40, 0x108);
    if c.grain_eaten == 0 {
        zero(pen, canvas, 0x130, 0x108);
    } else {
        delta(pen, canvas, c.grain_eaten.wrapping_neg(), 0x128, 0x108);
    }
    say(pen, ctx, canvas, FORECAST_GROUP, 0x1C, 0x40, 0x118);
    if c.grain_change_expected == 0 {
        zero(pen, canvas, 0x130, 0x118);
    } else {
        delta(pen, canvas, c.grain_change_expected, 0x128, 0x118);
    }
}

/// **`Panel_JobCattle` (`0x00413B30`)**, 29 call sites.
///
/// ```text
/// Ui_DrawCount(herd, 4, 0x130, 0x88)
/// herdCrowding 10 / 20 / 30 / else -> 77/8 / 9 / 10 / 11 at (0x40, 0xA0)
/// +0x274 == 0 ? 77/0x13 at (0x40, 0xB0)
///             : Ui_DrawCount(+0x274, 4, 0x40, 0xB0) + 77/0x14 (0x88) 0x15 (0x89) 0x16 (0x8C) 0x17 (0x8D)
/// [adv] the weather line on +0x270
/// 77/5 at (0x40, 0xD8), Ui_DrawCount(births expected, 4, 0x130, 0xD8)
/// 77/6 at (0x40, 0xE8), Ui_DrawCount(deaths expected, 4, 0x130, 0xE8)
/// 77/7 at (0x40, 0xF8), births == deaths ? zero at 0x130 : Ui_DrawDelta(births - deaths, …, 0x128)
/// 77/0x1B at (0x40, 0x108), herdEaten == 0 ? zero : Ui_DrawDelta(-herdEaten, …)
/// 77/0x1C at (0x40, 0x118), +0x258 == 0 ? zero : Ui_DrawDelta(+0x258, …)
/// ```
fn cattle(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County) {
    let advanced = ctx.game.kingdom.options.advanced_farming;
    count(pen, ctx, canvas, c.herd, NOUN_ANIMAL, 0x130, 0x88);
    let band = match c.herd_crowding {
        10 => 8,
        20 => 9,
        30 => 10,
        _ => 11,
    };
    say(pen, ctx, canvas, FORECAST_GROUP, band, 0x40, 0xA0);

    // `+0x274` — what last season's random event did to the herd.
    const EVENT_Y: i32 = 0xB0;
    if c.herd_event_change == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x13, 0x40, EVENT_Y);
    } else {
        let at = count(pen, ctx, canvas, c.herd_event_change, NOUN_ANIMAL, 0x40, EVENT_Y);
        let word = match c.event_id {
            0x88 => Some(0x14),
            0x89 => Some(0x15),
            0x8C => Some(0x16),
            0x8D => Some(0x17),
            _ => None,
        };
        if let Some(index) = word {
            say(pen, ctx, canvas, FORECAST_GROUP, index, at, EVENT_Y);
        }
    }
    if advanced {
        weather_line(pen, ctx, canvas, c.herd_weather_change, NOUN_ANIMAL);
    }

    say(pen, ctx, canvas, FORECAST_GROUP, 5, 0x40, 0xD8);
    count(pen, ctx, canvas, c.herd_births_expected, NOUN_ANIMAL, 0x130, 0xD8);
    say(pen, ctx, canvas, FORECAST_GROUP, 6, 0x40, 0xE8);
    count(pen, ctx, canvas, c.herd_deaths_expected, NOUN_ANIMAL, 0x130, 0xE8);

    say(pen, ctx, canvas, FORECAST_GROUP, 7, 0x40, 0xF8);
    if c.herd_births_expected == c.herd_deaths_expected {
        zero(pen, canvas, 0x130, 0xF8);
    } else {
        let farming = c.herd_births_expected.wrapping_sub(c.herd_deaths_expected);
        delta(pen, canvas, farming, 0x128, 0xF8);
    }
    say(pen, ctx, canvas, FORECAST_GROUP, 0x1B, 0x40, 0x108);
    if c.herd_eaten == 0 {
        zero(pen, canvas, 0x130, 0x108);
    } else {
        delta(pen, canvas, c.herd_eaten.wrapping_neg(), 0x128, 0x108);
    }
    say(pen, ctx, canvas, FORECAST_GROUP, 0x1C, 0x40, 0x118);
    if c.herd_change_expected == 0 {
        zero(pen, canvas, 0x130, 0x118);
    } else {
        delta(pen, canvas, c.herd_change_expected, 0x128, 0x118);
    }
}

/// **`Panel_JobReclamation` (`0x004140F3`)**, 6 call sites.
///
/// ```text
/// g_penAdvance = 0;
/// Ui_DrawNumber((byte) +0x204, '@', "", 0x40, 0xB8)
/// Eng_DrawString(77, +0x204 == 1 ? 0xC : 0xD, pen + 0x40, 0xB8)
/// +0x214 == 0 ? 77/0xF at (0x40, 200)
///             : 77/0xE at (0x40, 200) + Ui_DrawCount(+0x214, 0x42, pen + 0x40, 200)
/// ```
fn reclamation(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County) {
    // `(uint)(byte)field_0x204` — the byte, whatever our wider field holds.
    let fields = i32::from(c.fields_reclaiming as u8);
    let at = pen.number_in(Face::Body, canvas, 0x40, 0xB8, fields, '@', "", BODY_INK);
    say(pen, ctx, canvas, FORECAST_GROUP, if fields == 1 { 0x0C } else { 0x0D }, at, 0xB8);
    if c.reclaim_seasons_to_next == 0 {
        say(pen, ctx, canvas, FORECAST_GROUP, 0x0F, 0x40, 200);
    } else {
        let at = say(pen, ctx, canvas, FORECAST_GROUP, 0x0E, 0x40, 200);
        count(pen, ctx, canvas, c.reclaim_seasons_to_next, NOUN_SEASON, at, 200);
    }
}

/// **`Panel_JobIndustry` (`0x00412E6B`)**, 12 call sites, for iron, stone and
/// wood. One-based jobs 5, 6 and 7 map to industry records 1, 3 and 0 and to
/// group 8 nouns `0xC`, `0xE` and `0x10` — *Tonne*, three times over.
///
/// ```text
/// [adv] Eng_DrawString(76, 0, 0x40, 0xA0) + Ui_DrawNumber((char) +0x294 + r*0x18, '@', "%", pen + 0x40, 0xA0)
/// Ui_DrawCount(+0x2A8 + r*0x18, noun, 0x40, 0xB0) + 76/1
/// iron:  Ui_DrawCount(+0x284, noun, 0x40, 0xC0) + 76/2
/// stone: Ui_DrawCount(+0x28C, noun, 0x40, 0xC0) + 76/3
/// wood:  Ui_DrawCount(+0x280, noun, 0x40, 0xC0) + 76/2
///        Ui_DrawCount(+0x288, noun, 0x40, 0xD0) + 76/3
/// ```
///
/// `+0x280 … +0x28C` are `Industry_LabourEstimate`'s four figures, which
/// [`l2_kingdom::industry::panel_figures`] recomputes (C164): the blacksmiths'
/// wood and iron, and the castle's wood and stone still owed. The painter's
/// `g_jobPanelJob == 8` arm, which looks up a weapon's noun, is unreachable —
/// `Panel_JobDetail` calls this for 5, 6 and 7 only.
fn industry(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, c: &County, job: usize) {
    let k = &ctx.game.kingdom;
    let (record, noun) = match job {
        JOB_IRON_MINING => (Commodity::Iron, NOUN_IRON),
        JOB_STONE_QUARRYING => (Commodity::Stone, NOUN_STONE),
        _ => (Commodity::Wood, NOUN_WOOD),
    };
    let r = &c.industry[record.index()];
    if k.options.advanced_farming {
        let at = say(pen, ctx, canvas, INDUSTRY_GROUP, 0, 0x40, 0xA0);
        // `(int)*(char *)` — the efficiency byte, signed.
        let efficiency = i32::from(r.efficiency as u8 as i8);
        pen.number_in(Face::Body, canvas, at, 0xA0, efficiency, '@', "%", BODY_INK);
    }
    let at = count(pen, ctx, canvas, r.next_season, noun, 0x40, 0xB0);
    say(pen, ctx, canvas, INDUSTRY_GROUP, 1, at, 0xB0);

    let [smiths_wood, smiths_iron, castle_wood, castle_stone] =
        l2_kingdom::industry::panel_figures(&k.tables, c);
    match job {
        JOB_IRON_MINING => {
            let at = count(pen, ctx, canvas, smiths_iron, noun, 0x40, 0xC0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 2, at, 0xC0);
        }
        JOB_STONE_QUARRYING => {
            let at = count(pen, ctx, canvas, castle_stone, noun, 0x40, 0xC0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 3, at, 0xC0);
        }
        _ => {
            let at = count(pen, ctx, canvas, smiths_wood, noun, 0x40, 0xC0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 2, at, 0xC0);
            let at = count(pen, ctx, canvas, castle_wood, noun, 0x40, 0xD0);
            say(pen, ctx, canvas, INDUSTRY_GROUP, 3, at, 0xD0);
        }
    }
}

