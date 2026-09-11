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
use l2_game::Game;

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
/// player would hear the narrator stutter for as long as the panel was open.
///
/// **Ablation:** drop the `&& !self.stack…` half of `opened` and this goes red
/// while every other test in the file stays green.
#[test]
fn a_screen_speaks_once_and_not_once_a_frame() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let assets = Assets::placeholder();
    let mut game = world();
    // **The music has to be off**, and finding that out is the reason this
    // comment exists. The first draft of this test left it on, mixed two
    // buffers and compared them — and passed **with the guard deleted**,
    // because `scroll1.wav` advances 4096 frames between the two `mix` calls
    // and made them differ whatever the effect did. The assertion was true and
    // was about the music. `docs/agents.md`, *a check that passes for an
    // accidental reason*.
    game.prefs.music = false;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &machine, &game);

    machine.push(ScreenId::Supplies(1));
    // One tick, then take the first 4096 frames — which advances the voice's
    // playhead past them. Then fifty-nine more ticks of the *same* screen and
    // the next 4096 frames. `Mixer::play_effect` is retain-then-push, so a
    // second trigger rewinds the clip to zero: if the guard is gone, the second
    // buffer is the FIRST 4096 frames all over again and the two are equal.
    listen(&mut director, &mut audio, &machine, &game);
    let mut buf = vec![0.0f32; 2 * 4096];
    audio.mix(&mut buf);
    for _ in 0..59 {
        listen(&mut director, &mut audio, &machine, &game);
    }
    let mut buf2 = vec![0.0f32; 2 * 4096];
    audio.mix(&mut buf2);
    assert!(
        buf.iter().any(|s| *s != 0.0),
        "the supplies line never reached the mixer, so this test proves nothing"
    );
    assert_ne!(
        buf, buf2,
        "sixty ticks of one screen produced the same first 4096 frames twice: the clip is \
         being re-triggered on every tick, which is the narrator stuttering for as long as \
         the panel is open"
    );
    let _ = &assets;
}
