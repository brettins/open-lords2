//! **The two battle screens** — `Screen_BattlePrompt` (`0x00422D22`, screen
//! `0x12`) and `Screen_BattleResult` (`0x00422FF1`, screen `0x13`).
//!
//! *"A Battle is to be fought. Will you take the field?"* is the question the
//! engine has never been able to ask. [`crate::turn::end_turn`] answered
//! [`Answer::Decline`](crate::engagement::Answer::Decline) — the original's own
//! autocalc branch — because there was no screen to ask on, and every battle in
//! a played turn was therefore settled by arithmetic. These are the screens.
//!
//! # They are one painter with two endings
//!
//! Both draw the same window, the same medallion, the same two shields, the
//! same two lord names and the same seven-row roster; they differ in one
//! heading, one paragraph, and what the player can click. So they are one
//! module and two [`Screen`] impls over shared drawing, which is what the
//! original is:
//!
//! ```text
//!                                 0x12                      0x13
//! Ui_DrawBoxBorder(1, …)          0x20,0x30,0x1A,0x19       same
//! county name, group 100          (132, 66) centred w306    same, drawn later
//! heading                         80.0  / 80.7 siege        81.0 / 81.8 siege
//! paragraph                       80.1 / 80.2 / 80.3        — none —
//! medallion + two shields         icon_tmp.pl8              same
//! two lord names                  (40,160) and (240,160)    same
//! roster FUN_004224E7             mode 0                    mode 1
//! buttons                         thumb up / thumb down     the OK corner
//! ```
//!
//! # The painter, address by address
//!
//! Transcribed from `Screen_BattlePrompt` (`0x00422D22`); `Screen_BattleResult`
//! (`0x00422FF1`) is the same list with `Ui_OkButton(400, 400, 0)` in place of
//! the paragraph, group 81 in place of group 80, roster mode 1 in place of mode
//! 0, and the county name and heading moved to the end. Coordinates resolved to
//! decimal in the trailing comment.
//!
//! ```text
//! Screen_BattlePrompt():                                        0x00422D22
//!   Screen_DrawCampaign(1)                       the map underneath — excluded
//!   FUN_004093E0(0x20, 0x30, 0x1A, 0x19)     window, set 1, (32, 48) 416 x 400
//!   Ui_DrawCentred(100, slot*20 + county, 0x84, 0x42, 0x132, body)
//!                                                centred in x 132..438, y 66
//!   Eng_DrawString(80, siege ? 7 : 0, 0x8E, 0x56, heading)         (142, 86)
//!   FUN_0040328E(80, 1|2|3, 0x8E, 0x7A, 0xB4|0x118, 400, 0, 0, body)
//!                                       wrapped at 180 for 1, 280 for 2 and 3
//!   File_ReadChunk("icon_tmp.pl8", spriteBuffer, 160000, 0)
//!   Ui_DrawBevelRect(0x34, 0x44, 0x52, 0x52)   (52, 68) 82 x 82, FOUR LINES
//!   Sprite_WGenSprite(siege ? 0x38 : 0x37, 0x35, 0x45)      (53, 69) crossed
//!                                              swords, or a castle for a siege
//!   Sprite_WGenSprite(shieldA + 0x30, 0x7A, 0xB4)                (122, 180)
//!   Sprite_WGenSprite(shieldB + 0x30, 0x142, 0xB4)               (322, 180)
//!   FUN_004025D7(playerNames[ownerA], 0x28, 0xA0, 200, body)   centred, y 160
//!     — or Ui_DrawCentred(99, 0, …) when the owner is 6, "The people."
//!   FUN_004025D7(playerNames[ownerB], 0xF0, 0xA0, 200, body)     (240, 160)
//!   FUN_004224E7(armyA, armyB, 0x3E, 0xE0, body, 0)               the roster
//!
//! FUN_004224E7(a, b, x=0x3E, y=0xE0, font, mode):                0x004224E7
//!   mode 0, seven rows at y + row*0x18:
//!     Ui_DrawNumber(a.troops[row], ' ', " ", x+0x28,  y+4)             x 102
//!     Pl8_DrawFrame(misc_cty, 0x2F + row, x+0x55,  y)                  x 147
//!     Ui_DrawUnitNoun(2, 0x34 + row*2, x+0x7D, y+4)                    x 187
//!     Pl8_DrawFrame(misc_cty, 0x2F + row, x+0xF5, y)                   x 307
//!     Ui_DrawNumber(b.troops[row], ' ', " ", x+0x118, y+4)             x 342
//!     and the row's two counts are STASHED in DAT_00568420/DAT_0056843C
//!   then Ui_DrawCount(a.menTotal, 0x48, x+0x14,  y + 7*0x18 + 0xC)  (82, 404)
//!        Ui_DrawCount(b.menTotal, 0x48, x+0xDC, …)                (282, 404)
//!   mode 1, the same seven rows plus the two stashed figures:
//!     Ui_DrawNumber(stashedA[row], '(', ")", x-6,     y+4)              x 56
//!     Ui_DrawNumber(stashedB[row], '(', ")", x+0x146, y+4)             x 388
//!   mode 2 is UnitPanel_Draw's two-column army panel and is unreachable here
//! ```
//!
//! **The mercenary band is folded into its troop type before the row is drawn**
//! — `if (unit.mercTroop == row) count += unit.mercMen` — in both modes, and it
//! is the stashed value too, so a mercenary company shows up inside the
//! swordsmen rather than beside them.
//!
//! Three transcription slips came out of writing this listing: the noun column
//! is `0x3E + 0x7D` = **187** and this module had 185, the parenthesised
//! *before* column on the right is `0x3E + 0x146` = **388** and this module had
//! 390, and the **two troop icons a row** — `Misc_cty.pl8` frames `0x2F + row`
//! at x 147 and 307 — were not drawn at all.
//!
//! # Border set **1**
//!
//! `FUN_004093E0` is not `Ui_DrawBox`. It is the *other* frame kit, and passing
//! set 0 draws a visibly wrong window — [`Pen::window`]'s last argument is 1 for
//! both of these and 0 for almost everything else in the game.
//!
//! # Two things the original draws that are not here, and one it does not
//!
//! * **The lords' names.** `g_playerNames` is not modelled; a realm has no name
//!   in this tree. Each side is drawn as its realm number, in our own font,
//!   which `docs/decisions.md` C21 is the rule for: where we cannot establish
//!   what the original drew, it is visibly ours. The one name we *can* draw is
//!   the original's own — `L2.eng` group 99, the single string
//!   `"The people."`, which it draws for an ownerless army. A county levy is
//!   exactly that. **The original's test is `owner == 6`**, because 6 is the
//!   peasant faction's owner byte; ours is `owner == 0`, because
//!   `l2_kingdom::realm` numbers realms 1..=5 and reserves 0 for nobody.
//!   The **medallion is not a face** and this module used to say it was: it is
//!   `icon_tmp.pl8` frame `0x37`, one picture, with `0x38` for a siege, and it
//!   is drawn here now — as are the two shields at frame `0x30 + shieldIndex`,
//!   where `Battle_ChooseSettlement` substitutes **6** for a zero shield.
//! * **`Defence_Disband` at the end of the painter.** The original's `0x13`
//!   mutates the world from its draw function. Ours cannot, by design —
//!   [`Screen::draw`] takes `&Ctx` — and it does not need to:
//!   [`crate::engagement`] already runs `battle::disband_defence` on the resolve
//!   path, which is where the rule belongs.
//! * **A victory sentence.** There isn't one. `L2.eng` group 81 carries *"are
//!   victorious." / "have been defeated." / "have been crushed." / "have been
//!   annihilated."* and group 80 carries *"The army of" / "The people of" / "The
//!   bandits from"* — and **none of the seven is drawn anywhere in the
//!   binary.** Every `Eng_DrawString` on group 80 is index 0, 1, 2, 3 or 7 and
//!   every one on group 81 is index 0 or 8. They are dead strings, and a screen
//!   that composed *"The army of X have been crushed"* out of them would be
//!   inventing a sentence the game never printed. Recorded here because the
//!   strings look exactly like an instruction to do it.
//!
//! # Where the outcome banner lives
//!
//! Not on `0x13`. `L2.eng` group 82's seven heading/body pairs belong to
//! `Screen_BattleOutcome`, screen `0x2B`, which is a separate screen this module
//! does not build. It was read for the draw-call audit and is recorded here
//! rather than nowhere:
//!
//! ```text
//! Screen_BattleOutcome():                                       0x00423241
//!   if (g_battleChoiceOwner == 0)              the local player is a bystander
//!     FUN_004093E0(0x10, 0x90, 0x1C, 10)   window, set 1, (16, 144) 448 x 160
//!     Ui_OkButton(0x1A0, 0x100, 0)                                (416, 256)
//!     Eng_DrawString(82, 0xC, 0x30, 0xA8, heading)  "The conflict is over."
//!     FUN_0040328E(82, 0xD, 0x30, 0xE0, 0x180, 100, 0, 0, body)  wrapped at 384
//!   else if (g_optAnimations == 0)                  the same window, the pair
//!     FUN_004093E0(0x10, 0x90, 0x1C, 10)                   chosen by outcome
//!     Ui_OkButton(0x1A0, 0x100, 0)
//!     Eng_DrawString(82, outcome * 2,   0x30, 0xA8, heading)
//!     FUN_0040328E(82, outcome * 2 + 1, 0x30, 0xE0, 0x180, 100, 0, 0, body)
//!   else                                   animations on: a taller window and
//!     FUN_004093E0(0x10, 0x30, 0x1C, 0x16) a recess for the video, (16, 48)
//!     Ui_DrawInsetRect(0x27, 0x48, 0x192, 0xC2)        (39, 72) 402 x 194
//!     Ui_OkButton(0x1A0, 0x160, 0)                              (416, 352)
//!     Eng_DrawString(82, outcome * 2,   0x30, 0x138, heading)        y 312
//!     FUN_0040328E(82, outcome * 2 + 1, 0x30, 0x158, 0x180, 100, 0x20, 0x1A0, body)
//! ```
//!
//! **The three `Ui_OkButton` calls are one per branch**, not three buttons, and
//! only two coordinates exist between them: (416, 256) for the short window and
//! (416, 352) for the tall one. Ten draw call sites and **at most four run in a
//! frame**.
//!
//! `Battle_SelectOutcomeBanner` (`0x00478419`) writes `g_battleOutcome` and only
//! ever writes **0…5**: `siege ? (localWon ? (Awon ? 2 : 4) : (Awon ? 5 : 3))
//! : (localWon ? 0 : 1)`. The seventh pair, 12/13, is not a value of
//! `g_battleOutcome` at all — it is the `g_battleChoiceOwner == 0` branch, *"a
//! battle between two other realms"*. **That is what makes the count seven, and
//! all seven are reachable.** `docs/battle.md` does not name them; the seven
//! are `L2.eng` 82's pairs at 0/1 won, 2/3 lost, 4/5 siege won, 6/7 siege lost,
//! 8/9 siege lifted, 10/11 castle lost, 12/13 the conflict is over.
//!
//! [`crate::engagement::BattleReport::outcome`] already answers
//! which pair, so that screen is a painter and no more reverse engineering; this
//! one prints the pair's heading under its own so that the result of a battle is
//! not silently lost while `0x2B` does not exist.

use l2_kingdom::battle::Outcome;
use l2_kingdom::unit::ALL_TROOP_TYPES;
use l2_view::chrome::system;
use l2_view::{text, Canvas};

use crate::engagement::{Answer, Roster};
use crate::input::{Event, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::turn::{self, Question, TurnStep};

/// `L2.eng` group 80 — the prompt's own text.
pub const GROUP_PROMPT: usize = 80;
/// `L2.eng` group 81 — the result's.
pub const GROUP_RESULT: usize = 81;
/// `L2.eng` group 82 — the seven outcome heading/body pairs.
pub const GROUP_BANNER: usize = 82;
/// `L2.eng` group 99 — one string, `"The people."`, quotes included, which the
/// original draws in place of a lord's name for an ownerless army.
pub const GROUP_OWNERLESS: usize = 99;
/// `L2.eng` group 8's troop nouns, two apiece: singular then plural. The battle
/// roster always takes the plural, because `Ui_DrawUnitNoun`'s count argument
/// is the literal 2.
pub const NOUN_BASE: usize = 0x34;
/// `L2.eng` group 100 — the county names, `map_slot * 20 + county`.
pub const GROUP_COUNTY: usize = 100;
/// Group 8 index `0x48`/`0x49` — *"Total man"* / *"Total men"*.
pub const NOUN_TOTAL: usize = 0x48;

/// `FUN_004093E0(0x20, 0x30, 0x1A, 0x19)` — **border set 1**, 26 × 25 cells of
/// sixteen pixels, so 416 × 400 at (32, 48).
pub const BOX_X: i32 = 0x20;
pub const BOX_Y: i32 = 0x30;
pub const BOX_COLS: i32 = 0x1A;
pub const BOX_ROWS: i32 = 0x19;
/// Which frame kit. See the module header: **not** zero.
pub const BOX_SET: usize = 1;

pub fn window() -> Rect {
    Rect::new(BOX_X, BOX_Y, BOX_COLS * 16, BOX_ROWS * 16)
}

/// `Ui_DrawCentred(100, …, 0x84, 0x42, 0x132, …)` — the county's name.
const COUNTY: (i32, i32, i32) = (0x84, 0x42, 0x132);
/// `Eng_DrawString(group, …, 0x8E, 0x56, heading, …)`.
const HEADING: (i32, i32) = (0x8E, 0x56);
/// `FUN_0040328E(0x50, …, 0x8E, 0x7A, …)` — the wrapped paragraph.
const PARAGRAPH: (i32, i32) = (0x8E, 0x7A);
/// `Ui_DrawBevelRect(0x34, 0x44, 0x52, 0x52)` — the medallion's recess. Four
/// lines and **no fill**; see [`crate::screens::siege::bevel_rect`].
const MEDALLION: Rect = Rect::new(0x34, 0x44, 0x52, 0x52);
/// `File_ReadChunk("icon_tmp.pl8", …)` — the sheet both screens read whole and
/// then blit three frames of.
pub const MEDALLION_SHEET: &str = "Icon_tmp.pl8";
/// `Sprite_WGenSprite(0x37 | 0x38, 0x35, 0x45)` — one frame for a field battle
/// and another for a siege, inside the recess at (52, 68).
///
/// **Unverified against the artwork**: nothing in this tree has looked at what
/// frames 55 and 56 of `icon_tmp.pl8` actually are, only that the painter picks
/// between them on `g_battleIsSiege`.
pub const MEDALLION_BATTLE: usize = 0x37;
pub const MEDALLION_SIEGE: usize = 0x38;
pub const MEDALLION_AT: (i32, i32) = (0x35, 0x45);
/// `Sprite_WGenSprite(shield + 0x30, …)` — the two realms' shields, above their
/// names. `Battle_ChooseSettlement` (`0x004A6A30`) writes
/// `if (shield == 0) shield = 6`, so an ownerless army takes frame 54.
pub const SHIELD_FRAME0: usize = 0x30;
pub const OWNERLESS_SHIELD: usize = 6;
pub const SHIELD_A: (i32, i32) = (0x7A, 0xB4);
pub const SHIELD_B: (i32, i32) = (0x142, 0xB4);
/// The two lord names, each centred in 200 pixels.
const LORD_A: (i32, i32, i32) = (40, 160, 200);
const LORD_B: (i32, i32, i32) = (240, 160, 200);

/// `FUN_004224E7(a, b, 0x3E, 0xE0, …)` — the roster's origin, and the pitch of
/// its seven rows.
const ROSTER_Y: i32 = 0xE0;
const ROW_PITCH: i32 = 0x18;
/// Absolute x of each column: `FUN_004224E7`'s `param_3` is `0x3E` and every
/// column is an offset from it. Two of these were two pixels out until the
/// listing above was written from the decompilation.
const COL_A_BEFORE: i32 = 0x3E - 6; // 56
const COL_A_AFTER: i32 = 0x3E + 0x28; // 102
const COL_A_ICON: i32 = 0x3E + 0x55; // 147
const COL_NOUN: i32 = 0x3E + 0x7D; // 187, and this module had 185
const COL_B_ICON: i32 = 0x3E + 0xF5; // 307
const COL_B_AFTER: i32 = 0x3E + 0x118; // 342
const COL_B_BEFORE: i32 = 0x3E + 0x146; // 388, and this module had 390
/// `Pl8_DrawFrame(g_miscCtySheet, 0x2F + row, …)` — the seven campaign troop
/// icons, drawn **twice a row**, once for each side, at the row's own y with no
/// `+ 4`.
const TROOP_ICON_FRAME0: usize = 0x2F;
/// `Ui_DrawCount(total, 0x48, x, 404)` for each side, after the seven rows.
const TOTAL_Y: i32 = 404;
const TOTAL_A_X: i32 = 82;
const TOTAL_B_X: i32 = 282;

/// The two widgets of `DAT_004DDBB0`, **box-relative** exactly as the table
/// holds them: `(x, y, System.pl8 frame, side)`.
///
/// Frames 29 and 31 are a mailed hand giving a thumb **up** and a thumb
/// **down** — not a tick and a cross; `docs/screens-county.md` §4.2 decoded
/// them. Every yes/no in the game draws this same pair, which is why
/// `screens/saveload.rs` has the identical two constants.
pub const TAKE_THE_FIELD: (i32, i32, usize, i32) = (BOX_X + 300, BOX_Y + 68, 29, 32);
pub const DECLINE: (i32, i32, usize, i32) = (BOX_X + 340, BOX_Y + 68, 31, 32);

/// `Ui_OkButton(400, 400, 0)` — the result screen's corner.
pub const OK: (i32, i32) = (400, 400);

pub fn widget_rect(w: (i32, i32, usize, i32)) -> Rect {
    Rect::new(w.0, w.1, w.3, w.3)
}

pub fn ok_rect() -> Rect {
    Rect::new(OK.0, OK.1, system::OK_DIM, system::OK_DIM)
}

fn pen<'a>(ctx: &'a Ctx) -> Pen<'a> {
    Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    }
}

/// The county's name out of group 100, or ours when there is no `L2.eng`.
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
/// army. We have the second and not the first.
fn side_name(ctx: &Ctx, owner: u8) -> String {
    if owner == 0 {
        let s = ctx.assets.shell.text(GROUP_OWNERLESS, 0);
        if !s.is_empty() {
            return s.trim_matches('"').to_uppercase();
        }
        return "THE PEOPLE".into();
    }
    format!("LORD {owner}")
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
fn draw_frame(
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
    super::siege::bevel_rect(canvas, MEDALLION.x, MEDALLION.y, MEDALLION.w, MEDALLION.h);

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
/// there is no global.
fn draw_roster(
    ctx: &Ctx,
    canvas: &mut Canvas,
    a: (&Roster, Option<&Roster>),
    b: (&Roster, Option<&Roster>),
) {
    let p = pen(ctx);
    let ink = &ctx.assets.ink;
    for (row, troop) in ALL_TROOP_TYPES.iter().enumerate() {
        let y = ROSTER_Y + row as i32 * ROW_PITCH + 4;
        // `Pl8_DrawFrame(g_miscCtySheet, 0x2F + row, …)` twice a row, at the
        // row's own y rather than the text's. Nothing drew these until the
        // draw-call audit read `FUN_004224E7`.
        for x in [COL_A_ICON, COL_B_ICON] {
            p.misc_frame(canvas, TROOP_ICON_FRAME0 + row, x, y - 4);
        }
        // Always the plural: `Ui_DrawUnitNoun`'s count argument is the literal
        // 2, so the singular at `0x34 + t*2` can never be reached from here.
        let noun = ctx.assets.shell.text(8, NOUN_BASE + row * 2 + 1).to_string();
        let label = if noun.is_empty() { troop.name().to_uppercase() } else { noun.to_uppercase() };
        p.body(canvas, COL_NOUN, y, &label, font::TEXT);

        p.body(canvas, COL_A_AFTER, y, &a.0[row].to_string(), font::TEXT);
        p.body(canvas, COL_B_AFTER, y, &b.0[row].to_string(), font::TEXT);
        if let Some(was) = a.1 {
            p.body(canvas, COL_A_BEFORE, y, &format!("({})", was[row]), font::DISABLED);
        }
        if let Some(was) = b.1 {
            p.body(canvas, COL_B_BEFORE, y, &format!("({})", was[row]), font::DISABLED);
        }
    }
    let total = |r: &Roster| r.iter().sum::<i32>();
    let plural = |n: i32| ctx.assets.shell.text(8, NOUN_TOTAL + usize::from(n != 1)).to_string();
    for (x, r) in [(TOTAL_A_X, a.0), (TOTAL_B_X, b.0)] {
        let n = total(r);
        let noun = plural(n);
        let line =
            if noun.is_empty() { format!("{n} TOTAL MEN") } else { format!("{n} {}", noun.to_uppercase()) };
        p.body(canvas, x, TOTAL_Y, &line, font::TEXT);
    }
    let _ = ink;
}

// ---------------------------------------------------------------- the prompt

/// Screen `0x12`. It has no state of its own: the question lives on the
/// suspended turn, which is where the answer has to go back to.
pub struct BattlePromptScreen;

impl BattlePromptScreen {
    pub fn new() -> BattlePromptScreen {
        BattlePromptScreen
    }

    /// The question this screen is about, or `None` if the turn is no longer
    /// suspended — which a screen must survive, because the machine may still
    /// draw it for one frame after the turn moved on.
    fn question(ctx: &Ctx) -> Option<Question> {
        turn::pending_question(ctx.game)
    }

    /// **Answer, and carry the turn on.**
    ///
    /// The battle is fought or calculated inside this call, so what comes back
    /// is normally [`TurnStep::Report`] and the transition is a `Replace` onto
    /// screen `0x13` — which is the original's sequence exactly: `0x12`, the
    /// battle, `0x13`.
    fn answer(ctx: &mut Ctx, answer: Answer) -> Transition {
        match turn::answer_battle(ctx.game, answer) {
            TurnStep::Report(_) => Transition::Replace(ScreenId::BattleResult),
            // Another battle in the same turn, straight after this one.
            TurnStep::Ask(_) => Transition::Stay,
            // The turn is carrying on and has nothing more to ask: the map
            // underneath takes it from here, a tick a frame. See
            // [`turn::TurnStep::Running`].
            TurnStep::Running | TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }
}

impl Default for BattlePromptScreen {
    fn default() -> Self {
        BattlePromptScreen::new()
    }
}

impl Screen for BattlePromptScreen {
    fn id(&self) -> ScreenId {
        ScreenId::BattlePrompt
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "A Battle is to be fought".into()
    }

    /// A window over the campaign map, which the original repaints underneath
    /// it — `Screen_DrawCampaign(1)` is the first statement of both painters.
    fn is_overlay(&self) -> bool {
        true
    }

    /// **Two widgets and nothing else.**
    ///
    /// This is what `Screen_FrameInput`'s `0x12` arm actually is, and it took
    /// the input audit to find out. The whole arm is:
    ///
    /// ```c
    /// else if (g_screenId == '\x12') {
    ///     if (DAT_00553fc8 != 0) { Battle_Decline(); … }      /* the sync latch */
    ///     if (FUN_004bbea7() != 0) { Battle_Decline(); … }    /* the answer timeout */
    /// }
    /// ```
    ///
    /// and `FUN_004BBEA7` (`0x004BBEA7`) opens with
    /// `if (g_multiplayer == 0) return 0;`. **In a single-player game the arm
    /// does nothing at all** — no right-button test, no key, no OK corner. The
    /// only two exits are the two widgets of `DAT_004DDBB0`, which
    /// `Screen_HandleInput` (`0x004BA9C8`) hit-tests at offset `(0x20, 0x30)`
    /// with a count of `DAT_00554408`; that count is written by
    /// `Battle_ChooseSettlement` (`0x004A6A30`) and is **2 when
    /// `g_battleChoiceOwner == 1` and 0 otherwise**, so a bystander's prompt has
    /// no widgets and no way out but the multiplayer timeout. The table holds
    /// exactly two records — `g_sliderWidgets` begins at `0x004DDBE0`, 48 bytes
    /// on — so there is no third widget hiding behind the count.
    ///
    /// Gone from here, and counted as inventions rather than bugs
    /// (`docs/arms.json`): **right-click to Decline**, **Escape to Decline**,
    /// **Enter to take the field**, and answering on any click for a bystander.
    /// The prompt waits for ever in single player and that is correct — it is
    /// what `docs/symbols.json` records of `Battle_Decline` and it is not a
    /// thing to fix.
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **A bystander has no choice to make**, and no widget either. The
        // screen is a notice; the original leaves it up until the multiplayer
        // timeout, which single player does not have. Ours is reachable only
        // through the interactive door, and a bystander battle in a
        // single-player game cannot arise — `Battle_ChooseSettlement` returns 0
        // when neither side is human and the battle is settled with no screen —
        // so this arm is a guard against a state that has no route to it.
        let Some(q) = BattlePromptScreen::question(ctx) else { return Transition::Pop };
        if q.choice_owner != 1 {
            return Transition::Stay;
        }
        match event {
            // `DAT_004DDBB0[0]`, hotspot id 1 → `FUN_0043B593` →
            // `Battle_Start` (`0x004778A0`). It raises the battlefield; it does
            // **not** settle the battle.
            //
            // arm: 0x004BA9C8/prompt-fight left-release
            Event::Click { x, y } if widget_rect(TAKE_THE_FIELD).contains(x, y) => {
                if turn::take_the_field(ctx.game) {
                    Transition::Replace(ScreenId::Battlefield)
                } else {
                    // The armies could not be mustered — a slot is no longer a
                    // unit. Settle it the way a headless turn would rather than
                    // leaving the prompt up with nothing behind it.
                    BattlePromptScreen::answer(ctx, Answer::TakeTheField)
                }
            }
            // `DAT_004DDBB0[1]`, hotspot id 0 → `Battle_Decline`
            // (`0x0043B622`), which is `Battle_AutoResolve` and the report.
            //
            // arm: 0x004BA9C8/prompt-decline left-release
            Event::Click { x, y } if widget_rect(DECLINE).contains(x, y) => {
                BattlePromptScreen::answer(ctx, Answer::Decline)
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(q) = BattlePromptScreen::question(ctx) else { return };
        let p = pen(ctx);
        draw_frame(ctx, canvas, q.county, q.attacker_owner, q.defender_owner, q.is_siege);

        // **The heading is one of two**, and the siege one is the game saying
        // something different rather than the same thing about a siege: *"The
        // Siege commences."*
        let heading = if q.is_siege { 7 } else { 0 };
        let s = ctx.assets.shell.text(GROUP_PROMPT, heading).to_string();
        let s = if s.is_empty() { "A BATTLE IS TO BE FOUGHT.".to_string() } else { s };
        p.heading(canvas, HEADING.0, HEADING.1, &s, font::TEXT);

        // The paragraph, and the wrap width the original uses for each. 180 for
        // the question, 280 for the two statements — a narrower column where
        // the thumbs sit beside it.
        let (index, wrap) = match q.choice_owner {
            1 => (1usize, 0xB4),
            2 => (2, 0x118),
            _ => (3, 0x118),
        };
        let body = ctx.assets.shell.text(GROUP_PROMPT, index).to_string();
        let body = if body.is_empty() {
            match index {
                1 => "WILL YOU TAKE THE FIELD?".to_string(),
                2 => "YOUR OPPONENT HAS THE CHOICE.".to_string(),
                _ => "THE OPPONENTS ARE DECIDING.".to_string(),
            }
        } else {
            body
        };
        p.body_wrapped(canvas, PARAGRAPH.0, PARAGRAPH.1, wrap, &body, font::TEXT);

        draw_roster(
            ctx,
            canvas,
            (&q.attacker_roster, None),
            (&q.defender_roster, None),
        );

        // The two thumbs, and only when the choice is the local player's.
        if q.choice_owner == 1 {
            for w in [TAKE_THE_FIELD, DECLINE] {
                let drawn = ctx
                    .assets
                    .chrome
                    .as_ref()
                    .is_some_and(|c| c.draw_system(canvas, w.2, w.0, w.1));
                if !drawn {
                    shell::button_recess(canvas, w.0, w.1, w.3, w.3);
                }
            }
            // OURS: the thumbs are a mailed hand up and down and a modern
            // player has no legend for them. `docs/decisions.md` C21 — visibly
            // ours, in our own font, outside the original's widgets.
            let dim = ctx.assets.ink.dim;
            text::draw(canvas, TAKE_THE_FIELD.0 - 4, TAKE_THE_FIELD.1 + 34, "FIGHT", dim);
            text::draw(canvas, DECLINE.0 - 4, DECLINE.1 + 34, "AUTO", dim);
        }
    }
}

// ---------------------------------------------------------------- the result

/// Screen `0x13`. A **pure report** over a settled report: nothing it
/// draws changes anything, which is the one structural difference from the
/// original's painter (see the module header).
pub struct BattleResultScreen;

impl BattleResultScreen {
    pub fn new() -> BattleResultScreen {
        BattleResultScreen
    }
}

impl Default for BattleResultScreen {
    fn default() -> Self {
        BattleResultScreen::new()
    }
}

impl Screen for BattleResultScreen {
    fn id(&self) -> ScreenId {
        ScreenId::BattleResult
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The Battle is decided".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    /// **Two ways out, and `0x13` really does have the right-button one that
    /// `0x12` does not.**
    ///
    /// ```c
    /// if (g_mouseRightReleased == '\0') {
    ///     if (Ui_OkButtonClicked()) { g_screenId = 0; g_redrawRequest = 2; }
    /// } else { g_screenId = 0; g_redrawRequest = 2; }
    /// ```
    ///
    /// The two neighbouring screens differing on this is what made
    /// right-click-to-Decline on `0x12` look reasonable. There are no keys on
    /// either; Escape and Enter used to be here and were ours.
    ///
    /// // arm: 0x0042FF10/dismiss-report right-release
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let dismiss = matches!(event, Event::RightClick { .. })
            || matches!(event, Event::Click { x, y } if ok_rect().contains(x, y));
        if !dismiss {
            return Transition::Stay;
        }
        match turn::dismiss_report(ctx.game) {
            // Another battle this turn: back to the prompt for it.
            TurnStep::Ask(_) => Transition::Replace(ScreenId::BattlePrompt),
            TurnStep::Report(_) => Transition::Stay,
            // The rest of the turn belongs to the map, which winds it on one
            // tick a frame. See [`turn::TurnStep::Running`].
            TurnStep::Running | TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(r) = turn::pending_report(ctx.game) else { return };
        let p = pen(ctx);
        draw_frame(ctx, canvas, r.county, r.attacker_owner, r.defender_owner, r.is_siege);

        let heading = if r.is_siege { 8 } else { 0 };
        let s = ctx.assets.shell.text(GROUP_RESULT, heading).to_string();
        let s = if s.is_empty() { "THE BATTLE IS DECIDED.".to_string() } else { s };
        p.heading(canvas, HEADING.0, HEADING.1, &s, font::TEXT);

        // **The banner that belongs to screen `0x2B`.** Group 82's pair for
        // this battle, from the local player's point of view — printed here
        // rather than nowhere, because `0x2B` is not built and a player who is
        // told only *"The Battle is decided."* has not been told the outcome.
        // Marked as ours by being under the original's heading rather than in
        // place of it.
        let pair = r.outcome(ctx.game.player).pair();
        let banner = ctx.assets.shell.text(GROUP_BANNER, pair * 2).to_string();
        let banner = if banner.is_empty() { ours_banner(r.outcome(ctx.game.player)) } else { banner };
        p.body(canvas, PARAGRAPH.0, PARAGRAPH.1, &banner, font::HIGHLIGHT);

        draw_roster(
            ctx,
            canvas,
            (&r.attacker_roster.1, Some(&r.attacker_roster.0)),
            (&r.defender_roster.1, Some(&r.defender_roster.0)),
        );

        let drawn = ctx
            .assets
            .chrome
            .as_ref()
            .is_some_and(|c| c.draw_system(canvas, system::OK, OK.0, OK.1));
        if !drawn {
            crate::widget::button(canvas, &ctx.assets.ink, ok_rect(), "OK", false);
        }
    }
}

/// The seven banners in our own words, for an install with no `L2.eng`. The
/// original's are far better and are used whenever they are there.
fn ours_banner(outcome: Outcome) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every one of `L2.eng` group 82's seven pairs is reachable, and each maps
    /// to the index the original reads.
    ///
    /// Six come out of `Battle_SelectOutcomeBanner`'s two questions — was it a
    /// siege, and did the local player win — and the seventh is the pair for a
    /// battle he was in neither side of. That is what makes the count seven and
    /// not six or eight.
    #[test]
    fn all_seven_outcome_banners_are_reachable_and_distinct() {
        use l2_kingdom::battle::{outcome, Verdict};
        let a_won = Verdict::a_won(1, 2);
        let b_won = Verdict::b_won(1, 2);
        // (is_siege, local, winner, loser)
        let cases = [
            (false, 1u8, 1u8, 2u8, a_won, Outcome::Won),
            (false, 2, 1, 2, a_won, Outcome::Lost),
            (true, 1, 1, 2, a_won, Outcome::SiegeWon),
            (true, 1, 1, 2, b_won, Outcome::SiegeLifted),
            (true, 2, 1, 2, a_won, Outcome::CastleLost),
            (true, 2, 1, 2, b_won, Outcome::SiegeLost),
            (false, 9, 1, 2, a_won, Outcome::Bystander),
        ];
        let mut pairs = std::collections::BTreeSet::new();
        for (siege, local, winner, loser, verdict, want) in cases {
            let got = outcome(verdict, siege, local, winner, loser);
            assert_eq!(got, want, "siege={siege} local={local}");
            assert!(pairs.insert(got.pair()), "{want:?} shares a pair with another");
            // And each has a heading and a body, at 2n and 2n + 1.
            assert!(got.pair() * 2 + 1 < 14);
        }
        assert_eq!(pairs.len(), 7, "all seven, and no two the same");
        assert!(ours_banner(Outcome::Bystander).contains("CONFLICT"));
    }

    /// The widgets are inside the window and do not overlap, which is the one
    /// thing a transcribed hotspot table can get wrong in a way nothing else
    /// notices.
    #[test]
    fn both_thumbs_and_the_corner_are_inside_the_window() {
        let w = window();
        for r in [widget_rect(TAKE_THE_FIELD), widget_rect(DECLINE), ok_rect()] {
            assert!(r.x >= w.x && r.x + r.w <= w.x + w.w, "{r:?} escapes in x");
            assert!(r.y >= w.y && r.y + r.h <= w.y + w.h, "{r:?} escapes in y");
        }
        let (a, b) = (widget_rect(TAKE_THE_FIELD), widget_rect(DECLINE));
        assert!(a.x + a.w <= b.x, "the thumbs overlap: {a:?} {b:?}");
        // The table's own coordinates, plus the box's origin. Both halves are
        // stated so that a transcription slip in either shows up here.
        assert_eq!((a.x, a.y), (332, 116));
        assert_eq!((b.x, b.y), (372, 116));
    }

    /// The roster's seven rows and the totals under them all fit the window.
    #[test]
    fn the_roster_fits_between_the_names_and_the_corner() {
        let w = window();
        let last = ROSTER_Y + 6 * ROW_PITCH + 4;
        assert!(last < TOTAL_Y, "the rows run into the totals");
        assert!(TOTAL_Y < w.y + w.h, "the totals fall out of the window");
        assert!(ROSTER_Y > LORD_A.1, "the roster starts above the names");
        for x in [COL_A_BEFORE, COL_A_AFTER, COL_NOUN, COL_B_AFTER, COL_B_BEFORE] {
            assert!(x >= w.x && x < w.x + w.w, "column {x} is outside the window");
        }
    }
}
