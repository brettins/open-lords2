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
use crate::screens::setup::skirmish::ROWS_SHOWN;

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
            skirmish: Default::default(),
            troops: Default::default(),
            skirmish_files: Vec::new(),
            skirmish_file_top: 0,
            clock_minute: None,
            clock_redraw: false,
        }
    }

    pub fn name(&self) -> String {
        self.name.commit(text::PLAYER_NAME_LEN)
    }

    pub fn name_field(&self) -> &text::TextField {
        &self.name
    }

    pub fn options(&self) -> &SetupOptions {
        &self.options
    }

    pub fn map(&self) -> usize {
        self.map
    }

    pub fn shield(&self) -> u8 {
        SHIELD_OF_HOTSPOT[(self.shield + 1).min(5)]
    }

    pub fn player_starts(&self) -> usize {
        self.player_starts
    }

    pub(crate) fn read_map(&mut self, ctx: &Ctx) {
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

    /// Everything but *Nobles* shows its whole run. *Nobles* is shortened to
    /// the map's seat count — `FUN_00433999`: `DAT_00553FB4 =
    /// g_playerStartCount - 1` when the map seats fewer than five — which is
    /// how the original stops a person asking for more lords than the map has
    /// castles for.
    pub(crate) fn rows(&self, i: usize) -> usize {
        if i == crate::setup::option::NOBLES {
            SetupOptions::nobles_rows_for_map(self.player_starts)
        } else {
            OPTION_COUNT[i]
        }
    }

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
            SetupPage::SkirmishFile => {
                // **`FUN_00434174`** tests the pointer itself, not a widget
                // record: `0x80 ≤ x ≤ 0x1DF` and `0xB0 ≤ y ≤ 0x14F`, ten rows
                // of sixteen. The row is `DAT_004EA1A0 + (y - 0xB0) / 16`
                // (`00430000.c:1358`) — the scroll base, not the slot.
                //
                // Everything outside the ten rows is `SaveLoad_Cancel`
                // (`0x00434308`), the *OK* cross, which is the page's one way
                // back.
                for r in 0..10 {
                    v.push((
                        Rect::new(0x80, 0xB0 + r as i32 * 0x10, 0x160, 0x10),
                        Action::Skirmish(SkirmishArm::File(r)),
                    ));
                }
                v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
            }
            SetupPage::NoCd | SetupPage::Load => {
                v.push((Rect::new(0, 0, 640, 480), Action::Item(0)));
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti => {
                // The widget table at `0x004DCF68`, in its order and with its
                // own rectangles — `node tools/oracle/widgets.js widgets
                // 4dcf68 22`. The three buttons are `FUN_0043D649` (*Back*),
                // `FUN_0043DA9E` (*Cust.*) and `FUN_0043D5B7` (*Go*), and the
                // painter's caption boxes are not what is clickable.
                for (i, x) in [461, 519, 577].iter().enumerate() {
                    v.push((Rect::new(*x, 422, 56, 52), Action::Item(i)));
                }
                // Page 11 is the multiplayer skirmish and keeps only those
                // three. Its own arms — the lobby list and `FUN_0043DBAC`'s
                // 0…4 difficulty cycle, which is the other difficulty arm —
                // are **out of scope**, `docs/plan.md` 0.1 *Multiplayer is
                // excluded, deliberately and not by omission*. Uncited on
                // purpose: the widget table is read, the arms are not ported.
                if self.page == SetupPage::SkirmishMulti {
                    return v;
                }
                for r in 0..ROWS_SHOWN {
                    let y = 185 + r as i32 * 16;
                    v.push((
                        Rect::new(468, y, 133, 15),
                        Action::Skirmish(SkirmishArm::Row(r)),
                    ));
                }
                for (y, d) in [(181, -1), (266, 1)] {
                    v.push((
                        Rect::new(604, y, 20, 20),
                        Action::Skirmish(SkirmishArm::Scroll(d)),
                    ));
                }
                for x in [11, 278] {
                    v.push((
                        Rect::new(x, 259, 158, 45),
                        Action::Skirmish(SkirmishArm::Sides),
                    ));
                }
                for (kind, y) in [(2usize, 291), (0, 322), (1, 353), (3, 384)] {
                    v.push((
                        Rect::new(463, y, 30, 30),
                        Action::Skirmish(SkirmishArm::Kind(kind)),
                    ));
                }
                v.push((
                    Rect::new(495, 384, 141, 30),
                    Action::Skirmish(SkirmishArm::OpenFiles),
                ));
                for (i, x) in [20, 275].iter().enumerate() {
                    v.push((
                        Rect::new(*x, 350, 160, 125),
                        Action::Skirmish(SkirmishArm::Handicap(i + 1)),
                    ));
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
    pub(crate) fn dropdown_y(&self, y: i32, rows: usize) -> i32 {
        if self.open == crate::setup::option::NOBLES {
            y - (rows as i32 - 1) * 16
        } else {
            y
        }
    }

    pub(crate) fn at(&self, x: i32, y: i32) -> Option<(usize, Action)> {
        self.hotspots()
            .into_iter()
            .enumerate()
            .find(|(_, (r, _))| r.contains(x, y))
            .map(|(i, (_, a))| (i, a))
    }

    pub(crate) fn count(&self) -> usize {
        self.hotspots().len()
    }

    pub(crate) fn activate(&mut self, ctx: &mut Ctx) -> Transition {
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
    pub(crate) fn act(&mut self, action: Action, ctx: &mut Ctx) -> Transition {
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
            Action::Skirmish(arm) => self.skirmish_arm(arm),
        }
    }

    /// Page 12's five arms and page 13's one, each of which ends the same way
    /// in the original: `Skirmish_FillArmies` (`0x0042BF46`) and a redraw.
    fn skirmish_arm(&mut self, arm: SkirmishArm) -> Transition {
        match arm {
            SkirmishArm::Row(r) => {
                self.skirmish.pick_row(r);
            }
            SkirmishArm::Scroll(d) => self.skirmish.scroll(d),
            SkirmishArm::Kind(k) => self.skirmish.choose_kind(k),
            SkirmishArm::Sides => self.skirmish.swap_sides(),
            SkirmishArm::Handicap(h) => self.skirmish.handicap(h),
            // `FUN_0043DDF4` opens page 13 only on `0 < DAT_005653E8`
            // (`00430000.c:8140`) — the count of `.skr` files found. With
            // none, the field is not a button.
            SkirmishArm::OpenFiles => {
                if self.skirmish_files.is_empty() {
                    return Transition::Stay;
                }
                return self.go(SetupPage::SkirmishFile);
            }
            SkirmishArm::File(r) => {
                // `FUN_00434174` ignores a row past the end of the list, and
                // the row that is the name already chosen **returns 0 and
                // stays on page 13** (`00430000.c:1364-1367`) — only a new
                // name writes `g_setupPage`.
                let name = self.skirmish_files.get(self.skirmish_file_top + r).cloned();
                if let Some(name) = name {
                    if self.skirmish.choose_file(&name) {
                        return self.go(SetupPage::Skirmish);
                    }
                }
            }
        }
        Transition::Stay
    }

    fn item(&mut self, i: usize, ctx: &mut Ctx) -> Transition {
        match (self.page, i) {
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
            (SetupPage::Title, 2) => {
                Transition::Push(ScreenId::Movie(crate::movie::Film::LordsOfMagic))
            }
            (SetupPage::Title, 3) => Transition::Quit,
            (SetupPage::Options, 0) => self.go(SetupPage::Campaign),
            (SetupPage::Options, 1) => self.go(SetupPage::Load),
            (SetupPage::Options, 2) => self.go(SetupPage::Skirmish),
            (SetupPage::Options, 3) => self.go(SetupPage::Custom),
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
        if page == SetupPage::Shield && self.page != SetupPage::Shield {
            // arm: 0x00432B05/name-field-open left-release
            self.name = begin_name(&self.saved_name);
        }
        self.page = page;
        self.selected = 0;
        Transition::Stay
    }

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
    fn start(&mut self, ctx: &mut Ctx) -> Transition {
        // One person, in this build. `DAT_00553F98` is the lobby's count and
        if !self.map_read {
            self.read_map(ctx);
        }
        if HUMAN_PLAYERS > self.player_starts {
            return Transition::Stay;
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
    fn start_campaign(&mut self, ctx: &mut Ctx) -> Transition {
        let campaign = crate::victory::Campaign::new(self.track);
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


