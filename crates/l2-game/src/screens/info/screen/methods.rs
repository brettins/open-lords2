#![allow(unused_imports)]
use super::*;
use super::screen_impl::*;
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

    /// `FUN_00438ACC` is `g_pickedTileUnit = g_counties[g_pickedTileCounty]
    /// .garrisonUnit`, and `TileInfo_DrawCastle` is what decides the widget
    /// exists at all: a **castle tile** whose county has a garrison. Any owner.
    pub fn garrison(&self, ctx: &Ctx) -> Option<usize> {
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

    pub fn mercenary_offer(&self, ctx: &Ctx) -> bool {
        let Some(county) = self.county_town(ctx) else { return false };
        ctx.game
            .kingdom
            .counties
            .get(county as usize)
            .is_some_and(|c| c.mercenary_offer != 0)
    }

    /// `TileInfo_Draw`'s and `FUN_0041BEFE`'s `0x80` arm, split the same way in
    /// both: `g_pickedTileGraphic < 0x0D` is a resource site and
    /// `0x0C < graphic < 0x1A` is the castle plot ([`terrain::CASTLE_PLOT`]) or
    /// a castle standing on it. The bit is reached only after `0x20`, `0x04`,
    /// `0x10` and `0x40` have all failed, which is the order kept here.
    pub fn castle_tile(&self, ctx: &Ctx) -> Option<usize> {
        let tile = self.settlement_tile(ctx)?;
        let g = ctx.game.kingdom.campaign.map.terrain[tile];
        (g > 0x0C && g < 0x1A).then_some(tile)
    }

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

    pub fn farmland(&self, ctx: &Ctx) -> Option<usize> {
        let Target::Tile(tile) = self.target else { return None };
        let f = self.picked_flags(ctx);
        (f & (0x01 | 0x04 | 0x10 | 0x08) == 0 && f & l2_kingdom::map::flags::FARMLAND != 0)
            .then_some(tile)
    }

    /// `FUN_00437002` picks between `g_infoUnitButtons` (`0x004DC560`) and the
    /// garrisoned table (`0x004DC5A8`) on `unit.garrisonCounty`, behind three
    /// guards: a unit is picked, it is **kind 1**, and its owner is the local
    /// player. The two tables differ in one slot.
    pub(super) fn unit_buttons(&self, ctx: &Ctx) -> Option<(usize, bool)> {
        let Target::Unit(id) = self.target else { return None };
        let u = ctx.game.kingdom.campaign.units.get(id)?;
        if u.kind != l2_kingdom::unit::UnitKind::Army || u.owner != ctx.game.player {
            return None;
        }
        Some((id, u.garrison_county != 0))
    }

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
                    _ => Layout { row: 0x0F, headroom: 0 },
                }
            }
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
                //
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
    pub(crate) fn garrison_press(&mut self, ctx: &mut Ctx, event: Event) -> bool {
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

