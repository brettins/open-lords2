use super::*;

impl MapScreen {
    pub(super) fn tile_at(&self, x: i32, y: i32, candidates: impl Iterator<Item = usize>) -> Option<usize> {
        if !self.map_clip().contains(x, y) {
            return None;
        }
        let (hw, hh) = (self.zoom.tile_w as f32 / 2.0, self.zoom.tile_h as f32 / 2.0);
        for tile in candidates {
            let (tx, ty) = l2_kingdom::map::coords(tile);
            let Some((cx, cy)) =
                campaign::tile_centre(self.view, &self.zoom, tx as usize, ty as usize)
            else {
                continue;
            };
            let (dx, dy) = ((x - cx).abs() as f32, (y - cy).abs() as f32);
            if dx / hw + dy / hh <= 1.0 {
                return Some(tile);
            }
        }
        None
    }

    pub(super) fn tiles_with(ctx: &Ctx, county: u8, bit: u8) -> Vec<usize> {
        let map = &ctx.game.kingdom.campaign.map;
        (0..map.terrain.len())
            .filter(|&t| map.county[t] == county && map.flags[t] & bit != 0)
            .collect()
    }

    pub(super) fn settlements(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::SETTLEMENT)
    }

    pub fn settlements_for_test(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::settlements(ctx, county)
    }

    pub fn industry_sites_for_test(&self) -> Vec<(usize, u8, usize, u8)> {
        self.industry_sites.iter().map(|s| (s.tile, s.county, s.commodity, s.frame)).collect()
    }

    /// **This is a deliberate departure from the original
    /// this path.** `Map_PickTile` (`0x00429ba4`) is pure geometry — it divides
    /// by the pitch and resolves the diamond with a parity test, and never
    /// looks at a pixel — so in the original the top of the mine belongs to the
    /// tile behind it, where `Map_Click` finds no flags and does nothing. Ours
    /// answers
    /// one: the diamond is tried first and wins.
    pub(super) fn settlement_at(&self, ctx: &Ctx, county: u8, x: i32, y: i32) -> Option<usize> {
        let tiles = Self::settlements(ctx, county);
        if let Some(tile) = self.tile_at(x, y, tiles.iter().copied()) {
            return Some(tile);
        }
        if !self.map_clip().contains(x, y) {
            return None;
        }
        let slot = ctx.assets.slot(ctx.game.map_slot)?;
        for tile in tiles {
            let (tx, ty) = l2_kingdom::map::coords(tile);
            let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
            let (sx, sy) = campaign::cell_to_screen(self.view, &self.zoom, row, col);
            let bank_byte = slot.at(Plane::GfxBank, tx as usize, ty as usize);
            let frame = slot.at(Plane::GfxIndex, tx as usize, ty as usize) as usize;
            let bank = ((bank_byte & campaign::BANK_MASK) >> 2) as usize;
            let season = ctx.game.kingdom.season;
            let Some(sheet) = ctx.assets.map.bank(&self.zoom, season, bank) else { continue };
            let Some(decoded) = sheet.frame(frame) else { continue };
            let overhang = (decoded.height as i32 - self.zoom.tile_h).max(0);
            let (dx, dy) = (x - sx, y - (sy - overhang));
            if dx < 0 || dy < 0 || dx >= decoded.width as i32 || dy >= decoded.height as i32 {
                continue;
            }
            if decoded.opaque[dy as usize * decoded.width as usize + dx as usize] {
                return Some(tile);
            }
        }
        None
    }

    /// The constant is still spelled `CASTLE` in `l2-kingdom` and its own doc
    /// comment explains why (`docs/decisions.md` C25); the bit is the town.
    pub fn town(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::CASTLE)
    }

    /// * the `dx + W * dy` rule, `[V]` 10,971/10,971 in `l2-formats`;
    /// * every town block of all 44 shipped maps, swept: `part` runs 0, 1, 2, 3
    ///   over `(x, y)`, `(x+1, y)`, `(x, y+1)`, `(x+1, y+1)`;
    /// * **`County_FindTownTile` (`0x00467FD1`), which never reads `part` at
    ///   all.** It sweeps the grid in index order — `for y { for x { … } }`, the
    ///   same order [`MapScreen::tiles_with`] produces — counting the county's
    /// `flags & 0x40` tiles.
    ///
    ///   2nd**. Bank `0x80` is the *only* gate on `Sprite_TopIt` being called at
    ///   all (`FUN_00405EB5`: `if (tile.bank & 0x80) Sprite_TopIt(…)`), so the
    ///   original does not even *visit* the tile we were drawing on.
    pub(super) fn town_quadrant(ctx: &Ctx, county: u8, part: usize) -> Option<usize> {
        let town = Self::town(ctx, county);
        if town.len() != 4 || part > 3 {
            return None;
        }
        let origin = *town.first()?;
        let tile = origin + part % 2 + (part / 2) * l2_kingdom::map::MAP_DIM;
        town.contains(&tile).then_some(tile)
    }

    pub fn zoom(&self) -> &Zoom {
        &self.zoom
    }

    pub fn viewport(&self) -> Viewport {
        self.view
    }

    pub fn map_clip(&self) -> Clip {
        self.zoom.clip()
    }

    pub(super) fn ensure(&mut self, ctx: &Ctx) {
        if !self.opened {
            self.open_on_the_player(ctx);
        }
        let key = (
            ctx.game.map_slot,
            self.zoom.id,
            self.view,
            ctx.game.kingdom.turn_count,
            ctx.game.kingdom.season,
            Self::field_digest(ctx),
            Self::castle_key(ctx),
            self.industry_key(),
            Self::fog_key(ctx),
        );
        if self.built == Some(key) {
            return;
        }
        if self.holding_art_for_the_dark() {
            return;
        }
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else {
            return;
        };
        let lattice = Lattice::build(&slot);
        let mut overrides = Self::tile_graphics(ctx);
        self.add_industry_graphics(&mut overrides);
        self.base.clear(ctx.assets.ink.background);
        self.tags.clear();
        let hidden = |x: usize, y: usize| {
            ctx.game.hides_tile(l2_kingdom::map::index(x as u8, y as u8))
        };
        let fog: campaign::Fog =
            if ctx.game.kingdom.options.exploration { Some(&hidden) } else { None };
        campaign::draw(
            &mut self.base,
            &slot,
            &lattice,
            &ctx.assets.map,
            self.view,
            &self.zoom,
            &mut self.tags,
            &overrides,
            ctx.game.kingdom.season,
            fog,
        );
        self.built = Some(key);
    }

    pub(super) fn fog_key(ctx: &Ctx) -> u64 {
        if !ctx.game.kingdom.options.exploration {
            return 0;
        }
        let mut n: u64 = 1;
        for tile in 0..l2_kingdom::MAP_TILES {
            n = n.wrapping_mul(0x100_0000_01B3).wrapping_add(ctx.game.hides_tile(tile) as u64 + 1);
        }
        n
    }

    pub fn tile_graphics(ctx: &Ctx) -> campaign::Overrides {
        let mut out = Self::town_graphics(ctx);
        Self::add_field_graphics(ctx, &mut out);
        out
    }

    /// `Terrain_Set` (`0x0046D7F4`) is the game's single writer of a tile's
    /// `content` byte and it picks the graphic at the same moment; the frame it
    /// writes is a pure function of the new terrain
    /// whatever frame the tile already had, so it can be recomputed from the
/// file. [`l2_view::campaign::field_graphic`] is that
    /// function and carries the derivation.
    pub(super) fn add_field_graphics(ctx: &Ctx, out: &mut campaign::Overrides) {
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else { return };
        let map = &ctx.game.kingdom.campaign.map;
        for tile in 0..map.terrain.len() {
            if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
                continue;
            }
            let (x, y) = l2_kingdom::map::coords(tile);
            let stored = slot.at(Plane::GfxIndex, x as usize, y as usize);
            let (bank, frame) = campaign::field_graphic(map.terrain[tile], stored);
            out.set(x as usize, y as usize, bank, frame);
        }
    }

    pub(super) fn field_digest(ctx: &Ctx) -> u64 {
        let map = &ctx.game.kingdom.campaign.map;
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for tile in 0..map.terrain.len() {
            if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
                continue;
            }
            h ^= u64::from(map.terrain[tile]) ^ (tile as u64);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// **Put the towns back.** `Counties_PlaceSites` (`0x00468D4F`) rewrites
    /// every county's 2 × 2 town block at load.
    ///
    /// `FUN_0046ac22` stamps `frame = base + quadTable[part]`.
    pub fn town_graphics(ctx: &Ctx) -> campaign::Overrides {
        let mut out = campaign::Overrides::new();
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else { return out };
        let k = &ctx.game.kingdom;
        for id in k.county_ids() {
            let pop = k.counties[id].population;
            let base = TOWN_FRAME_BASE
                .iter()
                .find(|(limit, _)| pop < *limit)
                .map_or(55, |(_, base)| *base);
            for tile in Self::town(ctx, id as u8) {
                let (x, y) = l2_kingdom::map::coords(tile);
                let stored = slot.at(Plane::GfxIndex, x as usize, y as usize);
                out.set(x as usize, y as usize, TOWN_BANK, base + stored);
            }
            Self::castle_graphics(ctx, id as u8, &mut out);
        }
        out
    }

    pub(super) fn add_industry_graphics(&self, out: &mut campaign::Overrides) {
        for site in &self.industry_sites {
            let (x, y) = l2_kingdom::map::coords(site.tile);
            out.set(x as usize, y as usize, TOWN_BANK, site.frame);
        }
    }

    /// The original does not look: `County_PlaceResourceSites` stores each
    /// site's tile on the record at load (`Industry.siteTile`, county `+0x298 +
    /// c*0x18`) and every reader indexes it. We derive it from the settlement
    /// bit
    /// [`l2_kingdom::map::industry_site`] carries the argument for deriving
/// and cache the answer here:
    pub(super) fn rebuild_industry_sites(&mut self, ctx: &Ctx) {
        if self.industry_slot == Some(ctx.game.map_slot) {
            return;
        }
        self.industry_slot = Some(ctx.game.map_slot);
        self.industry_sites.clear();
        let k = &ctx.game.kingdom;
        for id in k.county_ids() {
            for c in l2_kingdom::tables::Commodity::ALL {
                let Some(tile) = l2_kingdom::map::industry_site(&k.campaign.map, id as u8, c)
                else {
                    continue;
                };
                self.industry_sites.push(IndustrySite {
                    tile,
                    county: id as u8,
                    commodity: c.index(),
                    frame: Self::industry_rest_frame(ctx, tile, c.index()),
                });
            }
        }
    }

    pub(super) fn industry_rest_frame(ctx: &Ctx, tile: usize, commodity: usize) -> u8 {
        let (idle, _, _, wrecked) = campaign::INDUSTRY_FRAMES[commodity.min(3)];
        let terrain = ctx.game.kingdom.campaign.map.terrain.get(tile).copied().unwrap_or(0);
        match l2_kingdom::map::industry_state(terrain) {
            Some((_, l2_kingdom::map::SiteState::Wrecked)) => wrecked,
            _ => idle,
        }
    }

    pub(super) fn step_industry(&mut self, ctx: &Ctx) -> bool {
        self.rebuild_industry_sites(ctx);
        // `Tick_Pulses` (`0x004BBC80`) gates on 20 ms and then sets
        // `stamp = now`, so the remainder is dropped and a gate is every second
        // 16 ms tick. The rungs count gates, not ticks. C179.
        self.industry_gate_ms += crate::TICK_MS;
        let gate = self.industry_gate_ms >= village::GATE_MS;
        if gate {
            self.industry_gate_ms = 0;
            self.industry_tick = self.industry_tick.wrapping_add(1);
        }
        let k = &ctx.game.kingdom;
        let mut moved = false;
        for site in &mut self.industry_sites {
            let terrain = k.campaign.map.terrain.get(site.tile).copied().unwrap_or(0);
            let working = matches!(
                l2_kingdom::map::industry_state(terrain),
                Some((_, l2_kingdom::map::SiteState::Working))
            );
            if !working {
                let (idle, _, _, wrecked) = campaign::INDUSTRY_FRAMES[site.commodity];
                let rest = match l2_kingdom::map::industry_state(terrain) {
                    Some((_, l2_kingdom::map::SiteState::Wrecked)) => wrecked,
                    _ => idle,
                };
                if site.frame != rest {
                    site.frame = rest;
                    moved = true;
                }
                continue;
            }
            if ctx.game.hides_tile(site.tile) {
                continue;
            }
            let output = k
                .counties
                .get(site.county as usize)
                .map_or(0, |c| c.industry[site.commodity].output);
            let every = village::gates_per_rung(campaign::industry_period_ms(output));
            if !gate || self.industry_tick % every != 0 {
                continue;
            }
            site.frame = campaign::industry_step(site.commodity, site.frame);
            moved = true;
        }
        moved
    }

    pub(super) fn industry_key(&self) -> u64 {
        let mut n: u64 = 0;
        for site in &self.industry_sites {
            n = n.wrapping_mul(0x100_0001).wrapping_add(site.frame as u64);
        }
        n
    }

    pub(super) fn castle_key(ctx: &Ctx) -> u64 {
        let k = &ctx.game.kingdom;
        let mut n: u64 = 0;
        for id in k.county_ids() {
            let c = &k.counties[id];
            n = n
                .wrapping_mul(0x100_0001)
                .wrapping_add(c.castle_type as u64)
                .wrapping_mul(0x101)
                .wrapping_add(c.castle_degraded as u64)
                .wrapping_mul(0x101)
                .wrapping_add(c.castle_percent as u64);
        }
        n
    }

    /// **Put the castle there at all.** `Castle_StampTile` (`0x0046826C`), the
    /// half of it that is artwork.
    ///
    /// Bank `0x10` is `(0x10 & 0x1C) >> 2 == 4`, `Castle1a.pl8` / `Castle2a.pl8`
    /// —
    /// `Gfx_LoadCountyMode` with the frame indices unchanged, so these numbers
    /// are season-independent. The install ships `Castle1a … Castle1d` and
    /// `Castle2a … Castle2d`, the same four-suffix shape as `Base`, `Mtns`,
    /// `Roads` and `Town`. `[V]`
    pub(super) fn castle_graphics(ctx: &Ctx, county: u8, out: &mut campaign::Overrides) {
        let c = &ctx.game.kingdom.counties[county as usize];
        let Some(stamp) =
            l2_kingdom::map::castle_stamp(c.castle_type, c.castle_degraded, c.castle_percent)
        else {
            return;
        };
        for (quadrant, tile) in
            l2_kingdom::map::castle_tiles(&ctx.game.kingdom.campaign.map, county)
                .into_iter()
                .take(4)
                .enumerate()
        {
            let (x, y) = l2_kingdom::map::coords(tile);
            out.set(x as usize, y as usize, stamp.bank, stamp.frames[quadrant]);
        }
    }

    pub(super) fn ensure_minimap(&mut self, ctx: &Ctx) {
        if self.minimap_slot == Some(ctx.game.map_slot) {
            return;
        }
        self.minimap = ctx.assets.minimap(ctx.game.map_slot);
        self.minimap_slot = Some(ctx.game.map_slot);
    }
}
