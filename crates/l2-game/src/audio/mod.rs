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
//! shape rather than by discipline: [`Audio`] is owned by the event loop in
//! `main.rs` and is *not* in [`crate::screen::Ctx`], so a screen cannot reach
//! it, cannot ask whether a sound finished, and cannot branch on one. The
//! information flows one way — the event loop looks at the world and decides
//! what should be audible. Nothing looks back.
//!
//! That constraint is also what fixes the design. Music is not pushed by
//! events; it is **derived**, once a tick, from what is on screen and how the
//! player's realm is doing, by [`Audio::follow`]. Asking for a track that is
//! already playing does nothing, so the derivation can run every frame and the
//! music never restarts.
//!
//! # It is optional, and it is optional at run time
//!
//! A machine with no audio device, no default output configuration, or an
//! install with the `.wav` files stripped must behave exactly as it did
//! before. So [`Audio::open`] cannot fail: every step that could — finding a
//! host, finding a device, building a stream, resolving a file, decoding it —
//! degrades to silence, and [`Audio::silent`] is the same object with the
//! device left out. `--no-sound` selects it explicitly.
//!
//! There is no cargo feature. A feature would mean the code that CI compiles
//! is not the code that ships, and the interesting failures here are all at
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
//! files are somebody speaking — which is why a player calls the voice acting
//! *"half the personality of the game"*. It went from 0 to 543 in one change,
//! and that is not because the change was large: it is because the whole class
//! hangs off one trigger, and the trigger is a countdown rather than an event.
//!
//! **A previous count here said 12 and was wrong at 11**, by reading the battle
//! table instead of driving it — `battle5.wav` ships, decodes, and cannot be
//! asked for, because the counter that selects it is `DAT_0057A0F0`, the third
//! battle mode [`track::BattleKind`] declines to guess at.
//!
//! **The count of triggers is `docs/audio.json`, and it is checked rather than
//! written.** `crates/l2-game/tests/sfx.rs` requires the set of sites that file
//! calls `reproduced` to equal the set of `// sfx:` markers in `crates/`, and
//! `node tools/oracle/sounds.js --check` requires the file's *rows* to equal
//! what the decompilation holds. So the sentence below cannot go stale without
//! something going red, which is the whole reason it is safe to write a number
//! here at all — the previous version of this table was prose, was hand-marked,
//! and was wrong in both directions.
//!
//! **75 of 143, and 3 of the 143 are dead in the shipped game.** The
//! denominator moved because the enumeration was one primitive short: see
//! [`track::Music::Setup`].
//!
//! | class | the original's call site | sites | ours |
//! |---|---|---:|---:|
//! | **Message narration** | `Msg_PlayVoice` `0x004B35C1` | 16 | **13** — [`voice_tick`]; two need video and one needs the tip screens |
//! | **Music** | `Music_StartCampaign`, `Music_StartBattle`, `Music_Play` | 23 | **11** — [`scene`]; the twelve left restart a bed a film stopped |
//! | **By name** | `Sound_PlayFile` | 49 | **16** — [`names::speech`], the fanfares and `Wall_Smash` |
//! | **The two sample banks** | `Sound_PlaySlot`, `Sound_RestartSlot`, `FUN_004262CF` | 49 | **27** — the march, the sites, the village's work, the click, and fifteen on the battlefield |
//! | **Troop cries** | `Sound_PlayTroopCry` `0x00499CB1` | 6 | **6** — [`TroopCries`] |
//!
//! What is left, in the order a player notices it:
//!
//! * **Nine battlefield sites, each on a mechanic rather than a channel.** This
//!   bullet used to say the whole battlefield — 25 sites — was *"a limit of the
//!   design rather than a to-do"*, because a sword swing is an event inside a
//!   tick and [`Director`] derives sound from the world after it. The premise
//!   was right and the conclusion was not: the world simply did not *record*
//!   the event. `l2_sim::cue` is that record — monotone counts the battle writes
//!   and never reads — and because every battlefield call is drop-if-busy, a
//!   count that moved since the last tick is exactly what the original's calls
//!   could make audible. Sixteen of the 25 sound now. The nine that do not are
//!   fire, boiling oil, a tower docking, state 17's own loose, the high-rampart
//!   catapult miss and a realm eliminated mid-battle — each named in
//!   `docs/audio.json`, and each a mechanic `l2-sim` does not have.
//! * **The two sample banks elsewhere.** [`names::KINGDOM_BANK`] and
//!   [`names::BATTLE_BANK`] are recovered and tested against the install; 27 of
//!   their 29 slots ship. The campaign half of the kingdom bank now sounds —
//!   the march, the resource sites, the village's work — and `dest_ind.wav`,
//!   the field brush and the peasant mob's other arms do not.
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
//!   [`Audio::is_playing`] now answers the gate; what is missing is a screen —
//!   `Tip_Update`, `Tip_Show`, the paragraph window and its computed OK corner.
//!   `S010_13.wav` is `g_msgVoiceS010`'s, which `FUN_004B36C0` reads. The
//!   letter's sting, `FUN_004B3B92(lord - 1)`, is `S246_02.wav + lord * 0x10`
//!   over four clips and a sentinel.
//! * **`ff_lose.wav`** needs a gate rather than a call site — see
//!   [`names::fanfare::AFTER_BATTLE`]. It is the only fanfare left.

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
/// **Three flags, no volumes.** There is no per-channel level in the original:
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
    /// and the conclusion does not follow: `setup.wav` **is** the music, played
    /// by `Music_Play(name, 0, 1)` — the same looping call the campaign picker
    /// ends in, reached from a function the eight-primitive audit did not
    /// enumerate. See [`track::Music::Setup`].
    FrontEnd,
    /// The campaign map, a county panel, the village — anything in the
    /// management surface. Carries what the ladder reads.
    Campaign { county_count: u8, share_of_map_pct: i32 },
    /// A battle, of a kind that picks the pair.
    Battle(BattleKind),
}

/// The audio layer. One per process, owned by the event loop.
pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    /// Dropping this closes the device, so it is held even though nothing
    /// calls it. `None` when there is no device, which is a supported state.
    stream: Option<cpal::Stream>,
    /// Lower-cased file name to the path the vfs resolved it to. Built once;
    /// a `BTreeMap` rather than a `HashMap` so that a listing of what was
    /// found is in a stable order when something has to be printed.
    files: BTreeMap<String, PathBuf>,
    /// Decoded one-shots. Only effects are cached — they are a few kilobytes
    /// each and there are a couple of dozen. Music is megabytes and is held
    /// only while it plays.
    cache: BTreeMap<String, Option<Arc<Sound>>>,
    battle: BattleCycle,
    /// The last [`Scene`] [`Audio::follow`] acted on, so a battle's track is
    /// stepped when the battle *starts* rather than once a frame for as long
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
    /// what a player would hear without a sound card. So the two are separate
    /// fields and the guard says what it means.
    decodes: bool,
    /// **Every file this layer has actually opened.** Not a cache and not a
    /// policy — a record of what travelled the road, which is the only
    /// trustworthy answer to *"how much of the game's audio can we play?"*.
    ///
    /// The alternative is to type the number into a document, and the first
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
    /// `Sound_PlayFile`'s drop ask the same buffer, so [`Audio::play_speech`] and
    /// [`Audio::play_file`] both set it, and [`Audio::one_shot_busy`] and
    /// [`Audio::play_file`] both read it.
    one_shot: Option<String>,
}

impl Audio {
    /// An audio layer with no device and no files. Every method is a no-op and
    /// none of them fail. This is what `--no-sound` builds, and what a machine
    /// with no sound card ends up with anyway.
    pub fn silent() -> Audio {
        Audio {
            mixer: Arc::new(Mutex::new(Mixer::new(44_100))),
            stream: None,
            files: BTreeMap::new(),
            cache: BTreeMap::new(),
            battle: BattleCycle::default(),
            scene: None,
            options: Options::default(),
            decodes: false,
            heard: std::collections::BTreeSet::new(),
            one_shot: None,
        }
    }

    /// Open the default output device and index the `.wav` files the vfs can
    /// see. **Never fails**; on any problem it reports once on stderr and
    /// returns something silent.
    pub fn open(vfs: &l2_mods::vfs::Vfs) -> Audio {
        let mut audio = Audio::index(vfs);
        match start_device(Arc::clone(&audio.mixer)) {
            Ok(stream) => {
                audio.stream = Some(stream);
                audio.decodes = true;
            }
            Err(e) => eprintln!("sound: no output device ({e}); running silent"),
        }
        if audio.files.is_empty() {
            eprintln!("sound: no .wav files in the install; running silent");
        }
        audio
    }

    /// **Everything but the sound card**: the install's files are indexed and
    /// decoded and the mixer runs, but no device is opened and nothing is
    /// audible. [`Audio::mix`] is where the result comes out.
    ///
    /// This exists so that a test can assert on *what a player would hear* —
    /// samples, from a real file, chosen by the real policy — on a machine
    /// somebody is sitting at. `docs/agents.md`: prefer the mechanism that
    /// needs nothing of the world. Reaching for a device instead would make the
    /// assertion depend on a sound card, on CI having one, and on nobody
    /// minding the noise, and it would still not be able to check the samples.
    pub fn headless(vfs: &l2_mods::vfs::Vfs) -> Audio {
        let mut audio = Audio::index(vfs);
        audio.decodes = true;
        audio
    }

    /// The files the vfs can see, with no device.
    fn index(vfs: &l2_mods::vfs::Vfs) -> Audio {
        let mut audio = Audio::silent();
        for name in vfs.entries_with_extension("wav") {
            if let Some(path) = vfs.resolve(name) {
                // The vfs already normalises, but the executable's tables and
                // the install's directory disagree about case, so the key is
                // pinned lower-case here as well rather than trusted.
                audio.files.insert(name.to_ascii_lowercase(), path.to_path_buf());
            }
        }
        audio
    }

    /// **The buffer the device callback would have been handed** — interleaved
    /// stereo `f32` at the mixer's rate, one call per callback's worth.
    ///
    /// The stream `start_device` builds does exactly this and then spreads it
    /// over the device's channel count, so this is the whole of what is
    /// audible and the only thing worth asserting on.
    pub fn mix(&mut self, out: &mut [f32]) {
        match self.mixer.lock() {
            Ok(mut m) => m.fill(out),
            Err(_) => out.iter_mut().for_each(|s| *s = 0.0),
        }
    }

    /// Whether a device is open. For the start-up line and for tests; nothing
    /// branches on it, because every method works either way.
    pub fn is_live(&self) -> bool {
        self.stream.is_some()
    }

    /// How many `.wav` files were found.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// **Every file this layer has actually opened, sorted.**
    ///
    /// The answer to *"how much of the game's 771 sounds does the engine
    /// reach?"*, measured rather than asserted. See the field.
    pub fn heard(&self) -> Vec<&str> {
        self.heard.iter().map(String::as_str).collect()
    }

    pub fn options(&self) -> Options {
        self.options
    }

    /// **Is this one-shot still sounding?** — the `GetStatus` bit test
    /// `Sound_OneShotBusy` (`0x00427C9B`) makes, asked by name because ours are
    /// many buffers rather than one.
    ///
    /// Audio-side only. [`Director`] may branch on it, because what it decides
    /// is also only audio; a screen cannot reach [`Audio`] and so cannot.
    pub fn is_playing(&self, name: &str) -> bool {
        self.mixer.lock().is_ok_and(|m| m.is_playing(&name.to_ascii_lowercase()))
    }

    /// **`Sound_OneShotBusy` (`0x00427C9B`)** — is the narrator still talking?
    ///
    /// The original has one one-shot buffer and every `Sound_PlayFile` goes into
    /// it, so the question has one answer. Here it is asked of the last clip
    /// [`Audio::play_speech`] or [`Audio::play_file`] put there, which is that
    /// buffer's occupant for every voice line and every troop cry. `[D]`: `Msg_DrawWindow`'s four `ff_msg.wav` fanfares also go
    /// through `Sound_PlayFile` and are played as effects here, so a fanfare
    /// does not count as busy — none of the categories that ask play one.
    pub fn one_shot_busy(&self) -> bool {
        self.one_shot.as_deref().is_some_and(|n| self.is_playing(n))
    }

    /// Apply the three switches. Turning music off stops it; turning it back on
    /// leaves [`Audio::follow`] to start the right track on the next tick,
    /// which is how `Opt_ToggleMusic` behaves.
    pub fn set_options(&mut self, options: Options) {
        self.options = options;
        if let Ok(mut m) = self.mixer.lock() {
            m.music_on = options.music;
            m.effects_on = options.effects;
            if !options.music {
                m.stop_music();
            }
        }
        if !options.music {
            // Forget what was playing so that switching back on re-derives.
            self.scene = None;
        }
    }

    /// **The music policy.** Call once a tick with what the world looks like;
    /// it starts, changes or stops the track, and does nothing at all when the
    /// answer has not changed.
    ///
    /// The campaign ladder is re-evaluated every call, so taking a county that
    /// crosses a threshold changes the bed at the moment it happens — which is
    /// what the original does too, since `FUN_00499ACA` is called on every
    /// return to the campaign map.
    pub fn follow(&mut self, scene: Scene) {
        if !self.options.music {
            return;
        }
        let want = match scene {
            // `App_WinMain` (`0x0040E9AB`) plays it as the process opens and
            // `FUN_00497A34` plays it again, looped, every time the front end
            // comes back. One bed, two call sites, and this is both.
            // sfx: App_WinMain#1,FUN_00497a34#2
            Scene::FrontEnd => Some(Music::Setup),
            Scene::Campaign { county_count, share_of_map_pct } => {
                Some(track::campaign(county_count, share_of_map_pct))
            }
            Scene::Battle(kind) => {
                // Step the counter only on the way *into* a battle. Calling
                // `next` once a frame would flip tracks sixty times a second.
                match self.scene {
                    Some(Scene::Battle(k)) if k == kind => {
                        self.music_name().and_then(current_battle_track)
                    }
                    _ => Some(self.battle.next(kind)),
                }
            }
        };
        self.scene = Some(scene);
        match want {
            Some(m) => self.play_music(m),
            None => self.stop_music(),
        }
    }

    /// What track is playing, by file name.
    pub fn music_name(&self) -> Option<String> {
        self.mixer.lock().ok()?.music_name().map(str::to_owned)
    }

    /// Start a track, unless it is already the one playing.
    pub fn play_music(&mut self, music: Music) {
        let name = music.file();
        if self.music_name().as_deref() == Some(name) {
            return;
        }
        // Decoded outside the lock: this is megabytes of work and the device
        // callback is on the other end of that mutex.
        let Some(sound) = self.load(name) else {
            // A missing or unreadable track stops the music rather than
            // leaving the previous one running under the wrong screen.
            self.stop_music();
            return;
        };
        if let Ok(mut m) = self.mixer.lock() {
            m.set_music(name.to_string(), sound);
        }
    }

    pub fn stop_music(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.stop_music();
        }
    }

    /// Fire a one-shot by file name, restarting it if it is already sounding —
    /// `Sound_RestartSlot` (`0x00426216`), the verb a click uses.
    ///
    /// Silently does nothing when the file is missing, when effects are off,
    /// or when there is no device. A sound that cannot be found is not an
    /// error: the install is the publisher's and we do not get to require it
    /// be complete.
    pub fn play_effect(&mut self, name: &str) {
        if !self.options.effects {
            return;
        }
        let Some(sound) = self.load(name) else { return };
        if let Ok(mut m) = self.mixer.lock() {
            m.play_effect(name.to_ascii_lowercase(), sound);
        }
    }

    /// Fire a one-shot **unless it is already playing** — `Sound_PlaySlot`
    /// (`0x00426120`), and the verb almost everything in the original uses.
    ///
    /// This is the one to reach for from anything that fires per frame or per
    /// step: a marching army, a cart, a mob, a fire. See
    /// [`mixer::Mixer::play_effect_if_idle`] for why the original works that
    /// way and what it buys.
    pub fn play_effect_if_idle(&mut self, name: &str) {
        if !self.options.effects {
            return;
        }
        // Ask before decoding: the common case is "already playing", and on
        // that path this should cost a string comparison and nothing else.
        let key = name.to_ascii_lowercase();
        if let Ok(m) = self.mixer.lock() {
            if m.is_playing(&key) {
                return;
            }
        }
        let Some(sound) = self.load(name) else { return };
        if let Ok(mut m) = self.mixer.lock() {
            m.play_effect_if_idle(key, sound);
        }
    }

    /// Fire a speech clip — `FUN_00427990(name, 1, 0)`. Gated by the *speech*
    /// switch rather than the effects one, as the original gates it.
    pub fn play_speech(&mut self, name: &str) {
        if !self.options.speech {
            return;
        }
        let Some(sound) = self.load(name) else { return };
        if let Ok(mut m) = self.mixer.lock() {
            m.play_effect(name.to_ascii_lowercase(), sound);
        }
        // It is `Sound_PlayFile` too, so it occupies the one buffer, and a troop
        // cry asked for over the narrator is dropped — see [`Audio::play_file`].
        self.one_shot = Some(name.to_ascii_lowercase());
    }

    /// **`Sound_PlayFile` (`0x00427990`), drop included** — the verb a troop
    /// cry and `Wall_Smash` use.
    ///
    /// ```c
    /// if (Sound_OneShotBusy()) return 0;         /* first, before either flag */
    /// Sound_StopOneShot();
    /// if (isSpeech == 1 && g_optSpeech == 0) return 0;
    /// if (isSpeech == 0 && g_optSoundEffects == 0) return 0;
    /// /* load the file into the one buffer and Play it */
    /// ```
    ///
    /// `[V]`. There is **one** one-shot buffer, so a file asked for while any
    /// other is still sounding is not played at all: a cry over a cry, a cry
    /// over the narrator, a cry over a wall coming down. That is the whole of
    /// the original's limit on how often the men answer, and it is why a player
    /// clicking ten orders a second hears one voice rather than ten. Answers
    /// whether it started.
    ///
    /// **Two older verbs here do not honour it, and are recorded rather than
    /// changed by the change that found it**: [`Audio::play_speech`] never drops
    /// — though it does occupy the buffer, so a cry is dropped over it — and the
    /// fanfares go through [`Audio::play_effect`], which neither drops nor
    /// occupies. Both are `Sound_PlayFile` in the original.
    pub fn play_file(&mut self, name: &str, speech: bool) -> bool {
        if let Some(last) = self.one_shot.as_deref() {
            if self.is_playing(last) {
                return false;
            }
        }
        let on = if speech { self.options.speech } else { self.options.effects };
        if !on {
            return false;
        }
        let Some(sound) = self.load(name) else { return false };
        let key = name.to_ascii_lowercase();
        if let Ok(mut m) = self.mixer.lock() {
            m.play_effect(key.clone(), sound);
        }
        self.one_shot = Some(key);
        true
    }

    /// Decode a file, caching the small ones.
    ///
    /// The cache stores the *failure* too, as `None`, so that a missing file is
    /// looked for once rather than on every click.
    fn load(&mut self, name: &str) -> Option<Arc<Sound>> {
        if !self.decodes {
            return None;
        }
        let key = name.to_ascii_lowercase();
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let sound = self.decode(&key).map(Arc::new);
        if sound.is_some() {
            self.heard.insert(key.clone());
        }
        // Music is not cached: five tracks at ~7 MB decoded each is 35 MB held
        // for the sake of a track change that happens twice an hour.
        let is_music = names::MUSIC_SCROLL.contains(&key.as_str())
            || names::MUSIC_BATTLE.contains(&key.as_str())
            || key == names::MUSIC_SETUP;
        if !is_music {
            self.cache.insert(key, sound.clone());
        }
        sound
    }

    fn decode(&self, key: &str) -> Option<Sound> {
        let path = self.files.get(key)?;
        // `PUMKIN.WAV` is 160 MB and is never played by the original either;
        // `wav::MAX_BYTES` refuses it, but refusing it before the read is what
        // keeps the 160 MB out of memory in the first place.
        match std::fs::metadata(path) {
            Ok(md) if md.len() as usize > wav::MAX_BYTES => {
                eprintln!("sound: {key} is {} bytes; not playing it", md.len());
                return None;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("sound: {key}: {e}");
                return None;
            }
        }
        let bytes = std::fs::read(path).ok()?;
        match wav::decode(&bytes) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("sound: {key}: {e}");
                None
            }
        }
    }
}

/// **What the world sounds like, from what is on screen and who is winning.**
///
/// The one place the screen stack is turned into a music decision, so that the
/// event loop can be three lines and this can be tested.
///
/// The original divides the same way and by the same quantity: `g_battlePhase`
/// 0 is *the whole management surface* — map, counties, village, save, the
/// popups over them — and it all runs `FUN_00499ACA`. There is no separate
/// county track. `[V]`, and the player confirmed it by ear: *"the scroll wavs
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
/// So the stack is asked as a whole, and the arms are the original's three
/// music phases rather than a screen id:
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
/// # The share is recomputed here rather than read
///
/// The original reads `Realm +0x60`, a byte `FUN_0049D1E0` rebuilds once a
/// season. We compute `PctOf(county_count, kingdom.county_count)` — the same
/// arithmetic, from the same two numbers — for two reasons, and the second is
/// the one that matters:
///
/// * `Realm::share_of_map_pct` is **0 on a freshly imported save** and stays 0
///   until a turn has been ended, because `l2-scenario` reads what the `.sav`
///   holds and the field is derived rather than stored. A player who loads a
///   game at forty per cent of the map would get `Scroll1`.
/// * and a music bed that depends on *whether some other subsystem has run
///   yet* is a bug waiting for the order to change. This function should be a
///   function of the world, not of the schedule.
///
/// The divergence is that the original's music can be up to a season stale
/// after a mid-turn conquest and ours cannot. It is a music bed; nothing reads
/// it back.
pub fn scene(machine: &crate::screen::Machine, game: &crate::Game) -> Scene {
    use crate::screen::ScreenId;
    let ids = machine.ids();
    // `g_battlePhase == 2`. The battlefield is three screen ids in the original
    // (`0x29` field, `0x2A` drag, `0x2B` outcome) and one of ours, and the
    // phase outlives all of them: panels open over the field while the battle
    // music keeps playing, so this asks the stack rather than its top.
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

/// **When the narrator speaks, by message category** — `Msg_DrawWindow`'s
/// (`0x0047309E`) voice schedule, and the answer is a *tick of the message
/// timer* rather than an event.
///
/// # The draw is the behaviour, so the trigger is a countdown
///
/// `Msg_DrawWindow` is 10,915 bytes and it is not a painter: it dismisses,
/// enqueues, sets its own timer and plays its own sound, all from inside the
/// draw. So there is no call site to put a voice beside. Every one of its
/// sixteen `Msg_PlayVoice` calls is guarded by `g_messageTimer == <constant>`,
/// and `g_messageTimer` counts **down** from [`crate::message::TIMER_START`]
/// (2000), one per tick. `[V]`.
///
/// | category | ticks after opening | constant |
/// |---|---:|---|
/// | `0x02`, `0x03`, `0x05`…`0x09`, `0x0F`, `0x10`, `0x11`, `0x12` | 10 | `0x7C6` |
/// | `0x00` notice, `0x0D` capture (unanimated) | 90 | `0x776` |
/// | `0x0E` ending (unanimated) | 100 | `0x76C` |
/// | `0x01` letter, `0x0A` pay prompt, `0x0B` alliance prompt | 200 | `0x708` |
/// | `0x04` tip | 10 | `0x5A`, against a timer clamped to 100 |
/// | `0x13` help | — | silent |
///
/// **The five constants are one rule and a delay.** `0x7C6` is 1990 against a
/// start of 2000 and `0x5A` is 90 against the tip's clamped 100: *both are ten
/// ticks after the window opened*, which is why the tip needed a constant of
/// its own rather than a different rule. The three larger delays are the
/// categories that play a **fanfare** on the opening frame — the voice waits
/// for the trumpet instead of talking over it, and the longest wait, 200 ticks,
/// is the one that also plays a lord's sting at 90.
///
/// `docs/audio-triggers.md` has the enumeration this came out of. The five
/// values were already on file — `crates/l2-game/src/screens/message.rs` lists
/// them and says they are *"recorded in `crate::message`"*, where they had
/// never been written — but **which category takes which** was not, and that is
/// the half a caller needs.
///
/// # Categories `0x0C` and `0x14` are absent on purpose
///
/// `Msg_DrawWindow` delegates them to `Msg_DrawDiplomacy` (`0x00475E07`) and
/// `Msg_DrawBeyondLetter` (`0x00476488`), which carry a voice call of their own
/// on their own schedule. Reading those two is a separate job and inventing a
/// tick for them would be worse than the silence. `docs/audio-triggers.md`
/// records them as unread rather than as absent.
pub fn voice_tick(category: u8) -> Option<i32> {
    use crate::message::category as c;
    Some(match category {
        c::NOTICE | c::CAPTURE => 0x776,
        c::LETTER | c::PAY_PROMPT | c::ALLIANCE_PROMPT => 0x708,
        c::ENDING => 0x76C,
        // The tip's timer is clamped to `TIP_TIMER` on the frame it opens, so
        // ten ticks in is 90 rather than 1990. Same rule, different start.
        c::TIP => 0x5A,
        c::COUNTY_PORTRAIT
        | c::COUNTY_NOTICE
        | c::EVENT
        | c::COUNTY_TALL
        | c::GARRISON_PROMPT
        | c::CASTLE => 0x7C6,
        n if (c::PARAGRAPHS_FIRST..=c::PARAGRAPHS_LAST).contains(&n) => 0x7C6,
        // `HELP`, and the two that delegate to a painter of their own.
        _ => return None,
    })
}

/// **The fanfare a message window opens with**, on the frame the timer is still
/// [`crate::message::TIMER_START`] — `Msg_DrawWindow`'s five `Sound_PlayFile`
/// calls, which is all of them. `[V]`.
///
/// `ff_capt.wav` is the conquest band and the guard is on the *group*, not the
/// category: `0x71 < group && group < 0x7F`, which is `L2.eng` 114…126. That is
/// the one place in the audio layer where a group decides a sound, and it is
/// why [`names::fanfare::CAPTURED`] had no caller until now.
fn open_fanfare(category: u8, group: u16) -> Option<&'static str> {
    use crate::message::category as c;
    match category {
        c::NOTICE if (0x72..=0x7E).contains(&group) => Some(names::fanfare::CAPTURED),
        c::LETTER | c::PAY_PROMPT | c::ALLIANCE_PROMPT | c::CAPTURE => {
            Some(names::fanfare::MESSAGE)
        }
        _ => None,
    }
}

/// **Everything audible, decided from the world after the tick that made it.**
///
/// One direction only: [`Director::listen`] reads the game and the screen stack
/// and tells the audio layer what should be true. It never writes to either,
/// and nothing it does is visible to the next tick — so the recording of a
/// session and a replay of it are the same simulation whether or not the
/// machine had a sound card. That is `docs/netcode.md` D-3, and it is why this
/// takes `&Machine` and `&Game` rather than `&mut`.
///
/// # Why this is a type in the library and not six lines in `main.rs`
///
/// It *was* six lines in `main.rs`, and the consequence is the reason this file
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
    /// outside a battle, and the reason is structural rather than convenient:
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
    /// title screen, and it is asserted in `tests/audio_wiring.rs` rather than
    /// left to be noticed.
    stack: Vec<crate::screen::ScreenId>,
    /// `g_mapZoom == 2` at the previous tick — `Map_ZoomOut`'s edge. `None`
    /// until the first tick, so starting zoomed out is not an event.
    zoom_far: Option<bool>,
    /// **Where every unit stood at the last tick**, so that a unit *entering a
    /// tile* can be noticed without the simulation reporting it. See
    /// [`Director::hear_the_march`].
    ///
    /// Empty until the first tick, which is what makes the first tick silent:
    /// there is nothing to have moved from.
    tiles: Vec<Option<(l2_kingdom::UnitKind, (u8, u8))>>,
    /// [`crate::screen::Machine::clicks`] at the previous tick — the widget
    /// click's edge. See [`Director::hear_the_click`].
    clicks: u32,
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
        // representations of one setting, and the one the player could reach
        // was the one nobody read.
        //
        // Pushed rather than shared, and in this direction only: the audio
        // layer must not be reachable from a screen. Guarded on inequality
        // because the setter takes the mixer's lock and the answer changes
        // about once an hour.
        let want = Options {
            music: game.prefs.music,
            effects: game.prefs.effects,
            speech: game.prefs.speech,
        };
        if want != audio.options() {
            audio.set_options(want);
        }

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
        // sfx: Battle_ChooseSettlement#1
        if opened(&|id| matches!(id, ScreenId::BattlePrompt)) {
            audio.play_effect(names::fanfare::BATTLE);
        }

        // **The narrator's interface commentary**, five screens' worth. Every
        // one of these is `Sound_PlayFile(name, 1, 0)` — the *speech* flag — in
        // the function that sets `g_screenId`, so the screen arriving is the
        // trigger and not a stand-in for it. See [`names::speech`].
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
                    audio.play_speech(names::speech::RATION_NOT_MET);
                } else if c.herd != 0 && c.herd_eaten == 0 && c.grain_eaten == 0 {
                    audio.play_speech(names::speech::RATION_ON_DAIRY);
                }
            }
        }
        // **Setup page 4, *"Choose your title and your shield."*** Four
        // handlers reach it and every one of them plays `S011_02.wav` as it
        // sets `g_setupPage = 4`, so the page arriving is the trigger and the
        // four are one sound. `FUN_00432B05`'s is the fourth and is **not**
        // claimed: it is the arm that runs after `Net_JoinGame` succeeds, and
        // we have no network join to arrive by.
        // sfx: FUN_00432cc8#1,FUN_00432cc8#2,Setup_ChooseCampaign#1
        if opened(&|id| {
            matches!(id, ScreenId::Setup(crate::screens::setup::SetupPage::Shield))
        }) {
            audio.play_speech(names::speech::CHOOSE_YOUR_SHIELD);
        }
        // `Sidebar_Button` (`0x0043AE30`) hotspot 3. The ownership gate is
        // already ours: the sidebar refuses to open supplies on somebody
        // else's county, so reaching this screen *is* the guarded branch.
        // sfx: Sidebar_Button#1
        if opened(&|id| matches!(id, ScreenId::Supplies(_))) {
            audio.play_speech(names::speech::SUPPLIES);
        }
        // `Panel_SplitButton` (`0x004378B3`), and `FUN_004376BB` is the same
        // sound from the move-order confirm's split-into-a-castle path, which
        // we do not have. Both open `g_screenId` `0x11`.
        // sfx: Panel_SplitButton#1
        if opened(&|id| matches!(id, ScreenId::Divide(_))) {
            audio.play_speech(names::speech::SPLIT_ARMY);
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
                    audio.play_effect(names::blacksmith::FIRE);
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
        self.stack = now;

        self.hear_the_click(audio, machine);

        // **`Map_ZoomOut` (`0x00434FD5`)**, whose last statement is
        // `Sound_PlayFile("S033_02.wav", 1, 0)`. It is a zoom *level*, not a
        // screen, so it is the one edge here that is not on the stack. There is
        // no matching sound on the way back in: `Map_ZoomIn` is silent.
        // sfx: Map_ZoomOut#1
        if game.map_zoom_far && self.zoom_far == Some(false) {
            audio.play_speech(names::speech::ZOOM_OUT);
        }
        self.zoom_far = Some(game.map_zoom_far);

        self.hear_the_march(audio, game);

        self.hear_the_battle(audio, game);

        // **The message window, which is where nearly all of the game's audio
        // lives.** 646 of the install's 771 files are somebody speaking, and
        // every one of them is played from `Msg_DrawWindow` (`0x0047309E`) or a
        // sibling it delegates to.
        //
        // **There is no end-of-turn sound in the original**, and the chime a
        // player hears at the end of a turn is a message window opening.
        // Nothing on the `Turn_End` / `Season_Advance` / phase-7 path plays
        // anything, the End Turn button is silent, and both call sites of the
        // end-of-turn screen fade carry no sound either.
        //
        // Until the message window existed this fired once per *turn that
        // produced any message*, from `game.turns_played`, and said in a
        // comment that it was an approximation. It is gone: the window is real
        // and the trigger is the original's own, which is the message timer
        // reaching a value rather than anything happening.
        //
        // **Equality, not a threshold, and that is deliberate.** The timer
        // decrements by one per tick (`MessageQueue::tick`), so each value
        // occurs exactly once per window and a `==` fires exactly once with no
        // memo to keep and nothing to reset when a window is dismissed early.
        // It is also precisely what the original tests. If the timer ever steps
        // by more than one this goes quiet rather than firing twice, which is
        // the failure worth having of the two.
        if let Some(record) = game.messages.open() {
            let timer = game.messages.timer();
            // **All five of `Msg_DrawWindow`'s `Sound_PlayFile` calls** — the
            // one `ff_capt.wav` and the four `ff_msg.wav`, which are four
            // categories of one fanfare rather than four sounds.
            // sfx: Msg_DrawWindow#1,Msg_DrawWindow#6,Msg_DrawWindow#11,Msg_DrawWindow#13,Msg_DrawWindow#17
            if timer == crate::message::TIMER_START {
                if let Some(fanfare) = open_fanfare(record.category, record.group) {
                    audio.play_effect(fanfare);
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
                // the narrator does not read rather than a failure.
                if let Some(name) = names::message_voice(record.group, record.variant) {
                    audio.play_speech(&name);
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
                audio.play_speech(name);
            }
        }
    }

    /// **The pointer click** — `Sound_RestartSlot(1)`, `click3.wav`, from
    /// inside `Widget_Test` (`0x0040DA1E`).
    ///
    /// The trigger is not here: it is in [`crate::press::Press::press`] and
    /// [`crate::press::Press::press_delayed`], which are `Widget_Test`'s kind-4
    /// and kind-5 arms, and the `// sfx:` markers are on those lines. This is
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
    /// cost one voice per distinct sound rather than twelve.
    /// [`Audio::play_effect_if_idle`] is exactly that verb and there is
    /// deliberately no throttle of our own on top of it: if this ever needs
    /// one, the mixer is wrong rather than the call site.
    ///
    /// # Why the movement is *found* rather than reported
    ///
    /// Nothing in the simulation hands the audio layer an event, and that is
    /// the property the whole module rests on: [`Director::listen`] takes
    /// `&Game`, so a sound cannot change what the simulation does in either
    /// value or timing (`docs/netcode.md` D-3). Threading a "who stepped"
    /// report out of `l2_kingdom::units_tick` and along to here would have put
    /// it on the [`crate::Game`], which is to say in the save and in the
    /// lockstep digest, for a sound. Diffing 150 tiles a tick is cheaper than
    /// that in every sense that matters.
    ///
    /// **The one thing it cannot tell apart** is a step from a teleport: an
    /// army garrisoning a castle is put on the keep's tile by
    /// `Army_GarrisonApply` and sounds here as though it walked there. `[D]` —
    /// one extra hoofbeat, dropped if the sound is already playing, and the
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

    /// **The battlefield: the men's cries, and the fighting.**
    ///
    /// Two sources, one per kind of occasion, and neither is a report the
    /// simulation hands over:
    ///
    /// * **the cries** are [`crate::battlefield::LiveBattle::cries`], a list the
    ///   six cry arms append to as a player selects and orders — `Battle_DragSelect`,
    ///   `Battle_OrderSelection` and `Battle_FormationKey`. Each is played through
    ///   [`TroopCries`], which is `Sound_PlayTroopCry`'s body;
    /// * **the fighting** is [`l2_sim::Cues`], the per-man record the battle
    ///   keeps (`crate::cue` in `l2-sim` is why counters are exact rather than an
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

/// One call the original makes from inside the battlefield's per-man state
/// machine, in the form the audio layer can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// `Sound_PlaySlot(n)`, or its 28-byte thunk `FUN_004262CF(n)` — a
    /// **1-based** slot of [`names::BATTLE_BANK`], dropped if that buffer is
    /// still sounding. [`Audio::play_effect_if_idle`].
    Slot(usize),
    /// `Sound_PlayFile(name, 0, 0)` — the one-shot buffer, dropped if it is
    /// still sounding. [`Audio::play_file`].
    File(&'static str),
}

/// **The battlefield's sounding call sites, as a function of what happened.**
///
/// `was` and `now` are one battle's [`l2_sim::Cues`] at two ticks; the answer
/// is every call the original would have made in between, **once per kind**.
/// That "once" is not a throttle of ours — every call below is drop-if-busy on
/// its own buffer, so a second request for the same slot inside one tick is
/// dropped by the original too. `l2-sim`'s `crate::cue` has the argument.
///
/// Each arm is the original's branch, beside the id of the site it reproduces.
/// Which *event* each counter records is decided where it is written, in
/// `l2-sim`; which *slot* that event plays is decided here, because it is the
/// original's ladder and not a rule of the battle.
///
/// Nine of the twenty-five sites are not here, and each is `blocked` in
/// `docs/audio.json` on a mechanic this engine does not model: fire
/// (`BattleMan_BurnTick` ×2, `FUN_0048551D`), boiling oil (`FUN_0047A814`),
/// a siege tower docking (`FUN_00491492`), state 17's own loose
/// (`BattleMan_StateCloseToAttack` ×2), a catapult shot on a rampart four high
/// (`Missile_Step#2`), and a realm eliminated mid-battle (`FUN_0047FE0B`).
pub fn battle_requests(was: &l2_sim::Cues, now: &l2_sim::Cues) -> Vec<Request> {
    use l2_sim::{Troop, WeaponClass, ALL_TROOPS, SIDE_A, SIDE_B};
    let struck = |t: Troop| now.melee_casualties(t) != was.melee_casualties(t);
    let hit = |w: WeaponClass| now.missile_hits(w) != was.missile_hits(w);
    let felled = |w: WeaponClass| now.missile_casualties(w) != was.missile_casualties(w);
    let loosed = |w: WeaponClass| now.loosed(w) != was.loosed(w);
    let mut out = Vec::new();
    let mut ask = |moved: bool, request: Request| {
        if moved {
            out.push(request);
        }
    };

    // **`Melee_Tick` (`0x00494908`), a man falling to a blow.** The sword is
    // chosen by the troop that **struck** him: `other.troopType == 2 ? 4 : == 3
    // ? 5 : == 6 ? 5 : 6`. `[V]`
    // sfx: Melee_Tick#1
    ask(struck(Troop::Macemen), Request::Slot(4));
    // sfx: Melee_Tick#2
    ask(struck(Troop::Swordsmen), Request::Slot(5));
    // sfx: Melee_Tick#3
    ask(struck(Troop::Knights), Request::Slot(5));
    // sfx: Melee_Tick#4
    ask(
        ALL_TROOPS
            .iter()
            .filter(|t| !matches!(t, Troop::Macemen | Troop::Swordsmen | Troop::Knights))
            .any(|&t| struck(t)),
        Request::Slot(6),
    );
    // **`Melee_Tick`, the last man of a figure**, by the dying figure's side:
    // `me.side == 0 ? 0xB : me.side == 4 ? 0xC`. `[V]`
    // sfx: Melee_Tick#5
    ask(now.melee_deaths(SIDE_A) != was.melee_deaths(SIDE_A), Request::Slot(0xb));
    // sfx: Melee_Tick#6
    ask(now.melee_deaths(SIDE_B) != was.melee_deaths(SIDE_B), Request::Slot(0xc));

    // **`Missile_Step` (`0x00492C8B`).** A catapult shot counted against a
    // wall; a shot striking a man, crossbow 10 and bow 8; the same slot again
    // on a casualty, which is always dropped because that buffer started a
    // statement earlier; and `0xD` for the last man. `[V]`
    // sfx: Missile_Step#1
    ask(now.walls_struck() != was.walls_struck(), Request::Slot(0xf));
    // sfx: Missile_Step#3
    ask(hit(WeaponClass::Crossbow), Request::Slot(10));
    // sfx: Missile_Step#4
    ask(hit(WeaponClass::Bow), Request::Slot(8));
    // sfx: Missile_Step#5
    ask(felled(WeaponClass::Crossbow), Request::Slot(10));
    // sfx: Missile_Step#6
    ask(felled(WeaponClass::Bow), Request::Slot(8));
    // sfx: Missile_Step#7
    ask(now.missile_deaths() != was.missile_deaths(), Request::Slot(0xd));

    // **The shot leaving.** `BattleMan_FireMissile` (`0x00483337`): crossbow 9,
    // bow 7. `BattleMan_StateEngineFire` (`0x004843BC`): the catapult, `0xE`.
    // `[V]` for the slots; `[D]` that our catapult's loose is the same occasion,
    // because ours fires through the shared reload path rather than the
    // engine's own 100-of-180 cadence.
    // sfx: BattleMan_FireMissile#1
    ask(loosed(WeaponClass::Crossbow), Request::Slot(9));
    // sfx: BattleMan_FireMissile#2
    ask(loosed(WeaponClass::Bow), Request::Slot(7));
    // sfx: BattleMan_StateEngineFire#1
    ask(loosed(WeaponClass::Catapult), Request::Slot(0xe));

    // **`Wall_Smash` (`FUN_0049694F`)**, whose first statement is
    // `Sound_PlayFile("bathit2.wav", 0, 0)`. `[V]`
    // sfx: FUN_0049694f#1
    ask(now.walls_smashed() != was.walls_smashed(), Request::File(names::battle::WALL_SMASH));

    out
}

/// **`Sound_PlayTroopCry` (`0x00499CB1`)'s body, and `g_troopCryCounter`
/// (`0x0053EF60`) with it.**
///
/// ```c
/// counter[unit][class] += 1;
/// if (3 < counter[unit][class]) counter[unit][class] = 0;
/// take = counter[unit][class];
/// if (class == 3) take = 0;
/// Sound_PlayFile(g_troopSounds + class*0x40 + take*0x10 + unit*0x100, 1, 0);
/// ```
///
/// `[V]`, and **no random number anywhere in it**: the take is a round robin
/// per (troop, class), stepped *before* it is read, so the first cry of each
/// pair is take **1**, not take 0, and the cycle is 1, 2, 3, 0. The counter is
/// in `.bss` and this function is its only writer — an exhaustive reference
/// search — so it starts at zero with the process and is never reset between
/// battles. Ours lives on the [`Director`], which lives as long as the process.
///
/// That settles the determinism question the brief raised before it could
/// arise: **a cry draws on no generator at all**, so there is nothing to keep
/// away from the simulation's `Pcg32`. If a future site does need presentation
/// randomness, it needs a generator of its own on the audio side, never the
/// battle's.
///
/// The counter steps **even when the cry is dropped**, because the drop is
/// inside `Sound_PlayFile`. So two orders in quick succession sound take 1 and
/// then, when the next is heard, take 3 — the dropped take 2 was spent.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TroopCries {
    /// `[troop][class]`, 0…3.
    counter: [[u8; 4]; 11],
}

impl TroopCries {
    /// Step the counter for one cry and name the file it plays — `None` for a
    /// cell holding `null.wav` (a siege engine told anything but to move) and
    /// for anything out of range. The counter steps in the `None` case too, as
    /// the original's does.
    pub fn cry(&mut self, troop: u8, class: u8) -> Option<&'static str> {
        let (t, c) = (troop as usize, class as usize);
        let n = self.counter.get_mut(t)?.get_mut(c)?;
        *n += 1;
        if *n > 3 {
            *n = 0;
        }
        let take = if c == 3 { 0 } else { *n as usize };
        names::troop_cry(t, c, take)
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
/// title is still the front end"* holds without a special case —
/// [`ScreenId::SaveLoad`] is the in-game one (`g_screenId` `0x35`/`0x36`) and
/// is a different screen. [`ScreenId::Menu`] is the placeholder front end the
/// application no longer starts on, and [`ScreenId::Index`] is ours and is
/// pushed from the title by `I`; over a running game both sit *above*
/// [`ScreenId::Campaign`], so the stack still answers `Campaign`.
fn before_the_campaign(id: crate::screen::ScreenId) -> bool {
    use crate::screen::ScreenId as S;
    match id {
        S::Setup(_) | S::Menu | S::Index => true,
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
        // from a screen of its own -- so a message is never the reason music
        // starts or stops, and it can only be up once a game is running. This
        // arm exists because the exhaustive match made somebody answer the
        // question, which is the whole point of it having no wildcard.
        | S::Message
        // Screen `0x27`, a tip's. `g_appPhase == 3` gates `Tip_Update`, so it
        // is only ever raised over a running game.
        | S::Tip
        | S::Options(_)
        | S::Battlefield
        | S::MenuBar(_)
        | S::About
        | S::Court
        | S::Supplies(_)
        | S::Ratings
        | S::Info(_) => false,
    }
}

/// Which of `battle1..5` a file name is, for the "still in the same battle"
/// case where the counter must not be stepped again.
fn current_battle_track(name: String) -> Option<Music> {
    names::MUSIC_BATTLE
        .iter()
        .position(|n| *n == name)
        .map(|i| Music::Battle(i as u8 + 1))
}

/// Open the default output device and hand it the mixer.
///
/// Split out so that every way this can go wrong arrives at one `Err` and one
/// message. `cpal` reports "no host", "no device" and "no supported config" as
/// three different shapes; downstream they are all "run silent".
fn start_device(mixer: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("no default output config: {e}"))?;
    let rate = supported.sample_rate().0;
    let channels = supported.channels() as usize;
    if let Ok(mut m) = mixer.lock() {
        *m = Mixer::new(rate);
    }
    let config: cpal::StreamConfig = supported.config();
    let err = |e| eprintln!("sound: stream error: {e}");

    // The callback fills a stereo scratch buffer and then spreads it over
    // however many channels the device has. A device with more than two gets
    // the pair on its first two and silence elsewhere, which is the right
    // answer for a game whose source material is at most stereo.
    macro_rules! stream {
        ($sample:ty, $to:expr) => {{
            let mixer = Arc::clone(&mixer);
            let mut scratch: Vec<f32> = Vec::new();
            device
                .build_output_stream(
                    &config,
                    move |out: &mut [$sample], _| {
                        let frames = out.len() / channels.max(1);
                        scratch.resize(frames * 2, 0.0);
                        match mixer.lock() {
                            Ok(mut m) => m.fill(&mut scratch),
                            // Poisoned or contended: silence for this buffer
                            // rather than a panic inside the audio thread.
                            Err(_) => scratch.iter_mut().for_each(|s| *s = 0.0),
                        }
                        for (i, frame) in out.chunks_mut(channels.max(1)).enumerate() {
                            for (c, slot) in frame.iter_mut().enumerate() {
                                let v = if c < 2 { scratch[i * 2 + c] } else { 0.0 };
                                *slot = $to(v);
                            }
                        }
                    },
                    err,
                    None,
                )
                .map_err(|e| format!("{e}"))?
        }};
    }

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => stream!(f32, |v: f32| v),
        cpal::SampleFormat::I16 => stream!(i16, |v: f32| (v * 32767.0) as i16),
        cpal::SampleFormat::U16 => stream!(u16, |v: f32| ((v * 32767.0) as i32 + 32768) as u16),
        other => return Err(format!("unsupported sample format {other:?}")),
    };
    stream.play().map_err(|e| format!("{e}"))?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything below runs on the silent layer, which is the state a machine
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
        a.play_speech("kt170_1.wav");
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
        // it is checked here without a device in the way.
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
