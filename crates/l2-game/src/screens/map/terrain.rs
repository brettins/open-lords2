use super::*;

impl MapScreen {
    /// Which of a set of candidate tiles a pixel is on, if any.
    ///
    /// The original inverts the isometric projection (`Map_PickTile`) and gets
    /// the tile from anywhere on the map. **Ours** hit-tests the diamonds of
    /// the tiles that could mean something — the county's fields and its
    /// settlements, a few dozen — which reaches the same answer on those and no
    /// answer elsewhere. Honest about being less than the original's picker,
    ///
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
            // The diamond, not its bounding box: |dx|/halfW + |dy|/halfH <= 1.
            let (dx, dy) = ((x - cx).abs() as f32, (y - cy).abs() as f32);
            if dx / hw + dy / hh <= 1.0 {
                return Some(tile);
            }
        }
        None
    }

    /// The county's tiles carrying one plane-0 bit.
    pub(super) fn tiles_with(ctx: &Ctx, county: u8, bit: u8) -> Vec<usize> {
        let map = &ctx.game.kingdom.campaign.map;
        (0..map.terrain.len())
            .filter(|&t| map.county[t] == county && map.flags[t] & bit != 0)
            .collect()
    }

    /// The county's settlement tiles — its four industry sites and its castle
    /// block. `Map_Click`'s own test: plane-0 bit `0x80`.
    pub(super) fn settlements(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::SETTLEMENT)
    }

    /// [`MapScreen::settlements`], for the tests that need to find a county's
    /// mine on the map without duplicating the flag test.
    pub fn settlements_for_test(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::settlements(ctx, county)
    }

    /// **Every industry site's tile
    /// test can watch the wheel turn without reaching into private state or
    /// re-deriving the site list beside the code that derives it.
    ///
    /// `(tile, county, commodity, frame)`, in [`MapScreen::rebuild_industry_sites`]'s
    /// own order. The list is empty until the first
    /// [`MapScreen::step_industry`], which is the first `update`.
    pub fn industry_sites_for_test(&self) -> Vec<(usize, u8, usize, u8)> {
        self.industry_sites.iter().map(|s| (s.tile, s.county, s.commodity, s.frame)).collect()
    }

    /// **Which settlement tile a pixel is on — the ground first, then the
    /// building standing on it.**
    ///
    /// The diamond alone is not enough, and this is the second half of a defect
    /// a player reported as *"I can't click the iron mine on the world map"*.
    /// `Town1a.pl8` frame 30, the mine, is **58 × 47** against a 58 × 30 tile,
    /// so seventeen rows of headframe are drawn *above* the tile's diamond and
    /// a further band of it falls inside the diamond's bounding box but outside
    /// the rhombus. Swept pixel by pixel, 1,314 of the mine's pixels are
    /// painted and only 857 of them were on the tile: **the whole upper half of
    /// the building — the part anybody would aim at — was dead.** The forest is
    /// worse, at 22 rows of overhang.
    ///
    /// **This is a deliberate departure from the original
    /// this path.** `Map_PickTile` (`0x00429ba4`) is pure geometry — it divides
    /// by the pitch and resolves the diamond with a parity test, and never
    /// looks at a pixel — so in the original the top of the mine belongs to the
    /// tile behind it, where `Map_Click` finds no flags and does nothing. Ours
    /// answers
    /// one: the diamond is tried first and wins.
    /// frame's own opaque mask, so it fires only on pixels where that building
/// is painted.
    ///
/// It reads the map file, not [`MapScreen::town_graphics`],
    /// the overrides plane holds towns — plane-0 bit `0x40` — and a settlement
    /// is bit `0x80`; the two sets are disjoint.
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

    /// The county's **town**: the 2 × 2 block on plane-0 bit `0x40`.
    ///
    /// The constant is still spelled `CASTLE` in `l2-kingdom` and its own doc
    /// comment explains why (`docs/decisions.md` C25); the bit is the town.
    pub fn town(ctx: &Ctx, county: u8) -> Vec<usize> {
        Self::tiles_with(ctx, county, l2_kingdom::map::flags::CASTLE)
    }

    /// **One quadrant of the town block, named by the number `Sprite_TopIt`
    /// tests** — `tile.part & 0xf`, plane 3, which `l2-formats` calls
    /// [`l2_formats::maps::Plane::ObjectPart`] and documents as `dx + W * dy`
    /// from the block's north-west tile.
    ///
    /// **This exists because `town()[n]` and `part == n` are not the same
    /// thing**, and a `town.get(1)` that meant `part == 2` is what a player saw
    /// as *"I haven't seen any mercenary icons on the town square yet."* For a
    /// 2 × 2 block with `W = 2`:
    ///
    /// | `part` | offset from the origin | index order |
    /// |---:|---|---:|
    /// | 0 | `(x, y)` | 0 |
    /// | 1 | `(x + 1, y)` | 1 |
    /// | 2 | `(x, y + 1)` | **2** |
    /// | 3 | `(x + 1, y + 1)` | 3 |
    ///
    /// so `part == 2` is the **third** tile in index order, one map row south of
    /// the origin — not the second. Three independent sources agree.
    /// third is the one that settles it:
    ///
    /// * the `dx + W * dy` rule, `[V]` 10,971/10,971 in `l2-formats`;
    /// * every town block of all 44 shipped maps, swept: `part` runs 0, 1, 2, 3
    ///   over `(x, y)`, `(x+1, y)`, `(x, y+1)`, `(x+1, y+1)`;
    /// * **`County_FindTownTile` (`0x00467FD1`), which never reads `part` at
    ///   all.** It sweeps the grid in index order — `for y { for x { … } }`, the
    ///   same order [`MapScreen::tiles_with`] produces — counting the county's
    /// `flags & 0x40` tiles.
    ///   2nd**. Bank `0x80` is the *only* gate on `Sprite_TopIt` being called at
    ///   all (`FUN_00405EB5`: `if (tile.bank & 0x80) Sprite_TopIt(…)`), so the
    ///   original does not even *visit* the tile we were drawing on.
    ///
    /// `None` when the county has no town block, or a block that is not four
/// which no shipped map has; a refusal, not a
    /// guess.
    pub(super) fn town_quadrant(ctx: &Ctx, county: u8, part: usize) -> Option<usize> {
        let town = Self::town(ctx, county);
        if town.len() != 4 || part > 3 {
            return None;
        }
        let origin = *town.first()?;
        let tile = origin + part % 2 + (part / 2) * l2_kingdom::map::MAP_DIM;
        // The arithmetic has to land back inside the block it came from; a town
        // that straddles the right edge of the 64-wide grid would wrap.
        town.contains(&tile).then_some(tile)
    }

    pub fn zoom(&self) -> &Zoom {
        &self.zoom
    }

    pub fn viewport(&self) -> Viewport {
        self.view
    }

/// The rectangle the map is drawn in at the current zoom.
    pub fn map_clip(&self) -> Clip {
        self.zoom.clip()
    }

    /// Paint the tiles and stamp the county ids.
    ///
    /// Called from `draw` *and* from `handle`, because a click can arrive
    /// before a frame has been drawn — in a test it always does — and a pick
    /// plane that only exists after the first repaint is a pick plane that
    /// works everywhere except in the tests.
    pub(super) fn ensure(&mut self, ctx: &Ctx) {
        if !self.opened {
            self.open_on_the_player(ctx);
        }
        // The turn count is in the key because [`town_graphics`] depends on
        // every county's population, which the end of a turn moves. The season
        // is in it because it repoints all five tile banks.
        // digest because a brush stroke repaints one tile without ending a
        // turn — see [`MapScreen::field_graphics`].
        //
        // **And the castles are in it too**
        // picture on the map in the middle of a turn and a cache keyed on the
        // turn alone would show the bare plot until the next one. Three of the
        // seven components were added by three different agents inside a day;
        // each is a thing that repaints the map without ending a turn.
        let key = (
            ctx.game.map_slot,
            self.zoom.id,
            self.view,
            ctx.game.kingdom.turn_count,
            ctx.game.kingdom.season,
            Self::field_digest(ctx),
            Self::castle_key(ctx),
// **And the industry wheels** move without a turn ending
            // without anything being clicked — the eighth component.
            // first one a *clock* writes. Without it a mine that stepped its
            // frame would keep the cached picture until something else
// invalidated it. That is the shape of *"industry map
            // things are still not animated when active."*
            self.industry_key(),
// **And the fog** lifts without a turn ending
            // the options page drops or raises in the middle of one. The
            // seventh painter-side input.
//
            Self::fog_key(ctx),
        );
        if self.built == Some(key) {
            return;
        }
        // The seasonal art changes in the dark. See
        // [`MapScreen::holding_art_for_the_dark`].
        if self.holding_art_for_the_dark() {
            return;
        }
        let Some(slot) = ctx.assets.slot(ctx.game.map_slot) else {
            return;
        };
        let lattice = Lattice::build(&slot);
        // The towns, the fields, the castles — and now the four industry
        // buildings per county, whose frame is the animation. `Overrides` was
        // written for the first three and never carried the fourth, which is
        // the whole of *"industry map things are still not animated"*: the
        // feature was enumerated and nothing drove it.
        let mut overrides = Self::tile_graphics(ctx);
        self.add_industry_graphics(&mut overrides);
        self.base.clear(ctx.assets.ink.background);
        self.tags.clear();
        // `Map_DrawTile`'s
        // with the viewer's seen bits behind it. `campaign::draw` has the
        // arithmetic; `Game::hides_tile` has the test.
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

    /// The fog, folded for the cache key: zero with the option off, and
    /// otherwise a fold of the viewer's seen bits over the 4,096 tiles in index
    /// order — `docs/netcode.md` §3's rule even though this never leaves the
    /// screen, because a hash of an unordered walk is how a cache comes to
    /// disagree with itself.
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

    /// Everything the game rewrites over the map file: the towns,
    /// fields.
    ///
/// Two passes over one plane because they are
    /// disjoint by construction — a town tile carries plane-0 bit `0x40` and a
    /// field carries `0x20`, and `maps-layers.md` §2's census has no tile with
    /// both.
    pub fn tile_graphics(ctx: &Ctx) -> campaign::Overrides {
        let mut out = Self::town_graphics(ctx);
        Self::add_field_graphics(ctx, &mut out);
        out
    }

    /// **Give every farm tile the picture its crop state calls for.**
    ///
    /// `Terrain_Set` (`0x0046D7F4`) is the game's single writer of a tile's
    /// `content` byte and it picks the graphic at the same moment; the frame it
    /// writes is a pure function of the new terrain
    /// whatever frame the tile already had, so it can be recomputed from the
/// file. [`l2_view::campaign::field_graphic`] is that
    /// function and carries the derivation.
    ///
    /// Until this existed the field brush painted markers of our own
    /// map showed the same ploughed field in March and in August, whatever the
    /// county's crops were doing. `maps-layers.md` §5.5 read the mapping and
    /// said so: *"Not yet drawn."*
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

    /// A cheap summary of every farm tile's crop state, for the repaint key.
    ///
    /// **Order-dependent and deterministic**, which is all it has to be: it
    /// never leaves this screen, is never saved and is not the lockstep digest.
    /// It exists so that a single brush stroke repaints the map — the turn
    /// counter does not move when a player ploughs one field, and without this
    /// the new picture would not appear until the season turned.
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
    /// it again every season; the bytes `L2_maps.dat` holds for those tiles are
    /// a placeholder the original never draws.
    ///
    /// Drawing the placeholder is what put **four quarries where a player's
/// town should be**.
    /// quarries: `Town1a.pl8` frame 0 *is* the stone quarry, which is how
    /// `County_PlaceResourceSites` identifies one (frame 0 stone, 20 wood, 30
    /// iron). The stored frames 0 … 3 are four of them.
    ///
    /// `FUN_0046ac22` stamps `frame = base + quadTable[part]`.
    /// frame already *is* `quadTable[part]` — 0, 2, 1, 3 for the north-west,
    /// north-east, south-west and south-east tiles — so the rewrite is the base
    /// added to what the file holds, and no quadrant table is needed here.
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

    /// Stamp each site's current frame over the map file's.
    ///
    /// Every site gets an override, not only the working ones: an idle mine's
    /// picture is *also* wrong from the file the moment
    /// `Industry_UpdateSiteTile` has ever run, and a wrecked one is a different
    /// frame entirely. The bank is [`TOWN_BANK`] because the mine, the quarry,
    /// the forest
    pub(super) fn add_industry_graphics(&self, out: &mut campaign::Overrides) {
        for site in &self.industry_sites {
            let (x, y) = l2_kingdom::map::coords(site.tile);
            out.set(x as usize, y as usize, TOWN_BANK, site.frame);
        }
    }

    /// **Find every industry building once**, when the map slot changes.
    ///
    /// The original does not look: `County_PlaceResourceSites` stores each
    /// site's tile on the record at load (`Industry.siteTile`, county `+0x298 +
    /// c*0x18`) and every reader indexes it. We derive it from the settlement
    /// bit
    /// [`l2_kingdom::map::industry_site`] carries the argument for deriving
/// and cache the answer here:
    /// what never changes while the terrain on it does.
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

    /// The frame a site shows when its wheel is **not** turning: the wrecked
    /// picture if an army trampled it, and otherwise the idle one
    /// `Industry_UpdateSiteTile` writes whenever the season's output was not
    /// positive.
    pub(super) fn industry_rest_frame(ctx: &Ctx, tile: usize, commodity: usize) -> u8 {
        let (idle, _, _, wrecked) = campaign::INDUSTRY_FRAMES[commodity.min(3)];
        let terrain = ctx.game.kingdom.campaign.map.terrain.get(tile).copied().unwrap_or(0);
        match l2_kingdom::map::industry_state(terrain) {
            Some((_, l2_kingdom::map::SiteState::Wrecked)) => wrecked,
            _ => idle,
        }
    }

    /// **`Sprite_TopIt` arm 5b, once per fixed tick — the wheel, and its rate.**
    ///
    /// A site whose terrain says *working* steps its own frame; one that is
    /// idle or wrecked is pinned to [`MapScreen::industry_rest_frame`]. There is
    /// **no overlay for either state**: the whole of "this mine is running" is
/// that its picture moves. A feature fully enumerated
    /// still looked like nothing was happening — `Overrides` was written for
    /// fields, towns and castles, so every site drew the frame `L2_maps.dat`
    /// stores, and that is the idle frame in all four cases.
    ///
    /// The rate is [`campaign::industry_period_ms`], banded from the season's
    /// output, and converted to *gates* by [`village::gates_per_rung`]: the
    /// chain counts 20 ms gates, so 640 ms is 32 gates and 80 ms is 4. A gate
    /// is every second 16 ms tick, because `Tick_Pulses` drops its remainder.
    ///
    /// **This is the only thing in the screen that a clock drives into a
    /// picture the base plane holds**, so it returns whether anything moved and
/// the caller repaints on that.
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
            // **Arm 5b is inside `Sprite_TopIt`, and so behind its fog test**: a
            // mine in the dark does not turn. The rest-frame pin above is
            // `Industry_UpdateSiteTile`'s and is not gated.
            if ctx.game.hides_tile(site.tile) {
                continue;
            }
            // `total - totalSnapshot`, which this crate keeps as
            // `Industry::output` and computes at exactly the same moment.
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

    /// The site frames, folded so the base plane's cache notices a wheel that
    /// turned. A fold over the list in build order, not a hash of anything
    /// unordered — `docs/netcode.md` §3, and it never leaves this screen.
    pub(super) fn industry_key(&self) -> u64 {
        let mut n: u64 = 0;
        for site in &self.industry_sites {
            n = n.wrapping_mul(0x100_0001).wrapping_add(site.frame as u64);
        }
        n
    }

    /// Every county's castle state, folded into one number, so that the painted
    /// map is rebuilt the moment a castle is ordered or a season of work moves
    /// its picture on. Not a hash of anything iterated in an unordered way —
    /// see `docs/netcode.md` — it is a fold over `county_ids` in order.
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
    /// A county's castle is **not in `L2_maps.dat`**. Unlike the mine, the
    /// quarry
/// `County_PlaceResourceSites` flags — the castle plot is plain
    /// ground in the base bank, and every castle you have ever seen on the
    /// original's campaign map was stamped in at run time. Ours drew the plain
    /// ground, so **no county's castle was on the map**.
    ///
/// The frame is chosen from the *castle's state*;
/// with the map screen's per-turn cache:
    /// three appearances per level, twenty frames apart, and a castle going up
    /// changes picture twice on its way.
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
        // The block's own order is index order over the 2×2, which is what
        // `Map_StampBlock` walks: north-west, north-east, south-west,
        // south-east. `castle_tiles` returns them in tile-index order, which is
        // the same walk.
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
