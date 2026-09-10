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
//! **Read this before believing the layer plays anything.** The list below was
//! wrong when it was written — it claimed the pointer click, which has never
//! had a call site — and the sentence above it claimed music, which
//! [`scene`] could not reach. Both are corrected here, and the count is the
//! honest measure: of the install's **771** `.wav` files, this layer can
//! currently reach **11** — `scroll1`…`scroll5`, `battle1`…`battle4`,
//! `ff_msg.wav` and `ff_batl.wav`.
//!
//! **That number is measured, not typed**, and the first time it *was* typed it
//! said 12: `battle5.wav` ships and decodes and nothing can ask for it, because
//! the counter that selects it is `DAT_0057A0F0` — the third battle mode
//! [`track::BattleKind`] declines to guess at.
//! `tests/audio_wiring.rs::eleven_of_the_installs_771_sounds_are_reachable`
//! drives every scene the policy can produce and reads [`Audio::heard`], so a
//! new call site moves the number by itself.
//!
//! **By the six classes the original divides its sound into, one and a half
//! work.** That is a more useful sentence than *"audio is broken"* and it is
//! the one to keep current:
//!
//! | class | the original's call site | ours |
//! |---|---|---|
//! | **Music** | `Music_StartCampaign` `0x00499ACA`, `Music_StartBattle` `0x00477B2F` | **✅ both** — [`scene`] and [`Audio::follow`] |
//! | **Event fanfares** | `Sound_PlayFile` at four sites | **◐ 2 of 4** — `ff_msg`, `ff_batl` |
//! | The pointer click | `Widget_Test` `0x0040DA1E`, slot 1 | ✗ |
//! | The two sample banks | `Sound_PlaySlot` `0x00426120` | ✗ |
//! | Message narration | `Msg_PlayVoice` `0x004B35C1` | ✗ |
//! | Troop cries | `FUN_00499CB1` | ✗ |
//!
//! The four that do not, in the order a player notices them:
//!
//! * **The pointer click.** `Widget_Test` (`0x0040DA1E`) plays slot 1,
//!   `click3.wav`, on every widget press — **one** call site in the original,
//!   because the whole game shares one hit-tester. Ours do not: 26 screen
//!   modules each match `Event::Click` against their own rectangles, so there
//!   is no single place to put it, and nothing above them can tell a press that
//!   landed on a widget from one that landed on grass. Playing it on every
//!   click would be an invention and a worse one than silence. **The enabling
//!   change is in the screen layer, not here**: `Screen::handle` would have to
//!   say whether it consumed the event at a widget, and then this is one call.
//! * **The two sample banks.** [`names::KINGDOM_BANK`] and
//!   [`names::BATTLE_BANK`] are recovered and tested against the install; 27 of
//!   their 29 slots ship, and **nothing calls [`Audio::play_effect_if_idle`]**.
//!   The village's work sounds, the marching army, the merchant's cart, the
//!   peasant mob, every sword and every arrow are in there.
//! * **The voices, including the industry toggle a player asked about.**
//!   [`names::lord_voice`] and [`names::system_voice`] generate the names of all
//!   448 lord takes and the system lines, and [`Audio::play_speech`] plays them;
//!   what is missing is `Msg_PlayVoice`'s trigger, which belongs to the message
//!   window. Switching an industry from the map is a *message*, not a sound
//!   effect: `Industry_ToggleFromMap` (`0x0043D309`) ends with
//!   `Msg_Enqueue(0, g_localPlayer, local_10 + 0xE6, 0, 4, 0, 0, 0)` where
//!   `local_10` is `industry * 2 + on` for the four industries and `-1`/`-2` for
//!   the castle switch — so `L2.eng` **228** and **229** are *Building off/on*
//!   and **230**…**237** are the four industries, and `S228_01.wav`…`S237_01.wav`
//!   all ship. `[V]`, from the decompilation; `docs/formats/eng.md` had the
//!   extent right and marked it `[D]`. `l2_kingdom::industry::toggle_from_map`
//!   makes the state change and **raises no message**, so there is nothing for a
//!   voice to hang on yet. Two dependencies, in this order: the enqueue, then
//!   the message window.
//! * **`ff_lose.wav`** needs a gate rather than a call site — see
//!   [`names::fanfare::AFTER_BATTLE`] — and **`ff_capt.wav`** is
//!   `Msg_DrawWindow`'s conquest band, which needs the message window too.

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
    /// Before a game exists — the front end. The original plays no music here:
    /// `Music_StartCampaign` is reached from the campaign coming up, and the
    /// title screen's only sound is `setup.wav`.
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
            Scene::FrontEnd => None,
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
            || names::MUSIC_BATTLE.contains(&key.as_str());
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
/// hand — which is exactly what the test that covered it built. `CNEW-bottom`.
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
    /// `game.turns_played` as it stood at the last tick, so that the end of a
    /// turn can be noticed without anything having to report it.
    turns_heard: u32,
    /// Whether the battle prompt was already up at the last tick, so
    /// `ff_batl.wav` sounds once when the battle is announced rather than sixty
    /// times a second while the player decides.
    prompt_heard: bool,
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

        audio.follow(scene(machine, game));

        // **`ff_batl.wav`, when a battle is announced.**
        // `Battle_ChooseSettlement` (`0x004A6A30`) plays it on the branch that
        // pushes screen `0x12` — the one where at least one side is human —
        // and takes no other sound with it, so this is the whole of that call
        // site. Our prompt appears on exactly that branch, so the screen
        // arriving *is* the event.
        let prompt_up = machine.ids().iter().any(|id| matches!(id, ScreenId::BattlePrompt));
        if prompt_up && !self.prompt_heard {
            audio.play_effect(names::fanfare::BATTLE);
        }
        self.prompt_heard = prompt_up;

        // **There is no end-of-turn sound in the original**, and this is not
        // one. Nothing on the `Turn_End` / `Season_Advance` / phase-7 path
        // plays anything, the End Turn button is silent, and both call sites of
        // the end-of-turn screen fade carry no sound either.
        //
        // What a player hears at the end of a turn is the *message window*
        // opening: `Msg_DrawWindow` (`0x0047309E`) plays `ff_msg.wav` on the
        // frame `g_messageTimer` reaches 2000, and a turn ends in a run of
        // message windows. So the chime belongs to the window.
        //
        // We have no message windows yet, so this fires **once per turn that
        // produced any message** rather than once per window. It is an
        // approximation and it is written down as one: when the message windows
        // exist, the call belongs on the window and this goes away.
        if game.turns_played != self.turns_heard {
            self.turns_heard = game.turns_played;
            let spoke = game.last_report.as_ref().is_some_and(|r| !r.messages.is_empty());
            if spoke {
                audio.play_effect(names::fanfare::MESSAGE);
            }
        }
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
