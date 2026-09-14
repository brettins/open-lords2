#![allow(unused_imports)]
use super::*;
use super::main::*;
use super::icon::*;
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



