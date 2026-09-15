//! `CLAUDE.md` rule 5. The middle column is `Screen_FrameInput`'s
//! (`0x0042FF10`) own dispatch order, which is a **ladder of guards**: the first
//! one that consumes the input ends the frame. Ours is the same ladder in the
//! same order, and where a guard declines we fall through.
//!
//! | # | arm | the original |
//! |---|---|---|
//! | 1 | the menu bar's three titles | `Menu_OpenDropdown` `0x0040DECA` — [`crate::screens::menubar`] |
//! | 2 | the pointer at the edge of the screen scrolls one cell | `Map_EdgeScroll` `0x00432221` |
//! | 3 | five buttons at (480, 448) | `FUN_004329A4` `0x004329A4`, table `0x004DC710` |
//! | 4 | left press on the field starts a box | `FUN_0043BF07` `0x0043BF07` |
//! | 5 | left release on the ground orders | `FUN_0043C57D` `0x0043C57D` |
//! | 6 | left press on a banner drops that man | `FUN_0043C2A9` `0x0043C2A9` |
//! | 7 | **right release clears the selection** | `FUN_0043C55C` `0x0043C55C` |
//! | 8 | the overview panel orders, or looks | `BattleMap_Click` `0x00432443` |
//! | 9 | the pointer picks one of four cursors | `Battle_Frame` `0x004B99C0`'s ladder |
//! | 10 | `1` … `9` recall a group | `FUN_0043C910` `0x0043C910` |
//! | 11 | `Ctrl` + `1` … `9` store one | `FUN_0043C885` `0x0043C885` |
//! | 12 | `H` forms a line | `FUN_0043C77A(0)` `0x0043C77A` |
//! | 13 | `V` forms a column | `FUN_0043C77A(1)` |
//! | 14 | `F2` cycles the debug panel | `0x004B29BE` — **not reproduced**, and gated on a debug flag |
//! | 15 | the arrow keys walk the debug figure | `0x004B29BE` — **not reproduced** |
//! | 16 | a right release dismisses the message scroll | `FUN_0047685D` — not this screen's |
//!
//! | # | arm | the original |
//! |---|---|---|
//! | 17 | the box follows the pointer | `FUN_0043BF07`'s held branch |
//! | 18 | release commits it | `FUN_0043BF07`'s release branch, `FUN_00479CF7` |
//! | 19 | **a double click commits it too** | the same branch's second test |
//! | 20 | right release cancels | inline, `g_screenId = 0x29` |
//! | 21 | the pointer leaving the field cancels | inline, `g_battleHoverOnField == 0` |
//! | 22 | the cursor is forced to the plain arrow | `Battle_Frame`'s ladder |
//!
//! | # | arm | the original |
//! |---|---|---|
//! | 23 | a right release skips the five-thousand-frame wait | inline, `DAT_00568470 = 0x1389` |
//! | 24 | the wait itself | `Battle_CheckOutcome` `0x00477DFC` |
//!
//! **The bar was not drawn here at all.** `Screen_DrawMenuBar` (`0x00419C78`)
//! guards itself on a list of screen ids it *refuses* — `0x08 … 0x0D`, `0x17`,
//! `0x1B … 0x20`, `0x22`, `0x2C … 0x2F` — and `0x29`, `0x2A` and `0x2B` are in
//! none of them, so the original paints the bar through the whole of a battle.
//!
//! The
//! drop-down painter, `FUN_0040C725`, is a two-way branch per row — `0x18` on a
//! plate for the row under the pointer, `0x3F` for every other — with no third
//! colour, no skip and no flag; `FUN_0040E099` hit-tests every row and
//! `FUN_0040DD92` dispatches whatever it returns. Not one of the sixteen
//! handlers reads `g_battlePhase`, and neither does `Menu_OpenDropdown`. So
//! **File > Save, File > Load, File > New Game and File > Quit are live in a
//! battle**, as are all five Options rows and all seven Help rows.
//!
//! The game *does* ship a greyed-item widget — `FUN_0040CB7C` paints a third
//! colour, `1`, when `FUN_0040CB4E(DAT_0058FCC0[id])` is set, over 20-byte
//! records with a visibility short at `+0x0E` — and it is **dead code**:
//!
//! `FUN_0040E12E`, its only entry point, has no callers anywhere in the binary,
//! and `DAT_0058FCC0` is written by nothing outside that cluster. **[V]**
//!
//! What a battle *does* take off the bar is two things, both inside
//! `Screen_DrawMenuBar` itself and both on `g_battlePhase == 0`: the per-realm
//! shield banners, and the year with its season. Four more painters check the
//! same flag and none of them is a menu: `Screen_DrawEndTurn` (`0x0041A734`),
//! the turn timer (`FUN_0041A639`), the minimap (`FUN_00410F4B`) and
//! `CountyStrip_Draw` (`0x0040F7D3`), which returns at once when
//! `g_battlePhase != 0`. The last two are why the campaign sidebar is not on
//! this screen: the frame loop paints both every frame whatever the screen id
//! is, and a battle is the one thing that stops them.
//!
//! Two of the five buttons open `Ui_OpenConfirm` (`0x0040E6F2`), which is screen
//! `0x1E`, a screen this tree does not have. The prompt indices are the
//! original's — `L2.eng` group 10, index 12 *"Retreat from field?"*, 11
//! *"Surrender castle?"*, 9 *"Autocalc battle?"* — and so is the geometry, a
//! 14 × 8 cell box at `(g_confirmX − 16, g_confirmY − 16)` with the thumbs at
//! `+ (64, 46)` and `+ (112, 50)`. **What is ours is that it is drawn by this
//! screen instead of by `Screen_ConfirmBox` on a screen of its own** — and that
//! is now the only thing left that is: the ground is `Screen_ConfirmBox`'s own
//! `FUN_004093E0(…, 0xE, 8)` in border set 1, and
//! the two pictures are `g_confirmWidgets`' frames 29 and 31
//! close corner and its neighbour.
//!
//! `Screen_BattleOutcome` (`0x00423241`) has three arms. The short window is
//! the one with animations off. The animated arm — `g_optAnimations` set and
//! the local player one of the two sides — dims the field, draws a taller
//! window with a 402 × 194 recess at (39, 72) and moves its text down 168
//! pixels, and `Battle_CheckOutcome` then plays one of
//! [`crate::movie::BATTLE_FILMS`] in the recess; when the film ends the banner
//! goes with it. The third arm — `g_battleChoiceOwner == 0`, a battle between
//! two other realms — is the neutral pair 12/13. Which pair is
//! `Battle_SelectOutcomeBanner`'s four-way siege reading, [`outcome_banner`].

mod view;
pub use view::*;

use l2_sim::runner::Formation;
use l2_sim::terrain::DIM;
use l2_view::{text, Canvas};

use crate::battlefield::{
    self, BannerLayout, Button, Cursor, LiveBattle, Mode, OVERVIEW, TILE, VIEW, VIEW_COLS,
    VIEW_ROWS,
};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::menubar;
use crate::shell::{font, Pen};
use crate::turn::{self, TurnStep};

/// `L2.eng` group 10 — the game's directory of confirmable actions.
pub const GROUP_CONFIRM: usize = 10;
/// `L2.eng` group 32 index 0 — what `FUN_00423B4F` prints across the bottom of
/// a paused battlefield.
pub const GROUP_PAUSED: usize = 32;
/// `L2.eng` group 82 — the seven outcome heading/body pairs, drawn on `0x2B`.
pub const GROUP_BANNER: usize = 82;

/// **The border set every one of these boxes is drawn in.** `FUN_004093E0` is
/// `Ui_DrawBoxBorder(1, …)` followed by `Ui_DrawBoxInterior` inset a cell —
/// the four-argument form is *always* set 1, and both painters below use it.
pub const BOX_SET: usize = 1;

/// `Ui_OpenConfirm(prompt, 0xA0, 0xA0, …)` and `Screen_ConfirmBox`
/// (`0x0040CCFA`): a 14 × 8 cell box at `(0xA0 − 0x10, 0xA0 − 0x10)`.
pub const CONFIRM_BOX: Rect = Rect::new(0xA0 - 0x10, 0xA0 - 0x10, 14 * 16, 8 * 16);
pub const CONFIRM_COLS: i32 = 14;
pub const CONFIRM_ROWS: i32 = 8;
/// `g_confirmWidgets` (`0x004DD310`), offset by `(g_confirmX, g_confirmY)`.
pub const CONFIRM_YES: Rect = Rect::new(0xA0 + 64, 0xA0 + 46, 32, 32);
pub const CONFIRM_NO: Rect = Rect::new(0xA0 + 112, 0xA0 + 50, 32, 32);
pub const CONFIRM_YES_FRAME: usize = 29;
pub const CONFIRM_NO_FRAME: usize = 31;

/// Both are `Widget_Test` kind **5**, read out of `0x004DD310` and `0x004DD328`:
///
/// Order matters: index 0 is hotspot id **1**, the tick, and index 1 is id
/// **0**, the cross. `Ui_ConfirmClicked` (`0x00434E1F`) is
/// `g_confirmAnswer = g_uiHotspotId; (*g_confirmCallback)();` — the answer *is*
/// the hotspot id.
pub const CONFIRM_WIDGETS: [Widget; 2] = [
    Widget::new(CONFIRM_YES, crate::arm!("0x00434E1F/confirm-yes", Delayed)),
    Widget::new(CONFIRM_NO, crate::arm!("0x00434E1F/confirm-no", Delayed)),
];

/// `Screen_BattleOutcome` (`0x00423241`)'s short window —
/// `FUN_004093E0(0x10, 0x90, 0x1C, 0x0A)` — and the three things inside it.
pub const OUTCOME_BOX: Rect = Rect::new(0x10, 0x90, 0x1C * 16, 0x0A * 16);
pub const OUTCOME_COLS: i32 = 0x1C;
pub const OUTCOME_ROWS: i32 = 0x0A;
pub const OUTCOME_OK: (i32, i32) = (0x1A0, 0x100);
pub const OUTCOME_HEAD: (i32, i32) = (0x30, 0xA8);
/// `FUN_0040328E(0x52, pair*2 + 1, 0x30, 0xE0, 0x180, 100, 0, 0, …)`.
pub const OUTCOME_BODY: (i32, i32, i32) = (0x30, 0xE0, 0x180);

pub struct BattlefieldScreen {
    /// The open yes/no box's `L2.eng` group 10 prompt index, if one is up.
    confirm: Option<usize>,
    press: Press,
    redraw: bool,
    outcome_seen: bool,
    overview: Overview,
    /// **Which tile sheets and which palette this battle runs under** —
    /// `Battle_LoadAssets`' `g_battleIsSiege` / `DAT_0057C910` ladder, cached
    /// because [`Screen::palette`] is handed no world. Written by
    /// [`Screen::update`] and by [`Screen::draw`], both of which the presenter
    /// runs before it asks for a palette.
    ground: l2_view::scene::Ground,
    /// `DAT_004E5B18`, the keep banner's eight-frame cycle —
    /// `BattleBanner_Draw` (`0x004BD574`) steps it on `g_pulse80` and wraps it
    /// past 7. Stepped here because `Screen::draw` is handed no clock.
    banner: Banner,
}

impl BattlefieldScreen {
    pub fn new() -> BattlefieldScreen {
        BattlefieldScreen {
            confirm: None,
            press: Press::new(),
            redraw: true,
            outcome_seen: false,
            overview: Overview::new(),
            ground: l2_view::scene::Ground::Field,
            banner: Banner::default(),
        }
    }

    /// `g_units[g_battleArmyB] + 0x02` — the garrison's shield, a copy of its
    /// realm's `+0x0A`. `g_battleArmyB` is the side-0 army, which in a siege is
    /// the defender. A field battle has no keep cell, so it has no banner.
    fn banner_of(&self, ctx: &Ctx) -> Option<(u8, u8)> {
        let live = ctx.game.battle.as_ref()?;
        if !live.is_siege() {
            return None;
        }
        let owner = ctx.game.kingdom.campaign.units.get(live.defender)?.owner;
        let shield = ctx.game.kingdom.realms.get(owner as usize)?.shield_index;
        Some((shield, self.banner.phase))
    }

    fn note_ground(&mut self, ctx: &Ctx) -> bool {
        let was = self.ground;
        self.ground = l2_view::scene::Ground::for_battle(
            ctx.game.battle.as_ref().and_then(|b| b.castle_level),
        );
        was != self.ground
    }

    fn step_overview(&mut self, ctx: &Ctx) -> bool {
        let Some(live) = ctx.game.battle.as_ref() else { return false };
        let Some(sheets) =
            ctx.assets.battle.as_ref().and_then(|a| a.ground(self.ground).overview.as_ref())
        else {
            return false;
        };
        let rows =
            if self.overview.full { DIM } else { l2_view::scene::OVERVIEW_ROWS_PER_FRAME };
        self.overview.row += rows;
        if DIM - rows < self.overview.row {
            self.overview.row = 0;
        }
        let occupants = overview_occupants(ctx.game, live);
        l2_view::scene::draw_overview_rows(
            &mut self.overview.raster,
            &live.runner.field,
            &occupants,
            sheets,
            self.overview.row,
            rows,
        );
        self.overview.full = false;
        true
    }

    fn live<'a>(ctx: &'a mut Ctx) -> Option<&'a mut LiveBattle> {
        ctx.game.battle.as_deref_mut()
    }

    /// The five buttons, in table order — `FUN_004329A4`'s `Hotspot_Test`.
    ///
    /// // arm: 0x004329A4/buttons left-press
    fn press_button(&mut self, ctx: &mut Ctx, b: Button) -> Transition {
        let is_siege = ctx.game.battle.as_ref().is_some_and(|l| l.is_siege());
        let garrison_is_local = ctx
            .game
            .battle
            .as_ref()
            .and_then(|l| ctx.game.kingdom.campaign.units.get(l.defender))
            .is_some_and(|u| u.owner == ctx.game.player);
        let Some(live) = BattlefieldScreen::live(ctx) else { return Transition::Pop };
        match b {
            Button::Pause => {
                live.press_pause();
            }
            Button::Retreat => self.confirm = live.press_retreat(),
            Button::Sally => {
                let _ = is_siege;
                let _ = live.press_sally(garrison_is_local);
            }
            Button::Charge => {
                live.press_charge();
            }
            Button::Autocalc => self.confirm = live.press_autocalc(),
        }
        self.redraw = true;
        Transition::Stay
    }

    fn answer_confirm(&mut self, ctx: &mut Ctx, yes: bool) -> Transition {
        self.confirm = None;
        self.redraw = true;
        if !yes {
            return Transition::Stay;
        }
        if let Some(live) = BattlefieldScreen::live(ctx) {
            live.confirm_autocalc();
        }
        self.settle(ctx)
    }

    fn settle(&mut self, ctx: &mut Ctx) -> Transition {
        match turn::finish_battle(ctx.game) {
            TurnStep::Report(_) => Transition::Replace(ScreenId::BattleResult),
            TurnStep::Ask(_) => Transition::Replace(ScreenId::BattlePrompt),
            TurnStep::Running | TurnStep::Done(_) | TurnStep::Stuck => Transition::Pop,
        }
    }

    /// `Screen_FrameInput`'s **epilogue**, which runs after every arm on every
    /// screen but `0x12`: `if ((left || right) && FUN_004323FE())`, and
    /// `FUN_004323FE` is `BattleMap_Click` while `g_battlePhase == 2`. So the
    /// overview panel is live on the field, during a drag, and under the outcome
    /// banner alike.
    fn epilogue(ctx: &mut Ctx, x: i32, y: i32, right: bool) -> bool {
        match BattlefieldScreen::live(ctx) {
            Some(live) => live.click_overview(x, y, right),
            None => false,
        }
    }
}

impl Default for BattlefieldScreen {
    fn default() -> Self {
        BattlefieldScreen::new()
    }
}

/// `L2.eng` group 10's three battle prompts, for an install without it.
fn ours_confirm(prompt: usize) -> &'static str {
    match prompt {
        9 => "AUTOCALC BATTLE?",
        11 => "SURRENDER CASTLE?",
        _ => "RETREAT FROM FIELD?",
    }
}

/// **`Battle_SelectOutcomeBanner` (`0x00478419`)** — which of group 82's pairs,
/// and so which row of [`crate::movie::BATTLE_FILMS`].
fn outcome_banner(game: &crate::Game, live: &LiveBattle) -> usize {
    if live.choice_owner == 0 {
        return 6;
    }
    let Some(c) = live.conclusion else { return 1 };
    let attacker_won = c.winner == l2_sim::SIDE_B;
    let winner = if attacker_won { live.attacker } else { live.defender };
    let mine = game.kingdom.campaign.units.get(winner).is_some_and(|u| u.owner == game.player);
    match (live.is_siege(), mine, attacker_won) {
        (false, true, _) => 0,
        (false, false, _) => 1,
        (true, true, true) => 2,
        (true, true, false) => 4,
        (true, false, true) => 5,
        (true, false, false) => 3,
    }
}

