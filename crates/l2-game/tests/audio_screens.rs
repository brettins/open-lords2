//! **The sounds a screen makes when it opens**, which is where nearly all of
//! the original's non-battlefield audio lives.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_screens
//! ```
//!
//! `tests/audio_wiring.rs` covers the music policy and the narrator's message
//! window. This file covers the class that `docs/audio-triggers.md` had counted
//! and nobody had wired: **`Sound_PlayFile(name, 1, 0)` and
//! `Sound_RestartSlot(n)` in the function that sets `g_screenId`.**
//!
//! # Why the screen arriving is the trigger and not a stand-in for it
//!
//! The temptation is to read `Director`'s screen-stack diff as an approximation
//! — *the original plays a sound, we notice a screen*. It is not.
//! `Panel_OpenRation` (`0x0043A846`) is **four statements** and two of them are
//! sounds:
//!
//! ```c
//! g_screenId = 0x19; Panel_Ration();
//! if (rationAchieved == 0)                                Sound_PlayFile("S021_02.wav", 1, 0);
//! else if (herd && !herdEaten && !grainEaten)             Sound_PlayFile("S021_01.wav", 1, 0);
//! ```
//!
//! The sound *is* the screen opening, with a condition on the county attached.
//! `Sidebar_Button`'s supplies arm, `Panel_SplitButton`, `Map_ZoomOut`,
//! `Panel_JobDetail` and `TileInfo_Draw` are all that shape, which is why one
//! mechanism reaches fourteen sites.
//!
//! Everything below drives real [`Event`]s through [`Machine::handle`] and then
//! runs [`audio::Director::listen`], the way `main.rs` does. Nothing constructs
//! a stack, for the reason `audio_wiring.rs` opens with.

use l2_game::audio::{self, names, Audio};
use l2_game::game::Assets;
use l2_game::screen::{Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::info::Target;
use l2_game::screens::setup::SetupPage;
use l2_game::input::{Event, Key};
use l2_game::screen::Ctx;
use l2_game::Game;

fn send(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

const APP_ROOT: ScreenId = ScreenId::Setup(SetupPage::Title);

fn world() -> Game {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(14);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].county_count = 1;
    g.player = 1;
    g
}

/// An audio layer that indexes and decodes the player's install with no device.
fn headless() -> Option<Audio> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Audio::headless(&platform.vfs))
}

/// Run the director over the machine as it stands, as the event loop does.
fn listen(d: &mut audio::Director, a: &mut Audio, m: &Machine, g: &Game) {
    d.listen(a, m, g);
}

/// **The front end is not silent, and it was.**
///
/// This is the sound the eight-primitive audit structurally could not see:
/// `setup.wav` is started by `Music_Play` (`0x004263AD`), a **ninth** leaf that
/// `docs/audio-triggers.md`'s table did not have, so no row of the inventory
/// said it was missing and `audio::scene`'s `FrontEnd => None` read as a
/// finding rather than a gap.
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

    // And the campaign replaces it rather than layering on it.
    let mut machine = machine;
    machine.push(ScreenId::Campaign);
    let _ = &mut game;
    listen(&mut director, &mut audio, &machine, &game);
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"));
}

/// **"All your people are fed by dairy" — the readout a player asked for, in
/// the medium the game actually uses.**
///
/// `docs/decisions.md` C133 searched every one of `L2.eng`'s 317 groups for
/// that sentence and correctly found nothing. It is not a string.
/// `Panel_OpenRation` (`0x0043A846`) **speaks** it: `S021_01.wav`, on the frame
/// the ration panel opens, when the county has a standing herd and opening the
/// larder took neither a cow nor a sack.
///
/// The two arms are asserted separately and in the original's order, because
/// the `else if` is load-bearing: a county that is fed on nothing gets the
/// complaint, never the compliment.
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
    // A county that is fed, holds cattle, and ate nothing to do it.
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

    // A county that is not fed at all takes the first arm instead. A fresh
    // director and a fresh audio layer, because `heard` is cumulative by
    // design.
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut director = audio::Director::new();
    game.kingdom.counties[1].ration_achieved = 0;
    // Still holding cattle and still eating nothing, so only the FIRST clause
    // can be what decides it.
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

/// **The industry sounds a player asked for**, *"when you right click them on
/// the map"*.
///
/// `TileInfo_Draw` (`0x0041C208`) plays the site's work as the information
/// panel paints it: a mine rings, a quarry and a smithy hammer, a lumber mill
/// saws. All four ranges are driven, because the interesting one is the
/// **blacksmith**, which plays the quarry's sound — the kingdom bank has no
/// forge — and a test that only checked the mine would not have noticed.
///
/// **Ablation:** delete the `resource_site_slot` call in `Director::listen` and
/// all four go red; change `7..=9`'s answer from 8 to 9 and only the third row
/// does.
#[test]
fn right_clicking_a_resource_site_plays_its_work() {
    let Some(_) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();

    // The four graphics are the original's own ranges, and the expected files
    // are read off `KINGDOM_BANK` by slot rather than recomputed from the
    // ladder under test.
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

    // And a tile that is not a settlement is silent, which is the `0x80` guard.
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

/// **The village's work, from `g_jobSound`.**
///
/// `Panel_JobDetail` (`0x00412B33`) opens the job popup with the sound of the
/// job being done. Six of the nine have one; job 8, the blacksmith, plays a
/// forge *and* a hammer, which is the only place in the game that fires a file
/// and a bank slot together; jobs 4 and 9 are silent and the zero in the table
/// is what says so.
///
/// **Ablation:** delete the `JOB_SOUND` lookup and the six go red while the
/// blacksmith stays green, which is the point of driving both branches.
#[test]
fn the_job_popup_opens_with_the_sound_of_the_job() {
    let Some(_) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();

    // Our job is zero-based; the original's `g_jobPanelJob` is not.
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
        // The campaign bed is in `heard` too and is not what this is about.
        let effects: Vec<&str> =
            audio.heard().into_iter().filter(|n| !n.starts_with("scroll")).collect();
        assert_eq!(effects, want, "job {} (1-based {})", job, job + 1);
    }
    let _ = &assets;
}

/// **The narrator on the setup pages and the sidebar.**
///
/// Four of these, in one test because they are one mechanism: a screen arrives
/// and the function that set `g_screenId` spoke. The shield page is the one
/// with four call sites behind it — three routes we have and one, the network
/// join, that we do not.
///
/// **Ablation:** remove any one `opened(...)` block and exactly one assertion
/// goes red, which is what makes this a test of four things rather than of one.
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
    // and there is no matching sound on the way back in.
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
/// checked "did it play" would pass just as well with the guard removed and the
/// player would hear the narrator repeat for as long as the panel was open.
///
/// **A repeat can only be heard once the line has ended.** `Sidebar_Button`'s
/// call is `Sound_PlayFile`, which drops a request while the one-shot buffer
/// sounds, so sixty ticks of the same screen *during* the line are sixty dropped
/// requests whether the edge is there or not. The previous version of this test
/// mixed two buffers mid-line and compared them, which was a test of the rewind a
/// second trigger used to cause; once `Audio::play_file` dropped, the drop hid
/// the missing edge exactly as the original's would. So the line is played to its
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

/// Mix until `name` has stopped sounding.
fn drain(audio: &mut Audio, name: &str) {
    let mut buf = vec![0.0f32; 2 * 4096];
    for _ in 0..2_000 {
        if !audio.is_playing(name) {
            return;
        }
        audio.mix(&mut buf);
    }
    panic!("{name} never finished");
}

/// **A line or a fanfare asked for while the one-shot buffer sounds is
/// dropped, and one asked for once it has finished plays.** `Sound_PlayFile`
/// opens with `if (Sound_OneShotBusy()) return 0;`, and `Map_ZoomOut` and
/// `Battle_ChooseSettlement` call it with nothing in front of it, so the
/// narrator's zoom-out line and the battle fanfare wait for nobody: asked for
/// over another clip, they are not played at all. `[V]`
///
/// The buffer is held by `Panel_OpenRation`'s `S021_01.wav`, itself a
/// `Sound_PlayFile`. **And a request that is dropped does not take the
/// buffer**: after each drop the ration line is still what `Sound_OneShotBusy`
/// asks about. Nothing is mixed until both drops have been asked for, so the
/// line cannot have ended by itself in between.
///
/// Ablations: take the busy test out of `Audio::play_file` and the zoom-out line
/// is heard over the ration line; give `Map_ZoomOut` `stop_and_play_file`, the
/// same; give `Battle_ChooseSettlement` `play_effect` back and `ff_batl.wav` is
/// heard; record the name in `play_file` before its busy return and
/// `one_shot_busy` answers no after the first drop.
#[test]
fn a_line_or_a_fanfare_over_the_one_shot_buffer_is_dropped_and_after_it_plays() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut game = world();
    game.kingdom.counties[1].ration_achieved = 3;
    game.kingdom.counties[1].herd = 400;
    game.kingdom.counties[1].herd_eaten = 0;
    game.kingdom.counties[1].grain_eaten = 0;
    game.select(1);
    // `Machine` has no pop, and the director reads nothing but the ids, so each
    // listen is handed the stack it should see.
    let stack = |over: &[ScreenId]| {
        let mut m = Machine::new(APP_ROOT);
        m.push(ScreenId::Campaign);
        for id in over {
            m.push(*id);
        }
        m
    };
    let ration = ScreenId::County(1, Panel::Ration);
    let prompt = ScreenId::BattlePrompt;
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &stack(&[]), &game);

    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    assert!(
        audio.is_playing(names::speech::RATION_ON_DAIRY),
        "the ration line did not start, so nothing below is tested - heard {:?}",
        audio.heard()
    );
    assert!(audio.one_shot_busy());

    game.map_zoom_far = true;
    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    assert!(!audio.heard().contains(&"s033_02.wav"), "Map_ZoomOut's line played over the ration line");
    assert!(audio.one_shot_busy(), "the dropped zoom-out line took the buffer");

    listen(&mut director, &mut audio, &stack(&[ration, prompt]), &game);
    assert!(!audio.heard().contains(&"ff_batl.wav"), "the battle fanfare played over the ration line");
    assert!(audio.one_shot_busy(), "the dropped fanfare took the buffer");

    drain(&mut audio, names::speech::RATION_ON_DAIRY);
    assert!(!audio.one_shot_busy());

    // The same two again, each into an idle buffer.
    game.map_zoom_far = false;
    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    game.map_zoom_far = true;
    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    assert!(audio.heard().contains(&"s033_02.wav"), "heard {:?}", audio.heard());
    assert!(audio.one_shot_busy(), "the zoom-out line holds the buffer");

    drain(&mut audio, names::speech::ZOOM_OUT);
    listen(&mut director, &mut audio, &stack(&[ration, prompt]), &game);
    assert!(audio.heard().contains(&"ff_batl.wav"), "heard {:?}", audio.heard());
    assert!(audio.one_shot_busy(), "and the fanfare holds it in turn");
}

/// **The standings page says which category you are looking at** —
/// `FUN_004B3994(DAT_0055CE7C)`, whose two callers are the court's *Greatest
/// nobles* button (`FUN_004351C4`) and one of the page's seven tabs
/// (`FUN_0043524E`). Both are unconditional, and that is the whole design of
/// the edge: `Game::nobles_spoken` is a counter both bump, so a tab pressed
/// twice speaks twice.
///
/// **Ablation, run:** make the director diff `game.nobles_category` instead of
/// the counter and the third assertion goes red — the second press of the same
/// tab is silent, where the original speaks.
#[test]
fn the_standings_page_speaks_its_category_every_time_it_is_asked() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);

    // The button, which is `FUN_004351C4`: the page opens on category 0 and
    // the line is *"Most counties,"*.
    machine.push(ScreenId::Nobles);
    game.nobles_spoken += 1;
    listen(&mut director, &mut audio, &machine, &game);
    let first = names::speech::STANDINGS_CATEGORY[0];
    assert!(audio.is_playing(first), "heard {:?}", audio.heard());
    drain(&mut audio, first);

    // A tab: the category moves and so does the file.
    game.nobles_category = 3;
    game.nobles_spoken += 1;
    listen(&mut director, &mut audio, &machine, &game);
    let crowns = names::speech::STANDINGS_CATEGORY[3];
    assert!(audio.is_playing(crowns), "heard {:?}", audio.heard());
    drain(&mut audio, crowns);

    // The **same** tab again. The category has not moved; the original plays
    // anyway, because `FUN_0043524E`'s call has no guard on it.
    game.nobles_spoken += 1;
    listen(&mut director, &mut audio, &machine, &game);
    assert!(
        audio.is_playing(crowns),
        "pressing the tab that is already showing must speak again - heard {:?}",
        audio.heard()
    );
    drain(&mut audio, crowns);

    // And sixty ticks of a page nobody has touched are silent.
    for _ in 0..60 {
        listen(&mut director, &mut audio, &machine, &game);
    }
    assert!(!audio.one_shot_busy(), "the page repeated its line with nothing pressed");
}

/// **The two lines a screen decides on and cannot play itself.**
///
/// `SaveLoad_Tick` (`0x004AD9F0`) and the merchant's four quantity handlers
/// are the six `Sound_PlayFile` sites whose condition is screen-local state
/// that is gone by the next tick: which box is up when the thumb up's latch is
/// taken, and what the quantity was *before* the step. There is nothing here
/// for a director to diff, so the screen reports the line on
/// [`Game::spoken`] and `Director::listen` plays the newest one.
///
/// The trade half is the interesting one, because the guard is a **crossing**:
/// `if (0 < qty && oldQty < 1)`, identically at all four handlers. Holding the
/// up arrow says it once.
///
/// Ablations, each observed red: drop the `Director` arm and every assertion
/// fails with an empty `heard`; swap `SAVE_GAME` and `LOAD_GAME` and the two
/// box rows fail on the take; change `before < 1` to `before < 0` and the
/// third block's first assertion fails.
#[test]
fn the_save_box_and_the_trade_spinner_speak_what_their_handlers_decided() {
    let Some(_) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();

    // The two boxes, each through its own Enter — `Edit_Confirm`
    // (`0x00401C5B`), which is the same latch the thumb up arms.
    for (mode, want, other) in [
        (l2_game::screens::saveload::Mode::Save, "s040_02.wav", "s040_01.wav"),
        (l2_game::screens::saveload::Mode::Load, "s040_01.wav", "s040_02.wav"),
    ] {
        let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
        let mut game = world();
        let mut machine = Machine::new(APP_ROOT);
        machine.push(ScreenId::Campaign);
        let mut director = audio::Director::new();
        listen(&mut director, &mut audio, &machine, &game);

        machine.push(ScreenId::SaveLoad(mode));
        send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Enter));
        listen(&mut director, &mut audio, &machine, &game);
        assert!(audio.heard().contains(&want), "{mode:?} should speak {want} - heard {:?}", audio.heard());
        assert!(!audio.heard().contains(&other), "{mode:?} also said {other}");
    }

    // The spinner, crossing from a standstill into buying with the up arrow.
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut game = world();
    game.kingdom.realms[1].gold = 10_000;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);

    machine.push(ScreenId::Trade(0, l2_kingdom::trade::Good::Grain as u8));
    send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Up));
    listen(&mut director, &mut audio, &machine, &game);
    assert!(
        audio.heard().contains(&"s068_01.wav"),
        "the up arrow's crossing into buying did not speak - heard {:?}",
        audio.heard()
    );

    // **And it is a crossing, not a value.** Every further step up is silent,
    // which is what `oldQty < 1` says: the count on `Game::spoken` must not
    // move again.
    let spoken = game.spoken.0;
    for _ in 0..5 {
        send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Up));
        listen(&mut director, &mut audio, &machine, &game);
    }
    assert_eq!(game.spoken.0, spoken, "the spinner said its line once per crossing");
}
