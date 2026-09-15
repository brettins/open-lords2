#![allow(unused_imports)]

mod engine;
pub use engine::*;

use super::*;
use super::engine::*;
use super::events::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mixer::Mixer;
use track::{BattleCycle, BattleKind, Music};
use wav::Sound;

/// The original divides the same way and by the same quantity: `g_battlePhase`
/// 0 is *the whole management surface* — map, counties, village, save, the
/// popups over them — and it all runs `FUN_00499ACA`. County track is not separate.
///
/// county track. `[V]`.
///
/// The predicate meant *"is the front end still up?"*; what it implemented was
/// *"was the front end ever up?"*, and those agree only in a machine built by
/// hand — which is exactly what the test that covered it built. `C116`.
///
/// | our stack | `g_battlePhase` | what the original calls |
/// |---|---|---|
/// | anything is `ScreenId::Battlefield` | 2 | `Music_StartBattle` (`0x00477B2F`), from `Battle_Start` (`0x004778A0`) |
/// | everything is a front-end screen | — | nothing; neither picker has been reached yet |
/// | anything else | 0 | `Music_StartCampaign` (`0x00499ACA`), from `Game_NewGame` (`0x004992F1`) |
///
/// `[V]` from the decompilation: `Battle_Start` opens with
/// `Sound_LoadBattleBank(); Music_StartBattle();` and `Game_NewGame` ends with
/// `Sound_LoadKingdomBank(); Gfx_LoadCountyMode(); …; Music_StartCampaign();`.
///
/// The original reads `Realm +0x60`, a byte `FUN_0049D1E0` rebuilds once a
/// season. We compute `PctOf(county_count, kingdom.county_count)` — the same
/// arithmetic, from the same two numbers — for two reasons.
pub fn scene(machine: &crate::screen::Machine, game: &crate::Game) -> Scene {
    use crate::screen::ScreenId;
    let ids = machine.ids();
    if matches!(machine.top_id(), Some(ScreenId::Movie(_))) {
        let over_battle = ids.iter().any(|id| matches!(id, ScreenId::Battlefield));
        return Scene::Film { over_battle };
    }
    if ids.iter().any(|id| matches!(id, ScreenId::Battlefield)) {
        let kind = match game.battle.as_ref().is_some_and(|b| b.is_siege()) {
            true => BattleKind::Siege,
            false => BattleKind::Field,
        };
        return Scene::Battle(kind);
    }
    if ids.iter().copied().all(before_the_campaign) {
        return Scene::FrontEnd;
    }
    // `g_screenId == 0x1C`. The interstitial is the top of the stack while it
    // is up — it is entered by `Transition::Replace` — and it plays its own
    // bed. `Screen_DrawConquest` (`0x0041E1DD`).
    if matches!(machine.top_id(), Some(ScreenId::Conquest)) {
        return Scene::Conquest { ended: game.campaign.is_complete() };
    }
    let county_count = game
        .kingdom
        .realms
        .get(game.player as usize)
        .map(|r| r.county_count)
        .unwrap_or(0);
    Scene::Campaign {
        county_count,
        share_of_map_pct: l2_kingdom::industry::pct_of(
            county_count as i32,
            game.kingdom.county_count as i32,
        ),
    }
}

#[derive(Debug, Default)]
pub struct Director {
    stack: Vec<crate::screen::ScreenId>,
    zoom_far: Option<bool>,
    /// [`crate::game::Game::nobles_spoken`] at the previous tick —
    /// `FUN_004B3994`'s edge. `None` until the first tick.
    nobles_spoken: Option<u32>,
    spoken: Option<u32>,
    tiles: Vec<Option<(l2_kingdom::UnitKind, (u8, u8))>>,
    terrain: Vec<u8>,
    clicks: u32,
    /// **The message record on screen at the previous tick**, so that a window
    /// *closing* is an edge this can see — which is `Msg_Dismiss`
    /// (`0x00476768`) and its `Sound_StopOneShot`. `None` both before the first
    /// tick and whenever no window is up, and those two mean the same thing
    /// here: nothing to have been dismissed.
    message: Option<crate::message::Record>,
    /// Ticks this director has listened to — the clock `FUN_004B3ACD`'s
    /// `timeGetTime()` becomes. See [`Director::chain_takes`].
    pub(super) ticks: u64,
    /// **`DAT_0052F004`, the chained takes' cursor**, and
    /// [`crate::tip::Tips::shows`] when it was last zeroed.
    take_cursor: usize,
    tip_shows: u32,
    take_busy_at: Option<u64>,
    weapons: Vec<usize>,
    brush_tile: Option<(usize, u8)>,
    cues: Option<l2_sim::Cues>,
    cries_heard: usize,
    troop_cries: TroopCries,
}

fn before_the_campaign(id: crate::screen::ScreenId) -> bool {
    use crate::screen::ScreenId as S;
    match id {
        S::Setup(_) | S::Menu | S::Index => true,
        S::Confirm(_) => false,
        S::Movie(film) => film.is_front_end(),
        S::Campaign
        | S::County(..)
        | S::Village(..)
        | S::Job(..)
        | S::Conquest
        | S::Diplomacy
        | S::DiploCompose(..)
        | S::SaveLoad(_)
        | S::Castle(_)
        | S::Siege(_)
        | S::RaiseArmy(_)
        | S::Armoury(_)
        | S::Rack(..)
        | S::Divide(_)
        | S::Merchant(_)
        | S::Trade(..)
        | S::BattlePrompt
        | S::BattleResult
        | S::Message
        | S::Tip
        | S::Options(_)
        | S::Battlefield
        | S::MenuBar(_)
        | S::About
        | S::Court
        | S::Nobles
        | S::Supplies(_)
        | S::Ratings
        | S::Info(_) => false,
    }
}

pub(super) fn current_battle_track(name: String) -> Option<Music> {
    names::MUSIC_BATTLE
        .iter()
        .position(|n| *n == name)
        .map(|i| Music::Battle(i as u8 + 1))
}


