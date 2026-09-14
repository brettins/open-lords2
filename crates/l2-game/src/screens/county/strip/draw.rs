#![allow(unused_imports)]
use super::*;
use super::render_helpers::*;
use super::*;
use super::layout::*;
use super::draw::*;
use l2_kingdom::tables::{
    HEALTH_BAND_NAMES, JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT, RATION_NAMES,
};
use l2_view::chrome::{self, misc_cty, system};
use l2_view::{text, Canvas};
use crate::game::{MAX_RATION_SPLIT, MAX_TAX_RATE};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen, TRAILING};
use crate::widget;

/// **The county strip: what `CountyStrip_Draw` (`0x0040F7D3`) puts in the
/// 162 × 94 plate at (478, 156), at its own coordinates.**
///
/// `focus` outlines one quadrant. That outline is ours — the original's
/// quadrants are invisible because it is a mouse game — and it is drawn only
/// when a panel is open, so the map's sidebar carries none.
pub fn draw_strip(ctx: &Ctx, canvas: &mut Canvas, county: u8, focus: Option<Panel>) {
    let ink = &ctx.assets.ink;
    let Some(c) = ctx.game.kingdom.counties.get(county as usize) else { return };
    let mine = ctx.game.is_players(county);
    // **The strip's ink is the original's literal `0x3F`, which is black.**
    //
    // Every string `CountyStrip_Draw` writes — the county's name, its
    // population, its happiness, the tax rate, both group-61 captions — passes
    // colour `0x3F` to `Ui_DrawText`, and `0x3F` in `Base01.256` is
    // `rgb(0, 0, 0)`. The one exception is the achieved ration when it is not
    // the wanted one, which is `0xF9`.
    //
    // These were `ink.text`, which resolves to *white*, and a player reported
    // it: *"the text should be black not white over the happiness."*
    // right.
    // that this text is written on the **original's own plate** — `Misc_cty`
    // frame `0x37` — so the index is a reading of the binary and not a choice
    // of ours.
    // fallback is our own ink.
    let strip_ink =
        if ctx.assets.chrome.is_some() { crate::shell::font::TEXT } else { ink.text };
    let strip_bad =
        if ctx.assets.chrome.is_some() { crate::shell::font::HIGHLIGHT } else { ink.bad };

    // `Ui_DrawCentred(100, scenarioIndex*0x14 + county, 0x1E0, 0xA5, 0xA0,
    // &g_fontBody, 0x3F)` — the county's name, centred across 160 pixels at
    // (480, 165), and **in the 14-pixel body font, not the strip's 9-pixel
    // one**. `docs/screens-county.md` §2.1 says "all of it in the 9-pixel
    // font"; the name is the exception, and `DAT_005AEA40` is set to 1 only
    // *after* it, so the name is embossed and the numbers below it are not.
    //
// The unowned plate is 162 × 274 and puts the name
    // fifteen pixels lower, at `0xB4`; that is the original's own difference,
    // not a rounding of ours.
    //
    // **The emboss is the parchment pair on both plates, and over the cloudy
    // one that is a bug of the original's that we reproduce.** `Ui_DrawText`
    // picks its emboss from `g_screenId` and two globals, never from the
    // caller, and `CountyStrip_Draw` sets neither of them around this call —
    // so the name is drawn with `0x10`/`0x1F`, a dark olive over a pale
    // parchment yellow, whether it is standing on the parchment plate
    // (`Misc_cty` frame `0x37`) or on the cloudy one (frame `0x3A`). On the
    // cloudy plate the highlight is a colour that is not in the picture and
    // the name reads as though it were fading into paper that is not there.
    //
    // A player reported it and asked for it to be reproduced *and* switchable:
    // [`crate::game::Quirks::grey_county_name`] is the switch and it is off by
    // default. `docs/bugs.md` B64.
    let name = county_name(ctx, county);
    let name_y = if mine { 165 } else { 180 };
    let name_style = if ctx.assets.quirks.grey_county_name && !mine {
        crate::shell::font::Style {
            colour: strip_ink,
            shadow: Some(crate::shell::font::SHADOW_GREY),
            caps: None,
        }
    } else {
        crate::shell::font::Style::new(strip_ink)
    };
    body_centred_styled(ctx, canvas, 480, name_y, 160, &name, name_style);

    if !mine {
        // **`owner != 0` — and unclaimed land is a third case, not a second.**
        //
        // `CountyStrip_Draw`'s else-arm draws the cloudy plate and the name for
        // *any* county that is not yours, and then:
        //
        // ```c
        // if (g_counties[g_selectedCounty].owner != 0) {
        //   DAT_0058fe9c = 1;                                   /* the grey emboss */
        //   colour = g_realms[owner].field_0x8;
        //   Ui_DrawCentred(0xf, 0, 0x1e0, 0xf0,  0xa0, &g_fontBody, colour);
        //   Ui_DrawCentred(0xf, 1, 0x1e0, 0x104, 0xa0, &g_fontBody, colour);
        //   FUN_004025d7(&g_playerNames + owner * 0x2c, 0x1e0, 0x118, 0xa0, &g_fontBody, colour);
        //   DAT_0058fe9c = 0;
        // }
        // ```
        //
        // So a county nobody holds gets the plate and its name and **nothing
        // else** — no banner, no owner line. `L2.eng` group 15 is two strings,
        // `"Sovereign land"` and `"of"`, and the third line is a lord's name
        // out of `g_playerNames`;
        // unowned county because the original never needs one.
        //
        // We drew `SOVEREIGN LAND / OF / UNCLAIMED` here, which is a sentence
        // the original cannot produce. A player reported it in one line:
        // *"Unclaimed lands have no 'sovereign land of'."*
        if c.owner == 0 {
            return;
        }
        // The three lines carry the **grey** emboss — `DAT_0058FE9C = 1` —
        // and not the parchment one the name above them uses. Two emboss pairs
        // in one plate is the original's own arrangement, and ours had
        // collapsed them into one.
        //
        // **All three take the same pen**, and that is worth stating because it
        // is the natural place to expect a difference. `CountyStrip_Draw`
        // computes `colour` once and passes it to all of them — the banner, the
        // "of", and the lord's name —
        // realm's colour is the *pen*, on every line:
        //
        // ```c
        // colour = g_realms[owner].field_0x8;
        // Ui_DrawCentred(0xf, 0, 0x1e0, 0xf0,  0xa0, &g_fontBody, colour);
        // Ui_DrawCentred(0xf, 1, 0x1e0, 0x104, 0xa0, &g_fontBody, colour);
        // FUN_004025d7(&g_playerNames + owner * 0x2c, 0x1e0, 0x118, 0xa0, &g_fontBody, colour);
        // ```
        //
        // **The third line is a name.** It read `REALM 3` because nothing in
        // this workspace filled `g_playerNames`; the front end fills it at
        // *Start* — the local player's from what was typed on setup page 4, an
        // AI lord's from `L2.eng` group 7 — so the line says *SOVEREIGN LAND /
        // OF / THE BARON* the way the original's does.
        //
        // **The fallback is not decoration.** A world that did not come through
        // the front end — a `.sav` imported by `l2-scenario`, a kingdom a test
        // built — has no names in it, and an empty third line under two full
// ones looks like a drawing fault. It is
        // [`super::message::lord_name`]'s, shared with the court, the battle
        // prompt and the three diplomacy screens: `g_playerNames` and then
        // `L2.eng` group 7 by the realm's **lord**, which is the pair
        // `Game_NewGame` itself uses.
        let owner = super::super::message::lord_name(ctx, c.owner);
        // **The pen is keyed by the realm's shield, not by its id**, and that
        // was the bug a player reported as *"the sovereign land text has the
        // wrong colours … the counties seem to have the right colours … but the
        // text doesn't match that"*. This drew from `Ink::realm` — a table of
        // our own, indexed by the **realm number** — while the minimap tint,
        // the menu-bar banner and the campaign flag all go through the shield.
        // Two keys and three tables for one fact.
        //
        // The shield is what the *human picks*; the AI lords take the slots
        // left over.
        // twenty-five realms across this project's eleven save fixtures fly a
        // shield that is not their id — which is how the wrong key was caught.
        // `docs/decisions.md` C112.
        let shield = ctx.game.kingdom.realms.get(c.owner as usize).map_or(0, |r| r.shield_index);
        // The fallback is `Ink`'s and is **visibly** ours: a world with no
        // shields is a placeholder world, and it should not borrow one of the
        // game's five real colours to look finished. See
        // `l2_view::chrome::realm_pen` on why this does not clamp to 1.
        let colour = l2_view::chrome::realm_pen(shield)
            .unwrap_or_else(|| ink.realm.get(c.owner as usize).copied().unwrap_or(ink.text));
        let style = crate::shell::font::Style {
            colour,
            shadow: Some(crate::shell::font::SHADOW_GREY),
            caps: None,
        };
        let banner = eng(ctx, 15, 0, "SOVEREIGN LAND");
        body_centred_styled(ctx, canvas, 480, 240, 160, &banner, style);
        body_centred_styled(ctx, canvas, 480, 260, 160, &eng(ctx, 15, 1, "OF"), style);
        body_centred_styled(ctx, canvas, 480, 280, 160, &owner, style);
        return;
    }

    // `Ui_DrawNumber(pop, ' ', " ", 0x1FC, 0xBD, &g_fontSmall, 0x3F)` and the
    // identical call for happiness at `0x25A`.
    //
    // **Both are left origins.** `Ui_DrawNumber` takes no anchoring argument —
    // the two calls differ only in their value and their x — so the happiness
// figure starts at 602. We right-anchored it,
    // which put a two-digit number on top of the plate's heart and would have
    // put a three-digit one further left still. See `docs/decisions.md` C42.
    strip_number(ctx, canvas, c.population, ' ', " ", 508, 189, strip_ink);
    strip_number(ctx, canvas, c.happiness as i32, ' ', " ", 602, 189, strip_ink);
    // Group 61, centred in 76 pixels at (0x1E0, 0xD5) and (0x234, 0xD5), and
    // **in colour 0x3F, the same as the numbers** — the captions are not dimmed
    // in the original and ours were unreadable against the plate.
    strip_centred(ctx, canvas, 480, 213, 76, &line_text(ctx, g61::STRIP_TAX), strip_ink);
    strip_centred(ctx, canvas, 564, 213, 76, &line_text(ctx, g61::STRIP_RATION), strip_ink);
    // (0x1FA, 0xE2), and the ration level centred in 76 at (0x234, 0xE2).
    strip_number(ctx, canvas, c.tax_rate as i32, ' ', "%", 506, 226, strip_ink);
    // "Red when it differs from rationWanted" is the original's own rule, and
// the colour it picks is `0xF9`.
    let colour = if c.ration_achieved == c.ration_wanted { strip_ink } else { strip_bad };
    strip_centred(ctx, canvas, 564, 226, 76, &ration_label(ctx, c.ration_achieved), colour);

    // Pl8_DrawFrameHere(g_miscCtySheet, band + 0x46, 0x228, 0xB5) — the
    // five-level health thermometer, in the dead band the hotspot leaves.
    let band = c.health_band.min(4);
    let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| {
        ch.draw_misc(canvas, THERMOMETER_FRAME + band as usize, THERMOMETER.0, THERMOMETER.1)
    });
    if !drawn {
        // OURS: a five-segment bar where the thermometer goes.
        for i in 0..5i32 {
            let lit = 4 - i <= band as i32;
            let y = THERMOMETER.1 + i * 12;
            let colour = if lit { ink.good } else { ink.border };
            canvas.fill_rect(THERMOMETER.0, y, THERMOMETER_W, 10, colour);
        }
    }

    // `Pl8_DrawFrame(g_miscCtySheet, 0x3D, share / 2 + 0x214, 0x106)` — the
    // farm/industry split's thumb, on the 162 × 52 plate at (478, 250) that
// `CountyStrip_Draw` paints as its last act. It is drawn *here*
    // by the map screen because the county panels repaint the whole sidebar
    // over the map, and a thumb only the map drew would vanish whenever a panel
    // was open.
    //
    // # The blue outline, which is a **frame**
    //
    // `CountyStrip_Draw`'s last branch, verbatim:
    //
    // ```c
    // if (county.labour[8].workers == 0)
    //     Pl8_DrawFrame(g_miscCtySheet, 0x3D, share / 2 + 0x214, 0x106);
    // else
    //     Pl8_DrawFrame(g_miscCtySheet, 0x55, share / 2 + 0x212, 0x104);
    // ```
    //
    // Slot 8 is *Idle townsfolk*, so **the thumb changes the moment anybody in
    // the county has nothing to do** — which is exactly what the player
    // remembered: *"the peasant slider I think had a blue outline if there
    // were idle peasants as well."* This module used to guess the castle job;
    // it is the idle pool.
    //
// And the outline is measurable. Frame `0x3D` is
    // 9 × 33 and frame `0x55` is 13 × 37 — four wider and four taller — drawn
    // two pixels left and two pixels up, so it is *the same thumb inside a
    // two-pixel ring*. Every one of the ring's 124 pixels is one of three
    // palette entries, and all three are blue: `95` = `rgb(0, 0, 121)`,
    // `65` = `rgb(157, 202, 234)` and `64` = `rgb(194, 230, 255)`.
    // `crates/l2-view/tests/install/main.rs` asserts that against the player's own
    // `Misc_cty.pl8`.
    let share = c.industry_share.clamp(0, 100);
    let idle = c.labour[JOB_IDLE_TOWNSFOLK] != 0;
    let (frame, tx, ty) = if idle {
        (misc_cty::SPLIT_THUMB_IDLE, share / 2 + 530, 260)
    } else {
        (misc_cty::SPLIT_THUMB, share / 2 + 532, 262)
    };
    let thumb = ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, tx, ty));
    if !thumb {
        // OURS, for an install with no `Misc_cty.pl8`: the ring is drawn as a
        // ring, because that is what it is.
        canvas.fill_rect(share / 2 + 532, 262, 9, 33, ink.highlight);
        if idle {
            widget::frame(canvas, Rect::new(share / 2 + 530, 260, 13, 37), ink.realm[2]);
        }
    }

    draw_produce_rows(ctx, canvas, c, strip_ink);
    // **The right-hand list**, which used to be a box of ours saying
    // `INDUSTRY / NOT DRAWN`. `CountyStrip_Draw` walks the two lists back to
    // back off the same `FUN_0040FEC1`, so they belong together here too.
    draw_industry_rows(ctx, canvas, c);

    // OURS: the original's quadrants are invisible. A one-pixel outline is how
    // a keyboard player sees which of the four is open — debug overlay only.
    if let (Some(p), true) = (focus, ctx.game.prefs.debug_overlay) {
        widget::frame(canvas, p.strip_hotspot(), ink.highlight);
    }
}

/// **The produce rows, and the blue outline that is the point of them.**
///
/// The plate below the slider — `Misc_cty` frame `0x38` at (478, 302) — carries
/// one row per thing the county makes. `FUN_0040FEC1` decides *which* rows,
/// into two lists, and `CountyStrip_Draw` then walks them; this is the **left**
/// list, the three farming rows, which is where the dairy is.
///
/// ```c
/// if (fieldsCattle || herd)     rows[n++] = 1;   // FUN_004100AF
/// if (fieldsGrain  || grain)    rows[n++] = 0;   // FUN_0041023A
/// if (fieldsReclaiming)         rows[n++] = 2;   // FUN_004103C5
/// DAT_0053E970 = n < 3 ? 0x3C : 0x2D;            // the row pitch
/// ```
///
/// Each drawer opens with the same three-way choice, and it is the one the
/// player asked about:
///
/// ```c
/// if (labour[slot].useful  < labour[slot].workers) ringed frame, two px up-left
/// else if (labour[slot].workers < labour[slot].wanted) shortfall frame
/// else                                                 plain frame
/// ```
///
/// So **the county's cow gets a blue ring around it the moment more people are
/// milking than the herd can use** — *"there's no blue outline for idle
/// peasants (eg too many on dairy)"*, exactly. The ceiling is
/// [`l2_kingdom::county::County::labour_useful`], the same word
/// `Village_RebuildIcons` uses to decide which peasants in the village are
/// drawn sitting down, and the floor is the one `Panel_JobDetail` already
/// colours the count red below.
///
/// # What is not here
///
/// * **The right-hand list** — wood, iron, stone, weapons and the castle. Three
///   of the five have no state at all (frames `0x2C`, `0x2D`, `0x2E`, drawn
///   flat), and the two that do — the blacksmith and the castle — pick their
///   frame from county `+0x290`, an unnamed byte, and from `+0x1B0`. Neither is
///   settled, so neither is drawn.
/// * **Four of the eight seasonal deltas** — the industry rows'. The three farm
///   rows' are drawn; this list is what is *not* here, and the paragraphs below
///   are kept because they are the reading.
///
/// # The seasonal deltas, and the report that found them missing
///
/// A player, before any of them were drawn: *"Sidebar doesn't show grain being
/// planted as a negative number."* He was right, and he was describing
/// **Spring**.
///
/// Every drawer follows its icon with a `Ui_DrawDelta` (`0x00402E0C`), which
/// is a *signed* number: `value < 0` draws `Ui_DrawNumber(-value, '-', …)` in
/// `colourNeg` (`0xF9`), `value > 0` gets a `'+'` lead in `colourPos`
/// (`0xFA`), and zero gets `'@'`, the blank glyph that keeps a zero
/// column-aligned. The minus is **not a separate mark** — it overwrites
/// `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves free
/// for a sign, and the whole string goes out in one `Ui_DrawText`. With
/// `mode == 0`, which is what all seven calls pass, a value of zero draws
/// **nothing at all**.
///
/// **It is not "the change since last season".** The tooltip layer says so in
/// the game's own words — `L2.eng` group 220 index 15 is *"Cattle, and change
/// next season"* and 16 is *"Wheat, and change next season"* — and the code
/// agrees: `County_RefreshEstimates(county, g_seasonNext)`.
///
/// **The grain row's value is county `+0x22C`, and it is written by a tail
/// this workspace did not have.** `Grain_LabourEstimate` (`0x0044D374`)
/// writes it *after* the search loop
/// [`l2_kingdom::land::grain_labour_estimate`] reproduces:
///
/// ```c
/// staff = county.labour[0].workers;                    /* the staffing */
/// county.field_0x230 = Grain_Sow(county, staff, county.grain);
/// if (season == 4) county.crop[2]      = Grain_Harvest(county, staff, county.crop[1]);
/// if (season == 2 || season == 3) county.field_0x2FC = Grain_Grow(county, staff, county.crop[1]);
///
/// if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
/// else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
/// else                  county.field_0x22C = -county.grainEaten;
/// ```
///
/// So in **Spring** the row is `−(sown) − eaten`, which cannot be anything but
/// negative — the player's sentence, exactly. Our port returned
/// `GrainEstimate { wanted, useful }` and stopped at the loop, so all four of
/// those writes were missing, and **the sign question never arose because the
/// number never arrived.** Nor could it be recovered from the estimate: the
/// loop calls `Grain_Sow(county, workers, grain − grainEaten)` and the tail
/// calls `Grain_Sow(county, staff, grain)` — a different third argument.
/// `crate::field`'s module docs already said the estimate round runs twice
/// "for … the panel forecasts, which the estimates fill from whatever the
/// allocator last decided"; these are those forecasts.
///
/// [`l2_kingdom::land::grain_preview`] is that tail, and
/// `crates/l2-game/tests/screens_county/main.rs`'s
/// `the_grain_row_draws_its_sowing_loss_from_the_brush_to_the_pixel` drives
/// it from the map brush to the glyph. **C123.**
///
/// The four industry rows read a different quantity again — commodity `c`'s
/// i32 at county `0x2A8 + c * 0x18`, the last field of its **own** `Industry`
/// record — and are drawn by [`draw_industry_rows`]. This said the word was
/// the next record's head and that stone's `0x2F0` ran past the array; the
/// array's base was four bytes low. `docs/decisions.md` C153.
fn draw_produce_rows(
    ctx: &Ctx,
    canvas: &mut Canvas,
    c: &l2_kingdom::county::County,
    strip_ink: u8,
) {
    // `FUN_0040FEC1`, in its own order: cattle, then grain, then reclamation —
    // and the same list `job_row_at` hit-tests, so the picture and the target
    // cannot drift apart.
    let rows = farm_rows(c);
    // `DAT_0053E970`: three rows or more and they close up.
    let pitch = farm_pitch(rows.len());

    for (n, &slot) in rows.iter().enumerate() {
        let y = pitch * n as i32;
        let (plain, short, ringed) = PRODUCE_ICONS[slot];
        let state = if c.labour_useful[slot] < c.labour[slot] {
            // The ringed frame, and its own position — two pixels up and left
            // of the plain one, because it is the plain one plus a ring.
            Some(ringed)
        } else if c.labour[slot] < c.labour_wanted[slot] {
            short
        } else {
            None
        };
        let (frame, x, dy) = match (slot, state) {
            (1, Some(f)) => (f, 482, 0x131),
            (1, None) => (plain, 484, 0x133),
            (0, Some(f)) if f == ringed => (f, 484, 0x12D),
            (0, Some(f)) => (f, 485, 0x12E),
            (0, None) => (plain, 486, 0x12F),
            (_, Some(f)) => (f, 491, 0x130),
            (_, None) => (plain, 493, 0x132),
        };
        let drawn =
            ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, x, y + dy));
        if !drawn {
            // OURS, with no `Misc_cty.pl8`: a label and, when it applies, the
            // ring — because the ring is the thing being said.
            let ink = &ctx.assets.ink;
            let name = ["GRAIN", "DAIRY", "RECLAIM"][slot];
            text::draw(canvas, x, y + dy + 8, name, ink.dim);
            if state == Some(ringed) {
                widget::frame(canvas, Rect::new(x - 2, y + dy - 2, 44, 32), ink.realm[2]);
            }
        }
        // **The row's forecast for next season.** `Ui_DrawDelta(value, 0, " ",
        // " ", 0x204, pitch*row + dy, &g_font10, 0xFA, 0xF9)` — the same `x` at
        // all three farm rows, and `dy` `0x139` for the two that have a stock
        // and `0x133` for reclamation, which has a countdown instead.
        //
        // **All three draw one, and all three are the tail of an estimate pass
        // whose loop was ported without it** — the same defect three times, in
        // one file, found once. C123, C128 and C129:
        //
        // | row | county | the tail |
        // |---|---|---|
        // | cattle | `+0x258` | `Herd_LabourEstimate` (`0x0044DD4D`), `(births − deaths) − herdEaten` |
        // | grain | `+0x22C` | `Grain_LabourEstimate` (`0x0044D374`), `−sown − eaten` entering Spring |
        // | reclamation | `+0x20C` | `Field_ReclaimEstimate` (`0x0044C278`), fields *finished* next season |
        //
        // The four industry rows are **not** the same fix and are still absent:
        // each reads an `i32` at the head of the `Industry` record *above* the
        // commodity its row is for, and settling that base is its own job.
        // `docs/draws-map.md` §5.5.
        let (delta, delta_dy) = match slot {
            1 => (c.herd_change_expected, 0x139),
            0 => (c.grain_change_expected, 0x139),
            _ => (c.reclaim_fields_finishing, 0x133),
        };
        strip_delta(ctx, canvas, delta, 0x204, y + delta_dy);
        // **The reclamation row's second figure**, and it is the only produce row
        // with one: `Ui_DrawNumber(county +0x214, ' ', " ", 0x20A, y + 0x143,
        // &g_font10, 0xFA)`, drawn **only when it is non-zero** — the original's
// own `if`.
        // a zero. `Field_ReclaimEstimate`'s tail computes it as *seasons until
        // the nearest-to-finished field is done*, rounded up, from the full
        // reclamation staffing.
        if slot == 2 && c.reclaim_seasons_to_next != 0 {
            // `&DAT_004D3D60` is `" "`, read out of the image.
            ten_number(ctx, canvas, c.reclaim_seasons_to_next, ' ', " ", 0x20A, y + 0x143, DELTA_POS);
        }
        // `Ui_DrawNumberRight(store, ' ', …, 0x1E0, y + 0x14D, 0x3C,
        // &g_fontBody, 0x3F)` — the store itself.
        //
        // **`Ui_DrawNumberRight` centres.** Its tail is `FUN_004025D7`, which
        // computes `x + (width − textWidth) / 2`; the name and the
        // `docs/symbols.json` comment both said right-aligned and both were
        // wrong, found independently by two draw audits. So this is centred in
        // sixty pixels from x = 480, not anchored at 540.
        // Reclamation's number is a field this project has not named, so that
        // row carries none.
        let store = match slot {
            0 => Some(c.grain),
            1 => Some(c.herd),
            _ => None,
        };
        if let Some(v) = store {
            body_number_centred(ctx, canvas, v, ' ', " ", 480, y + 0x14D, 0x3C, strip_ink);
        }
    }
}

/// **The five industry rows — the other half of the same plate, and the
/// seventeen draw calls a box of ours was standing on.**
///
/// The sidebar used to carry `INDUSTRY / NOT DRAWN` over the right half of the
/// jobs plate, on the reading recorded in [`draw_produce_rows`]'s own header:
/// *"three of the five have no state at all … and the two that do pick their
/// frame from bytes this project has not settled."* Both halves of that were
/// wrong, and both were checkable:
///
/// * **The two stateful rows' bytes are settled.** The blacksmith's frame is
///   `county[+0x290] + 0x30`, and `Industry_LabourEstimate` indexes
///   `&g_weaponCost + county[+0x290] * 8` with the same byte — so `+0x290` is
///   [`County::weapon_type`](l2_kingdom::county::County::weapon_type), which
///   this crate has had all along under a comment saying *"engine state"* with
///   no offset. The castle's are `+0x1D0` and `+0x1D4`, already carried as
///   `castle_stone_owed` and `castle_wood_owed`.
/// * **The three flat rows still draw a number**, and it is a *forecast*, not a
///   stock. `Ui_DrawDelta` at `(0x22C, pitch*row + 0x139)`, from county
///   `+0x2A8 + c*0x18` — see
///   [`Industry::next_season`](l2_kingdom::county::Industry::next_season).
///   `L2.eng` group 220's tooltips say what it is in the game's own words:
///   *"Wood produced next season"*, *"Stone …"*, *"Iron …"*, *"Weapons
///   produced. Click for smithy."*
///
/// So *"there is nothing to put here"* answered a question it also raised, and
/// the answer was no. `docs/draws-map.md` §5.5.
///
/// # The row order, and a document this contradicts
///
/// `FUN_0040FEC1` fills the right list with **labour slots** — 6 wood, 4 iron,
/// 5 stone, 7 blacksmith, 3 castle, pushed in the order wood, iron, stone,
/// weapons, castle — and `CountyStrip_Draw` dispatches on the slot:
///
/// | slot | painter | row |
/// |---|---|---|
/// | 4 | `FUN_00410502` | iron |
/// | 5 | `FUN_00410598` | stone |
/// | 6 | `FUN_0041062E` | wood |
/// | 7 | `FUN_004106C4` | weapons |
/// | 3 | `CountyStrip_DrawCastleIcon` (`0x004107D1`) | the castle |
///
/// **`docs/draws-map.md` §2 has the first three the wrong way round**, naming
/// `0x00410502` stone, `0x00410598` wood and `0x0041062E` iron. Two independent
/// readings say otherwise: the dispatch above, and `Unit_TrampleTile`
/// (`0x0046873F`), whose iron arm zeroes the same `industry + 2` word
/// `FUN_00410502` draws. `docs/decisions.md` C135.
fn draw_industry_rows(ctx: &Ctx, canvas: &mut Canvas, c: &l2_kingdom::county::County) {
    use l2_kingdom::tables::Commodity;

    // The same list `job_row_at` hit-tests, so the picture and the target
    // cannot drift apart — and `DAT_0053F04C`, the pitch, which is *three*
    // cases here against the farm column's two.
    let rows = industry_rows(c);
    let pitch = industry_pitch(rows.len());

    for (n, &slot) in rows.iter().enumerate() {
        let y = pitch * n as i32;
        // `DAT_0056D68C` is the row counter every one of the five painters
        // advances on the way out; `n` is it.
        let flat = match slot {
            4 => Some((misc_cty::INDUSTRY_IRON, misc_cty::INDUSTRY_X[0], Commodity::Iron)),
            5 => Some((misc_cty::INDUSTRY_STONE, misc_cty::INDUSTRY_X[1], Commodity::Stone)),
            6 => Some((misc_cty::INDUSTRY_WOOD, misc_cty::INDUSTRY_X[2], Commodity::Wood)),
            _ => None,
        };
        if let Some((frame, x, commodity)) = flat {
            // `Pl8_DrawFrame(g_miscCtySheet, frame, x, pitch*row + 0x133)`.
            draw_strip_icon(ctx, canvas, frame, x, y + 0x133, INDUSTRY_LABEL[commodity.index()]);
            strip_delta(ctx, canvas, c.industry[commodity.index()].next_season, 0x22C, y + 0x139);
            continue;
        }
        if slot == 7 {
            // **`FUN_004106C4`, the blacksmith — the one industry row that
            // reacts to its staffing.** The same three-way question the farm
            // rows ask, minus the shortfall arm:
            //
            // ```c
            // if (labour[7].useful < labour[7].workers)
            //     Pl8_DrawFrame(sheet, weaponType + 0x4F, 0x256, pitch*row + 0x131);
            // else
            //     Pl8_DrawFrame(sheet, weaponType + 0x30, 600,   pitch*row + 0x133);
            // ```
            //
            // Six weapon types, six frames each way — `misc_cty::RINGED_PAIRS`
            // rows 3..=8, which were read off the file before anything drew
            // them.
            let idle = c.labour_useful[7] < c.labour[7];
            let weapon = c.weapon_type.min(l2_kingdom::tables::WEAPON_TYPE_COUNT - 1);
            let (plain, ringed) = misc_cty::RINGED_PAIRS[3 + weapon];
            let (frame, x, dy) =
                if idle { (ringed, 0x256, 0x131) } else { (plain, 600, 0x133) };
            let drawn = ctx
                .assets
                .chrome
                .as_ref()
                .is_some_and(|ch| ch.draw_misc(canvas, frame, x, y + dy));
            if !drawn {
                let ink = &ctx.assets.ink;
                text::draw(canvas, x, y + dy + 8, "SMITH", ink.dim);
                if idle {
                    widget::frame(canvas, Rect::new(x, y + dy, 44, 32), ink.realm[2]);
                }
            }
            strip_delta(
                ctx,
                canvas,
                c.industry[Commodity::Weapons.index()].next_season,
                0x22C,
                y + 0x139,
            );
            continue;
        }
        if slot == 3 {
            draw_castle_row(ctx, canvas, c, y, n);
        }
    }
}

/// One `Misc_cty` frame with our own word behind it when the sheet is absent.
fn draw_strip_icon(ctx: &Ctx, canvas: &mut Canvas, frame: usize, x: i32, y: i32, label: &str) {
    let drawn = ctx.assets.chrome.as_ref().is_some_and(|ch| ch.draw_misc(canvas, frame, x, y));
    if !drawn {
        text::draw(canvas, x, y + 8, label, ctx.assets.ink.dim);
    }
}

/// **`CountyStrip_DrawCastleIcon` (`0x004107D1`)** — the castle's cell on the
/// strip, and eight draw calls in one small function.
///
/// ```c
/// nudge = (row < 2) ? 6 : 0;
/// if (county.castleSwitch == 0) return;                    /* the whole body is inside this */
/// if (labour[3].useful < labour[3].workers)
///      Pl8_DrawFrame(sheet, 0x4E, 0x255, pitch*row + nudge + 0x129);
/// else Pl8_DrawFrame(sheet, 0x40, 0x25B, pitch*row + nudge + 300);
/// if (stoneOwed == 0 && woodOwed == 0) {
///     if (seasons != 0) {
///         Ui_DrawNumber  (seasons, ' ', " ", 0x23C, …+0x13A, font10,   0xFA);
///         Ui_DrawUnitNoun(seasons, 0x42,     0x234, …+0x146, fontSmall, 0xFA);
///     }
/// } else {
///     …one of three materials icons at (0x23C, …)…
///     Eng_DrawString(0x47, 0x12, 0x234, …+0x146, fontSmall, 0xFA);   /* "Needed" */
/// }
/// ```
///
/// Three things worth having beyond the coordinates.
///
/// * **The gate is `castleSwitch` (`+0x1B0`), not `castleDegraded`.** The row
///   is *listed* when the county has a castle job at all — `FUN_0040FEC1` tests
///   `castleDegraded` — and then draws **nothing** while the switch is off. So
///   a row of the strip can be present, hit-testable and blank, which is the
///   original's behaviour and not a hole.
/// * **The 6-pixel nudge applies to the first two rows only**, so the castle
///   sits lower in a short list than in a long one.
/// * **The ringed castle is a different picture, not the plain one in a ring** —
///   `0x4E` is 32 × 34 against `0x40`'s 23 × 26 and is drawn six left and three
///   up, where every other pair on this plate is two and two. See
///   [`misc_cty::CASTLE_PLAIN`].
fn draw_castle_row(
    ctx: &Ctx,
    canvas: &mut Canvas,
    c: &l2_kingdom::county::County,
    y: i32,
    row: usize,
) {
    if !c.castle_switch {
        return;
    }
    let nudge = if row < 2 { 6 } else { 0 };
    let y = y + nudge;
    let idle = c.labour_useful[l2_kingdom::tables::JOB_CASTLE_BUILDING]
        < c.labour[l2_kingdom::tables::JOB_CASTLE_BUILDING];
    let (frame, x, dy) = if idle {
        (misc_cty::CASTLE_RINGED, 0x255, 0x129)
    } else {
        (misc_cty::CASTLE_PLAIN, 0x25B, 300)
    };
    draw_strip_icon(ctx, canvas, frame, x, y + dy, "CASTLE");

    let (stone, wood) = (c.castle_stone_owed, c.castle_wood_owed);
    if stone == 0 && wood == 0 {
        // `county +0x1A6` — [`l2_kingdom::industry::castle_seasons_left`], the
        // number the tooltip layer calls *"Seasons left to build castle"*
        // (`L2.eng` 220/22). Zero draws nothing at all, which is the original's
        // own `if (value != 0)`.
        let seasons = l2_kingdom::industry::castle_seasons_left(&ctx.game.kingdom.tables, c);
        if seasons != 0 {
            // `Ui_DrawNumber(value, ' ', &DAT_004D3D84, 0x23C, …+0x13A,
            // &g_font10, 0xFA)` — and `&DAT_004D3D84` is `" "`. This used to be
            // `"{seasons} "` with no lead, in `Fntl2_9.pl8`, so the digits sat
            // four pixels left of the original's in the wrong face.
            ten_number(ctx, canvas, seasons, ' ', " ", 0x23C, y + 0x13A, DELTA_POS);
            // `Ui_DrawUnitNoun(seasons, 0x42, 0x234, …+0x146, &g_fontSmall,
            // 0xFA)` (`0x0041AC3E`) — `L2.eng` group 8 index `0x42`/`0x43`,
            // *"Season"* and *"Seasons"*. Its rule is `value == 1`, not
            // `Ui_DrawCount`'s `|value| == 1`; a castle's seasons are never
            // negative, so the two agree here, and this is the one it calls.
            let index = if seasons == 1 { 0x42 } else { 0x43 };
            let noun = eng(ctx, 8, index, if index == 0x42 { "SEASON" } else { "SEASONS" });
            small_dropped(ctx, canvas, 0x234, y + 0x146, &noun, DELTA_POS);
        }
        return;
    }
    let (needs, ndy) = match (stone != 0, wood != 0) {
        (true, true) => (misc_cty::CASTLE_NEEDS_BOTH, 0x131),
        (false, _) => (misc_cty::CASTLE_NEEDS_WOOD, 0x137),
        (true, false) => (misc_cty::CASTLE_NEEDS_STONE, 0x137),
    };
    draw_strip_icon(ctx, canvas, needs, 0x23C, y + ndy, "NEEDS");
    // `Eng_DrawString(0x47, 0x12, …)` — group 71 index 18, *"Needed"*.
    small_dropped(ctx, canvas, 0x234, y + 0x146, &eng(ctx, 71, 18, "NEEDED"), DELTA_POS);
}


