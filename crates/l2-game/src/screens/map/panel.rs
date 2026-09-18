use super::*;

impl MapScreen {
    pub fn new() -> MapScreen {
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
    pub(super) fn clear_minimap_mode(&mut self, x: i32, y: i32) -> bool {
        if x < PANEL_X || !(0x18..0x99).contains(&y) || self.minimap_mode == MinimapMode::Owner {
            return false;
        }
        // arm: 0x00439079/right-clears-minimap-mode right-release
        self.minimap_mode = MinimapMode::Owner;
        self.status = "MINIMAP OWNERS".into();
        true
    }

    pub(super) fn job_row_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        if !ctx.game.is_players(ctx.game.selected) {
            return None;
        }
        let c = ctx.game.kingdom.counties.get(ctx.game.selected as usize)?;
        county::job_row_at(c, x, y)
    }

    pub fn set_scroll_speed(&mut self, speed: i32) {
        self.scroll_speed = speed.clamp(0, 100);
        self.scroll_wait = 0;
    }

    /// **`Map_ScrollThrottle` (`0x004BBBE3`), in ticks.**
    ///
    /// options-defaults routine at `0x004AE310` is **60**, which is 50 ms,
    /// which is **20 tiles a second**. **[V]** — decoded from the binary.
    pub(super) fn scroll_interval_ticks(&self) -> u32 {
        let speed = self.scroll_speed.clamp(0, 100);
        let q = (100 - speed) / 10;
        if q >= 10 {
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
        if next != current && ctx.game.kingdom.set_industry_share(id, next) {
            self.status = format!("INDUSTRY {next}% FARM {}%", 100 - next);
        }
    }

    /// **Open the map where the player's own county is** — correction C48.
    ///
    /// the last thing `Game_SetupRealmsAndCounties` (`0x0049BD99`) does, after
    /// every realm has its county and its starting garrison, is
    ///
    /// ```c
    /// FUN_00432746(g_playerStartTable[g_localPlayer * 2]);   /* 0x00432746 */
    ///     -> if (county.townTile) { Map_CentreOnTile(county.townTile);
    ///                               g_selectedCounty = county; }
    /// ```
    ///
    /// on the turn-one fixture the near view is eight lattice columns wide and
    /// county 8's town is fourteen columns outside it, so his county, his
    /// merchants
    /// That is the second half of *"I raised an army and nothing appeared"*;
    /// the first half is `l2_kingdom::levy::muster_tile`, C47.
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
        let Some(&tile) = Self::town(ctx, county).first() else { return };
        let (x, y) = l2_kingdom::map::coords(tile);
        self.centre_on_tile(x as usize, y as usize);
        self.saved = self.view;
    }
}
