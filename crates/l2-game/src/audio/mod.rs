//! **Sound.** The game shipped 771 `.wav` files and 396 MB of audio and none
//! of it has ever played; this is the layer that plays some of it.
//!
//! # Where this may live, and why it may not live anywhere else
//!
//! In `l2-game`, with `winit` and `pixels`, and nowhere below it.
//! `docs/netcode.md` D-3: everything under this crate is dependency-free and
//! deterministic on purpose, and a crate that opens an audio device is a crate
//! whose behaviour depends on the machine it is on.
//!
//! **A sound must never be able to affect the simulation**, in timing or in
//! ordering, because the lockstep argument depends on it. That is enforced by
//! shape: [`Audio`] is owned by the event loop in
//! `main.rs` and is *not* in [`crate::screen::Ctx`].
//! it, cannot ask whether a sound finished, and cannot branch on one. The
//! information flows one way — the event loop looks at the world and decides
//! what should be audible. Nothing looks back.
//!
//! That constraint is also what fixes the design. Music is not pushed by
//! events; it is **derived**, once a tick, from what is on screen and how the
//! player's realm is doing, by [`Audio::follow`]. Asking for a track that is
//! already playing does nothing.
//! music never restarts.
//!
//! # It is optional, and it is optional at run time
//!
//! A machine with no audio device, no default output configuration, or an
//! install with the `.wav` files stripped must behave as it did
//! before. So [`Audio::open`] cannot fail: every step that could — finding a
//! host, finding a device, building a stream, resolving a file, decoding it —
//! degrades to silence, and [`Audio::silent`] is the same object with the
//! device left out. `--no-sound` selects it explicitly.
//!
//! Degradation happens at runtime.
//! is not the code that ships.
//! run time anyway.
//!
//! # Files are resolved, never named by path
//!
//! Sounds come out of [`l2_mods::vfs::Vfs`] like every other asset, which
//! means the install is found once, the casing is normalised once — the
//! install spells them `Scroll1.wav` and the executable spells them
//! `scroll1.wav` — and **a mod can replace a sound by dropping a file in a
//! layer**, for free, with no code here knowing.
//!
//! # What this does not do
//!
//! **Read this before believing the layer plays anything.** This list was wrong
//! twice: it claimed the pointer click, which has never had a call site, and
//! the sentence above it claimed music, which [`scene`] could not reach. So the
//! counts here are **measured by `tests/audio_wiring.rs`, not typed**, and a
//! new call site moves them by itself.
//!
//! Of the install's **771** `.wav` files, this layer can reach **674**:
//!
//! | | files | how |
//! |---|---:|---|
//! | the narrator | **541** | 448 lord takes + 93 system clips, via [`Director`] and [`voice_tick`] — eleven of the 93 are tips' first lines, since `crate::tip` posts tips |
//! | the tips' chained takes | 24 | [`Director::chain_takes`] — 27 in the table, three behind battle tips nothing can post |
//! | **the troop cries** | **66** | [`TroopCries`], from the six cry arms on [`crate::battlefield::LiveBattle`] |
//! | music | 10 | `scroll1`…`scroll5`, `battle1`…`battle4`, `setup` |
//! | fanfares | 3 | `ff_msg`, `ff_batl`, `ff_capt` |
//! | the screen class | 16 | six spoken lines and ten bank slots, by [`Director::listen`]'s screen edges |
//! | **the fighting** | **13** | twelve battle-bank slots and `bathit2.wav`, by [`battle_requests`] |
//! | the pointer click | 1 | `click3.wav`, by [`Director::hear_the_click`] — `Widget_Test`'s two live sites |
//!
//! **The narrator is 84 % of the game's audio by file count** — 646 of the 771
//! files are somebody speaking — a player calls the voice acting
//! *"half the personality of the game"*. It went from 0 to 543 in one change,
//! and it is because the whole class
//! hangs off one trigger.
//!
//! **A previous count here said 12 and was wrong at 11**, by reading the battle
//! table instead of driving it — `battle5.wav` ships, decodes, and cannot be
//! asked for, because the counter that selects it is `DAT_0057A0F0`, the third
//! battle mode [`track::BattleKind`] declines to guess at.
//!
//! **The count of triggers is `docs/audio.json`, and it is checked.**
//! written.** `crates/l2-game/tests/sfx.rs` requires the set of sites that file
//! calls `reproduced` to equal the set of `// sfx:` markers in `crates/`, and
//! `node tools/oracle/sounds.js --check` requires the file's *rows* to equal
//! what the decompilation holds. So the sentence below cannot go stale without
//! something going red, which is the whole reason it is safe to write a number
//! here at all — the previous version of this table was prose, was hand-marked,
//! and was wrong in both directions.
//!
//! **80 of 143, and 3 of the 143 are dead in the shipped game.** The
//! denominator moved because the enumeration was one primitive short: see
//! [`track::Music::Setup`].
//!
//! | class | the original's call site | sites | ours |
//! |---|---|---:|---:|
//! | **Message narration** | `Msg_PlayVoice` `0x004B35C1` | 16 | **15** — [`voice_tick`].
//! | **Music** | `Music_StartCampaign`, `Music_StartBattle`, `Music_Play` | 23 | **15** — [`scene`], four of them the bed restarting after a film ([`Scene::Film`]) |
//! | **By name** | `Sound_PlayFile` | 49 | **17** — [`names::speech`], the fanfares, the forge, the tips' chain and `Wall_Smash`; every one through [`Audio::play_file`] or, where the original stops the buffer first, [`Audio::stop_and_play_file`] |
//! | **The two sample banks** | `Sound_PlaySlot`, `Sound_RestartSlot`, `FUN_004262CF` | 49 | **27** — the march, the sites, the village's work, the click, and fifteen on the battlefield |
//! | **Troop cries** | `Sound_PlayTroopCry` `0x00499CB1` | 6 | **6** — [`TroopCries`] |
//!
//! What is left, in the order a player notices it:
//!
//! * **Three battlefield sites, each on a mechanic.** This
//!   bullet used to say the whole battlefield — 25 sites — was *"a limit of the
//!   design, because a sword swing is an event inside a
//!   tick and [`Director`] derives sound from the world after it. The premise
//! was right: the world did not *record*
//!   the event. `l2_sim::cue` is that record — monotone counts the battle writes
//!   and never reads — and because every battlefield call is drop-if-busy, a
//!   count that moved since the last tick is exactly what the original's calls
//!   could make audible. Twenty-two of the 25 sound now: sixteen from the
//!   record as it was written, and six more once fire, boiling oil, a tower
//! docking and the rampart too high to shoot down were built in
//!   `l2_sim::fire` and `l2_sim::siege`. The three that do not are state 17's
//!   own loose and a realm eliminated mid-battle — each named in
//!   `docs/audio.json`, and each a mechanic `l2-sim` does not have.
//! * **The two sample banks elsewhere.** [`names::KINGDOM_BANK`] and
//!   [`names::BATTLE_BANK`] are recovered and tested against the install; 27 of
//!   their 29 slots ship. The campaign half of the kingdom bank now sounds —
//!   the march, the resource sites, the village's work — and `dest_ind.wav`'s
//! six campaign sites.
//!   not. (The battlefield's bridge fire plays the same `dest_ind.wav` by name,
//!   so the file is reachable; those six sites are not.)
//!
//!   > This used to end *"and **nothing calls
//!   > [`Audio::play_effect_if_idle`]**"*, and that is no longer true. The
//!   > clip-clop a player asked for is `Unit_MoveInFacing` (`0x00466D84`)
//!   > calling `Sound_PlaySlot(0xb)` **on every step of every moving unit** —
//!   > a rate, not a trigger, and it works only because `Sound_PlaySlot` drops
//!   > the request when that buffer is still playing, which is exactly what
//!   > [`Audio::play_effect_if_idle`] is. [`Director::hear_the_march`] is that
//!   > call site, one hoofbeat per tile per unit, and it is the first caller
//!   > the drop-if-busy verb has had. See `docs/audio-triggers.md`.
//! * **The tip screens**, which silence two things at once: every tip's first
//!   line (`Msg_DrawWindow#24`) and `FUN_004B3ACD`'s chain of further takes —
//!   40 files. The chain has one caller, `Msg_DrawWindow`'s categories
//!   `0x05`…`0x09` branch; it plays `S201_02.wav + (n - 1) * 0x10` one clip at a
//!   time, a full second after `Sound_OneShotBusy()` last saw one playing, and
//!   its cursor is reset only in `Tip_Show` (`0x00476DA9`) — the only function in
//!   the original that posts those categories. **This bullet used to say the
//!   blocker was a per-message cursor and that the chain was *"why
//!   `S010_13.wav` exists"*.** The cursor is two globals and a table, and
//! [`Audio::is_playing`] now answers the gate.
//!   `Tip_Update`, `Tip_Show`, the paragraph window and its computed OK corner.
//!   `S010_13.wav` is `g_msgVoiceS010`'s, which `FUN_004B36C0` reads. The
//!   letter's sting, `FUN_004B3B92(lord - 1)`, is `S246_02.wav + lord * 0x10`
//!   over four clips and a sentinel.
//! * **`ff_lose.wav`** needs a gate — see
//!   [`names::fanfare::AFTER_BATTLE`]. It is the only fanfare left.

mod engine;
pub use engine::*;
mod director;
pub use director::*;
mod events;
pub use events::*;

pub mod mixer;
pub mod names;
pub mod track;
pub mod wav;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use mixer::Mixer;
use track::{BattleCycle, BattleKind, Music};
use wav::Sound;

/// The three switches the original's sound options screen (`0x42`) has, in its
/// order: `Opt_ToggleMusic`, `Opt_ToggleSoundEffects`, `Opt_ToggleSpeech`
/// (`0x004349A4`, `0x00434A29`, `0x00434A9A`), writing `g_optMusic`,
/// `g_optSoundEffects` and `g_optSpeech`.
///
/// **Three flags, no volumes.** The original has no per-channel level:
/// the slider screen's *"Adjusting music level"* prompt belongs to a different
/// panel, and `L2.eng` group 57's *"Music is"* is from a superseded combined
/// options screen that no live function reaches. So this is what the game had,
/// and inventing a mixer here would be inventing a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub music: bool,
    pub effects: bool,
    pub speech: bool,
}

impl Default for Options {
    /// All three on. `FUN_004AF35E` sets `g_optMusic`, `g_optSoundEffects` and
    /// `g_optSpeech` to 1 at start-up.
    fn default() -> Options {
        Options { music: true, effects: true, speech: true }
    }
}

/// What the world sounds like right now — the *input* to the music policy.
///
/// A plain value, computed by the caller from the game and the screen stack,
/// so that [`Audio::follow`] is a pure decision and can be tested without
/// either. It is deliberately tiny: anything that is not one of these three
/// things cannot change the music.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// **Before a game exists — the front end, and it is not silent.**
    ///
    /// This arm used to answer `None`, on the reasoning that
    /// `Music_StartCampaign` is only reached once the campaign comes up and
    /// *"the title screen's only sound is `setup.wav`"*. Both halves are true
    ///: `setup.wav` **is** the music, played
    /// by `Music_Play(name, 0, 1)` — the same looping call the campaign picker
    /// ends in, reached from a function the eight-primitive audit did not
    /// enumerate. See [`track::Music::Setup`].
    FrontEnd,
    /// The campaign map, a county panel, the village — anything in the
    /// management surface. Carries what the ladder reads.
    Campaign { county_count: u8, share_of_map_pct: i32 },
    /// **The conquest interstitial, screen `0x1C`** — the one screen over a
    /// running game that is not campaign music. `Screen_DrawConquest`
    /// (`0x0041E1DD`) opens with
    /// `Music_Play(g_campaignMap < 8 ? "setup.wav" : "setup2.wav", 0,
    /// g_campaignMap < 8)`, so `ended` — the campaign past its eighth map,
    /// [`crate::victory::Campaign::is_complete`] — picks both the file **and**
    /// the loop flag.
    Conquest { ended: bool },
    /// A battle, of a kind that picks the pair.
    Battle(BattleKind),
    /// **A film is up.** Every one of `Smk_Play`'s
    /// callers stops the music first — `Music_Stop(0)` in `CastleBuild_Confirm`,
    /// in both animated `Msg_DrawWindow` branches, in `Battle_CheckOutcome`, in
    /// `FUN_00432B05`; and `App_WinMain` stops the `setup.wav` it has just
    /// started before the intro opens. The film plays its own track instead.
    ///
    /// `over_battle` is whether the battlefield is under it, because
    /// `Smk_OnFinished` restarts the campaign bed **only when `g_battlePhase`
    /// is 0**: a battle that ended in a film stays silent until it is left.
    Film { over_battle: bool },
}

/// The audio layer. One per process, owned by the event loop.
pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    /// Dropping this closes the device, so it is held even though nothing
/// calls it. `None`
    stream: Option<cpal::Stream>,
    /// Lower-cased file name to the path the vfs resolved it to. Built once;
    /// a `BTreeMap` to keep the listing in a stable order.
    /// found is in a stable order when something has to be printed.
    files: BTreeMap<String, PathBuf>,
    /// The `.smk` files, the same way — a film's track is decoded out of the
    /// film when the film opens. See [`Audio::play_film`].
    films: BTreeMap<String, PathBuf>,
    /// Decoded one-shots. Only effects are cached — they are a few kilobytes
    /// each and there are a couple of dozen. Music is megabytes and is held
    /// only while it plays.
    cache: BTreeMap<String, Option<Arc<Sound>>>,
    battle: BattleCycle,
    /// The last [`Scene`] [`Audio::follow`] acted on.
    /// stepped when the battle *starts*, not once a frame for as long
    /// as it lasts.
    scene: Option<Scene>,
    options: Options,
    /// **Whether a file is worth decoding at all**, which is not the same
    /// question as whether a device is open and used to be conflated with it.
    ///
    /// A machine whose sound card refused a stream should not read and decode
    /// 3.8 MB every time the ladder changes track — that is what the guard was
    /// for. But [`Audio::headless`] has no device *and does* want the decode,
    /// because mixing into a buffer nobody hears is the only way to assert on
/// what a player would hear without a sound card. The two are separate
    /// fields and the guard says what it means.
    decodes: bool,
    /// **Every file this layer has opened.** Not a cache and not a
    /// policy — a record of what travelled the road, which is the only
    /// trustworthy answer to *"how much of the game's audio can we play?"*.
    ///
    /// The alternative is to type the number into a document.
    /// time it was typed it was wrong: `battle5.wav` is in the table, ships,
    /// and is **unreachable**, because the counter that selects it is
    /// `DAT_0057A0F0` — the third battle mode [`BattleKind`] deliberately does
    /// not name. A set collected by driving beats a set assembled by reading.
    heard: std::collections::BTreeSet<String>,
    /// **What the one-shot buffer holds** — `DAT_00522AEC`, the single
    /// DirectSound buffer `Sound_PlayFile` (`0x00427990`) builds every file
    /// into. The original has one, so "is the one-shot busy" is a question about
    /// whatever was put there last; ours are many voices, so the name is kept
    /// and the mixer is asked about it. `Sound_OneShotBusy` (`0x00427C9B`) and
    /// `Sound_PlayFile`'s drop ask the same buffer, so [`Audio::play_file`] is
    /// the only thing that sets it — [`Audio::stop_and_play_file`] through it —
    /// [`Audio::stop_one_shot`] is the only thing that clears it, and
    /// [`Audio::one_shot_busy`] is the only thing that reads it.
    one_shot: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything below runs on the silent layer.
    /// with no sound card is in. If any of it panics or blocks there, that
    /// machine cannot run the game.
    #[test]
    fn a_machine_with_no_device_survives_everything() {
        let mut a = Audio::silent();
        assert!(!a.is_live());
        assert_eq!(a.file_count(), 0);
        a.follow(Scene::FrontEnd);
        a.follow(Scene::Campaign { county_count: 1, share_of_map_pct: 7 });
        a.follow(Scene::Battle(BattleKind::Siege));
        a.play_music(Music::Scroll(3));
        a.play_effect(names::fanfare::MESSAGE);
        a.play_effect_if_idle("army.wav");
        assert!(!a.play_file(names::fanfare::BATTLE, false), "nothing plays with no device");
        assert!(!a.stop_and_play_file("kt170_1.wav", true));
        assert!(!a.one_shot_busy());
        a.stop_one_shot();
        a.stop_music();
        a.set_options(Options { music: false, effects: false, speech: false });
        assert_eq!(a.music_name(), None, "nothing plays with no device");
    }

    #[test]
    fn a_missing_file_is_looked_for_once() {
        let mut a = Audio::silent();
        a.play_effect("nothing_like_this.wav");
        // With no device `load` returns before it ever touches the cache, which
        // is the cheaper of the two right answers.
        assert!(a.cache.is_empty());
    }

    #[test]
    fn the_front_end_is_silent_and_the_campaign_is_not() {
        // The decision, not the sound: `follow` is the whole music policy and
// it is checked here without a device.
        let mut a = Audio::silent();
        a.follow(Scene::FrontEnd);
        assert_eq!(a.scene, Some(Scene::FrontEnd));
        a.follow(Scene::Campaign { county_count: 4, share_of_map_pct: 30 });
        assert_eq!(a.scene, Some(Scene::Campaign { county_count: 4, share_of_map_pct: 30 }));
    }

    #[test]
    fn staying_in_a_battle_does_not_step_the_counter() {
        // The bug this prevents is audible: `follow` runs sixty times a second
        // and `BattleCycle::next` flips the track every call.
        let mut a = Audio::silent();
        a.follow(Scene::Battle(BattleKind::Field));
        let after_one = a.battle;
        for _ in 0..100 {
            a.follow(Scene::Battle(BattleKind::Field));
        }
        assert_eq!(a.battle, after_one, "the counter moved during one battle");
        // Leaving and coming back is a new battle and does step it.
        a.follow(Scene::Campaign { county_count: 3, share_of_map_pct: 20 });
        a.follow(Scene::Battle(BattleKind::Field));
        assert_ne!(a.battle, after_one);
    }

    /// **"Will you take the field?" — spoken, and which take is the answer to
    /// what `docs/audio.json` filed as unread.**
    ///
    /// The nine `S080` sites are three call sites carrying one ladder on
    /// `g_battleChoiceOwner`, and all three run on the return from
    /// `Battle_ChooseSettlement` that raises screen `0x12`. So the take is the
    /// same field the prompt already draws its sentence from.
    /// is *not* the group-80 one: 1 -> `_03`, 2 -> `_01`, 0 -> `_02`.
    ///
    /// **The effects switch is off**, and that is a necessity:
    /// fanfare `Battle_ChooseSettlement` plays first goes into the same
    /// one-shot buffer, and bare `Sound_PlayFile` drops what will not fit. The
    /// last block asserts exactly that — with the trumpet on, the line the
    /// ladder picked is never opened at all.
    ///
    /// Ablations, each observed red: rotate [`names::speech::BATTLE_PROMPT`]
    /// by one and all three rows fail on the take; drop the `play_file` and
    /// they fail with an empty `heard`; move the call above the fanfare and
    /// the last block fails.
    #[test]
    fn the_battle_prompt_speaks_the_take_its_choice_owner_picks() {
        let Some(dir) = l2_testkit::install_dir() else { l2_testkit::skip!("no game install") };
        let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");

        for (owner, want, others) in [
            (1u8, "s080_03.wav", ["s080_01.wav", "s080_02.wav"]),
            (2, "s080_01.wav", ["s080_02.wav", "s080_03.wav"]),
            (0, "s080_02.wav", ["s080_01.wav", "s080_03.wav"]),
        ] {
            let mut audio = Audio::headless(&platform.vfs);
            // The trumpet is an effect and the line is speech.
            // the buffer empty for the line the ladder picks.
            let mut game = prompt_world(owner);
            game.prefs.effects = false;
            let mut machine = crate::screen::Machine::new(crate::screen::ScreenId::Campaign);
            let mut director = Director::new();
            director.listen(&mut audio, &machine, &game);

            machine.push(crate::screen::ScreenId::BattlePrompt);
            director.listen(&mut audio, &machine, &game);
            assert!(
                audio.heard().contains(&want),
                "choice_owner {owner} should speak {want} - heard {:?}",
                audio.heard()
            );
            for other in others {
                assert!(!audio.heard().contains(&other), "choice_owner {owner} also said {other}");
            }
        }

        // And with the effects switch on, the fanfare takes the buffer first
        // and the line is dropped — the original's own behaviour at these nine
        // sites, and they are worth naming.
        let mut audio = Audio::headless(&platform.vfs);
        let game = prompt_world(1);
        let mut machine = crate::screen::Machine::new(crate::screen::ScreenId::Campaign);
        let mut director = Director::new();
        director.listen(&mut audio, &machine, &game);
        machine.push(crate::screen::ScreenId::BattlePrompt);
        director.listen(&mut audio, &machine, &game);
        assert!(audio.heard().contains(&"ff_batl.wav"), "heard {:?}", audio.heard());
        assert!(
            !audio.heard().contains(&"s080_03.wav"),
            "the trumpet is still sounding, so Sound_PlayFile drops the line"
        );
    }

    /// A game suspended on the battle prompt, with the choice in one hand.
    fn prompt_world(choice_owner: u8) -> crate::Game {
        let mut game = crate::Game::new(5);
        game.kingdom.set_county_count(3);
        game.player = 1;
        crate::turn::suspend_on(
            &mut game,
            crate::turn::Question {
                attacker: 0,
                defender: 1,
                county: 1,
                is_siege: false,
                attacker_owner: 1,
                defender_owner: 2,
                attacker_men: 100,
                defender_men: 80,
                attacker_roster: [0; l2_kingdom::unit::TROOP_TYPES],
                defender_roster: [0; l2_kingdom::unit::TROOP_TYPES],
                choice_owner,
                castle_level: None,
            },
        );
        game
    }

    #[test]
    fn turning_music_off_forgets_what_was_playing() {
        let mut a = Audio::silent();
        a.follow(Scene::Campaign { county_count: 4, share_of_map_pct: 30 });
        a.set_options(Options { music: false, ..Options::default() });
        assert_eq!(a.scene, None, "so that turning it back on re-derives");
        a.follow(Scene::Campaign { county_count: 4, share_of_map_pct: 30 });
        assert_eq!(a.scene, None, "and follow does nothing while music is off");
    }
}

