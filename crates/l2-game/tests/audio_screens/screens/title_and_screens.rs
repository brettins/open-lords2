#![allow(unused_imports)]
use super::*;
use super::panels_and_sites::*;
use super::standings_and_popups::*;
use super::*;
use super::audio_behavior::*;
use l2_game::audio::{self, names, Audio};
use l2_game::game::Assets;
use l2_game::screen::{Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::info::Target;
use l2_game::screens::setup::SetupPage;
use l2_game::input::{Event, Key};
use l2_game::screen::Ctx;
use l2_game::Game;

/// **The front end is not silent, and it was.**
///
/// This is the sound the eight-primitive audit structurally could not see:
/// `setup.wav` is started by `Music_Play` (`0x004263AD`), a **ninth** leaf that
/// `docs/audio-triggers.md`'s table did not have, so no row of the inventory
/// said it was missing and `audio::scene`'s `FrontEnd => None` read as a
/// finding.
///
/// A player reported *"I don't hear music"*; C116 fixed the campaign half of
/// that and the title screen stayed quiet.
///
/// **Ablation:** change `Scene::FrontEnd`'s arm back to `None` and this goes
/// red on the `music_name` — and *only* on the front-end assertion, because the
/// campaign one below it is a different ladder.
#[test]
fn the_title_screen_plays_setup_wav_and_the_campaign_still_does_not() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let _assets = Assets::placeholder();
    let mut game = world();
    let machine = Machine::new(APP_ROOT);
    let mut director = audio::Director::new();

    listen(&mut director, &mut audio, &machine, &game);
    assert_eq!(
        audio.music_name().as_deref(),
        Some("setup.wav"),
        "the front end is silent; Music_Play's nine sites are all this bed"
    );

    // And the campaign replaces it.
    let mut machine = machine;
    machine.push(ScreenId::Campaign);
    let _ = &mut game;
    listen(&mut director, &mut audio, &machine, &game);
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"));
}

/// **The narrator on the setup pages and the sidebar.**
///
/// Four of these, in one test because they are one mechanism: a screen arrives
/// and the function that set `g_screenId` spoke. The shield page is the one
/// with four call sites behind it — three routes we have and one, the network
/// join, that we do not.
///
/// **Ablation:** remove any one `opened(...)` block and exactly one assertion
/// goes red.
#[test]
fn four_screens_speak_as_they_open() {
    let Some(_) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();

    for (push, want) in [
        (ScreenId::Setup(SetupPage::Shield), names::speech::CHOOSE_YOUR_SHIELD),
        (ScreenId::Supplies(1), names::speech::SUPPLIES),
        (ScreenId::Divide(1), names::speech::SPLIT_ARMY),
    ] {
        let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
        let game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut director = audio::Director::new();
        listen(&mut director, &mut audio, &machine, &game);

        machine.push(push);
        listen(&mut director, &mut audio, &machine, &game);
        assert!(
            audio.heard().contains(&want.to_ascii_lowercase().as_str()),
            "{push:?} should speak {want} - heard {:?}",
            audio.heard()
        );
    }

    // `Map_ZoomOut` is the one edge that is not a screen: it is the zoom level,
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);
    assert!(!audio.heard().contains(&"s033_02.wav"), "starting zoomed in is not an event");
    game.map_zoom_far = true;
    listen(&mut director, &mut audio, &machine, &game);
    assert!(audio.heard().contains(&"s033_02.wav"), "heard {:?}", audio.heard());
    // Zooming back in is silent, and staying out does not repeat.
    let before = audio.heard().len();
    listen(&mut director, &mut audio, &machine, &game);
    game.map_zoom_far = false;
    listen(&mut director, &mut audio, &machine, &game);
    assert_eq!(audio.heard().len(), before, "Map_ZoomIn has no sound");
    let _ = &assets;
}

/// **A screen that is already up does not keep speaking**, which is the whole
/// reason this is an edge and not a predicate.
///
/// `Director::listen` runs sixty times a second. The original's sound is in the
/// handler that *changes* `g_screenId`, so it happens once; a test that only
/// checked "did it play" would pass and the
/// player would hear the narrator repeat for as long as the panel was open.
///
/// **A repeat can only be heard once the line has ended.** `Sidebar_Button`'s
/// call is `Sound_PlayFile`, which drops a request while the one-shot buffer
/// sounds, so sixty ticks of the same screen *during* the line are sixty dropped
/// requests whether the edge is there or not. The previous version of this test
/// mixed two buffers mid-line and compared them, which was a test of the rewind a
/// second trigger used to cause; once `Audio::play_file` dropped, the drop hid
/// the missing edge. So the line is played to its
/// end, and then the panel, still up, is listened to again.
///
/// **Ablation:** drop the `&& !self.stack…` half of `opened` and this goes red
/// on the line starting again.
#[test]
fn a_screen_speaks_once_and_not_once_a_frame() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();
    let game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);

    machine.push(ScreenId::Supplies(1));
    listen(&mut director, &mut audio, &machine, &game);
    let line = names::speech::SUPPLIES;
    assert!(audio.is_playing(line), "the supplies line never started, so this proves nothing");

    let mut buf = vec![0.0f32; 2 * 4096];
    for n in 0.. {
        if !audio.is_playing(line) {
            break;
        }
        assert!(n < 2_000, "{line} never finished");
        audio.mix(&mut buf);
    }
    for _ in 0..60 {
        listen(&mut director, &mut audio, &machine, &game);
    }
    assert!(
        !audio.is_playing(line),
        "sixty ticks of a panel that was already up started its line again: the narrator \
         repeats for as long as the panel is open"
    );
    let _ = &assets;
}

