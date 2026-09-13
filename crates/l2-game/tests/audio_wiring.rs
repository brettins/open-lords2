//! **Is anything driving the audio layer?**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_wiring
//! ```
//!
//! `tests/audio_install.rs` checks that the *tables* name files that ship and
//! that the *mixer* turns a file into samples. Both were green, both still are,
//! and **no music has ever played**: `audio::scene` asked whether the screen at
//! the bottom of the stack is a setup page, and the front end is *pushed
//! under* the campaign, so it answered `FrontEnd` 
//! for every state a running game can be in. `docs/decisions.md` C116.
//!
//! The test that covered it did this:
//!
//! ```ignore
//! let machine = Machine::new(ScreenId::Campaign);
//! assert_eq!(scene(&machine, &game), Scene::Campaign { .. });
//! ```
//!
//! — a machine with one screen on it, which the application cannot produce.
//! The assertion was right, the subject was a fixture, and that is
//! `docs/agents.md`'s *a test that drives the picture from the wrong field*
//! with a stack instead of a field.
//!
//! So everything here starts from **the root `main.rs` builds** and reaches
//! every other state through [`Machine::handle`] with real events. Nothing
//! constructs a stack.

use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

/// **The screen `main.rs` roots the machine on.** If this stops being what the
/// application starts from, every test below is testing a state nobody reaches
/// — which is the whole defect this file exists for — so it is named once,
/// here, and `the_root_is_the_one_the_application_starts_on` checks it against
/// `main.rs` itself.
const APP_ROOT: ScreenId = ScreenId::Setup(SetupPage::Title);

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

/// A world with a realm holding one county of fourteen, which is what the
/// England start is and what `Scroll1` is the answer to.
fn world() -> Game {
    let mut g = Game::new(5);
    // **Tip screens: No.** A tip posted twenty frames in queues ahead of the
    // messages these tests post and speaks first, which is right; the tips'
    // own narration is asserted in `tests/tips.rs`.
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(14);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].county_count = 1;
    g.player = 1;
    g
}

/// **`main.rs` and `APP_ROOT` must agree**, and they are maintained by
/// different work — `docs/agents.md`'s one reliable shape. Reading the source
/// is crude and it is the only artefact that cannot be fooled: a binary's
/// constants are not importable from an integration test.
#[test]
fn the_root_is_the_one_the_application_starts_on() {
    let src = include_str!("../src/main.rs");
    assert!(
        src.contains("Machine::new(ScreenId::Setup(SetupPage::Title))"),
        "main.rs no longer starts on {APP_ROOT:?}; every test in this file is now \
         asserting about a state the application does not reach. Update APP_ROOT \
         and re-read audio::scene's front-end arm."
    );
}

/// **And that the application still calls the thing these tests exercise.**
///
/// Everything below runs [`audio::Director::listen`].
/// about the game if the game runs it too, and the game is a binary an
/// integration test cannot link against — so the two are held together by the
/// one artefact both sides share, which is the source. Crude, and it is the
/// difference between testing the feature and testing a library nobody calls:
/// C27, nine instances, and this was the tenth.
#[test]
fn the_application_calls_the_director() {
    let src = include_str!("../src/main.rs");
    assert!(
        src.contains("self.director.listen(&mut self.audio, &self.machine, &self.game)"),
        "main.rs no longer calls Director::listen, so nothing drives the audio layer \
         and every test in this file passes anyway"
    );
    assert!(
        src.contains("self.listen();"),
        "App::tick no longer calls App::listen, so the director is never asked"
    );
}

/// **The bug, in the smallest form that reproduces it.**
///
/// Not a constructed stack: the root the application starts on, with a screen
/// put on it the way [`Machine::apply_at`] puts one on for
/// `Transition::Push` — which is what `SetupScreen`'s Start button returns.
///
/// Ablation: restore `if let Some(ScreenId::Setup(_)) = machine.ids().first()`
/// and this goes red on every one of the twenty-odd screens.
#[test]
fn every_in_game_screen_over_the_front_end_is_campaign_music() {
    let game = world();
    // The front end alone is silent, which is the half that was always right.
    let root = Machine::new(APP_ROOT);
    assert_eq!(audio::scene(&root, &game), Scene::FrontEnd);

    // And every screen the game can open over it is not. The list is the
    // screens reachable in a running game; the battlefield is excluded because
    // it is the *other* answer and has its own test below.
    let in_game = [
        ScreenId::Campaign,
        ScreenId::County(1, l2_game::screens::county::Panel::Tax),
        ScreenId::Village(1),
        ScreenId::Court,
        ScreenId::Diplomacy,
        ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Save),
        ScreenId::Options(l2_game::screens::options::Page::Sound),
        ScreenId::BattlePrompt,
    ];
    for id in in_game {
        let mut m = Machine::new(APP_ROOT);
        m.push(id);
        assert!(
            matches!(audio::scene(&m, &game), Scene::Campaign { .. }),
            "{id:?} over the front end answered silence — the front end is PUSHED \
             under the game, so the bottom of the stack is a setup page for the \
             whole session"
        );
    }

    // **The conquest interstitial is the exception and used to be in the list
    // above.** It is over a running game, so it is not silence — but
// `Screen_DrawConquest` (`0x0041E1DD`) plays its own bed
    // `Music_StartCampaign`'s, so asserting campaign music here was asserting
    // the thing the screen does not do. Which bed is
    // `the_conquest_interstitial_plays_its_own_bed`'s subject.
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Conquest);
    assert!(
        matches!(audio::scene(&m, &game), Scene::Conquest { .. }),
        "the interstitial is neither silent nor the campaign's music"
    );
}

/// **The interstitial's own bed, and the only unlooped music in the game.**
///
/// `Screen_DrawConquest` (`0x0041E1DD`) opens with
/// `Music_Play(g_campaignMap < 8 ? "setup.wav" : "setup2.wav", 0,
/// g_campaignMap < 8)`, so one test covers the file and the loop flag: the
/// campaign in flight gets the front end's looping bed back, and the campaign
/// past its eighth map gets `setup2.wav` once.
///
/// **Ablated**: `Music::Setup2` for both arms, or `Mixer::set_music` for both
/// loop flags, each goes red on one half.
#[test]
fn the_conquest_interstitial_plays_its_own_bed() {
    use l2_game::audio::track::Music;

    let mut game = world();
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Conquest);

    assert!(!game.campaign.is_complete(), "a new campaign is on its first map");
    assert_eq!(audio::scene(&m, &game), Scene::Conquest { ended: false });
    assert_eq!(Music::Setup.file(), "setup.wav");
    assert!(Music::Setup.loops(), "`Music_Play(…, 0, 1)` below the eighth map");

    // The eighth map won: `g_campaignMap` is 8 and the screen says so.
    game.campaign.map = l2_game::victory::CAMPAIGN_LENGTH;
    assert_eq!(audio::scene(&m, &game), Scene::Conquest { ended: true });
    assert_eq!(Music::Setup2.file(), "setup2.wav");
    assert!(!Music::Setup2.loops(), "`Music_Play(…, 0, 0)` above it");
}

/// The front end's own pages stay silent however deep the stack gets, which is
/// the clause the fix must not break: `SetupPage::Load` is the title's load
/// screen and is not `ScreenId::SaveLoad`.
#[test]
fn the_front_end_is_its_own_scene_on_all_thirteen_of_its_pages() {
    let game = world();
    for page in SetupPage::ALL {
        let m = Machine::new(ScreenId::Setup(page));
        assert_eq!(audio::scene(&m, &game), Scene::FrontEnd, "{page:?}");
    }
    // And the index, which is ours and is pushed from the title by `I`.
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Index);
    assert_eq!(audio::scene(&m, &game), Scene::FrontEnd, "the index over the title");
    // Over a running game the same screen is not the front end.
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Campaign);
    m.push(ScreenId::Index);
    assert!(matches!(audio::scene(&m, &game), Scene::Campaign { .. }));
}

/// **`g_battlePhase == 2` outlives the battlefield's three screen ids**, so the
/// scene is read off the whole stack: a panel over the
/// field does not stop the battle music.
#[test]
fn the_battlefield_is_battle_music_under_whatever_is_over_it() {
    let game = world();
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Campaign);
    assert!(matches!(audio::scene(&m, &game), Scene::Campaign { .. }));
    m.push(ScreenId::Battlefield);
    assert!(
        matches!(audio::scene(&m, &game), Scene::Battle(_)),
        "Battle_Start opens with Sound_LoadBattleBank(); Music_StartBattle();"
    );
    m.push(ScreenId::Info(l2_game::screens::info::Target::Tile(0)));
    assert!(matches!(audio::scene(&m, &game), Scene::Battle(_)), "a panel over the field");
}

/// **The whole chain, from the title screen to samples**, with no device.
///
/// This is the test the feature did not have. It presses the front end's
/// buttons, lets the machine apply the transitions, derives the scene the way
/// `App::listen` derives it, and then asks the mixer for the buffer the sound
/// card would have been handed. Every link is the real one except the sound
/// card, which is the one link that cannot be in a test.
///
/// Ablate any of them and it goes red: `scene`'s front-end arm, `follow`'s
/// campaign arm, `play_music`, `Mixer::set_music`, `Mixer::fill`.
#[test]
fn pressing_start_on_the_title_screen_makes_a_noise() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no artwork for the front end and no .wav files");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut game = world();

    let mut machine = Machine::new(APP_ROOT);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    assert!(audio.file_count() > 700, "only {} wav files indexed", audio.file_count());

    // One tick of the event loop, as `App::tick` runs it: update, then listen.
    // `Director::listen` is the function the game calls, not a copy of it.
    macro_rules! tick {
        () => {{
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }};
    }

    tick!();
    // **The title screen is not silent, and this line used to say it was.**
    // `Music_Play` (`0x004263AD`) is a ninth sound primitive that
    // `docs/audio-triggers.md`'s eight did not enumerate, and all nine of its
    // call sites are the front end playing `setup.wav`. See
    // `tests/audio_screens.rs`.
    assert_eq!(audio.music_name().as_deref(), Some("setup.wav"), "the front end's own bed");

    // *Single player* — page 1 item 0, the first hotspot, so the selection is
    // already on it. Then *Custom game* — page 2 item 3.
    send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(machine.top_id(), Some(ScreenId::Setup(SetupPage::Options)));
    for _ in 0..3 {
        send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Down));
    }
    send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(machine.top_id(), Some(ScreenId::Setup(SetupPage::Custom)));

    tick!();
    assert_eq!(
        audio.music_name().as_deref(),
        Some("setup.wav"),
        "still the front end, three pages in — and still one bed, not restarted"
    );

    // *Start* — the third caption at y = 0xC6, the same coordinates
    // `tests/setup.rs` presses.
    send(&mut machine, &mut game, &assets, Event::Click { x: 0xF3 + 20, y: 0xC6 });
    assert!(
        machine.ids().contains(&ScreenId::Campaign),
        "Start did not raise the campaign map: {:?}",
        machine.ids()
    );

    tick!();
    assert_eq!(
        audio.music_name().as_deref(),
        Some("scroll1.wav"),
        "Game_NewGame ends with Music_StartCampaign, and one county is Scroll1 \
         whatever share that is"
    );

    // And it is audible. A tenth of a second of what the device callback gets.
    let mut buf = vec![0f32; 4_410 * 2];
    audio.mix(&mut buf);
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "scroll1 is loaded and the mixer produced silence (peak {peak})");
    assert!(buf.iter().all(|s| s.is_finite() && (-1.0..=1.0).contains(s)), "out of range");

    // The music does not restart while the answer has not changed — the
    // property that lets `listen` run sixty times a second.
    let before = audio.music_name();
    for _ in 0..120 {
        tick!();
    }
    assert_eq!(audio.music_name(), before, "the track restarted under a steady scene");
}

/// **The narrator speaks, and he speaks once.**
///
/// A player: *"that guy's voice acting is half the personality of the game."*
/// He is right about the proportion — **646 of the install's 771 files are
/// somebody talking**, 449 lord takes and 197 system clips.
///
/// `Msg_DrawWindow` (`0x0047309E`) dismisses, enqueues,
/// sets its own timer and plays its own sound from inside the draw, so there is
/// no call site to hang a voice on. The trigger is the message timer reaching a
/// value, and this drives the **real queue through the real pump** — enqueue,
/// `Machine::update`, which is where `pump_messages` lives — and counts what
/// came out.
#[test]
fn a_message_window_speaks_ten_ticks_after_it_opens() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clips to speak");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    // Group 130 is a plain notice with a system clip, `S130_01.wav`. Category
    // `0x03` takes the ten-tick schedule.
    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::COUNTY_NOTICE;
    rec.to = game.player;
    assert!(game.messages.enqueue(rec, game.player), "the record was accepted");

    // The tick the *voice* landed on, by name. Counting `heard()` wholesale
    // would count `scroll1.wav` too — the music starts on the first `listen`,
    // which is correct and is not what this test is about.
    let mut spoke_at = Vec::new();
    let mut was_heard = false;
    for _ in 0..40 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        let timer = game.messages.timer();
        director.listen(&mut audio, &machine, &game);
        let now = audio.heard().contains(&"s130_01.wav");
        if now && !was_heard {
            spoke_at.push(timer);
        }
        was_heard = now;
    }
    assert!(
        machine.ids().contains(&ScreenId::Message),
        "the pump never opened a window: {:?}",
        machine.ids()
    );
    assert_eq!(
        spoke_at,
        vec![0x7C6],
        "the voice should land exactly once, on the tick Msg_DrawWindow tests"
    );
    assert!(audio.heard().contains(&"s130_01.wav"), "heard {:?}", audio.heard());
}

/// **The Speech switch has to silence him, and only him.**
///
/// `Sound_PlayFile(name, 1, 0)` — the `1` is what gates every voice line on
/// `g_optSpeech`, so the Sounds page's third
/// row is a separate switch from its second. A narrator that ignored it would
/// be a poor first impression of the feature.
#[test]
fn the_speech_switch_silences_the_narrator_and_leaves_the_music_alone() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut game = world();
    // `Opt_ToggleSpeech` (`0x00434A9A`), the Sounds page's third row.
    game.prefs.speech = false;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::COUNTY_NOTICE;
    rec.to = game.player;
    game.messages.enqueue(rec, game.player);
    for _ in 0..40 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
    }
    assert!(
        !audio.heard().contains(&"s130_01.wav"),
        "Speech: Off did not silence the narrator - heard {:?}",
        audio.heard()
    );
    // And the music is on a different switch, so it is still playing.
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"));

    // Turn it back on and the next window speaks.
    game.prefs.speech = true;
    let mut rec2 = message::Record::default();
    rec2.group = 131;
    rec2.category = message::category::COUNTY_NOTICE;
    rec2.to = game.player;
    game.messages.enqueue(rec2, game.player);
    message::dismiss(&mut game);
    for _ in 0..40 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
    }
    assert!(audio.heard().contains(&"s131_01.wav"), "heard {:?}", audio.heard());
}

/// **A letter's fanfare takes the one-shot buffer, and the lord cuts it off.**
///
/// `Msg_DrawWindow` opens a letter with `Sound_PlayFile("ff_msg.wav", 1, 0)` —
/// nothing in front of it, and the *speech* flag — and speaks 200 ticks later
/// through `Msg_PlayVoice`, which puts `Sound_StopOneShot()` in front of its own
/// `Sound_PlayFile`. So the fanfare is the buffer's occupant, the lord talks
/// over a trumpet that is still going by stopping it, and the switch that
/// silences the trumpet is Speech, not Sound Effects. `[V]`
///
/// Nothing is mixed, so the fanfare is still sounding when the lord is asked
/// for — the case that tells the two verbs apart.
///
/// Ablations: speak through `play_file` and
/// the lord is dropped; play the fanfare with `false` and Speech: Off no
/// longer silences it.
#[test]
fn a_letters_fanfare_holds_the_buffer_and_the_lord_cuts_it_off() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let fanfare = l2_game::audio::names::fanfare::MESSAGE;
    let lord = l2_game::audio::names::message_voice(170, 0).expect("group 170 has a voice");
    let letter = |game: &Game| {
        let mut rec = message::Record::default();
        rec.group = 170;
        rec.category = message::category::LETTER;
        rec.to = game.player;
        rec
    };

    let mut game = world();
    game.prefs.tip_screens = false;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    assert!(game.messages.enqueue(letter(&game), game.player), "the letter was accepted");
    let mut held = None;
    for _ in 0..240 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
        if held.is_none() && audio.heard().contains(&fanfare) {
            held = Some(audio.is_playing(fanfare) && audio.one_shot_busy());
        }
        if audio.is_playing(&lord) {
            break;
        }
    }
    assert_eq!(held, Some(true), "the fanfare did not take the buffer - heard {:?}", audio.heard());
    assert!(audio.is_playing(&lord), "the lord was dropped over his fanfare - heard {:?}", audio.heard());
    assert!(!audio.is_playing(fanfare), "and the fanfare was cut off rather than left under him");

    // Speech: Off, Sound Effects: On — and the trumpet is silent.
    let mut game = world();
    game.prefs.tip_screens = false;
    game.prefs.speech = false;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    game.messages.enqueue(letter(&game), game.player);
    for _ in 0..20 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
    }
    assert!(machine.ids().contains(&ScreenId::Message), "the letter never opened");
    assert!(!audio.heard().contains(&fanfare), "Speech: Off left the letter's fanfare - heard {:?}", audio.heard());
}

/// **The ten industry lines a player asked for by name.**
///
/// `Industry_ToggleFromMap` (`0x0043D309`) ends with
/// `Msg_Enqueue(0, g_localPlayer, local_10 + 0xE6, …)`, `local_10` being
/// `industry * 2 + on` for the four industries and `-1`/`-2` for the castle
/// switch. **We supply the voice; the enqueue is the industry branch's.** So
/// this asserts the half that is ours — that every one of those ten groups
/// resolves to a clip that ships — and it will start speaking the moment the
/// message arrives, with no further change here.
#[test]
fn the_industry_toggle_groups_all_have_a_voice_that_ships() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);
    // 228/229 are the castle switch, 230..237 the four industries off and on.
    let labels = [
        "Building off", "Building on", "Forestry off", "Forestry on",
        "Mining off", "Mining on", "Blacksmith off", "Blacksmith on",
        "Quarrying off", "Quarrying on",
    ];
    for (i, label) in labels.iter().enumerate() {
        let group = 228 + i as u16;
        let name = l2_game::audio::names::message_voice(group, 0)
            .unwrap_or_else(|| panic!("group {group} ({label}) has no voice"));
        assert_eq!(name, format!("S{group}_01.wav"));
        // `Msg_PlayVoice`'s own verb, which stops the buffer first and so never
        // drops the next group's clip over this one's.
        audio.stop_and_play_file(&name, true);
        assert!(
            audio.heard().contains(&name.to_ascii_lowercase().as_str()),
            "group {group} ({label}) -> {name} did not decode"
        );
    }
}

/// **How much of the game's audio the engine can play, measured.**
///
/// 771 files ship. The number that matters is how many of them any code path
/// can reach, and it was **0** until the fix in `docs/decisions.md`
/// C116. Nobody had ever quoted it, and the first time it *was* quoted
/// — in three documents, by typing — it was wrong: `battle5.wav` is in the
/// table, ships, decodes, and **cannot be reached**, because the counter that
/// selects it is `DAT_0057A0F0`, the third battle mode `BattleKind` does not
/// name.
///
/// So the set is collected by *driving every scene the policy can produce*
/// through the real `follow` and reading `Audio::heard` — what travelled the
/// road, which is `docs/agents.md`'s standard for a field being tested at all.
/// A new call site makes this go red with the name it added, which is the only
/// way this count stays true.
#[test]
fn the_voice_class_is_84_percent_of_the_games_audio() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);

    // Every name `Msg_PlayVoice` can produce, asked for through its own verb,
    // `stop_and_play_file` — `Sound_StopOneShot(); Sound_PlayFile(name, 1, 0)`,
    // which never drops, so nothing here needs mixing. The three bands are the original's tables: 170..=197 have
    // a lord and sixteen takes, 100..=169 and 200..=284 have one clip each.
    //
    // **Except the tip groups, 200..=218**, which this count used to include
    // and which no player of this engine can hear. Their windows are
    // categories 0x05..=0x09, and `Tip_Show` (`0x00476DA9`) is the only
    // function in the original that posts one; nothing here posts a tip. A
    // name-driven loop proves the *name* resolves, not that the game can ask
    // for it, and thirteen tip clips were sitting in the 543 on that basis.
    // `docs/audio.json` `Msg_DrawWindow#24` and `FUN_004b3acd#1`.
    //
    // **Not 219**, though `FUN_00476A5D` clears twenty tip flags from 200:
    // `L2.eng` 219 is *"Already in alliance."*, a refusal `Diplo_SendClicked`
    // posts, and its clip ships. The first draft of this range swallowed it and
    // the count came out one short — which is the check working.
    //
    // What this does not establish: that every *other* group in the bands is
    // posted by this engine. Only the tip band has been checked.
    const TIP_GROUPS: std::ops::RangeInclusive<u16> = 200..=218;
    for group in (100..=300u16).filter(|g| !TIP_GROUPS.contains(g)) {
        for variant in 0..16u8 {
            if let Some(name) = l2_game::audio::names::message_voice(group, variant) {
                audio.stop_and_play_file(&name, true);
            }
        }
    }
    let spoken = audio.heard().len();
    assert_eq!(
        spoken, 530,
        "the narrator's reachable performance moved: {spoken} clips"
    );
    // 448 lord takes and 82 system clips. The system bands outside the tips
    // cover 135 groups and only 82 of them ship an `_01`.
    // the tables - a group with no clip is a message the narrator does not read.
    assert_eq!(audio.heard().iter().filter(|n| n.starts_with('s')).count(), 82);
    assert_eq!(spoken - 82, 448);

    // **What is still out of reach in this class**, so the number is not read
    // as "the voice is done": the tip screens, first line and chain alike.
    // `FUN_004B3ACD(group)` walks a five-wide table at `0x004E1E40` whose only
    // live rows are groups 200..=218, playing `S201_02.wav + (n - 1) * 0x10`
    // one clip at a time as each finishes. This used to name `S010_13.wav` as
    // the chain's evidence, and that clip is not in the chain at all: it is
    // `g_msgVoiceS010`'s, a sibling table with a different caller.
    for clip in ["s200_01.wav", "s201_01.wav", "s201_02.wav", "s218_03.wav"] {
        assert!(!audio.heard().contains(&clip), "{clip} is a tip clip and no tip is posted");
    }
}

#[test]
fn the_music_fanfares_screens_and_the_click_are_fifty_five_more() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);
    assert_eq!(audio.file_count(), 771, "the install's sound count moved");

    // The front end's own bed, which is `Music_Play`'s and not either picker's.
    audio.follow(Scene::FrontEnd);
    // Every rung of `Music_StartCampaign`'s ladder, both clauses.
    for (counties, share) in [(1, 7), (2, 7), (2, 8), (2, 15), (2, 29), (2, 43)] {
        audio.follow(Scene::Campaign { county_count: counties, share_of_map_pct: share });
    }
    // Both battle pairs, twice each, so both sides of both toggles are taken.
    for _ in 0..2 {
        for kind in [
            l2_game::audio::track::BattleKind::Field,
            l2_game::audio::track::BattleKind::Siege,
        ] {
            audio.follow(Scene::Campaign { county_count: 1, share_of_map_pct: 7 });
            audio.follow(Scene::Battle(kind));
        }
    }
    // And the two fanfares `Director::listen` fires, by the same names it uses.
    audio.play_effect(l2_game::audio::names::fanfare::MESSAGE);
    audio.play_effect(l2_game::audio::names::fanfare::BATTLE);
    // `ff_capt.wav`, which `Msg_DrawWindow` plays for the conquest band and
    // which had no caller until the message window arrived.
    audio.play_effect(l2_game::audio::names::fanfare::CAPTURED);

    // **The screen class**, which `tests/audio_screens.rs` drives through the
    // machine and which is asked for here by name, so that this one assertion
    // stays the whole count of what the engine can reach outside the narrator.
    // Six bank slots from `g_jobSound`, four from `TileInfo_Draw`'s ladder —
    // which overlap, because a mine sounds the same in the village and on the
    // map — the forge, and five spoken lines.
    for job in 1..=9usize {
        if let Some(n) = l2_game::audio::names::slot(
            l2_game::audio::names::Bank::Kingdom,
            l2_game::audio::names::JOB_SOUND[job],
        ) {
            audio.play_effect(n);
        }
    }
    audio.play_effect(l2_game::audio::names::blacksmith::FIRE);
    for graphic in 0..13u8 {
        if let Some(n) = l2_game::audio::names::resource_site_slot(graphic)
            .and_then(|s| l2_game::audio::names::slot(l2_game::audio::names::Bank::Kingdom, s))
        {
            audio.play_effect(n);
        }
    }
    for line in [
        l2_game::audio::names::speech::RATION_NOT_MET,
        l2_game::audio::names::speech::RATION_ON_DAIRY,
        l2_game::audio::names::speech::SUPPLIES,
        l2_game::audio::names::speech::ZOOM_OUT,
        l2_game::audio::names::speech::SPLIT_ARMY,
        l2_game::audio::names::speech::CHOOSE_YOUR_SHIELD,
    ] {
        // `play_effect`, not the director's `play_file`: nothing here mixes, so
        // the dropping verb would open the first line and drop the other five.
        audio.play_effect(line);
    }
    // **The four screen-voice tables**, which are the sibling-thunk band:
    // `FUN_004B3714`'s mercenary offer, `FUN_004B3768`'s health line and
    // `FUN_004B37BC`'s picked unit or castle.
    //
    // **Twenty-five distinct files, and the health line contributes four.**
    // `healthBand` is `0 ..= 4`, entries 3 and 4 of the table are both
    // `S020_04.wav`, and so **`S020_05.wav` ships, is named in the binary, and
    // nothing in a running game can ask for it** — `battle5.wav`'s situation
// exactly, and found the same way: by counting distinct files
    // table entries.
    for line in l2_game::audio::names::speech::MERCENARY_OFFER
        .iter()
        .take(l2_kingdom::mercenary::ROSTER.len() - 1)
        .chain(l2_game::audio::names::speech::POPULATION_HEALTH.iter().take(5))
        .chain(l2_game::audio::names::speech::PICKED_UNIT.iter())
        .chain(l2_game::audio::names::speech::PICKED_CASTLE.iter())
    {
        audio.play_effect(line);
    }
    // The four movement sounds `Director::hear_the_march` asks for.
    for slot in [5usize, 11, 12] {
        if let Some(n) =
            l2_game::audio::names::slot(l2_game::audio::names::Bank::Kingdom, slot)
        {
            audio.play_effect_if_idle(n);
        }
    }
    // **The pointer click** — `Widget_Test`'s `Sound_RestartSlot(1)`, the slot
    // `Director::hear_the_click` asks for. `tests/click.rs` is when it sounds.
    if let Some(n) = l2_game::audio::names::slot(l2_game::audio::names::Bank::Kingdom, 1) {
        audio.play_effect(n);
    }

    assert_eq!(
        audio.heard(),
        [
            "army.wav",
            "battle1.wav",
            "battle2.wav",
            "battle3.wav",
            "battle4.wav",
            "click3.wav",
            "fallow.wav",
            "ff_batl.wav",
            "ff_capt.wav",
            "ff_msg.wav",
            "fire.wav",
            "iron.wav",
            "merchant.wav",
            "moo_2.wav",
            "rioters.wav",
            "s011_02.wav",
            "s016_01.wav",
            "s016_02.wav",
            "s016_03.wav",
            "s016_04.wav",
            "s016_05.wav",
            "s016_06.wav",
            "s016_07.wav",
            "s016_08.wav",
            "s016_09.wav",
            "s016_10.wav",
            "s016_11.wav",
            "s016_12.wav",
            "s017_01.wav",
            "s020_01.wav",
            "s020_02.wav",
            "s020_03.wav",
            "s020_04.wav",
            "s021_01.wav",
            "s021_02.wav",
            "s031_01.wav",
            "s031_02.wav",
            "s031_03.wav",
            "s031_04.wav",
            "s033_01.wav",
            "s033_02.wav",
            "s071_02.wav",
            "s071_03.wav",
            "s071_04.wav",
            "s071_05.wav",
            "s071_06.wav",
            "scroll1.wav",
            "scroll2.wav",
            "scroll3.wav",
            "scroll4.wav",
            "scroll5.wav",
            "setup.wav",
            "stonecut.wav",
            "wheat.wav",
            "woodcut.wav",
        ],
        "the set of sounds this engine can reach has changed. If a call site was \
         ADDED this is good news and the number in crates/l2-game/src/audio/mod.rs, \
         docs/mechanics.md and docs/decisions.md C116 moves with it."
    );
    assert_eq!(audio.heard().len(), 55, "55 of 771 outside the voice class");

    // `battle5.wav` ships and decodes; nothing can ask for it.
    // gap in the wiring, it is `DAT_0057A0F0` being unidentified, and
    // `BattleKind` refusing to guess at a third mode is why.
    assert!(!audio.heard().contains(&"battle5.wav"));
}

/// **The Sounds page had no effect on anything audible.** `screens::options`
/// writes `game.prefs`; `Audio` kept a second copy of the same three flags that
/// nothing ever wrote. `App::listen` pushes one into the other, and this is
/// that push, driven the way the event loop drives it.
#[test]
fn turning_music_off_on_the_sounds_page_stops_the_music() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no .wav files to stop playing");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    macro_rules! listen {
        () => {
            director.listen(&mut audio, &machine, &game)
        };
    }

    listen!();
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"));

    // `Opt_ToggleMusic` (`0x004349A4`) — the Sounds page's first row.
    game.prefs.music = false;
    listen!();
    assert_eq!(audio.music_name(), None, "Music: Off left the track playing");
    let mut buf = vec![0f32; 512];
    audio.mix(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0), "and the mixer is still producing samples");

// And back on, which the original re-derives.
    game.prefs.music = true;
    listen!();
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"), "Music: On did not resume");
}

/// **The pointer click reaches a speaker, once per press — and not from a
/// hotspot.**
///
/// `tests/click.rs` asserts when [`Machine::clicks`] moves; this asserts that
/// [`audio::Director::listen`] turns the movement into `click3.wav` and nothing
/// else into it. `Widget_Test` (`0x0040DA1E`) is the only function in the game
/// whose click sound is live — the other two sites are dead code
/// (`docs/audio.json`) — and it sounds on the **initial press** of a kind-4 or
/// kind-5 widget only.
///
/// The held half is the one worth having: the buffer is drained with
/// [`Audio::mix`] until the click has finished, and then the arrow is held for
/// two seconds of ticks. A click on any repeat pulse would put `click3.wav`
/// back in the mixer, and `is_playing` would see it on that very tick.
///
/// **Ablations, run:** delete the `self.hear_the_click(..)` call in
/// `Director::listen` and the loud assertion goes red; make `hear_the_click`
/// play on `now != 0` and the held assertion does.
#[test]
fn a_widget_press_is_heard_once_and_a_hotspot_press_is_not() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no click3.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    // **The sidebar, which is `Hotspot_Test` kind 1.** Each button on a fresh
    // machine, so that every one of them is pressed from the map.
    let mut opened = 0;
    for b in l2_game::screens::map::SIDEBAR_BUTTONS {
        let mut game = world();
        game.selected = 1;
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        director.listen(&mut audio, &machine, &game);
        let r = b.rect();
        send(&mut machine, &mut game, &assets, Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 });
        if machine.top_id() != Some(ScreenId::Campaign) {
            opened += 1;
        }
        director.listen(&mut audio, &machine, &game);
        assert!(!audio.heard().contains(&"click3.wav"), "{} is a hotspot and clicked", b.name);
    }
    assert!(opened >= 1, "no sidebar button opened anything, so the silence proves nothing");

    // **The tax arrow, which is `Widget_Test` kind 4.**
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    machine.push(ScreenId::County(1, l2_game::screens::county::Panel::Tax));
    macro_rules! tick {
        () => {{
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }};
    }
    tick!();
    let up = l2_game::screens::county::Panel::Tax.increase_button().expect("an up arrow");
    send(&mut machine, &mut game, &assets, Event::Click { x: up.x + up.w / 2, y: up.y + up.h / 2 });
    tick!();
    assert!(audio.heard().contains(&"click3.wav"), "the press was silent; heard {:?}", audio.heard());
    assert!(audio.is_playing("click3.wav"), "and it is sounding now");

    // Let it finish. A second of samples at a time, and no more than ten.
    let mut buf = vec![0f32; 44_100 * 2];
    for _ in 0..10 {
        if !audio.is_playing("click3.wav") {
            break;
        }
        audio.mix(&mut buf);
    }
    assert!(!audio.is_playing("click3.wav"), "click3.wav never finished");

    // Hold for two seconds of ticks.
    let mut steps = 0;
    for t in 0..125 {
        let before = game.kingdom.counties[1].tax_rate;
        tick!();
        if game.kingdom.counties[1].tax_rate != before {
            steps += 1;
        }
        assert!(
            !audio.is_playing("click3.wav"),
            "tick {t} of the hold put the click back in the mixer; the original plays it on the press only"
        );
    }
    assert!(steps >= 4, "the hold must have repeated for its silence to mean anything: {steps}");
    send(&mut machine, &mut game, &assets, Event::Release { x: up.x + up.w / 2, y: up.y + up.h / 2 });
    tick!();
    assert!(!audio.is_playing("click3.wav"), "letting go of the arrow clicked");
}

/// **The field brush's three sounds.** `FUN_00438B02` (`0x00438B02`) answers
/// every one of the five buttons and picks the slot off the terrain it is
/// about to paint: `0x13` → 4 `moo_2.wav`, `2` → 7 `wheat.wav`, and `1`, `0`
/// and `0x19` → 6 `fallow.wav`.
///
/// Driven through the panel's own handler at the pixels the hotspot table
/// gives, with [`audio::Director`] between the two ticks — the brush closes
/// the panel (`g_screenId = 0`), so the tile it painted is gone from the stack
/// by the tick that hears it.
///
/// **Ablation, run:** delete the `hear_the_brush` call in `Director::listen`
/// and all four arms go red.
#[test]
fn the_field_brush_sounds_what_it_paints() {
    use l2_game::screens::info::{Target, BRUSH_DIM, BRUSH_FIELD_X, BRUSH_ROW_Y, BRUSH_WASTE_X};
    use l2_kingdom::field::terrain;
    use l2_kingdom::map::flags;
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no fallow.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    // brush x, the terrain the tile starts on, and what the original plays.
    let arms: [(i32, bool, u8, &str); 4] = [
        (BRUSH_FIELD_X[1], true, terrain::FALLOW, "wheat.wav"),
        (BRUSH_FIELD_X[2], true, terrain::FALLOW, "moo_2.wav"),
        (BRUSH_FIELD_X[0], true, terrain::GRAIN, "fallow.wav"),
        (BRUSH_WASTE_X[0], false, terrain::WASTE, "fallow.wav"),
    ];
    for (bx, field_menu, from, want) in arms {
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        let mut game = world();
        // Twenty fields for county 1, which `world` gives the player. The flag
// `field::set_type` refuses a tile that is in no slot.
        let tile = {
            let map = &mut game.kingdom.campaign.map;
            let c = &mut game.kingdom.counties[1];
            for slot in 0..20u8 {
                let (x, y) = (10 + slot % 5, 20 + slot / 5);
                map.set_flags(x, y, flags::FARMLAND);
                map.terrain[l2_kingdom::map::index(x, y)] = from;
                // The panel takes the county off the *map*, not off the slot.
                map.county[l2_kingdom::map::index(x, y)] = 1;
                c.set_field_tile(slot as usize, Some(l2_kingdom::map::index(x, y)));
            }
            l2_kingdom::map::index(10, 20)
        };

        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        machine.push(ScreenId::Info(Target::Tile(tile)));
        // The panel's own arrival sound is `scroll1.wav`; none of the brush's
        // three is heard until a button is hit. (`heard` is a set, so this is
        // membership and not a sequence.)
        const BRUSH_WAVS: [&str; 3] = ["moo_2.wav", "wheat.wav", "fallow.wav"];
        director.listen(&mut audio, &machine, &game);
        assert!(
            !BRUSH_WAVS.iter().any(|w| audio.heard().contains(w)),
            "opening the panel painted nothing: {:?}",
            audio.heard()
        );

        let at = (bx + BRUSH_DIM / 2, BRUSH_ROW_Y + BRUSH_DIM / 2);
        send(&mut machine, &mut game, &assets, Event::Click { x: at.0, y: at.1 });
        send(&mut machine, &mut game, &assets, Event::Release { x: at.0, y: at.1 });
        assert_ne!(
            game.kingdom.campaign.map.terrain[tile], from,
            "the click at {at:?} never reached Field_SetType"
        );
        assert_eq!(machine.top_id(), Some(ScreenId::Campaign), "the brush closes the panel");

        director.listen(&mut audio, &machine, &game);
        let brushes: Vec<&str> =
            BRUSH_WAVS.into_iter().filter(|w| audio.heard().contains(w)).collect();
        assert_eq!(
            brushes,
            [want],
            "brush at x {bx} on the {} menu: all {:?}, terrain now {:#x}",
            if field_menu { "field" } else { "waste" },
            audio.heard(),
            game.kingdom.campaign.map.terrain[tile]
        );
    }
}

/// **The narrator has to stop when the window he is reading closes.**
///
/// A player reported it on build `EE0CB9233`: *"VO doesn't seem to stop when
/// the dialogue that produces it is closed, eg tutorial it will finish the
/// line."* `Msg_Dismiss` (`0x00476768`) is where the original does it, four
/// statements in and nowhere near the drawing:
///
/// ```c
/// if (g_messageGroup != 0xc2) Sound_StopOneShot();
/// ```
///
/// **Ablations, run:** delete the `audio.stop_one_shot()` in
/// `Director::listen`'s dismissal block and the first half goes red; drop the
/// `!= FOILED_AGAIN` guard and the second half does.
#[test]
fn closing_a_message_window_stops_the_narrator_unless_it_is_group_194() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clip to cut off");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    // One window, opened for real and spoken, then dismissed. Answers whether
    // the one-shot buffer was still sounding afterwards.
    let speaks_on_after_dismissal = |group: u16| -> bool {
        let mut game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();

        let mut rec = message::Record::default();
        rec.group = group;
        rec.category = message::category::COUNTY_NOTICE;
        rec.to = game.player;
        assert!(game.messages.enqueue(rec, game.player), "the record was accepted");

        // Run until the ten-tick schedule has put the line in the buffer.
        for _ in 0..40 {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }
        // 194 is inside the diplomatic band, so its clip is a lord's take and
        // not `S194_01.wav`; `Msg_PlayVoice` is what knows that.
        let clip = l2_game::audio::names::message_voice(group, rec.variant)
            .expect("both groups have a clip")
            .to_ascii_lowercase();
        assert!(audio.is_playing(&clip), "{clip} never started; heard {:?}", audio.heard());
        assert!(audio.one_shot_busy(), "and it is the one-shot's occupant");

        // The OK button, by the route `Msg_Dismiss`'s commonest caller takes.
        message::dismiss(&mut game);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
        audio.one_shot_busy()
    };

    // Group 130 is a plain notice with a system clip, and the same one
    // `a_message_window_speaks_ten_ticks_after_it_opens` uses.
    assert!(
        !speaks_on_after_dismissal(130),
        "the narrator read on past the window that produced him",
    );
    // **Group 194, *\"Foiled again.\"*** — the one group the original exempts.
    assert!(
        speaks_on_after_dismissal(message::group::FOILED_AGAIN),
        "group 194 is the exception Msg_Dismiss carves out and it was cut off",
    );
}

/// **`Opt_ToggleMusic` cuts the narrator too**, which is the half of that
/// function nobody would guess from the row's label:
///
/// ```c
/// if (g_optMusic == 0) { Music_Stop(0); Sound_StopOneShot(); }
/// ```
///
/// `turning_music_off_on_the_sounds_page_stops_the_music` covers the first
/// statement; this is the second. **Ablation, run:** delete the
/// `music_went_off` block and this goes red while that one stays green.
#[test]
fn turning_music_off_also_cuts_whoever_is_speaking() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clip to cut off");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::COUNTY_NOTICE;
    rec.to = game.player;
    assert!(game.messages.enqueue(rec, game.player), "the record was accepted");
    for _ in 0..40 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
    }
    assert!(audio.one_shot_busy(), "the narrator never started: {:?}", audio.heard());

    game.prefs.music = false;
    director.listen(&mut audio, &machine, &game);
    assert!(!audio.one_shot_busy(), "Music: Off left the narrator talking");
    // And the window is still up, so this was the switch and not a dismissal.
    assert!(game.messages.is_open(), "the window closed, which would prove the wrong thing");
}

/// **The mercenary offer is announced, and by the button that announces it.**
///
/// The second defect of the pair: *"No VO for 'A band of scottish pikemen are
/// available for hire, my lord' with a mercenary."* `Sidebar_Button`
/// (`0x0043AE30`) hotspot 1 opens `g_screenId = 0x17` and then, and only when
/// the county has an offer, `FUN_004B3714(offer - 1)`.
///
/// Band 1 is `l2_kingdom::mercenary::ROSTER[1]` — the Scottish pikemen — so
/// the reported line is `S016_01.wav` exactly.
///
/// **Ablations, run:** remove the `mercenary_offer` gate and the no-offer case
/// goes red; remove the `Armoury` gate on the previous stack and the *Change*
/// case does.
#[test]
fn the_mercenary_offer_speaks_when_the_sidebar_opens_the_raise_army_screen() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    // The ARMY button on the county the player owns, with `band` standing in
    // its town. Answers what the audio layer decoded.
    let open_raise_army = |band: u8| -> Vec<String> {
        let mut game = world();
        game.selected = 1;
        game.kingdom.counties[1].mercenary_offer = band;
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        director.listen(&mut audio, &machine, &game);

        let b = l2_game::screens::map::SIDEBAR_BUTTONS[0];
        assert_eq!(b.name, "ARMY", "the first sidebar button is hotspot 1");
        let r = b.rect();
        send(&mut machine, &mut game, &assets, Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 });
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        assert!(
            machine.ids().contains(&ScreenId::RaiseArmy(1)),
            "the button did not open the raise-army screen: {:?}",
            machine.ids(),
        );
        director.listen(&mut audio, &machine, &game);
        audio.heard().iter().map(|s| s.to_string()).collect()
    };

    let spoken = open_raise_army(1);
    assert!(
        spoken.iter().any(|n| n == "s016_01.wav"),
        "the Scottish pikemen were not announced; heard {spoken:?}",
    );
    let silent = open_raise_army(0);
    assert!(
        !silent.iter().any(|n| n.starts_with("s016_")),
        "a county with no offer announced one anyway: {silent:?}",
    );

    // **`Armoury_Button` id 2, *Change*, writes the same `g_screenId` and is
    // silent.** Ours is `Transition::Replace`, so the armoury is on the
    // previous tick's stack and that is what tells the two arrivals apart.
    let mut game = world();
    game.selected = 1;
    game.kingdom.counties[1].mercenary_offer = 1;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    machine.push(ScreenId::Armoury(1));
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    director.listen(&mut audio, &machine, &game);
    let change = l2_game::screens::armoury::CHANGE_BOX;
    send(
        &mut machine,
        &mut game,
        &assets,
        Event::Click { x: change.x + change.w / 2, y: change.y + change.h / 2 },
    );
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert!(
        machine.ids().contains(&ScreenId::RaiseArmy(1)) && !machine.ids().contains(&ScreenId::Armoury(1)),
        "Change did not replace the armoury with the raise-army screen: {:?}",
        machine.ids(),
    );
    director.listen(&mut audio, &machine, &game);
    assert!(
        !audio.heard().contains(&"s016_01.wav"),
        "arriving from the armoury announced the band; the original's Change button is silent",
    );
}

/// **The population panel's health line**, `Panel_OpenPopulation`
/// (`0x0043A8F2`) — the twin of `Panel_OpenRation`'s two arms, on the panel
/// next door.
///
/// The interesting half is the *table*: bands 3 and 4 share `S020_04.wav` at
/// `0x004E2058`, so a generated name would speak `S020_05.wav` — a real file —
/// for the healthiest county in the game and nothing would notice.
/// `tests/audio_install.rs` pins the table; this pins the wiring.
///
/// **Ablation, run:** index `POPULATION_HEALTH` by `band + 1` and the first
/// assertion goes red.
#[test]
fn the_population_panel_speaks_the_countys_health_band() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S020 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    let open_population = |band: u8| -> Vec<String> {
        let mut game = world();
        game.kingdom.counties[1].health_band = band;
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        director.listen(&mut audio, &machine, &game);
        machine.push(ScreenId::County(1, l2_game::screens::county::Panel::Population));
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        audio.heard().iter().map(|s| s.to_string()).collect()
    };

    assert!(open_population(0).iter().any(|n| n == "s020_01.wav"), "band 0");
    assert!(open_population(2).iter().any(|n| n == "s020_03.wav"), "band 2");
    // The duplicate, from the wiring's side.
    for band in [3, 4] {
        let heard = open_population(band);
        assert!(heard.iter().any(|n| n == "s020_04.wav"), "band {band}: {heard:?}");
        assert!(!heard.iter().any(|n| n == "s020_05.wav"), "band {band} spoke the next one along");
    }
}

/// **The information panel says what it is looking at** — `FUN_004B37BC`, the
/// last statement of both functions that open screen `0x04`.
///
/// Four of its five arms are the unit ladder, and the fifth is the ladder's
/// hole: **a merchant is silent**, because `Map_Click` sends a click on one to
/// the stall instead and the panel never opens on it. That hole is what this
/// asserts hardest, because the easy mistake is to fill it.
///
/// **And it is silence in the original, not a gap of ours** — it was reported
/// as one (*"picking a merchant says nothing, where a unit or a castle
/// speaks"*) and there is a third reading that settles it
/// failing to find an arm. `L2.eng` group 31 holds **five** unit descriptions;
/// indices 13 … 16 are these four, in exactly the order `S031_01` … `04`, and
/// index **12** is *"Merchants allow a county to buy needed supplies and raise
/// revenue by selling goods."* — the one member of the run with prose and no
/// recording. Only four `S031_*.wav` ship. `docs/audio.json` `FUN_004b37bc#1`.
///
/// **Ablations, run:** give `UnitKind::Merchant` a line and the silence goes
/// red; drop the owner test on the army arm and the *theirs* case does.
#[test]
fn the_information_panel_speaks_the_unit_it_opened_on() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S031 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();

    let open_info_on = |kind: l2_kingdom::UnitKind, owner: u8| -> Vec<String> {
        let mut game = world();
        let unit = l2_kingdom::unit::Unit::new(kind, owner, 10, 10);
        let id = game.kingdom.campaign.units.spawn(unit).expect("a free slot");
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut audio = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        director.listen(&mut audio, &machine, &game);
        machine.push(ScreenId::Info(l2_game::screens::info::Target::Unit(id)));
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        director.listen(&mut audio, &machine, &game);
        audio.heard().iter().map(|s| s.to_string()).collect()
    };

    use l2_kingdom::UnitKind;
    // `world()`'s local player is realm 1.
    let mine = open_info_on(UnitKind::Army, 1);
    assert!(mine.iter().any(|n| n == "s031_04.wav"), "my own army: {mine:?}");
    let theirs = open_info_on(UnitKind::Army, 2);
    assert!(theirs.iter().any(|n| n == "s031_03.wav"), "somebody else's army: {theirs:?}");
    let transport = open_info_on(UnitKind::Transport, 1);
    assert!(transport.iter().any(|n| n == "s031_02.wav"), "a transport: {transport:?}");
    let mob = open_info_on(UnitKind::PeasantMob, 2);
    assert!(mob.iter().any(|n| n == "s031_01.wav"), "a peasant mob: {mob:?}");
    // **The hole.** `FUN_004B37BC` has `kind == 1`, `== 4` and `== 2` and no
    // arm for 3.
    let merchant = open_info_on(UnitKind::Merchant, 1);
    assert!(
        !merchant.iter().any(|n| n.starts_with("s031_")),
        "the merchant has no arm in the original's ladder: {merchant:?}",
    );
}
