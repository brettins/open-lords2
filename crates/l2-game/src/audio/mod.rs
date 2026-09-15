//! **A previous count here said 12 and was wrong at 11**, by reading the battle
//! table instead of driving it — `battle5.wav` ships, decodes, and cannot be
//! asked for, because the counter that selects it is `DAT_0057A0F0`, the third
//! battle mode [`track::BattleKind`] declines to guess at.
//!
//! written.** `crates/l2-game/tests/sfx/main.rs` requires the set of sites that file
//! calls `reproduced` to equal the set of `// sfx:` markers in `crates/`, and
//! `node tools/oracle/sounds.js --check` requires the file's *rows* to equal
//! what the decompilation holds. So the sentence below cannot go stale without
//! something going red, which is the whole reason it is safe to write a number
//! here at all — the previous version of this table was prose, was hand-marked,
//! and was wrong in both directions.
//!
//! | class | the original's call site | sites | ours |
//! |---|---|---:|---:|
//! | **Message narration** | `Msg_PlayVoice` `0x004B35C1` | 16 | **15** — [`voice_tick`].
//!
//! | **Music** | `Music_StartCampaign`, `Music_StartBattle`, `Music_Play` | 23 | **15** — [`scene`], four of them the bed restarting after a film ([`Scene::Film`]) |
//! | **By name** | `Sound_PlayFile` | 49 | **17** — [`names::speech`], the fanfares, the forge, the tips' chain and `Wall_Smash`; every one through [`Audio::play_file`] or, where the original stops the buffer first, [`Audio::stop_and_play_file`] |
//! | **The two sample banks** | `Sound_PlaySlot`, `Sound_RestartSlot`, `FUN_004262CF` | 49 | **27** — the march, the sites, the village's work, the click, and fifteen on the battlefield |
//! | **Troop cries** | `Sound_PlayTroopCry` `0x00499CB1` | 6 | **6** — [`TroopCries`] |
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
//!
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
//!
//!   `S010_13.wav` is `g_msgVoiceS010`'s, which `FUN_004B36C0` reads. The
//!   letter's sting, `FUN_004B3B92(lord - 1)`, is `S246_02.wav + lord * 0x10`
//!   over four clips and a sentinel.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    ///: `setup.wav` **is** the music, played
    FrontEnd,
    Campaign { county_count: u8, share_of_map_pct: i32 },
    /// **The conquest interstitial, screen `0x1C`** — the one screen over a
    /// running game that is not campaign music. `Screen_DrawConquest`
    /// (`0x0041E1DD`) opens with
    /// `Music_Play(g_campaignMap < 8 ? "setup.wav" : "setup2.wav", 0,
    /// g_campaignMap < 8)`, so `ended` — the campaign past its eighth map,
    /// [`crate::victory::Campaign::is_complete`] — picks both the file **and**
    /// the loop flag.
    Conquest { ended: bool },
    Battle(BattleKind),
    /// **A film is up.** Every one of `Smk_Play`'s
    /// callers stops the music first — `Music_Stop(0)` in `CastleBuild_Confirm`,
    /// in both animated `Msg_DrawWindow` branches, in `Battle_CheckOutcome`, in
    /// `FUN_00432B05`; and `App_WinMain` stops the `setup.wav` it has just
    /// started before the intro opens. The film plays its own track instead.
    Film { over_battle: bool },
}

pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    stream: Option<cpal::Stream>,
    files: BTreeMap<String, PathBuf>,
    films: BTreeMap<String, PathBuf>,
    cache: BTreeMap<String, Option<Arc<Sound>>>,
    pub(super) battle: BattleCycle,
    scene: Option<Scene>,
    options: Options,
    decodes: bool,
    /// time it was typed it was wrong: `battle5.wav` is in the table, ships,
    /// and is **unreachable**, because the counter that selects it is
    /// `DAT_0057A0F0` — the third battle mode [`BattleKind`] deliberately does
    /// not name. A set collected by driving beats a set assembled by reading.
    heard: std::collections::BTreeSet<String>,
    /// **What the one-shot buffer holds** — `DAT_00522AEC`, the single
    /// DirectSound buffer `Sound_PlayFile` (`0x00427990`) builds every file
    /// into. The original has one, so "is the one-shot busy" is a question about
    /// whatever was put there last; ours are many voices, so the **handle** of
    /// the voice [`Audio::play_file`] started is kept and the mixer is asked
    /// about that. A name would not do: [`crate::audio::Mixer::play_effect`]
    /// replaces a voice of the same file and the voice cap evicts the oldest,
    /// either of which makes a name answer for a clip the buffer does not hold.
    ///
    /// The buffer is the mixer's own voice, as `DAT_00522AEC` is the original's
    /// own buffer — [`crate::audio::Mixer::play_one_shot`].
    ///
    /// `Sound_OneShotBusy` (`0x00427C9B`) and
    /// `Sound_PlayFile`'s drop ask the same buffer, so [`Audio::play_file`] is
    /// the only thing that sets it — [`Audio::stop_and_play_file`] through it —
    /// [`Audio::stop_one_shot`] is the only thing that clears it, and
    /// [`Audio::one_shot_busy`] is the only thing that reads it.
    one_shot: Option<u64>,
    /// Whether the last scene [`Audio::follow`] was given was the battlefield,
    /// so that `Music_StartBattle` (`0x00477B2F`) fires on the way in and not
    /// once a frame. Films over the battle keep it set.
    in_battle: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(a.cache.is_empty());
    }

    #[test]
    fn the_front_end_is_silent_and_the_campaign_is_not() {
        let mut a = Audio::silent();
        a.follow(Scene::FrontEnd);
        assert_eq!(a.scene, Some(Scene::FrontEnd));
        a.follow(Scene::Campaign { county_count: 4, share_of_map_pct: 30 });
        assert_eq!(a.scene, Some(Scene::Campaign { county_count: 4, share_of_map_pct: 30 }));
    }

    #[test]
    fn staying_in_a_battle_does_not_step_the_counter() {
        let mut a = Audio::silent();
        a.follow(Scene::Battle(BattleKind::Field));
        let after_one = a.battle;
        for _ in 0..100 {
            a.follow(Scene::Battle(BattleKind::Field));
        }
        assert_eq!(a.battle, after_one, "the counter moved during one battle");
        a.follow(Scene::Campaign { county_count: 3, share_of_map_pct: 20 });
        a.follow(Scene::Battle(BattleKind::Field));
        assert_ne!(a.battle, after_one);
    }

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

