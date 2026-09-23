//! **The skirmish masters** — `Screen_BattleMasterRank` (`0x00421D09`),
//! `g_screenId` `0x2F`, drawn by `Screen_Draw`'s `g_screenId == '/'` arm
//! (`00400000.c:8496`).
//!
//! The painter is transcribed in the module header of [`super`]. What reaches
//! it and what leaves it:
//!
//! ```text
//! Screen_FrameInput (0x0042FF10), 00420000.c:7500-7519:
//!   if (g_screenId == '.') {                       0x2E, the ratings sheet
//!     if (g_mouseLeftPressed)  { g_screenId = '/'; g_redrawRequest = 2; }
//!     if (g_mouseRightPressed) { g_screenId = '/'; g_redrawRequest = 2; }
//!   }
//!   else if (g_screenId == '/') {                  0x2F, this sheet
//!     if (g_mouseRightPressed) { Music_Play("setup3.wav",0,1); FUN_0042B780(); }
//!     if (g_mouseLeftPressed)  { Music_Play("setup3.wav",0,1); FUN_0042B780(); }
//!   }
//! FUN_0042B780 (0x0042B780, 00420000.c:4519-4536):
//!   FUN_004360F2(); FUN_0043F24B();
//!   g_screenId = 0x1F; g_setupPage = g_multiplayer ? 0xB : 0xC;
//!   Skirmish_Setup(); DAT_005533E0[g_localPlayer] = 1; Net_SendCommand(0x55,0);
//!   FUN_0042C5FE(); Skirmish_FillArmies(); g_redrawRequest = 1;
//! ```
//!
//! So the chain is `0x2E` → `0x2F` → page 12, on **either** button, pressed,
//! anywhere; neither sheet has a back door and neither tests the keyboard.
//! `Music_Play("setup3.wav", 0, 1)` is `docs/audio.json`'s two `0x0042FF10`
//! records and is **not** wired here: our bed is picked by
//! `audio::director::scene` from the screen stack, which has no restart edge.
use super::*;

/// `L2.eng` group 38 — the twelve ranks `Ui_DrawCentred(0x26, DAT_0053E9E0, …)`
/// picks from.
pub const GROUP_RANK: usize = 38;
/// `Ui_DrawCentred(0x25, 5, …)` — *"The skirmish masters!!"*.
pub const MASTERS: usize = 5;

pub const RANK_BACKGROUND: &str = "Score2.pl8";
pub const RANK_PALETTE: &str = "Score2.256";

/// `Ui_DrawRectOutline(0x82, 10, 0x17C, 0x10E, 0x3F)` and
/// `Ui_DrawRectOutline(0x82, 300, 0x17C, 0xAA, 0x3F)`.
pub const TOP_BOX: (i32, i32, i32, i32) = (0x82, 10, 0x17C, 0x10E);
pub const MINE_BOX: (i32, i32, i32, i32) = (0x82, 300, 0x17C, 0xAA);

/// The two `Ui_DrawCentred(…, 0, y, 0x280, &g_fontHeading, 0x3F)` — centred
/// across the whole 640, not inside either box.
pub const MASTERS_Y: i32 = 0x16;
pub const RANK_Y: i32 = 0x138;
pub const HEADING_W: i32 = 0x280;

/// `row * 0x14 + 0x46` and `row * 0x14 + 0x168`, name at x 200 and number at
/// x 400.
pub const ROW_PITCH: i32 = 0x14;
pub const TOP_Y0: i32 = 0x46;
pub const MINE_Y0: i32 = 0x168;
pub const NAME_X: i32 = 200;
pub const SCORE_X: i32 = 400;

pub struct RankScreen;

impl RankScreen {
    pub fn new() -> RankScreen {
        RankScreen
    }
}

impl Default for RankScreen {
    fn default() -> RankScreen {
        RankScreen::new()
    }
}

impl Screen for RankScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Rank
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The skirmish masters — screen 0x2F".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(RANK_PALETTE)
    }

    /// `FUN_0042D179` runs before any sheet opens; with no `score.dat`
    /// (`FUN_0042D216`) its table is the whole book.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.game.battle_master.seed_defaults(&ctx.assets.shell);
        Transition::Stay
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        match event {
            // arm: 0x0042FF10/rank-any-press left-press
            Event::Click { .. } | Event::RightClick { .. } => {
                Transition::Replace(ScreenId::Setup(crate::screens::setup::SetupPage::Skirmish))
            }
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        // `DAT_005AEA40 = 1` is up across the whole painter and `DAT_0058FE2C`
        // is raised only over the two headings.
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: None,
            caps: None,
        };
        let head = pen.drop_caps();
        if !crate::shell::background(canvas, a, RANK_BACKGROUND) {
            canvas.clear(ink.background);
        }

        let book = &ctx.game.battle_master;
        pen.outline(canvas, TOP_BOX.0, TOP_BOX.1, TOP_BOX.2, TOP_BOX.3, font::TEXT);
        head.eng_heading_centred(canvas, GROUP, MASTERS, 0, MASTERS_Y, HEADING_W, font::TEXT);
        for row in 0..TOP {
            let y = row as i32 * ROW_PITCH + TOP_Y0;
            let Some(e) = book.top.get(row) else { break };
            pen.body(canvas, NAME_X, y, &e.name, font::TEXT);
            pen.number_in(Face::Body, canvas, SCORE_X, y, e.score, ' ', " ", font::TEXT);
        }

        pen.outline(canvas, MINE_BOX.0, MINE_BOX.1, MINE_BOX.2, MINE_BOX.3, font::TEXT);
        head.eng_heading_centred(
            canvas,
            GROUP_RANK,
            book.rank_index(),
            0,
            RANK_Y,
            HEADING_W,
            font::TEXT,
        );
        // The five rows all carry `&g_playerNames + g_localPlayer * 0x2C` —
        // one name repeated, not five.
        // `00420000.c:635`: drawn as is, empty or not.
        let name = ctx.game.player_names.get(ctx.game.player as usize).map_or(String::new(), |n| n.as_str());
        for row in 0..MINE_SHOWN {
            let y = row as i32 * ROW_PITCH + MINE_Y0;
            pen.body(canvas, NAME_X, y, &name, font::TEXT);
            pen.number_in(Face::Body, canvas, SCORE_X, y, book.mine[row], ' ', " ", font::TEXT);
        }

        if ctx.game.prefs.debug_overlay {
            l2_view::text::draw(
                canvas,
                4,
                470,
                "SCORE.DAT IS NOT READ OR WRITTEN: THIS BOOK LASTS ONE RUN",
                ink.dim,
            );
        }
    }
}
