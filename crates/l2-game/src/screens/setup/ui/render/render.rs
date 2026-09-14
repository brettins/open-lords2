#![allow(unused_imports)]
use super::*;

use super::*;
use super::screen::*;
use super::input::*;
use super::*;
use super::helpers::*;
use super::constants::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

impl SetupScreen {
    pub fn new(page: SetupPage) -> SetupScreen {
        SetupScreen {
            page,
            selected: 0,
            under: SetupPage::Custom,
            open: 0,
            options: SetupOptions::new(),
            shield: 0,
            map_top: 0,
            map: 0,
            player_starts: 5,
            map_read: false,
            unhonoured: Vec::new(),
            failure: None,
            campaign: false,
            autosave: false,
            track: crate::victory::Track::First,
            name: begin_name(text::DEFAULT_PLAYER_NAME),
            saved_name: text::DEFAULT_PLAYER_NAME.to_string(),
            clock_minute: None,
            clock_redraw: false,
        }
    }

    /// What the name field holds, for a test or a caller that wants to know
    /// what *Start* would name the lord.
    pub fn name(&self) -> String {
        self.name.commit(text::PLAYER_NAME_LEN)
    }

    /// The field itself, for a test that wants to look at the caret.
    pub fn name_field(&self) -> &text::TextField {
        &self.name
    }

    /// The twelve selections, for a test or a caller that wants to know what
    /// the screen would start.
    pub fn options(&self) -> &SetupOptions {
        &self.options
    }

    /// The map slot the list has selected — `g_scenarioIndex`.
    pub fn map(&self) -> usize {
        self.map
    }

    /// **The colour page 4 has picked, as the game numbers them** — 1 red,
    /// 2 yellow, 3 black, 4 magenta, 5 blue.
    ///
    /// [`SetupScreen::shield`] is one-based because that is what
    /// `g_realms[p].shieldIndex` and `g_playerNames + 0x25` hold and what
    /// `g_realmColour` and `g_lordChoice` are indexed by; the field behind it
    /// is zero-based because it is also a frame-pair index into `panels2.pl8`.
    /// [`SHIELD_OF_HOTSPOT`] is the original's own table for the conversion and
    /// it is the identity, so the two numberings differ by exactly one.
    pub fn shield(&self) -> u8 {
        SHIELD_OF_HOTSPOT[(self.shield + 1).min(5)]
    }

    /// How many lords the selected map seats.
    pub fn player_starts(&self) -> usize {
        self.player_starts
    }

    /// `Map_LoadPlanes`'s side effect on the option block: read the seat count
    /// off the chosen map and set *Nobles* from it.
    ///
    /// **Both halves, or neither.** A map whose planes cannot be read leaves
/// the seat count alone, because zero
    /// would silently drive the lord count to two.
    fn read_map(&mut self, ctx: &Ctx) {
        self.map_read = true;
        let Some(slot) = ctx.assets.slot(self.map) else { return };
        let seats = slot.player_start_count();
        if seats == 0 {
            return;
        }
        self.player_starts = seats;
        self.options.set_nobles_from_map(seats);
    }

    pub fn page(&self) -> SetupPage {
        self.page
    }

    /// The value of option `i`, as an index into `L2.eng` group 103.
    pub fn option_value(&self, i: usize) -> usize {
        OPTION_BASE[i] + self.options.get(i)
    }

    /// How many rows option `i`'s open list shows.
    ///
    /// Everything but *Nobles* shows its whole run. *Nobles* is shortened to
    /// the map's seat count — `FUN_00433999`: `DAT_00553FB4 =
    /// g_playerStartCount - 1` when the map seats fewer than five — which is
    /// how the original stops a person asking for more lords than the map has
    /// castles for.
    fn rows(&self, i: usize) -> usize {
        if i == crate::setup::option::NOBLES {
            SetupOptions::nobles_rows_for_map(self.player_starts)
        } else {
            OPTION_COUNT[i]
        }
    }

    /// The clickable rectangles of the page, in the order the painter draws
    /// them. Hit-testing and highlighting read the same list, so they cannot
    /// disagree — the failure `menu.rs` avoids by the same means.
    fn hotspots(&self) -> Vec<(Rect, Action)> {
        let mut v = Vec::new();
        match self.page {
            SetupPage::Title => {
                for (i, _) in TITLE_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Options => {
                for (i, _) in OPTION_ITEMS.iter().enumerate() {
                    v.push((item_rect(i), Action::Item(i)));
                }
            }
            SetupPage::Shield => {
                for i in 0..5 {
                    v.push((
                        Rect::new(SHIELD_X + i * SHIELD_STEP, SHIELD_Y, SHIELD_W, SHIELD_H),
                        Action::Item(i as usize),
                    ));
                }
                for (i, (x, y, _)) in SHIELD_BUTTONS.iter().enumerate() {
                    v.push((Rect::new(*x, *y, ITEM_W, ITEM_H), Action::Item(5 + i)));
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                for (i, x) in PAIR_X.iter().enumerate() {
                    v.push((Rect::new(*x, PAIR_Y, PAIR_W, ITEM_H), Action::Item(i)));
                }
            }
            SetupPage::Custom | SetupPage::CustomMulti => {
                for (i, &(x, y, _)) in OPTION_CELLS.iter().enumerate() {
                    v.push((Rect::new(x, y, OPTION_BOX_W, OPTION_BOX_H), Action::Open(i)));
                }
                for row in 0..MAP_LIST_ROWS {
                    let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
                    v.push((
                        Rect::new(MAP_LIST_X, y, MAP_LIST_W, MAP_LIST_ROW),
                        Action::Map(row),
                    ));
                }
                let n = if self.page == SetupPage::Custom { 3 } else { 4 };
                for (i, (x, _)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
                    // [I] The painter centres the caption in 76 pixels at
                    // y = 0xC6 and registers no rectangle of its own; the
                    // hit box is that caption's box, four pixels above it.
                    v.push((
                        Rect::new(*x, CUSTOM_BUTTON_Y - 4, CUSTOM_BUTTON_W, ITEM_H),
                        Action::Item(i),
                    ));
                }
            }
            SetupPage::Dropdown => {
                let n = self.rows(self.open);
                let (x, y, _) = OPTION_LIST[self.open];
                let y = self.dropdown_y(y, n);
                for i in 0..n {
                    v.push((
                        Rect::new(x, y + 16 + i as i32 * 16, OPTION_BOX_W, 16),
                        Action::Choose(i),
                    ));
                }
            }
            SetupPage::NoCd | SetupPage::Load | SetupPage::SkirmishFile => {
                // One way out, and the whole page is it.
                v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti => {
                for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
                    v.push((Rect::new(*x, 0x1B8 - 4, 0x38, ITEM_H), Action::Item(i)));
                }
            }
        }
        v
    }

    /// `FUN_0041FDD6` shifts the *Nobles* drop-down up by one row per item so
    /// that a list opened from the bottom row of the grid still fits on the
    /// screen. It is the only option that gets the treatment.
    ///
    /// **`rows` is the item count, `DAT_00553FB4`** — the same number the
    /// painter loops over, not the geometry table's count-plus-two. That
    /// matters now that the count can be shortened: on a map that seats three
    /// lords the list is two rows and rides two rows lower
    /// original's does, because both read the one variable.
    fn dropdown_y(&self, y: i32, rows: usize) -> i32 {
        if self.open == crate::setup::option::NOBLES {
            y - (rows as i32 - 1) * 16
        } else {
            y
        }
    }

    pub(super) fn at(&self, x: i32, y: i32) -> Option<(usize, Action)> {
        self.hotspots()
            .into_iter()
            .enumerate()
            .find(|(_, (r, _))| r.contains(x, y))
            .map(|(i, (_, a))| (i, a))
    }

    pub(super) fn count(&self) -> usize {
        self.hotspots().len()
    }

    fn activate(&mut self, ctx: &mut Ctx) -> Transition {
        let Some(&(_, action)) = self.hotspots().get(self.selected) else {
            return Transition::Stay;
        };
        self.act(action, ctx)
    }

    /// The page graph, read out of `FUN_00432B05` (page 1) and `FUN_00432CC8`
    /// (page 2).
    ///
    /// **[D], ** Those
    /// handlers branch on `g_uiHotspotId`, and the ids do not run in the order
    /// the painter draws the items: on page 1 id 3 sets the quit flag and id 4
    /// plays `lom.smk`, while the painter draws *"Lords of Magic?"* third and
    /// *"Exit game"* fourth. **The table says why** — `node
    /// tools/oracle/widgets.js widgets 4dcb48 4` gives its third record hotspot
    /// id 4 and its fourth id 3, so the records are in drawing order and the
    /// ids are not, and both readings were right. The
    /// destinations below are keyed to the **captions**, which are [V], not to
    /// the ids.
    fn act(&mut self, action: Action, ctx: &mut Ctx) -> Transition {
        match action {
            Action::Item(i) => self.item(i, ctx),
            Action::Open(i) => {
                self.under = self.page;
                self.open = i;
                self.page = SetupPage::Dropdown;
                self.selected = self.options.get(i);
                Transition::Stay
            }
            Action::Choose(v) => {
                // `FUN_00433A23`: `Setup_SetOption(open, row - 1)`, then back to
                // the page underneath.
                self.options.set(self.open, v);
                self.page = self.under;
                self.selected = 0;
                Transition::Stay
            }
            Action::Map(row) => {
                // `FUN_00433905`: the row sets `g_scenarioIndex`, the planes are
                // loaded, and the seat count that comes out of them sets
                // *Nobles*. All three, or the lord count is left claiming a
                // number the new map cannot seat.
                self.map = (self.map_top + row).min(MAP_COUNT - 1);
                self.read_map(ctx);
                Transition::Stay
            }
        }
    }

    fn item(&mut self, i: usize, ctx: &mut Ctx) -> Transition {
        match (self.page, i) {
            // Page 1. "Single player" opens page 2; "Multiple players" opens
            // page 4 (or page 10 with no disc); "Lords of Magic?" plays an
            // advertisement we have no player for; "Exit game" quits.
            (SetupPage::Title, 0) => self.go(SetupPage::Options),
            (SetupPage::Title, 1) => {
                // `FUN_00432B05` opens with `DAT_0057D320 = 0` before any of
                // its four arms: arriving at page 4 from the title menu is
                // **not** a campaign, whatever the last visit set.
                //
                // arm: 0x00432B05/multiplayer-clears-campaign left-release
                self.campaign = false;
                self.go(SetupPage::Shield)
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
            //
            // Sierra's trailer for its 1997 game,
            // the largest file in the install.
            (SetupPage::Title, 2) => {
                Transition::Push(ScreenId::Movie(crate::movie::Film::LordsOfMagic))
            }
            (SetupPage::Title, 3) => Transition::Quit,
            // Page 2.
            (SetupPage::Options, 0) => self.go(SetupPage::Campaign),
            (SetupPage::Options, 1) => self.go(SetupPage::Load),
            (SetupPage::Options, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Options, 3) => self.go(SetupPage::Custom),
            (SetupPage::Options, 4) => self.go(SetupPage::Title),
            // Page 4: five shields, then "Back" and "Continue".
            //
            // **`FUN_00432EE6` (`0x00432EE6`), and it does two things.**
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
// The re-deal is not run here either.
            // but a shape: `assign_lords` is a pure function of the slot, the
            // lord count and this choice, so running it per click and running
            // it once at world construction give the same world. Nothing on
            // page 4 draws a lord.
            //
            // arm: 0x00432EE6/pick-shield left-press
            (SetupPage::Shield, 0..=4) => {
                self.shield = i;
                Transition::Stay
            }
            (SetupPage::Shield, 5) => self.go(SetupPage::Title),
            (SetupPage::Shield, 6) => self.continue_pressed(ctx),
            // Page 5: either campaign. Page 6: full game or skirmish.
            //
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
                self.go(SetupPage::Shield)
            }
            (SetupPage::GameType, 0) => self.go(SetupPage::Shield),
            (SetupPage::GameType, 1) => self.go(SetupPage::Skirmish),
            // Pages 7 and 8: "Cancel", "Start", "Defaults", "Load".
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
            // Pages 11 and 12: "Back", "Cust."/"Norm.", "Go".
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 0) => self.go(SetupPage::Options),
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Skirmish | SetupPage::SkirmishMulti, _) => Transition::Stay,
            // The three pages with one way out.
            (SetupPage::NoCd, _) => self.go(SetupPage::Title),
            (SetupPage::Load, _) => self.go(SetupPage::Options),
            (SetupPage::SkirmishFile, _) => self.go(SetupPage::Skirmish),
            _ => Transition::Stay,
        }
    }

    /// **Every arrival at page 4 re-seeds the name field**, because every one
    /// of the original's does.
    ///
    /// `FUN_00432B05` (page 1 → 4, *Multiple players*), `FUN_00432CC8` (page 2
    /// → 4, both of its two arms) **and `Setup_ChooseCampaign` (page 5 → 4)**
    /// each set `g_setupPage = 4` and then immediately run
    /// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` and `Edit_RecomputeLength`.
    ///
    /// **Corrected:
    /// one.** It said "arriving from page 5 or 6 does not — those two arms set
    /// the page and nothing else", and `Setup_ChooseCampaign`'s third statement
    /// is that very `Edit_Begin` call. All four writers of `g_setupPage = 4`
    /// re-seed, which is what the code below has always done, so the code was
    /// right and the sentence describing it was not.
    /// `docs/decisions.md` C117.
    fn go(&mut self, page: SetupPage) -> Transition {
        if page == SetupPage::Shield && self.page != SetupPage::Shield {
            // arm: 0x00432B05/name-field-open left-release
            self.name = begin_name(&self.saved_name);
        }
        self.page = page;
        self.selected = 0;
        Transition::Stay
    }

    /// ***Start*, **
    ///
    /// The original's own order is `FUN_004335F0`'s hotspot-2 arm:
    ///
    /// ```text
    /// if (humanPlayers <= g_playerStartCount) {
    ///     Setup_CommitOptions();      /* 0x00499DC3 - the twelve into the eleven */
    ///     Setup_StartGame();          /* 0x004329EC - which calls Game_NewGame  */
    /// }
    /// ```
    ///
    /// — and the guard is real: **pressing *Start* on a map that seats fewer
    /// lords than there are people does nothing at all.** No message, no
/// refusal; the button is inert. Reproduced, because a person who
    /// meets it in the original meets a button that does not work and a
    /// reimplementation that helpfully explained itself would be a different
    /// program. In a single-player game there is one person and every shipped
    /// map seats at least two, so it never fires here.
    ///
    /// # The map is built now
    ///
    /// This section used to say the opposite, and it named exactly what was
    /// missing: *"building a world from a `L2_maps.dat` slot means
    /// `Map_InitScenario` … and none of that exists here"*. It does now —
    /// `l2_scenario::newgame` — so **the slot the list names is the world the
    /// game starts in**. Pick Ireland and you play Ireland.
    ///
    /// The three steps are `Game_NewGame`'s, in its order:
    ///
    /// 1. [`crate::scenario::new_game`] — `Map_InitScenario` and
    ///    `County_Reset`, which is the world;
    /// 2. [`crate::setup::Settings::apply_to`] — `FUN_0049BD99`'s option half:
    /// the stores, the treasury, the armoury, the castle and the lord count;
    /// 3. `Kingdom::start_new_game` — the one immediate `Season_Advance` that
    ///    is why a new game begins in **Winter 1268**.
    ///
    /// **A world that cannot be built is not half-started.** An install with no
    /// `L2_maps.dat`, or a slot that is an empty template, leaves the game
    ///
/// (`docs/decisions.md` C21)
    /// different map than the one they chose.
    fn start(&mut self, ctx: &mut Ctx) -> Transition {
        // One person, in this build. `DAT_00553F98` is the lobby's count and
        //
        if !self.map_read {
            self.read_map(ctx);
        }
        if HUMAN_PLAYERS > self.player_starts {
            return Transition::Stay;
        }
        // **The quirk set is already on the game**, because the quirks page
        // writes it there whether or not a campaign is running - one home for
// the value.
        // See [`crate::screens::options`].
        let settings = self.options.commit(HUMAN_PLAYERS, ctx.game.kingdom.options.quirks);
        self.unhonoured = settings.unhonoured();
        let slot = self.map;
        self.new_game(ctx, slot, settings, None)
    }

    /// ***Continue*, at the bottom of page 4 — `FUN_00433155`'s hotspot-2
    /// arm.**
    ///
    /// This button is not the custom game's *Start* and it was being treated as
    /// though it were. The original's arm branches first:
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
    /// **Two things were wrong with reading it as *Start*.** The campaign limb
    /// never ran, so *Play Now!* started the map list's slot — slot 0, England,
    /// which is the campaign's **fifth** map — and the other limb started a
    /// game at all, where the original walks on to the page that chooses one.
    ///
/// `FUN_004335F0`'s
    /// `humanPlayers <= g_playerStartCount` test guards the *custom* Start and
    /// this arm has none, which is right: a campaign row's map and lord count
    /// come from the same table and cannot disagree.
    ///
    /// arm: 0x00433155/continue left-press
    fn continue_pressed(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.campaign {
            // `DAT_0055302C` picks between page 7/8 and page 11/12 here. This
            // build has one person and no skirmish setup, so the one live
            // destination is the custom page.
            return self.go(SetupPage::Custom);
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
    ///
    /// The counter is `Campaign::new(track)`, which is
    /// [`crate::victory::Track::first_map`] — 0 for the first campaign and
    /// **2** for the second
    ///
    /// **The campaign goes onto the new game, not the old one.** `new_game`
    /// returns a whole fresh [`crate::game::Game`], whose `campaign` field is a
    /// default `Campaign::new(Track::First)`; the track and counter chosen on
    /// page 5 have to be written over it or the second campaign would play the
    /// first campaign's maps from its second win onward. That is the same
    /// three-globals-survive rule `crate::victory`'s header states, arriving at
    /// the one moment the world is replaced.
    fn start_campaign(&mut self, ctx: &mut Ctx) -> Transition {
        let campaign = crate::victory::Campaign::new(self.track);
        let Some(row) = campaign.current() else {
            self.failure = Some("the campaign has no first map".into());
            return Transition::Stay;
        };
        let settings = row.settings(ctx.game.kingdom.options.quirks);
        self.unhonoured = settings.unhonoured();
        // `g_scenarioIndex` is one global, so the list follows the campaign's
// choice.
        self.map = row.scenario;
        self.map_read = false;
        self.new_game(ctx, row.scenario, settings, Some(campaign))
    }

    /// `Setup_StartGame` → `Game_NewGame`, shared by both of page 4's and
    /// page 7's routes into it, so that neither can drift from the other.
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
            // **Page 4's choice, on both routes into here.** A campaign row
            // rewrites every one of the twelve options and says nothing about
            // the colour, which is right: page 4 is the page a campaign passes
            // *through*, so the shield the person picked there survives
            // `Campaign_LoadEntry`
            self.shield(),
            crate::scenario::SEED,
            tables,
        ) {
            Ok(game) => {
                // `Game_NewGame` does not call `FUN_00476A5D`: a tip seen in
                // the last game stays seen. `crate::tip`.
                let tips = ctx.game.tips;
                *ctx.game = game;
                ctx.game.tips = tips;
                self.failure = None;
            }
            Err(e) => {
                self.failure = Some(e.to_string());
                return Transition::Stay;
            }
        }
        if let Some(c) = campaign {
            ctx.game.campaign = c;
        }
        settings.apply_to(ctx.game);
        self.name_the_lords(ctx);
        // `Game_NewGame`'s last economic call. Everything above is the position
        // the original hands to it.
        ctx.game.last_report = Some(ctx.game.kingdom.start_new_game());
        // **`Game_NewGame`'s own `Save_RotateAndWrite()` (`0x00497E2B`)**, the
        // second of that function's only two call sites — the other is the turn
        // boundary. It is why a played install's `lastturn.sav` reads turn 1,
        // Winter 1268 after a new game and before any End Turn.
        // See [`crate::screen::Screen::take_autosave`].
        self.autosave = true;
        Transition::Push(ScreenId::Campaign)
    }

    /// **Fill `g_playerNames`** — the one place the typed name stops being a
    /// keystroke and becomes part of the game.
    ///
    /// Two sources, and both are the original's:
    ///
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


