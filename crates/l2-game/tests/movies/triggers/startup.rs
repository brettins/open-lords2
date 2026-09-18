#![allow(unused_imports)]
use super::*;
use super::gameplay::*;
use super::validation::*;
use super::*;
use super::playback::*;
use super::audio_part::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message::{category, Record};
use l2_game::movie::{self, Film};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, setup};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

/// **`App_WinMain`'s `FUN_004B3571(0)`: the application opens on the intro.**
#[test]
fn the_application_opens_on_the_intro_over_the_title_page() {
    let src = include_str!("../../../src/main.rs");
    assert!(
        src.contains("l2_game::movie::start_up(&mut machine);"),
        "main.rs no longer plays the intro at start-up"
    );
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    movie::start_up(&mut m);
    assert_eq!(
        m.ids(),
        vec![ScreenId::Setup(setup::SetupPage::Title), ScreenId::Movie(Film::Intro)]
    );
}

/// Seven of the eight `Smk_Play` calls pass `g_screenId` itself or the front
/// end's `0x1F` — *come back where you were*, which in a stack is a pop. **One
/// passes a literal**: `CastleBuild_Confirm` (`0x00436B59`) passes `0`, the
/// campaign map, so the end of a castle film is the map and the chooser that
/// raised it is gone with it. That is [`Transition::Goto`], and nothing else
/// here needs it.
#[test]
fn the_start_up_films_chain_intro_logo_credits_and_nothing_else_does() {
    use l2_game::screen::Transition::*;
    assert_eq!(Film::Intro.then(), Replace(ScreenId::Movie(Film::ImpTitle)));
    assert_eq!(Film::ImpTitle.then(), Replace(ScreenId::Movie(Film::Credits)));
    assert_eq!(Film::Credits.then(), Pop);
    assert_eq!(Film::LordsOfMagic.then(), Pop);
    assert_eq!(Film::Battle { file: "bat_win1.smk" }.then(), Pop);
    assert_eq!(Film::Castle(2).then(), Goto(ScreenId::Campaign));
    assert_eq!(Film::Intro.on_failure(), Pop);
    assert_eq!(Film::Castle(2).on_failure(), Goto(ScreenId::Campaign));
    assert_eq!(Film::Intro.at(), (40, 80));
    assert_eq!(Film::ImpTitle.at(), (80, 80));
    assert_eq!(Film::Credits.at(), (0, 0));
    assert_eq!(Film::LordsOfMagic.at(), (70, 80));
    assert_eq!(Film::Castle(0).at(), (158, 20));
    assert_eq!(Film::Battle { file: "bat_win1.smk" }.at(), (39, 73));
}

#[test]
fn a_missing_intro_goes_straight_to_the_title_page() {
    let a = Assets::placeholder();
    let mut g = Game::new(1);
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    movie::start_up(&mut m);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::Intro)), "held one tick");
    tick(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Setup(setup::SetupPage::Title)], "no logo, no credits");
}

#[test]
fn input_to_a_film_that_could_not_open_reaches_the_screen_beneath() {
    let a = Assets::placeholder();
    let mut g = Game::new(1);
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    movie::start_up(&mut m);
    send(&mut m, &mut g, &a, Event::Release { x: 300, y: 300 });
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::Intro)), "nothing beneath wants a release");
    send(&mut m, &mut g, &a, Event::KeyDown(Key::Escape));
    assert!(m.should_quit(), "the title page's Escape got the key: {:?}", m.ids());
}

#[test]
fn lords_of_magic_plays_its_trailer_on_the_release() {
    let a = Assets::placeholder();
    let mut g = Game::new(1);
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    let (x, y) = centre(setup::item_rect(2));
    send(&mut m, &mut g, &a, Event::Click { x, y });
    assert_eq!(m.top_id(), Some(ScreenId::Setup(setup::SetupPage::Title)), "the press only selects");
    send(&mut m, &mut g, &a, Event::Release { x, y });
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::LordsOfMagic)));
    tick(&mut m, &mut g, &a);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(setup::SetupPage::Title)));
}

