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


/// **`Msg_DrawWindow#16` and `#15`, from a county taken on the map.** The march
/// is `triggers::capture`'s — `County_ChangeOwner` runs, `arrival::capture_record`
/// posts the category-`0x0D` letter, `Msg_DrawWindow`'s animated branch plays
/// `cap_cty*.smk` — and this one listens to it: the narrator reads the letter as
/// the film opens (`#16`, `Msg_PlayVoice(DAT_004F0374, DAT_004F0354)` after
/// `Smk_Play` returns with the first frame up), and when the film goes the
/// campaign bed starts again from its first sample (`#15`, the branch's
/// `Music_StartCampaign`).
///
/// Ablations: delete the `voice` call from `Director::listen` and the narrator
/// half goes red; make `Scene::Film` answer the campaign track instead of
/// `None` in `Audio::follow` and the bed half does.
#[test]
fn the_narrator_reads_a_capture_over_its_film_and_the_bed_starts_over_after_it() {
    let (p, a) = install!();
    let mut audio = Audio::headless(&p.vfs);
    let mut director = audio::Director::new();
    let (mut g, _, mut m) = capture_world();
    march_on_the_town(&mut g);

    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    let before = audio.music_name().expect("the campaign bed");
    assert_eq!(before, "scroll1.wav", "one county is FUN_00499ACA's first band");
    for _ in 0..4 {
        buffer(&mut audio);
    }

    let film = loop {
        tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
        if let Some(f) = film_on_top(&m) {
            break f;
        }
        assert_ne!(m.top_id(), Some(ScreenId::Message), "the letter stayed an ordinary window");
    };
    let Film::Capture { record, .. } = film else { panic!("{film:?}") };
    assert_eq!(audio.film_name().as_deref(), Some(film.file()), "the film's own track");
    assert_eq!(audio.music_name(), None, "Music_Stop(0) before Smk_Play");

    let line = l2_game::audio::names::message_voice(record.group, record.variant)
        .unwrap_or_else(|| panic!("group {:#x} has no clip", record.group));
    assert!(
        audio.heard().contains(&line.to_ascii_lowercase().as_str()),
        "{line} was not spoken; heard {:?}",
        audio.heard()
    );

    send(&mut m, &mut g, &a, Event::RightClick { x: 5, y: 5 });
    tick_and_listen(&mut m, &mut g, &a, &mut audio, &mut director);
    assert_eq!(audio.film_name(), None, "SmackClose");
    // Two counties of six is a higher band than one and `Audio::follow` picks
    // the bed on the derivation the film's screen leaves, so the restart is a
    // *different* track from the `scroll1.wav` the film interrupted — which is
    // the first sample by construction: nothing was playing it to resume.
    // The sibling test above holds the same-track half of `Music_StartCampaign`.
    let after = audio.music_name().expect("the bed is back");
    assert_ne!(after, before, "the taken county moved the player up a band");
    assert!(buffer(&mut audio).iter().any(|&s| s != 0.0), "and it is audible");
}
