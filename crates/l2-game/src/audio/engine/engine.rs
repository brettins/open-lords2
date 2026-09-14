#![allow(unused_imports)]
use super::*;
use super::device::*;
use super::*;
use super::director::*;
use super::events::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mixer::Mixer;
use track::{BattleCycle, BattleKind, Music};
use wav::Sound;

impl Audio {
    /// An audio layer with no device and no files. Every method is a no-op and
    /// none of them fail. This is what `--no-sound` builds, and what a machine
    /// with no sound card ends up with anyway.
    pub fn silent() -> Audio {
        Audio {
            mixer: Arc::new(Mutex::new(Mixer::new(44_100))),
            stream: None,
            files: BTreeMap::new(),
            films: BTreeMap::new(),
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
    /// decoded and the mixer runs.
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
                // pinned lower-case here as well.
                audio.files.insert(name.to_ascii_lowercase(), path.to_path_buf());
            }
        }
        for name in vfs.entries_with_extension("smk") {
            if let Some(path) = vfs.resolve(name) {
                audio.films.insert(name.to_ascii_lowercase(), path.to_path_buf());
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

    /// **Every file this layer has opened, sorted.**
    ///
    /// The answer to *"how much of the game's 771 sounds does the engine
    /// reach?"*, measured by running it. See the field.
    pub fn heard(&self) -> Vec<&str> {
        self.heard.iter().map(String::as_str).collect()
    }

    pub fn options(&self) -> Options {
        self.options
    }

    /// **Is this one-shot still sounding?** — the `GetStatus` bit test
    /// `Sound_OneShotBusy` (`0x00427C9B`) makes, asked by name because ours are
    /// many buffers for each sound.
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
    /// [`Audio::play_file`] put there — every voice line, every troop cry, every
    /// fanfare, `fire.wav` and `bathit2.wav`, since every one of those sites now
    /// goes through it. A request that was dropped never becomes the occupant.
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
            // `Screen_DrawConquest` (`0x0041E1DD`)'s first statement after its
            // early return, both arms of it.
            // sfx: Screen_DrawConquest#1,Screen_DrawConquest#2
            Scene::Conquest { ended } => Some(match ended {
                false => Music::Setup,
                true => Music::Setup2,
            }),
            Scene::Battle(kind) => {
                // Step the counter only on the way *into* a battle. Calling
                // `next` once a frame would flip tracks sixty times a second.
                match self.scene {
                    Some(Scene::Battle(k)) if k == kind => {
                        self.music_name().and_then(current_battle_track)
                    }
                    // `Battle_CheckOutcome` stopped the bed for its film and
                    // `Smk_OnFinished` restarts nothing while `g_battlePhase`
                    // is 2: the field stays silent until it is left.
                    // counter is not stepped, because no battle started.
                    Some(Scene::Film { over_battle: true }) => return,
                    _ => Some(self.battle.next(kind)),
                }
            }
            // **Silence, which is what makes the next derivation a restart.**
            // Every caller of `Smk_Play` runs `Music_Stop(0)` first and then,
            // on whichever arm follows — the film ending (`Smk_OnFinished`) or
            // the film failing to open (the callers' own `if (!Smk_Play(…))`)
            // — starts the bed again from its first sample: `Music_Play
            // ("setup.wav")` back on setup page 1, `Music_StartCampaign()`
            // everywhere else. Ours stops it here, so that when the film's
            // screen goes [`Audio::play_music`] finds nothing playing and
            // starts the track over, keeping the samples. A film
            // that fails to open is held on the stack for one tick
            // (`crate::screens::movie`) holds it for one tick so this arm sees it.
            //
            // **Two of the eight such sites are not claimed, and not for want
            // of this line.** `Msg_DrawWindow#15` is the capture film's fail
            // arm, and nothing in this engine posts a category-`0x0D` letter:
            // `County_ChangeOwner` raises groups `0x75`…`0x7E` with it and
            // `l2_kingdom::conquest::change_owner` leaves them to a caller that
            // does not exist yet. `Msg_DrawWindow#19` is the ending's
            // *fast-media* fail arm, a layout this install never takes. Both
            // would sound through this arm unchanged; neither can be reached.
            // sfx: Smk_OnFinished#1,Smk_OnFinished#2,CastleBuild_Confirm#1,Msg_DrawWindow#20
            Scene::Film { .. } => None,
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
            // A missing or unreadable track stops the music:
            // leaving the previous one running under the wrong screen.
            self.stop_music();
            return;
        };
        if let Ok(mut m) = self.mixer.lock() {
            // `Music_Play`'s loop flag, which only the finished campaign's
            // interstitial passes as 0. See [`Music::loops`].
            match music.loops() {
                true => m.set_music(name.to_string(), sound),
                false => m.set_music_once(name.to_string(), sound),
            }
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
/// or
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
    /// (`0x00426120`).
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

    /// **A film's sound track, from its first sample** — what `SmackOpen` with
    /// track 0 enabled starts, replacing any film already sounding.
    ///
    /// Decoded whole when the film opens: the longest track in the install is
    /// `LOM.SMK`'s 2.66 MB of 8-bit PCM.
    /// none of the three switches, for the reason [`mixer::Mixer`]'s `film`
    /// field gives. A film with no file, or one that will not parse, is silent,
    /// which is also what its picture is.
    pub fn play_film(&mut self, name: &str) {
        match self.decode_film(name) {
            Some(sound) => {
                if let Ok(mut m) = self.mixer.lock() {
                    m.set_film(name.to_ascii_lowercase(), Arc::new(sound));
                }
            }
            None => self.stop_film(),
        }
    }

    /// `SmackClose`.
    pub fn stop_film(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.stop_film();
        }
    }

    /// The film whose track is sounding.
    pub fn film_name(&self) -> Option<String> {
        self.mixer.lock().ok()?.film_name().map(str::to_owned)
    }

    fn decode_film(&mut self, name: &str) -> Option<Sound> {
        if !self.decodes {
            return None;
        }
        let key = name.to_ascii_lowercase();
        let bytes = std::fs::read(self.films.get(&key)?).ok()?;
        let smk = l2_smk::Smk::parse(bytes).ok()?;
        let track = smk.header().track(0)?;
        let pcm = smk.audio(0).ok()?;
        self.heard.insert(key);
        Some(Sound {
            channels: track.channels(),
            rate: track.rate,
// Unsigned 8-bit, centred on 128,
            samples: pcm.iter().map(|&b| ((b as i16) - 128) << 8).collect(),
        })
    }

    /// **`Sound_StopOneShot` (`0x00427D19`)** — stop and release the one-shot
    /// buffer, whatever it holds. `[V]`: `Stop`, `Release`, and `DAT_00522AEC`
    /// zeroed, so `Sound_OneShotBusy` answers 0 until something is put there
    /// again.
    pub fn stop_one_shot(&mut self) {
        if let Some(name) = self.one_shot.take() {
            if let Ok(mut m) = self.mixer.lock() {
                m.stop_effect(&name);
            }
        }
    }

    /// **`Sound_PlayFile` (`0x00427990`), drop included** — every file the
    /// original plays by name, with the flag it passes.
    ///
    /// ```c
    /// if (Sound_OneShotBusy()) return 0;         /* first, before either flag */
    /// Sound_StopOneShot();
    /// if (isSpeech == 1 && g_optSpeech == 0) return 0;
    /// if (isSpeech == 0 && g_optSoundEffects == 0) return 0;
    /// /* load the file into the one buffer and Play it */
    /// ```
    ///
    /// `[V]`. There is **one** one-shot buffer.
    /// other is still sounding is not played at all, **and does not take the
    /// buffer**: the clip that was there stays the one `Sound_OneShotBusy`
    /// asks about. A cry over a cry, a cry over the narrator, a fanfare over a
    /// cry, the narrator's *"supplies"* over a wall coming down — every one is
    /// dropped. That is the whole of the original's limit on how often the men
    /// answer, and it is why a player clicking ten orders a second hears one
    /// voice. Answers whether it started.
    ///
    /// `speech` is the call's own `isSpeech`, read at each site,
    /// implied by what the file is: **the message fanfares pass 1**, so it is
    /// the Speech switch that silences `ff_msg.wav` and `ff_capt.wav`, and
    /// `ff_batl.wav`, `fire.wav` and `bathit2.wav` pass 0.
    ///
    /// Where the original **interrupts** instead — `Sound_StopOneShot` in front
    /// of the call — the verb is [`Audio::stop_and_play_file`].
    pub fn play_file(&mut self, name: &str, speech: bool) -> bool {
        if self.one_shot_busy() {
            return false;
        }
        // The buffer is idle, so this releases a clip that has finished and
        // stops nothing audible.
        self.stop_one_shot();
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

    /// **`Sound_StopOneShot(); Sound_PlayFile(name, …)`** — the sites that
    /// *cut in* over whatever the buffer holds. `[V]`,
    /// and there are exactly two functions' worth among what we play:
    ///
    /// * **`Msg_PlayVoice` (`0x004B35C1`)**, in all three of its bands — which
    ///   is why *"a lord cuts off the previous lord"*, and why a message's
    ///   narration talks over its own fanfare if the trumpet is still going;
    /// * **setup page 4's handlers**, `FUN_00432CC8` twice and
    ///   `Setup_ChooseCampaign` (`0x00433461`), each `S011_02.wav`.
    ///
    /// Every other `Sound_PlayFile` we reproduce has no stop in front of it and
    /// is [`Audio::play_file`]. The drop inside `Sound_PlayFile` cannot fire
    /// here, because the stop has just emptied the buffer.
    pub fn stop_and_play_file(&mut self, name: &str, speech: bool) -> bool {
        self.stop_one_shot();
        self.play_file(name, speech)
    }

    /// Decode a file, caching the small ones.
    ///
    /// The cache stores the *failure* too, as `None`, so that a missing file is
    /// looked for once.
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

