#![allow(unused_imports)]
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

impl Director {
    pub fn new() -> Director {
        Director::default()
    }

    /// One tick's worth of sound. Call after the simulation tick that produced
    /// the state, and never from inside one.
    pub fn listen(
        &mut self,
        audio: &mut Audio,
        machine: &crate::screen::Machine,
        game: &crate::Game,
    ) {
        use crate::screen::ScreenId;
        self.ticks += 1;
        // `Tip_Show`'s `DAT_0052F004 = 0`, carried across the seam as a count.
        if game.tips.shows() != self.tip_shows {
            self.tip_shows = game.tips.shows();
            self.take_cursor = 0;
        }

        // **The three switches on the Sounds page**, which had no effect on
        // anything audible until this line: `screens::options` writes
        // `game.prefs.music` / `.effects` / `.speech` — the original's
        // `g_optMusic` (`0x0053F218`), `g_optSoundEffects` (`0x0053F214`) and
        // `g_optSpeech` (`0x0053F20C`), flipped by `Opt_ToggleMusic`
        // (`0x004349A4`) and its two neighbours — while [`Audio`] kept a second
        // copy of the same three flags that nothing ever wrote. Two
        // representations of one setting.
        // was the one nobody read.
        //
        // Pushed in this direction only: the audio
        // layer must not be reachable from a screen. Guarded on inequality
        // because the setter takes the mixer's lock and the answer changes
        // about once an hour.
        let want = Options {
            music: game.prefs.music,
            effects: game.prefs.effects,
            speech: game.prefs.speech,
        };
        // **`Opt_ToggleMusic`'s second statement, and it is not about music.**
        // `[V]`, the whole branch:
        //
        // ```c
        // g_optMusic = (g_optMusic != 1);
        // if (g_optMusic == 0) { Music_Stop(0); Sound_StopOneShot(); }
        // ```
        //
        // So turning the Music row off **also cuts whoever is speaking**, which
        // is not obvious from the row's label and is the only place in the
        // binary where one switch reaches another channel. Read before the push
        // because `audio.options()` is the previous tick's copy of `g_optMusic`
        // — the push below is what makes it the current one.
        let music_went_off = audio.options().music && !want.music;
        if want != audio.options() {
            audio.set_options(want);
        }
        if music_went_off {
            audio.stop_one_shot();
        }

        // **`Msg_Dismiss`'s (`0x00476768`) fourth statement** — the half of
        // closing a message window that is not drawing:
        //
        // ```c
        // if (g_messageGroup != 0xc2) Sound_StopOneShot();
        // ```
        //
        // `[V]`. **This is the answer to *"VO doesn't seem to stop when the
        // dialogue that produces it is closed"***: every narrated line in the
        // game is put in the one one-shot buffer by `Msg_PlayVoice`, and
        // closing the window is what empties it. Without this the narrator
        // finishes the sentence over whatever the player opened next, and —
        // worse — [`Audio::play_file`]'s drop-if-busy means the next window's
        // own fanfare is swallowed by a line nobody is reading any more.
        //
        // **Group `0xC2` is exempt**, and it is the one group the original
        // lets run on: `L2.eng` 194, *"Foiled again."*, the AI's lament.
        //
        // The edge is the open record *changing*, because `Msg_Dismiss` is a
        // call and a director can only see state. Every route into it is
        // covered — the OK button, `Msg_DismissUnlessQuestion`'s click on the
        // map, `Msg_Pump`'s two timeouts.
        // that dismiss themselves before starting a film. `[D]` on one case it
        // cannot see: two **identical** records back to back, where the window
        // is replaced by its own twin and this stays quiet. That is one voice
        // line too many, which is what the whole tick did before.
        let open_now = game.messages.open().copied();
        if let Some(was) = self.message {
            if open_now != Some(was) && was.group != crate::message::group::FOILED_AGAIN {
                audio.stop_one_shot();
            }
        }
        self.message = open_now;

        // **Nine of the original's fourteen music sites, and not one of them is
        // a call site of ours.** [`Audio::follow`] re-derives the bed from the
        // world every tick, so every site whose *whole* effect is "start the
        // track this phase should have" is reproduced by construction: a new
        // game, a battle starting, a siege starting, a county changing hands,
        // the music switch, the return to the battle-result screen and the two
        // in the resume-after-load path.
        //
        // The five it does **not** cover are the five that restart a bed a
        // **video** stopped, and they are recorded against the video rather
        // than against this line. See `docs/audio.json`.
        // sfx: Game_NewGame#1,Battle_Start#1,FUN_00477c89#1,County_ChangeOwner#1,Opt_ToggleMusic#1,Opt_ToggleMusic#2,FUN_004788c6#1,FUN_004976a1#1,FUN_004976a1#2
        audio.follow(scene(machine, game));

        let now = machine.ids();
        let opened = |want: &dyn Fn(&ScreenId) -> bool| {
            now.iter().any(|id| want(id)) && !self.stack.iter().any(|id| want(id))
        };

        // **`ff_batl.wav`, when a battle is announced.**
        // `Battle_ChooseSettlement` (`0x004A6A30`) plays it on the branch that
        // pushes screen `0x12` — the one where at least one side is human —
        // and takes no other sound with it, so this is the whole of that call
        // site. Our prompt appears on exactly that branch, so the screen
        // arriving *is* the event.
        //
        // `Sound_PlayFile("ff_batl.wav", 0, 0)`, with no stop in front: the
        // one-shot buffer, the Effects switch, and dropped while the buffer
        // sounds. `[V]`.
        // sfx: Battle_ChooseSettlement#1
        if opened(&|id| matches!(id, ScreenId::BattlePrompt)) {
            audio.play_file(names::fanfare::BATTLE, false);
            // **And the spoken question on top of it**.
            // statement: all three
            // functions that raise screen `0x12` follow the return with the
            // same three-way ladder on `g_battleChoiceOwner` —
            // `Battle_BeginFromCampaign` (`0x004A7158`), `FUN_004A6C68` and
            // `Siege_LaunchAssault` (`0x004A8AAB`). One edge, because there is
            // one screen and the three callers are three routes onto it.
            //
            // `docs/audio.json` filed all nine as *"which of the three a given
            // call plays is unread"*. It is `g_battleChoiceOwner`, which is
            // [`crate::turn::Question::choice_owner`].
            // the group-80 one: see [`names::speech::BATTLE_PROMPT`].
            //
            // **The fanfare above will usually swallow this**, in the original
            // and here, because both go through the one-shot buffer and bare
            // `Sound_PlayFile` drops what it cannot fit. That is reproduced by
            // making the calls in the original's order.
            // sfx: Battle_BeginFromCampaign#1,Battle_BeginFromCampaign#2,Battle_BeginFromCampaign#3,FUN_004a6c68#1,FUN_004a6c68#2,FUN_004a6c68#3,Siege_LaunchAssault#1,Siege_LaunchAssault#2,Siege_LaunchAssault#3
            if let Some(q) = crate::turn::pending_question(game) {
                if let Some(&name) = names::speech::BATTLE_PROMPT.get(q.choice_owner as usize) {
                    audio.play_file(name, true);
                }
            }
        }

        // **The narrator's interface commentary**, five screens' worth. Every
        // one of these is `Sound_PlayFile(name, 1, 0)` — the *speech* flag — in
        // the function that sets `g_screenId`, so the screen arriving is the
        // trigger and not a stand-in for it. See [`names::speech`].
        //
        // **Which of them wait and which cut in is the binary's, site by site.**
        // `Panel_OpenRation`, `Sidebar_Button` and `Panel_SplitButton` call
        // `Sound_PlayFile` bare.
        // dropped: [`Audio::play_file`]. Setup page 4's handlers put
        // `Sound_StopOneShot()` in front of it and interrupt:
        // [`Audio::stop_and_play_file`]. `[V]`, the statement before each call.
        //
        // **`Panel_OpenRation` is the one a player asked for**, and it is the
        // answer to *"sorely missing: 'All your people are fed by dairy'"*.
        // `docs/decisions.md` C133 searched every one of `L2.eng`'s 317 groups
        // for that sentence, correctly found nothing, and concluded the readout
        // did not exist. It exists; it is not text. `Panel_OpenRation`
        // (`0x0043A846`) speaks `S021_01.wav` when the county has a standing
        // herd and opening the larder took **neither a cow nor a sack** — which
        // is the condition, exactly — and `S021_02.wav` when the ration is not
        // met at all.
        //
        // The order is the original's `if / else if`: a county that is fed on
        // nothing gets the complaint, not the compliment.
        // sfx: Panel_OpenRation#1,Panel_OpenRation#2
        if opened(&|id| matches!(id, ScreenId::County(_, crate::screens::county::Panel::Ration))) {
            let county = now.iter().find_map(|id| match id {
                ScreenId::County(c, crate::screens::county::Panel::Ration) => Some(*c as usize),
                _ => None,
            });
            if let Some(c) = county.and_then(|c| game.kingdom.counties.get(c)) {
                if c.ration_achieved == 0 {
                    audio.play_file(names::speech::RATION_NOT_MET, true);
                } else if c.herd != 0 && c.herd_eaten == 0 && c.grain_eaten == 0 {
                    audio.play_file(names::speech::RATION_ON_DAIRY, true);
                }
            }
        }
        // **Setup page 4, *"Choose your title and your shield."*** Four
        // handlers reach it and every one of them plays `S011_02.wav` as it
        // sets `g_setupPage = 4`.
        // four are one sound. `FUN_00432B05`'s is the fourth and is **not**
        // claimed: it is the arm that runs after `Net_JoinGame` succeeds, and
        // we have no network join to arrive by.
        //
        // All four are `Sound_StopOneShot(); Sound_PlayFile(…, 1, 0)`, so the
        // page cuts the narrator off. `[V]`.
        // sfx: FUN_00432cc8#1,FUN_00432cc8#2,Setup_ChooseCampaign#1
        if opened(&|id| {
            matches!(id, ScreenId::Setup(crate::screens::setup::SetupPage::Shield))
        }) {
            audio.stop_and_play_file(names::speech::CHOOSE_YOUR_SHIELD, true);
        }
        // `Sidebar_Button` (`0x0043AE30`) hotspot 3. The ownership gate is
        // already ours: the sidebar refuses to open supplies on somebody
        // else's county, so reaching this screen *is* the guarded branch.
        // sfx: Sidebar_Button#1
        if opened(&|id| matches!(id, ScreenId::Supplies(_))) {
            audio.play_file(names::speech::SUPPLIES, true);
        }
        // **The mercenary offer** — `Sidebar_Button`'s *other* voice, on
        // hotspot 1.
        // of Scottish pikemen are available for hire, my lord'."*
        //
        // ```c
        // g_screenId = 0x17; …
        // if (county.mercenaryOffer != 0) FUN_004B3714(county.mercenaryOffer - 1);
        // ```
        //
        // `[V]`. Same shape as the supplies arm above — the ownership gate is
        // the screen's.
        // difference that had to be looked for: **`g_screenId = 0x17` has two
        // writers and only this one speaks.** `Armoury_Button` (`0x00435AE8`)
        // id 2, *Change*, sets the same screen id with no sound at all, and
        // ours is `Transition::Replace(RaiseArmy)` out of the armoury. So the
        // previous tick's stack is what tells the two arrivals apart, and a
        // player toggling Change / Continue does not hear the band announced
        // once a second.
        // sfx: FUN_004b3714#1
        if opened(&|id| matches!(id, ScreenId::RaiseArmy(_)))
            && !self.stack.iter().any(|id| matches!(id, ScreenId::Armoury(_)))
        {
            let band = now
                .iter()
                .find_map(|id| match id {
                    ScreenId::RaiseArmy(c) => Some(*c as usize),
                    _ => None,
                })
                .and_then(|c| game.kingdom.counties.get(c))
                .map_or(0, |c| c.mercenary_offer);
            // `FUN_004B3714`'s own guard is `-1 < n && n < 0x10` on the
            // *decremented* band, which is this.
            if let Some(name) =
                (band != 0).then(|| names::speech::MERCENARY_OFFER.get(band as usize - 1)).flatten()
            {
                // Bare `Sound_PlayFile(…, 1, 0)`, so dropped over anything
                // still sounding — a sidebar click is no more urgent than the
                // line already playing.
                audio.play_file(name, true);
            }
        }
        // **The population panel's health line** — `Panel_OpenPopulation`
        // (`0x0043A8F2`), whose middle statement is
        // `FUN_004B3768(county.healthBand)`. The exact twin of
        // `Panel_OpenRation`'s two arms above, on the panel next door.
        // table is indexed straight: see
        // [`names::speech::POPULATION_HEALTH`], entries 3 and 4.
        // sfx: FUN_004b3768#1
        if opened(&|id| matches!(id, ScreenId::County(_, crate::screens::county::Panel::Population)))
        {
            let band = now
                .iter()
                .find_map(|id| match id {
                    ScreenId::County(c, crate::screens::county::Panel::Population) => {
                        Some(*c as usize)
                    }
                    _ => None,
                })
                .and_then(|c| game.kingdom.counties.get(c))
                .map_or(0, |c| c.health_band);
            if let Some(name) = names::speech::POPULATION_HEALTH.get(band as usize) {
                audio.play_file(name, true);
            }
        }
        // **The map information panel's own sentence** — `FUN_004B37BC`, called
        // as the last statement of both functions that open screen `0x04`:
        // `FUN_0043893C` (the sidebar's route in) and `FUN_0043CAF4` (the map
        // click's). `[V]` Both are `… FUN_0041B032(); FUN_004B37BC();`, so the
        // panel arriving is the trigger, which is the edge
        // `TileInfo_Draw#1…#4` below already uses.
        //
        // The third writer of `g_screenId = 4`, `Army_SplitConfirm`
// (`0x00437AFB`), is silent: our
        // division screen pops back to whatever opened it.
        // sfx: FUN_004b37bc#1
        if opened(&|id| matches!(id, ScreenId::Info(_))) {
            use crate::screens::info::Target;
            let target = now.iter().find_map(|id| match id {
                ScreenId::Info(t) => Some(*t),
                _ => None,
            });
            let line = match target {
                // `g_pickedTileUnit != 0` — the unit ladder, which has no arm
                // for a merchant. See [`names::speech::PICKED_UNIT`].
                Some(Target::Unit(id)) => {
                    game.kingdom.campaign.units.get(id).and_then(|u| match u.kind {
                        l2_kingdom::UnitKind::Army => Some(if u.owner == game.player {
                            names::speech::PICKED_UNIT[3]
                        } else {
                            names::speech::PICKED_UNIT[2]
                        }),
                        l2_kingdom::UnitKind::Transport => Some(names::speech::PICKED_UNIT[1]),
                        l2_kingdom::UnitKind::PeasantMob => Some(names::speech::PICKED_UNIT[0]),
                        l2_kingdom::UnitKind::Merchant => None,
                    })
                }
                // The tile branch, all three guards: a settlement tile, its
                // graphic at the castle end of the ladder, and a castle type
                // in 1…5.
                Some(Target::Tile(tile)) => {
                    let map = &game.kingdom.campaign.map;
                    let settlement = map
                        .flags
                        .get(tile)
                        .is_some_and(|f| f & l2_kingdom::map::flags::SETTLEMENT != 0);
                    let castle = map.terrain.get(tile).is_some_and(|&g| g >= 0x15);
                    (settlement && castle)
                        .then(|| map.county.get(tile))
                        .flatten()
                        .and_then(|&c| game.kingdom.counties.get(c as usize))
                        .and_then(|c| {
                            names::speech::PICKED_CASTLE.get(c.castle_type.checked_sub(1)? as usize)
                        })
                        .copied()
                }
                None => None,
            };
            if let Some(name) = line {
                audio.play_file(name, true);
            }
        }
        // `Panel_SplitButton` (`0x004378B3`), and `FUN_004376BB` is the same
        // sound from the move-order confirm's split-into-a-castle path, which
        // we do not have. Both open `g_screenId` `0x11`.
        // sfx: Panel_SplitButton#1
        if opened(&|id| matches!(id, ScreenId::Divide(_))) {
            audio.play_file(names::speech::SPLIT_ARMY, true);
        }
        // **`Panel_JobDetail` (`0x00412B33`) — the village's work.** The job
        // popup opens with the sound of the job being done:
        // `Sound_RestartSlot(g_jobSound[job])` for six of the nine, and for the
        // blacksmith a forge and a hammer together. [`names::JOB_SOUND`] is the
        // table and [`names::blacksmith`] the branch.
        //
        // Our job number is zero-based and `g_jobPanelJob` is not, which is
        // what the `+ 1` is.
        // sfx: Panel_JobDetail#1,Panel_JobDetail#2,Panel_JobDetail#3
        if opened(&|id| matches!(id, ScreenId::Job(..))) {
            if let Some(job) = now.iter().find_map(|id| match id {
                ScreenId::Job(_, j) => Some(j + 1),
                _ => None,
            }) {
                if job == 8 {
                    // `Sound_PlayFile("fire.wav", 0, 0)` — the one-shot buffer,
                    // dropped while it sounds — and then the hammer's slot. `[V]`
                    audio.play_file(names::blacksmith::FIRE, false);
                    if let Some(n) = names::slot(names::Bank::Kingdom, names::blacksmith::SLOT) {
                        audio.play_effect(n);
                    }
                } else if let Some(&s) = names::JOB_SOUND.get(job) {
                    if let Some(n) = names::slot(names::Bank::Kingdom, s) {
                        audio.play_effect(n);
                    }
                }
            }
        }
        // **`TileInfo_Draw` (`0x0041C208`) — the industry sounds a player asked
        // for**, *"when you right click them on the map"*. The information
        // panel is screen `0x04` and its tile half plays the site's work as it
        // paints: a mine rings, a quarry and a smithy hammer, a lumber mill
        // saws. [`names::resource_site_slot`] is the ladder.
        //
        // The original calls it from the **painter**, which is fine there
        // because the painter runs on `g_redrawRequest` and ours runs sixty
        // times a second. So the edge is the panel opening, which is the same
        // occasion and not the same line. `[D]` — a repaint the original makes
        // for another reason would sound twice and ours will not.
        // sfx: TileInfo_Draw#1,TileInfo_Draw#2,TileInfo_Draw#3,TileInfo_Draw#4
        if opened(&|id| matches!(id, ScreenId::Info(crate::screens::info::Target::Tile(_)))) {
            if let Some(tile) = now.iter().find_map(|id| match id {
                ScreenId::Info(crate::screens::info::Target::Tile(t)) => Some(*t),
                _ => None,
            }) {
                let map = &game.kingdom.campaign.map;
                if map.flags.get(tile).is_some_and(|f| f & l2_kingdom::map::flags::SETTLEMENT != 0) {
                    if let Some(n) = map
                        .terrain
                        .get(tile)
                        .and_then(|&g| names::resource_site_slot(g))
                        .and_then(|s| names::slot(names::Bank::Kingdom, s))
                    {
                        audio.play_effect(n);
                    }
                }
            }
        }
        // **A film's own track.**
        //
        // `Smk_Open` starts the film's sound with its first frame and
        // `SmackClose` ends it — at the last frame, on a skip, or when
        // `Smk_OnFinished` opens the next film of the start-up sequence. So the
        // edge is the film on the stack *changing*.
        let film_of = |ids: &[ScreenId]| {
            ids.iter().find_map(|id| match id {
                ScreenId::Movie(f) => Some(*f),
                _ => None,
            })
        };
        let (was, is) = (film_of(&self.stack), film_of(&now));
        if was != is {
            match is {
                Some(film) => {
                    audio.play_film(film.file());
                    // `Msg_PlayVoice(DAT_004F0374, DAT_004F0354)` — the last
                    // line of both animated message branches, after `Smk_Play`
                    // has returned with the first frame up. So the narrator
                    // reads the capture or the fall over the film's opening,
                    // and he reads it whether or not the film opened.
                    //
                    // `#16`, the capture's copy of the same line, is served by
                    // this code and is not claimed: no capture letter is ever
                    // posted here — see the `Scene::Film` arm of `Audio::follow`.
                    // sfx: Msg_DrawWindow#21
                    if let Some((group, variant)) = film.voice() {
                        if let Some(name) = names::message_voice(group, variant) {
                            audio.stop_and_play_file(&name, true);
                        }
                    }
                }
                None => audio.stop_film(),
            }
        }
        self.stack = now;

        self.hear_the_click(audio, machine);

        // **`Map_ZoomOut` (`0x00434FD5`)**, whose last statement is
        // `Sound_PlayFile("S033_02.wav", 1, 0)`. It is a zoom *level*, not a
        // screen, so it is the one edge here that is not on the stack. There is
        // no matching sound on the way back in: `Map_ZoomIn` is silent.
        // sfx: Map_ZoomOut#1
        if game.map_zoom_far && self.zoom_far == Some(false) {
            // Bare `Sound_PlayFile`, so dropped over anything still sounding.
            audio.play_file(names::speech::ZOOM_OUT, true);
        }
        self.zoom_far = Some(game.map_zoom_far);

        // **The standings page saying its category out loud** —
        // `FUN_004B3994(DAT_0055CE7C)`, whose two callers are the court's
        // button (`FUN_004351C4`) and one of the page's seven tabs
        // (`FUN_0043524E`). Both speak unconditionally, so the edge is
        // [`crate::game::Game::nobles_spoken`] — a counter both call sites
        // bump — and **not** a diff on the category, which would swallow a
        // tab pressed twice and an open that did not change it.
        //
        // This is the sibling of the castle chooser's five, still `blocked`
        // for the reason this one no longer is: `docs/audio.json` says a
        // selection a screen keeps to itself is invisible here.
        // original keeps this one in the data segment.
        // sfx: FUN_004b3994#1
        if self.nobles_spoken != Some(game.nobles_spoken) {
            if self.nobles_spoken.is_some() {
                if let Some(name) =
                    names::speech::STANDINGS_CATEGORY.get(game.nobles_category as usize)
                {
                    // Bare `Sound_PlayFile(…, 1, 0)`, so dropped over anything
                    // still sounding.
                    audio.play_file(name, true);
                }
            }
            self.nobles_spoken = Some(game.nobles_spoken);
        }

        // **The lines a screen decided on and could not play.** Six of the
        // original's `Sound_PlayFile` sites are inside a screen's own handler,
        // guarded by state that screen keeps to itself and that is gone by the
        // next tick — so there is nothing here to diff and the screen reports
        // the line instead. See [`crate::game::Game::spoken`] for why the
        // count is the edge.
        //
        // Both groups are bare `Sound_PlayFile(name, 1, 0)`.
        // for while the buffer sounds is dropped: [`Audio::play_file`].
        if self.spoken != Some(game.spoken.0) {
            if self.spoken.is_some() && !game.spoken.1.is_empty() {
                audio.play_file(game.spoken.1, true);
            }
            self.spoken = Some(game.spoken.0);
        }

        self.hear_the_march(audio, game);

        self.hear_the_wreck(audio, game);

        self.hear_the_smithy(audio, game);

        self.hear_the_brush(audio, game);

        self.hear_the_battle(audio, game);

        // **The message window, which is where nearly all of the game's audio
        // lives.** 646 of the install's 771 files are somebody speaking, and
        // every one of them is played from `Msg_DrawWindow` (`0x0047309E`) or a
        // sibling it delegates to.
        //
        //.
        // player hears at the end of a turn is a message window opening.
        // Nothing on the `Turn_End` / `Season_Advance` / phase-7 path plays
        // anything, the End Turn button is silent, and both call sites of the
        // end-of-turn screen fade carry no sound either.
        //
        // Until the message window existed this fired once per *turn that
        // produced any message*, from `game.turns_played`, and said in a
        // comment that it was an approximation. It is gone: the window is real
        // and the trigger is the original's own.
        // reaching a value.
        //
        // **Equality, not a threshold, and that is deliberate.** The timer
        // decrements by one per tick (`MessageQueue::tick`), so each value
        // occurs exactly once per window and a `==` fires exactly once with no
        // memo to keep and nothing to reset when a window is dismissed early.
// It is also what the original tests. If the timer ever steps
        // by more than one this goes quiet: firing twice is impossible, which is
        // the failure worth having of the two.
        if let Some(record) = game.messages.open() {
            let timer = game.messages.timer();
            // **All five of `Msg_DrawWindow`'s `Sound_PlayFile` calls** — the
            // one `ff_capt.wav` and the four `ff_msg.wav`.
            // categories of one fanfare.
            // sfx: Msg_DrawWindow#1,Msg_DrawWindow#6,Msg_DrawWindow#11,Msg_DrawWindow#13,Msg_DrawWindow#17
            //
            // Each is `Sound_PlayFile(name, 1, 0)` with nothing in front: the
            // one buffer, dropped while it sounds, and — **the `1`** — the
            // Speech switch, not Sound Effects. `[V]`, all five.
            if timer == crate::message::TIMER_START {
                if let Some(fanfare) = open_fanfare(record.category, record.group) {
                    audio.play_file(fanfare, true);
                }
            }
            // **Thirteen of `Msg_PlayVoice`'s sixteen sites.** One ladder, one
            // condition each, and [`voice_tick`] is that ladder — so one call
            // answers all thirteen and it would be a fiction to write thirteen.
            //
            // Two it does not answer are `#16` and `#21`, the **animated**
            // capture and ending branches: those save the group and variant,
            // dismiss the message, play a Smacker film and speak afterwards
            // from `DAT_004F0374`/`DAT_004F0354`. They need video, not a tick.
            //
            // **`#24` is the tip window's first line**, and it was claimed here
            // once before it could sound: it is the categories `0x05`…`0x09`
            // branch, and `Tip_Show` (`0x00476DA9`) is the only function in the
            // original that posts one. It sounds now because `crate::tip` posts
            // tips — `S200_01.wav` ten ticks after *"Game Objectives:"* opens,
            // asserted by name in `tests/tips.rs`.
            // sfx: Msg_DrawWindow#2,Msg_DrawWindow#3,Msg_DrawWindow#4,Msg_DrawWindow#5,Msg_DrawWindow#7,Msg_DrawWindow#8,Msg_DrawWindow#9,Msg_DrawWindow#10,Msg_DrawWindow#12,Msg_DrawWindow#14,Msg_DrawWindow#18,Msg_DrawWindow#22,Msg_DrawWindow#23,Msg_DrawWindow#24
            if voice_tick(record.category) == Some(timer) {
                // `Msg_PlayVoice(g_messageGroup, g_messageVariant)`. A group
                // outside the three bands has no clip, and that is a message
                // the narrator does not read.
                //
                // All three of its bands are `Sound_StopOneShot();
                // Sound_PlayFile(name, 1, 0)`, so this cuts off whatever the
                // buffer holds — the window's own fanfare included — rather
                // than waiting for it. `[V]`
                if let Some(name) = names::message_voice(record.group, record.variant) {
                    audio.stop_and_play_file(&name, true);
                }
            } else if crate::tip::is_tip_window(record) && timer < 0x780 {
                // `else if (g_messageTimer < 0x780) FUN_004B3ACD(g_messageGroup);`
                self.chain_takes(audio, record.group);
            }
        }
    }

    /// **`FUN_004B3ACD` (`0x004B3ACD`) — the rest of a tip, read aloud.** `[V]`:
    ///
    /// ```c
    /// n = table[group * 5 + DAT_0052F004];                 /* 0x004E1E40 */
    /// if (n != 0) {
    ///     if (Sound_OneShotBusy()) _DAT_004E59FC = timeGetTime();
    ///     else if (timeGetTime() - _DAT_004E59FC > 999
    ///              && (DAT_0052F004++, n != 0) && n < 0x1F)
    ///         Sound_PlayFile("S201_02.wav" + (n - 1) * 0x10, 1, 0);
    /// }
    /// ```
    ///
    /// Called every frame of a tip window from eighty ticks in. So after the
    /// first line, each take waits for the narrator to fall silent and then a
    /// further **full second** — measured from the last frame he was heard, not
    /// from when he started — and the cursor stops on the first zero in the
    /// group's row. [`names::tip_take`] is the table.
    ///
    /// **The one divergence is the clock**, and it is stated: the original
    /// reads `timeGetTime()`, and this counts the director's own ticks at
    /// [`crate::TICK_MS`] each, so *"more than 999 ms"* is 63 ticks. A stall
    /// that drops frames stretches ours and not theirs. The stamp starts
    /// unset, where the original's starts at zero against a clock that has been
    /// running since boot — both mean *"long ago"*.
    // sfx: FUN_004b3acd#1
    fn chain_takes(&mut self, audio: &mut Audio, group: u16) {
        let n = names::tip_take(group, self.take_cursor);
        if n == 0 {
            return;
        }
        if audio.one_shot_busy() {
            self.take_busy_at = Some(self.ticks);
            return;
        }
        let quiet_for = self.take_busy_at.map_or(u64::MAX, |t| (self.ticks - t) * crate::TICK_MS as u64);
        if quiet_for > 999 {
            self.take_cursor += 1;
            if let Some(name) = names::take_name(n) {
                // Bare `Sound_PlayFile`. Its own drop cannot fire here — the
                // busy test above has just answered no — but it is the same call.
                audio.play_file(name, true);
            }
        }
    }

    /// **The pointer click** — `Sound_RestartSlot(1)`, `click3.wav`, from
    /// inside `Widget_Test` (`0x0040DA1E`).
    ///
    /// The trigger is not here: it is in [`crate::press::Press::press`] and
    /// [`crate::press::Press::press_delayed`], which are `Widget_Test`'s kind-4
    /// and kind-5 arms.
    /// only the far end of the wire — [`crate::screen::Machine::clicks`] is
    /// monotone, so *"it moved since the last tick"* is *"a widget was pressed"*
    /// and nothing a screen does can read it back (`docs/netcode.md` D-3).
    ///
    /// **What is silent is the half that is easy to get wrong**, and it is
    /// silent because nothing on those paths counts: the kind-4 auto-repeat's
    /// later pulses ([`crate::press::Press::tick`]), kind 5's delayed fire
    /// (also `tick`), `Hotspot_Test`'s three kinds, and `Ui_OkButtonClicked`.
    ///
    /// **Slot 1 is `click3.wav` in both banks** ([`names::KINGDOM_BANK`] and
    /// [`names::BATTLE_BANK`] both open with it), so which bank the original has
    /// loaded does not change the sound and the battlefield's confirm box clicks
    /// like the county's tax arrows. `Sound_RestartSlot` rewinds a sounding
    /// buffer, which is [`Audio::play_effect`] — and is also why two presses
    /// inside one tick are one sound here and would have been one in the
    /// original.
    /// **The hammer when the smithy is given a new weapon.**
    /// `FUN_0043A997` (`0x0043A997`) closes with
    ///
    /// ```c
    /// if (g_localPlayer == g_counties[county].owner) {
    ///     Panel_JobBlacksmith();          /* the page repaints in place */
    ///     Sound_RestartSlot(8);           /* stonecut.wav, the quarry's */
    /// }
    /// ```
    ///
    /// so the sound is the *setter's*, not the page's, and it is gated on the
    /// county being the local player's.
    /// [`names::blacksmith::SLOT`] is the same slot the page opens with.
    ///
    /// **Found**, for [`Director::hear_the_march`]'s
    /// reason: a "the weapon changed" flag on [`crate::Game`] would be in the
    /// save and in the lockstep digest, for a sound. `[D]` that the diff is the
    /// same occasion — the *only* writer of `weapon_type` outside this setter is
    /// `AI_ChooseIndustry`'s rota, which never runs on a county the local player
    /// owns.
    /// call.
    // sfx: FUN_0043a997#1
    fn hear_the_smithy(&mut self, audio: &mut Audio, game: &crate::Game) {
        let now: Vec<usize> = game.kingdom.counties.iter().map(|c| c.weapon_type).collect();
        if self.weapons.len() == now.len() {
            let local = game.player;
            let changed = now.iter().zip(&self.weapons).enumerate().any(|(id, (a, b))| {
                a != b && game.kingdom.counties.get(id).is_some_and(|c| c.owner == local)
            });
            if changed {
                if let Some(name) = names::slot(names::Bank::Kingdom, names::blacksmith::SLOT) {
                    audio.play_effect(name);
                }
            }
        }
        self.weapons = now;
    }

    /// **The field brush.** `FUN_00438B02` (`0x00438B02`) is the handler all
    /// five brush buttons call, and its first statement after `Map_ResolvePick`
    /// is the sound: [`names::field_brush_slot`] is that ladder.
    ///
    /// **Found**, for [`Director::hear_the_smithy`]'s
    /// reason — a "the brush painted" flag on [`crate::Game`] would be in the
    /// save and in the lockstep digest, for a sound, and a screen cannot reach
    /// [`Audio`] at all (`docs/netcode.md` D-3). What the original's handler
    /// does is observable instead: it paints the tile the information panel is
    /// about and then sets `g_screenId = 0`, so the panel is already gone by
    /// the tick this runs on and the remembered tile is the only thing left to
    /// diff.
    ///
    /// `[D]` that the diff is the same occasion: while the tile panel is up
    /// nothing else writes that tile's terrain — the weather and
    /// `Field_ReclaimTick` run on the turn, which the panel covers.
    // sfx: FUN_00438b02#1,FUN_00438b02#2,FUN_00438b02#3,FUN_00438b02#4,FUN_00438b02#5
    fn hear_the_brush(&mut self, audio: &mut Audio, game: &crate::Game) {
        use crate::screen::ScreenId;
        let map = &game.kingdom.campaign.map;
        if let Some((tile, was)) = self.brush_tile {
            if let Some(&is) = map.terrain.get(tile) {
                if is != was {
                    if let Some(name) = names::field_brush_slot(is)
                        .and_then(|s| names::slot(names::Bank::Kingdom, s))
                    {
                        audio.play_effect(name);
                    }
                }
            }
        }
        self.brush_tile = self.stack.iter().find_map(|id| match id {
            ScreenId::Info(crate::screens::info::Target::Tile(t)) => {
                map.terrain.get(*t).map(|&g| (*t, g))
            }
            _ => None,
        });
    }

    fn hear_the_click(&mut self, audio: &mut Audio, machine: &crate::screen::Machine) {
        let now = machine.clicks();
        if now != self.clicks {
            if let Some(name) = names::slot(names::Bank::Kingdom, 1) {
                audio.play_effect(name);
            }
        }
        self.clicks = now;
    }

    /// **The clip-clop.** `Unit_MoveInFacing` (`0x00466D84`) plays a sound as
    /// its first statement after unlinking the unit from the tile it is
    /// leaving. `[V]`:
    ///
    /// ```c
    /// if (kind == 3 || kind == 4) Sound_PlaySlot(0xb);   /* merchant.wav */
    /// else if (kind == 2)         Sound_PlaySlot(5);     /* rioters.wav  */
    /// else if (kind == 1)         Sound_PlaySlot(0xc);   /* army.wav     */
    /// ```
    ///
    /// # It is a rate, not an event
    ///
    /// There is **no guard on owner and none on visibility**: every step of
    /// every moving unit sounds, including an AI's on the far side of the map.
    /// So this is not "an army set off", it is the tempo of the campaign map,
    /// and it is the same number as `tests/pacing.rs`'s — one hoofbeat per tile
    /// per unit, eight ticks apart on a road and thirty-two off one. A player
    /// asked for *"the clip-clop of the merchants on end turn"* and it is
    /// audible pacing: before `Unit_StepOnce`'s sub-tile counter landed this
    /// would have been a single 180 ms gallop at the end of every turn.
    ///
    /// # Why it does not become a roar
    ///
    /// `Sound_PlaySlot` is the **drop-if-busy** verb, so twelve units marching
    /// cost one voice per distinct sound.
    /// [`Audio::play_effect_if_idle`] is exactly that verb and there is
    /// deliberately no throttle of our own on top of it: if this ever needs
    /// one, the mixer is wrong.
    ///
    /// # Why the movement is *found*
    ///
    /// Nothing in the simulation hands the audio layer an event, and that is
    /// the property the whole module rests on: [`Director::listen`] takes
    /// `&Game`.
    /// value or timing (`docs/netcode.md` D-3). Threading a "who stepped"
    /// report out of `l2_kingdom::units_tick` and along to here would have put
    /// it on the [`crate::Game`], which is to say in the save and in the
    /// lockstep digest, for a sound. Diffing 150 tiles a tick is cheaper than
    /// that in every sense that matters.
    ///
    /// **The one thing it cannot tell apart** is a step from a teleport: an
    /// army garrisoning a castle is put on the keep's tile by
    /// `Army_GarrisonApply` and sounds here as though it walked there. `[D]` —
    /// one extra hoofbeat, dropped if the sound is already playing.
    /// alternative was the report above.
    fn hear_the_march(&mut self, audio: &mut Audio, game: &crate::Game) {
        use l2_kingdom::UnitKind;

        let mut now = vec![None; l2_kingdom::unit::MAX_UNITS];
        for (id, u) in game.kingdom.campaign.units.iter() {
            now[id] = Some((u.kind, u.tile()));
        }
        for (id, entry) in now.iter().enumerate() {
            let (Some((kind, at)), Some(Some((was, from)))) = (entry, self.tiles.get(id)) else {
                continue;
            };
            // A slot that has been reused is a different unit standing
            // somewhere else, not a march.
            if kind != was || at == from {
                continue;
            }
            let slot = match kind {
                UnitKind::Army => 0xc,
                UnitKind::PeasantMob => 5,
                UnitKind::Merchant | UnitKind::Transport => 0xb,
            };
            // sfx: Unit_MoveInFacing#1,Unit_MoveInFacing#2,Unit_MoveInFacing#3,Unit_MoveInFacing#4
            if let Some(name) = names::slot(names::Bank::Kingdom, slot) {
                audio.play_effect_if_idle(name);
            }
        }
        self.tiles = now;
    }

    /// **Something an army marched over was wrecked** — `dest_ind.wav`, bank
    /// slot 3, from all six of the original's `Sound_RestartSlot(3)` calls:
    ///
    /// | site | what it wrecked | our tile write |
    /// |---|---|---|
    /// | `Unit_BurnDwelling#1` (`0x00468AE2`) | a dwelling | content `0x10` → `0x13` on a plot |
    /// | `Unit_TrampleTile#1…4` (`0x0046873F`) | one of the four industry sites | content → 3, 6, 9, 12 on a settlement |
    /// | `Unit_CrossField#1` (`0x0046673C`) | a standing field | content → 0 on farmland |
    ///
    /// Every one of the six is the statement *after* the tile write, so the
    /// wrecked tile **is** the trigger and there is nothing to approximate.
    ///
    /// # Why it diffs the plane
    ///
    /// Same reason as [`Director::hear_the_march`], and the same shape:
    /// [`Director::listen`] holds `&Game`, so threading a report out of
    /// `l2_kingdom::movement::Step` and along to here would put it on the
    /// [`crate::Game`] — in the save and in the lockstep digest — for a sound
    /// (`docs/netcode.md` D-3). One pass over the 4,096-byte content plane is
    /// cheaper than that in every sense.
    ///
    /// **The classification is by the value written, and it is exact because
    /// `l2_kingdom` has exactly three writers of the plane** — `destroy_field`,
    /// `trample` and the [`l2_kingdom::movement::Entry::Plot`] arm, which are
    /// these three rows. A fourth writer (a season that re-sows a field, say)
    /// would have to be excluded here, and the field row is the one it would
    /// reach: `0` is also a bare field.
    ///
    /// `Sound_RestartSlot` is the **rewind-and-play** verb, so a second wreck
    /// in the same tick restarts the clip —
    /// [`Audio::play_effect`], not `play_effect_if_idle`. Six tiles wrecked by
    /// one march is still one sound, because one buffer is one voice.
    fn hear_the_wreck(&mut self, audio: &mut Audio, game: &crate::Game) {
        use l2_kingdom::map::{flags, terrain};

        let now = &game.kingdom.campaign.map.terrain;
        if self.terrain.len() == now.len() {
            let map = &game.kingdom.campaign.map;
            let wrecked = now.iter().zip(&self.terrain).enumerate().any(|(i, (&to, &from))| {
                if to == from {
                    return false;
                }
                let f = map.flags[i];
                // sfx: Unit_BurnDwelling#1
                if f & flags::PLOT != 0 {
                    return from == terrain::DWELLING && to == terrain::DWELLING_BURNT;
                }
                // sfx: Unit_TrampleTile#1,Unit_TrampleTile#2,Unit_TrampleTile#3,Unit_TrampleTile#4
                if f & flags::SETTLEMENT != 0 {
                    return l2_kingdom::movement::RUIN_LADDER.iter().any(|&(_, r, _)| to == r);
                }
                // sfx: Unit_CrossField#1
                if f & flags::FARMLAND != 0 {
                    return to == 0
                        && (terrain::FIELD_STANDING_FROM..terrain::FIELD_STANDING_TO)
                            .contains(&from);
                }
                false
            });
            if wrecked {
                if let Some(name) = names::slot(names::Bank::Kingdom, 3) {
                    audio.play_effect(name);
                }
            }
        }
        if self.terrain.len() != now.len() || self.terrain != *now {
            self.terrain = now.clone();
        }
    }

    /// **The battlefield: the men's cries.**
    ///
    /// Two sources, one per kind of occasion, and neither is a report the
    /// simulation hands over:
    ///
    /// * **the cries** are [`crate::battlefield::LiveBattle::cries`], a list the
    ///   six cry arms append to as a player selects and orders — `Battle_DragSelect`,
    ///   `Battle_OrderSelection` and `Battle_FormationKey`. Each is played through
    ///   [`TroopCries`], which is `Sound_PlayTroopCry`'s body;
    /// * **the fighting** is [`l2_sim::Cues`], the per-man record the battle
    ///   keeps (`crate::cue` in `l2-sim` makes counters exact.
    ///   approximation). [`battle_requests`] turns the counts that moved since the
    ///   last tick into the original's calls.
    ///
    /// Cries first, because they are input and our machine handles input before
    /// the tick that follows it. The two can only meet on the one-shot buffer,
    /// which a wall coming down and a cry share.
    ///
    /// **Why this cannot change the battle**: it holds `&Game`. A test that plays
    /// one battle with this listening and one without, and requires the two to
    /// agree at every tick, is `tests/audio_battle.rs`.
    fn hear_the_battle(&mut self, audio: &mut Audio, game: &crate::Game) {
        let Some(live) = game.battle.as_deref() else {
            self.cues = None;
            self.cries_heard = 0;
            return;
        };

        // A list shorter than what was heard is a different battle.
        let from = if live.cries.len() < self.cries_heard { 0 } else { self.cries_heard };
        for cry in &live.cries[from..] {
            // **The take advances whether or not the cry is heard** —
            // `Sound_PlayTroopCry` steps its counter and only then calls
            // `Sound_PlayFile`, which may drop it. So `cry` runs first and
            // unconditionally.
            if let Some(name) = self.troop_cries.cry(cry.troop, cry.class) {
                audio.play_file(name, true);
            }
        }
        self.cries_heard = live.cries.len();

        let now = live.runner.sim.cues;
        let was = match self.cues {
            Some(c) if !now.is_behind(&c) => c,
            // The first tick of a battle, or a new battle: everything since zero.
            _ => l2_sim::Cues::default(),
        };
        for request in battle_requests(&was, &now) {
            match request {
                Request::Slot(slot) => {
                    if let Some(name) = names::slot(names::Bank::Battle, slot) {
                        audio.play_effect_if_idle(name);
                    }
                }
                Request::File(name) => {
                    audio.play_file(name, false);
                }
            }
        }
        self.cues = Some(now);
    }
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

