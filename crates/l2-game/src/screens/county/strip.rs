#![allow(unused_imports)]
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

// -------------------------------------------------------- drawing the strip
//
// A free function, not a method, because **the campaign map draws it too**.
// `Screen_DrawCampaign` puts `CountyStrip_Draw` in the sidebar of the map
// itself; until now our map screen drew a box of our own numbers over the jobs
// plate below it and left this plate empty, which is the thing a player looks
// at every turn.

/// The one line of text the strip's font draws.
///
/// `CountyStrip_Draw` uses **`Fntl2_9.pl8`**, and it is the only caller of that
/// font in the whole game (`docs/screens-county.md` §2.1). We draw it where the
/// install has it and fall back to our own 5 × 7 font where it does not, so the
/// *layout* is the original's on every machine and the *letters* are only ours
/// on a machine with no game.
///
/// `colour` is a resolved palette index
/// constants, because one of the rules here is a colour: the achieved ration is
/// **red when it differs from the wanted one**. That rule is the original's; the
/// index we spell red with is ours, out of [`Ink`].
fn strip_text(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    match ctx.assets.shell.small.as_ref() {
        Some(f) => {
            // `CountyStrip_Draw` sets `DAT_005AEA40 = 1` for the whole numeric
            // block and clears it after, and that global switches
            // `Ui_DrawText`'s emboss **off**. The strip's numbers are flat.
            let style = crate::shell::font::Style { colour, shadow: None, caps: None };
            f.draw(canvas, x, y, s, &style);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// The same, centred in `width` from `x` — `Ui_DrawCentred`, which clamps the
/// offset at zero.
///
/// Public under a longer name because the End Turn caption is drawn in this
/// font too (`Screen_DrawEndTurn`), and it is the map screen that draws it.
pub fn strip_centred_at(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    width: i32,
    s: &str,
    colour: u8,
) {
    strip_centred(ctx, canvas, x, y, width, s, colour)
}

fn strip_centred(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, width: i32, s: &str, colour: u8) {
    let w = match ctx.assets.shell.small.as_ref() {
        Some(f) => f.width(s),
        None => text::width(s),
    };
    strip_text(ctx, canvas, x + ((width - w) / 2).max(0), y, s, colour);
}

/// One line in the **body** font (`Fntl2_14.pl8`), centred in `width` from `x`.
///
/// The strip's own font is the 9-pixel one, but the county's name and the
/// three "sovereign land of …" lines are drawn with `g_fontBody`, embossed —
/// `DAT_005AEA40` is only set for the numeric block between them.
/// **This helper really does right-align, and it is OURS.** It is named after
/// `Ui_DrawNumberRight`, which does not: that function centres (C119), and the
/// resemblance is the name only. Kept because the produce rows were laid out
/// against it and changing the anchoring is a separate, visible decision.
///
/// **Flat, not embossed.** Each produce row sets `DAT_005AEA40 = 1` around its
/// number and clears it after — the same switch the strip's own figures are
/// drawn under — and that global turns `Ui_DrawText`'s emboss off.
/// **`Ui_DrawNumber` (`0x00402F64`) — a number with its sign column.**
///
/// A player: *"Happiness # and population # in the sidebar are slightly left of
/// where they should be."* **Four pixels left, both of them.
/// too.** The cause is one character:
///
/// ```c
/// Ui_NumberToBuffer(value, 1, 0);            /* digits from index 1 */
/// if (lead != '\0') g_numberBuffer = lead;   /* index 0 */
/// ... append suffix ...
/// Ui_DrawText(&g_numberBuffer, x, y, font, colour);
/// ```
///
/// `Ui_NumberToBuffer`'s `start = 1` **leaves index 0 free for a sign**, and
/// every call site fills it — the strip's three pass `' '`. So the string drawn
/// at `x` is `" 435 "`, not `"435"`, and the digits begin one space-advance to
/// the right of `x`. We drew the bare digits at the same `x` and were short by
/// exactly [`SPACE_ADVANCE`](crate::shell::font::SPACE_ADVANCE) = 4.
///
/// **The lead is a column, not padding**, and this workspace already knew that
/// one level down: `SPACE_ADVANCE`'s own doc comment says `'@'` is *"an
/// invisible sign column that still occupies its place in a column of
/// numbers"*. `Ui_DrawDelta` uses that column for `'-'`, `'+'` and `'@'` so
/// that a rising and a falling forecast line up; a plain `Ui_DrawNumber` leaves
/// it blank and keeps the same left edge. Drawing the digits without it silently
/// opts out of the alignment the whole sidebar is built on.
///
/// **The discriminating prediction, because a second cause fitted the report.**
/// Right-anchoring where the original centres would displace a two-digit
/// happiness *further* than a three-digit population. This displaces both by
/// **the same four pixels**, because a lead is one character whatever the value
/// is — and these two are `Ui_DrawNumber`, which has no anchoring argument at
/// all, so the anchoring hypothesis could not apply to them. `Ui_DrawNumberRight`
/// is the one that centres, and it is [`body_number_centred`] below.
///
/// `lead` is `'\0'` for a caller that wants no column — which the original
/// treats as *terminate immediately*, since index 0 is the NUL the buffer was
/// cleared to, so no shipped call site passes it.
/// `Ui_DrawNumber(value, lead, suffix, x, y, &g_fontSmall, colour)` — the
/// numeric block's population, happiness and tax rate, which are flat
/// (`DAT_005AEA40 = 1`). The jobs plate's numbers are [`ten_number`].
#[allow(clippy::too_many_arguments)]
fn strip_number(
    ctx: &Ctx,
    canvas: &mut Canvas,
    value: i32,
    lead: char,
    suffix: &str,
    x: i32,
    y: i32,
    colour: u8,
) {
    strip_text(ctx, canvas, x, y, &format!("{lead}{value}{suffix}"), colour);
}

/// **`Ui_DrawText(s, x, y, &g_font10, colour)` with `g_dropShadow` set** — how
/// every number on the jobs plate is drawn. **[V]**
///
/// All nine `&g_font10` call sites in the image are inside the eight row
/// painters, and every one of those sets `g_dropShadow = 1` on entry with
/// `DAT_005AEA40` already clear, so the face and the shadow always travel
/// together; see [`font::DROP_SHADOW_COLOUR`](crate::shell::font::DROP_SHADOW_COLOUR).
///
/// **Only a number may be drawn here.** `Font_10.pl8` holds digits and
/// punctuation and a 2 × 2 stub where every letter goes ([`font::TEN`](crate::shell::font::TEN)),
/// so a word drawn in it paints nothing and still advances — the failure a
/// whole-canvas comparison passes straight over. The original never builds such
/// a string; the assertion is so that we cannot either.
fn ten_text(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    debug_assert!(
        !s.chars().any(|c| c.is_ascii_alphabetic()),
        "{s:?} drawn in Font_10.pl8, whose letters are 2x2 stubs: words on the strip are g_fontSmall"
    );
    match ctx.assets.shell.ten.as_ref() {
        Some(f) => {
            f.draw_dropped(canvas, x, y, s, colour);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// `Ui_DrawNumber(value, lead, suffix, x, y, &g_font10, colour)` (`0x00402F64`):
/// lead in slot 0, digits, suffix, one [`ten_text`].
#[allow(clippy::too_many_arguments)]
fn ten_number(
    ctx: &Ctx,
    canvas: &mut Canvas,
    value: i32,
    lead: char,
    suffix: &str,
    x: i32,
    y: i32,
    colour: u8,
) {
    ten_text(ctx, canvas, x, y, &format!("{lead}{value}{suffix}"), colour);
}

/// **`&g_fontSmall` with `g_dropShadow` set** — the castle cell's two captions,
/// `Ui_DrawUnitNoun`'s *"Season(s)"* and `L2.eng` 71/18 *"Needed"*, which
/// `CountyStrip_DrawCastleIcon` draws inside the same `g_dropShadow = 1` as its
/// number. The rest of the strip's `&g_fontSmall` text is flat ([`strip_text`]).
fn small_dropped(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) {
    match ctx.assets.shell.small.as_ref() {
        Some(f) => {
            f.draw_dropped(canvas, x, y, s, colour);
        }
        None => {
            text::draw(canvas, x, y, s, colour);
        }
    }
}

/// **`Ui_DrawNumberRight` (`0x004030C6`) — the same buffer, laid out in a
/// width.** Its tail is `FUN_004025D7`, which **centres**; see
/// [`body_centred_in`]. The lead column is `Ui_DrawNumber`'s, so it widens the
/// string and moves the digits half a space right of a bare centring.
fn body_number_centred(
    ctx: &Ctx,
    canvas: &mut Canvas,
    value: i32,
    lead: char,
    suffix: &str,
    x: i32,
    y: i32,
    w: i32,
    colour: u8,
) {
    body_centred_in(ctx, canvas, x, y, w, &format!("{lead}{value}{suffix}"), colour);
}

/// **`Ui_DrawDelta` (`0x00402E0C`) — the produce rows' signed forecast.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative number."*
/// This is the routine that would have. The original, in full:
///
/// ```c
/// if (value == 0 && mode == 0) return;                     /* nothing at all */
/// Ui_DrawText(prefix, x, y, font, value < 0 ? colourNeg : colourPos);
/// if      (mode == 2) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else if (value < 0) Ui_DrawNumber(-value, '-', suffix, x + g_penAdvance, …, colourNeg);
/// else if (value < 1) Ui_DrawNumber( value, '@', suffix, x + g_penAdvance, …, colourPos);
/// else                Ui_DrawNumber( value, '+', suffix, x + g_penAdvance, …, colourPos);
/// ```
///
/// Four things in it are worth having exactly, and three of them are the sort a
/// reimplementation drops without noticing:
///
/// * **The minus is a lead *character*, not a mark.** `Ui_DrawNumber` writes it
///   over `g_numberBuffer[0]`, the slot `Ui_NumberToBuffer(value, 1, 0)` leaves
///   free for a sign, so sign and digits go out in one `Ui_DrawText`. There is
///   no separate glyph to place or to lose.
/// * **A positive value carries an explicit `'+'`.** Only the *sign* tells the
///   player which way a forecast runs; the row has no other cue.
/// * **`mode == 0` and a value of zero draw nothing whatever.** All seven
///   produce-row calls pass mode 0. That is why an absent delta has read as a
///   quiet row — a county with nothing happening
///   looks the same either way.
/// * **The colour is the sign too**: `0xFA` positive, `0xF9` negative, at every
///   one of the seven call sites.
///
/// **Seven, not eight** — three farm rows and four industry rows; the castle
/// painter has no delta. This comment said eight, and so does
/// `docs/draws-map.md` §5.5.
///
/// The prefix and the suffix are a single space at all seven — read out of
/// `Lords2.exe` at `0x004D3D40 … 0x004D3D84`, eighteen pointers that all hold
/// `" "`. They are drawn as two separate strings, so
/// [`TRAILING`](crate::shell::TRAILING)'s four pixels fall between
/// the prefix and the number and **not** between the number and its suffix.
/// Concatenating the three into one string would lose those four pixels, which
/// is the whole reason this is not a `format!`.
///
/// **The face is `&g_font10`, the seventh argument at all seven**, drawn with
/// the drop shadow its painters set ([`ten_text`]). This used to be
/// `Fntl2_9.pl8` under a comment calling it ours, because `Font_10.pl8` was not
/// loaded.
fn strip_delta(ctx: &Ctx, canvas: &mut Canvas, value: i32, x: i32, y: i32) {
    // `if ((value != 0) || (mode != 0))` — every produce row passes mode 0.
    if value == 0 {
        return;
    }
    let colour = if value < 0 { DELTA_NEG } else { DELTA_POS };
    let lead = if value < 0 { '-' } else { '+' };
    // `Ui_DrawText(prefix, x, y, font, colour)`, then the number at
    // `x + g_penAdvance` — which is the prefix's width plus `Ui_DrawText`'s own
    // four trailing pixels, not the prefix's width alone.
    let prefix = " ";
    let advance = match ctx.assets.shell.ten.as_ref() {
        Some(f) => f.width(prefix),
        None => text::width(prefix),
    } + crate::shell::TRAILING;
    ten_text(ctx, canvas, x, y, prefix, colour);
    // `Ui_DrawNumber(|value|, lead, suffix, …, &g_font10, …)` — one string,
    // lead in slot 0.
    ten_number(ctx, canvas, value.abs(), lead, " ", x + advance, y, colour);
}

/// `Ui_DrawDelta`'s `colourPos`, the eighth argument at all seven produce-row
/// call sites.
const DELTA_POS: u8 = 0xFA;

/// `Ui_DrawDelta`'s `colourNeg`, the ninth. It is the same index
/// [`font::HIGHLIGHT`](crate::shell::font::HIGHLIGHT) carries and they are kept
/// apart on purpose: that one is *"this is the thing you are looking at"* and
/// this one is *"this number is negative"*, and a rename of either must not
/// drag the other.
const DELTA_NEG: u8 = 0xF9;

/// **`Ui_DrawNumberRight` (`0x004030C6`) centres.** It is not right-aligned and
/// its tail is `FUN_004025D7(buf, x, y, width, font, colour)`,
/// whose whole body is
///
/// ```c
/// Ui_DrawText(str, x + max(0, (width - Ui_TextWidth(str, font)) / 2), y, font, colour);
/// ```
///
/// The name is the original's shape — `docs/symbols.json`'s
/// comment said *"Ui_DrawNumber, right-aligned inside width"* and that comment
/// is corrected on this branch. Two draw audits found it independently in the
/// same week.
/// read.
///
/// It lands here: the produce rows' stock figure was anchored at x = 540 and
/// belongs centred between 480 and 540. This helper used to be `body_right` and
/// used to do that, which is the same defect the row's missing delta was
/// reported alongside — *"grain not shown as a negative"* and *"grain in the
/// wrong place"* would have looked like one complaint.
fn body_centred_in(ctx: &Ctx, canvas: &mut Canvas, x: i32, y: i32, w: i32, s: &str, colour: u8) {
    let style = crate::shell::font::Style { colour, shadow: None, caps: None };
    body_centred_styled(ctx, canvas, x, y, w, s, style);
}

/// Centred in `width` from `x`, with the emboss pair chosen by the caller — because
/// `CountyStrip_Draw` uses **two different ones** in the same plate. See
/// [`crate::shell::font::SHADOW_GREY`].
fn body_centred_styled(
    ctx: &Ctx,
    canvas: &mut Canvas,
    x: i32,
    y: i32,
    w: i32,
    s: &str,
    style: crate::shell::font::Style,
) {
    match ctx.assets.shell.body.as_ref() {
        Some(f) => {
            f.draw_centred(canvas, x, y, w, s, &style);
        }
        None => {
            text::draw_centred(canvas, x + w / 2, y, s, style.colour);
        }
    }
}

/// The county's name: `L2.eng` group 100, index `scenarioIndex * 20 + countyId`
/// — and `g_scenarioIndex` *is* the map slot ([`crate::game::Game::map_slot`]).
///
/// Falls back to `COUNTY n` for an install with no `L2.eng`, which is also what
/// the tests run against.
pub fn county_name(ctx: &Ctx, id: u8) -> String {
    let index = ctx.game.map_slot * 20 + id as usize;
    let name = ctx.assets.shell.text(100, index);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// **One `L2.eng` string with a fallback — the panels' vocabulary,
/// strip's two.**
///
/// All four county panels hard-coded their words — `"RATION"`, `"WANTED:"`,
/// `"PEOPLE PAY"` — where the original draws `Eng_DrawString(group, index)`.
///
/// **The ration panel is wired through here. The other three are not, and this
/// is the list**, so that "recorded" does not become "left":
///
/// | panel | `g_screenId` | group | painter | state |
/// |---|---|---|---|---|
/// | ration | `0x19` | 87 | `Panel_Ration` (`0x00411B72`) | wired |
/// | tax | `0x15` | 86 | `Panel_Tax` (`0x0041152F`) | **hard-coded** |
/// | population | `0x14` | 73 | | **hard-coded** |
/// | happiness | `0x16` | 85 | | **hard-coded** |
///
/// The ration panel's own numbers say what the other three are likely to cost:
/// wiring it went from **12 of `Panel_Ration`'s 26 content draws to 18**, and
/// six of the six added were *labels* — the frames that turn three unlabelled
/// numbers into a Fed row. `CLAUDE.md` rule 6.
///
/// The honest account of how that happened, because it is a habit and not an
/// oversight: **we read these panels' numbers out of the binary and wrote their
/// words ourselves**, treating the numbers as the mechanism and the text as a
/// skin over it. `Panel_Ration` is the *only* consumer of group 87 in the whole
/// binary and draws seven of its twelve strings, so the group **is** the
/// panel's specification. A screen's strings are part of what it does.
///
/// No case handling here: [`l2_view::text::glyph`] upper-cases, so a lower-case
/// string out of `L2.eng` draws the same as our shouted fallback.
pub(super) fn eng(ctx: &Ctx, group: usize, index: usize, fallback: &str) -> String {
    let s = ctx.assets.shell.text(group, index);
    if s.is_empty() {
        fallback.to_string()
    } else {
        s.to_string()
    }
}

/// The ration level's name — `L2.eng` group 21, which is what `CountyStrip_Draw`
/// indexes with county `+0x15D`.
pub(super) fn ration_label(ctx: &Ctx, level: i32) -> String {
    let level = level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
    eng(ctx, GROUP_RATION_LEVELS, level as usize, ration_name(level))
}

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
    // `crates/l2-view/tests/install.rs` asserts that against the player's own
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
/// `crates/l2-game/tests/screens_county.rs`'s
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

/// The fallback words for the three flat industry rows, in commodity order —
/// ours, and reached only by an install with no `Misc_cty.pl8`.
const INDUSTRY_LABEL: [&str; 4] = ["WOOD", "IRON", "WEAPONS", "STONE"];

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

