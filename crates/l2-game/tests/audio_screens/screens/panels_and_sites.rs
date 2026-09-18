#![allow(unused_imports)]
use super::*;
use super::title_and_screens::*;
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

/// `docs/decisions.md` C133 searched every one of `L2.eng`'s 317 groups for
/// that sentence and correctly found nothing.
///
/// `Panel_OpenRation` (`0x0043A846`) **speaks** it: `S021_01.wav`, on the frame
/// the ration panel opens, when the county has a standing herd and opening the
/// larder took neither a cow nor a sack.
///
/// **Ablation:** delete the `herd != 0 && herd_eaten == 0 && grain_eaten == 0`
/// arm and the first assertion goes red with an empty `heard`; swap the two
/// arms' order and the *second* goes red with `s021_01.wav` where
/// `s021_02.wav` belongs.
#[test]
fn the_ration_panel_speaks_when_the_county_lives_on_dairy() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();
    let mut game = world();
    game.kingdom.counties[1].ration_achieved = 3;
    game.kingdom.counties[1].herd = 400;
    game.kingdom.counties[1].herd_eaten = 0;
    game.kingdom.counties[1].grain_eaten = 0;
    game.select(1);

    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);

    machine.push(ScreenId::County(1, Panel::Ration));
    listen(&mut director, &mut audio, &machine, &game);
    assert!(
        audio.heard().contains(&"s021_01.wav"),
        "the dairy line did not play - heard {:?}",
        audio.heard()
    );
    assert!(!audio.heard().contains(&"s021_02.wav"), "and not the complaint");

    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut director = audio::Director::new();
    game.kingdom.counties[1].ration_achieved = 0;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    listen(&mut director, &mut audio, &machine, &game);
    machine.push(ScreenId::County(1, Panel::Ration));
    listen(&mut director, &mut audio, &machine, &game);
    assert!(
        audio.heard().contains(&"s021_02.wav"),
        "an unfed county should get the complaint - heard {:?}",
        audio.heard()
    );
    assert!(!audio.heard().contains(&"s021_01.wav"), "and never the compliment");
    let _ = &assets;
}

/// `TileInfo_Draw` (`0x0041C208`) plays the site's work as the information
/// panel paints it: a mine rings, a quarry and a smithy hammer, a lumber mill
/// saws. All four ranges are driven, because the interesting one is the
/// **blacksmith**, which plays the quarry's sound — the kingdom bank has no
/// forge — and a test that only checked the mine would not have noticed.
#[test]
fn right_clicking_a_resource_site_plays_its_work() {
    let Some(_) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();

    for (graphic, want) in
        [(1u8, "iron.wav"), (5, "stonecut.wav"), (8, "stonecut.wav"), (11, "woodcut.wav")]
    {
        let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
        let mut game = world();
        let tile = l2_kingdom::map::index(20, 20);
        game.kingdom.campaign.map.county[tile] = 1;
        game.kingdom.campaign.map.terrain[tile] = graphic;
        game.kingdom.campaign.map.flags[tile] |= l2_kingdom::map::flags::SETTLEMENT;

        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut director = audio::Director::new();
        listen(&mut director, &mut audio, &machine, &game);

        machine.push(ScreenId::Info(Target::Tile(tile)));
        listen(&mut director, &mut audio, &machine, &game);
        assert!(
            audio.heard().contains(&want),
            "graphic {graphic} should sound {want} - heard {:?}",
            audio.heard()
        );
    }

    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut game = world();
    let tile = l2_kingdom::map::index(21, 21);
    game.kingdom.campaign.map.terrain[tile] = 1;
    game.kingdom.campaign.map.flags[tile] &= !l2_kingdom::map::flags::SETTLEMENT;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);
    machine.push(ScreenId::Info(Target::Tile(tile)));
    listen(&mut director, &mut audio, &machine, &game);
    assert!(!audio.heard().contains(&"iron.wav"), "a bare tile is not a mine");
    let _ = &assets;
}

/// `Panel_JobDetail` (`0x00412B33`) opens the job popup with the sound of the
/// job being done. Six of the nine have one; job 8, the blacksmith, plays a
/// forge *and* a hammer, which is the only place in the game that fires a file
/// and a bank slot together; jobs 4 and 9 are silent and the zero in the table
/// is what says so.
#[test]
fn the_job_popup_opens_with_the_sound_of_the_job() {
    let Some(_) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();

    let rows: [(usize, &[&str]); 4] = [
        (0, &["wheat.wav"]),          // job 1, grain
        (1, &["moo_2.wav"]),          // job 2, cattle
        (7, &["fire.wav", "stonecut.wav"]), // job 8, the blacksmith - both
        (8, &[]),                     // job 9, idle: g_jobSound[9] is 0
    ];
    for (job, want) in rows {
        let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
        let game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut director = audio::Director::new();
        listen(&mut director, &mut audio, &machine, &game);

        machine.push(ScreenId::Job(1, job));
        listen(&mut director, &mut audio, &machine, &game);
        let effects: Vec<&str> =
            audio.heard().into_iter().filter(|n| !n.starts_with("scroll")).collect();
        assert_eq!(effects, want, "job {} (1-based {})", job, job + 1);
    }
    let _ = &assets;
}

