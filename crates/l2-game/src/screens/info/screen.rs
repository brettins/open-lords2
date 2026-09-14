#![allow(unused_imports)]
use super::*;
use super::types::*;
use super::constants::*;
use super::painters::*;
use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

impl InfoScreen {
    pub fn new(target: Target) -> InfoScreen {
        InfoScreen { target, status: String::new(), press: Press::new() }
    }

    pub fn target(&self) -> Target {
        self.target
    }

    /// The garrison this panel's tile would show, if its castle holds one.
    ///
    /// `FUN_00438ACC` is `g_pickedTileUnit = g_counties[g_pickedTileCounty]
    /// .garrisonUnit`, and `TileInfo_DrawCastle` is what decides the widget
    /// exists at all: a **castle tile** whose county has a garrison. Any owner.
    pub fn garrison(&self, ctx: &Ctx) -> Option<usize> {
        // **[`InfoScreen::castle_tile`], not `terrain > CASTLE_PLOT`.**
        // `DAT_00568474` is written by `TileInfo_DrawCastle`, which runs for
        // the whole `0x0C < graphic < 0x1A` range — the bare plot included —
        // so the widget the press arm answers and the widget the painter draws
        // have to agree on one predicate. They did not.
        let tile = self.castle_tile(ctx)?;
        let county = ctx.game.kingdom.campaign.map.county[tile] as usize;
        let unit = ctx.game.kingdom.counties.get(county).map_or(0, |c| c.garrison_unit);
        (unit != 0).then_some(unit)
    }

    /// **`Map_ResolvePick` (`0x0046D5FE`)'s `_g_pickedTileFlags`** — the plane-0
    /// byte every predicate below tests, with the two tiles the pick *blanks*:
    ///
    /// ```c
    /// if ((flags & 0x80) != 0 && g_pickedTileGraphic == 0x14) _g_pickedTileFlags = 0;
    /// if ((flags & 0x10) != 0 && g_pickedTileGraphic == 0)    _g_pickedTileFlags = 0;
    /// ```
    ///
    /// A **bare castle plot** and an **empty dwelling plot** therefore reach
    /// `TileInfo_Draw` with no bits at all and take its scrubland
    /// fall-through — not the castle arm and not the village arm. The panel
    /// used to draw the castle arm on a bare plot; every county that starts
    /// castleless showed *"Castle."* over open ground.
    ///
    /// **Not reproduced:** the same function's anchor walk for a 2×2 block
    /// (`local_8`, from `part & 0x0F`), which re-reads the flags from the
    /// block's north-west tile. Every tile of a town or castle block carries
    /// the same flags and graphic, so it changes no word on this panel.
    pub fn picked_flags(&self, ctx: &Ctx) -> u8 {
        use l2_kingdom::map::{flags, terrain};
        let Target::Tile(tile) = self.target else { return 0 };
        let map = &ctx.game.kingdom.campaign.map;
        let (f, g) = (map.flags[tile], map.terrain[tile]);
        if f & flags::SETTLEMENT != 0 && g == terrain::CASTLE_PLOT {
            return 0;
        }
        if f & flags::PLOT != 0 && g == 0 {
            return 0;
        }
        f
    }

    /// **`TileInfo_Draw` (`0x0041C208`)'s flag ladder**, in its order:
    /// `0x01`, `0x04`, `0x10`, `0x08`, `0x20`, `0x40`, `0x80`, then the
    /// scrubland fall-through. The five arms this returns that no other method
    /// covers are [`TILE_LADDER`]'s; the last four defer to the predicates that
    /// already carry the same exclusion sets.
    pub fn tile_kind(&self, ctx: &Ctx) -> Option<TileKind> {
        use l2_kingdom::map::flags;
        let Target::Tile(tile) = self.target else { return None };
        let map = &ctx.game.kingdom.campaign.map;
        let f = self.picked_flags(ctx);
        Some(if f & flags::ROAD != 0 {
            TileKind::Road
        } else if f & flags::NO_COUNTY != 0 {
            TileKind::Sea
        } else if f & flags::PLOT != 0 {
            if map.terrain[tile] == VILLAGE_GRAPHIC {
                TileKind::Village
            } else {
                TileKind::RuinedVillage
            }
        } else if f & flags::ROUGH != 0 {
            // `DAT_005651BC`
            if map.is_mountain(tile) {
                TileKind::Mountain
            } else {
                TileKind::Woodland
            }
        } else if f & flags::FARMLAND != 0 {
            TileKind::Farmland
        } else if f & flags::CASTLE != 0 {
            TileKind::CountyTown
        } else if f & flags::SETTLEMENT != 0 {
            if map.terrain[tile] < 0x0D {
                TileKind::Industry
            } else {
                TileKind::Castle
            }
        } else {
            TileKind::Scrubland
        })
    }

    /// **The county whose town this tile is**, or `None`.
    ///
    /// Plane-0 bit `0x40` is the county town — `docs/decisions.md` C25 is why
    /// `l2-kingdom` still spells the constant `CASTLE` — and it is reached only
    /// after `FUN_0041BEFE` and `TileInfo_Draw` have both failed `0x20`
    /// (farmland), `0x04` (no county) and `0x10` (a dwelling plot), which is the
    /// order kept here. A tile can carry more than one of those bits and the
    /// ladder, not the bit, decides which panel you get.
    pub fn county_town(&self, ctx: &Ctx) -> Option<u8> {
        use l2_kingdom::map::flags;
        let Target::Tile(tile) = self.target else { return None };
        let map = &ctx.game.kingdom.campaign.map;
        let f = self.picked_flags(ctx);
        if f & (flags::FARMLAND | flags::NO_COUNTY | 0x10) != 0 || f & flags::CASTLE == 0 {
            return None;
        }
        Some(map.county[tile])
    }

    /// Whether this town's county has a band standing in it — the condition on
    /// both halves of the mercenary tail, and the same byte
    /// [`crate::screens::map`]'s marker reads.
    pub fn mercenary_offer(&self, ctx: &Ctx) -> bool {
        let Some(county) = self.county_town(ctx) else { return false };
        ctx.game
            .kingdom
            .counties
            .get(county as usize)
            .is_some_and(|c| c.mercenary_offer != 0)
    }

    /// **The castle tile this panel describes**, or `None`.
    ///
    /// `TileInfo_Draw`'s and `FUN_0041BEFE`'s `0x80` arm, split the same way in
    /// both: `g_pickedTileGraphic < 0x0D` is a resource site and
    /// `0x0C < graphic < 0x1A` is the castle plot ([`terrain::CASTLE_PLOT`]) or
    /// a castle standing on it. The bit is reached only after `0x20`, `0x04`,
    /// `0x10` and `0x40` have all failed, which is the order kept here.
    ///
    /// [`terrain::CASTLE_PLOT`]: l2_kingdom::map::terrain::CASTLE_PLOT
    pub fn castle_tile(&self, ctx: &Ctx) -> Option<usize> {
        let tile = self.settlement_tile(ctx)?;
        let g = ctx.game.kingdom.campaign.map.terrain[tile];
        (g > 0x0C && g < 0x1A).then_some(tile)
    }

    /// **The resource site this panel describes**, or `None` — the *other* half
    /// of the `0x80` arm, `g_pickedTileGraphic < 0x0D`, and the commodity its
    /// record belongs to.
    ///
    /// The industry index is `local_18`, read through
    /// [`l2_kingdom::industry::map_toggle_for_graphic`] so the panel and the
    /// left click that toggles the site cannot drift apart; `MapToggle::Castle`
    /// is unreachable below `0x0D` and is the `None` here.
    pub fn resource_site(&self, ctx: &Ctx) -> Option<(usize, l2_kingdom::tables::Commodity)> {
        let tile = self.settlement_tile(ctx)?;
        let g = ctx.game.kingdom.campaign.map.terrain[tile];
        if g >= 0x0D {
            return None;
        }
        match l2_kingdom::industry::map_toggle_for_graphic(g) {
            Some(l2_kingdom::industry::MapToggle::Industry(c)) => Some((tile, c)),
            _ => None,
        }
    }

    /// **`TileInfo_Draw`'s `flags & 0x80` arm, the flag half of it**, shared by
    /// [`InfoScreen::castle_tile`] and [`InfoScreen::resource_site`] because the
    /// painter reaches both through one test and then splits on the terrain
    /// byte alone.
    ///
    /// The bit is reached only after `0x01` (road), `0x04` (sea), `0x10` (a
    /// dwelling plot), `0x08` (mountain or wood), `0x20` (farmland) and `0x40`
    /// (the county town) have all failed, which is `TileInfo_Draw`'s order and
    /// the exclusion set here. `docs/decisions.md` C25 is why
    /// [`flags::SETTLEMENT`] and [`flags::CASTLE`] read backwards from their
    /// names.
    ///
    /// **`FUN_0041BEFE` tests a shorter ladder than its callee** — `0x20`,
    /// `0x04`, `0x10`, `0x40`, `0x80`, with no `0x01` and no `0x08` —
    /// settlement tile that also carried road or rough would take the `0x80`
    /// *row* and the road's *words*. No such tile exists in the England
/// position; `tests/screens_info.rs` asserts that.
    ///
    /// [`flags::SETTLEMENT`]: l2_kingdom::map::flags::SETTLEMENT
    /// [`flags::CASTLE`]: l2_kingdom::map::flags::CASTLE
    fn settlement_tile(&self, ctx: &Ctx) -> Option<usize> {
        use l2_kingdom::map::flags;
        let Target::Tile(tile) = self.target else { return None };
        let f = self.picked_flags(ctx);
        let before = flags::ROAD
            | flags::NO_COUNTY
            | flags::PLOT
            | flags::ROUGH
            | flags::FARMLAND
            | flags::CASTLE;
        (f & before == 0 && f & flags::SETTLEMENT != 0).then_some(tile)
    }

    /// **The farm tile this panel describes**, or `None` — `TileInfo_Draw`'s
    /// ladder, in its order: bits `0x01` (road), `0x04` (sea), `0x10` (a
    /// dwelling), `0x08` (mountain or wood) are all tested before `0x20`.
    pub fn farmland(&self, ctx: &Ctx) -> Option<usize> {
        let Target::Tile(tile) = self.target else { return None };
        let f = self.picked_flags(ctx);
        (f & (0x01 | 0x04 | 0x10 | 0x08) == 0 && f & l2_kingdom::map::flags::FARMLAND != 0)
            .then_some(tile)
    }

    /// The three army buttons, and which of the two tables they come from.
    ///
    /// `FUN_00437002` picks between `g_infoUnitButtons` (`0x004DC560`) and the
    /// garrisoned table (`0x004DC5A8`) on `unit.garrisonCounty`, behind three
    /// guards: a unit is picked, it is **kind 1**, and its owner is the local
    /// player. The two tables differ in one slot.
    fn unit_buttons(&self, ctx: &Ctx) -> Option<(usize, bool)> {
        let Target::Unit(id) = self.target else { return None };
        let u = ctx.game.kingdom.campaign.units.get(id)?;
        if u.kind != l2_kingdom::unit::UnitKind::Army || u.owner != ctx.game.player {
            return None;
        }
        Some((id, u.garrison_county != 0))
    }

    /// Which of the eleven layouts this panel is using.
    pub fn layout(&self, ctx: &Ctx) -> Layout {
        match self.target {
            Target::Unit(id) => {
                let k = &ctx.game.kingdom;
                match k.campaign.units.get(id) {
                    Some(u) if u.kind == l2_kingdom::unit::UnitKind::Army => {
                        if u.owner == ctx.game.player {
                            Layout { row: 2, headroom: 0 }
                        } else {
                            Layout { row: 0x12, headroom: 0 }
                        }
                    }
                    // Peasants, merchant and transport all take `0x0F`.
                    _ => Layout { row: 0x0F, headroom: 0 },
                }
            }
            // The tile half's full ladder needs the plane-0 flags and the
            // county's castle state; what is reproduced here is the two arms a
// right click on the campaign map can reach today —
            // farmland of the player's own county, and everything else.
            Target::Tile(tile) => {
                let map = &ctx.game.kingdom.campaign.map;
                let county = map.county[tile];
                let mine = ctx
                    .game
                    .kingdom
                    .counties
                    .get(county as usize)
                    .is_some_and(|c| c.owner == ctx.game.player);
                // **The county town, which is the one arm of the ladder that
                // moves for a reason other than terrain.** `FUN_0041BEFE`:
                //
                // ```c
                // else {                                     /* flags & 0x40 */
                //   if (county.mercenaryOffer == 0) DAT_00553d2c = 0x11;
                //   else                            DAT_00553d2c = 0xf;
                //   DAT_005651c8 = 2;
                // }
                // ```
                //
                // Two extra rows of panel, granted so that the marker and its
                // one line of text have somewhere to go. **No ownership gate**:
                // the offer is advertised on anybody's town.
                if self.county_town(ctx).is_some() {
                    let offer = ctx
                        .game
                        .kingdom
                        .counties
                        .get(county as usize)
                        .is_some_and(|c| c.mercenary_offer != 0);
                    return Layout { row: if offer { 0x0F } else { 0x11 }, headroom: 2 };
                }
                // **The castle, `FUN_0041BEFE`'s `0x80` arm.** Three rows by
                // the county's castle state, and the resource sites below
                // `0x0D` fall through to the `0x11` fallback that ends this
                // ladder:
                //
                // ```c
                // if (g_pickedTileGraphic < 0xd)                    DAT_00553d2c = 0x11;
                // else if (county.castleDegraded == 0)
                //      if (county.field_0x1c2 == '\0')              DAT_00553d2c = 0xe;
                //      else                                         DAT_00553d2c = 0x11;
                // else if (g_localPlayer == g_pickedCountyOwner)    DAT_00553d2c = 10;
                // else                                             DAT_00553d2c = 0x11;
                // ```
                //
                // `0x0A` is the tallest tile layout in the game, and it is
                // tall because `Castle_DrawStatusBlock` needs five lines.
                if self.castle_tile(ctx).is_some() {
                    let c = ctx.game.kingdom.counties.get(county as usize);
                    let row = match c {
                        Some(c) if c.castle_degraded == 0 => {
                            if c.castle_ruined {
                                0x11
                            } else {
                                0x0E
                            }
                        }
                        Some(_) if mine => 0x0A,
                        _ => 0x11,
                    };
                    return Layout { row, headroom: 2 };
                }
                // `FUN_0041BEFE`'s `0x20` arm tests the blighted pair **before**
                // the owner: a flooded or parched field is row `0x11` on anybody's
                // county, because it offers no brush.
                // **`FUN_0041BEFE`'s `0x04` and `0x10` arms**, the two layouts
                // that grant no head-room: sea is `0x11` with none and a
                // dwelling plot is `0x10` with none. Both are tested after
                // `0x20` there, which is the guard here.
                let picked = self.picked_flags(ctx);
                let f = picked;
                if f & l2_kingdom::map::flags::FARMLAND == 0 {
                    if f & l2_kingdom::map::flags::NO_COUNTY != 0 {
                        return Layout { row: 0x11, headroom: 0 };
                    }
                    if f & l2_kingdom::map::flags::PLOT != 0 {
                        return Layout { row: 0x10, headroom: 0 };
                    }
                }
                let t = map.terrain[tile];
                if picked & l2_kingdom::map::flags::FARMLAND != 0
                    && mine
                    && t != 0x17
                    && t != 0x18
                {
                    if t == 0 || t > 0x18 {
                        Layout { row: 0x0C, headroom: 2 }
                    } else {
                        Layout { row: 5, headroom: 2 }
                    }
                } else {
                    Layout { row: 0x11, headroom: 2 }
                }
            }
        }
    }

    /// Whether the field brush is offered: farmland, the player's own county,
    /// and not a tile the weather ruined this season.
    pub fn brush(&self, ctx: &Ctx) -> Option<&'static [u8]> {
        let Target::Tile(tile) = self.target else { return None };
        let map = &ctx.game.kingdom.campaign.map;
        if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
            return None;
        }
        let t = map.terrain[tile];
        if t == 0x17 || t == 0x18 {
            return None;
        }
        let county = map.county[tile];
        if !ctx.game.kingdom.counties.get(county as usize).is_some_and(|c| c.owner == ctx.game.player)
        {
            return None;
        }
        if t == 0 || t > 0x18 {
            Some(&BRUSH_WASTE_ID)
        } else {
            Some(&BRUSH_FIELD_ID)
        }
    }

    /// **`FUN_00438A91`**: a press or a double click on the garrison widget,
    /// and `FUN_00438ACC` behind it, which turns the tile half into the unit
    /// half in place. True when it did.
    fn garrison_press(&mut self, ctx: &mut Ctx, event: Event) -> bool {
        if self.press.event(&garrison_widgets(), event).is_none() || ctx.game.map_zoom_far {
            return false;
        }
        let Some(unit) = self.garrison(&Ctx { game: ctx.game, assets: ctx.assets }) else {
            return false;
        };
        self.target = Target::Unit(unit);
        true
    }
}

impl Screen for InfoScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Info(self.target)
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// The garrison widget's picture coming back up. See
    /// [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        match self.target {
            Target::Unit(id) => format!("Unit {id} — screen 0x04"),
            Target::Tile(t) => format!("Tile {t} — screen 0x04"),
        }
    }

    fn is_overlay(&self) -> bool {
        true
    }

    /// **`Widget_Test`'s countdown over `DAT_004DD640`**, which runs the garrison
    /// widget's pressed picture back up.
    ///
    /// This screen had no `update`, so the picture went down on the first press
    /// and stayed down. The repeat that a held press produces here is inert —
    /// `FUN_00438ACC` has already turned the panel into the unit half, which
    /// has no garrison widget — so nothing is done with it.
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        let _ = self.press.tick();
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // The release ends the garrison widget's hold, and the pointer leaving
        // it is the hit test ceasing to match. Before the ladder, because the
        // pointer arm below is the edge scroll and it must still run.
        if matches!(event, Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft) {
            let fired = self.press.event(&garrison_widgets(), event);
            debug_assert!(fired.is_none(), "the garrison widget is kind 4, not kind 3");
        }
        let Event::Click { x, y } = event else {
            return match event {
                // The same button opens and closes it.
                // arm: 0x0042FF10/info-right-close right-release
                Event::RightClick { .. } => Transition::Pop,
                // arm: ours/info-keyboard-close key
                Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
                // **`Map_EdgeScroll` is the SECOND guard of the `0x04` arm and a
                // scroll CLOSES the panel**: pushing the pointer into the edge
                // of the screen with the information panel up puts you back on
                // the map.
                //
                // ```c
                // if (Map_EdgeScroll()) { g_screenId = 0; FUN_0043CC56(); }
                // ```
                //
                // `Map_EdgeScroll` (`0x00432221`) returns 0 **at the far zoom**
                // — `if (g_battlePhase == 0 && g_mapZoom == 2) return 0;` — so
// the gesture does nothing there, so this reads
// [`crate::game::Game::map_zoom_far`]
                // any edge. Its other refusal, a message scroll being up, is
                // vacuous here: we have no message scroll.
                //
                // The *edge* is the outermost pixel of a 640 × 480 screen;
                // `main.rs` clamps a pointer in the letterbox border onto it,
                // so the gesture works at the edge of the window. That is
                // [`crate::screens::map::MapScreen::edge_direction`]'s own
                // argument and this is the same predicate.
                // arm: 0x0042FF10/info-edge-scroll-closes pointer
                Event::Pointer { x, y } if !ctx.game.map_zoom_far && at_screen_edge(x, y) => {
                    Transition::Pop
                }
                // **A double click reaches the garrison widget and nothing else
                // on this screen.** `0x04`'s arm runs `Ui_OkButtonClicked` and
                // the brush (`Hotspot_Test` kind 3), both of which read
                // `g_mouseLeftReleased`; then `FUN_00438A91`, whose
                // `Widget_Test` kind 4 reads `g_mouseLeftPressed ||
                // g_mouseLeftDoubleClick`; then the unit buttons, `Hotspot_Test`
                // kind 1, `g_mouseLeftPressed` alone; and the minimap epilogue
                // is guarded by `g_mouseLeftPressed || g_mouseRightPressed`.
                // So of the five, one answers. `[V]` This screen dropped it.
                Event::DoubleClick { .. } => {
                    self.garrison_press(ctx, event);
                    Transition::Stay
                }
                _ => Transition::Stay,
            };
        };
        // **The campaign minimap is live under this panel**, and it was not:
        // `Screen_FrameInput`'s epilogue runs `Minimap_Click` on every press on
        // every screen id but `0x12`, and closes the management surface on a
        // hit. This screen swallowed the press instead, so the one control that
        // works from everywhere did not work from here.
        //
        // **Only the raster, not the column.** `0x04`'s arm does not run the
        // six sidebar guards — the village's and the four county panels' do,
        // and this one does not —
        // split slider are all dead with the information panel up, and only the
        // 128 × 128 minimap is not. See `screens/court.rs` at the same arm and
        // `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`, which is the
        // campaign map's half of it.
        // arm: 0x0042FF10/minimap-under-the-info-panel left-press
        if l2_view::chrome::minimap_hit_area().contains(x, y) {
            return Transition::Pass;
        }
        // arm: 0x0042FF10/info-ok left-release
        if OK.contains(x, y) {
            return Transition::Pop;
        }
        // `FUN_00438990` — the brush, on left **release**, and every button
        // closes the panel afterwards because `Field_SetType` sets
        // `g_screenId = 0`.
        // arm: 0x00438990/field-brush left-release
        //
        // `0x00438990/tile-panel-hotspots` is the same arm, filed a second time
        // while the table stood in a popup of ours on the campaign map; the
        // popup is gone and both records now name this one test.
        // arm: 0x00438990/tile-panel-hotspots left-release
        if let Some(ids) = self.brush(ctx) {
            let xs: &[i32] = if ids.len() == 3 { &BRUSH_FIELD_X } else { &BRUSH_WASTE_X };
            for (i, &bx) in xs.iter().enumerate() {
                let box_ = Rect::new(bx, BRUSH_ROW_Y, BRUSH_DIM, BRUSH_DIM);
                if box_.contains(x, y) {
                    if let Target::Tile(tile) = self.target {
                        let county = ctx.game.kingdom.campaign.map.county[tile] as usize;
                        let kind = match ids[i] {
                            0 => l2_kingdom::field::FieldType::Waste,
                            1 => l2_kingdom::field::FieldType::Fallow,
                            2 => l2_kingdom::field::FieldType::Grain,
                            0x13 => l2_kingdom::field::FieldType::Pasture,
                            _ => l2_kingdom::field::FieldType::Reclaiming,
                        };
                        let _ = ctx.game.kingdom.paint_field(county, tile, kind);
                    }
                    return Transition::Pop;
                }
            }
        }
        // **`FUN_00438A91` — the tile half's one widget.** It is tested between
        // the brush and the unit buttons, and it is the only control on this
        // screen that changes what the panel is *about* without leaving it:
        // `FUN_00438ACC` writes the county's garrison into `g_pickedTileUnit`
        // and calls the painter again, so the tile half becomes the unit half
// in place. `g_screenId` never moves, so this is a mutation
        // of `self.target` and not a transition.
        //
        // `FUN_00438ACC` opens `if (g_mapZoom != 2)` and does nothing at the far
        // zoom, which is [`crate::game::Game::map_zoom_far`] here. The arm is
        // declared on [`garrison_widgets`].
        if self.garrison_press(ctx, event) {
            return Transition::Stay;
        }
        // **`FUN_00437002` — the three army buttons, on left *press*** while the
        // brush above fires on release. Which three depends on
        // `unit.garrisonCounty`: `g_infoUnitButtons` (`0x004DC560`) in the field
        // and `0x004DC5A8` inside a castle, differing in the first slot only.
        if let Some((id, garrisoned)) = self.unit_buttons(&Ctx { game: ctx.game, assets: ctx.assets })
        {
            let l = self.layout(ctx);
            for (i, &bx) in BUTTON_X.iter().enumerate() {
                if !Rect::new(bx, l.y(BUTTON_DY - l.row * 16), BUTTON_DIM, BUTTON_DIM).contains(x, y)
                {
                    continue;
                }
                return match (i, garrisoned) {
                    // **`Panel_MoveButton` (`0x004371CE`) — the door to screen
                    // `0x10`.** Two statements: `g_screenId = 0` and
                    // `Map_BeginMoveSelection()`. Ours has no global to write,
                    // so the request goes on [`crate::game::Game`] and the map
                    // picks it up on its next tick; the pop is the `g_screenId
                    // = 0`.
                    //
                    // **The besieging case is not reproduced**: the original
                    // asks `L2.eng` 10/13 *"Lift the siege?"* through
                    // `Ui_OpenConfirm` first, and we have no confirm box. It is
// named in `docs/arms.json`.
                    // arm: 0x00437002/info-move left-press
                    (0, false) => {
                        ctx.game.begin_move_order = Some(id);
                        Transition::Pop
                    }
                    // `Army_LeaveCastle` (`0x004374C4`) — **leave the castle**,
                    // the garrisoned table's first slot, and `g_screenId = 0`
                    // is the pop. [`crate::game::Game::leave_castle`] is its
                    // body `FUN_00437535`, whose tail starts the sortie battle
                    // when the castle is besieged — `Battle_BeginFromCampaign`
                    // (`0x004A7158`), staged by `Game::leave_castle`; the
                    // prompt comes up on the map behind this pop.
                    (0, true) => match ctx.game.leave_castle(id) {
                        LeftCastle::Marched { tile, .. } => {
                            self.status = format!("MARCHED OUT TO {},{}", tile.0, tile.1);
                            Transition::Pop
                        }
                        LeftCastle::Destroyed => {
                            self.status = "NOWHERE TO STAND - THE GARRISON IS LOST".into();
                            Transition::Pop
                        }
                        LeftCastle::NotAGarrison => Transition::Stay,
                    },
                    // **`Panel_DisbandButton` (`0x0043733A`)**, in both tables.
                    // It picks the county the men would join — the home county,
                    // or the one the army stands in when the home county has
                    // changed hands — and asks *"Disband army?"* only when that
                    // county is the owner's. Otherwise it raises message `0x91`,
                    // `L2.eng` group 145, and closes the panel.
                    //
                    // [`l2_kingdom::divide::disband_county`] is the two clauses
                    // and [`crate::game::Game::disband_army`] the whole of it,
                    // including the refusal. **The confirm box is not
                    // reproduced** — `Ui_OpenConfirm(6, …)` is `L2.eng` 10/6 —
                    // and neither is the message scroll, so the refusal is a
                    // status line of ours.
                    // arm: 0x00437002/info-disband left-press
                    (1, _) => match ctx.game.disband_army(id) {
                        Ok((county, men)) => {
                            let name = super::county::county_name(&*ctx, county);
                            self.status = format!("{men} MEN WENT HOME TO {name}");
                            Transition::Pop
                        }
                        Err(_) => {
                            self.status =
                                "MARCH IT TO A COUNTY YOU RULE BEFORE DISBANDING".into();
                            Transition::Pop
                        }
                    },
                    // **`Panel_SplitButton` (`0x004378B3`)**, and it is a
// `Push` because `0x11` goes
                    // **back to `0x04`**: every one of the division screen's
                    // three ways out — the turn-ended latch, the right release
                    //
                    // That is `docs/arms.json`
                    // `0x0042FF10/back-one-rather-than-to-the-map`, whose note
                    // said ours reached the campaign map "because we have no
                    // unit panel to go back to". There is one now.
                    // arm: 0x004378B3/info-split left-press
                    (2, _) => Transition::Push(ScreenId::Divide(id)),
                    _ => Transition::Stay,
                };
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let l = self.layout(ctx);
        let b = l.box_at();
        pen.window(canvas, b.x, b.y, b.w / 16, b.h / 16, 0);
        pen.ok_button(canvas, OK.x, OK.y, 0);

        let icon = |frame: usize, canvas: &mut Canvas| {
            if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
            }
        };

        match self.target {
            Target::Unit(id) => {
                let k = &ctx.game.kingdom;
                let Some(u) = k.campaign.units.get(id) else { return };
                use l2_kingdom::unit::UnitKind;
                let (heading, body, frame) = match u.kind {
                    UnitKind::Merchant => MERCHANT,
                    UnitKind::PeasantMob => PEASANTS,
                    UnitKind::Transport => TRANSPORT,
                    _ if u.owner == ctx.game.player => OWN_ARMY,
                    _ => ENEMY_ARMY,
                };
                icon(frame, canvas);
                // **The heading line, and every arm of it is `&g_fontHeading`.**
                // We drew the merchant's, the peasants' and the transport's in the
                // body face through `Pen::eng`, put the transport's at the
                // others' place, and drew no army's name at all. `UnitPanel_Draw`:
                //
                // ```c
                // if (local_20 == 2) {                                /* transport */
                //   g_penAdvance = 0;
                //   Eng_DrawString(0x1f, 2, 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
                //   Eng_DrawString(100, unit[+0x167] + g_scenarioIndex * 0x14,
                //                  g_penAdvance + 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
                // } else if (local_20 != 6) {                         /* merchant, peasants */
                //   Eng_DrawString(0x1f, local_20, 0x28, R * 0x10 + 0x40, &g_fontHeading, 0x3f);
                // }
                // ...                                                 /* kind == 1 */
                // g_penAdvance = 0;
                // Eng_DrawString((char)unit.owner + 0x5d, unit.nameIndex, 0x28,
                //                R * 0x10 + 0x30, &g_fontHeading, 0x3f);
                // ```
                //
                // `+0x167` on a transport is [`l2_kingdom::unit::Unit::cargo_county`].
                match u.kind {
                    UnitKind::Transport => {
                        let y = l.y(TRANSPORT_HEADING_AT.1);
                        let w = pen.eng_in(
                            Face::Heading,
                            canvas,
                            UNIT_GROUP,
                            heading,
                            TRANSPORT_HEADING_AT.0,
                            y,
                            font::TEXT,
                        );
                        let name = super::county::county_name(ctx, u.cargo_county);
                        pen.heading(canvas, w, y, &name, font::TEXT);
                    }
                    UnitKind::Army => {
                        pen.eng_in(
                            Face::Heading,
                            canvas,
                            ARMY_NAME_GROUP + u.owner as usize,
                            u.name_index as usize,
                            HEADING_X,
                            l.y(ARMY_NAME_DY),
                            font::TEXT,
                        );
                    }
                    _ => {
                        pen.eng_in(
                            Face::Heading,
                            canvas,
                            UNIT_GROUP,
                            heading,
                            HEADING_X,
                            l.y(HEADING_DY),
                            font::TEXT,
                        );
                    }
                }
                if u.kind == UnitKind::Army {
                    // **Outside the ownership gate**: the name, "An army from"
                    // record.
                    // **`w`, not `HEADING_X + w`.** The original writes
                    // `g_penAdvance = 0; Eng_DrawString(31, 9, 0x28, …);
                    // Eng_DrawString(100, county, g_penAdvance + 0x28, …)` —
                    // `g_penAdvance` is the *width the label advanced*, so
                    // `g_penAdvance + 0x28` is the label's x plus its width.
                    // Our `Pen` returns that sum already, so adding the x again
                    // pushed all four of this panel's chained lines a label's
                    // origin to the right. Four sites here, one on the court,
                    // one inside `Pen::count` itself and one on the ratings
                    // sheet: **seven instances of one confusion**, and it is
// structural — every coordinate in the
                    // decompilation except `g_penAdvance` is absolute, so
                    // transcribing a painter faithfully produces it.
                    // `docs/decisions.md` C110.
                    let w = pen.eng(canvas, UNIT_GROUP, ARMY_FROM, HEADING_X, l.y(0x4A), font::TEXT);
                    let name = super::county::county_name(ctx, u.home_county);
                    pen.body(canvas, w, l.y(0x4A), &name, font::TEXT);
                }
                if u.kind != UnitKind::Army {
                    let s = a.text(UNIT_GROUP, body).to_string();
                    pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), BODY_WRAP, &s, font::TEXT);
                } else {
                    // **The army's body line is the fourth thing `g_optArmiesEat`
                    // gates** — `UnitPanel_Draw` (`0x0041B19D`), the `kind == 1`
                    // arm. With foraging off it is one line at `+0x70`; with it
                    // on the body moves up to `+0x5E` and two more follow:
                    //
                    // ```c
                    // if (g_optArmiesEat == 1) {
                    //   FUN_0040328E(0x1f, local_1c,  0x68, R*0x10+0x5e, 0x120, …, 0x3f);
                    //   FUN_0040328E(0x1f, local_18,  0x68, R*0x10+0x72, 0x120, …, 0x3f);
                    //   local_8 = unit.starvation == 0 ? 0x3f : 0xf9;
                    //   FUN_0040328E(0x1f, unit.starvation + 0x1b, 0x68, R*0x10+0x86, 0x140, …, local_8);
                    // } else FUN_0040328E(0x1f, local_1c, 0x68, R*0x10+0x70, 0x120, …, 0x3f);
                    // ```
                    //
                    // `local_18` is 31/23…26 by the two owners — the supply
                    // state `docs/armies.md` §3.4 tabulates, and 31/26 is
                    // unreachable here because an enemy army in your county
                    // takes 24. The starvation line is 31/27…31 off `+0x155`,
                    // red once the counter leaves zero.
                    let armies_eat = k.options.armies_eat;
                    let s = a.text(UNIT_GROUP, body).to_string();
                    let dy = if armies_eat { 0x5E } else { 0x70 };
                    pen.body_wrapped(canvas, BODY_X, l.y(dy), BODY_WRAP, &s, font::TEXT);
                    if armies_eat {
                        let county_owner =
                            k.counties.get(u.county as usize).map_or(0, |c| c.owner as i32);
                        let supply = if u.owner == ctx.game.player {
                            if county_owner == ctx.game.player as i32 { SUPPLY0 } else { SUPPLY0 + 1 }
                        } else if county_owner == u.owner as i32 {
                            SUPPLY0 + 2
                        } else {
                            SUPPLY0 + 1
                        };
                        let s = a.text(UNIT_GROUP, supply).to_string();
                        pen.body_wrapped(canvas, BODY_X, l.y(0x72), BODY_WRAP, &s, font::TEXT);
                        let band = (u.starvation.clamp(0, 4)) as usize;
                        let colour = if band == 0 { font::TEXT } else { STARVING };
                        let s = a.text(UNIT_GROUP, HEALTH0 + band).to_string();
                        pen.body_wrapped(canvas, BODY_X, l.y(0x86), 0x140, &s, colour);
                    }
                }
                if u.kind == UnitKind::Transport {
                    // `troops[0]` is grain and `troops[2]` cattle — group 8
                    // nouns `0x44` and `0x46`, and the two `Misc_cty` icons.
                    pen.misc_frame(canvas, 0x21, 0x38, l.y(0xA0));
                    pen.count(canvas, 0x60, l.y(0xA4), u.troops[0], 0x44, font::TEXT);
                    pen.misc_frame(canvas, 0x26, 0x104, l.y(0xA0));
                    pen.count(canvas, 0x12E, l.y(0xA4), u.troops[2], 0x46, font::TEXT);
                }
                if u.kind == UnitKind::Army && u.owner == ctx.game.player {
                    let w = pen.eng(canvas, UNIT_GROUP, FORMED, HEADING_X, l.y(0xA0), font::TEXT);
                    // `Ui_DrawYear(yearFormed, g_penAdvance + 0x28, …, 0)` —
                    // **style 0, which appends `L2.eng` 26/1 "AD"**. We drew the
                    // bare number
                    // holds for this line was missing. `CLAUDE.md` rule 6.
                    pen.year(canvas, w, l.y(0xA0), u.year_formed, 0, font::TEXT);
                    let w = pen.eng(canvas, UNIT_GROUP, WAGES, 0xF8, l.y(0xA0), font::TEXT);
                    pen.count(canvas, w, l.y(0xA0), u.wages, 0, font::TEXT);
                    if u.garrison_county == 0 {
                        let left = (MOVE_ALLOWANCE - u.moves_used as i32).max(0);
                        // `Ui_DrawNumber(left, '@', &DAT_004D4228, 0xF8, …, body)`, and
                        // `DAT_004D4228` is a NUL: the lead holds a column and there is
                        // no suffix. The old `number(…, true)` dropped the one and
                        // invented the other, so the digit sat four pixels left and
                        // *"moves left"* landed where it should by coincidence. **[V]**
                        let face = crate::shell::Face::Body;
                        let w = pen.number_in(face, canvas, 0xF8, l.y(0x170), left, '@', "", font::TEXT);
                        pen.eng(canvas, UNIT_GROUP, MOVES_LEFT, w, l.y(0x170), font::TEXT);
                    }
                    // The three buttons, and the sortie frame when garrisoned.
                    let first = if u.garrison_county == 0 { icon::MOVE } else { icon::SORTIE };
                    for (i, &frame) in [first, icon::DISBAND, icon::SPLIT].iter().enumerate() {
                        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                            canvas.blit(&f, BUTTON_X[i], l.y(BUTTON_DY - l.row * 16));
                        }
                    }
                    // The seven troop rows.
                    for t in 0..7usize {
                        let row = (t / 2) as i32 * 13;
                        let (cx, nx) = if t % 2 == 0 { (0x38, 0x58) } else { (0xF8, 0x118) };
                        pen.misc_frame(canvas, 0x2F + t, cx, l.y(0xBE + row));
                        // `FUN_004224E7(unit, 0, 0x38, …, &g_fontBody, 2)` →
                        // `Ui_DrawCount(troops[t], t * 2 + 0x34, …, body)`.
                        pen.count(
                            canvas,
                            nx,
                            l.y(0xC2 + row),
                            u.troops[t],
                            0x34 + t * 2,
                            font::TEXT,
                        );
                    }
                    // **The mercenary line — `UnitPanel_Draw`'s last block, all
                    // `&g_fontHeading`, and it was not drawn at all.**
                    //
                    // ```c
                    // if (unit.mercMen == 0) {
                    //   Eng_DrawString(0x10, 0, 0x38, R * 0x10 + 0x130, &g_fontHeading, 0x3f);
                    // } else {
                    //   g_penAdvance = 0;
                    //   Ui_DrawNumber(mercMen, '@', &DAT_004d422c, 0x38, R * 0x10 + 0x130, &g_fontHeading, 0x3f);
                    //   Eng_DrawString(0x10, mercBand, g_penAdvance + 0x38, …, &g_fontHeading, 0x3f);
                    //   Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, g_penAdvance + 0x38, …);
                    // }
                    // ```
                    //
                    // `Ui_DrawUnitNoun` (`0x0041AC3E`) is `value == 1 ? index : index + 1`
                    // — no `-1` arm, unlike `Ui_DrawCount`, and none is needed for a byte.
                    let y = l.y(MERC_LINE_AT.1);
                    match u.mercenaries {
                        Some(m) if m.men != 0 => {
                            let w = pen.number_in(
                                Face::Heading,
                                canvas,
                                MERC_LINE_AT.0,
                                y,
                                m.men(),
                                '@',
                                "",
                                font::TEXT,
                            );
                            let w = pen.eng_in(
                                Face::Heading,
                                canvas,
                                MERC_GROUP,
                                m.band as usize,
                                w,
                                y,
                                font::TEXT,
                            );
                            let noun = 0x34 + m.troop as usize * 2 + usize::from(m.men != 1);
                            pen.eng_in(
                                Face::Heading,
                                canvas,
                                crate::shell::COUNT_NOUN_GROUP,
                                noun,
                                w,
                                y,
                                font::TEXT,
                            );
                        }
                        _ => {
                            pen.eng_in(
                                Face::Heading,
                                canvas,
                                MERC_GROUP,
                                0,
                                MERC_LINE_AT.0,
                                y,
                                font::TEXT,
                            );
                        }
                    }
                }
            }
            Target::Tile(tile) => {
                let map = &ctx.game.kingdom.campaign.map;
                // `FUN_0041BEFE`, after the box and the OK button and before
                // `TileInfo_Draw`: the recessed well the tile panel's words sit in.
                pen.inset(canvas, Rect::new(0x20, l.y(0x38), 400, (0x18 - l.row) * 16));
                // **`TileInfo_Draw`'s farmland arm** — the heading, the body or
                // the report, and the icon. See [`draw_farmland`].
                if let Some(field) = self.farmland(ctx) {
                    draw_farmland(ctx, &pen, canvas, l, field);
                }
                // The county's name, centred over the box in the head-room the
                // layout granted.
                if l.headroom != 0 {
                    let name = super::county::county_name(ctx, map.county[tile]);
                    pen.heading_centred(canvas, 8, l.y(0x18), 0x1C0, &name, font::TEXT);
                }
                // The brush, if the tile is one of the player's fields.
                if let Some(ids) = self.brush(ctx) {
                    // `Ui_DrawBevelRect` - the reverse lighting of an inset.
                    crate::shell::button_recess(
                        canvas,
                        BRUSH_BEVEL.x,
                        BRUSH_BEVEL.y,
                        BRUSH_BEVEL.w,
                        BRUSH_BEVEL.h,
                    );
                    let xs: &[i32] = if ids.len() == 3 { &BRUSH_FIELD_X } else { &BRUSH_WASTE_X };
                    for (i, &bx) in xs.iter().enumerate() {
                        let frame = match ids[i] {
                            0 => icon::BRUSH_ABANDON,
                            1 => icon::BRUSH_FALLOW,
                            2 => icon::BRUSH_GRAIN,
                            0x13 => icon::BRUSH_PASTURE,
                            _ => icon::BRUSH_RECLAIM,
                        };
                        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                            canvas.blit(&f, bx, BRUSH_ROW_Y);
                        }
                    }
                    let s = a.text(TILE_GROUP, BRUSH_CAPTION).to_string();
                    pen.body_wrapped(
                        canvas,
                        BRUSH_CAPTION_AT.0,
                        BRUSH_CAPTION_AT.1,
                        BRUSH_CAPTION_AT.2,
                        &s,
                        font::TEXT,
                    );
                }
                // **`TileInfo_Draw`'s castle arm and `TileInfo_DrawCastle`
                // under it.** See [`draw_castle`], which also draws 71/14
                // *"View these troops?"* and its widget — the one place the
                // original puts either.
                if let Some(castle) = self.castle_tile(ctx) {
                    draw_castle(ctx, &pen, canvas, l, castle, self.press.is_pressed(0), ink);
                }
                // **The other half of the `0x80` arm.** A player clicked a mine
                //
                // [`draw_resource_site`].
                if let Some((site, c)) = self.resource_site(ctx) {
                    draw_resource_site(ctx, &pen, canvas, l, site, c);
                }
                // **The county-town arm, and the one line of English the
                // mercenary has anywhere in the game.** The map's marker
                // (`screens/map/paint.rs`'s `draw_flags`) is a picture with no words
                // on it; this is where the original says what it means, and a
                // player who had looked straight at the marker still reported
                // never having seen a mercenary. Rule 6: the strings are the
                // specification, and they come out of the player's `L2.eng`.
                if self.county_town(ctx).is_some() {
                    icon(COUNTY_TOWN_ICON, canvas);
                    // **`&g_fontHeading`, not the body face.** `TileInfo_Draw`
                    // passes `&g_fontHeading` to both of its `Eng_DrawString`
                    // headings and to the county name's `Ui_DrawCentred` — three
                    // calls, and its three delegates (`TileInfo_DrawGrain`,
                    // `…Herd`, `…Castle`) pass it to none. This line used to say
                    // the unit half drew *its* headings in body through
                    // `Pen::eng`; it did, and now draws all eight of
                    // `UnitPanel_Draw`'s heading-face calls in the heading face.
                    let s = a.text(TILE_GROUP, COUNTY_TOWN_HEADING).to_string();
                    pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &s, font::TEXT);
                    let s = a.text(TILE_GROUP, COUNTY_TOWN_BODY).to_string();
                    pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), TILE_BODY_WRAP, &s, font::TEXT);
                    if self.mercenary_offer(ctx) {
                        // `g_flagsSheet` frame `0x81` — the *map's* sheet, at
                        // whichever zoom is loaded, because that is the global
                        // the original blits from. `Pl8_DrawFrameClipped` takes
                        // an absolute position and does no centring.
                        let zoom = &l2_view::campaign::ZOOMS[usize::from(ctx.game.map_zoom_far)];
                        let marker = ctx
                            .assets
                            .map
                            .flag_sheet(zoom)
                            .and_then(|s| s.frame(l2_view::campaign::MERCENARY_MARKER_FRAME));
                        if let Some(f) = marker {
                            canvas.blit(&f, MERC_MARKER_AT.0, l.y(MERC_MARKER_AT.1));
                        }
                        let s = a.text(TILE_GROUP, MERCENARIES_AVAILABLE).to_string();
                        pen.body_wrapped(
                            canvas,
                            MERC_TEXT_AT.0,
                            l.y(MERC_TEXT_AT.1),
                            MERC_TEXT_AT.2,
                            &s,
                            font::TEXT,
                        );
                    }
                }
                // **The rest of `TileInfo_Draw`'s ladder** — road, sea, the two
                // villages, mountain, wood
                // heading, one wrapped body, one icon, all three off
                // [`TILE_LADDER`]. See [`draw_plain_tile`].
                if let Some(kind) = self.tile_kind(ctx) {
                    draw_plain_tile(&pen, canvas, l, kind);
                }
            }
        }
        // **Ours**, debug overlay only. The original answers a refused disband
        // with message `0x91` on a scroll we have not built; this is the same
        // sentence with nowhere else to go.
        if ctx.game.prefs.debug_overlay && !self.status.is_empty() {
            l2_view::text::draw(canvas, 12, 452, &self.status, ink.highlight);
        }
    }
}

