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
//! Music, the two message fanfares and the pointer click. Not the troop
//! voices, not the village's work sounds, not the lord voices — the tables for
//! all three are recovered and written down in [`names`], and what remains is
//! call sites in screens two other agents are inside. See the report.

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
        }
    }

    /// Open the default output device and index the `.wav` files the vfs can
    /// see. **Never fails**; on any problem it reports once on stderr and
    /// returns something silent.
    pub fn open(vfs: &l2_mods::vfs::Vfs) -> Audio {
        let mut audio = Audio::silent();
        for name in vfs.entries_with_extension("wav") {
            if let Some(path) = vfs.resolve(name) {
                // The vfs already normalises, but the executable's tables and
                // the install's directory disagree about case, so the key is
                // pinned lower-case here as well rather than trusted.
                audio.files.insert(name.to_ascii_lowercase(), path.to_path_buf());
            }
        }
        match start_device(Arc::clone(&audio.mixer)) {
            Ok(stream) => audio.stream = Some(stream),
            Err(e) => eprintln!("sound: no output device ({e}); running silent"),
        }
        if audio.files.is_empty() {
            eprintln!("sound: no .wav files in the install; running silent");
        }
        audio
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
        if self.stream.is_none() {
            return None;
        }
        let key = name.to_ascii_lowercase();
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let sound = self.decode(&key).map(Arc::new);
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
/// The front end is read off the **bottom** of the stack rather than the top,
/// because the load screen opened from the title is still the front end while
/// the same screen opened mid-game is not.
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
    if let Some(ScreenId::Setup(_)) = machine.ids().first() {
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
