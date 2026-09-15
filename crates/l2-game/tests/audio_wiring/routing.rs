#![allow(unused_imports)]
use super::*;
use super::ui_and_speech::*;
use super::audio_controls_and_feedback::*;
use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

#[test]
fn the_root_is_the_one_the_application_starts_on() {
    let src = include_str!("../../src/main.rs");
    assert!(
        src.contains("Machine::new(ScreenId::Setup(SetupPage::Title))"),
        "main.rs no longer starts on {APP_ROOT:?}; every test in this file is now \
         asserting about a state the application does not reach. Update APP_ROOT \
         and re-read audio::scene's front-end arm."
    );
}

/// C27, nine instances, and this was the tenth.
#[test]
fn the_application_calls_the_director() {
    let src = include_str!("../../src/main.rs");
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

/// Ablation: restore `if let Some(ScreenId::Setup(_)) = machine.ids().first()`
/// and this goes red on every one of the twenty-odd screens.
#[test]
fn every_in_game_screen_over_the_front_end_is_campaign_music() {
    let game = world();
    let root = Machine::new(APP_ROOT);
    assert_eq!(audio::scene(&root, &game), Scene::FrontEnd);

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

    game.campaign.map = l2_game::victory::CAMPAIGN_LENGTH;
    assert_eq!(audio::scene(&m, &game), Scene::Conquest { ended: true });
    assert_eq!(Music::Setup2.file(), "setup2.wav");
    assert!(!Music::Setup2.loops(), "`Music_Play(…, 0, 0)` above it");
}

#[test]
fn the_front_end_is_its_own_scene_on_all_thirteen_of_its_pages() {
    let game = world();
    for page in SetupPage::ALL {
        let m = Machine::new(ScreenId::Setup(page));
        assert_eq!(audio::scene(&m, &game), Scene::FrontEnd, "{page:?}");
    }
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Index);
    assert_eq!(audio::scene(&m, &game), Scene::FrontEnd, "the index over the title");
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Campaign);
    m.push(ScreenId::Index);
    assert!(matches!(audio::scene(&m, &game), Scene::Campaign { .. }));
}

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

