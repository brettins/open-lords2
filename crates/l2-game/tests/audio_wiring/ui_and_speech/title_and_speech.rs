#![allow(unused_imports)]
use super::*;
use super::voice_and_audio_metrics::*;
use super::*;
use super::routing::*;
use super::audio_controls_and_feedback::*;
use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

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

    macro_rules! tick {
        () => {{
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }};
    }

    tick!();
    // `Music_Play` (`0x004263AD`) is a ninth sound primitive that
    // `docs/audio-triggers.md`'s eight did not enumerate, and all nine of its
    // call sites are the front end playing `setup.wav`. See
    // `tests/audio_screens.rs`.
    assert_eq!(audio.music_name().as_deref(), Some("setup.wav"), "the front end's own bed");

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

    let mut buf = vec![0f32; 4_410 * 2];
    audio.mix(&mut buf);
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "scroll1 is loaded and the mixer produced silence (peak {peak})");
    assert!(buf.iter().all(|s| s.is_finite() && (-1.0..=1.0).contains(s)), "out of range");

    let before = audio.music_name();
    for _ in 0..120 {
        tick!();
    }
    assert_eq!(audio.music_name(), before, "the track restarted under a steady scene");
}

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

    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::COUNTY_NOTICE;
    rec.to = game.player;
    assert!(game.messages.enqueue(rec, game.player), "the record was accepted");

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
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"));

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

