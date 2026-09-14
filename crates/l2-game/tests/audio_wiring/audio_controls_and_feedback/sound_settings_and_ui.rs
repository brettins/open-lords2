#![allow(unused_imports)]
use super::*;
use super::map_and_castle_feedback::*;
use super::speech_and_panels::*;
use super::*;
use super::routing::*;
use super::ui_and_speech::*;
use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

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

