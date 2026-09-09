//! **Battle Master ratings** — `Screen_BattleMasterRatings` (`0x00421707`),
//! `g_screenId` `0x2E`.
//!
//! # The shell's description was wrong in shape
//!
//! It said *"the seven rating rows per player and the shield sprites"*. There
//! are no rating rows. There are **two player blocks**, each a table of **seven
//! columns by three rows**: the columns are the seven campaign troop types and
//! the rows are *Before*, *Killed* and *Kills*. There are always exactly two
//! players, because this is the stand-alone Battle Master result screen and not
//! a campaign league table. The group — 37 — is **correct**.
//!
//! # `L2.eng` 37/1, *"Before"*, is drawn by nothing
//!
//! Group 37 is six strings: 0 *"Battle Master ratings"*, 1 *"Before"*,
//! 2 *"Killed"*, 3 *"Kills"*, 4 *"Scored"*, 5 *"The skirmish masters!!"*. Every
//! group-37 access in the whole binary was enumerated — eight, all in this
//! painter and `Screen_BattleMasterRank` — and **none uses index 1**. The top
//! row of both blocks is unlabelled unless `score1.pl8` paints the word into
//! the background. That is a defect in the original and [`BEFORE`] names it so
//! nobody goes looking again.
//!
//! # The painter, address by address
//!
//! ```text
//! Screen_BattleMasterRatings():                                 0x00421707
//!   FUN_00498DCB()                    battle assets; g_miscCtySheet := misc_ske.PL8
//!   File_ReadChunk("score1.256", palette, 0x300, 0)
//!   FUN_00408FCB("score1.pl8", 0x1E0)     a raw 640 x 480 page, not a sheet
//!   Palette_Set(score1.256)
//!   Ui_OkButton(0x204, 0x186, 0)                                 (516, 390)
//!   Ui_DrawCentred(37, 0, 0x60, 0x44, 0x1BE, heading)   centred in x 96..542
//!   block A, the local player, at y 100:
//!     Pl8_DrawFrame(misc_ske, realm.shieldIndex + 8, 0x70, 100)     (112, 100)
//!     Ui_DrawText(name, 0xD8, 0x6E, body)                          (216, 110)
//!     Eng_DrawString(37, 4, pen + 0xD8, 0x6E, body)     "Scored"
//!     Ui_DrawNumber(score, ' ', " ", pen + 0xEC, 0x69, heading)    y 105
//!     Eng_DrawString(37, 2, 0x68, 200, body)            "Killed"   (104, 200)
//!     Eng_DrawString(37, 3, 0x68, 0xDC, body)           "Kills"    (104, 220)
//!     for c in 0..7:
//!       hue = after[c] == 0 ? 0x3F : 0x20
//!       Ui_DrawNumberRight(before[c],            ' ', " ", c*0x32+0xAD, 0x0B4, 0x3C, body, hue)
//!       Ui_DrawNumberRight(before[c] - after[c], ' ', " ", c*0x32+0xAD, 200,   0x3C, body, hue)
//!       Ui_DrawNumberRight(theirBefore[c] - theirAfter[c], …, 0x0DC, …, 0xF9)
//!   block B, the opponent: the same, +160 in y, with the two sides swapped
//! ```
//!
//! Block pitch is exactly 160 and row pitch exactly 20. The column x is
//! `c * 0x32 + 0xAD` in a **60-pixel box** at a 50-pixel pitch, so the boxes
//! overlap by ten — and `Ui_DrawNumberRight` **centres** rather than
//! right-aligning (`FUN_004025D7` is `(width − textWidth) / 2`, the same helper
//! `Ui_DrawCentred` uses), which `docs/symbols.json` has wrong for every caller
//! in the binary.
//!
//! **The highlight is inverted from what you would guess**: a troop type with
//! **zero** survivors is drawn in `0x3F`, the colour every other label uses,
//! and a type with survivors in `0x20`. That is what the code says; whether it
//! looks right under `score1.256` is not settled.
//!
//! # The scoring rule — `FUN_0042C64F` (`0x0042C64F`), and it is documented nowhere
//!
//! `docs/rules.md` §6 *"How the score is calculated"* is `Score_RankRealms`,
//! the **campaign realm ranking** — a different mechanic with a different
//! formula. `docs/battle.md` stops before the outcome. `docs/mechanics.md` has
//! no skirmish row. Everything in [`score`] is new.
//!
//! Both scores are computed **once, at the end of the battle**, and this
//! painter only reads them. The weights are `g_troopStrengthWeight`
//! (`0x004D4B98`) — `2, 16, 8, 13, 9, 13, 22` — read out of the executable, in
//! the same troop order the seven columns are drawn in.
//!
//! ```text
//! strength(army)   = Σ troops[t] * weight[t]
//! killShare(me)    = 100 * (theirStrengthLost) / (bothStrengthsLost)
//! survive(me)      = 100 * menAfter / menBefore
//! score            = 6 * killShare + 3 * survive + 500
//! ```
//!
//! with the last two terms dropped by ladder:
//!
//! | case | mine | theirs |
//! |---|---|---|
//! | I withdrew | **0** | `6 k + 3 s`, **no 500** |
//! | he withdrew | `6 k + 3 s`, no 500 | **0** |
//! | my castle fell | `6 k` | `6 k + 3 s + 500` |
//! | I took his castle | `6 k + 3 s + 500` | `6 k` |
//! | I was wiped out | `6 k` | `6 k + 3 s + 500` |
//! | otherwise | `6 k + 3 s + 500` | `6 k` |
//!
//! So the **ceiling is 1,400** — 600 kill share, 300 survival, 500 for the win
//! — and the two kill-share terms always sum to at most 600 between the
//! players. A withdrawal is the only case that scores a flat zero, and it
//! scores the *winner* no bonus either.
//!
//! ## The defensive-advantage handicap is computed and thrown away — **[D]**
//!
//! Immediately before the ladder the original computes `Pct(300, adv * 10)` and
//! `Pct(300, 100 − adv * 10)` from `g_troopsAdvantage` and **assigns neither**.
//! The two sum to 300, which is the shape of the `3 * survive` term they were
//! meant to replace. As decompiled, in every battle category except 0 — that is
//! every castle battle and every `.skr` scenario — the handicap has no effect
//! on either score. Marked `[D]` rather than `[V]` because it rests on Ghidra's
//! dead-value elimination and not on the disassembly; one look at `0x0042C8xx`
//! would settle it. [`HANDICAP_IS_DEAD`] is the switch, and it is *off*,
//! because reproducing a dead computation is reproducing nothing.
//!
//! # It is not dead code, and the way out is forward
//!
//! Seven sites write `g_screenId = 0x2E`, every one guarded by `DAT_0057A0F0`
//! — *this is a stand-alone battle, not a campaign one* — whose `== 0` sibling
//! goes to campaign screen `0x13` instead. So the ratings screen is the
//! skirmish's end and `screens/battle.rs` is the campaign's.
//!
//! `Screen_FrameInput`'s arm is three lines and worth quoting whole:
//!
//! ```c
//! if (g_screenId == 0x2E) {
//!   if (g_mouseLeftPressed)  { g_screenId = 0x2F; g_redrawRequest = 2; }
//!   if (g_mouseRightPressed) { g_screenId = 0x2F; g_redrawRequest = 2; }
//! }
//! ```
//!
//! Three things follow, and each is a decision somebody made:
//!
//! * **the OK picture at (516, 390) is decorative** — the arm never calls
//!   `Ui_OkButtonClicked`, so a click *anywhere* leaves;
//! * it fires on **press**, not release, unlike essentially every other arm in
//!   the function;
//! * it goes **forward** to `0x2F`, the rank screen, not back. The exit chain is
//!   `0x2E` → click → `0x2F` → click → `0x1F`, the skirmish setup page. This
//!   screen has no back door.
//!
//! `0x2F` is not built. This screen therefore pops, which is where `0x2F`
//! eventually returns to anyway, and the difference is recorded.
//!
//! # What is not here, and why
//!
//! **The skirmish itself.** This engine has a campaign battlefield and no
//! Battle Master mode: there is no `DAT_0057A0F0`, no eight-slot before/after
//! snapshot and no `score.dat`. So [`Ratings`] is a value the *battle* fills in
//! and this screen draws, and until a skirmish exists the only thing that fills
//! it is a test. That is stated rather than hidden: a screen whose input does
//! not exist yet is honest about it, and the scoring rule is the part worth
//! having now, because it is a rule and it was written down nowhere.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 37.
pub const GROUP: usize = 37;
pub const HEADING: usize = 0;
/// **Never drawn by anything in the binary.** See the module docs.
pub const BEFORE: usize = 1;
pub const KILLED: usize = 2;
pub const KILLS: usize = 3;
pub const SCORED: usize = 4;

/// The full-screen page and its palette.
pub const BACKGROUND: &str = "Score1.pl8";
pub const PALETTE: &str = "Score1.256";

pub const OK: Rect = Rect::new(0x204, 0x186, 24, 24);
/// `Ui_DrawCentred(37, 0, 0x60, 0x44, 0x1BE, …)`.
pub const HEADING_AT: (i32, i32, i32) = (0x60, 0x44, 0x1BE);

/// The two player blocks are 160 pixels apart and everything below is relative
/// to the block's own top.
pub const BLOCK_Y: [i32; 2] = [100, 260];
pub const SHIELD_X: i32 = 0x70;
/// `misc_ske.PL8` frame `shieldIndex + 8` — frames 9…13 are the five 65 × 74
/// shields, measured from the file.
pub const SHIELD_FRAME0: usize = 8;
pub const NAME_AT: (i32, i32) = (0xD8, 10);
pub const SCORE_DY: i32 = 5;
/// The label column, and the three row offsets from the block's top.
pub const LABEL_X: i32 = 0x68;
pub const ROW_DY: [i32; 3] = [80, 100, 120];
/// The seven columns: `c * 0x32 + 0xAD`, centred in a 60-pixel box.
pub const COL_X0: i32 = 0xAD;
pub const COL_PITCH: i32 = 0x32;
pub const COL_W: i32 = 0x3C;
pub const COLUMNS: usize = 7;

/// `g_troopStrengthWeight` (`0x004D4B98`), read out of `Lords2.exe`, in the
/// `TROOPS*.ENG` column order the seven rating columns are drawn in: peasant,
/// crossbowman, maceman, swordsman, pikeman, archer, knight.
pub const STRENGTH_WEIGHT: [i32; COLUMNS] = [2, 16, 8, 13, 9, 13, 22];

/// The three components of a score, before the ladder drops any of them.
pub const KILL_SHARE_SCALE: i32 = 6;
pub const SURVIVAL_SCALE: i32 = 3;
pub const WIN_BONUS: i32 = 500;
/// `6 * 100 + 3 * 100 + 500`.
pub const MAX_SCORE: i32 = KILL_SHARE_SCALE * 100 + SURVIVAL_SCALE * 100 + WIN_BONUS;

/// **The defensive-advantage handicap is dead in the shipped binary.** Off, and
/// see the module docs for what it would have been.
pub const HANDICAP_IS_DEAD: bool = true;

/// One army at one moment: the seven troop counts and the head count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub troops: [i32; COLUMNS],
    pub men: i32,
}

impl Snapshot {
    /// `Σ troops[t] * g_troopStrengthWeight[t]`.
    pub fn strength(&self) -> i32 {
        self.troops.iter().zip(STRENGTH_WEIGHT).map(|(n, w)| n * w).sum()
    }
}

/// How the battle ended, which is what the ladder switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// One side left the field. The withdrawer scores **zero** and the other
    /// side gets no win bonus.
    Withdrew { local: bool },
    /// A castle was entered. `local` is true when it was the local player's.
    CastleFell { local: bool },
    /// Nobody withdrew and no castle fell.
    Fought,
}

/// Everything the two blocks draw, and everything [`score`] reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratings {
    /// `(before, after)` for the local player and for the opponent.
    pub mine: (Snapshot, Snapshot),
    pub theirs: (Snapshot, Snapshot),
    pub ending: Ending,
    /// Shield indices for the two blocks.
    pub shields: (u8, u8),
}

impl Default for Ratings {
    fn default() -> Ratings {
        Ratings {
            mine: (Snapshot::default(), Snapshot::default()),
            theirs: (Snapshot::default(), Snapshot::default()),
            ending: Ending::Fought,
            shields: (1, 2),
        }
    }
}

/// `PctOf(a, b) = a * 100 / b`, **zero when `b` is zero** — the original's own
/// guard, and the reason a battle that started with no men does not divide.
fn pct_of(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a * 100 / b
    }
}

/// **`FUN_0042C64F`** — both scores, `(local, opponent)`.
///
/// See the module docs for the ladder and for the handicap this deliberately
/// does not compute.
pub fn score(r: &Ratings) -> (i32, i32) {
    let lost_mine = r.mine.0.strength() - r.mine.1.strength();
    let lost_theirs = r.theirs.0.strength() - r.theirs.1.strength();
    let total = lost_mine + lost_theirs;
    // **The kill share is the share of destroyed strength that was the OTHER
    // side's**, which is why the two arguments are crossed.
    let kill_mine = pct_of(lost_theirs, total);
    let kill_theirs = pct_of(lost_mine, total);
    let survive_mine = pct_of(r.mine.1.men, r.mine.0.men);
    let survive_theirs = pct_of(r.theirs.1.men, r.theirs.0.men);

    let full = |kill: i32, survive: i32| {
        KILL_SHARE_SCALE * kill + SURVIVAL_SCALE * survive + WIN_BONUS
    };
    let no_bonus = |kill: i32, survive: i32| KILL_SHARE_SCALE * kill + SURVIVAL_SCALE * survive;
    let kill_only = |kill: i32| KILL_SHARE_SCALE * kill;

    match r.ending {
        // A withdrawal zeroes the withdrawer and denies the other side the
        // bonus. It is the only case that produces a flat zero.
        Ending::Withdrew { local: true } => (0, no_bonus(kill_theirs, survive_theirs)),
        Ending::Withdrew { local: false } => (no_bonus(kill_mine, survive_mine), 0),
        Ending::CastleFell { local: true } => {
            (kill_only(kill_mine), full(kill_theirs, survive_theirs))
        }
        Ending::CastleFell { local: false } => {
            (full(kill_mine, survive_mine), kill_only(kill_theirs))
        }
        // `menAfter < 1` is the wipe-out, and it reads exactly like a lost
        // castle. Everything else is a win for the local player.
        Ending::Fought if r.mine.1.men < 1 => {
            (kill_only(kill_mine), full(kill_theirs, survive_theirs))
        }
        Ending::Fought => (full(kill_mine, survive_mine), kill_only(kill_theirs)),
    }
}

pub struct RatingsScreen {
    ratings: Ratings,
}

impl RatingsScreen {
    pub fn new() -> RatingsScreen {
        RatingsScreen { ratings: Ratings::default() }
    }

    pub fn with(ratings: Ratings) -> RatingsScreen {
        RatingsScreen { ratings }
    }

    pub fn ratings(&self) -> &Ratings {
        &self.ratings
    }
}

impl Default for RatingsScreen {
    fn default() -> RatingsScreen {
        RatingsScreen::new()
    }
}

impl Screen for RatingsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Ratings
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Battle Master ratings — screen 0x2E".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(PALETTE)
    }

    /// A full page: `score1.pl8` is a raw 640 × 480 image, not a sheet.
    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // **Any press, either button, anywhere.** The OK picture is not
            // tested and this fires on press rather than release.
            //
            // The original goes *forward* to `0x2F`, the rank screen, which is
            // not built; `0x2F` returns to the skirmish setup page, which is
            // where popping lands too.
            // arm: 0x0042FF10/ratings-any-press
            Event::Click { .. } | Event::RightClick { .. } => Transition::Pop,
            // **Ours.**
            // arm: ours/ratings-keyboard-close
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        if !crate::shell::background(canvas, a, BACKGROUND) {
            canvas.clear(ink.background);
        }
        pen.ok_button(canvas, OK.x, OK.y, 0);
        pen.eng_heading_centred(
            canvas,
            GROUP,
            HEADING,
            HEADING_AT.0,
            HEADING_AT.1,
            HEADING_AT.2,
            font::TEXT,
        );

        let (mine, theirs) = score(&self.ratings);
        let blocks = [
            (self.ratings.mine, self.ratings.theirs, self.ratings.shields.0, mine),
            (self.ratings.theirs, self.ratings.mine, self.ratings.shields.1, theirs),
        ];
        for (b, &((before, after), (their_before, their_after), shield, points)) in
            blocks.iter().enumerate()
        {
            let top = BLOCK_Y[b];
            pen.misc_frame(canvas, SHIELD_FRAME0 + shield as usize, SHIELD_X, top);
            // The lord names are not in this tree; `screens/battle.rs` has the
            // same hole and this is its stand-in rather than a second one.
            let name = format!("PLAYER {}", b + 1);
            let w = pen.body(canvas, NAME_AT.0, top + NAME_AT.1, &name, font::TEXT);
            let w = w + pen.eng(canvas, GROUP, SCORED, NAME_AT.0 + w, top + NAME_AT.1, font::TEXT);
            pen.number(
                canvas,
                NAME_AT.0 + w + 20,
                top + NAME_AT.1 - SCORE_DY,
                points,
                false,
                font::TEXT,
            );

            // Row 1 has no label — `L2.eng` 37/1 is drawn by nothing.
            pen.eng(canvas, GROUP, KILLED, LABEL_X, top + ROW_DY[1], font::TEXT);
            pen.eng(canvas, GROUP, KILLS, LABEL_X, top + ROW_DY[2], font::TEXT);

            for c in 0..COLUMNS {
                let x = c as i32 * COL_PITCH + COL_X0;
                // The inverted highlight: a wiped-out troop type takes the
                // *ordinary* colour and a surviving one takes `0x20`.
                let hue = if after.troops[c] == 0 { font::TEXT } else { 0x20 };
                pen.number_centred(canvas, x, top + ROW_DY[0], COL_W, before.troops[c], hue);
                pen.number_centred(
                    canvas,
                    x,
                    top + ROW_DY[1],
                    COL_W,
                    before.troops[c] - after.troops[c],
                    hue,
                );
                pen.number_centred(
                    canvas,
                    x,
                    top + ROW_DY[2],
                    COL_W,
                    their_before.troops[c] - their_after.troops[c],
                    0xF9,
                );
            }
        }

        l2_view::text::draw(
            canvas,
            4,
            470,
            "NO SKIRMISH MODE: THESE ARE A BATTLE'S NUMBERS AND NOTHING FILLS THEM YET",
            ink.dim,
        );
    }
}
