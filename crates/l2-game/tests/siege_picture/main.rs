//! **A siege is fought on the castle's tiles, not the field's.**
//!
//! ```text
//! cargo test -p l2-game --test siege_picture
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test siege_picture
//! ```
//!
//! `Battle_LoadAssets` (`0x004987B7`) fills the tile renderer's two sheet
//! pointers from slots 0 and 1 of the battle asset table at `0x004DA550`, and
//! the ladder that picks them is two flags:
//!
//! ```c
//! if (g_battleIsSiege == 0)   { if (slot == 1) continue; entry = 0; }      /* t32_bat1     */
//! else if (DAT_0057C910 == 0) { entry = slot == 1 ? 5 : 4; }               /* t32_wod1/2   */
//! else                        { entry = slot == 1 ? 3 : 2; }               /* t32_stn1/2   */
//! ```
//!
//! with `DAT_0057C910 = (uint)(1 < g_castleLevel)` (`Siege_LaunchAssault`, `0x004A8AAB`).
//! `Screen_DrawBattlefield` (`0x004233F7`) then ends the repaint with
//! `Palette_Set(0x568EE0)` or `Palette_Set(0x5675A0)` on the same siege flag.
//! Every battle here was drawn from `T32_bat1.pl8` under `T32_bat1.256`.
//!
//! **Which of the two sheets a cell comes from is cell byte `+2`**, bits
//! `0x1C`: `Battlefield_Draw32` (`0x004BCBDC`) takes slot 0 at 0 and slot 1 at
//! 4, and nothing else draws at all. The castle is slot 0 — masonry, towers,
//! gates, the keep door, the wall walk — and the ground it stands on is slot
//! 1: sixteen grass variants, the moat's 49-variant water set and the rubble a
//! collapsed wall leaves. That split is `Battlefield_BuildCastle`'s three
//! escape codes (`FUN_0047E1DC`, `FUN_0047DCCE`, `FUN_0047DFE0`), each of
//! which sets the selector, against every raster cell, which clears it.

mod tables_and_sheets;
pub use tables_and_sheets::*;
mod rendering_tests;
pub use rendering_tests::*;

use l2_game::battlefield::LiveBattle;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::siege::{code, frames_with_code, STRUCTURE_STONE, STRUCTURE_WOOD};
use l2_sim::terrain::{tileset, DIM};
use l2_sim::Troop;
use l2_view::scene::{self, Ground};
use l2_view::Canvas;

/// `.data` begins at RVA `0x4D2000` / file offset `0xD0200`, so the two
/// structure tables at `0x004D7B80` and `0x004D7D80` are these. Written out
///
const STONE_AT: usize = 0xD5D80;
const WOOD_AT: usize = 0xD5F80;

fn staged(castle_level: Option<u8>) -> (Game, Machine) {
    let field = match castle_level {
        Some(level) => l2_sim::siege::our_castle(level),
        None => l2_sim::runner::blank_field(),
    };
    let human: &[(Troop, u16)] = &[(Troop::Swordsmen, 4)];
    let ai: &[(Troop, u16)] = &[(Troop::Pikemen, 4)];
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let mut live = LiveBattle::new(runner, 0, 0, 0, castle_level, 1, 1);
    live.paused = true;
    // On the castle, looking at the wall the gate is in.
    live.cam = (33, 17);
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.prefs.tool_tips = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

pub(crate) fn paint(m: &mut Machine, g: &mut Game, a: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    {
        let mut ctx = Ctx { game: g, assets: a };
        m.update(&mut ctx);
    }
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut canvas);
    canvas
}

// --------------------------------------------------------------- the tables

