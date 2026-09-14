use super::*;

impl MapScreen {
    pub fn new() -> MapScreen {
        // `Map_InitMode`: zoom 0, scroll origin row 0x4A col 0x14.
        let view = Viewport::START.clamped(&NEAR);
        MapScreen {
            zoom: NEAR,
            view,
            saved: view,
            base: Canvas::screen(),
            tags: Tags::screen(),
            built: None,
            industry_sites: Vec::new(),
            industry_slot: None,
            industry_tick: 0,
            industry_gate_ms: 0,
            minimap: None,
            minimap_slot: None,
            focus: Focus::None,
            status: "CLICK A COUNTY".into(),
            pointer: (CANVAS_W / 2, CANVAS_H / 2),
            pointer_in: false,
            scrolled: false,
            opened: false,
            flag_tick: 0,
            flag_phase: 0,
            herd_tick: 0,
            herd_phase: 0,
            selected_unit: None,
            move_order: None,
            slider_held: false,
            minimap_mode: MinimapMode::Owner,
            fading: None,
            autosave: false,
            gold_at_turn_start: 0,
            scroll_speed: DEFAULT_SCROLL_SPEED,
            scroll_wait: 0,
            press: Press::default(),
        }
    }

    /// **`MoveOrder_ConfirmCombine` (`0x004A975D`)** — the *"Combine armies?"*
    /// callback,
    ///
    /// Yes is `Army_Combine(g_hoverMergeUnit, mover)` (`0x004AA181`): the
    /// mover folds **into the standing army** and its slot is gone, which is
    /// the direction the original's fifth `Unit_OrderMove` argument fixes. No
    /// leaves the mover where `Entry::Occupied` stopped it — the halt that
    /// stood here before the question did.
    ///
    /// The original tests the 1500 cap *before* the order and says `L2.eng`
    /// 274 when it fails; ours meets it here, in
    /// [`l2_kingdom::unit::combine`]'s own refusal,
    /// status line because the map has no message scroll yet.
    pub(super) fn answer_combine(&mut self, ctx: &mut Ctx, yes: bool) -> Transition {
        let Some((mover, occupant)) = ctx.game.combine_ask.take() else { return Transition::Stay };
        self.scrolled = true;
        if !yes {
            self.status = "THE ARMY STANDS".into();
            return Transition::Stay;
        }
        let units = &mut ctx.game.kingdom.campaign.units;
        self.status = match l2_kingdom::unit::combine(units, occupant, mover) {
            Ok(men) => format!("COMBINED - {men} MEN"),
            // `L2.eng` 274, the original's own refusal for the cap.
            Err(l2_kingdom::unit::CombineRefusal::TooMany) => {
                ours_or(ctx.assets.shell.text(274, 0), "OVER THE LIMIT OF 1500 TROOPS")
            }
            // `L2.eng` 167: the mercenaries will not fight together.
            Err(l2_kingdom::unit::CombineRefusal::TwoMercenaryBands) => ours_or(
                ctx.assets.shell.text(167, 0),
                "THESE MERCENARIES WILL NOT FIGHT TOGETHER",
            ),
            Err(l2_kingdom::unit::CombineRefusal::NotAnArmy) => "NOTHING TO COMBINE".into(),
        };
        Transition::Stay
    }

    /// `Minimap_ModeButton` (`0x0043AB76`), button 0…3 of
    /// [`MINIMAP_MODE_BUTTONS`].
    ///
/// **This is
    /// shape of it:
    ///
    /// * in mode 0, buttons 1…3 select their mode and button 4 toggles the map
    ///   zoom;
    /// * in any other mode, **button 4 turns the overlay off** and buttons 1…3
    ///   do nothing at all.
    ///
/// the overlay
    /// has to be turned off first. The artwork agrees — `Misc_cty` frame `0x5B`,
    /// the strip drawn while a mode is up, has one button on it where frame
    /// `0x5C` has four.
    pub(super) fn minimap_mode_button(&mut self, ctx: &mut Ctx, button: usize) {
        if self.minimap_mode == MinimapMode::Owner {
            match MinimapMode::from_button(button) {
                // arm: 0x0043AB76/minimap-mode-set left-press
                Some(mode) => {
                    self.minimap_mode = mode;
                    self.status = format!("MINIMAP {}", minimap_mode_name(mode));
                }
                // arm: 0x0043AB76/minimap-zoom-toggle left-press
                None => self.toggle_zoom(ctx),
            }
        } else if button == 3 {
            // arm: 0x0043AB76/minimap-mode-clear left-press
            self.minimap_mode = MinimapMode::Owner;
            self.status = "MINIMAP OWNERS".into();
        }
    }

    /// **`FUN_00439079` (`0x00439079`)** — a right release anywhere in
    /// `x >= 0x1DE, 0x18 <= y < 0x99` with a minimap overlay up **clears the
    /// overlay and swallows the click**, and does nothing at all when no overlay
    /// is up.
    ///
    /// It is guard 2 of the `g_screenId == 0` arm, ahead of the sidebar and
    /// *outside* the turn-ended gate, and it is also guard 6 of the village's
    /// and of all four county panels'. This module's header carried it as *"not
    /// reproduced"*.
    ///
    /// The rectangle is the **minimap and its button strip**, not the whole
    /// column: y stops at 152, which is four pixels above the county strip's
    /// plate at 156.
    pub(super) fn clear_minimap_mode(&mut self, x: i32, y: i32) -> bool {
        if x < PANEL_X || !(0x18..0x99).contains(&y) || self.minimap_mode == MinimapMode::Owner {
            return false;
        }
        // arm: 0x00439079/right-clears-minimap-mode right-release
        self.minimap_mode = MinimapMode::Owner;
        self.status = "MINIMAP OWNERS".into();
        true
    }

    /// `CountyStrip_JobClick`'s ownership gate and its geometry, which is
    /// [`county::job_row_at`].
    pub(super) fn job_row_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        if !ctx.game.is_players(ctx.game.selected) {
            return None;
        }
        let c = ctx.game.kingdom.counties.get(ctx.game.selected as usize)?;
        county::job_row_at(c, x, y)
    }

    /// Set `g_optScrollSpeed`. 0 … 100; 0 disables scrolling, as it does in the
    /// original. See [`MapScreen::scroll_interval_ticks`].
    pub fn set_scroll_speed(&mut self, speed: i32) {
        self.scroll_speed = speed.clamp(0, 100);
        self.scroll_wait = 0;
    }

    /// **`Map_ScrollThrottle` (`0x004BBBE3`), in ticks.**
    ///
    /// The original:
    ///
    /// ```c
    /// elapsed = timeGetTime() - g_lastScrollTick;
    /// q = (100 - g_optScrollSpeed) / 10;
    /// if (q >= 10) return 0;                       /* speed 0 never scrolls */
    /// if (g_screenId == 0x10) q += 2;
    /// if (q * 12 + 2 > elapsed) return 0;
    /// g_lastScrollTick = timeGetTime();  return 1;
    /// ```
    ///
    /// So the interval is **`((100 − speed) / 10) × 12 + 2` milliseconds**, the
/// remainder is discarded, and `Map_EdgeScroll` itself
    /// is called unconditionally every frame — the *detection* runs at frame
    /// rate and only the *movement* is gated. `g_optScrollSpeed` is a 0 … 100
    /// slider in steps of ten shown as 0 … 10.
    /// options-defaults routine at `0x004AE310` is **60**, which is 50 ms,
    /// which is **20 tiles a second**. **[V]** — decoded from the binary.
    ///
    /// Ours had no throttle at all and scrolled one tile per fixed tick, which
    /// is 62.5 a second: **three times too fast**. A player said *"mouse scroll
    /// needs to be like… half that speed, not sure if it's a game default or
    /// some cycle thing"*, and it was both — there is a game default and it is
    /// applied on a timer.
    ///
    /// # The quantisation is ours and this is it
    ///
    /// Nothing below `main.rs` may read a clock (`docs/netcode.md`), so the
    /// millisecond interval becomes a whole number of [`TICK_MS`] ticks,
    /// **rounded to nearest** and never below one. At the default that is 3
    /// ticks — 48 ms, 20.8 tiles a second against the original's 20.0. Rounding
/// keeps it close:
    /// 64 ms and visibly slower than the game.
    pub(super) fn scroll_interval_ticks(&self) -> u32 {
        let speed = self.scroll_speed.clamp(0, 100);
        let q = (100 - speed) / 10;
        if q >= 10 {
            // Speed 0 disables scrolling outright, which is a real setting and
            // not a degenerate one: `Map_ScrollThrottle` returns 0 for ever.
            return u32::MAX;
        }
        let ms = (q * 12 + 2) as u32;
        ((ms + TICK_MS / 2) / TICK_MS).max(1)
    }

    /// One frame of the farm/industry slider: `FUN_00439122`'s body, once the
    /// button is known to be down
    pub(super) fn drag_split(&mut self, ctx: &mut Ctx, x: i32) {
        let id = ctx.game.selected as usize;
        let Some(current) = ctx.game.kingdom.counties.get(id).map(|c| c.industry_share) else {
            return;
        };
        let next = split_from_click(x, current);
        // `if (next == share) return 1;` — the original checks and skips the
        // recompute, which matters here for the same reason: dragging along
        // one snapped step must not rerun the allocator on every pixel.
        if next != current && ctx.game.kingdom.set_industry_share(id, next) {
            self.status = format!("INDUSTRY {next}% FARM {}%", 100 - next);
        }
    }

    /// **Open the map where the player's own county is** — correction C48.
    ///
    /// `Map_InitMode` puts the scroll origin at row `0x4A`, column `0x14`, and
    /// we reproduced that faithfully and stopped there. The original does not:
    /// the last thing `Game_SetupRealmsAndCounties` (`0x0049BD99`) does, after
    /// every realm has its county and its starting garrison, is
    ///
    /// ```c
    /// FUN_00432746(g_playerStartTable[g_localPlayer * 2]);   /* 0x00432746 */
    ///     -> if (county.townTile) { Map_CentreOnTile(county.townTile);
    ///                               g_selectedCounty = county; }
    /// ```
    ///
    /// so a new game opens looking at **the player's own town**
    /// `0x4A`. Ours opened on a stretch of England the player owned nothing in:
    /// on the turn-one fixture the near view is eight lattice columns wide and
    /// county 8's town is fourteen columns outside it, so his county, his
    /// merchants
    /// That is the second half of *"I raised an army and nothing appeared"*;
    /// the first half is `l2_kingdom::levy::muster_tile`, C47.
    ///
    /// **Two departures, both deliberate.** The original does this at
    /// `Game_NewGame` time and we do it the first time the campaign screen is
    /// built, because a screen is constructed from a [`ScreenId`] with no game
    /// in hand. And it centres on the *start* county from `g_playerStartTable`,
    /// which a loaded position does not carry; we centre on the selected county
    /// when it is the player's and otherwise on his lowest-numbered one, which
    /// is the same county on turn one.
    ///
    /// The guard is the original's too: `FUN_00432746` does nothing at all when
    /// the county's town tile is zero,
    /// synthetic map in the test suite — stays exactly where `Map_InitMode`
    /// left it.
    pub(super) fn open_on_the_player(&mut self, ctx: &Ctx) {
        self.opened = true;
        let g = &ctx.game;
        let county = if g.is_players(g.selected) {
            g.selected
        } else {
            match g.kingdom.county_ids().into_iter().find(|&c| g.is_players(c as u8)) {
                Some(c) => c as u8,
                None => return,
            }
        };
        // `g_counties[c].townTile`, whose zero means "no town".
        let Some(&tile) = Self::town(ctx, county).first() else { return };
        let (x, y) = l2_kingdom::map::coords(tile);
        self.centre_on_tile(x as usize, y as usize);
        self.saved = self.view;
    }
}
