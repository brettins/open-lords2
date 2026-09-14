#![allow(unused_imports)]
use super::*;
use super::drawing::*;
use super::*;
use super::ui::*;
use super::*;

/// Half the side of a unit's marker, in pixels.
///
/// **The three size classes are the original's** — `Army_Tick` picks sprite
/// bank `0x48`, `0x60` or `0x78` at **301 and 601 men**, which the player
/// described as one, two or three figures (`docs/armies.md` §2.4) —
/// marker grows with them so that the same thing is legible. **The square is
/// ours**: `Sprite1a.pl8` holds the actual figures and we do not place them.
pub(super) fn unit_marker_half(zoom: &Zoom, unit: &l2_kingdom::Unit) -> i32 {
    let base = if zoom.id == FAR.id { 1 } else { 3 };
    base + unit.size_class() as i32
}

/// **What `Map_DrawArmies` (`0x00408438`) draws one unit with**: the sheet; the
/// frame its tick handler wrote on the way into the last sweep
/// ([`crate::game::UnitFrames`]); the per-kind nudge;
/// its facing and `+0x149`, which is how far across its tile it is
/// ([`campaign::walk_offset`]).
///
/// One function for the painter
/// across a tile is clicked where it is seen.
pub(super) fn unit_sprite(zoom: &Zoom, game: &crate::game::Game, id: usize, unit: &l2_kingdom::Unit) -> campaign::UnitSprite {
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
/// **What is still ours** is the *selection*: the original shows a selected
/// army by flood-filling its reachable tiles. A garrisoned unit is drawn hollow
/// because it is inside the castle, not on the tile — the
/// original draws it not at all and flies a flag over the castle instead (see
/// [`draw_flags`], which also carries the besieger's mark).
///
/// The square marker is the fallback for an install with no `Sprite?a.pl8`, and
/// for the placeholder assets the tests use. It says *there is something here*
/// without claiming to be the game's art. `docs/decisions.md` C21.
/// **The order `Map_DrawArmies` (`0x00408438`) is called in** — which is not
/// the order the unit array is in, and that was the defect.
///
/// The army pass is `FUN_00405487` (`0x00405487`) and it does not walk
/// `g_units` at all. It walks the **lattice**: the top aligned row's `cols`
/// cells, then `FUN_004059AF` / `FUN_00405862` alternating down the viewport,
/// each cell setting `g_tileCursor` and calling `Map_DrawArmies`, whose body is
/// a list walk over that one tile —
///
/// ```c
/// local_28 = g_tiles[g_tileCursor].unit;
/// for (; local_28 != 0; local_28 = g_units[local_28].field_0x4) { …draw… }
/// ```
///
/// So the painter's order is **lattice row, then column, then the tile's own
/// list**, and a figure standing on a lower row is painted over one standing
/// behind it whatever their array slots are. Ours drew in array order, so which
/// of two overlapping armies was on top was decided by which had the lower id —
/// a unit that marched *behind* another could be drawn in front of it.
/// answer flipped when a slot was reused.
///
/// `l2_view::campaign::tile_to_cell` is the lattice address, so sorting by it
/// **is** the walk: the walk visits every cell of a row left to right and every
/// row top to bottom, and skips nothing a unit could be standing on.
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
///
/// ```c
/// local_14 = flags & 0x50;                                   /* the town, or a dwelling plot */
/// if ((flags & 0x80) != 0 && content != 0x14) local_14 = 1;  /* a site or a castle, not a bare plot */
/// ```
///
/// The plane-0 bits
/// the reach, not a unit standing there. So your own town is gold, a town past
/// the budget is gold, and an enemy army on open ground is coloured by its cost
/// like any other tile — which is narrower than *"an attack"*, and is the
/// original's. `Map_HoverUnitTarget` asks the same bits separately, with an
/// owner test, to decide what a click would *do*; the ball does not.
pub(super) fn path_marker_is_action(map: &l2_kingdom::map::CampaignMap, x: u8, y: u8) -> bool {
    use l2_kingdom::map::{flags, terrain};
    let tile = l2_kingdom::map::index(x, y);
    let f = map.flags[tile];
    f & (flags::CASTLE | flags::PLOT) != 0
        || (f & flags::SETTLEMENT != 0 && map.terrain[tile] != terrain::CASTLE_PLOT)
}

