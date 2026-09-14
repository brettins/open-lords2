#![allow(unused_imports)]
use super::*;
use super::triggers::*;
use super::playback::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message::{category, Record};
use l2_game::movie::{self, Film};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, setup};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

/// **A film stops the bed and the bed starts over after it** — the five
/// restart sites `docs/audio.json` now calls reproduced, heard
/// read. The castle film's track plays in between. Ablation: make
/// `Scene::Film` answer the campaign track
#[test]
fn a_film_silences_the_campaign_bed_and_it_starts_again_from_its_first_sample() {
    let (p, a) = install!();
    let mut audio = Audio::headless(&p.vfs);
    let mut director = audio::Director::new();
    let (mut g, _) = castle_world();
    g.kingdom.realms[1].county_count = 1;
    let mut m = Machine::new(ScreenId::Campaign);

    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    let bed = audio.music_name().expect("the campaign bed");
    let first = buffer(&mut audio);
    for _ in 0..8 {
        buffer(&mut audio);
    }

    m.push(ScreenId::Movie(Film::Castle(0)));
    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert_eq!(audio.music_name(), None, "Music_Stop(0) before Smk_Play");
    assert_eq!(audio.film_name().as_deref(), Some("castle1.smk"), "the film's own track");
    assert!(buffer(&mut audio).iter().any(|&s| s != 0.0), "and it is audible");

    send(&mut m, &mut g, &a, Event::RightClick { x: 5, y: 5 });
    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert_eq!(audio.film_name(), None, "SmackClose");
    assert_eq!(audio.music_name(), Some(bed), "the bed is back");
    assert_eq!(buffer(&mut audio), first, "from its first sample, not from where it stopped");
}

/// **`Smk_OnFinished#1`**: back on the title page after the trailer,
/// `setup.wav` starts over.
#[test]
fn the_trailer_stops_setup_wav_and_the_title_page_starts_it_over() {
    let (p, a) = install!();
    let mut audio = Audio::headless(&p.vfs);
    let mut director = audio::Director::new();
    let mut g = Game::new(1);
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));

    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert_eq!(audio.music_name().as_deref(), Some("setup.wav"));
    let first = buffer(&mut audio);
    buffer(&mut audio);

    let (x, y) = centre(setup::item_rect(2));
    send(&mut m, &mut g, &a, Event::Click { x, y });
    send(&mut m, &mut g, &a, Event::Release { x, y });
    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert_eq!(audio.music_name(), None);
    assert_eq!(audio.film_name().as_deref(), Some("lom.smk"));

    send(&mut m, &mut g, &a, Event::KeyDown(Key::Space));
    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert_eq!(audio.music_name().as_deref(), Some("setup.wav"));
    assert_eq!(buffer(&mut audio), first);
}

/// **`Msg_DrawWindow#21`**: the narrator reads the fall **as the film opens**,
/// because `Smk_Play` returns with the first frame up. Ablation: delete the
/// `voice` call from `Director::listen`.
#[test]
fn the_narrator_reads_an_ending_over_the_opening_of_its_film() {
    let (p, a) = install!();
    let mut audio = Audio::headless(&p.vfs);
    let mut director = audio::Director::new();
    let mut g = realms();
    let player = g.player;
    assert!(g.messages.enqueue(ending(3, 194), player));
    let mut m = Machine::new(ScreenId::Campaign);

    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert!(matches!(film_on_top(&m), Some(Film::Ending { .. })));
    assert_eq!(audio.film_name().as_deref(), Some("cart_brn.smk"));
    let line = l2_game::audio::names::message_voice(194, 0).expect("194 has a clip");
    assert!(
        audio.heard().contains(&line.to_ascii_lowercase().as_str()),
        "{line} was not spoken; heard {:?}",
        audio.heard()
    );
}

