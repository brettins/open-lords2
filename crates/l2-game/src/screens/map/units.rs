use super::*;

impl MapScreen {
    pub fn pick_tile(&self, x: i32, y: i32) -> Option<(u8, u8)> {
        if !self.map_clip().contains(x, y) {
            return None;
        }
        // `Map_PickTile` (`0x00429BA4`) *"divides by `g_mapTileHalfStep` and
        // `g_mapRowStep`"*, which are the half **pitch**
        // and 15 near, 6 and 3 far. These were `tile_w / 2` and `tile_h / 2`,
        // which are 29 and 15: the near tile is 58 wide but the lattice pitch
        // is 60, so the diamonds were two pixels narrow and **did not tile the
        // plane** — 56 dead pixels around every tile centre.
        let (hw, hh) = (self.zoom.half_pitch, self.zoom.row_step);
        let dim = l2_kingdom::MAP_DIM;
        for ty in 0..dim {
            for tx in 0..dim {
                let Some((cx, cy)) = campaign::tile_centre(self.view, &self.zoom, tx, ty) else {
                    continue;
                };
                if (x - cx).abs() * hh + (y - cy).abs() * hw <= hw * hh {
                    return Some((tx as u8, ty as u8));
                }
            }
        }
        None
    }

    /// `Map_ResolvePick` (`0x0046D5FE`) reads it out of the tile record —
    /// `g_pickedTileUnit = g_tiles[t].unit` — so in the original a click
    /// **anywhere on a unit's tile** is that unit. This asked the unit's little
    /// *marker* instead, a box `unit_marker_half + 1` around the tile centre,
    /// which at near zoom is nine pixels across on a diamond that is 58 × 30.
    ///
    /// That is what a player reported as *"if I click a merchant while the map
    /// has a different county selected it will open up the tax window"*: the
    /// click missed the box, fell past the unit arm and past the settlement,
    /// town and field arms, and landed on our own "a second click on the
    /// selected county opens it". Same shape as the mine (`docs/decisions.md`
    /// C57)
    pub fn unit_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        if !self.map_clip().contains(x, y) {
            return None;
        }
        let units = &ctx.game.kingdom.campaign.units;
        if let Some((tx, ty)) = self.pick_tile(x, y) {
            if let Some(id) = units.iter().find(|(_, u)| u.x == tx && u.y == ty).map(|(id, _)| id) {
                return Some(id);
            }
        }
        units.iter().find_map(|(id, u)| {
            if u.is_garrisoned() {
                let (cx, cy) = campaign::tile_centre(self.view, &self.zoom, u.x as usize, u.y as usize)?;
                let r = unit_marker_half(&self.zoom, u) + 1;
                return ((x - cx).abs() <= r && (y - cy).abs() <= r).then_some(id);
            }
            self.unit_sprite_covers(ctx, id, u, x, y).then_some(id)
        })
    }

    pub(super) fn unit_sprite_covers(&self, ctx: &Ctx, id: usize, u: &l2_kingdom::Unit, x: i32, y: i32) -> bool {
        let Some((cx, cy)) =
            campaign::tile_centre(self.view, &self.zoom, u.x as usize, u.y as usize)
        else {
            return false;
        };
        let sprite = unit_sprite(&self.zoom, ctx.game, id, u);
        match campaign::unit_sprite_rect(&ctx.assets.map, self.view, &self.zoom, (u.x as usize, u.y as usize), sprite) {
            Some((ox, oy, decoded)) => {
                let (dx, dy) = (x - ox, y - oy);
                dx >= 0
                    && dy >= 0
                    && dx < decoded.width as i32
                    && dy < decoded.height as i32
                    && decoded.opaque[dy as usize * decoded.width as usize + dx as usize]
            }
            None => {
                let r = unit_marker_half(&self.zoom, u) + 1;
                (x - cx).abs() <= r && (y - cy).abs() <= r
            }
        }
    }

    pub fn selected_unit(&self) -> Option<usize> {
        self.selected_unit
    }

    /// * **From the map a besieging army never sees the "Lift the siege?"
    ///   prompt.** `Panel_MoveButton` raises `L2.eng` 10/13 when its unit is
    ///   besieging; the map does not reach `Panel_MoveButton` in that case at
    ///   all, it opens the siege screen instead. Two routes to one decision, and
    ///   only one of them asks.
    ///
    /// **`Map_BeginMoveSelection` (`0x0043723A`)** — the map enters move-order
    /// mode, `g_screenId = 0x10`.
    ///
    /// The whole of what it does that we can do: `g_selectedUnit = unit`, then
    /// `Move_FloodFill` from the unit's own tile. It also sets
    /// `g_moveOrderClickGuard` (`0x00553ECC`) to 40 — forty frames in which a
    /// left button that is *down* is not read as the destination, so that the
    /// press which opened the mode cannot also close it. **We do not need it
    /// and do not have it**: the original polls the button's level once a frame,
    /// where we are handed one [`Event::Click`] per press.
    pub(super) fn begin_move_selection(&mut self, ctx: &Ctx, unit: usize) {
        let Some(u) = ctx.game.kingdom.campaign.units.get(unit) else { return };
        let start = u.tile();
        let cost = ctx.game.kingdom.campaign.map.cost_map();
        let field =
            l2_kingdom::movement::flood_fill(&cost, start, l2_kingdom::movement::Routing::Direct);
        self.selected_unit = Some(unit);
        self.move_order =
            Some(MoveOrder { unit, cost, field, hovered: None, path: Vec::new() });
    }

    pub(super) fn cancel_move_selection(&mut self) {
        self.selected_unit = None;
        self.move_order = None;
    }

    /// **`Map_HoverUnitTarget` (`0x004A8E0B`) — the arm this project missed,
    ///
    /// The sprite sheet was read
    /// C61.
    ///
    /// **It had no marker and no record until the gesture-kind audit.** The arm
    /// was built by C61's branch and then counted by nothing: `arms.rs` checks
    /// that every record has a marker and every marker has a record, and an arm
    /// with neither is invisible to both directions of that check. It is a
    /// `hover` — the original runs it from `Screen_DrawWidgets`' `0x10` arm once
    /// a frame, where every other screen draws its widget table — so it has no
    /// kind byte
    ///
    /// // arm: 0x004A8E0B/hover-march-target hover
    ///
    /// Two economies of the original are kept because they are behaviour, not
    /// speed: the descent runs **only when the hovered tile changed**
    /// (`if (DAT_005691E0 != g_hoverTileOffset)`).
    ///
    // arm: 0x004A8E0B/hover-unit-target hover
    pub(super) fn update_hover_path(&mut self, x: i32, y: i32) {
        if self.move_order.is_none() {
            return;
        }
        // `g_hoverTileOffset >= 0x0FFF0000` — the pointer is not over a tile —
        // clears `g_moveOrderAvailable` and draws nothing.
        let tile = if self.map_clip().contains(x, y) { self.pick_tile(x, y) } else { None };
        let sel = self.move_order.as_mut().expect("checked directly above");
        if sel.hovered == tile {
            return;
        }
        sel.hovered = tile;
        sel.path = tile
            .and_then(|dest| {
                l2_kingdom::movement::extract_path(&sel.cost, &sel.field, dest)
            })
            .unwrap_or_default();
    }

    pub(super) fn click_unit(&mut self, ctx: &mut Ctx, unit: usize) -> Transition {
        // ```c
        // else if (g_counties[g_pickedTileCounty].owner == g_localPlayer) {
        //     DAT_00553C64 = g_pickedTileUnit;                  /* the trading unit */
        //     if (g_counties[g_pickedTileCounty].townTile != 0) {
        //         g_selectedCounty = g_pickedTileCounty;
        //         Map_CentreOnTile(g_counties[...].townTile);
        //         g_screenId = 8;
        //     }
        // } else Msg_Enqueue(..., 0x70, ...);
        // ```
//
        // merchant" — which `docs/screens.md` §6 and `docs/symbols.md` both said
// A merchant belongs to nobody and is
        // clickable while it stands in a county you own. C50.
        if ctx.game.kingdom.campaign.units.get(unit).map(|u| u.kind)
            == Some(l2_kingdom::UnitKind::Merchant)
        {
            return self.click_merchant(ctx, unit);
        }
        if !ctx.game.is_players_unit(unit) {
            self.status = "NOT YOUR UNIT".into();
            return Transition::Stay;
        }
        let besieging = {
            let l2_kingdom::Kingdom { counties, campaign, .. } = &mut ctx.game.kingdom;
            if campaign.units.get(unit).is_some_and(|u| u.besieging_county != 0) {
                l2_kingdom::siege::validate_link(counties, &mut campaign.units, unit);
            }
            ctx.game.kingdom.campaign.units.get(unit).is_some_and(|u| u.besieging_county != 0)
        };
        // arm: 0x0043CE1A/siege-preparation left-release
        if besieging {
            self.cancel_move_selection();
            return Transition::Push(ScreenId::Siege(unit));
        }
        // arm: 0x0043CE1A/unit-orders left-release
        self.begin_move_selection(ctx, unit);
        let (men, left) = ctx
            .game
            .kingdom
            .campaign
            .units
            .get(unit)
            .map_or((0, 0), |u| (u.men, u.moves_left()));
        self.status = format!("{men} MEN, {left} MOVES - CLICK A TILE TO MARCH");
        Transition::Stay
    }

    /// `DAT_00553C64`, which the original sets here, has **exactly one writer
    /// in the whole binary — this line** — and two readers, both in the
    /// merchant screen's price arithmetic. So the trading screen is reachable
    /// only by clicking a merchant on the map.
    ///
    /// // arm: 0x0043CE1A/merchant left-release
    pub(super) fn click_merchant(&mut self, ctx: &mut Ctx, unit: usize) -> Transition {
        let county = ctx.game.kingdom.campaign.units.get(unit).map_or(0, |u| u.county);
        if !ctx.game.is_players(county) {
            self.status = "THAT MERCHANT IS NOT IN ONE OF YOUR COUNTIES".into();
            return Transition::Stay;
        }
        let Some(&town) = Self::town(ctx, county).first() else {
            self.status = "THAT COUNTY HAS NO TOWN TO TRADE IN".into();
            return Transition::Stay;
        };
        let (x, y) = l2_kingdom::map::coords(town);
        ctx.game.select(county);
        self.centre_on_tile(x as usize, y as usize);
        self.cancel_move_selection();
        // `DAT_00553C64 = g_pickedTileUnit` — and this is the call that carries
// it,
        Transition::Push(ScreenId::Merchant(unit))
    }

    /// The original picks at most one confirmation out of the targets
    /// `Map_HoverUnitTarget` collected, in a fixed priority — slaughter
    /// villagers, destroy field, combine armies, garrison castle, besiege castle
    /// — and otherwise lets the order through. Those are `L2.eng` group 10
    /// indices 4, 10, 5, 7 and 8.
    ///
    /// ```c
    /// if ((g_mouseLeftPressed != '\0') && (g_moveOrderClickGuard < 1)) {
    ///     g_screenId = 0; DAT_0056D64C = 1; Map_ConfirmMoveOrder(); }
    /// ```
    pub(super) fn confirm_move_order(&mut self, ctx: &mut Ctx, unit: usize, dest: (u8, u8)) -> Transition {
        let available = self.move_order.as_ref().is_some_and(|sel| {
            sel.field.cost_to(dest.0, dest.1).is_some_and(|d| d > 0)
        });
        self.cancel_move_selection();
        if !available {
            self.status = "THE ARMY IS ALREADY THERE".into();
            return Transition::Stay;
        }
        self.order_march(ctx, unit, dest)
    }

    pub(super) fn order_march(&mut self, ctx: &mut Ctx, unit: usize, dest: (u8, u8)) -> Transition {
        match ctx.game.order_unit_move(unit, dest) {
            Some(0) => self.status = "THAT TILE CANNOT BE REACHED - THE ARMY STANDS".into(),
            Some(steps) => {
                let left = ctx
                    .game
                    .kingdom
                    .campaign
                    .units
                    .get(unit)
                    .map_or(0, |u| u.moves_left());
                self.status = format!(
                    "MARCHING TO {},{} - {steps} STEPS, {left} MOVES LEFT",
                    dest.0, dest.1
                );
            }
            None => self.status = "NO ROAD THAT WAY - NOTHING ORDERED".into(),
        }
        Transition::Stay
    }
}
