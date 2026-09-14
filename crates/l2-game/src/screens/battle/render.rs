#![allow(unused_imports)]
use super::*;
use super::prompt::*;
use super::result::*;
use super::tests_part::*;
use l2_kingdom::battle::Outcome;
use l2_kingdom::unit::ALL_TROOP_TYPES;
use l2_view::chrome::system;
use l2_view::{text, Canvas};
use crate::engagement::{Answer, Roster};
use crate::input::{Event, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::turn::{self, Question, TurnStep};

/// The county's name out of group 100, or ours.
fn county_name(ctx: &Ctx, id: u8) -> String {
    let name = ctx.assets.shell.text(GROUP_COUNTY, ctx.game.map_slot * 20 + id as usize);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// What to write over a side. The original draws a lord's name from
/// `g_playerNames`, and `L2.eng` group 99 — *"The people."* — for an ownerless
/// army. Both, now: the same hole the court screen had, with the same fix.
fn side_name(ctx: &Ctx, owner: u8) -> String {
    if owner == 0 {
        let s = ctx.assets.shell.text(GROUP_OWNERLESS, 0);
        if !s.is_empty() {
            return s.trim_matches('"').to_uppercase();
        }
        return "THE PEOPLE".into();
    }
    super::super::message::lord_name(ctx, owner)
}

/// The realm's shield frame, with `Battle_ChooseSettlement`'s own substitution
/// of 6 for a realm that has none.
fn shield_frame(ctx: &Ctx, owner: u8) -> usize {
    let index = ctx
        .game
        .kingdom
        .realms
        .get(owner as usize)
        .map_or(0, |r| r.shield_index as usize);
    SHIELD_FRAME0 + if index == 0 { OWNERLESS_SHIELD } else { index }
}

/// The common half of both screens: the window, the county, the medallion, the
/// two shields and the two names.
pub(crate) fn draw_frame(
    ctx: &Ctx,
    canvas: &mut Canvas,
    county: u8,
    a_owner: u8,
    b_owner: u8,
    is_siege: bool,
) {
    let p = pen(ctx);
    p.window(canvas, BOX_X, BOX_Y, BOX_COLS, BOX_ROWS, BOX_SET);
    p.body_centred(canvas, COUNTY.0, COUNTY.1, COUNTY.2, &county_name(ctx, county), font::TEXT);

    // `Ui_DrawBevelRect(0x34, 0x44, 0x52, 0x52)` — the raised edge only. The
    // fill this used to draw first is not in the original and, under the game's
    // own palette, is a black square where the medallion goes.
    super::super::siege::bevel_rect(canvas, MEDALLION.x, MEDALLION.y, MEDALLION.w, MEDALLION.h);

    // `icon_tmp.pl8`: the medallion and the two shields, three
    // `Sprite_WGenSprite` calls with the sheet read whole immediately before.
    let sheet = p.assets.sheet(MEDALLION_SHEET);
    let blit = |canvas: &mut Canvas, frame: usize, x: i32, y: i32| {
        if let Some(f) = sheet.and_then(|s| s.frame(frame)) {
            canvas.blit(&f, x, y);
        }
    };
    blit(
        canvas,
        if is_siege { MEDALLION_SIEGE } else { MEDALLION_BATTLE },
        MEDALLION_AT.0,
        MEDALLION_AT.1,
    );
    blit(canvas, shield_frame(ctx, a_owner), SHIELD_A.0, SHIELD_A.1);
    blit(canvas, shield_frame(ctx, b_owner), SHIELD_B.0, SHIELD_B.1);

    p.body_centred(canvas, LORD_A.0, LORD_A.1, LORD_A.2, &side_name(ctx, a_owner), font::TEXT);
    p.body_centred(canvas, LORD_B.0, LORD_B.1, LORD_B.2, &side_name(ctx, b_owner), font::TEXT);
}

/// `FUN_004224E7` — the seven-row roster both screens share.
///
/// `before` is `None` on the prompt, where nothing has happened yet: the
/// original's mode 0 *records* the counts into two arrays and prints only the
/// one column, and mode 1 reads them back as the parenthesised figure beside
/// what is left. Here the recording is [`BattleReport::attacker_roster`] and
///
///
/// `totals` is the pair the original reads **out of the unit record** rather
/// than off the rows — see the loop at the end of this function.
pub(crate) fn draw_roster(
    ctx: &Ctx,
    canvas: &mut Canvas,
    a: (&Roster, Option<&Roster>),
    b: (&Roster, Option<&Roster>),
    totals: (i32, i32),
) {
    let p = pen(ctx);
    let ink = &ctx.assets.ink;
    for (row, troop) in ALL_TROOP_TYPES.iter().enumerate() {
        let y = ROSTER_Y + row as i32 * ROW_PITCH + 4;
        // `Pl8_DrawFrame(g_miscCtySheet, 0x2F + row, …)` twice a row, at the
// row's own y. Nothing drew these until the
        // draw-call audit read `FUN_004224E7`.
        for x in [COL_A_ICON, COL_B_ICON] {
            p.misc_frame(canvas, TROOP_ICON_FRAME0 + row, x, y - 4);
        }
        // Always the plural: `Ui_DrawUnitNoun`'s count argument is the literal
        // 2, so the singular at `0x34 + t*2` can never be reached from here.
        // **As the file spells it.** This was upper-cased, which in
        // `Fntl2_14.pl8` is a row of blackletter capitals — the defect the
        // totals below had and lost. The fallback keeps our own name, in the
        // debug font's own case.
        let noun = ctx.assets.shell.text(8, NOUN_BASE + row * 2 + 1).to_string();
        let label = if noun.is_empty() { troop.name().to_string() } else { noun };
        p.body(canvas, COL_NOUN, y, &label, font::TEXT);

        // `Ui_DrawNumber(n, ' ', &DAT_004D4410 | …14 | …1C | …20, x, y, font,
        // 0x3F)` — a **space lead and a one-space suffix** at every column.
        // These were `n.to_string()` with no lead, so both columns sat four
        // pixels left of `param_3 + 0x28` and `param_3 + 0x118`. **[V]**
        let face = shell::Face::Body;
        p.number_in(face, canvas, COL_A_AFTER, y, a.0[row], ' ', " ", font::TEXT);
        p.number_in(face, canvas, COL_B_AFTER, y, b.0[row], ' ', " ", font::TEXT);
        // Mode 1's recorded counts: `Ui_DrawNumber(was, '(', &DAT_004D4418 |
        // …24, …, 0x3F)`, whose suffix is `")"`. The string is the one this
        // drew before; **the colour is not** — the original passes `0x3F`, the
        // same as every other number in the roster, where this passed
        // `font::DISABLED`. **[V]**
        if let Some(was) = a.1 {
            p.number_in(face, canvas, COL_A_BEFORE, y, was[row], '(', ")", font::TEXT);
        }
        if let Some(was) = b.1 {
            p.number_in(face, canvas, COL_B_BEFORE, y, was[row], '(', ")", font::TEXT);
        }
    }
    // **The total is `+0x168`, not the sum of the seven rows**, and the two are
    // not the same number. `Mercenary_Hire` (`0x004AC7F3`) adds the band's men
    // to `menTotal` and never touches `+0x16C`, so an army raised with nothing
    // but a hired band has seven zero counts and a real total. Summing the rows
    // printed *"0 Total men"* under a prompt asking whether to fight with them
    // — reported as *"when I attacked and it asked me to decide it said I had 0
    // men, I think it's because it was just mercenaries"*. The band is folded
    // into its own row by [`crate::engagement::roster_of`], which is what makes
    // the rows add up to this figure again.
    for (x, n) in [(TOTAL_A_X, totals.0), (TOTAL_B_X, totals.1)] {
        // `FUN_004224E7`: `Ui_DrawCount(unit.menTotal, 0x48, x + 0x14, …, font)`
        // — group 8's *"Total man"* / *"Total men"* **as the file spells it**,
        // singular at ±1. We upper-cased it, which in `Fntl2_14.pl8` is a line of
        // blackletter capitals — the illegibility a player reported of the title
        // screen, here on the battle result — and built it as `"{n} {noun}"`,
        // with no `'@'` lead, so the whole line sat four pixels left.
        let noun = ctx.assets.shell.text(8, crate::shell::count_noun(n, NOUN_TOTAL)).to_string();
        let noun = if noun.is_empty() { "Total men".to_string() } else { noun };
        p.count_with_noun(crate::shell::Face::Body, canvas, x, TOTAL_Y, n, &noun, font::TEXT);
    }
    let _ = ink;
}

/// The seven banners in our own words, for an install with no `L2.eng`. The
/// original's are far better and are used whenever they are there.
pub(crate) fn ours_banner(outcome: Outcome) -> String {
    match outcome {
        Outcome::Won => "THE BATTLE IS WON.",
        Outcome::Lost => "THE BATTLE IS LOST.",
        Outcome::SiegeWon => "THE SIEGE IS WON.",
        Outcome::SiegeLost => "THE SIEGE IS LOST.",
        Outcome::SiegeLifted => "THE SIEGE IS LIFTED.",
        Outcome::CastleLost => "THE CASTLE IS LOST.",
        Outcome::Bystander => "THE CONFLICT IS OVER.",
    }
    .to_string()
}

