#![allow(unused_imports)]
use super::*;

use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::message::{self, category, Prompt, Record, Shape};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};


pub(crate) fn county_name(ctx: &Ctx, id: u8) -> String {
    let name = ctx.assets.shell.text(message::GROUP_COUNTY, ctx.game.map_slot * 20 + id as usize);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// `g_playerNames[realm]`, with `L2.eng` group 7 standing in for a realm whose
/// name
/// place.
///
/// **Every screen that names a lord comes here**, and that is the point of it
/// being one function. `Ui_DrawText(&g_playerNames + realm * 0x2C, …)` is the
/// original's draw at all of them — the court, the battle prompt, the county
/// strip's third line (`CountyStrip_Draw` `0x0040F7D3`, whose call is
/// `FUN_004025D7(&g_playerNames + owner * 0x2C, 0x1E0, 0x118, 0xA0, …)`),
/// `Diplo_DrawScreen`'s heading, `Diplo_DrawLordCard`'s caption and the three
/// compose dialogs —
/// Three of them carried their own copy and each invented `REALM n` for a world
/// that never came through the front end; `docs/decisions.md` C189 fixed the
/// first two and this is the rest.
///
/// **Group 7 is indexed by the `lord`, not by the realm.** `Eng_Seek(7,
/// realm[+0x07])` is what new-game setup calls before copying sixteen bytes
/// into `g_playerNames`, so falling back to it *is* reconstructing what the
/// front end would have put there. **[V]**
pub fn lord_name(ctx: &Ctx, realm: u8) -> String {
    let named = ctx.game.player_names.get(realm as usize).map(|n| n.as_str()).unwrap_or_default();
    if !named.is_empty() {
        return named;
    }
    let lord = ctx.game.kingdom.realms.get(realm as usize).map_or(0, |r| r.lord);
    let s = ctx.assets.shell.text(7, lord.min(4) as usize);
    if s.is_empty() {
        format!("REALM {realm}")
    } else {
        s.to_string()
    }
}

pub(crate) fn label(ctx: &Ctx, group: u16) -> String {
    let s = crate::arrival::words(&ctx.assets.shell, group, 0);
    if s.is_empty() {
        format!("MESSAGE {group}")
    } else {
        s
    }
}

/// The body — `FUN_0040328E(group, variant + 1, …)`, one wrapped paragraph.
///
/// The player's own `L2.eng`, and our transcription only where the file gave
/// nothing and [`crate::arrival::TEXT`] has the string. Public so a test can
/// ask what a window says without reading pixels.
pub fn body(ctx: &Ctx, record: &Record) -> String {
    crate::arrival::words(&ctx.assets.shell, record.group, record.body_index())
}


/// So `+0x13` is not spare: it puts a **second realm's** name where the group's
/// own label would have gone.
pub(super) fn draw_notice(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    let heading = if record.county != 0 {
        county_name(ctx, record.county)
    } else if record.spare != 0 {
        lord_name(ctx, record.spare)
    } else {
        label(ctx, record.group)
    };
    if record.county == 0 && record.spare != 0 {
        pen.heading(canvas, f.x + 0x20, f.y + 0x20, &heading, font::TEXT);
    } else {
        pen.heading_centred(canvas, f.x + 0x10, f.y + 0x20, f.w - 0x20, &heading, font::TEXT);
    }
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x50, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Category `0x0D`, a county taken, with animations off** — `Msg_DrawWindow`'s
/// unanimated capture branch, which is not category 0's layout. `[V]`:
///
/// ```c
/// FUN_004093e0(0x20, 0xa0, 0x1a, 0xc);
/// Ui_OkButton(x + w - 0x30, h + y - 0x30, 0);
/// Ui_DrawCentred(100, g_scenarioIndex * 0x14 + county, x + 0x10, y + 0x20, w - 0x20, heading);
/// FUN_0040328e(g_messageGroup, 1, x + 0x20, y + 0x40, w - 0x40, 400, 0, 0, body);
/// ```
pub(super) fn draw_capture(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    let heading = county_name(ctx, record.county);
    pen.heading_centred(canvas, f.x + 0x10, f.y + 0x20, f.w - 0x20, &heading, font::TEXT);
    let words = crate::arrival::words(&ctx.assets.shell, record.group, 1);
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x40, f.w - 0x40, &words, font::TEXT);
}

pub(super) fn draw_letter(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    draw_portrait(pen, ctx, canvas, record.from, f);
    let from = ctx.assets.shell.text(message::GROUP_FROM, 0).to_string();
    let mut x = pen.heading(canvas, f.x + 0x6C, f.y + 0x20, &from, font::TEXT);
    pen.heading(canvas, x, f.y + 0x20, &lord_name(ctx, record.from), font::TEXT);

    x = pen.body(canvas, f.x + 0x6C, f.y + 0x40, &label(ctx, record.group), font::TEXT);
    if record.category == category::PAY_PROMPT {
        let price = ctx.game.kingdom.diplomacy.help_price;
        pen.count(canvas, x, f.y + 0x40, price, 0, font::TEXT);
    } else if record.spare != 0 {
        pen.body(canvas, x, f.y + 0x40, &lord_name(ctx, record.spare), font::TEXT);
    } else if record.county != 0 {
        pen.body(canvas, x, f.y + 0x40, &county_name(ctx, record.county), font::TEXT);
    }
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x70, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Category `0x02`** — the portrait panel with `L2.eng` 109/1 over the county
/// name, both centred in 0x140.
///
/// The well is **not** empty: the arm blits a face into it like a lord's letter
/// does, only the frame is [`peasant_face_frame`]'s and not a realm's. The
/// county the picture answers to is the record's — `DAT_0055CE54`, the same
/// byte the heading's county name is read with two lines below.
pub(super) fn draw_county_portrait(
    pen: &Pen,
    ctx: &Ctx,
    canvas: &mut Canvas,
    record: &Record,
    f: message::Frame,
) {
    let population =
        ctx.game.kingdom.counties.get(record.county as usize).map_or(0, |c| c.population);
    draw_face(pen, canvas, peasant_face_frame(population), f);
    let heading = ctx.assets.shell.text(message::GROUP_FROM, 1).to_string();
    pen.heading_centred(canvas, f.x + 0x6C, f.y + 0x18, 0x140, &heading, font::TEXT);
    let county = county_name(ctx, record.county);
    pen.heading_centred(canvas, f.x + 0x6C, f.y + 0x38, 0x140, &county, font::TEXT);
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x70, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Category `0x0E`, the ending.** The name at the top is the *local player's*
/// for group 225 and the *sender's* for everything else —
/// one layout serve *"Victory!"*, *"Defeat!"* and an AI's obituary.
///
/// The well is **not** empty: the arm calls `FUN_00475D73(DAT_00553EE0)` and
/// blits the frame like every other portrait layout. `DAT_00553EE0` is
/// `g_messageFrom`, so the face is the **sender's**, whoever the heading names
/// — the dead lord for an obituary, the player himself for group 224 *Defeat!*
///
/// (`from == g_localPlayer` is what makes it his), and the realm-0 end of the
/// ladder for group 225, which `Msg_Enqueue(0, g_localPlayer, 0xE1, …)` posts
/// with `from = 0`.
pub(super) fn draw_ending(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    draw_face(pen, canvas, realm_face_frame(ctx, record.from), f);
    let who = if record.group == l2_kingdom::victory::MSG_VICTORY {
        ctx.game.player
    } else {
        record.from
    };
    pen.heading_centred(canvas, f.x + 0x20, f.y + 0x20, 0x1A0, &lord_name(ctx, who), font::TEXT);
    pen.body_centred(
        canvas,
        f.x + 0x20,
        f.y + 0x48,
        f.w - 0x40,
        &label(ctx, record.group),
        font::TEXT,
    );
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x70, f.w - 0x40, &body(ctx, record), font::TEXT);
}

pub(super) fn draw_garrison(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    pen.heading_centred(
        canvas,
        f.x + 0x10,
        f.y + 0x20,
        f.w - 0x20,
        &label(ctx, record.group),
        font::TEXT,
    );
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x40, f.w - 0x40, &body(ctx, record), font::TEXT);

    let county = ctx.game.kingdom.counties.get(record.county as usize);
    let garrison = county
        .map(|c| c.garrison_unit)
        .filter(|u| *u != 0)
        .and_then(|u| ctx.game.kingdom.campaign.units.get(u))
        .map_or(0, |u| u.men);
    // `*(int *)(&DAT_004D8A0C + castleType * 4)` — which is
    // `CASTLE_GARRISON_CAP[castleType - 1]`, because the table at `0x004D8A10`
    // is indexed by castle types **1..=5**. It looks like an off-by-one and is
    // not one; the entry it would read at type 0 is the tail of
    // `CASTLE_WORKFORCE` and a county with no castle cannot be garrisoned.
    let capacity =
        county.map_or(0, |c| l2_kingdom::industry::garrison_cap(&ctx.game.kingdom.tables, c.castle_type));
    let room = capacity - garrison;
    // `Msg_DrawWindow`: `Ui_DrawNumber(room, '@', &DAT_004D700C, x + 0x20,
    // y + 0x70)` and then the noun at `x + g_penAdvance + 0x20`. The suffix is
    // **one space**, so the old no-lead-plus-space had the digits and the noun
    // both four pixels left. **[V]**
    let body = shell::Face::Body;
    let x = pen.number_in(body, canvas, f.x + 0x20, f.y + 0x70, room, '@', " ", font::TEXT);
    pen.eng(canvas, record.group as usize, 2, x, f.y + 0x70, font::TEXT);

    let x = pen.eng(canvas, record.group as usize, 3, f.x + 0x20, f.y + 0x80, font::TEXT);
    let men = ctx
        .game
        .kingdom
        .campaign
        .units
        .get(record.variant as usize)
        .map_or(0, |u| u.men) as i32;
    // `Ui_DrawNumber(menTotal, '@', &DAT_004D7010, …)`, one space. **[V]**
    pen.number_in(body, canvas, x, f.y + 0x80, men, '@', " ", font::TEXT);
    pen.eng(canvas, record.group as usize, 4, f.x + 0x20, f.y + 0xB0, font::TEXT);
}

/// **Category `0x0F`, a county's random event** — `Msg_DrawWindow`'s arm at
/// `00470000.c:2067`, whose record `FUN_00448D7E` posts as
/// `Msg_Enqueue(0, g_localPlayer, county.eventId, 0, 0x0F, county, 0, 0)`.
///
/// ```c
/// Ui_DrawCentred(group, 0, x + 0x10, y + 0x20, w - 0x20, &g_fontHeading, 0x3F);
/// FUN_0040328E(group, 1, x + 0x20, y + 0x40, w - 0x40, 400, 0, 0, &g_fontBody, 0x3F);
/// g_penAdvance = 0;
/// switch (g_counties[county].eventId) {       /* the county's, not the record's */
///   0x87 Rats:          Ui_DrawCount(+0x278, 2, x + 0x20, y + 0x90) + 77/0x19
///   0x8B Grain found:   Ui_DrawCount(+0x278, 2, …)                  + 77/0x1A
///   0x88 Mad cows:      Ui_DrawCount(+0x274, 4, …)                  + 77/0x14
///   0x89 Wolves:        Ui_DrawCount(+0x274, 4, …)                  + 77/0x15
///   0x8C Bad cattle:    Ui_DrawCount(+0x274, 4, …)                  + 77/0x16
///   0x8D Cow bonanza:   Ui_DrawCount(+0x274, 4, …)                  + 77/0x17
///   0x8A Plague:        Ui_DrawNumber(+0x2F8, '@', " ", …)          + 77/0x1D
///   0x8E Wedding fever: Ui_DrawNumber(+0x2F8, '@', " ", …)          + 77/0x1E
/// }
/// ```
///
/// **All eight number lines are drawn.** The last two were not: *Plague* and
/// *Wedding fever* print county `+0x2F8`, `Population_UpdateAll`'s event swing,
/// which nothing of ours carried. It is carried now —
/// `County::event_population_swing`, written by the population pass and imported
/// from the save (`docs/stored-fields.json`, `County+0x2F8`) — so the two draw
/// `Ui_DrawNumber(+0x2F8, '@', " ", …)` and 77/`0x1D` *"extra deaths."* or
/// 77/`0x1E` *"extra births."* after it.
///
/// `DAT_004D7050` and `DAT_004D7054` are both `" "`, dumped.
///
/// **The record is posted by [`message::post_event`]**, the port of
/// `FUN_00448D7E`, which `Machine::update` runs once a frame for
/// `Game::selected`
pub(super) fn draw_event(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    pen.heading_centred(
        canvas,
        f.x + 0x10,
        f.y + 0x20,
        f.w - 0x20,
        &label(ctx, record.group),
        font::TEXT,
    );
    let text = ctx.assets.shell.text(record.group as usize, 1).to_string();
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x40, f.w - 0x40, &text, font::TEXT);

    let Some(c) = ctx.game.kingdom.counties.get(record.county as usize) else { return };
    enum Line {
        Count(i32, usize),
        Number(i32),
    }
    let (line, word) = match c.event_id {
        0x87 => (Line::Count(c.grain_event_change, EVENT_NOUN_SACK), 0x19),
        0x8B => (Line::Count(c.grain_event_change, EVENT_NOUN_SACK), 0x1A),
        0x88 => (Line::Count(c.herd_event_change, EVENT_NOUN_ANIMAL), 0x14),
        0x89 => (Line::Count(c.herd_event_change, EVENT_NOUN_ANIMAL), 0x15),
        0x8C => (Line::Count(c.herd_event_change, EVENT_NOUN_ANIMAL), 0x16),
        0x8D => (Line::Count(c.herd_event_change, EVENT_NOUN_ANIMAL), 0x17),
        0x8A => (Line::Number(c.event_population_swing), 0x1D),
        0x8E => (Line::Number(c.event_population_swing), 0x1E),
        _ => return,
    };
    let (x, y) = (f.x + 0x20, f.y + 0x90);
    let next = match line {
        Line::Count(value, noun) => pen.count(canvas, x, y, value, noun, font::TEXT),
        Line::Number(value) => {
            pen.number_in(shell::Face::Body, canvas, x, y, value, '@', " ", font::TEXT)
        }
    };
    pen.eng(canvas, EVENT_GROUP, word, next, y, font::TEXT);
}

/// `L2.eng` group 77, whose indices `0x14` … `0x1A` are the event letters'
/// last words: *"died of disease."*, *"eaten by rats."* and the rest.
const EVENT_GROUP: usize = 77;
const EVENT_NOUN_SACK: usize = 2;
const EVENT_NOUN_ANIMAL: usize = 4;

pub(super) fn draw_tip(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, pointer: (i32, i32)) {
    if record.category != category::TIP {
        return;
    }
    let (mx, my) = pointer;
    let x = if mx < 0xF1 { mx + 0x20 } else { mx - 0xA8 }.clamp(0x50, 0xF0);
    let y = if my < 0xF1 { my + 0x20 } else { my - 0x50 }.clamp(0x50, 0xF0);
    pen.window(canvas, x, y, 0xB, 3, BOX_SET);
    pen.body_centred(canvas, x, y + 0x10, 0xB0, &label(ctx, record.group), font::TEXT);
}


fn draw_portrait_well(pen: &Pen, canvas: &mut Canvas, f: message::Frame) {
    let (dx, dy, w, h) = FACE_WELL;
    pen.inset(canvas, Rect::new(f.x + dx, f.y + dy, w, h));
}

/// The well and one `Faces.pl8` frame in it — `Ui_DrawInsetRect(x + 0xF, y +
/// 0x11, 0x52, 0x4E)` then `Blit_Raster(0x4EEB80, x + 0x10, y + 0x12, 0x50,
/// 0x4C)`. `0x4EEB80` is the buffer `FUN_00475D73` decoded the frame into, so
/// the pair is *one* portrait draw, and every arm of `Msg_DrawWindow` that has
/// a portrait writes it out identically.
fn draw_face(pen: &Pen, canvas: &mut Canvas, frame: usize, f: message::Frame) {
    draw_portrait_well(pen, canvas, f);
    if let Some(sheet) = pen.assets.sheet(FACES) {
        if let Some(bitmap) = sheet.frame(frame) {
            canvas.blit(&bitmap, f.x + FACE_AT.0, f.y + FACE_AT.1);
        }
    }
}

fn draw_portrait(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, realm: u8, f: message::Frame) {
    let r = ctx.game.kingdom.realms.get(realm as usize);
    if let (Some(r), Some(chrome)) = (r, pen.chrome) {
        let shield = SHIELD_BASE + r.shield_index.clamp(0, 5) as usize;
        chrome.draw_panel_frame(canvas, shield, f.x + f.w - 0x1E, f.y + 0x12);
    }
    draw_face(pen, canvas, realm_face_frame(ctx, realm), f);
}

/// `FUN_00475D73(realm)` — the argument every portrait arm passes is a realm,
/// and the ladder reads that realm's lord and whether a person plays him.
fn realm_face_frame(ctx: &Ctx, realm: u8) -> usize {
    let r = ctx.game.kingdom.realms.get(realm as usize);
    face_frame(r.map_or(0, |r| r.lord), r.is_some_and(|r| r.is_human), realm)
}

/// It is the build stamp's assertion, and it is here
/// because the test may not know the geometry — computing the probe from the
/// same constants the code reads is what makes an ablation prove nothing.
pub fn repaint_clickables(ctx: &Ctx, canvas: &mut Canvas, record: &Record) {
    let pen = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };
    if let Some(prompt) = record.answer_widgets() {
        draw_prompt(&pen, canvas, prompt, &Press::new());
    }
    if record.shape().has_ok_button() {
        if let Some(frame) = window_frame(ctx, record) {
            let (x, y) = frame.ok_button();
            pen.ok_button(canvas, x, y, 0);
        }
    }
}


pub fn window_frame(ctx: &Ctx, record: &Record) -> Option<message::Frame> {
    match record.shape() {
        Shape::Paragraphs(n) => Some(tip_layout(ctx, record, n).0.frame),
        _ => message::frame_of(record),
    }
}

/// **The tip window's paragraphs, broken into lines the way `FUN_0040328E`
/// breaks them**, and the layout those line counts give.
///
/// The words are [`crate::tip::words`] — the player's `L2.eng`, and our
/// transcription only where the file is silent — and they are measured in the
/// body font the paragraphs are drawn in, which is `&g_fontBody` in both of
/// `Msg_DrawWindow`'s loops.
pub fn tip_layout(ctx: &Ctx, record: &Record, n: usize) -> (message::Paragraphs, Vec<Vec<String>>) {
    let texts: Vec<Vec<String>> = (1..=n)
        .map(|i| {
            let words = crate::tip::words(&ctx.assets.shell, record.group, i);
            message::break_lines(&words, message::PARAGRAPH_WIDTH, |c| glyph_width(ctx, c))
        })
        .collect();
    let lines: Vec<usize> = texts.iter().map(Vec::len).collect();
    (message::paragraph_layout(&lines), texts)
}

/// `FUN_004015B9(c, &g_fontBody)` — one glyph's advance. `[I]` for a character
/// the font has no frame for: the original answers 0 and
/// [`crate::shell::font::Font::width`] answers a space's advance; no tip string
/// has been found to contain one.
fn glyph_width(ctx: &Ctx, c: char) -> i32 {
    let s = c.to_string();
    match &ctx.assets.shell.body {
        Some(f) => f.width(&s),
        None => l2_view::text::width(&s),
    }
}

pub(super) fn draw_paragraphs(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, n: usize) {
    let (layout, texts) = tip_layout(ctx, record, n);
    let f = layout.frame;
    pen.window(canvas, f.x, f.y, f.w / 16, f.h / 16, BOX_SET);
    let heading = crate::tip::words(&ctx.assets.shell, record.group, 0);
    pen.heading(canvas, layout.heading.0, layout.heading.1, &heading, font::TEXT);
    for (lines, top) in texts.iter().zip(&layout.tops) {
        for (k, line) in lines.iter().enumerate() {
            pen.body(canvas, f.x + 0x10, top + 0x10 * k as i32, line, font::TEXT);
        }
    }
    let (x, y) = f.ok_button();
    pen.ok_button(canvas, x, y, 0);
}

pub(super) fn draw_prompt(pen: &Pen, canvas: &mut Canvas, prompt: Prompt, press: &Press) {
    let [yes, no] = prompt.widgets();
    // `Widget_Draw` adds one to the frame while `+0x0D` runs.
    for (i, (at, frame)) in [(yes, Prompt::FRAME_YES), (no, Prompt::FRAME_NO)].into_iter().enumerate() {
        let frame = if press.is_pressed(i) { frame + 1 } else { frame };
        if !pen.system_frame(canvas, frame, at.0, at.1) {
            crate::shell::button_recess(canvas, at.0, at.1, Prompt::SIDE, Prompt::SIDE);
        }
    }
}


