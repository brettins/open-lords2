#![allow(unused_imports)]
use super::*;
use super::rows::*;
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
        let owner = super::super::super::super::message::lord_name(ctx, c.owner);
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

