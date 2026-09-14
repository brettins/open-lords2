#![allow(unused_imports)]
use super::*;
use super::title_and_screens::*;
use super::panels_and_sites::*;
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

/// **The standings page says which category you are looking at** —
/// `FUN_004B3994(DAT_0055CE7C)`, whose two callers are the court's *Greatest
/// nobles* button (`FUN_004351C4`) and one of the page's seven tabs
/// (`FUN_0043524E`). Both are unconditional, and that is the whole design of
/// the edge: `Game::nobles_spoken` is a counter both bump, so a tab pressed
/// twice speaks twice.
///
/// **Ablation, run:** make the director diff `game.nobles_category`
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


