#![allow(unused_imports)]
use super::*;
use super::drawing::*;
use super::*;
use super::ui::*;
use super::*;

pub(crate) fn unit_marker_half(zoom: &Zoom, unit: &l2_kingdom::Unit) -> i32 {
    let base = if zoom.id == FAR.id { 1 } else { 3 };
    base + unit.size_class() as i32
}

/// **What `Map_DrawArmies` (`0x00408438`) draws one unit with**: the sheet; the
/// frame its tick handler wrote on the way into the last sweep
/// ([`crate::game::UnitFrames`]); the per-kind nudge;
/// its facing and `+0x149`, which is how far across its tile it is
/// ([`campaign::walk_offset`]).
pub(crate) fn unit_sprite(zoom: &Zoom, game: &crate::game::Game, id: usize, unit: &l2_kingdom::Unit) -> campaign::UnitSprite {
    campaign::UnitSprite {
        sheet: unit.sprite_sheet(),
        frame: game.unit_frame(id, unit),
        nudge: unit.sprite_nudge(),
        walk: campaign::walk_offset(zoom, unit.facing, unit.sub_tile),
    }
}

/// **`Map_DrawArmies` (`0x00408438`)** — every unit on the map, as the figure
/// the original draws.
///
/// The sheet, the frame
/// `Sprite1a.pl8` for armies, mobs **and merchants**, `Sprite1b.pl8` for
/// transports alone, `frame = bank + 3*((facing+1)&7) + walk` for the first two
/// and `6*((facing+1)&7) + phase` for the other two, anchored on the tile's
/// bottom vertex, then **dragged back toward the tile it is leaving** by the
/// walk tables. See [`unit_sprite`], [`l2_kingdom::Unit::sprite_frame`] and
/// [`campaign::walk_offset`]. This used to leave the walk tables out as needing
/// *"a sub-tile step counter we do not keep"*; `docs/decisions.md` C134 gave
/// the unit that counter and nothing here read it, so every army jumped from
/// tile to tile.
///
/// The square marker is the fallback for an install with no `Sprite?a.pl8`, and
/// for the placeholder assets the tests use. It says *there is something here*
/// without claiming to be the game's art. `docs/decisions.md` C21.
///
/// **The order `Map_DrawArmies` (`0x00408438`) is called in** — which is not
/// the order the unit array is in, and that was the defect.
///
/// The army pass is `FUN_00405487` (`0x00405487`) and it does not walk
/// `g_units` at all. It walks the **lattice**: the top aligned row's `cols`
/// cells, then `FUN_004059AF` / `FUN_00405862` alternating down the viewport,
/// each cell setting `g_tileCursor` and calling `Map_DrawArmies`, whose body is
/// a list walk over that one tile —
///
/// **`[D]` on the within-tile tie-break.** The original's is the tile's linked
/// list, which is insertion order and which this crate does not keep; ascending
/// id is what stands in for it. It decides only which of two units *on the same
/// tile* is on top.
pub fn units_in_paint_order(game: &crate::game::Game) -> Vec<usize> {
    let mut order: Vec<(i32, i32, usize)> = game
        .kingdom
        .campaign
        .units
        .iter()
        .map(|(id, u)| {
            let (row, col) = campaign::tile_to_cell(u.x as usize, u.y as usize);
            (row, col, id)
        })
        .collect();
    order.sort_unstable();
    order.into_iter().map(|(.., id)| id).collect()
}

/// **`Map_DrawPathMarker`'s `local_14` (`0x004081A6`) — is this a tile a click
/// would act on?**
pub(super) fn path_marker_is_action(map: &l2_kingdom::map::CampaignMap, x: u8, y: u8) -> bool {
    use l2_kingdom::map::{flags, terrain};
    let tile = l2_kingdom::map::index(x, y);
    let f = map.flags[tile];
    f & (flags::CASTLE | flags::PLOT) != 0
        || (f & flags::SETTLEMENT != 0 && map.terrain[tile] != terrain::CASTLE_PLOT)
}

