#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::hits::*;
use super::items::*;
use super::super::*;
use super::super::*;
use super::super::screen::*;
use super::super::input::*;
use super::super::*;
use super::super::helpers::*;
use super::super::constants::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};
use crate::screens::setup::skirmish::ROWS_SHOWN;

impl SetupScreen {
    /// The original's own order is `FUN_004335F0`'s hotspot-2 arm:
    ///
    /// ```text
    /// if (humanPlayers <= g_playerStartCount) {
    ///     Setup_CommitOptions();      /* 0x00499DC3 - the twelve into the eleven */
    ///     Setup_StartGame();          /* 0x004329EC - which calls Game_NewGame  */
    /// }
    /// ```
    ///
    /// 1. [`crate::scenario::new_game`] — `Map_InitScenario` and
    ///    `County_Reset`, which is the world;
    /// 2. [`crate::setup::Settings::apply_to`] — `FUN_0049BD99`'s option half:
    ///
/// (`docs/decisions.md` C21)
    /// different map than the one they chose.
    pub(super) fn start(&mut self, ctx: &mut Ctx) -> Transition {
        // One person, in this build. `DAT_00553F98` is the lobby's count and
        if !self.map_read {
            self.read_map(ctx);
        }
        if HUMAN_PLAYERS > self.player_starts {
            return Transition::Stay;
        }
        if self.network {
            return self.open_lobby(ctx);
        }
        let settings = self.options.commit(HUMAN_PLAYERS, ctx.game.kingdom.options.quirks);
        self.unhonoured = settings.unhonoured();
        let slot = self.map;
        self.new_game(ctx, slot, settings, None)
    }

    /// ***Continue*, at the bottom of page 4 — `FUN_00433155`'s hotspot-2
    /// arm.**
    ///
    /// ```text
    /// else if (g_uiHotspotId == 2 && (g_multiplayer == 0 || DAT_0057C940 != 0)) {
    ///     if (DAT_0057D320 == 1) {        /* a campaign */
    ///         Campaign_LoadEntry();       /* 0x00499E5D — the row, not the list */
    ///         Setup_StartGame();          /* 0x004329EC */
    ///     } else {                        /* a custom game or a skirmish */
    ///         ...g_setupPage = 7 / 8 / 0xB / 0xC...
    ///     }
    /// }
    /// ```
    ///
/// `FUN_004335F0`'s
    /// `humanPlayers <= g_playerStartCount` test guards the *custom* Start and
    /// this arm has none, which is right: a campaign row's map and lord count
    /// come from the same table and cannot disagree.
    ///
    /// arm: 0x00433155/continue left-press
    pub(super) fn continue_pressed(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.campaign {
            // FUN_00433155 0x00433155 :794-808, single-player arms: DAT_0055302C == 3 -> 0xC, else 7.
            return self.go(if self.via_skirmish { SetupPage::Skirmish } else { SetupPage::Custom });
        }
        self.start_campaign(ctx)
    }

    /// **`Campaign_LoadEntry` (`0x00499E5D`) and then `Setup_StartGame`.**
    ///
    /// The map a campaign starts on is **not** the map list's slot and not
    /// slot 0: it is column `+0x00` of row `g_campaignMap` of the track's table,
    /// which for the original campaign's first map is slot **17,
    /// Quaintville** — four counties against one lord. `crate::victory` holds
    /// the table and [`crate::victory::CampaignMap::settings`] is the rest of
    /// `Campaign_LoadEntry`.
    fn start_campaign(&mut self, ctx: &mut Ctx) -> Transition {
        self.start_campaign_row(ctx, crate::victory::Campaign::new(self.track))
    }

    /// Setup_StartGame 0x004329EC on `campaign`'s row; screen 0x1C's arm reaches it too.
    pub(crate) fn start_campaign_row(
        &mut self,
        ctx: &mut Ctx,
        campaign: crate::victory::Campaign,
    ) -> Transition {
        let Some(row) = campaign.current() else {
            self.failure = Some("the campaign has no first map".into());
            return Transition::Stay;
        };
        let settings = row.settings(ctx.game.kingdom.options.quirks);
        self.unhonoured = settings.unhonoured();
        self.map = row.scenario;
        self.map_read = false;
        self.new_game(ctx, row.scenario, settings, Some(campaign))
    }

    fn new_game(
        &mut self,
        ctx: &mut Ctx,
        slot: usize,
        settings: crate::setup::Settings,
        campaign: Option<crate::victory::Campaign>,
    ) -> Transition {
        let tables = ctx.game.kingdom.tables;
        match crate::scenario::new_game(
            ctx.assets,
            slot,
            &settings,
            HUMAN_PLAYERS,
            self.shield(),
            // `Game_NewGame` re-deals the start table every game
            // (`PlayerStart_Shuffle`, `0x00497E65`); one fixed seed handed the
            // human the same county forever — on slot 17 the eastern one, the
            // deal's one-in-four arm. `crate::scenario::new_game_seed`.
            crate::scenario::new_game_seed(ctx.assets.wall_clock, None),
            tables,
        ) {
            Ok(game) => {
                // `Game_NewGame` does not call `FUN_00476A5D`: a tip seen in
                // the last game stays seen. `crate::tip`.
                let tips = ctx.game.tips;
                // g_options is a global `Game_NewGame` does not touch:
                // `g_optTipScreens` has two writers in the image (`Options_SetDefaults`
                // 004a0000.c:6090, `Opt_ToggleTipScreens` `0x00434787` 00430000.c:1716), neither of
                // them here (`crate::prefs`).
                let prefs = ctx.game.prefs;
                let before = ctx.game.chosen_gold;
                let markers = crate::setup::battle_process(ctx.game);
                *ctx.game = game;
                crate::setup::carry_battle_markers(ctx.game, markers);
                ctx.game.tips = tips;
                ctx.game.prefs = prefs;
                // FUN_0049C995 0x0049C995 :5760-5763.
                crate::setup::carry_chosen_gold(ctx.game, before, settings.gold);
                self.failure = None;
            }
            Err(e) => {
                self.failure = Some(e.to_string());
                return Transition::Stay;
            }
        }
        // DAT_0055302C: 1 from Setup_ChooseCampaign 0x00433461, else 2 (FUN_00432CC8 :646). [V]
        ctx.game.game_type = if campaign.is_some() {
            crate::victory::GameType::Campaign
        } else {
            crate::victory::GameType::Custom
        };
        if let Some(c) = campaign {
            ctx.game.campaign = c;
        }
        settings.apply_to(ctx.game);
        self.name_the_lords(ctx);
        ctx.game.last_report = Some(ctx.game.kingdom.start_new_game());
        // **`Game_NewGame`'s own `Save_RotateAndWrite()` (`0x00497E2B`)**, the
        // second of that function's only two call sites — the other is the turn
        // boundary. It is why a played install's `lastturn.sav` reads turn 1,
        // Winter 1268 after a new game and before any End Turn.
        self.autosave = true;
        Transition::Push(ScreenId::Campaign)
    }

    /// * the local player's is `Player_SetHuman` (`0x0049BAE9`), which is
    ///   `g_realms[p].isHuman = 1; g_playerNames[p] = g_options; …` — the
    ///   thirty-one bytes of the settings block's name field, copied by
    ///   `FUN_00401136(0x53F1E0, &g_playerNames + p * 0x2C, 0x1F)`;
    /// * every other realm's is `Eng_Seek(7, realm.lord)` and **sixteen** bytes
    ///   copied. `L2.eng` group 7 is *"The Knight, The Baron, The Countess, The
    /// Bishop"* and index 4 is *"No player"*, and the index is the **lord**,
    ///   not the realm and not the colour — `docs/diplomacy.md` §0.1.
    ///
    /// **Sixteen, not thirty-one, for the AI half** — the copy width really is
    /// different between the two paths, and none of group 7's four titles is
    /// long enough for it to show.
    fn name_the_lords(&self, ctx: &mut Ctx) {
        let local = ctx.game.player as usize;
        for realm in 0..l2_kingdom::realm::MAX_REALMS {
            let name = if realm == local {
                // arm: 0x0049BAE9/name-to-playernames left-release
                self.name.commit(text::PLAYER_NAME_LEN)
            } else {
                let lord = ctx.game.kingdom.realms[realm].lord as usize;
                let title = ctx.assets.shell.text(LORD_TITLE_GROUP, lord.min(4));
                title.chars().take(0x10).collect()
            };
            ctx.game.player_names[realm] = text::PlayerName::new(&name);
        }
    }
}



