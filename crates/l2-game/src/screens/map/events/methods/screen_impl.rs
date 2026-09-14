#![allow(unused_imports)]
use super::*;

use super::*;
use super::*;

impl Screen for MapScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Campaign
    }

    /// **`g_screenId` `0x10` while an army is picked up.** `Panel_MoveButton`
    /// and `Map_Click` reach `Map_BeginMoveSelection`, which writes it.
    /// three ways out write `0` — which is [`MapScreen::move_order`] being
    /// `Some` and `None`. `Tip_Update`'s *"Army Movement:"* arm is the reader.
    fn mode_screen_id(&self) -> Option<u8> {
        self.move_order.is_some().then_some(0x10)
    }

    /// `g_minimapMode`, 0 owners … 3 happiness, for the tool tips.
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
        // **The yes/no box is modal**: `Ui_OpenConfirm` sets `g_screenId =
        // 0x1E`, so none of the map's arms run under it. Kind 5 — the press
        // starts the gauntlet's countdown and `Screen::update` gives the
        // answer; the arms are declared on `CONFIRM_WIDGETS`.
        if ctx.game.combine_ask.is_some() {
            let fired = self.press.event(&CONFIRM_WIDGETS, event);
            if fired.is_some() || self.press.busy() {
                self.scrolled = true;
            }
            return Transition::Stay;
        }
        // **While a turn is being wound on the map is a spectator.** Every
        // hotspot it offers writes to state the phase machine is in the middle
        // of reading,
        // motion still gets through, because a frozen cursor reads as a hang
//
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
            // **No `// arm:` marker yet**: `docs/arms.json` still files
            // `0x004B29BE/escape-quits-outside-the-game` as *missing*, and
            // `tests/arms.rs` rejects a marker for a record that says so. The
            // half built here is the `g_appPhase == 3` branch; the record is
            // the lead's to flip.
            Event::KeyDown(Key::Escape) => {
                return Transition::Push(ScreenId::Confirm(confirm::Ask::Quit))
            }
            // **Ours, and only the key is.** The original has no keyboard route
            // into a county panel at all; the strip's quadrants are it. Enter
            // opens the one the strip's bottom-left quadrant opens.
            // panel's own Up/Down cycle the other three.
            Event::KeyDown(Key::Enter) => {
                if ctx.game.selected != 0 {
                    return Transition::Push(ScreenId::County(
                        ctx.game.selected,
                        county::Panel::Tax,
                    ));
                }
            }
            // The original scrolls by pushing the pointer into the edge of the
            // *desktop* (`Map_EdgeScroll`), which a headless test cannot do and
            // a windowed player would find surprising today. The directions,
            // the step
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
// **A shortcut, now that the route works.** The original opens
            // the village by clicking the county's *town*: `Map_Click` tests
            // plane-0 bit `0x40`, checks the county is the local player's,
            // centres the map on it and sets `g_screenId = 2`. That arm is
            // wired above — this key is a convenience for a county whose town
            // is off-screen, and it is ours.
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
                // `Game::open_levy` is `Sidebar_Button`'s own body: the county
                // check, `Levy_SetPercent` at the slider's last position, and
                // the basket seeded before the screen id moves.
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
            // **Ours entirely.** The original has no cycle key; it has a map
            // you can see the whole of at zoom 2. Ours is here because an army
            // three screens away is otherwise unreachable without scrolling.
            Event::KeyDown(Key::Char('N')) => self.cycle_unit(ctx),
            // Ours, and marked as such where it lands: the demo's index of
            // every screen, so the ones the game logic cannot yet open can
            // still be walked. `screens::index`.
            Event::KeyDown(Key::Char('I')) => return Transition::Push(ScreenId::Index),
            // **Ours, and only the key is.** The original reaches `0x35` and
            // `0x36` through the menu bar's Game drop-down (`Menu_LoadGame` and
            // `Menu_SaveGame`, which save `g_screenId` into `g_screenIdSaved`
            // so that `SaveLoad_Cancel` can put it back — which is what
            // `Transition::Pop` does here). The drop-down is not drawn yet, so
            // the destinations are the original's
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
                // `Map_HoverUnitTarget`. It runs once a frame in the original,
                // out of `Screen_DrawWidgets`'s `0x10` arm — where every other
                // screen draws its widget table, this one recomputes the route
                // under the cursor — and its first act is to compare the hovered
                // tile with the last one and do nothing if it has not changed.
                // Driving it from pointer motion is that comparison, made by the
                // event loop
                self.update_hover_path(x, y);
                // The slider's whole gesture: held **and** moved, tested
                // against the rectangle again every time, which is what lets
                // the pointer wander off the sidebar and come back without
                // letting go. See [`SPLIT_SLIDER`].
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
// **On the map the right button opens**, not closes.
            // `Screen_FrameInput`'s `g_screenId == 0` arm ends with
            // `if (onATile && rightReleased) { g_screenId = 4; FUN_0043CAF4(); }`
            // — screen `0x04`, the information panel, which
            // `FUN_0043CAF4` fills by picking whatever is under the cursor:
            // `UnitPanel_Draw` for a unit, `FUN_0041BEFE` for a bare tile. The
            // shipped `Readme.txt` errata describes exactly this — *"Right
            // clicking on an army accesses an information pop-up that includes
            // the army's county of origin"* —
            // `Eng_DrawString(31, 9, ...)`, *"An army from"*, followed by group
            // 100 indexed `homeCounty + scenarioIndex * 20`.
            //
            // It is **not** gated on hitting a unit, or on owning anything. The
            // gate is `Map_PickTile` finding a tile at all, which is our map
            // clip. A right-click on the sidebar reaches the minimap-mode
            // clear instead, and we have no minimap modes to clear.
            // **Guard 2 of the arm, and it is tested before anything else the
            // right button does** — including the information panel below.
            // `FUN_00439079` consumes the click only when an overlay is up, so
// with no overlay this falls through
            // `return 0` does.
            Event::RightClick { x, y } if self.clear_minimap_mode(x, y) => {}
            Event::RightClick { x, y } if self.map_clip().contains(x, y) => {
                // **This is how an army is deselected, and it was missing.**
                //
                // A player found it in a minute: *"you cannot deselect an
                // army."* The information panel above is screen `0`'s arm, and
                // while an army is picked the screen is `0x10`, whose entire
                // right-button clause is
                //
                //     if (g_mouseRightReleased != 0) {
                //         g_screenId = 0; g_redrawRequest = 2; }
                //
                // — leave move-order mode, redraw, and that is all. No
                // information panel, no confirmation. Ours reached the panel
                // instead because the arm was written for one screen
// mode it belongs to
                //
                // Note what is *not* here: a click on empty ground does **not**
                // cancel. In the original that click is `Map_ConfirmMoveOrder`
                // and it either places the order or does nothing at all. Adding
                // "click away to deselect" would be the same invention as the
                // tax-panel convenience removed above, made in the opposite
                // direction.
                // arm: 0x0042FF10/move-order-right-cancels right-release
                if self.selected_unit.is_some() {
                    self.cancel_move_selection();
                    self.status = "ORDERS CANCELLED".into();
                    return Transition::Stay;
                }
                // **`0x04` is a real screen now** and it needs to know what the
                // right click resolved to, because the original picks between
                // its two painters on `g_pickedTileUnit`: a unit under the
                // cursor gets the unit panel, anything else gets the tile
                // panel and — on the player's own farmland — the field brush.
                // See [`crate::screens::info`].
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
                // Three guards.
                // **right-clicking a unit selects nothing**, because the panel
                // that comes up is about the army and not about the ground.
                // The town is fetched before the selection
                // is inside `townTile != 0`,
                // shows its panel and leaves the sidebar where it was — the
                // same shape `Map_Click`'s industry arm has, without that
                // arm's outright refusal.
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
            // **Ours.** `winit` has no "the pointer left"; `main.rs` synthesises
            // this so a cursor that walked off the window stops scrolling the
            // map from wherever it was last seen.
            Event::PointerLeft => {
                self.pointer_in = false;
                self.focus = Focus::None;
            }
            // `FUN_00439122` eats the release and does nothing with it — the
            // value was already set on the way down and on every move since.
            // All the release does is end the drag.
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
// It is here because it is
                // here in the original: the scroll's own arm returned zero and
                // let the click through, and this is what the click found.
                // arm: 0x00476710/map-click-dismiss left-release
                if ctx.game.messages.is_open() {
                    ctx.game.messages.dismiss_unless_question();
                    return Transition::Stay;
                }
                // **`Sidebar_ButtonClicked` is guard 3 and it is OUTSIDE the
                // turn-ended gate**, which the menu bar below it is inside. So
                // the five icons and End Turn keep working once the turn has
                // been ended and nothing else in the column does. Verbatim, the
                // `g_screenId == 0` arm:
                //
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
                    // `Screen_FrameInput`'s forty-nine arms open with
                    // `if (DAT_0055403C != 0 && !debugOverride)` and force-close.
                    // The observable result is the same and it has to happen
                    // here, because this button is reachable *through* a county
                    // panel and `Machine::update` ticks only the top screen — a
                    // turn started from under a panel would be a turn nothing
                    // wound on.
                    //
                    // `docs/arms.json` `0x0042FF10/force-close-on-turn-end` is
                    // the general arm and stays `missing`: this is one screen's
                    // corner of it, not the guard.
                    // arm: 0x0043AC23/end-turn left-press
                    let t = self.end_turn(ctx);
                    return if t == Transition::Stay { Transition::Reveal } else { t };
                } else if let Some(b) = SIDEBAR_BUTTONS.iter().find(|b| b.rect().contains(x, y)) {
                    // `g_sidebarButtons`. Three of the five reach a screen we
                    // can draw; the other two name the function the original
                    // dispatches to and do nothing, which is the honest state.
                    // Three of the five are gated on the county being yours,
                    //
                    // Each of the five is an arm of its own: `Sidebar_Button`
                    // (`0x0043AE30`) is a five-way `if` on `g_uiHotspotId`.
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
                    // **The ownership gate is the function's second line** —
                    // `if (g_counties[g_selectedCounty].owner == g_localPlayer)`
                    // — and this tested only `selected != 0`, so the slider
                    // moved another lord's peasants.
                    // arm: 0x00439122/split-slider drag
                    self.slider_held = true;
                    self.drag_split(ctx, x);
                } else if let Some(job) = self.job_row_at(&*ctx, x, y) {
                    // **`CountyStrip_JobClick` (`0x00438E3B`)** — the produce
                    // rows on the 162 × 128 plate at y = 302, which open the job
                    // popup for that row. Another of this module's header's
                    // three "not reproduced" lines.
                    // arm: 0x00438E3B/job-rows left-press
                    return Transition::Push(ScreenId::Job(ctx.game.selected, job));
                } else if let Some(panel) = county::panel_at(x, y) {
                    // **The county strip is a 2 x 2 hotspot and it is the whole
// navigation into the four county panels** —
                    // other way into any of them, in the original or here
                    // (`docs/screens-county.md` §2.3). It used not to be tested
// on this screen at all. A player could reach
                    // tax and nothing else: our own COUNTY PANEL button opened
                    // the county screen on its own default.
                    // arm: 0x00438CEB/strip-population left-press
                    // arm: 0x00438CEB/strip-happiness left-press
                    // arm: 0x00438CEB/strip-tax left-press
                    // arm: 0x00438CEB/strip-ration left-press
                    if ctx.game.selected != 0 {
                        return Transition::Push(ScreenId::County(ctx.game.selected, panel));
                    }
                } else if chrome::minimap_hit_area().contains(x, y) {
                    // `Minimap_Click`: the county raster decides, then
                    // `Map_CentreOnTile` moves the viewport onto it.
                    //
                    // **It is reached from every screen, not only this one.**
                    // `Screen_FrameInput`'s epilogue runs it on any press with
                    // `g_screenId != 0x12`, and closes the management surface on
                    // a hit — so this arm is live under a county panel, the job
                    // popup and a shell. Our stack says that with
                    // [`Transition::Reveal`]: the overlay passes the press down,
                    // this runs,
                    //
                    // `Minimap_Click` returns **1 for any press inside the
                    // raster**, county or no county, so the surface closes even
                    // where the raster is blank. And it returns 0 outright while
                    // `g_screenId` is `0x05` or `0x06` — the village's two drag
// screens — the village keeps a peasant drag
//
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
                    // **At the far zoom the left button does not select
                    // anything — it zooms in on the tile under it.**
                    //
                    // ```c
                    // if (picked && leftPressed && g_mapZoom == 2) {
                    //     g_screenId = 0; Map_ZoomInAtTile(); return;
                    // }
                    // ```
                    //
// The arm **returns**, not falls through.
                    // `Map_Click` is unreachable at zoom 2 and every tile arm
                    // below — the village, the industry switch, the field
                    // brush, selecting a county — is dead there. Ours had no
                    // click arm at all at the far zoom,
                    // near zoom's click does, on a tile ten pixels wide.
                    //
                    // It sits inside the `g_screenId == 0` arm, so it is *not*
                    // live in move-order mode (`0x10`), whose own four-line arm
                    // has no zoom test: `self.selected_unit` is that screen id
// here. It is guarded first.
                    //
                    // `Map_ZoomInAtTile` (`0x004350A1`) centres the near view on
                    // the picked tile with the same `col - 4`, `(row & ~1) - 12`
                    // arithmetic as `Map_CentreOnTile`, which is
                    // [`MapScreen::centre_on_tile`].
                    // arm: 0x0042FF10/map-zoom-in-at-tile left-press
                    if self.selected_unit.is_none() && self.zoom.id == FAR.id {
                        if let Some((tx, ty)) = self.pick_tile(x, y) {
                            self.set_zoom(ctx, NEAR);
                            self.centre_on_tile(tx as usize, ty as usize);
                            self.status = "ZOOMED IN".into();
                            return Transition::Stay;
                        }
                    }
                    // **Move-order mode is a screen, not a flag, and that is the
                    // whole reason this block is shaped the way it is.**
                    //
                    // `Screen_FrameInput` dispatches on `g_screenId`, and while
                    // an army is picked that is `0x10`, not `0`. So the map's
                    // own arm — `Map_Click`, every hotspot below, the sidebar
                    // guards, the minimap — **is not reachable at all**. The
                    // `0x10` arm is four lines long: the turn-ended latch,
                    // `Map_EdgeScroll`, a left press that is
                    // `Map_ConfirmMoveOrder`, and a right release that leaves.
                    //
                    // We had this as a flag consulted *after* the unit hit test,
// so clicking a second army re-selected it. In the
                    // original that click is a destination: the second army is
                    // `g_hoverMergeUnit`
                    if let Some(unit) = self.selected_unit {
                        if !ctx.game.is_players_unit(unit) {
                            // The turn-ended latch's civilian cousin: the
                            // selection's owner changed under it.
                            self.cancel_move_selection();
                        // arm: 0x0042FF10/move-order-confirm left-press
                        } else if let Some(dest) = self.pick_tile(x, y) {
                            return self.confirm_move_order(ctx, unit, dest);
                        } else {
                            self.cancel_move_selection();
                            return Transition::Stay;
                        }
                    }
                    // **`Map_Click` tests the picked *unit* before it tests any
                    // tile flag**, and both of its unit branches return without
                    // ever reaching the terrain dispatch below. That order is
                    // the rule: an army standing on your own farmland is an
                    // army, not a field.
                    // **A castle is a move *target*, not a unit**, and it is
                    // tested first because the garrison inside it stands on the
                    // castle tile.
                    //
                    // `Map_HoverUnitTarget` collects a plane-0 `0x80` tile with
                    // terrain above `0x14` as a target for the selected army —
                    // as `g_hoverGarrisonCounty` when the county is the mover's
                    // and as `g_hoverSiegeCounty` when it is not, raising
                    // `L2.eng` 10/7 *"Garrison castle?"* or 10/8 *"Besiege
                    // castle?"*. The original never offers the garrison itself,
                    // because **it does not draw a garrisoned unit at all** — it
                    // flies a flag over the castle instead. Ours draws a hollow
                    // marker so a player can see his men are in there.
                    // marker sat on top of the only route to a siege: clicking
                    // an enemy castle selected its garrison, and there was no
                    // way to order an army to besiege anything.
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
                    // **A click on one of your own buildings or fields takes
                    // precedence over selecting the county**, and in that
                    // order, which is `Map_Click`'s own plane-0 dispatch:
                    // `0x80` is a settlement and switches its industry, `0x40`
                    // is the county town and opens the village, `0x20` is
                    // farmland and opens the field brush. All three are gated
                    // on the county being the local player's.
                    if county != 0 && ctx.game.is_players(county) {
                        // arm: 0x0043CE1A/industry-toggle left-release
                        if let Some(tile) = self.settlement_at(ctx, county, x, y) {
                            // **The industry arm selects the county first**, and
                            // it is the only one of the three flag arms that
                            // guards on the selection:
                            //
                            //     if (pickedCounty != g_selectedCounty) {
                            //         if (townTile == 0) return;
                            //         g_selectedCounty = pickedCounty;
                            //         Map_CentreOnTile(townTile);
                            //     }
                            //
                            // A county with no town refuses the toggle outright
                            // — the `return` is before `Industry_ToggleFromMap`.
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
                                    // **`Industry_ToggleFromMap`'s last
                                    // statement**, which was missing entirely:
                                    // *"there's no message saying or visually
                                    // showing mining on / mining off."*
                                    //
                                    // `Msg_Enqueue(0, g_localPlayer, group, 0,
                                    // 0x04, 0, 0, 0)` — a **floating tip**, so
                                    // the words appear by the cursor, carry no
                                    // OK button and time out on their own.
                                    // `industry::toggle_message_group` is the
                                    // id
                                    //
                                    // The guard is the original's own
                                    // `if (county.owner == g_localPlayer)` and
                                    // is already satisfied: this arm is inside
                                    // `ctx.game.is_players(county)`.
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
                                    // OURS, and it stays: the sidebar's status
                                    // line is this interface's only running
                                    // commentary and a player reading it should
                                    // not have to catch a tip that lasts a
                                    // hundred ticks.
                                    self.status = format!(
                                        "{} {}",
                                        toggle_name(what),
                                        if on { "ON" } else { "OFF" }
                                    );
                                }
                                // Terrain 13 … 20 — the empty castle plot —
                                // is the original's own `return`.
                                None => self.status = "NOTHING TO SWITCH THERE".into(),
                            }
                            return Transition::Stay;
                        }
                        // **`0x40` — the county town — opens the village.**
                        // `Map_Click`'s second arm: `g_screenId = 2;
                        // Village_Draw(1)`, after `Map_CentreOnTile` has put
                        // the town in the middle. This arm was missing,
                        // click on the town fell through to "select the county"
                        //
                        // arm: 0x0043CE1A/village left-release
                        if let Some(tile) = self.tile_at(x, y, Self::town(ctx, county).into_iter())
                        {
                            // `if (townTile != 0) { g_selectedCounty = picked;
                            // Map_CentreOnTile(townTile); ... }` — the selection
                            // is inside the guard, `g_screenId = 2` outside it.
                            // We had the recentre and not the selection.
                            let (tx, ty) = l2_kingdom::map::coords(tile);
                            ctx.game.select(county);
                            self.centre_on_tile(tx as usize, ty as usize);
                            return Transition::Push(ScreenId::Village(county));
                        }
                        // **`0x20` — farmland — opens screen `0x04`, the panel a
                        // RIGHT click opens.** `Map_Click`'s third flag arm:
                        //
                        // ```c
                        // if ((flags & 0x20) && g_counties[pickedCounty].owner == g_localPlayer) {
                        //     _DAT_005681CC = 3; g_screenId = 4; FUN_0041B032();
                        // }
                        // ```
                        //
                        //
                        // `_DAT_005681CC = 3`
                        // picks the tile half on `g_pickedTileUnit == 0`. So on
                        // your own field **the two buttons open one screen**, the
                        // player's own words: *"right click and left click on
// fields does the same thing in the game."* Ours
                        // opened a popup of our own here instead
                        // (`ours/brush-popup-on-the-map`, removed); a left click
                        // on somebody else's field falls out of the bottom, as the
                        // original's owner test inside the arm makes it.
                        //
                        // The tile is `Map_PickTile`'s, the same one the right
                        // button resolves, so the two gestures cannot disagree
                        // about which field was meant.
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
// **And that is the end of `Map_Click`.
                    // below this one.**
                    //
                    // A click that reaches here — plain ground, sea, a county
                    // that is not yours, your own county away from its town, its
                    // fields and its buildings — falls out of the bottom having
                    // changed nothing at all. The last statement in the original
                    // is `else { DAT_0056D64C = 0; }`, a scroll latch.
                    //
                    // We had a county-selection arm here, and then a second
                    // click on the selected county opened its tax panel. A
                    // player reported it: *"there's some weird thing where if
                    // you click anywhere on grass it opens up the tax window
                    // too."* Both halves are gone. The three ways into a
                    // selection are the ones the original has: the county strip,
                    // the minimap.
                    // industry building or a merchant, each of which selects the
                    // county it belongs to on the way to opening something.
                    // `docs/decisions.md` C61.
                    let _ = county;
                }
            }
            _ => {}
        }
        Transition::Stay
    }

    /// One tick of edge scrolling, and nothing else.
    ///
/// It lives here because the gesture is *holding*
    /// the cursor against the edge: no further event arrives while it is held,
    /// so a scroller driven by events moves one step
    /// the clamp are `Map_ScrollStep`'s and `Map_ClampScroll`'s; the rate is
    /// one step per fixed tick, which is ours because the original's is a frame
    /// rate and nothing below this crate may read a clock.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // **`Panel_MoveButton`'s second statement.** The information panel
        // (`0x04`) writes `g_screenId = 0` and then calls
        // `Map_BeginMoveSelection()`; ours pops back to here and leaves the
        // request on [`crate::game::Game::begin_move_order`], because the
        // selection is this screen's state and not a global. Taken on the tick
        // after the pop, which is the same frame ordering the original has —
        // `Screen_FrameInput` runs last in a frame, so its navigation lands one
        // frame late by construction.
        if let Some(unit) = ctx.game.begin_move_order.take() {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            self.begin_move_selection(&read, unit);
        }
        // **`Widget_Test`'s countdown** — index 0 is the thumb up, index 1 the
        // thumb down, and the answer comes from the timer,.
        if let Some(widget) = self.press.tick().next() {
            return self.answer_combine(ctx, widget == 0);
        }
        // **The campaign stands still under the question**, the way it stands
        // still under screen `0x12`: `Ui_OpenConfirm` is a screen of its own in
        // the original, so `Units_Tick` below does not run while it is up.
        if ctx.game.combine_ask.is_some() {
            if self.press.any_pressed() {
                self.scrolled = true;
            }
            return Transition::Stay;
        }
        // **The turn is not wound here** — [`MapScreen::wind_turn`] is, and
        // the frame driver calls it whatever is on top. What is left below is
        // `Screen_FrameInput`'s half: the animation counters
        // which only run while this screen is the one being driven.
        // `Map_DrawFrame`: `if (0x7F < tick) tick = 0; phase = tick >> 4;`
        // Only a change of phase is a repaint,
// costs eight frames every 2.05 seconds.
        self.flag_tick = (self.flag_tick + 1) & 0x7F;
        let phase = self.flag_tick >> 4;
        if phase != self.flag_phase {
            self.flag_phase = phase;
            self.scrolled = true;
        }
        // The same frame steps the herd's counter, off the same gate.
        // `Map_DrawFrame`: `if (0x5F < tick) tick = 0; phase = tick >> 4;` —
        // note the wrap is `0x60`, not a mask, so the six phases are 0 … 5 and
        // the counter is **not** a power of two.
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
        // natural guess — that everything on screen shares one — is wrong about
        // all three pairs.
        {
            let read = Ctx { game: ctx.game, assets: ctx.assets };
            if self.step_industry(&read) {
                // A wheel that turned changes the **base plane**, unlike a flag
                // or a cow, so this is a repaint of the map and not only of the
                // overlays. `ensure`'s key picks it up.
                self.scrolled = true;
            }
        }
        // The season has turned
        // Nothing else may run: the fade *is* the frame.
        if self.fading.is_some() {
            return Transition::Stay;
        }
        // **`Map_ScrollThrottle` (`0x004BBBE3`)** — the map does not step on
        // every frame the pointer is at the edge. See
        // [`MapScreen::scroll_interval_ticks`].
        let every = self.scroll_interval_ticks();
        self.scroll_wait = self.scroll_wait.saturating_sub(1);
        // `q >= 10` — speed 0 — is the original's own early return, and no
        // amount of waiting satisfies it.
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

    /// **`Turn_Tick(); Units_Tick();`**, the frame loop's half of a frame.
    ///
    /// Everything here used to sit in [`MapScreen::update`], which the machine
    /// runs for the top screen only —
    /// menu stopped the campaign dead underneath it. `Battle_Frame`
    /// (`0x004B99C0`) has no screen test on this call at all; see
    /// [`Screen::wind_turn`] and [`crate::screen::Machine::wind_turn`].
    ///
    /// **The question the phase machine is waiting on is its own gate.**
    /// [`MapScreen::resume_turn`] asks for screen `0x12` or `0x13` before it
    /// ticks anything,
    /// however many frames go by — which is the original's `g_turnPhaseStep`
/// waiting on an answer.
    fn wind_turn(&mut self, ctx: &mut Ctx) -> Transition {
        // The season has turned
        // Nothing else may run: the fade *is* the frame.
        if self.fading.is_some() {
            return self.tick_fade();
        }
        // **The turn timer ran out.** `Turn_Tick`'s phase-4 arm called
        // `Turn_End` — this screen's End Turn handler —
        // closed what the turn-ended guard closes; the request waits here because
        // only this screen can start a turn. It goes through the button's own
        // door, and like the button's frame it is one tick of the turn and no
        // more. `crate::turn_clock`, `Machine::run_turn_clock`.
        if ctx.game.turn_clock.end_turn_pending() && !turn::turn_in_flight(ctx.game) {
            ctx.game.turn_clock.take_end_turn();
            return self.end_turn(ctx);
        }
        let resumed = self.resume_turn(ctx);
        if resumed != Transition::Stay {
            return resumed;
        }
        // A turn in flight winds itself on above and runs `Units_Tick` as part
        // of doing so; the sweep below must not run a second time in the same
        // frame, or every unit would take two tiles a tick and three of the
        // seven phases would settle early.
        if !turn::turn_in_flight(ctx.game) {
            // **`Units_Tick` on an ordinary frame.** This is what makes an army
            // the player has just ordered walk away while he watches, rather
            // than standing still until End Turn. See
            // [`turn::tick_units_only`].
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
        let ink = &ctx.assets.ink;
        let game = &ctx.game;
        let k = &game.kingdom;
        let clip = self.map_clip();

        canvas.pixels.copy_from_slice(&self.base.pixels);

        // **The yellow outline used to be drawn here, and it is gone.** It was
        // the visual half of the county-selection arm this module's header
        // records removing, and it survived that removal by being in the
// painter. `docs/agents.md`'s second
        // place behaviour hides. The original draws no selection on the map at
        // all: its borders are in the tile data (plane-0 bit `0x02`, the
        // `roads` bank's boundary frames) and its *selection* is which county
        // the right panel describes. `the_selection_is_not_drawn_on_the_map`
        // is the assertion that could not exist while it did.

        // Ours: one marker per county in view, at its anchor tile, coloured by
        // owner — **debug overlay only**. It stood in for the owner's banner
        // before `draw_flags` placed it; `Sprite_TopIt` draws the banner on the
        // town's quadrant 0
        // square on the town square the original never had.
        let debug = game.prefs.debug_overlay;
        for id in k.county_ids().filter(|_| debug) {
            let (ax, ay) = (game.anchor_x[id] as usize, game.anchor_y[id] as usize);
            // **In the dark, not at all.** This marker is ours, standing in for
            // the owner's banner `Sprite_TopIt` flies — and that banner is
            // behind the fog test, so an owner colour here would tell the
            // player what the original hides.
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
            // Ours, under the original's two lines: debug overlay only.
            if debug {
                text::draw(canvas, 24, FAR_BOX_ADVICE_Y + 14, &self.status, ink.text);
            }
        }

        // Ours: the player's own county's fields, marked by what each is being
        // used for — **debug overlay only**, because the original draws nothing
        // over a field but its own artwork and a herd. See [`brush`].
        if debug && game.is_players(game.selected) {
            for (tile, kind) in k.field_tiles(game.selected as usize) {
                // Ours, and kept out of the dark with everything else a tile
                // carries. A county of the player's is seen from the moment it
                // is his, so this skips nothing a player can reach.
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

        // `Map_DrawFrame`'s order: the terrain, then the building/flag pass,
        // then the unit sprites —
        // walking past it.
        // **Two loops where the original has one, and it shows in one place.**
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

        // `g_battlePhase == 0` — the campaign map is never up during a battle.
        draw_menu_bar(canvas, ctx, false);
        draw_right_panel(self, canvas, ctx);
        draw_unit_banner(self, canvas, ctx);
        draw_combine_box(self, canvas, ctx);
    }
}


