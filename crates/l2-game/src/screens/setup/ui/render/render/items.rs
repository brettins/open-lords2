#![allow(unused_imports)]
use super::*;
use super::layout::*;
use super::hits::*;
use super::launch::*;
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
    pub(super) fn item(&mut self, i: usize, ctx: &mut Ctx) -> Transition {
        match (self.page, i) {
            (SetupPage::Title, 0) => {
                self.network = false;
                self.go(SetupPage::Options)
            }
            (SetupPage::Title, 1) => {
                // `FUN_00432B05` opens with `DAT_0057D320 = 0` before any of
                // its four arms: arriving at page 4 from the title menu is
                // **not** a campaign, whatever the last visit set.
                //
                // arm: 0x00432B05/multiplayer-clears-campaign left-release
                self.campaign = false;
                // [D] C255: the lobby's entry page first; Host a game opens page 4 as `SetupScreen::new(Shield)`.
                Transition::Push(ScreenId::Lobby)
            }
            // `FUN_00432B05`'s hotspot 4 — *"Lords of Magic?"* — is the THIRD
            // record of `DAT_004DCB48` and carries hotspot id 4, which is what
            // settles the question the note on `act` used to leave open: the
            // ids are not the drawing order, the records are. It is also the
            // one **kind 3** record of the four (`node tools/oracle/widgets.js
            // widgets 4dcb48 4`), so it fires on the release — see `handle`.
            //
            // ```c
            // Music_Stop(0); g_mouseLeftReleased = 0; FUN_004B1897(); FUN_004B11CE();
            // Smk_Play("lom.smk", 0x46, 0x50, 0, g_screenId);  g_redrawRequest = 2;
            // ```
            (SetupPage::Title, 2) => {
                Transition::Push(ScreenId::Movie(crate::movie::Film::LordsOfMagic))
            }
            (SetupPage::Title, 3) => Transition::Quit,
            // arm: 0x00432CC8/campaign left-press
            (SetupPage::Options, 0) => self.go(SetupPage::Campaign),
            // arm: 0x00432CC8/load left-press
            (SetupPage::Options, 1) => self.go(SetupPage::Load),
            // FUN_00432CC8 0x00432CC8 hotspot 5 (00430000.c:650-659): page 4 with
            // `DAT_0055302c = 3`; FUN_00433155 :794 sends its Continue to 0xC. [V]
            // arm: 0x00432CC8/skirmish left-press
            (SetupPage::Options, 2) => {
                (self.campaign, self.via_skirmish) = (false, true);
                self.go(SetupPage::Shield)
            }
            // FUN_00432CC8 0x00432CC8 hotspot 3: `g_setupPage = 4; DAT_0055302c = 2;
            // DAT_0057d320 = 0;` — page 4 first; its Continue (FUN_00433155) goes to 7. [V]
            // arm: 0x00432CC8/custom left-press
            (SetupPage::Options, 3) => {
                (self.campaign, self.via_skirmish) = (false, false);
                self.go(SetupPage::Shield)
            }
            // arm: 0x00432CC8/back left-press
            (SetupPage::Options, 4) => self.go(SetupPage::Title),
            // **`FUN_00432EE6` (`0x00432EE6`), and it does two things.**
            //
            // `FUN_00432FAB(g_uiHotspotId)` claims the colour and then
            // `Realms_AssignLords()` runs **immediately**, on every click —
            // the original re-deals the AI colours and lords while the page is
// still up.
            //
            // ```c
            // if ((&DAT_0057cb40)[hotspot] == '\0') {          /* free? */
            //     for (i = 1; i < 6; i++)                      /* let mine go */
            //         if ((&DAT_0057cb40)[i] == g_localPlayer) (&DAT_0057cb40)[i] = 0;
            //     (&DAT_0057cb40)[hotspot] = g_localPlayer;    /* claim it */
            //     (&DAT_00553d75)[g_localPlayer * 0x2c] = (&DAT_004d5548)[hotspot * 4];
            //     g_realms[g_localPlayer].shieldIndex = (&DAT_00553d75)[...];
            // }
            // ```
            //
            // **The claim table is the multiplayer half and is not reproduced**
            // — `DAT_0057CB40` exists so that two people cannot both be blue,
            // and with one person the only occupied entry is his own. Its one
            // single-player consequence is that clicking the colour you already
            // hold does nothing, and an assignment to the value it already has
            // is that, exactly.
            //
            // arm: 0x00432EE6/pick-shield left-press
            (SetupPage::Shield, 0..=4) => {
                self.shield = i;
                Transition::Stay
            }
            (SetupPage::Shield, 5) => self.go(SetupPage::Title),
            (SetupPage::Shield, 6) => self.continue_pressed(ctx),
            // **`Setup_ChooseCampaign` (`0x00433461`)**, and it does three
            // things this used to do none of: it stores the hotspot in
            // `g_campaignTrack`, it starts `g_campaignMap` at that track's first
            // row — `Campaign::new` — and it raises `DAT_0057D320` so that
            // *Continue* on page 4 loads a campaign row instead of the map
            // list's slot. The counter is not kept here; it is
            // [`crate::victory::Track::first_map`], which is where it was
            // already read out of this function.
            //
            // arm: 0x00433461/choose-campaign left-press
            (SetupPage::Campaign, i) => {
                self.track = if i == 1 {
                    crate::victory::Track::Second
                } else {
                    crate::victory::Track::First
                };
                self.campaign = true;
                // Setup_ChooseCampaign 0x00433461, 00430000.c:892-897: both globals, before any game.
                ctx.game.prefs.kept.remember(&crate::victory::Campaign::new(self.track));
                self.go(SetupPage::Shield)
            }
            (SetupPage::GameType, 0) => self.go(SetupPage::Shield),
            (SetupPage::GameType, 1) => self.go(SetupPage::Skirmish),
            (SetupPage::Custom | SetupPage::CustomMulti, 0) => self.go(SetupPage::Options),
            (SetupPage::Custom | SetupPage::CustomMulti, 1) => self.start(ctx),
            // *Defaults*: `Setup_DefaultOptions` (`0x004AE539`) and then
            // `FUN_004AE5E2(g_playerStartCount)`, which is the click handler's
            // own order — the twelve go back to the game's defaults and the map
            // then overrules *Nobles* again. **It is not twelve zeroes**, which
            // is what this used to write: six of the twelve defaults are not 0,
            // so the button was resetting to a game the original never offers.
            (SetupPage::Custom | SetupPage::CustomMulti, 2) => {
                self.options = SetupOptions::new();
                self.options.set_nobles_from_map(self.player_starts);
                self.unhonoured.clear();
                Transition::Stay
            }
            (SetupPage::Custom | SetupPage::CustomMulti, _) => self.go(SetupPage::Load),
            // ***Back* is `FUN_0043D649` and it goes to page 1**, not page 2:
            //
            // `g_setupPage = 1`, `DAT_0057A0F0 = 0` — the skirmish flag down
            // again — `g_battlePhase = 0` and the campaign's button set back.
            //
            // arm: 0x0043D649/skirmish-back left-press
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 0) => self.go(SetupPage::Title),
            // ***Cust.*** — `FUN_0043DA9E`. The flag flips and both handicaps
            // go back to 2; the two custom musters it draws instead
            // (`FUN_0042130F`, `FUN_00420DE4`) are not built.
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 1) => {
                self.skirmish.toggle_custom();
                Transition::Stay
            }
            (SetupPage::Skirmish, 2) => self.go_skirmish(ctx),
            (SetupPage::SkirmishMulti, _) => Transition::Stay,
            (SetupPage::Skirmish, _) => Transition::Stay,
            (SetupPage::NoCd, _) => self.go(SetupPage::Title),
            (SetupPage::Load, _) => self.go(SetupPage::Options),
            (SetupPage::SkirmishFile, _) => self.go(SetupPage::Skirmish),
            _ => Transition::Stay,
        }
    }

    /// `FUN_00432B05` (page 1 → 4, *Multiple players*), `FUN_00432CC8` (page 2
    /// → 4, both of its two arms) **and `Setup_ChooseCampaign` (page 5 → 4)**
    /// each set `g_setupPage = 4` and then immediately run
    /// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` and `Edit_RecomputeLength`.
    ///
    /// `docs/decisions.md` C117.
    pub(crate) fn go(&mut self, page: SetupPage) -> Transition {
        // [D] C255: the host path's Back and Escape return to the lobby's entry page, not page 1.
        if page == SetupPage::Title && self.network {
            return Transition::Replace(ScreenId::Lobby);
        }
        if page == SetupPage::Shield && self.page != SetupPage::Shield {
            // arm: 0x00432B05/name-field-open left-release
            self.name = begin_name(&self.saved_name);
        }
        if page == SetupPage::Load && self.page != SetupPage::Load {
            // FUN_004335F0 0x004335F0 00430000.c:975, FUN_00432CC8 :632 `DAT_00553e5c = g_setupPage`. [V]
            self.load_opener = self.page;
        }
        self.page = page;
        self.selected = 0;
        Transition::Stay
    }

}
