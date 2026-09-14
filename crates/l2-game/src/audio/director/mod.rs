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

/// **What the world sounds like, from what is on screen and who is winning.**
///
/// The one place the screen stack is turned into a music decision, so that the
/// event loop can be three lines and this can be tested.
///
/// The original divides the same way and by the same quantity: `g_battlePhase`
/// 0 is *the whole management surface* — map, counties, village, save, the
/// popups over them — and it all runs `FUN_00499ACA`. County track is not separate.
/// county track. `[V]`.
/// are the bg music for the map/county management screen"*.
///
/// # The question is *"has `Game_NewGame` run"*, and it is asked of the whole stack
///
/// **This function returned `FrontEnd` for every state a running game can be
/// in, for the entire life of the audio layer, and no music has ever played.**
/// It asked whether the screen at the *bottom* of the stack is a setup page —
/// and `SetupScreen`'s Start button returns `Transition::Push(ScreenId::Campaign)`,
/// so the title screen stays at the bottom of the stack for the whole session.
/// The predicate meant *"is the front end still up?"*; what it implemented was
/// *"was the front end ever up?"*, and those agree only in a machine built by
/// hand — which is exactly what the test that covered it built. `C116`.
///
/// So the stack is asked as a whole.
/// music phases:
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
/// Nothing before `Game_NewGame` reaches either picker, which is what makes the
/// front end silent — not a rule about the title screen.
///
/// `before_the_campaign` is exhaustive over [`crate::screen::ScreenId`] with no `_` arm, so
/// a new screen does not compile until somebody has said which side of
/// `Game_NewGame` it is on. That is the only defence that would have caught the
/// original defect, because every check that *reads* this function agreed with
/// it.
///
/// # The share is recomputed here
///
/// The original reads `Realm +0x60`, a byte `FUN_0049D1E0` rebuilds once a
/// season. We compute `PctOf(county_count, kingdom.county_count)` — the same
/// arithmetic, from the same two numbers — for two reasons.
/// the one that matters:
///
/// * `Realm::share_of_map_pct` is **0 on a freshly imported save** and stays 0
///   until a turn has been ended, because `l2-scenario` reads what the `.sav`
/// holds and the field is derived. A player who loads a
///   game at forty per cent of the map would get `Scroll1`.
/// * and a music bed that depends on *whether some other subsystem has run
/// is a bug waiting for the order to change. This function should be a
///   function of the world, not of the schedule.
///
/// The divergence is that the original's music can be up to a season stale
/// after a mid-turn conquest and ours cannot. It is a music bed; nothing reads
/// it back.
pub fn scene(machine: &crate::screen::Machine, game: &crate::Game) -> Scene {
    use crate::screen::ScreenId;
    let ids = machine.ids();
    // `g_screenId == 0x22`: whatever else is true, a film is playing and the
    // bed is stopped. See [`Scene::Film`].
    if matches!(machine.top_id(), Some(ScreenId::Movie(_))) {
        let over_battle = ids.iter().any(|id| matches!(id, ScreenId::Battlefield));
        return Scene::Film { over_battle };
    }
    // `g_battlePhase == 2`. The battlefield is three screen ids in the original
    // (`0x29` field, `0x2A` drag, `0x2B` outcome) and one of ours.
    // phase outlives all of them: panels open over the field while the battle
    // music keeps playing, so this asks the stack, not its top.
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

/// **Everything audible, decided from the world after the tick that made it.**
///
/// One direction only: [`Director::listen`] reads the game and the screen stack
/// and tells the audio layer what should be true. It never writes to either,
/// and nothing it does is visible to the next tick — so the recording of a
/// session and a replay of it are the same simulation whether or not the
/// machine had a sound card. That is `docs/netcode.md` D-3, and it is why this
/// takes `&Machine` and `&Game`.
///
/// # Why this is a type in the library and not six lines in `main.rs`
///
/// It *was* six lines in `main.rs`.
/// exists in the shape it does: **a binary's code cannot be called by a test**,
/// so the only test available was one that re-typed the same six lines beside
/// its assertions and then checked them. That test passes with `main.rs`
/// deleted. `docs/agents.md` — *two artefacts that must agree and are
/// maintained by the same person at the same time is the pattern that lies* —
/// and re-typing a function into its own test is the purest form of it.
///
/// The two edge counters are the state that forces it to be a type at all: a
/// fanfare must sound when something *becomes* true, and `listen` runs sixty
/// times a second.
#[derive(Debug, Default)]
pub struct Director {
    /// **The screen stack at the previous tick**, so that a screen *arriving*
    /// is an edge this can see.
    ///
    /// This is the single mechanism behind most of what the original plays
    /// outside a battle.
    /// **the original's sounds are in the function that sets `g_screenId`**.
    /// `Panel_OpenRation` is four statements and two of them are sounds;
    /// `Sidebar_Button`'s supplies arm, `Panel_SplitButton`, `Map_ZoomOut` and
    /// `Panel_JobDetail` are all the same shape. So *"screen `0x19` is up and
    /// was not"* is not an approximation of the trigger — it is the trigger,
    /// with the one difference that ours cannot fire twice if the player is
    /// already there, and neither can theirs.
    ///
    /// Empty until the first tick, which makes the first tick's whole stack
    /// look like an arrival. That is harmless because the first tick is the
    /// title screen, and it is asserted in `tests/audio_wiring.rs`.
    /// left to be noticed.
    stack: Vec<crate::screen::ScreenId>,
    /// `g_mapZoom == 2` at the previous tick — `Map_ZoomOut`'s edge. `None`
    /// until the first tick, so starting zoomed out is not an event.
    zoom_far: Option<bool>,
    /// [`crate::game::Game::nobles_spoken`] at the previous tick —
    /// `FUN_004B3994`'s edge. `None` until the first tick.
    /// does not announce a category nobody asked for.
    nobles_spoken: Option<u32>,
    /// [`crate::game::Game::spoken`]'s count at the previous tick — the edge
    /// for the six sites a screen has to report.
    spoken: Option<u32>,
    /// **Where every unit stood at the last tick**, so that a unit *entering a
    /// tile* can be noticed without the simulation reporting it. See
    /// [`Director::hear_the_march`].
    ///
    /// Empty until the first tick, which is what makes the first tick silent:
    /// there is nothing to have moved from.
    tiles: Vec<Option<(l2_kingdom::UnitKind, (u8, u8))>>,
    /// **The map's content plane at the last tick**, so that a tile *being
    /// wrecked* can be noticed without the simulation reporting it. See
    /// [`Director::hear_the_wreck`]. Empty until the first tick, which is what
    /// makes the first tick silent — the same reason [`Director::tiles`] is.
    terrain: Vec<u8>,
    /// [`crate::screen::Machine::clicks`] at the previous tick — the widget
    /// click's edge. See [`Director::hear_the_click`].
    clicks: u32,
    /// **The message record on screen at the previous tick**, so that a window
    /// *closing* is an edge this can see — which is `Msg_Dismiss`
    /// (`0x00476768`) and its `Sound_StopOneShot`. `None` both before the first
    /// tick and whenever no window is up, and those two mean the same thing
    /// here: nothing to have been dismissed.
    message: Option<crate::message::Record>,
    /// Ticks this director has listened to — the clock `FUN_004B3ACD`'s
    /// `timeGetTime()` becomes. See [`Director::chain_takes`].
    ticks: u64,
    /// **`DAT_0052F004`, the chained takes' cursor**, and
    /// [`crate::tip::Tips::shows`] when it was last zeroed.
    take_cursor: usize,
    tip_shows: u32,
    /// `_DAT_004E59FC` — the tick the one-shot buffer was last seen busy.
    /// `None` until it has been, which is the original's zero against a
    /// `timeGetTime()` that has been running since the machine booted.
    take_busy_at: Option<u64>,
    /// **Every county's `weapon_type` at the previous tick**, so that a smithy
    /// being *reassigned* is an edge this can see. See
    /// [`Director::hear_the_smithy`].
    ///
    /// Empty until the first tick, which is what makes the first tick silent.
    weapons: Vec<usize>,
    /// **The tile the information panel was about at the previous tick and its
    /// terrain byte then**, so that the field brush *painting* it is an edge
    /// this can see. `None` whenever no tile panel is up. See
    /// [`Director::hear_the_brush`].
    brush_tile: Option<(usize, u8)>,
    /// **The live battle's [`l2_sim::Cues`] at the previous tick**, so that an
    /// event inside the tick since is an edge this can see. `None` while no
    /// battle is up. See [`Director::hear_the_battle`].
    cues: Option<l2_sim::Cues>,
    /// How many of the live battle's [`crate::battlefield::Cry`]s have been
    /// played or dropped already.
    cries_heard: usize,
    /// `g_troopCryCounter` — process-lifetime, as the original's is.
    troop_cries: TroopCries,
}

/// **Is this a screen that can be up before `Game_NewGame` has run?**
///
/// Exhaustive on purpose, with no `_` arm: adding a [`ScreenId`] is a compile
/// error here until somebody has said whether the campaign music should be
/// playing behind it. Every line is a real question, which is the test
/// `docs/agents.md` asks of an exhaustive list before it is worth having.
///
/// The `true` side is small and closed. `SetupPage` covers all thirteen front
/// end pages including `SetupPage::Load`, so *"the load screen opened from the
/// title is still the front end"* holds —
/// [`ScreenId::SaveLoad`] is the in-game one (`g_screenId` `0x35`/`0x36`) and
/// is a different screen. [`ScreenId::Menu`] is the placeholder front end the
/// application no longer starts on, and [`ScreenId::Index`] is ours and is
/// pushed from the title by `I`; over a running game both sit *above*
/// [`ScreenId::Campaign`], so the stack still answers `Campaign`.
fn before_the_campaign(id: crate::screen::ScreenId) -> bool {
    use crate::screen::ScreenId as S;
    match id {
        S::Setup(_) | S::Menu | S::Index => true,
        // The intro, the logo, the credits and the trailer play before any game
        // exists; the other four are raised by one. `scene` answers `Film`
        // for all eight before it asks this, so the arm is here to be answered
        // to be reached.
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
        // The message scroll. It is an overlay the game paints over whatever is
        // underneath -- Msg_DrawWindow is called from the screen ladder, not
        // from a screen of its own -- so a message is
        // starts or stops, and it can only be up once a game is running. This
        // arm exists because the exhaustive match made somebody answer the
        // question, which is the whole point of it having no wildcard.
        | S::Message
        // Screen `0x27`, a tip's. `g_appPhase == 3` gates `Tip_Update`, so it
// is raised over a running game.
        | S::Tip
        | S::Options(_)
        | S::Battlefield
        | S::MenuBar(_)
        | S::About
        | S::Court
        // The standings, `0x20`: reachable only through the court's button,
// so over a running game.
        | S::Nobles
        | S::Supplies(_)
        | S::Ratings
        | S::Info(_) => false,
    }
}

/// Which of `battle1..5` a file name is, for the "still in the same battle"
/// case where the counter must not be stepped again.
pub(super) fn current_battle_track(name: String) -> Option<Music> {
    names::MUSIC_BATTLE
        .iter()
        .position(|n| *n == name)
        .map(|i| Music::Battle(i as u8 + 1))
}


