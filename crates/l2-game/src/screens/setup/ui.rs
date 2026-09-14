#![allow(unused_imports)]
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

    fn at(&self, x: i32, y: i32) -> Option<(usize, Action)> {
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

impl Screen for SetupScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Setup(self.page)
    }

    fn title(&self, ctx: &Ctx) -> String {
        let name = ctx.assets.shell.text(GROUP, 0);
        if name.is_empty() {
            format!("Lords of the Realm II — setup page {}", self.page.number())
        } else {
            format!("{name} — setup page {}", self.page.number())
        }
    }

    fn palette(&self) -> Option<&'static str> {
        Some(self.page.palette())
    }

    /// `Game_NewGame`'s `Save_RotateAndWrite()`. Raised by
/// [`SetupScreen::new_game`] once a world has been built.
    fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    /// `Map_LoadPlanes`'s effect on the option block, once, on the first tick.
    ///
    /// The original loads the planes the moment the custom page is opened and
    /// again on every change of scenario, and `g_playerStartCount` falls out of
    /// that load. The constructor has no [`Ctx`] and so no `L2_maps.dat`, so
    /// the first read waits for the first tick — which is also what makes a
    /// screen built in a test with no install work: it never gets a slot, and
    /// the seat count stays at its five.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if !self.map_read {
            self.read_map(ctx);
        }
        // `Edit_DrawCaret` counts its own frames; ours counts ticks, because
        // nothing under the renderer may read a clock. `crate::text`.
        if self.page == SetupPage::Shield {
            self.name.tick();
        }
        // **The clock on page 1 — ours, and this is the only thing that makes
        // it move.** The reading comes in on `Assets`; the minute it falls in
        // is compared with the minute already on the screen, and only a change
        // asks for a repaint. Nothing here reads a clock, and on any page but
        // the title
        if self.page == SetupPage::Title {
            let minute = ctx.assets.wall_clock.map(crate::wallclock::minute);
            if minute != self.clock_minute {
                self.clock_minute = minute;
                self.clock_redraw = true;
            }
        }
        Transition::Stay
    }

    /// The minute turning, and nothing else — see [`SetupScreen::clock_minute`].
    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.clock_redraw)
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let n = self.count().max(1);
        // **The name field gets first refusal on page 4, and only there.**
        //
        // `Screen_HandleInput`'s page-4 arm is the one that sets `g_editActive`
        // (`0x005AEB78`), and that flag is what decides whether a keystroke
        // reaches the buffer at all — so on every other page of the front end
        // the keys below keep the meaning they have here today. On page 4 they
        // do not: `Space` is a space in a name, and `I` is the letter I.
        //
        // The commit is `Edit_Commit(&g_options, 0x1F)`, which the original
// runs **every frame** while the page is up.
        // Doing it per keystroke is the same thing at the only moments the
        // buffer can have changed.
        if self.page == SetupPage::Shield {
            // arm: 0x004BA9C8/setup-name key
            let metrics = text::FontMetrics::of(&ctx.assets.shell);
            if self.name.event(event, &metrics) {
                self.saved_name = self.name.commit(text::PLAYER_NAME_LEN);
                return Transition::Stay;
            }
        }
        // **Everything below this line is ours.** The front end has no keyboard
        // at all in the original: not one of `Screen_HandleInput`'s thirteen
        // `g_setupPage` arms tests a key, and the window procedure has no
        // `g_screenId == 0x1F` case. Its whole interface is `Hotspot_Test` and
// `Widget_Test`. That is recorded — a menu a person
        // cannot drive from the keyboard is worse, not more faithful — and the
        // records are `ours/setup-*` in `docs/arms.json`.
        match event {
            // arm: ours/setup-key-up key
            Event::KeyDown(Key::Up) => self.selected = (self.selected + n - 1) % n,
            // arm: ours/setup-key-down key
            Event::KeyDown(Key::Down) => self.selected = (self.selected + 1) % n,
            // arm: ours/setup-key-activate key
            Event::KeyDown(Key::Enter) | Event::KeyDown(Key::Space) => return self.activate(ctx),
            // Ours: the demo's index of every screen. `screens::index` says
            // why it exists and marks itself as not the game's.
            //
            // arm: ours/setup-key-index key
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            // arm: ours/setup-key-escape key
            Event::KeyDown(Key::Escape) => {
                // Whatever the page is, Escape is its own way back — the
                // original's Back button where there is one, and out of the
                // front end where there is not.
                return match self.page {
                    // Pop, not Quit. Popping the last screen quits anyway —
                    // `Machine::apply` — so this is the right answer both when
                    // the front end is the root and when it was opened from
                    // somewhere else, without the screen having to know which.
                    SetupPage::Title => Transition::Pop,
                    SetupPage::Dropdown => {
                        self.page = self.under;
                        Transition::Stay
                    }
                    _ => self.go(SetupPage::Title),
                };
            }
            Event::Pointer { x, y } => {
                if let Some((i, _)) = self.at(x, y) {
                    self.selected = i;
                }
            }
            Event::Click { x, y } => {
                if let Some((i, action)) = self.at(x, y) {
                    self.selected = i;
                    // The one kind-3 record on the title page fires on the
                    // release, below; its press only selects.
                    if self.page == SetupPage::Title && action == Action::Item(2) {
                        return Transition::Stay;
                    }
                    return self.act(action, ctx);
                }
            }
            // `Hotspot_Test` kind 3: `DAT_004DCB48` record 2, hotspot id 4.
            // arm: 0x00432B05/lords-of-magic left-release
            Event::Release { x, y } if self.page == SetupPage::Title => {
                if let Some((i, action @ Action::Item(2))) = self.at(x, y) {
                    self.selected = i;
                    return self.act(action, ctx);
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        // **[D]** The front end's own text flags. `DAT_005AEA40` is set around
        // every menu item, button caption and body line and cleared for the
        // heading, so on these pages *only the heading is embossed*; and
        // `DAT_0058FE2C` is set around the heading alone, which draws its
        // capitals in colour 1. So there are two pens here, not one: `head`
        // for the heading and `pen` — flat, no drop capitals — for everything
        // else. A reimplementation that embossed the lot would be wrong on
        // every page of the front end at once.
        let head = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW_GATEWAY),
            caps: Some(1),
        };
        let pen = head.flat();
        // Page 9 is drawn over whatever was underneath it, so the background
        // and the page beneath are painted first and only then the open list.
        let base = if self.page == SetupPage::Dropdown { self.under } else { self.page };
        if !shell::background(canvas, a, base.background()) {
            // No install, or a partial one. Say so in our own font — never in
            // the original's — so that an empty page can never be mistaken for
            // a page the game drew empty.
            canvas.clear(ctx.assets.ink.background);
            let line = format!(
                "SETUP PAGE {} - NO {}",
                base.number(),
                base.background().to_uppercase()
            );
            l2_view::text::draw(canvas, 4, 4, &line, ctx.assets.ink.dim);
        }
        self.paint(ctx, canvas, &pen, &head, base);
        if self.page == SetupPage::Dropdown {
            self.paint_dropdown(canvas, &pen, ctx);
        }
    }
}

impl SetupScreen {
    fn colour(&self, index: usize) -> u8 {
        if index == self.selected {
            font::HIGHLIGHT
        } else {
            font::TEXT
        }
    }

    /// A menu item: the recess, then the caption centred in it.
    fn draw_item(
        &self,
        canvas: &mut Canvas,
        pen: &Pen,
        index: usize,
        rect: Rect,
        group: usize,
        s: usize,
    ) {
        shell::button_recess(canvas, rect.x, rect.y, rect.w, rect.h);
        pen.eng_centred(
            canvas,
            group,
            s,
            rect.x,
            rect.y + ITEM_TEXT,
            rect.w,
            self.colour(index),
        );
    }

    fn paint(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        match page {
            SetupPage::Title => {
                pen.window_from(canvas, BOX_SHEET, 0xA0, 10, 0x14, 0xF);
                head.eng_heading_centred(canvas, GROUP, 0, TITLE_X, TITLE_Y, TITLE_W, font::TEXT);
                pen.eng_centred(canvas, GROUP, 1, TITLE_X, SUBTITLE_Y, TITLE_W, font::TEXT);
                for (i, s) in TITLE_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
                // **The one thing on this page that must not be quiet.**
                // Every `Pen` method degrades to the 5 × 7 debug font per call
                // and says nothing, so a checkout that cannot read its fonts
                // draws a complete, correct, illegible front end. The load-time
                // complaint goes to stderr, which a player double-clicking an
                // executable never sees. This is the same sentence on the first
                // screen he does.
                missing_fonts_banner(ctx, canvas);
                // **Ours, **
                // Not the original's — see [`crate::build_id`], which exists
                // because a player spent an evening reporting three defects
                // against a binary four merges old.
                crate::build_id::draw(canvas, pen);
                // **Ours too, and a deliberate divergence: `FUN_0041EA14`
                // draws no clock.** A player asked for the time in MST on this
                // screen. Bottom-right, opposite the build stamp, below
                // everything the original's page-1 painter reaches (its window
                // ends at y 250). Do not "fix" it toward the binary and do not
                // count it as a reproduction — `crate::wallclock` carries the
                // reading of `FUN_0041EA14` that says there is nothing there.
                //
                // The time itself is `ctx.assets.wall_clock`, which only the
                // shell ever fills: no clock is read here or anywhere below it
                // (`docs/netcode.md` D-5)
// on the page.
                if let Some(now) = ctx.assets.wall_clock {
                    crate::wallclock::draw(canvas, pen, now);
                }
            }
            SetupPage::Options => {
                pen.window_from(canvas, BOX_SHEET, 0xB0, 10, 0x12, 0x12);
                head.eng_heading_centred(canvas, GROUP, 5, 0xB0, 0x2D, 0x120, font::TEXT);
                for (i, s) in OPTION_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
            }
            SetupPage::Load => {
                // `FUN_004148E4(5)`, transcribed. The box, the caption, one
                // **recess** and three **outlines** — and the difference
                // between the two was wrong here until the draw-call audit read
                // the painter: `FUN_00403EE4` is the bevelled recess (top and
                // right `0x35`, bottom and left `0x28`) and `FUN_00403CF4` is a
                // flat one-pixel rectangle in a single colour. Only the outer
                // frame is a recess; the name field, the file list and the
                // status line are outlines in `0x3F`.
                pen.window_from(canvas, BOX_SHEET, 0x60, 10, 0x1C, 0x15);
                head.eng_heading_centred(canvas, GROUP_FILE, 5, 0x60, 0x22, 0x1C0, font::TEXT);
                shell::button_recess(canvas, 0x70, 0x42, 400, 0x100);
                for r in LOAD_OUTLINES {
                    outline_rect(canvas, r, font::TEXT);
                }
                // **What used to be here was invented.** The line under the
                // list read `L2.eng` 40/8 *"Right click to exit."*, which the
                // original draws on **page 13** and never here.
                // `SaveLoad_DrawStatus` puts 40/2 *"Loading game. Please
                // wait."* at (128, 292) and only while `DAT_0057D3C4` — a
// frame countdown set to 150 or 400 when a load
                // starts, and zeroed when the box opens — is running. An idle
                // load box has an empty status line, so ours has one too.
                //
                // The rest of `SaveLoad_DrawStatus` is not drawn: the four
                // `Panels2.pl8` plates, the file name being typed with its
                // caret, and up to thirty save names in three columns from
                // (128, 118). [`super::saveload`] is the same function on
                // screens `0x35`/`0x36`; page 3 is that screen inside the front
                // end's window, and joining them is a job on its own.
            }
            SetupPage::Shield => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x10);
                head.eng_heading_centred(canvas, GROUP, 10, 0x50, 0x23, 0x1E0, font::TEXT);
                self.paint_shields(canvas, pen);
                for (i, (x, y, s)) in SHIELD_BUTTONS.iter().enumerate() {
                    self.draw_item(
                        canvas,
                        pen,
                        5 + i,
                        Rect::new(*x, *y, ITEM_W, ITEM_H),
                        GROUP,
                        *s,
                    );
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                pen.window_from(canvas, BOX_SHEET, 0x40, 0x32, 0x20, 8);
                head.eng_heading_centred(canvas, GROUP_EXPANSION, 0, 0x40, 0x50, 0x200, font::TEXT);
                let items: [usize; 2] =
                    if page == SetupPage::Campaign { [4, 5] } else { [1, 2] };
                for i in 0..2 {
                    shell::button_recess(canvas, PAIR_X[i], PAIR_Y, PAIR_W, ITEM_H);
                    pen.eng_centred(
                        canvas,
                        GROUP_EXPANSION,
                        items[i],
                        PAIR_TEXT_X[i],
                        PAIR_Y + 6,
                        PAIR_TEXT_W,
                        self.colour(i),
                    );
                }
            }
            SetupPage::NoCd => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x13);
                head.eng_heading_centred(canvas, GROUP, 0x10, 0x50, 0x24, 0x1E0, font::TEXT);
                // Three wrapped paragraphs at width 0x180 and one plain line.
                // The plain one is drawn in colour 1, not 0x3F — the only
                // string on any of these pages that is.
                let a = pen.assets;
                for (i, y) in [(0x11usize, 0x48), (0x12, 0x78)] {
                    let t = a.text(GROUP, i).to_string();
                    pen.body_wrapped(canvas, 0x80, y, 0x180, &t, font::TEXT);
                }
                pen.eng(canvas, GROUP, 0x30, 0x80, 0xDC, 1);
                let t = a.text(GROUP, 0x31).to_string();
                pen.body_wrapped(canvas, 0x80, 0xF0, 0x180, &t, font::TEXT);
            }
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => {
                self.paint_custom(ctx, canvas, pen, page)
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti | SetupPage::SkirmishFile => {
                self.paint_skirmish(canvas, pen, head, page)
            }
        }
    }

    /// `FUN_0041F1DD` and `FUN_0041F321`: the name field and the five shields.
    ///
    /// **[V]** and worth writing down, because the obvious reading is wrong.
    /// The painter blits from `DAT_004EABEC` — the general scratch buffer,
    /// which on this page holds **`panels2.pl8`** — and *not* from
    /// `g_miscCtySheet`. `Misc_sel.pl8` has seventeen frames; the indices here
    /// run to 215, and `Panels2.pl8` has 216. The file settles it: frames
    /// 205 … 214 are five pairs of roughly 60 × 65 shields, one pair per realm
    /// colour, 204 is a 224 × 32 plate the size of a name field, and 215 is a
    /// 54 × 27 plaque. Nothing else in either file is that shape.
    ///
    /// Frame `2i + 0xCB` is the shield when the colour is free and `2i + 0xCC`
    /// when it is taken, at `x = 0x70 + 88(i - 1)`, `y = 0x8C`; the 54 × 27
    /// plaque marks the chosen one at `(x + 4, 0x70)`.
    fn paint_shields(&self, canvas: &mut Canvas, pen: &Pen) {
        // **The name field, and it now has a name in it.**
        //
        // `FUN_0041F321` is five statements and every one of them is here:
        //
        // ```c
        // g_caretPlaced = 0; g_caretX = 0; g_drawIndex = 0;
        // g_editDrawing = 1; g_penAdvance = 0;
        // Pl8_DrawFrameHere(panels2, 0xCC, 0xD0, 0x48);      /* the plate      */
        // Ui_DrawText(&g_options, 0xD6, 0x50, &g_fontBody, 0x3F);
        // if (!g_caretPlaced) { g_caretX = g_penAdvance; g_caretPlaced = 1; }
        // g_caretX += 0xD6;  g_caretY = 0x52;
        // Edit_DrawCaret(0x5AF8F0, 0x3F);                    /* the caret      */
        // ```
        //
        // The plate is drawn **before** the text and the caret **after** it,
// so the caret is a solid bar
        // eats. `g_caretPlaced` is set by `Ui_DrawText` itself when the drawing
        // index reaches `g_editCaret`, so the caret x is the pen after that
        // many characters and needs nothing from the caller;
        // `TextField::caret_x` computes the same number the same way.
        //
        // **The plate is the same frame whether or not the field is being
// typed into.** the caret
// is the whole of the affordance, so it had to be built
        //
        let sheet = pen.assets.sheet(BOX_SHEET);
        match sheet.and_then(|s| s.frame(0xCC)) {
            Some(f) => canvas.blit(&f, NAME_PLATE_X, NAME_PLATE_Y),
            // No `Panels2.pl8`. A recess of our own, so the field is still a
            // field on a placeholder install and a test can still find it.
            None => shell::button_recess(canvas, NAME_PLATE_X, NAME_PLATE_Y, 0xE0, 0x20),
        }
        pen.body(canvas, NAME_X, NAME_Y, &self.name.text(), font::TEXT);
        // `font::TEXT` is `0x3F`, a palette index; with no font loaded the
        // fallback renderer draws in named interface colours instead, and the
        // caret has to follow the text it belongs to.
        let ink = if pen.assets.body.is_some() { font::TEXT } else { pen.ink.text };
        self.name.draw_caret(canvas, NAME_X, NAME_Y, ink, &text::FontMetrics::of(pen.assets));
        let Some(sheet) = sheet else { return };
        for i in 1..6usize {
            let x = SHIELD_X + (i as i32 - 1) * SHIELD_STEP;
            if i - 1 == self.shield {
                if let Some(f) = sheet.frame(0xD7) {
                    canvas.blit(&f, x + 4, 0x70);
                }
            }
            // Free, not taken: this shell has no lobby, so every colour is
            // offered and the taken variant (`2i + 0xCC`)
            if let Some(f) = sheet.frame(i * 2 + 0xCB) {
                canvas.blit(&f, x, 0x8C);
            }
        }
    }

    /// Pages 7 and 8: the twelve options, the map list, the buttons.
    fn paint_custom(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen, page: SetupPage) {
        let a = pen.assets;
        if page == SetupPage::Custom {
            if let Some(s) = a.sheet(ICON_SHEET) {
                if let Some(f) = s.frame(0x0F) {
                    canvas.blit(&f, 0xA0, 0);
                }
            }
        }
        // The map list plate, its five rows, and the row the pointer is on.
        if let Some(s) = a.sheet(ICON_SHEET) {
            if let Some(f) = s.frame(0x10) {
                canvas.blit(&f, MAP_LIST_X, 9);
            }
        }
        // **The thumbnail of the map the list is pointing at**, which nothing
        // here drew. `ScenarioList_Draw`'s second statement is
        // `FUN_00410C71(0, 0x1F0, 9)` — the same helper the send-supplies panel
        // and the diplomacy county picker use, at the same `(x - 2, y + 3)`
        // offset — so the plate is a frame round a live minimap and not a
        // picture of one. County 0 is passed, so nothing is highlighted.
        if let Some(m) = ctx.assets.minimap(self.map) {
            let owner = |c: u8| ctx.game.kingdom.counties.get(c as usize).map_or(0, |c| c.owner);
            l2_view::chrome::draw_minimap_at(
                canvas,
                &m,
                MAP_THUMB,
                0,
                &l2_view::chrome::MinimapTint::Owner(&owner),
            );
        }
        for row in 0..MAP_LIST_ROWS {
            let slot = self.map_top + row;
            if slot >= MAP_COUNT {
                break;
            }
            let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
            let chosen = slot == self.map;
            canvas.fill_rect(
                MAP_LIST_X,
                y,
                MAP_LIST_W,
                MAP_LIST_ROW,
                if chosen { font::TEXT } else { font::DISABLED },
            );
            let name = a.text(GROUP_MAPS, slot).to_string();
            pen.body(
                canvas,
                MAP_LIST_TEXT_X,
                y + 1,
                &name,
                if chosen { font::DISABLED } else { font::TEXT },
            );
        }
        self.paint_scrollbar(canvas);
        // The twelve options.
        let chrome = pen.chrome;
        for (i, &(x, boxy, labely)) in OPTION_CELLS.iter().enumerate() {
            // `FUN_0040328E(102, i, x, labelY, 100, …)`: wrapped at 100 pixels,
            // which is what makes "Advanced Farming" two lines that end where
            // the box begins instead of one that runs into the next column.
            let label = a.text(GROUP_OPTIONS, i).to_string();
            pen.body_wrapped(canvas, x, labely, OPTION_LABEL_W, &label, font::TEXT);
            match chrome {
                // `FUN_004093E0` is `Ui_DrawBox` with border set 1.
                Some(c) => c.draw_box(canvas, x, boxy, 6, 3, 1),
                None => shell::button_recess(canvas, x, boxy, OPTION_BOX_W, OPTION_BOX_H),
            }
            let value = a.text(GROUP_VALUES, self.option_value(i)).to_string();
            pen.body_centred(canvas, x + 1, boxy + 16, 0x60, &value, font::HIGHLIGHT);
        }
        let n = if page == SetupPage::Custom { 3 } else { 4 };
        for (i, (x, s)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
            pen.eng_centred(
                canvas,
                GROUP,
                *s,
                *x,
                CUSTOM_BUTTON_Y,
                CUSTOM_BUTTON_W,
                self.colour(12 + MAP_LIST_ROWS + i),
            );
        }
        // **Both custom pages draw the five player cards, not only page 8.**
        // This comment used to say page 8; `FUN_0041F6C7` — page 7's painter —
        // calls `FUN_0041FBCB` with no guard
        // The card is `misc_sel` frame `2 * shieldIndex - 2` at (10, 94n + 6),
        // the lord's portrait frame `lord + 9` (14 for a human) at (84, 94n +
        // 10), and the name centred in 160 pixels at y = 94n + 80 in the
        // realm's own palette byte. `Realms_AssignLords` runs *inside* the
        // painter, so the cards are the assignment as much as a picture of it.
        //
        // Not drawn: the front end has not assigned lords in this workspace and
        // a card built from `Realm::default()` would be five copies of one
        // face. The chat log (`FUN_0041FF75`) and the chat input line
        // (`FUN_00420147`) are **multiplayer only** — both open with
        // `if (g_multiplayer != 0)` — so on page 7 the original draws nothing
        // for them either, and that is four call sites correctly absent rather
        // than missing.
        self.paint_gaps(canvas, pen, ctx.game.prefs.debug_overlay);
    }

    /// `ScenarioList_Draw`'s scroll bar, transcribed.
    ///
    /// Three stacked fills: the run above the window in `0x20`, the window in
    /// `0x3F`, the run below in `0x20`. The thumb absorbs the rounding error of
    /// all three percentages so the track is always exactly 44 pixels.
    ///
    /// **[V] with an empty list it is one full-length thumb**: `PctOf` returns
    /// 0 when the total is 0, so all three heights come out 0 and the
    /// correction hands the whole 44 to the middle segment.
    fn paint_scrollbar(&self, canvas: &mut Canvas) {
        // **The total is the one number here that is not the original's.**
        // `ScenarioList_Draw` divides by `DAT_00554018`, which
        // `FUN_0046A101` sets to *how many of the sixty slots have a
        // `MAPnn.PL8` on disk* — 44 on a shipped install, because the game
        // ships eleven of the fifteen files. We divide by all sixty. The
        // module header says what it would take to have the real one.
        let total = MAP_COUNT as i32;
        let pct_of = |a: i32, b: i32| if b == 0 { 0 } else { a * 100 / b };
        let pct = |x: i32, p: i32| p * x / 100;
        let top = self.map_top as i32;
        let rows = MAP_LIST_ROWS as i32;
        let above = pct(SCROLLBAR_H, pct_of(top, total));
        let below = pct(SCROLLBAR_H, pct_of(total - top - rows, total));
        // `iVar2 + ((0x2C - iVar4) - iVar2 - iVar3)`, which is the window's own
        // percentage plus whatever the three roundings lost — and simplifies to
        // the track minus the other two, exactly.
        let thumb = SCROLLBAR_H - above - below;
        for (y, h, colour) in [
            (SCROLLBAR_Y, above, font::DISABLED),
            (SCROLLBAR_Y + above, thumb, font::TEXT),
            (SCROLLBAR_Y + above + thumb, below, font::DISABLED),
        ] {
            if h != 0 {
                canvas.fill_rect(SCROLLBAR_X, y, SCROLLBAR_W, h, colour);
            }
        }
    }

    /// **What this build cannot honour, said on the page.**
    ///
    /// `docs/decisions.md` C21: a switch wired to nothing must not look
    /// finished. Two things go here — an option whose behaviour does not exist
    /// (*Exploration*), and the map, which the list can select and the world
    /// builder cannot yet build.
    ///
    /// **In our own font, never the original's**, for the same reason
    /// [`Screen::draw`]'s missing-background line is: nothing the original
    /// never drew may appear in its typeface, or a screenshot stops being
    /// evidence of anything.
    ///
    /// **The *NOT IMPLEMENTED* lines are debug overlay only**; the refusal to
    /// start a map is not, because without it the Start button silently does
    /// nothing.
    fn paint_gaps(&self, canvas: &mut Canvas, pen: &Pen, debug: bool) {
        let mut y = 462;
        let mut say = |line: &str| {
            l2_view::text::draw(canvas, MAP_LIST_X - 180, y, line, font::HIGHLIGHT);
            y += 9;
        };
        for &i in self.unhonoured.iter().filter(|_| debug) {
            let label = pen.assets.text(GROUP_OPTIONS, i).to_string();
            say(&format!("NOT IMPLEMENTED: {}", label.to_uppercase()));
        }
        // **The map line is gone**, and that is the point of this commit: the
        // slot the list names is now the world *Start* builds. What is left is
        // the case where it cannot be built at all.
        if let Some(why) = &self.failure {
            say(&format!("CANNOT START THIS MAP: {}", why.to_uppercase()));
        }
    }

    /// Page 9: the box the option opens, over the page underneath.
    fn paint_dropdown(&self, canvas: &mut Canvas, pen: &Pen, _ctx: &Ctx) {
        // `DAT_00553FB4` is the item count and the box is that plus the two
        // border cells — the painter and the hit test read the one number, so a
        // *Nobles* list shortened to the map cannot draw four rows and accept
        // three.
        let n = self.rows(self.open);
        let (x, y, _) = OPTION_LIST[self.open];
        let y = self.dropdown_y(y, n);
        pen.window(canvas, x, y, 6, n as i32 + 2, 1);
        for i in 0..n {
            let s = pen.assets.text(GROUP_VALUES, OPTION_BASE[self.open] + i).to_string();
            let colour = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            pen.body_centred(canvas, x + 1, y + 16 + i as i32 * 16, 0x60, &s, colour);
        }
    }

    /// Pages 11, 12 and 13. The battlefield itself is `l2-sim`'s, and the
    /// skirmish editor's palette of tools is `L2.eng` group 41; neither is
    /// drawn here. What is drawn is the page's own furniture: the background,
    /// and the three captions at `y = 0x1B8` that `FUN_0042051C` and
    /// `FUN_00420630` put there.
    fn paint_skirmish(&self, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        let items: [usize; 3] = match page {
            // 12 and 11: "Back", "Cust.", "Go".
            SetupPage::SkirmishFile => [0x24, 0x25, 0x26],
            _ => [0x25, 0x26, 0x24],
        };
        for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
            pen.eng_centred(canvas, GROUP, items[i], *x, 0x1B8, 0x38, self.colour(i));
        }
        if page == SetupPage::SkirmishFile {
            // `FUN_0042150B` opens `Ui_DrawBox(0x60, 100, 0x1C, 0x12)` — border
            // set **0**, the only window on any of these pages that is not set
            // 1 — over the skirmish page, and draws group 40's file captions
            // into it. The rest is `FUN_00414E06(999)`: a parchment plate, the
            // same outline again, and up to ten `.skr` names at (128, 176 + 16n)
            // with the selected one on a `0x3F` bar. Nothing here reads a
            // directory of skirmish files, so the rows are absent and the box
            // they sit in is not.
            pen.window(canvas, 0x60, 100, 0x1C, 0x12, 0);
            head.eng_heading_centred(canvas, GROUP_FILE, 6, 0x60, 0x84, 0x1C0, font::TEXT);
            outline_rect(canvas, SKIRMISH_FILE_LIST, font::TEXT);
            pen.eng_centred(canvas, GROUP_FILE, 8, 0x60, 0x164, 0x1C0, font::TEXT);
            // `FUN_00414E06`'s own two, in its order: the parchment first and
            // **the same outline again** over it. The plate is 336 x 176 and
            // the outline 352 x 165, so the plate overhangs the bottom edge and
            // the second draw puts it back — the original's overdraw, kept.
            pen.box_interior(canvas, 0x7E, 0xAE, 0x15, 0xB);
            outline_rect(canvas, SKIRMISH_FILE_LIST, font::TEXT);
            // `Ui_OkButton(0x1F8, 0x164, 0)` — the last statement of the
            // painter, and the only page of the thirteen that has one.
            pen.ok_button(canvas, SKIRMISH_FILE_OK.0, SKIRMISH_FILE_OK.1, 0);
        }
    }
}

