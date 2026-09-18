
use std::sync::Arc;

use super::wav::Sound;

struct Voice {
    /// **The buffer handle** — `DAT_00522AEC`, the pointer
    /// `Sound_OneShotBusy` (`0x00427C9B`) asks `GetStatus` of. A name is not a
    /// handle here: [`Mixer::play_effect`] (`Sound_RestartSlot`) replaces a
    /// voice of the same file, and the eight-voice cap evicts the oldest, so a
    /// name can answer *busy* for a clip this mixer no longer holds and *idle*
    /// for one it does. Minted per voice, never reused.
    id: u64,
    name: String,
    sound: Arc<Sound>,
    pos: u64,
    step: u64,
    looping: bool,
    gain: i32,
}

impl Voice {
    pub(super) fn new(name: String, sound: Arc<Sound>, out_rate: u32, looping: bool, gain: i32) -> Voice {
        let step = ((sound.rate as u64) << 32) / (out_rate.max(1) as u64);
        Voice { id: 0, name, sound, pos: 0, step, looping, gain }
    }

    fn next(&mut self) -> Option<(i32, i32)> {
        let frames = self.sound.frames();
        if frames == 0 {
            return None;
        }
        let i = (self.pos >> 32) as usize;
        if i >= frames {
            if !self.looping {
                return None;
            }
            self.pos %= (frames as u64) << 32;
            return self.next();
        }

        let (l0, r0) = self.sound.frame(i);
        let (l1, r1) = if i + 1 < frames {
            self.sound.frame(i + 1)
        } else if self.looping {
            self.sound.frame(0)
        } else {
            (0, 0)
        };
        let f = ((self.pos >> 16) & 0xffff) as i32;
        let lerp = |a: i16, b: i16| (a as i32) + (((b as i32) - (a as i32)) * f >> 16);

        self.pos = self.pos.wrapping_add(self.step);
        Some((
            lerp(l0, l1) * self.gain >> 8,
            lerp(r0, r1) * self.gain >> 8,
        ))
    }
}

const MAX_EFFECTS: usize = 8;

pub struct Mixer {
    out_rate: u32,
    music: Option<Voice>,
    music_name: Option<String>,
    effects: Vec<Voice>,
    /// **The one-shot buffer** — `DAT_00522AEC`, the buffer `Sound_PlayFile`
    /// (`0x00427990`) builds every file into. **Its own, as in the original**:
    ///
    /// the slot bank above is `Sound_PlaySlot`/`Sound_RestartSlot`
    /// (`0x00426120`, `0x00426216`) and never touches it, so a click firing the
    /// same file neither stops it nor makes `Sound_OneShotBusy` (`0x00427C9B`)
    /// answer idle, and the eight-slot cap cannot evict it.
    one_shot: Option<Voice>,
    next_id: u64,
    /// `Smk_Open` asks `SmackOpen` for track 0 (flag `0x2000`, `[I]` from RAD's
    /// published SDK constants) whenever DirectSound is up, and plays it
    /// through `smackw32`'s own buffer — not through `Music_Play` and not
    /// through the effect slots, so **none of the three sound switches touches
    /// it**. Its own voice here for the same reason, mixed at unity.
    film: Option<Voice>,
    pub music_on: bool,
    pub effects_on: bool,
}

impl Mixer {
    pub fn new(out_rate: u32) -> Mixer {
        Mixer {
            out_rate: out_rate.max(1),
            music: None,
            music_name: None,
            effects: Vec::new(),
            one_shot: None,
            next_id: 1,
            film: None,
            music_on: true,
            effects_on: true,
        }
    }

    pub fn set_film(&mut self, name: String, sound: Arc<Sound>) {
        self.film = Some(Voice::new(name, sound, self.out_rate, false, 256));
    }

    pub fn stop_film(&mut self) {
        self.film = None;
    }

    pub fn film_name(&self) -> Option<&str> {
        self.film.as_ref().map(|v| v.name.as_str())
    }

    pub fn out_rate(&self) -> u32 {
        self.out_rate
    }

    pub fn music_name(&self) -> Option<&str> {
        self.music_name.as_deref()
    }

    pub fn set_music(&mut self, name: String, sound: Arc<Sound>) {
        self.start_music(name, sound, true);
    }

    /// Start a track that plays **once** and then stops — `Music_Play`
    /// (`0x004263AD`)'s third argument 0, which only `Screen_DrawConquest`
    /// (`0x0041E1DD`) passes: `setup2.wav` over the interstitial once the
    /// campaign is past its eighth map.
    pub fn set_music_once(&mut self, name: String, sound: Arc<Sound>) {
        self.start_music(name, sound, false);
    }

    fn start_music(&mut self, name: String, sound: Arc<Sound>, looping: bool) {
        self.music = Some(Voice::new(name.clone(), sound, self.out_rate, looping, 192));
        self.music_name = Some(name);
    }

    pub fn stop_music(&mut self) {
        self.music = None;
        self.music_name = None;
    }

    pub fn is_playing(&self, name: &str) -> bool {
        self.effects.iter().any(|v| v.name == name)
    }

    /// Whether that file is sounding **anywhere** — a slot or the one-shot
    /// buffer. The observability question; [`Mixer::is_playing`] is the
    /// `GetStatus` `Sound_PlaySlot` (`0x00426120`) asks of one slot, and
    /// [`Mixer::is_playing_handle`] the one `Sound_OneShotBusy` (`0x00427C9B`)
    /// asks of `DAT_00522AEC`.
    pub fn is_sounding(&self, name: &str) -> bool {
        self.is_playing(name) || self.one_shot.as_ref().is_some_and(|v| v.name == name)
    }

    /// Stop that effect wherever it is in the mix — the `Stop` half of
    /// `Sound_StopOneShot` (`0x00427D19`). Nothing when it is not sounding.
    pub fn stop_effect(&mut self, name: &str) {
        self.effects.retain(|v| v.name != name);
    }

    /// Fire a one-shot, restarting it if it is already sounding —
    /// `Sound_RestartSlot` (`0x00426216`), which does `SetCurrentPosition(0)`
    /// then `Play` unconditionally. This is what a click uses.
    pub fn play_effect(&mut self, name: String, sound: Arc<Sound>) {
        self.effects.retain(|v| v.name != name);
        if self.effects.len() >= MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(Voice::new(name, sound, self.out_rate, false, 256));
    }

    /// **Load and play the one-shot buffer** — `Sound_PlayFile` (`0x00427990`),
    /// which owns `DAT_00522AEC` and replaces whatever it held. [`Mixer`]'s
    /// `one_shot` field says why it is not a slot.
    ///
    /// Answers the handle `Sound_OneShotBusy` (`0x00427C9B`) asks `GetStatus`
    /// of.
    pub fn play_one_shot(&mut self, name: String, sound: Arc<Sound>) -> u64 {
        let mut voice = Voice::new(name, sound, self.out_rate, false, 256);
        voice.id = self.next_id;
        self.next_id += 1;
        self.one_shot = Some(voice);
        self.next_id - 1
    }

    /// **`Sound_OneShotBusy` (`0x00427C9B`)**, asked of the handle:
    ///
    /// `GetStatus(DAT_00522AEC) == 1`.
    pub fn is_playing_handle(&self, handle: u64) -> bool {
        self.one_shot.as_ref().is_some_and(|v| v.id == handle)
    }

    /// The `Stop`/`Release` half of `Sound_StopOneShot` (`0x00427D19`), of that
    /// buffer and not of a slot that shares its file.
    pub fn stop_handle(&mut self, handle: u64) {
        if self.is_playing_handle(handle) {
            self.one_shot = None;
        }
    }

    /// Fire a one-shot **only if that sound is not already playing** —
    /// `Sound_PlaySlot` (`0x00426120`), which reads `GetStatus` and returns 0
    /// when the buffer is `DSBSTATUS_PLAYING`.
    ///
    /// **This is the whole of the original's effect-mixing policy**, and it is
    /// why the game does not stack. `Unit_MoveInFacing` (`0x00466D84`) asks for
    /// `Army.wav` on *every step of every moving unit*; drop-if-busy is what
    /// turns a burst of identical requests into one continuous march, and what
    /// stops twelve units marching at once from being twelve times as loud.
    pub fn play_effect_if_idle(&mut self, name: String, sound: Arc<Sound>) -> bool {
        if self.effects.iter().any(|v| v.name == name) {
            return false;
        }
        if self.effects.len() >= MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(Voice::new(name, sound, self.out_rate, false, 256));
        true
    }

    pub fn fill(&mut self, out: &mut [f32]) {
        for f in out.iter_mut() {
            *f = 0.0;
        }
        for frame in out.chunks_mut(2) {
            let mut l = 0i32;
            let mut r = 0i32;
            if self.music_on {
                if let Some(v) = self.music.as_mut() {
                    if let Some((a, b)) = v.next() {
                        l += a;
                        r += b;
                    }
                }
            }
            if let Some(v) = self.film.as_mut() {
                match v.next() {
                    Some((a, b)) => {
                        l += a;
                        r += b;
                    }
                    None => self.film = None,
                }
            }
            if self.effects_on {
                if let Some(v) = self.one_shot.as_mut() {
                    match v.next() {
                        Some((a, b)) => {
                            l += a;
                            r += b;
                        }
                        None => self.one_shot = None,
                    }
                }
                self.effects.retain_mut(|v| match v.next() {
                    Some((a, b)) => {
                        l += a;
                        r += b;
                        true
                    }
                    None => false,
                });
            } else {
                // `Opt_ToggleSoundEffects` (`0x00434A29`, `00430000.c:1881`) flips the flag and
                // stops nothing: `Sound_PlayFile` reads it at start (`00420000.c:2609`), so a
                // one-shot already sounding plays on with effects off.
                self.effects.clear();
            }
            frame[0] = (l.clamp(-32768, 32767) as f32) / 32768.0;
            if let Some(second) = frame.get_mut(1) {
                *second = (r.clamp(-32768, 32767) as f32) / 32768.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(rate: u32, value: i16, frames: usize) -> Arc<Sound> {
        Arc::new(Sound { channels: 1, rate, samples: vec![value; frames] })
    }

    #[test]
    fn a_one_shot_ends_and_is_dropped() {
        let mut m = Mixer::new(11025);
        m.play_effect("a".into(), constant(11025, 1000, 3));
        let mut buf = [0f32; 16];
        m.fill(&mut buf);
        assert!(buf[0] > 0.0, "the sound started");
        assert_eq!(buf[15], 0.0, "and it did not run forever");
        m.fill(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0), "the voice was retired");
    }

    #[test]
    fn a_looping_track_keeps_going_past_its_length() {
        let mut m = Mixer::new(11025);
        m.set_music("x".into(), constant(11025, 4000, 3));
        let mut buf = [0f32; 64];
        m.fill(&mut buf);
        assert!(buf[62] != 0.0, "still playing well past three frames");
    }

    /// `Music_Play(name, 0, 0)` — `Screen_DrawConquest` (`0x0041E1DD`) above
    /// the eighth campaign map, and the only call site that passes it.
    ///
    /// Ablation: `set_music_once` forwarding to `set_music` and this is red.
    #[test]
    fn a_one_shot_track_stops_at_its_end_instead_of_starting_again() {
        let mut m = Mixer::new(11025);
        m.set_music_once("x".into(), constant(11025, 4000, 3));
        let mut buf = [0f32; 64];
        m.fill(&mut buf);
        assert!(buf[0] != 0.0, "it played");
        assert_eq!(buf[62], 0.0, "and it did not come round again");
    }

    #[test]
    fn upsampling_stretches_a_sound_by_the_rate_ratio() {
        let mut m = Mixer::new(44100);
        m.play_effect("a".into(), constant(11025, 8000, 4));
        let mut buf = [0f32; 2 * 20];
        m.fill(&mut buf);
        assert!(buf[2 * 15] != 0.0, "frame 15 is still inside the sound");
        assert_eq!(buf[2 * 17], 0.0, "frame 17 is past the end");
    }

    #[test]
    fn silencing_music_leaves_effects_alone_and_the_other_way_round() {
        let mut m = Mixer::new(11025);
        m.set_music("x".into(), constant(11025, 4000, 64));
        m.music_on = false;
        let mut buf = [0f32; 8];
        m.fill(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));

        m.music_on = true;
        m.fill(&mut buf);
        assert!(buf[0] != 0.0);
    }

    #[test]
    fn the_mix_is_clamped_rather_than_wrapped() {
        let mut m = Mixer::new(11025);
        for i in 0..MAX_EFFECTS {
            m.play_effect(format!("{i}"), constant(11025, i16::MAX, 8));
        }
        let mut buf = [0f32; 4];
        m.fill(&mut buf);
        assert!(buf.iter().all(|&s| (0.0..=1.0).contains(&s)), "{buf:?}");
    }

    #[test]
    fn overlapping_more_than_the_limit_drops_the_oldest_rather_than_growing() {
        let mut m = Mixer::new(11025);
        for i in 0..MAX_EFFECTS + 5 {
            m.play_effect(format!("{i}"), constant(11025, 100, 1000));
        }
        assert_eq!(m.effects.len(), MAX_EFFECTS);
    }

    /// `DAT_00522AEC` is not one of the slots: neither `Sound_RestartSlot`'s
    /// same-file replacement nor the cap can take it.
    #[test]
    fn the_slot_cap_cannot_evict_the_one_shot_buffer() {
        let mut m = Mixer::new(11025);
        let h = m.play_one_shot("bathit2.wav".into(), constant(11025, 3000, 1000));
        m.play_effect("bathit2.wav".into(), constant(11025, 3000, 1000));
        assert!(m.is_playing_handle(h), "a slot of the same file took the buffer");
        for i in 0..MAX_EFFECTS + 5 {
            m.play_effect(format!("{i}"), constant(11025, 100, 1000));
        }
        assert!(m.is_playing_handle(h), "the cap evicted the buffer");
        assert_eq!(m.effects.len(), MAX_EFFECTS, "and the cap still holds over the slots");
    }

    #[test]
    fn a_marching_army_asks_every_step_and_gets_one_voice() {
        let mut m = Mixer::new(11025);
        assert!(m.play_effect_if_idle("army.wav".into(), constant(11025, 3000, 200)));
        for _ in 0..50 {
            assert!(!m.play_effect_if_idle("army.wav".into(), constant(11025, 3000, 200)));
        }
        assert_eq!(m.effects.len(), 1);

        assert!(m.play_effect_if_idle("merchant.wav".into(), constant(11025, 3000, 200)));
        assert_eq!(m.effects.len(), 2);

        let mut buf = [0f32; 2 * 256];
        m.fill(&mut buf);
        assert!(m.play_effect_if_idle("army.wav".into(), constant(11025, 3000, 200)));
    }

    #[test]
    fn a_click_restarts_itself_rather_than_being_dropped() {
        let mut m = Mixer::new(11025);
        m.play_effect("click3.wav".into(), constant(11025, 3000, 200));
        m.play_effect("click3.wav".into(), constant(11025, 3000, 200));
        assert_eq!(m.effects.len(), 1, "one voice, rewound - not two stacked");
        assert_eq!(m.effects[0].pos, 0, "and it started over");
    }
}
