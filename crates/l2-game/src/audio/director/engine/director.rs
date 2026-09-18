#![allow(unused_imports)]
use super::*;

use super::*;
use super::*;
use super::events::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mixer::Mixer;
use track::{BattleCycle, BattleKind, Music};
use wav::Sound;

impl Director {
    pub fn new() -> Director {
        Director::default()
    }

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
        let want = Options {
            music: game.prefs.music,
            effects: game.prefs.effects,
            speech: game.prefs.speech,
        };
        // `[V]`, the whole branch:
        //
        // So turning the Music row off **also cuts whoever is speaking**, which
        // is not obvious from the row's label and is the only place in the
        // binary where one switch reaches another channel. Both arms of the
        // stop are in [`Audio::set_options`], which is where `Opt_ToggleMusic`
        // (`0x004349A4`) is reproduced: off empties the buffer, and on during a
        // battle empties it through `Music_StartBattle` (`0x00477B2F`).
        if want != audio.options() {
            audio.set_options(want);
        }

        // **`Msg_Dismiss`'s (`0x00476768`) fourth statement** — the half of
        // closing a message window that is not drawing:
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

        // sfx: Game_NewGame#1,Battle_Start#1,FUN_00477c89#1,County_ChangeOwner#1,Opt_ToggleMusic#1,Opt_ToggleMusic#2,FUN_004788c6#1,FUN_004976a1#1,FUN_004976a1#2
        audio.follow(scene(machine, game));

        let now = machine.ids();
        let opened = |want: &dyn Fn(&ScreenId) -> bool| {
            now.iter().any(|id| want(id)) && !self.stack.iter().any(|id| want(id))
        };

        // `Battle_ChooseSettlement` (`0x004A6A30`) plays it on the branch that
        // pushes screen `0x12` — the one where at least one side is human —
        // and takes no other sound with it, so this is the whole of that call
        // site. Our prompt appears on exactly that branch, so the screen
        // arriving *is* the event.
        //
        // `Sound_PlayFile("ff_batl.wav", 0, 0)`, with no stop in front: the
        // one-shot buffer, the Effects switch, and dropped while the buffer
        // sounds. `[V]`.
        //
        // sfx: Battle_ChooseSettlement#1
        if opened(&|id| matches!(id, ScreenId::BattlePrompt)) {
            audio.play_file(names::fanfare::BATTLE, false);
            // statement: all three
            // functions that raise screen `0x12` follow the return with the
            // same three-way ladder on `g_battleChoiceOwner` —
            // `Battle_BeginFromCampaign` (`0x004A7158`), `FUN_004A6C68` and
            // `Siege_LaunchAssault` (`0x004A8AAB`). One edge, because there is
            // one screen and the three callers are three routes onto it.
            //
            // sfx: Battle_BeginFromCampaign#1,Battle_BeginFromCampaign#2,Battle_BeginFromCampaign#3,FUN_004a6c68#1,FUN_004a6c68#2,FUN_004a6c68#3,Siege_LaunchAssault#1,Siege_LaunchAssault#2,Siege_LaunchAssault#3
            if let Some(q) = crate::turn::pending_question(game) {
                if let Some(&name) = names::speech::BATTLE_PROMPT.get(q.choice_owner as usize) {
                    audio.play_file(name, true);
                }
            }
        }

        // [`Audio::stop_and_play_file`]. `[V]`, the statement before each call.
        //
        // `docs/decisions.md` C133 searched every one of `L2.eng`'s 317 groups
        // for that sentence, correctly found nothing, and concluded the readout
        // did not exist. It exists; it is not text. `Panel_OpenRation`
        // (`0x0043A846`) speaks `S021_01.wav` when the county has a standing
        // herd and opening the larder took **neither a cow nor a sack** — which
        // is the condition, exactly — and `S021_02.wav` when the ration is not
        // met at all.
        //
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
        // four are one sound. `FUN_00432B05`'s is the fourth and is **not**
        // claimed: it is the arm that runs after `Net_JoinGame` succeeds, and
        // we have no network join to arrive by.
        //
        // All four are `Sound_StopOneShot(); Sound_PlayFile(…, 1, 0)`, so the
        // page cuts the narrator off. `[V]`.
        //
        // sfx: FUN_00432cc8#1,FUN_00432cc8#2,Setup_ChooseCampaign#1
        if opened(&|id| {
            matches!(id, ScreenId::Setup(crate::screens::setup::SetupPage::Shield))
        }) {
            audio.stop_and_play_file(names::speech::CHOOSE_YOUR_SHIELD, true);
        }
        // `Sidebar_Button` (`0x0043AE30`) hotspot 3. The ownership gate is
        // already ours: the sidebar refuses to open supplies on somebody
        // else's county, so reaching this screen *is* the guarded branch.
        //
        // sfx: Sidebar_Button#1
        if opened(&|id| matches!(id, ScreenId::Supplies(_))) {
            audio.play_file(names::speech::SUPPLIES, true);
        }
        // ```c
        // g_screenId = 0x17; …
        // if (county.mercenaryOffer != 0) FUN_004B3714(county.mercenaryOffer - 1);
        // ```
        //
        // `[V]`. Same shape as the supplies arm above — the ownership gate is
        // the screen's.
        //
        // difference that had to be looked for: **`g_screenId = 0x17` has two
        // writers and only this one speaks.** `Armoury_Button` (`0x00435AE8`)
        // id 2, *Change*, sets the same screen id with no sound at all, and
        // ours is `Transition::Replace(RaiseArmy)` out of the armoury. So the
        // previous tick's stack is what tells the two arrivals apart, and a
        // player toggling Change / Continue does not hear the band announced
        // once a second.
        //
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
                audio.play_file(name, true);
            }
        }
        // **The population panel's health line** — `Panel_OpenPopulation`
        // (`0x0043A8F2`), whose middle statement is
        // `FUN_004B3768(county.healthBand)`. The exact twin of
        // `Panel_OpenRation`'s two arms above, on the panel next door.
        //
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
        //
        // `FUN_0043893C` (the sidebar's route in) and `FUN_0043CAF4` (the map
        // click's). `[V]` Both are `… FUN_0041B032(); FUN_004B37BC();`, so the
        // panel arriving is the trigger, which is the edge
        // `TileInfo_Draw#1…#4` below already uses.
        //
        // The third writer of `g_screenId = 4`, `Army_SplitConfirm`
// (`0x00437AFB`), is silent: our
        // division screen pops back to whatever opened it.
        //
        // sfx: FUN_004b37bc#1
        if opened(&|id| matches!(id, ScreenId::Info(_))) {
            use crate::screens::info::Target;
            let target = now.iter().find_map(|id| match id {
                ScreenId::Info(t) => Some(*t),
                _ => None,
            });
            let line = match target {
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
        //
        // sfx: Panel_SplitButton#1
        if opened(&|id| matches!(id, ScreenId::Divide(_))) {
            audio.play_file(names::speech::SPLIT_ARMY, true);
        }
        // **`Panel_JobDetail` (`0x00412B33`) — the village's work.** The job
        // popup opens with the sound of the job being done:
        //
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

        // **And the buffer is emptied when the popup goes** — `Screen_FrameInput`
        // (`0x0042FF10`) does it on both routes out of screen `0x0F`, `[V]`:
        //
        // ```c
        // /* the OK button or a right-release: */
        // g_screenId = (DAT_005533f4 == 0) ? 2 : 0; DAT_005533f4 = 0;
        // Sound_StopOneShot(); g_redrawRequest = 2; FUN_0041438c();
        // /* and a press anywhere on the map: */
        // if (g_screenId == 0x0f) { Sound_StopOneShot(); FUN_0041438c(); }
        // ```
        //
        // `[D]`, twice and narrowly: the map-press route stops the buffer on
        // the press even when `g_battlePhase != 0` keeps the popup up, and
        // ours fires only where the popup actually goes; and our Job screen
        // pops on Escape/Enter — an arm labelled ours — which reaches this
        // stop as well.
        if self.stack.iter().any(|id| matches!(id, ScreenId::Job(..)))
            && !now.iter().any(|id| matches!(id, ScreenId::Job(..)))
        {
            audio.stop_one_shot();
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
        //
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
                    // sfx: Msg_DrawWindow#16,Msg_DrawWindow#21
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
        //
        // sfx: Map_ZoomOut#1
        if game.map_zoom_far && self.zoom_far == Some(false) {
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
        // sfx: FUN_004b3994#1
        if self.nobles_spoken != Some(game.nobles_spoken) {
            if self.nobles_spoken.is_some() {
                if let Some(name) =
                    names::speech::STANDINGS_CATEGORY.get(game.nobles_category as usize)
                {
                    audio.play_file(name, true);
                }
            }
            self.nobles_spoken = Some(game.nobles_spoken);
        }

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
        //.
        if let Some(record) = game.messages.open() {
            let timer = game.messages.timer();
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
            //
            // sfx: Msg_DrawWindow#2,Msg_DrawWindow#3,Msg_DrawWindow#4,Msg_DrawWindow#5,Msg_DrawWindow#7,Msg_DrawWindow#8,Msg_DrawWindow#9,Msg_DrawWindow#10,Msg_DrawWindow#12,Msg_DrawWindow#14,Msg_DrawWindow#18,Msg_DrawWindow#22,Msg_DrawWindow#23,Msg_DrawWindow#24
            if voice_tick(record.category) == Some(timer) {
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
                audio.play_file(name, true);
            }
        }
    }

    /// **The pointer click** — `Sound_RestartSlot(1)`, `click3.wav`, from
    /// inside `Widget_Test` (`0x0040DA1E`).
    ///
    /// `FUN_0043A997` (`0x0043A997`) closes with
    ///
    /// **Found**, for [`Director::hear_the_march`]'s
    /// reason: a "the weapon changed" flag on [`crate::Game`] would be in the
    /// save and in the lockstep digest, for a sound. `[D]` that the diff is the
    /// same occasion — the *only* writer of `weapon_type` outside this setter is
    /// `AI_ChooseIndustry`'s rota, which never runs on a county the local player
    /// owns.
    ///
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
    /// `[D]` that the diff is the same occasion: while the tile panel is up
    /// nothing else writes that tile's terrain — the weather and
    /// `Field_ReclaimTick` run on the turn, which the panel covers.
    ///
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
    /// **The one thing it cannot tell apart** is a step from a teleport: an
    /// army garrisoning a castle is put on the keep's tile by
    /// `Army_GarrisonApply` and sounds here as though it walked there. `[D]` —
    /// one extra hoofbeat, dropped if the sound is already playing.
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

    /// | site | what it wrecked | our tile write |
    /// |---|---|---|
    /// | `Unit_BurnDwelling#1` (`0x00468AE2`) | a dwelling | content `0x10` → `0x13` on a plot |
    /// | `Unit_TrampleTile#1…4` (`0x0046873F`) | one of the four industry sites | content → 3, 6, 9, 12 on a settlement |
    /// | `Unit_CrossField#1` (`0x0046673C`) | a standing field | content → 0 on farmland |
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

    fn hear_the_battle(&mut self, audio: &mut Audio, game: &crate::Game) {
        let Some(live) = game.battle.as_deref() else {
            self.cues = None;
            self.cries_heard = 0;
            return;
        };

        let from = if live.cries.len() < self.cries_heard { 0 } else { self.cries_heard };
        for cry in &live.cries[from..] {
            if let Some(name) = self.troop_cries.cry(cry.troop, cry.class) {
                audio.play_file(name, true);
            }
        }
        self.cries_heard = live.cries.len();

        let now = live.runner.sim.cues;
        let was = match self.cues {
            Some(c) if !now.is_behind(&c) => c,
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


