#![allow(unused_imports)]
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
///
/// `main.rs` is a binary and cannot be called, so the source is read — the one
/// artefact the application and this test share, as `tests/audio_wiring.rs`
/// does for the director. Ablation: delete the `start_up` call from `main.rs`.
#[test]
fn the_application_opens_on_the_intro_over_the_title_page() {
    let src = include_str!("../src/main.rs");
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

/// **`Smk_OnFinished`'s start-up chain, and where each film returns.** A skip
/// is `Smk_OnFinished` too, so this is also what a click during the intro does.
///
/// # `g_smkReturnScreen`, off all eight call sites
///
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
    // `Smk_Play(castle1.smk + level * 0x10, 0x9E, 0x14, 0, 0)` — screen 0.
    assert_eq!(Film::Castle(2).then(), Goto(ScreenId::Campaign));
    // `Smk_Play` failing never reaches `Smk_OnFinished`, so it never chains —
    // but it does write `g_screenId = returnScreen`, so it goes to the same
    // place.
    assert_eq!(Film::Intro.on_failure(), Pop);
    assert_eq!(Film::Castle(2).on_failure(), Goto(ScreenId::Campaign));
    // The positions, off the seven call sites.
    assert_eq!(Film::Intro.at(), (40, 80));
    assert_eq!(Film::ImpTitle.at(), (80, 80));
    assert_eq!(Film::Credits.at(), (0, 0));
    assert_eq!(Film::LordsOfMagic.at(), (70, 80));
    assert_eq!(Film::Castle(0).at(), (158, 20));
    assert_eq!(Film::Battle { file: "bat_win1.smk" }.at(), (39, 73));
}

/// **A film the install does not have** holds for one tick — so the audio
/// layer sees the music stopped for it — and then goes where `Smk_Play` sends
/// a failure: back, with no chain. Ablation: make `State::Failed` return
/// `Film::then` and the logo is asked for.
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

/// **A film that could not open is not there.** `Smk_Play` failing puts
/// `g_screenId` straight back, so input belongs to the screen underneath even
/// while ours holds the film's screen for its one tick. Here that is the title
/// page's own Escape, which leaves the front end. Ablation: return `Stay`
/// instead of `Pass` for `State::Failed`.
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

/// **"Lords of Magic?" is hotspot id 4, the third record, kind 3.** Its press
/// selects and its release plays `lom.smk`. Ablation: drop the `Event::Release`
/// arm from `SetupScreen::handle`.
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
    // It returns to the page it was played from.
    tick(&mut m, &mut g, &a);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(setup::SetupPage::Title)));
}

/// **`CastleBuild_Confirm`'s tail.** An order with animations on plays
/// `castle<n>.smk` over the chooser, and the chooser goes when the film does,
/// because `Smk_Play` was told to return to screen 0. With animations off the
/// map comes straight back.
///
/// **The order and the film both arrive on the twentieth frame**, not on the
/// press — see [`order_the_castle`]. Ablations: delete the `Push` in `confirm`
/// declare the thumbs `Press` in `castle::widgets`
/// and the screen's own debug assertion fires on the press; make
/// [`Film::then`](movie::Film::then)'s castle arm a `Pop` again and the chooser
/// is still on the stack when the film goes.
#[test]
fn an_ordered_castle_plays_its_film_over_the_chooser() {
    let a = Assets::placeholder();
    let (mut g, mut m) = castle_world();
    assert_eq!(g.kingdom.counties[1].castle_type, 0, "nothing is ordered by the press");
    order_the_castle(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.counties[1].castle_type, 1, "the order stands on the twentieth frame");
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Castle(1), ScreenId::Movie(Film::Castle(0))],
        "the film plays over the chooser's preview well"
    );
    for _ in 0..3 {
        tick(&mut m, &mut g, &a);
    }
    // **The end of the film is the map**, and the chooser goes with it.
    // `Smk_Play(…, 0, 0)`: screen `0` is the campaign map, and the original has
    // one `g_screenId` byte with nothing to leave the chooser on. This used to
    // read *"no `Movie` on the stack"* and could not say more, because ours
    // popped the film and left the chooser to pop itself on its next `update` —
    // which `Machine::update`'s `run_tips` could take away from it first.
    //
    // The tip the chooser posted during its twenty press frames may be *up* by
    // now, over the map, and that is the original: `FUN_00476E21`'s record
    // outlives the screen that queued it and `Msg_Pump` opens it on `0x00`. So
    // the assertion is about the chooser and the film, and the base.
    assert_eq!(m.ids()[0], ScreenId::Campaign, "the film ends on the map: {:?}", m.ids());
    assert!(
        !m.ids().iter().any(|id| matches!(id, ScreenId::Movie(_) | ScreenId::Castle(_))),
        "and takes the chooser with it: {:?}",
        m.ids()
    );

    let (mut g, mut m) = castle_world();
    g.prefs.animations = false;
    order_the_castle(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "no film with animations off");
}

/// **`FUN_00475B41`** — the later the fall, the worse the end. Every name is a
/// literal out of `.rdata`. Ablation: swap two of the year thresholds.
#[test]
fn a_fallen_lords_film_is_chosen_by_his_title_and_how_long_the_game_has_run() {
    let mut g = realms();
    let at = |g: &mut Game, years: i32, realm: u8| {
        g.kingdom.year = movie::FIRST_YEAR + years;
        movie::ending_film(g, realm, 194)
    };
    // Realm 3 is the Baron (lord 2), a computer lord.
    assert_eq!(at(&mut g, 1, 3), "cart_brn.smk");
    assert_eq!(at(&mut g, 6, 3), "pill_brn.smk");
    assert_eq!(at(&mut g, 12, 3), "jail.smk");
    assert_eq!(at(&mut g, 18, 3), "hang.smk");
    assert_eq!(at(&mut g, 24, 3), "axmen.smk");
    // The Countess has no cart film and the Bishop no pillory.
    assert_eq!(at(&mut g, 1, 4), "pill_cts.smk");
    assert_eq!(at(&mut g, 7, 5), "cart_bsp.smk");
    // A human is never carted or pilloried, and lasts longer before the axe.
    assert_eq!(at(&mut g, 1, 1), "jail.smk");
    assert_eq!(at(&mut g, 24, 1), "hang.smk");
    assert_eq!(at(&mut g, 32, 1), "axmen.smk");
    assert_eq!(movie::ending_film(&g, 3, 225), "win_game.smk", "group 0xE1");
}

/// **The animated ending**, played: the window closes itself on the frame it
/// opens and the fall plays in its place; the victory it enqueued follows as
/// a film of its own, and that one returns to the conquest screen `Msg_Dismiss`
/// entered. Ablation: delete the `animate` call from `MessageScreen::update` and
/// the message stays on screen.
#[test]
fn an_ending_with_animations_on_is_a_film_and_the_victory_after_it_is_another() {
    let a = Assets::placeholder();
    let mut g = realms();
    g.campaign.ranking = l2_kingdom::victory::Ranking { opponents_remaining: 0, ..Default::default() };
    let player = g.player;
    assert!(g.messages.enqueue(ending(3, 194), player));
    let mut m = Machine::new(ScreenId::Campaign);

    tick(&mut m, &mut g, &a);
    let Some(Film::Ending { file, record, game_over }) = film_on_top(&m) else {
        panic!("no film: {:?}", m.ids());
    };
    assert_eq!((file, record.group, game_over), ("cart_brn.smk", 194, false));
    assert!(g.messages.open().is_none(), "Msg_Dismiss ran inside the draw");
    assert_eq!(g.messages.waiting().iter().map(|r| r.group).collect::<Vec<_>>(), vec![225]);

    let mut victory = None;
    for _ in 0..12 {
        tick(&mut m, &mut g, &a);
        if let Some(f @ Film::Ending { record, .. }) = film_on_top(&m) {
            if record.group == 225 {
                victory = Some(f);
                break;
            }
        }
    }
    let Some(Film::Ending { file, game_over, .. }) = victory else { panic!("no victory film") };
    assert_eq!((file, game_over), ("win_game.smk", true));
    tick(&mut m, &mut g, &a);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "Smk_Play was told to return to 0x1C");
    assert_eq!(g.outcome(), l2_kingdom::victory::Outcome::Won);
}

/// **The animated capture's rotation, `DAT_00553ED4`**: stepped before use, so
/// the first capture of a session is `cap_cty2.smk`.
///
/// **No player reaches this.** Nothing in this engine posts a category-`0x0D`
/// letter — `County_ChangeOwner`'s nine are not built — so the record here is
/// posted by hand, and `docs/audio.json` keeps its two sound sites `blocked`
/// for exactly that reason.
#[test]
fn a_capture_letter_would_play_the_capture_films_in_rotation() {
    let a = Assets::placeholder();
    let mut g = realms();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut takes = Vec::new();
    for _ in 0..3 {
        let player = g.player;
        let r = Record { to: 0, group: 0x75, category: category::CAPTURE, county: 2, ..Record::default() };
        assert!(g.messages.enqueue(r, player));
        tick(&mut m, &mut g, &a);
        match film_on_top(&m) {
            Some(f @ Film::Capture { take, .. }) => {
                takes.push((take, f.file()));
            }
            other => panic!("{other:?}"),
        }
        tick(&mut m, &mut g, &a);
        tick(&mut m, &mut g, &a);
        assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    }
    assert_eq!(takes, vec![(1, "cap_cty2.smk"), (2, "cap_cty3.smk"), (0, "cap_cty1.smk")]);

    // **`Msg_DrawWindow`'s guard, the other way.** `g_optAnimations == 0` never
    // reaches the taller window, so the letter is the ordinary one and no film
    // plays. Ablation: drop `!game.prefs.animations` from `message::animate` —
    // red here, and the capture half above passes either way.
    g.prefs.animations = false;
    let player = g.player;
    let r = Record { to: 0, group: 0x75, category: category::CAPTURE, county: 2, ..Record::default() };
    assert!(g.messages.enqueue(r, player));
    tick(&mut m, &mut g, &a);
    assert_eq!(film_on_top(&m), None, "a capture letter played a film with animations off");
    assert_eq!(m.top_id(), Some(ScreenId::Message), "and the ordinary window is up instead");
}

/// **`Battle_CheckOutcome`'s film**, from `g_battleOutcome * 4 +
/// DAT_0053F084`. Our attacker is `SIDE_B`. The siege row is the one this
/// branch corrected: the player who **held** his castle is outcome 4, not 2.
/// Ablation: collapse `outcome_banner`'s siege arms back to two.
#[test]
fn a_decided_battle_plays_the_film_for_its_outcome_in_rotation() {
    let a = Assets::placeholder();
    let (mut g, mut m) = decided(1, None, l2_sim::SIDE_B);
    tick(&mut m, &mut g, &a);
    assert_eq!(film_on_top(&m), Some(Film::Battle { file: "bat_win1.smk" }), "won, take 0");
    // A film that does not open leaves the banner its full wait.
    tick(&mut m, &mut g, &a);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
    let live = g.battle.as_ref().expect("still up");
    assert!(live.outcome_ticks < l2_game::battlefield::OUTCOME_FRAMES, "the banner keeps its wait");
    assert_eq!(g.films.battle, 1, "DAT_0053F084 stepped");

    // Held the castle: a siege, the player the garrison, the garrison won.
    let (mut g, mut m) = decided(2, Some(3), l2_sim::SIDE_A);
    g.films.battle = 1;
    tick(&mut m, &mut g, &a);
    assert_eq!(film_on_top(&m), Some(Film::Battle { file: "sge_win2.smk" }), "outcome 4, take 1");

    // No film with animations off.
    let (mut g, mut m) = decided(1, None, l2_sim::SIDE_A);
    g.prefs.animations = false;
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
}

// ================================================================== install

/// **Every film a trigger names ships**
/// played are named. Ablation: misspell a table entry.
#[test]
fn every_trigger_names_a_film_the_install_ships() {
    let (_p, a) = install!();
    let mut played: Vec<&str> = vec!["intro.smk", "imptitle.smk", "credits.smk", "lom.smk"];
    played.extend(movie::CASTLE_FILMS);
    played.extend(movie::CAPTURE_FILMS);
    played.extend(movie::ENDING_FILMS.iter().flatten());
    played.push(movie::VICTORY_FILM);
    played.extend(movie::BATTLE_FILMS.iter().flatten());
    for name in &played {
        assert!(a.films.path(name).is_some(), "{name} is asked for and not shipped");
    }
    let unplayed: Vec<&str> = a.films.names().filter(|n| !played.contains(n)).collect();
    assert_eq!(
        unplayed,
        vec![
            // The one file `Lords2.exe` never names: it asks for `axmen.smk`.
            "axemen.smk",
            // `DAT_0057A0F0`'s table, the third battle mode.
            "bat_los5.smk",
            "bat_los6.smk",
            "bat_win5.smk",
            "bat_win6.smk",
            "cas_los3.smk",
            "cas_win3.smk",
        ],
    );
}

