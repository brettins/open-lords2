//! **Is anything actually driving the audio layer?**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_wiring
//! ```
//!
//! `tests/audio_install.rs` checks that the *tables* name files that ship and
//! that the *mixer* turns a file into samples. Both were green, both still are,
//! and **no music has ever played**: `audio::scene` asked whether the screen at
//! the bottom of the stack is a setup page, and the front end is *pushed
//! under* the campaign rather than replaced by it, so it answered `FrontEnd`
//! for every state a running game can be in. `docs/decisions.md` C116.
//!
//! The test that covered it did this:
//!
//! ```ignore
//! let machine = Machine::new(ScreenId::Campaign);   // "through the real entry point"
//! assert_eq!(scene(&machine, &game), Scene::Campaign { .. });
//! ```
//!
//! — a machine with one screen on it, which the application cannot produce.
//! The assertion was right, the subject was a fixture, and that is
//! `docs/agents.md`'s *a test that drives the picture from the wrong field*
//! with a stack instead of a field.
//!
//! So everything here starts from **the root `main.rs` builds** and reaches
//! every other state through [`Machine::handle`] with real events. Nothing
//! constructs a stack.

use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

/// **The screen `main.rs` roots the machine on.** If this stops being what the
/// application starts from, every test below is testing a state nobody reaches
/// — which is the whole defect this file exists for — so it is named once,
/// here, and `the_root_is_the_one_the_application_starts_on` checks it against
/// `main.rs` itself rather than against memory.
const APP_ROOT: ScreenId = ScreenId::Setup(SetupPage::Title);

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

/// A world with a realm holding one county of fourteen, which is what the
/// England start is and what `Scroll1` is the answer to.
fn world() -> Game {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(14);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].county_count = 1;
    g.player = 1;
    g
}

/// **`main.rs` and `APP_ROOT` must agree**, and they are maintained by
/// different work — `docs/agents.md`'s one reliable shape. Reading the source
/// is crude and it is the only artefact that cannot be fooled: a binary's
/// constants are not importable from an integration test.
#[test]
fn the_root_is_the_one_the_application_starts_on() {
    let src = include_str!("../src/main.rs");
    assert!(
        src.contains("Machine::new(ScreenId::Setup(SetupPage::Title))"),
        "main.rs no longer starts on {APP_ROOT:?}; every test in this file is now \
         asserting about a state the application does not reach. Update APP_ROOT \
         and re-read audio::scene's front-end arm."
    );
}

/// **And that the application still calls the thing these tests exercise.**
///
/// Everything below runs [`audio::Director::listen`]. That is only evidence
/// about the game if the game runs it too, and the game is a binary an
/// integration test cannot link against — so the two are held together by the
/// one artefact both sides share, which is the source. Crude, and it is the
/// difference between testing the feature and testing a library nobody calls:
/// C27, nine instances, and this was the tenth.
#[test]
fn the_application_calls_the_director() {
    let src = include_str!("../src/main.rs");
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

/// **The bug, in the smallest form that reproduces it.**
///
/// Not a constructed stack: the root the application starts on, with a screen
/// put on it the way [`Machine::apply_at`] puts one on for
/// `Transition::Push` — which is what `SetupScreen`'s Start button returns.
///
/// Ablation: restore `if let Some(ScreenId::Setup(_)) = machine.ids().first()`
/// and this goes red on every one of the twenty-odd screens.
#[test]
fn every_in_game_screen_over_the_front_end_is_campaign_music() {
    let game = world();
    // The front end alone is silent, which is the half that was always right.
    let root = Machine::new(APP_ROOT);
    assert_eq!(audio::scene(&root, &game), Scene::FrontEnd);

    // And every screen the game can open over it is not. The list is the
    // screens reachable in a running game; the battlefield is excluded because
    // it is the *other* answer and has its own test below.
    let in_game = [
        ScreenId::Campaign,
        ScreenId::County(1, l2_game::screens::county::Panel::Tax),
        ScreenId::Village(1),
        ScreenId::Court,
        ScreenId::Diplomacy,
        ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Save),
        ScreenId::Options(l2_game::screens::options::Page::Sound),
        ScreenId::BattlePrompt,
        ScreenId::Conquest,
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
}

/// The front end's own pages stay silent however deep the stack gets, which is
/// the clause the fix must not break: `SetupPage::Load` is the title's load
/// screen and is not `ScreenId::SaveLoad`.
#[test]
fn the_front_end_stays_silent_through_all_thirteen_of_its_pages() {
    let game = world();
    for page in SetupPage::ALL {
        let m = Machine::new(ScreenId::Setup(page));
        assert_eq!(audio::scene(&m, &game), Scene::FrontEnd, "{page:?}");
    }
    // And the index, which is ours and is pushed from the title by `I`.
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Index);
    assert_eq!(audio::scene(&m, &game), Scene::FrontEnd, "the index over the title");
    // Over a running game the same screen is not the front end.
    let mut m = Machine::new(APP_ROOT);
    m.push(ScreenId::Campaign);
    m.push(ScreenId::Index);
    assert!(matches!(audio::scene(&m, &game), Scene::Campaign { .. }));
}

/// **`g_battlePhase == 2` outlives the battlefield's three screen ids**, so the
/// scene is read off the whole stack rather than its top: a panel over the
/// field does not stop the battle music.
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

/// **The whole chain, from the title screen to samples**, with no device.
///
/// This is the test the feature did not have. It presses the front end's
/// buttons, lets the machine apply the transitions, derives the scene the way
/// `App::listen` derives it, and then asks the mixer for the buffer the sound
/// card would have been handed. Every link is the real one except the sound
/// card, which is the one link that cannot be in a test.
///
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

    // One tick of the event loop, as `App::tick` runs it: update, then listen.
    // `Director::listen` is the function the game calls, not a copy of it.
    macro_rules! tick {
        () => {{
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
            director.listen(&mut audio, &machine, &game);
        }};
    }

    tick!();
    assert_eq!(audio.music_name(), None, "the title screen is silent");

    // *Single player* — page 1 item 0, the first hotspot, so the selection is
    // already on it. Then *Custom game* — page 2 item 3.
    send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(machine.top_id(), Some(ScreenId::Setup(SetupPage::Options)));
    for _ in 0..3 {
        send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Down));
    }
    send(&mut machine, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(machine.top_id(), Some(ScreenId::Setup(SetupPage::Custom)));

    tick!();
    assert_eq!(audio.music_name(), None, "still the front end, three pages in");

    // *Start* — the third caption at y = 0xC6, the same coordinates
    // `tests/setup.rs` presses.
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

    // And it is audible. A tenth of a second of what the device callback gets.
    let mut buf = vec![0f32; 4_410 * 2];
    audio.mix(&mut buf);
    let peak = buf.iter().fold(0f32, |a, b| a.max(b.abs()));
    assert!(peak > 0.001, "scroll1 is loaded and the mixer produced silence (peak {peak})");
    assert!(buf.iter().all(|s| s.is_finite() && (-1.0..=1.0).contains(s)), "out of range");

    // The music does not restart while the answer has not changed — the
    // property that lets `listen` run sixty times a second.
    let before = audio.music_name();
    for _ in 0..120 {
        tick!();
    }
    assert_eq!(audio.music_name(), before, "the track restarted under a steady scene");
}

/// **How much of the game's audio the engine can actually play, measured.**
///
/// 771 files ship. The number that matters is how many of them any code path
/// can reach, and it was **0** until the fix in `docs/decisions.md`
/// C116. Nobody had ever quoted it, and the first time it *was* quoted
/// — in three documents, by typing — it was wrong: `battle5.wav` is in the
/// table, ships, decodes, and **cannot be reached**, because the counter that
/// selects it is `DAT_0057A0F0`, the third battle mode `BattleKind` does not
/// name.
///
/// So the set is collected by *driving every scene the policy can produce*
/// through the real `follow` and reading `Audio::heard` — what travelled the
/// road, which is `docs/agents.md`'s standard for a field being tested at all.
/// A new call site makes this go red with the name it added, which is the only
/// way this count stays true.
#[test]
fn eleven_of_the_installs_771_sounds_are_reachable() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);
    assert_eq!(audio.file_count(), 771, "the install's sound count moved");

    // Every rung of `Music_StartCampaign`'s ladder, both clauses.
    for (counties, share) in [(1, 7), (2, 7), (2, 8), (2, 15), (2, 29), (2, 43)] {
        audio.follow(Scene::Campaign { county_count: counties, share_of_map_pct: share });
    }
    // Both battle pairs, twice each, so both sides of both toggles are taken.
    for _ in 0..2 {
        for kind in [
            l2_game::audio::track::BattleKind::Field,
            l2_game::audio::track::BattleKind::Siege,
        ] {
            audio.follow(Scene::Campaign { county_count: 1, share_of_map_pct: 7 });
            audio.follow(Scene::Battle(kind));
        }
    }
    // And the two fanfares `Director::listen` fires, by the same names it uses.
    audio.play_effect(l2_game::audio::names::fanfare::MESSAGE);
    audio.play_effect(l2_game::audio::names::fanfare::BATTLE);

    assert_eq!(
        audio.heard(),
        [
            "battle1.wav",
            "battle2.wav",
            "battle3.wav",
            "battle4.wav",
            "ff_batl.wav",
            "ff_msg.wav",
            "scroll1.wav",
            "scroll2.wav",
            "scroll3.wav",
            "scroll4.wav",
            "scroll5.wav",
        ],
        "the set of sounds this engine can reach has changed. If a call site was \
         ADDED this is good news and the number in crates/l2-game/src/audio/mod.rs, \
         docs/mechanics.md and docs/decisions.md C116 moves with it."
    );
    assert_eq!(audio.heard().len(), 11, "11 of 771");

    // `battle5.wav` ships and decodes; nothing can ask for it. That is not a
    // gap in the wiring, it is `DAT_0057A0F0` being unidentified, and
    // `BattleKind` refusing to guess at a third mode is why.
    assert!(!audio.heard().contains(&"battle5.wav"));
}

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

    // And back on, which the original re-derives rather than resuming.
    game.prefs.music = true;
    listen!();
    assert_eq!(audio.music_name().as_deref(), Some("scroll1.wav"), "Music: On did not resume");
}
