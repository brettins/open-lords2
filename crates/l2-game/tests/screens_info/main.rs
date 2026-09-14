//! The field brush and the tile information panel.
//!
//! Split out of `tests/screens.rs`; the shared helpers are in `tests/common/`.


#[macro_use]
#[path = "../common/mod.rs"]
mod common;

mod field_panel;
pub use field_panel::*;
mod tile_panel_part;
pub use tile_panel_part::*;

use common::*;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// One of `county`'s fields whose centre is on the map at the campaign screen's
/// **opening** viewport, that picks back to itself and has no unit standing on
/// it.
///
/// **The ruler is a second `MapScreen` that has been drawn once**, because the
/// map opens on the player's county lazily, on its first `ensure`
/// (`MapScreen::open_on_the_player`): an undrawn probe is looking at the
/// constructor's corner of England. The first version of this helper did not
/// draw it and found no field of the player's in view at all. A [`Machine`]
/// clicked in these tests is drawn first too, which is also what a player does.
fn visible_field(
    game: &mut Game,
    assets: &Assets,
    county: u8,
    want: impl Fn(l2_kingdom::field::FieldType) -> bool,
) -> Option<(usize, (i32, i32))> {
    let mut probe = MapScreen::new();
    draw(&mut probe, game, assets);
    let clip = probe.map_clip();
    game.kingdom.field_tiles(county as usize).into_iter().filter(|&(_, k)| want(k)).find_map(
        |(tile, _)| {
            let (tx, ty) = l2_kingdom::map::coords(tile);
            let (x, y) =
                campaign::tile_centre(probe.viewport(), probe.zoom(), tx as usize, ty as usize)?;
            (clip.contains(x, y)
                && probe.pick_tile(x, y) == Some((tx, ty))
                && game.kingdom.campaign.units.at(tx, ty).is_none())
            .then_some((tile, (x, y)))
        },
    )
}

