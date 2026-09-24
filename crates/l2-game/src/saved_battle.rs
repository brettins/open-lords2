//! A save taken mid-battle resumes it. `Save_Read`'s last call, `FUN_004976A1`
//! (`0x004976A1`), is `if (g_battlePhase == 2) FUN_00477C89()` -> screen 0x29
//! (`00470000.c:3194`); the battle is the blocks `Save_Write` (`0x004ADE93`)
//! dumped with it. [V]

use l2_formats::save::Save;
use l2_kingdom::field_playlist::{FieldPlaylist, PLAYLIST_LEN};
use l2_sim::runner::{FIELD_LEN, MEN_LEN};

use crate::battlefield::LiveBattle;
use crate::game::Game;

const BATTLE_PHASE: u32 = 0x0057_C8FC;
const ARMY_A: u32 = 0x0056_8210;
const ARMY_B: u32 = 0x0056_8218;
const COUNTY: u32 = 0x0056_822C;
const IS_SIEGE: u32 = 0x0053_E8FC;
const CASTLE_LEVEL: u32 = 0x0055_401C;
const CHOICE_OWNER: u32 = 0x0055_33D0;
const FIELD: u32 = 0x0054_40E0;
const PLAYLIST: u32 = 0x0057_CAE0;
const CURSOR: u32 = 0x0056_53F8;
const BUDGETS: u32 = 0x0055_3080;
const MEN: u32 = 0x0055_4480;
const LANE: u32 = 0x0056_D6B0;
const RALLY: u32 = 0x0053_F654;

/// [V] Save_Read 0x004ADFE6 restores block 56 as saved and builds no field. [D] we re-run
/// Battle_Start, whose open-field Battlefield_BuildRandom 0x0047AAA3 bumps the cursor, so an
/// open-field resume starts one back (wrapping at 0x2e as Sync_Rollback 0x0043F5C5 does); a
/// siege takes Battlefield_BuildCastle 0x0047C4BA, which never bumps, and keeps it.
fn saved_playlist(save: &Save, siege: bool) -> Option<FieldPlaylist> {
    let bytes = |va| -> Option<[u8; PLAYLIST_LEN]> { block(save, va, PLAYLIST_LEN).try_into().ok() };
    let cursor = usize::try_from(save.i32_at(CURSOR).ok()?).ok()?;
    let back = if siege { cursor } else if cursor < 1 { 0x2e } else { cursor - 1 };
    Some(FieldPlaylist::restore(bytes(PLAYLIST)?, back, bytes(BUDGETS)?))
}

fn block(save: &Save, va: u32, len: usize) -> Vec<u8> {
    (0..len as u32).map(|i| save.u8_at(va + i).unwrap_or(0)).collect()
}

/// `Battle_Start` (`0x004778A0`) again over the saved armies A (side 4) and B
/// (side 0), then the saved field and men laid over it. Returns the figures
/// whose troop agreed; `None` when the save holds no running battle.
/// [V] playlist, cursor and budgets are save blocks 178, 56, 191.
pub fn resume(save: &Save, game: &mut Game) -> Option<usize> {
    if save.i32_at(BATTLE_PHASE).ok()? != 2 {
        return None;
    }
    let attacker = usize::try_from(save.i32_at(ARMY_A).ok()?).ok()?;
    let defender = usize::try_from(save.i32_at(ARMY_B).ok()?).ok()?;
    let county = save.i32_at(COUNTY).ok()? as u8;
    let castle_level = (save.i32_at(IS_SIEGE).ok()? != 0)
        .then(|| save.i32_at(CASTLE_LEVEL).unwrap_or(0) as u8);
    let choice_owner = save.u8_at(CHOICE_OWNER).ok()?;
    game.kingdom.campaign.field_playlist = saved_playlist(save, castle_level.is_some())?;
    let mut runner = crate::engagement::begin_fight_in(
        &mut game.kingdom,
        attacker,
        defender,
        castle_level,
        crate::scenario::SEED,
        false,
    )?;
    // Blocks 92/93 hold the pair Battle_Start already stepped.
    game.kingdom.campaign.battle_selectors =
        (save.i32_at(LANE).ok()? as u8, save.i32_at(RALLY).ok()? as u8);
    let agreed = runner.overlay_saved(&block(save, FIELD, FIELD_LEN), &block(save, MEN, MEN_LEN));
    game.battle = Some(Box::new(LiveBattle::new(
        runner,
        attacker,
        defender,
        county,
        castle_level,
        game.player,
        choice_owner,
    )));
    Some(agreed)
}
