#![allow(unused_imports)]
use super::*;

use super::*;
use super::*;

impl Screen for MapScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Campaign
    }

    fn mode_screen_id(&self) -> Option<u8> {
        self.move_order.is_some().then_some(0x10)
    }

    fn minimap_mode(&self) -> Option<u8> {
        Some(self.minimap_mode as u8)
    }

    fn title(&self, ctx: &Ctx) -> String {
        format!(
            "Lords of the Realm II - {} {}",
            season_name(ctx.game.kingdom.season),
            ctx.game.kingdom.year
        )
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        if ctx.game.combine_ask.is_some() {
            let fired = self.press.event(&CONFIRM_WIDGETS, event);
            if fired.is_some() || self.press.busy() {
                self.scrolled = true;
            }
            return Transition::Stay;
        }
        if (turn::turn_in_flight(ctx.game) || self.fading.is_some())
            && !matches!(event, Event::Pointer { .. } | Event::PointerLeft | Event::Release { .. })
        {
            return Transition::Stay;
        }
        match event {
            // **`App_WndProc` (`0x004B29BE`) `VK_ESCAPE`**: `if (g_appPhase ==
            // 3) Menu_Quit(); else g_quitRequest = 1;` — inside a game the key
            // *is* File > Quit, which is `Ui_OpenConfirm(0, …)`, and not a way
            // out on its own. Ours used to pop straight off the map.
            //
            // arm: 0x004B29BE/escape-quits-outside-the-game key
            Event::KeyDown(Key::Escape) => {
                return Transition::Push(ScreenId::Confirm(confirm::Ask::Quit))
            }
            Event::KeyDown(Key::Enter) => {
                if ctx.game.selected != 0 {
                    return Transition::Push(ScreenId::County(
                        ctx.game.selected,
                        county::Panel::Tax,
                    ));
                }
            }
            Event::KeyDown(Key::Up) => {
                self.scroll(Dir::N);
            }
            Event::KeyDown(Key::Down) => {
                self.scroll(Dir::S);
            }
            Event::KeyDown(Key::Left) => {
                self.scroll(Dir::W);
            }
            Event::KeyDown(Key::Right) => {
                self.scroll(Dir::E);
            }
            Event::KeyDown(Key::Char('V')) => {
                if ctx.game.is_players(ctx.game.selected) {
                    return Transition::Push(ScreenId::Village(ctx.game.selected));
                }
                self.status = "NOT YOUR COUNTY".into();
            }
            // **Ours, and only the key is.** The original reaches the
            // raise-army screen from the county strip's first button —
            // `Sidebar_Button` (`0x0043AE30`), hotspot 1, *"the county's
            // army"*, which sets `g_screenId = 0x17`. That strip is another
            // agent's, so the destination is the original's
            // not. See `screens::army`.
            Event::KeyDown(Key::Char('R')) => {
                let county = ctx.game.selected;
                if ctx.game.open_levy(county) {
                    return Transition::Push(ScreenId::RaiseArmy(county));
                }
                self.status = "SELECT ONE OF YOUR COUNTIES FIRST".into();
            }
            // **Ours, and only the key is.** The original reaches screen `0x11`
            // from the *unit panel*'s split button, which is one of the three
            // hotspots `FUN_00437002` tests over an army. We have no unit
            // panel — our map click goes straight to move-order mode, which is
            // what `Panel_MoveButton` does with two of its three siblings
            // unbuilt — so this is the door to it. See `screens::divide`.
            Event::KeyDown(Key::Char('A')) => match self.selected_unit {
                Some(unit) if ctx.game.is_players_unit(unit) => {
                    return Transition::Push(ScreenId::Divide(unit))
                }
                _ => self.status = "CLICK ONE OF YOUR ARMIES FIRST".into(),
            },
            Event::KeyDown(Key::Char('N')) => self.cycle_unit(ctx),
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            Event::KeyDown(Key::Char('S')) => {
                return Transition::Push(ScreenId::SaveLoad(SaveLoadMode::Save))
            }
            Event::KeyDown(Key::Char('L')) => {
                return Transition::Push(ScreenId::SaveLoad(SaveLoadMode::Load))
            }
            Event::KeyDown(Key::Char('Z')) => self.toggle_zoom(ctx),
            Event::KeyDown(Key::Char('E')) | Event::KeyDown(Key::Space) => {
                return self.end_turn(ctx)
            }
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                self.pointer_in = true;
                self.update_hover_path(x, y);
                if self.slider_held && SPLIT_SLIDER.contains(x, y) {
                    self.drag_split(ctx, x);
                }
                self.focus = if END_TURN_BUTTON.contains(x, y) {
                    Focus::EndTurn
                } else {
                    match SIDEBAR_BUTTONS.iter().position(|b| b.rect().contains(x, y)) {
                        Some(i) => Focus::Sidebar(i),
                        None => Focus::None,
                    }
                };
            }
            // `Screen_FrameInput`'s `g_screenId == 0` arm ends with
            // `if (onATile && rightReleased) { g_screenId = 4; FUN_0043CAF4(); }`
            // — screen `0x04`, the information panel, which
            // `FUN_0043CAF4` fills by picking whatever is under the cursor:
//
            // `UnitPanel_Draw` for a unit, `FUN_0041BEFE` for a bare tile. The
            // shipped `Readme.txt` errata describes exactly this — *"Right
            // clicking on an army accesses an information pop-up that includes
            // the army's county of origin"* —
            // `Eng_DrawString(31, 9, ...)`, *"An army from"*, followed by group
            // 100 indexed `homeCounty + scenarioIndex * 20`.
//
            // `FUN_00439079` consumes the click only when an overlay is up, so
// with no overlay this falls through
            // `return 0` does.
            Event::RightClick { x, y } if self.clear_minimap_mode(x, y) => {}
            Event::RightClick { x, y } if self.map_clip().contains(x, y) => {
                // arm: 0x0042FF10/move-order-right-cancels right-release
                if self.selected_unit.is_some() {
                    self.cancel_move_selection();
                    self.status = "ORDERS CANCELLED".into();
                    return Transition::Stay;
                }
                let target = self
                    .pick_tile(x, y)
                    .map(|(tx, ty)| {
                        let tile = l2_kingdom::map::index(tx, ty);
                        match ctx.game.kingdom.campaign.units.at(tx, ty) {
                            Some(unit) => crate::screens::info::Target::Unit(unit),
                            None => crate::screens::info::Target::Tile(tile),
                        }
                    })
                    .unwrap_or(crate::screens::info::Target::Tile(0));
                // **`FUN_0043CAF4`'s extra step, between the pick
                // panel — the right button also selects a county.**
                //
                // `docs/decisions.md` correction
                // C199.
                //
                // ```c
                // Map_ResolvePick();
                // ...
                // if (g_pickedTileCounty != 0 && g_pickedTileCounty != g_selectedCounty
                //     && g_pickedTileUnit == 0) {
                //   DAT_0053f0dc = g_counties[g_pickedTileCounty].townTile;
                //   if (DAT_0053f0dc != 0) {
                //     g_selectedCounty = g_pickedTileCounty;
                //     Map_CentreOnTile(DAT_0053f0dc);
                //   }
                //   DAT_004eb260 = 1; FUN_004050c0();
                // }
                // FUN_0041b032();
                // ```
                //
                // The redraw and `FUN_004050C0` outside the inner `if` are the
                // sidebar repaint, which we do every frame.
                if let crate::screens::info::Target::Tile(tile) = target {
                    let county = ctx.game.kingdom.campaign.map.county[tile];
                    if county != 0 && county != ctx.game.selected {
                        if let Some(&town) = Self::town(ctx, county).first() {
                            let (tx, ty) = l2_kingdom::map::coords(town);
                            ctx.game.select(county);
                            self.centre_on_tile(tx as usize, ty as usize);
                        }
                    }
                }
                // arm: 0x0042FF10/map-right-opens-info right-release
                return Transition::Push(ScreenId::Info(target));
            }
            Event::PointerLeft => {
                self.pointer_in = false;
                self.focus = Focus::None;
            }
            // `FUN_00439122` eats the release and does nothing with it — the
            // value was already set on the way down and on every move since.
            Event::Release { .. } => self.slider_held = false,
            Event::Click { x, y } => {
                // **`Map_Click`'s outermost guard, and it is the whole
                // function.** `Map_Click` (`0x0043CE1A`) is
                // `if (g_messageGroup == 0) { …all 1,263 bytes of it… } else
                // { Msg_DismissUnlessQuestion(); }` — so with a message scroll
                // up, a left click on the map closes it and **the map does
                // nothing else**: no tile picked, no county selected, no
                // village, no army ordered. A *question* survives, which is
                // what stops a stray click from silently declining an alliance.
                //
                // arm: 0x00476710/map-click-dismiss left-release
                if ctx.game.messages.is_open() {
                    // `Msg_Dismiss` (`00470000.c:2444`): a game-over letter's
                    // dismissal is `Campaign_EnterConquest(); g_screenId = 0x1C`.
                    return match crate::message::dismiss_unless_question(ctx.game) {
                        Some(crate::message::Dismissal::GameOver(_)) => {
                            Transition::Replace(ScreenId::Conquest)
                        }
                        _ => Transition::Stay,
                    };
                }
                // ```c
                // if (Map_EdgeScroll() || FUN_00439079() || Sidebar_ButtonClicked()) goto done;
                // if (!syncWait && (!turnEnded || debugOverride)) {
                //     if (Menu_OpenDropdown(&g_menuBarItems, 3) || Minimap_ModeButtonClicked()
                //         || CountyStrip_Click() || Labour_SplitSliderDrag()
                //         || CountyStrip_JobClick()) goto done;
                //     ...the message scroll, the zoom, Map_Click, the info panel...
                // }
                // ```
                if END_TURN_BUTTON.contains(x, y) {
                    // **Ending the turn takes every open panel with it**, and
                    // that is not this arm's doing: `Turn_End` writes
                    // `DAT_0055403C`.
                    //
                    // `Screen_FrameInput`'s forty-nine arms open with
                    // `if (DAT_0055403C != 0 && !debugOverride)` and force-close.
                    //
                    // `docs/arms.json` `0x0042FF10/force-close-on-turn-end` is
                    // the general arm and stays `missing`: this is one screen's
                    // corner of it, not the guard.
                    //
                    // arm: 0x0043AC23/end-turn left-press
                    let t = self.end_turn(ctx);
                    return if t == Transition::Stay { Transition::Reveal } else { t };
                } else if let Some(b) = SIDEBAR_BUTTONS.iter().find(|b| b.rect().contains(x, y)) {
                    // Each of the five is an arm of its own: `Sidebar_Button`
                    // (`0x0043AE30`) is a five-way `if` on `g_uiHotspotId`.
                    //
                    // arm: 0x0043AE30/sidebar-levy left-press
                    // arm: 0x0043AE30/sidebar-court left-press
                    // arm: 0x0043AE30/sidebar-supplies left-press
                    // arm: 0x0043AE30/sidebar-castle left-press
                    // arm: 0x0043AE30/sidebar-lords left-press
                    let SidebarAction::Screen(id) = b.action;
                    let gated = matches!(id, 0x17 | 0x18 | 0x1B);
                    if gated && !ctx.game.is_players(ctx.game.selected) {
                        self.status = "NOT YOUR COUNTY".into();
                        return Transition::Stay;
                    }
                    return Transition::Push(sidebar_destination(id, ctx.game.selected));
                } else if let Some(title) = menubar::title_at(&*ctx, x, y) {
                    // **`Menu_OpenDropdown` (`0x0040DECA`)** — the one line this
                    // module's header carried as *"not reproduced"*. It saves
                    // `g_screenId` into `g_menuPrevScreen` and writes `0x32`,
                    // which is a push here.
                    //
                    // arm: 0x0040DECA/open-dropdown left-press
                    return Transition::Push(ScreenId::MenuBar(title));
                } else if let Some(i) =
                    MINIMAP_MODE_BUTTONS.iter().position(|r| r.contains(x, y))
                {
                    // arm: 0x0043292D/minimap-mode-buttons left-press
                    self.minimap_mode_button(ctx, i);
                } else if SPLIT_SLIDER.contains(x, y)
                    && ctx.game.is_players(ctx.game.selected)
                {
                    // `FUN_00439122`, the farm/industry split. The press is the
                    // first frame of a **drag**: the button is now down, and
                    // every pointer move while it stays down moves the slider.
                    //
                    // arm: 0x00439122/split-slider drag
                    self.slider_held = true;
                    self.drag_split(ctx, x);
                } else if let Some(job) = self.job_row_at(&*ctx, x, y) {
                    // **`CountyStrip_JobClick` (`0x00438E3B`)** — the produce
                    // rows on the 162 × 128 plate at y = 302, which open the job
                    // popup for that row. Another of this module's header's
                    // three "not reproduced" lines.
                    //
                    // arm: 0x00438E3B/job-rows left-press
                    return Transition::Push(ScreenId::Job(ctx.game.selected, job));
                } else if let Some(panel) = county::panel_at(x, y) {
                    // arm: 0x00438CEB/strip-population left-press
                    // arm: 0x00438CEB/strip-happiness left-press
                    // arm: 0x00438CEB/strip-tax left-press
                    // arm: 0x00438CEB/strip-ration left-press
                    if ctx.game.selected != 0 {
                        return Transition::Push(ScreenId::County(ctx.game.selected, panel));
                    }
                } else if chrome::minimap_hit_area().contains(x, y) {
                    // arm: 0x0043253A/minimap-click left-press
                    self.ensure_minimap(ctx);
                    let county = self.minimap.as_ref().map_or(0, |m| m.county_at(x, y));
                    if county != 0 && ctx.game.select(county) {
                        let id = county as usize;
                        let anchor = (
                            ctx.game.anchor_x[id] as usize,
                            ctx.game.anchor_y[id] as usize,
                        );
                        self.centre_on_county(anchor);
                        self.status = format!("COUNTY {county} SELECTED");
                    }
                    // arm: 0x0042FF10/minimap-closes-the-surface left-press
                    return Transition::Reveal;
                } else if self.map_clip().contains(x, y) {
                    self.ensure(ctx);
                    // `Map_ZoomInAtTile` (`0x004350A1`) centres the near view on
                    // the picked tile with the same `col - 4`, `(row & ~1) - 12`
                    // arithmetic as `Map_CentreOnTile`, which is
                    // [`MapScreen::centre_on_tile`].
                    //
                    // arm: 0x0042FF10/map-zoom-in-at-tile left-press
                    if self.selected_unit.is_none() && self.zoom.id == FAR.id {
                        if let Some((tx, ty)) = self.pick_tile(x, y) {
                            self.set_zoom(ctx, NEAR);
                            self.centre_on_tile(tx as usize, ty as usize);
                            self.status = "ZOOMED IN".into();
                            return Transition::Stay;
                        }
                    }
                    if let Some(unit) = self.selected_unit {
                        if !ctx.game.is_players_unit(unit) {
                            self.cancel_move_selection();
                        // arm: 0x0042FF10/move-order-confirm left-press
                        } else if let Some(dest) = self.pick_tile(x, y) {
                            return self.confirm_move_order(ctx, unit, dest);
                        } else {
                            self.cancel_move_selection();
                            return Transition::Stay;
                        }
                    }
                    // `Map_HoverUnitTarget` collects a plane-0 `0x80` tile with
                    // terrain above `0x14` as a target for the selected army —
                    // as `g_hoverGarrisonCounty` when the county is the mover's
                    // and as `g_hoverSiegeCounty` when it is not, raising
                    // `L2.eng` 10/7 *"Garrison castle?"* or 10/8 *"Besiege
                    // castle?"*. The original never offers the garrison itself,
                    // because **it does not draw a garrisoned unit at all** — it
                    // flies a flag over the castle instead. Ours draws a hollow
                    // marker so a player can see his men are in there.
                    let castle_target = self.pick_tile(x, y).filter(|&(tx, ty)| {
                        let map = &ctx.game.kingdom.campaign.map;
                        map.has(tx, ty, l2_kingdom::map::flags::SETTLEMENT)
                            && map.terrain_at(tx, ty) > l2_kingdom::map::terrain::CASTLE_PLOT
                    });
                    if let (Some(unit), Some(dest)) = (self.selected_unit, castle_target) {
                        if ctx.game.is_players_unit(unit) {
                            return self.order_march(ctx, unit, dest);
                        }
                    }
                    if let Some(unit) = self.unit_at(&Ctx { game: ctx.game, assets: ctx.assets }, x, y)
                    {
                        return self.click_unit(ctx, unit);
                    }
                    let county = self.county_at(x, y);
                    if county != 0 && ctx.game.is_players(county) {
                        // arm: 0x0043CE1A/industry-toggle left-release
                        if let Some(tile) = self.settlement_at(ctx, county, x, y) {
                            if county != ctx.game.selected {
                                let Some(&town) = Self::town(ctx, county).first() else {
                                    self.status = "THAT COUNTY HAS NO TOWN".into();
                                    return Transition::Stay;
                                };
                                let (tx, ty) = l2_kingdom::map::coords(town);
                                ctx.game.select(county);
                                self.centre_on_tile(tx as usize, ty as usize);
                            }
                            let terrain = ctx.game.kingdom.campaign.map.terrain[tile];
                            match industry::map_toggle_for_graphic(terrain) {
                                Some(what) => {
                                    let on = ctx.game.kingdom.toggle_industry(county as usize, what);
                                    let player = ctx.game.player;
                                    ctx.game.messages.enqueue(
                                        crate::message::Record {
                                            to: 0,
                                            from: player,
                                            group: industry::toggle_message_group(what, on),
                                            variant: 0,
                                            category: crate::message::category::TIP,
                                            county: 0,
                                            spare: 0,
                                            payload: 0,
                                        },
                                        player,
                                    );
                                    self.status = format!(
                                        "{} {}",
                                        toggle_name(what),
                                        if on { "ON" } else { "OFF" }
                                    );
                                }
                                None => self.status = "NOTHING TO SWITCH THERE".into(),
                            }
                            return Transition::Stay;
                        }
                        // arm: 0x0043CE1A/village left-release
                        if let Some(tile) = self.tile_at(x, y, Self::town(ctx, county).into_iter())
                        {
                            let (tx, ty) = l2_kingdom::map::coords(tile);
                            ctx.game.select(county);
                            self.centre_on_tile(tx as usize, ty as usize);
                            return Transition::Push(ScreenId::Village(county));
                        }
                        // ```c
                        // if ((flags & 0x20) && g_counties[pickedCounty].owner == g_localPlayer) {
                        //     _DAT_005681CC = 3; g_screenId = 4; FUN_0041B032();
                        // }
                        // ```
                        //
                        // arm: 0x0043CE1A/field-brush left-release
                        if let Some((tx, ty)) = self.pick_tile(x, y) {
                            let tile = l2_kingdom::map::index(tx, ty);
                            let map = &ctx.game.kingdom.campaign.map;
                            if map.flags[tile] & l2_kingdom::map::flags::FARMLAND != 0
                                && ctx.game.is_players(map.county[tile])
                            {
                                return Transition::Push(ScreenId::Info(
                                    crate::screens::info::Target::Tile(tile),
                                ));
                            }
                        }
                    }
                    // A click that reaches here — plain ground, sea, a county
                    // that is not yours, your own county away from its town, its
                    // fields and its buildings — falls out of the bottom having
                    // changed nothing at all. The last statement in the original
                    // is `else { DAT_0056D64C = 0; }`, a scroll latch.
//
                    // `docs/decisions.md` C61.
                    let _ = county;
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if let Some(unit) = ctx.game.begin_move_order.take() {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            self.begin_move_selection(&read, unit);
        }
        if let Some(widget) = self.press.tick().next() {
            return self.answer_combine(ctx, widget == 0);
        }
        if ctx.game.combine_ask.is_some() {
            if self.press.any_pressed() {
                self.scrolled = true;
            }
            return Transition::Stay;
        }
        self.flag_tick = (self.flag_tick + 1) & 0x7F;
        let phase = self.flag_tick >> 4;
        if phase != self.flag_phase {
            self.flag_phase = phase;
            self.scrolled = true;
        }
        self.herd_tick += 1;
        if self.herd_tick > 0x5F {
            self.herd_tick = 0;
        }
        let phase = self.herd_tick >> 4;
        if phase != self.herd_phase {
            self.herd_phase = phase;
            self.scrolled = true;
        }
        // **And the industry wheels**
        // run off `Tick_Pulses` (`0x004BBC80`), the 20 ms `timeGetTime` divider
        // chain the *village* animates from, at whichever of its four rungs the
        // mine's output picked. Three different clocks on one screen.
        {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            if self.step_industry(&read) {
                self.scrolled = true;
            }
        }
        if self.fading.is_some() {
            return Transition::Stay;
        }
        // **`Map_ScrollThrottle` (`0x004BBBE3`)** — the map does not step on
        // every frame the pointer is at the edge. See
        // [`MapScreen::scroll_interval_ticks`].
        let every = self.scroll_interval_ticks();
        self.scroll_wait = self.scroll_wait.saturating_sub(1);
        // arm: 0x00432221/map-edge-scroll hover-at-edge
        if self.scroll_wait == 0 && every != u32::MAX {
            if let Some(dir) = self.edge_direction() {
                self.scroll_wait = every;
                if self.scroll(dir) {
                    self.scrolled = true;
                }
            }
        }
        Transition::Stay
    }

    /// Everything here used to sit in [`MapScreen::update`], which the machine
    /// runs for the top screen only —
    /// menu stopped the campaign dead underneath it. `Battle_Frame`
    /// (`0x004B99C0`) has no screen test on this call at all; see
    /// [`Screen::wind_turn`] and [`crate::screen::Machine::wind_turn`].
    fn wind_turn(&mut self, ctx: &mut Ctx) -> Transition {
        if self.fading.is_some() {
            return self.tick_fade();
        }
        if ctx.game.turn_clock.end_turn_pending() && !turn::turn_in_flight(ctx.game) {
            ctx.game.turn_clock.take_end_turn();
            return self.end_turn(ctx);
        }
        let resumed = self.resume_turn(ctx);
        if resumed != Transition::Stay {
            return resumed;
        }
        if !turn::turn_in_flight(ctx.game) {
            // **`Turn_Tick`'s phase-4 arm on an ordinary frame.** The original
            // is *in* phase 4 while the person deliberates and calls
            // `AI_RunTurnStep` (`0x0049A581`) every frame of it, so the AI
            // realms take their turn before End Turn and not inside it. See
            // [`turn::tick_ai_frame`].
            turn::tick_ai_frame(ctx.game);
            if turn::tick_units_only(ctx.game) > 0 {
                self.scrolled = true;
            }
        }
        Transition::Stay
    }

    fn fade(&self) -> Option<u8> {
        self.fading.as_ref().map(|f| f.phase)
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::replace(&mut self.scrolled, false)
    }

    /// `FUN_0049A3E6`'s `Save_RotateAndWrite()`. Raised in
    /// [`MapScreen::tick_fade`] at [`l2_view::fade::is_darkest`].
    fn take_autosave(&mut self) -> bool {
        core::mem::take(&mut self.autosave)
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.ensure(ctx);
        self.ensure_minimap(ctx);
        // `Screen_DrawWidgets` (`0x004BA26E`)'s `0x10` arm calls
        // `Map_HoverUnitTarget` (`0x004A8E0B`) here, in the draw pass, once a
        // frame — not on pointer motion. [V] `004a0000.c:3726`:
        // `Path_MarkPreviewTiles()` (`0x004A91BA`) stands *above* the
        // `if (DAT_005691E0 != g_hoverTileOffset)` guard at 3727, so the trail
        // is re-marked (tile bank `0x40`) every frame with the pointer still;
        // only the descent under the guard — `Move_ExtractPath`, the six
        // `g_hover*` classifications — waits for the tile to change.
        // [V] `DAT_005691E0` has exactly two references in the decompilation,
        // lines 3727 and 3728: no caller resets it,
        // `Map_BeginMoveSelection` (`0x0043723A`) included.
        let (px, py) = self.pointer;
        self.update_hover_path(px, py);
        let ink = &ctx.assets.ink;
        let game = &ctx.game;
        let k = &game.kingdom;
        let clip = self.map_clip();

        canvas.pixels.copy_from_slice(&self.base.pixels);


        let debug = game.prefs.debug_overlay;
        for id in k.county_ids().filter(|_| debug) {
            let (ax, ay) = (game.anchor_x[id] as usize, game.anchor_y[id] as usize);
            if game.hides_tile(l2_kingdom::map::index(ax as u8, ay as u8)) {
                continue;
            }
            let Some((cx, cy)) = campaign::tile_centre(self.view, &self.zoom, ax, ay) else {
                continue;
            };
            let owner = k.counties[id].owner as usize;
            let colour = ink.realm.get(owner).copied().unwrap_or(ink.dim);
            fill_clipped(
                canvas,
                cx - MARKER - 1,
                cy - MARKER - 1,
                MARKER * 2 + 3,
                ink.background,
                clip,
            );
            fill_clipped(
                canvas,
                cx - MARKER,
                cy - MARKER,
                MARKER * 2 + 1,
                colour,
                clip,
            );
        }

        // **`Screen_DrawCampaign`'s far-zoom arm** (`0x0040F5FD`), whole:
        //
        // ```c
        // Ui_DrawBox(0, 0x19C, 0x1E, 4);            /* (0, 412), 480 x 64   */
        // DAT_0058FE2C = 1;
        // g_penAdvance = 0;
        // Eng_DrawString(0x65, g_scenarioIndex, 0x40, 0x1A8, &g_fontHeading, 0x3F);
        // Eng_DrawString(0x22, 0, g_penAdvance + 0x50, 0x1A8, &g_fontHeading, 0x3F);
        // Ui_DrawYear(g_year, g_penAdvance + 0x60, 0x1A8, 1);
        // DAT_0058FE2C = 0;
        // Eng_DrawString(0x22, 1, 0x50, 0x1C6, &g_fontBody, 0x3F);
        // ```
        //
        // The box was here
        // status line out of it and left the box **empty**, which is the hole
// that correction filed. Group 101 is the sixty map
        // names and group 34 is *"Year"* and *"Click on the county you wish to
        // view."* — the game saying in its own words what the far zoom is for,
// so `Map_Click` does nothing at zoom 2. `docs/draws-map.md`
        // §5.4, C89, C189.
        //
        // §7 both say *"the map's name, the season
        // draws three things
        // the menu bar, out of group 29.
        //
        // `g_penAdvance` is the width drawn **since the reset**, cumulative
        // over both strings, and every `Pen` method returns an absolute x — so
        // `g_penAdvance + 0x50` after a name drawn at `0x40` is sixteen pixels
        // past where that name ended, and `g_penAdvance + 0x60` after a label
        // drawn at `0x50` is sixteen past *that*. Written as the subtraction
        // the decompilation implies, like `draw_menu_bar` two hundred lines
        // below. `docs/decisions.md` C61 — the same confusion, seven times.
        if self.zoom.id == FAR.id {
            match &ctx.assets.chrome {
                Some(c) => c.draw_box(canvas, 0, 412, 30, 4, 0),
                None => widget::panel(canvas, ink, Rect::new(0, 412, 480, 64)),
            }
            let pen = crate::shell::Pen {
                assets: &ctx.assets.shell,
                ink,
                chrome: ctx.assets.chrome.as_ref(),
                shadow: Some(font::SHADOW),
                caps: None,
            };
            // `DAT_0058FE2C = 1` over the three heading draws.
            let caps = pen.drop_caps();
            let face = crate::shell::Face::Heading;
            let name = map_name(ctx);
            let after_name = caps.text_in(face, canvas, FAR_BOX_NAME_X, FAR_BOX_Y, &name, font::TEXT);
            let after_year_label = caps.text_in(
                face,
                canvas,
                after_name - FAR_BOX_NAME_X + FAR_BOX_YEAR_LABEL_X,
                FAR_BOX_Y,
                &far_box_text(ctx, FAR_BOX_YEAR_LABEL),
                font::TEXT,
            );
            caps.year(
                canvas,
                after_year_label - FAR_BOX_YEAR_LABEL_X + FAR_BOX_YEAR_X,
                FAR_BOX_Y,
                ctx.game.kingdom.year,
                1,
                font::TEXT,
            );
            // `DAT_0058FE2C = 0` again.
            pen.body(
                canvas,
                FAR_BOX_ADVICE_X,
                FAR_BOX_ADVICE_Y,
                &far_box_text(ctx, FAR_BOX_ADVICE),
                font::TEXT,
            );
            if debug {
                text::draw(canvas, 24, FAR_BOX_ADVICE_Y + 14, &self.status, ink.text);
            }
        }

        if debug && game.is_players(game.selected) {
            for (tile, kind) in k.field_tiles(game.selected as usize) {
                if game.hides_tile(tile) {
                    continue;
                }
                let (tx, ty) = l2_kingdom::map::coords(tile);
                let Some((cx, cy)) =
                    campaign::tile_centre(self.view, &self.zoom, tx as usize, ty as usize)
                else {
                    continue;
                };
                let m = brush::FIELD_MARKER;
                fill_clipped(
                    canvas,
                    cx - m - 1,
                    cy - m - 1,
                    m * 2 + 3,
                    ink.background,
                    clip,
                );
                fill_clipped(
                    canvas,
                    cx - m,
                    cy - m,
                    m * 2 + 1,
                    field_colour(ink, kind),
                    clip,
                );
            }
        }

        // `FUN_004071A0` handles all four arms inside a single lattice-ordered
        // traversal,
        // earlier one. Ours draws every herd and then every flag, so the flag
        // wins instead. A tile is never both, so nothing is drawn twice; the
        // only visible difference is a 58 × 30 meadow overlapping the 32 × 24
// banner of the town up and to its left. Recorded
        // over — merging the passes means the county loop
        // becoming one, which is a bigger change than the defect.
        draw_herds(self, canvas, ctx, clip);
        draw_flags(self, canvas, ctx, clip);
        draw_path_preview(self, canvas, ctx, clip);
        draw_units(self, canvas, ctx, clip);

        draw_menu_bar(canvas, ctx, false);
        draw_right_panel(self, canvas, ctx);
        draw_unit_banner(self, canvas, ctx);
        draw_combine_box(self, canvas, ctx);
    }
}


