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
//! two lord names                  (40,160) and (240,160)    same
//! roster FUN_004224E7             mode 0                    mode 1
//! buttons                         thumb up / thumb down     the OK corner
//! ```
//!
//! # Border set **1**
//!
//! `FUN_004093E0` is not `Ui_DrawBox`. It is the *other* frame kit, and passing
//! set 0 draws a visibly wrong window — [`Pen::window`]'s last argument is 1 for
//! both of these and 0 for almost everything else in the game.
//!
//! # Two things the original draws that are not here, and one it does not
//!
//! * **The lords' faces and names.** `g_playerNames` and the `icon_tmp.pl8`
//!   medallion are not modelled; a realm has no name in this tree. Each side is
//!   drawn as its realm number, in our own font, which `docs/decisions.md` C21
//!   is the rule for: where we cannot establish what the original drew, it is
//!   visibly ours. The one name we *can* draw is the original's own —
//!   `L2.eng` group 99, the single string `"The people."`, which it draws for
//!   an ownerless army. A county levy is exactly that.
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
//! does not build. [`crate::engagement::BattleReport::outcome`] already answers
//! which pair, so that screen is a painter and no more reverse engineering; this
//! one prints the pair's heading under its own so that the result of a battle is
//! not silently lost while `0x2B` does not exist.

use l2_kingdom::battle::Outcome;
use l2_kingdom::unit::ALL_TROOP_TYPES;
use l2_view::chrome::system;
use l2_view::{text, Canvas};

use crate::engagement::{Answer, Roster};
use crate::input::{Event, Key, Rect};
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
/// `Ui_DrawBevelRect(0x34, 0x44, 0x52, 0x52)` — the medallion's recess.
const MEDALLION: Rect = Rect::new(0x34, 0x44, 0x52, 0x52);
/// The two lord names, each centred in 200 pixels.
const LORD_A: (i32, i32, i32) = (40, 160, 200);
const LORD_B: (i32, i32, i32) = (240, 160, 200);

/// `FUN_004224E7(a, b, 0x3E, 0xE0, …)` — the roster's origin, and the pitch of
/// its seven rows.
const ROSTER_Y: i32 = 0xE0;
const ROW_PITCH: i32 = 0x18;
/// Absolute x of each column, out of the painter.
const COL_A_BEFORE: i32 = 56;
const COL_A_AFTER: i32 = 102;
const COL_NOUN: i32 = 185;
const COL_B_AFTER: i32 = 342;
const COL_B_BEFORE: i32 = 390;
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

/// The common half of both screens: the window, the county, the medallion
/// recess and the two names.
fn draw_frame(ctx: &Ctx, canvas: &mut Canvas, county: u8, a_owner: u8, b_owner: u8) {
    let p = pen(ctx);
    p.window(canvas, BOX_X, BOX_Y, BOX_COLS, BOX_ROWS, BOX_SET);
    p.body_centred(canvas, COUNTY.0, COUNTY.1, COUNTY.2, &county_name(ctx, county), font::TEXT);

    // The medallion. `icon_tmp.pl8` is loaded by these two screens alone and we
    // do not load it, so the recess stays empty and looks it.
    canvas.fill_rect(MEDALLION.x, MEDALLION.y, MEDALLION.w, MEDALLION.h, ctx.assets.ink.background);
    crate::widget::frame(canvas, MEDALLION, ctx.assets.ink.border);

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
            TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
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

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // **A bystander has no choice to make.** `Battle_ChooseSettlement`
        // writes a widget count of 2 when the local player holds the choice and
        // 0 otherwise, so the thumbs are not drawn and cannot be clicked; the
        // battle is settled by the policy and the screen is only a notice.
        let Some(q) = BattlePromptScreen::question(ctx) else { return Transition::Pop };
        if q.choice_owner != 1 {
            return match event {
                Event::Click { .. } | Event::RightClick { .. } | Event::KeyDown(_) => {
                    let a = ctx.game.field_policy;
                    BattlePromptScreen::answer(ctx, a)
                }
                _ => Transition::Stay,
            };
        }
        match event {
            Event::Click { x, y } if widget_rect(TAKE_THE_FIELD).contains(x, y) => {
                BattlePromptScreen::answer(ctx, Answer::TakeTheField)
            }
            Event::Click { x, y } if widget_rect(DECLINE).contains(x, y) => {
                BattlePromptScreen::answer(ctx, Answer::Decline)
            }
            // `Screen_FrameInput`'s `0x12` arm: a right release runs
            // `Battle_Decline`. **There is no timeout** — the gate that would
            // impose one returns 0 outright unless `g_multiplayer`, so in a
            // single-player game the prompt waits for ever.
            Event::RightClick { .. } | Event::KeyDown(Key::Escape) => {
                BattlePromptScreen::answer(ctx, Answer::Decline)
            }
            Event::KeyDown(Key::Enter) => BattlePromptScreen::answer(ctx, Answer::TakeTheField),
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(q) = BattlePromptScreen::question(ctx) else { return };
        let p = pen(ctx);
        draw_frame(ctx, canvas, q.county, q.attacker_owner, q.defender_owner);

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
            text::draw(
                canvas,
                TAKE_THE_FIELD.0 - 4,
                TAKE_THE_FIELD.1 + 34,
                "FIGHT",
                ctx.assets.ink.dim,
            );
            text::draw(canvas, DECLINE.0 - 4, DECLINE.1 + 34, "AUTO", ctx.assets.ink.dim);
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

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let dismiss = matches!(
            event,
            Event::RightClick { .. } | Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter)
        ) || matches!(event, Event::Click { x, y } if ok_rect().contains(x, y));
        if !dismiss {
            return Transition::Stay;
        }
        match turn::dismiss_report(ctx.game) {
            // Another battle this turn: back to the prompt for it.
            TurnStep::Ask(_) => Transition::Replace(ScreenId::BattlePrompt),
            TurnStep::Report(_) => Transition::Stay,
            TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(r) = turn::pending_report(ctx.game) else { return };
        let p = pen(ctx);
        draw_frame(ctx, canvas, r.county, r.attacker_owner, r.defender_owner);

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
