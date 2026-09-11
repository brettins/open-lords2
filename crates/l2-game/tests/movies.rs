//! **The films, played — every place `Smk_Play` is called, and every way out.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test movies
//! ```
//!
//! Two halves, and the split is the install. Without it every film fails to
//! open, which is still a real path — `Smk_Play`'s callers each have an arm for
//! it — so *which* film each trigger asks for, and where the machine goes when
//! it cannot have it, run everywhere. With it the films open, and the tests ask
//! what a player sees and hears: the gestures that end one, the tick it ends
//! on, where it is drawn, the palette the screen runs under, and the music bed
//! stopping for it and starting over after it.
//!
//! Everything goes through [`Machine::handle`] and [`Machine::update`], and the
//! sound through [`audio::Director::listen`] — the functions the game calls.
//!
//! **Ablations run on this file**, each named at its test: delete the line the
//! assertion is about and the test goes red.

use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message::{category, Record};
use l2_game::movie::{self, Film};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, setup};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn centre(r: l2_game::input::Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

/// The install, mounted, and its assets — or `None`.
fn installed() -> Option<(l2_mods::Platform, Assets)> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().ok()?;
    let assets = Assets::load(&platform.vfs).ok()?;
    if assets.films.is_empty() {
        return None;
    }
    Some((platform, assets))
}

macro_rules! install {
    () => {
        match installed() {
            Some(x) => x,
            None => l2_testkit::skip!("no install with films (the DOS release ships none)"),
        }
    };
}

/// Five realms, realm 1 the human, as `tests/messages.rs` has them — **with
/// animations on**, which is the original's default and ours.
fn realms() -> Game {
    let mut g = Game::new(0xF11A);
    g.player = 1;
    g.kingdom.set_county_count(6);
    for realm in 1..=5usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 1;
    g.kingdom.year = movie::FIRST_YEAR + 1;
    assert!(g.prefs.animations, "animations default to on, as FUN_004AF35E sets them");
    g
}

fn ending(from: u8, group: u16) -> Record {
    Record { to: 0, from, group, category: category::ENDING, ..Record::default() }
}

fn film_on_top(m: &Machine) -> Option<Film> {
    match m.top_id() {
        Some(ScreenId::Movie(f)) => Some(f),
        _ => None,
    }
}

// =============================================================== no install

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
#[test]
fn the_start_up_films_chain_intro_logo_credits_and_nothing_else_does() {
    use l2_game::screen::Transition::*;
    assert_eq!(Film::Intro.then(), Replace(ScreenId::Movie(Film::ImpTitle)));
    assert_eq!(Film::ImpTitle.then(), Replace(ScreenId::Movie(Film::Credits)));
    assert_eq!(Film::Credits.then(), Pop);
    assert_eq!(Film::LordsOfMagic.then(), Pop);
    assert_eq!(Film::Castle(2).then(), Pop);
    // `Smk_Play` failing never reaches `Smk_OnFinished`, so it never chains.
    assert_eq!(Film::Intro.on_failure(), Pop);
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

fn castle_world() -> (Game, Machine) {
    let mut g = Game::new(11);
    g.kingdom.set_county_count(2);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].wood = 5_000;
    g.kingdom.realms[1].stone = 5_000;
    g.player = 1;
    g.selected = 1;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Castle(1));
    (g, m)
}

/// **`CastleBuild_Confirm`'s tail.** An order with animations on plays
/// `castle<n>.smk` over the chooser, and the chooser goes when the film does,
/// because `Smk_Play` was told to return to screen 0. With animations off the
/// map comes straight back. Ablation: delete the `Push` in `confirm`.
#[test]
fn an_ordered_castle_plays_its_film_over_the_chooser() {
    let a = Assets::placeholder();
    let (mut g, mut m) = castle_world();
    send(&mut m, &mut g, &a, Event::Click { x: castle::OK.x + 4, y: castle::OK.y + 4 });
    assert_eq!(g.kingdom.counties[1].castle_type, 1, "the order stands first");
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Castle(1), ScreenId::Movie(Film::Castle(0))],
        "the film plays over the chooser's preview well"
    );
    for _ in 0..3 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "and both are gone after it");

    let (mut g, mut m) = castle_world();
    g.prefs.animations = false;
    send(&mut m, &mut g, &a, Event::Click { x: castle::OK.x + 4, y: castle::OK.y + 4 });
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
}

fn army(g: &mut Game, owner: u8, county: u8, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = 300;
    u.troops[TroopType::Peasant.index()] = 300;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// A battlefield whose battle has just been decided for `winner`.
fn decided(attacker_owner: u8, castle: Option<u8>, winner: l2_sim::Side) -> (Game, Machine) {
    let mut g = realms();
    let attacker = army(&mut g, attacker_owner, 1, (10, 10));
    let defender = army(&mut g, 3 - attacker_owner, 2, (11, 10));
    let runner = l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 7)
        .expect("two armies");
    let mut live = l2_game::battlefield::LiveBattle::new(runner, attacker, defender, 2, castle, 1, 1);
    live.mode = l2_game::battlefield::Mode::Outcome;
    live.conclusion = Some(l2_sim::runner::Conclusion { winner, cause: l2_sim::End::Annihilation });
    g.battle = Some(Box::new(live));
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Battlefield);
    (g, m)
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

/// **Every film a trigger names ships**, and the ones that ship and are never
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

/// **The four ways out of screen `0x22`, as a player makes them.** A release of
/// either button, or any key; never a press or a move. And during start-up each
/// one moves on a film. Ablation: make the left-release arm a `Click` arm.
#[test]
fn a_film_ends_on_a_release_of_either_button_or_any_key_and_not_on_a_press() {
    let (_p, a) = install!();
    let mut g = Game::new(1);

    // `Smk_Play` opens the film inside the call, so a release on the very next
    // event — before any tick — already meets a playing intro.
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    movie::start_up(&mut m);
    send(&mut m, &mut g, &a, Event::Release { x: 1, y: 1 });
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::ImpTitle)), "skipped before a tick");

    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    movie::start_up(&mut m);
    tick(&mut m, &mut g, &a);
    for e in [
        Event::Click { x: 300, y: 200 },
        Event::DoubleClick { x: 300, y: 200 },
        Event::Pointer { x: 10, y: 10 },
        Event::Text('x'),
    ] {
        send(&mut m, &mut g, &a, e);
        assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::Intro)), "{e:?} ended the intro");
    }
    send(&mut m, &mut g, &a, Event::Release { x: 300, y: 200 });
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::ImpTitle)), "a left release: the logo");
    tick(&mut m, &mut g, &a);
    send(&mut m, &mut g, &a, Event::RightClick { x: 300, y: 200 });
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::Credits)), "a right release: the credits");
    tick(&mut m, &mut g, &a);
    send(&mut m, &mut g, &a, Event::KeyDown(Key::Char('Q')));
    assert_eq!(m.ids(), vec![ScreenId::Setup(setup::SetupPage::Title)], "any key: the title page");
}

/// **The castle film ignores the release of the click that ordered it**, and
/// only that one. See `MovieScreen::new`. Ablation: set `swallow_release` false.
#[test]
fn the_castle_film_is_not_skipped_by_the_click_that_ordered_it() {
    let (_p, a) = install!();
    let (mut g, mut m) = castle_world();
    let ok = (castle::OK.x + 4, castle::OK.y + 4);
    send(&mut m, &mut g, &a, Event::Click { x: ok.0, y: ok.1 });
    tick(&mut m, &mut g, &a);
    send(&mut m, &mut g, &a, Event::Release { x: ok.0, y: ok.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::Castle(0))), "the ordering click's release");
    send(&mut m, &mut g, &a, Event::Release { x: ok.0, y: ok.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Castle(1)), "a second release skips");
    tick(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Campaign]);
}

fn decode_frame0(a: &Assets, name: &str) -> l2_smk::Decoder {
    let smk = a.films.open(name).expect("the film");
    let mut d = smk.decoder();
    d.next_frame(&smk).expect("frame 0");
    d
}

/// **`SmackToBuffer` at `Smk_Play`'s position, on black, under the film's
/// palette.** The logo is 500 × 292 at (80, 80); the intro is stored 144 rows
/// tall and drawn 288, each row twice. Ablation: pass `y_scale` 1, or drop
/// `live_palette`.
#[test]
fn a_film_is_drawn_where_smk_play_puts_it_under_its_own_palette() {
    let (_p, a) = install!();
    let mut g = Game::new(1);

    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    m.push(ScreenId::Movie(Film::ImpTitle));
    tick(&mut m, &mut g, &a);
    let mut canvas = l2_view::Canvas::screen();
    m.draw(&Ctx { game: &mut g, assets: &a }, &mut canvas);
    let d = decode_frame0(&a, "imptitle.smk");
    let (w, h) = d.size();
    assert_eq!((w, h), (500, 292));
    for y in 0..h {
        for x in 0..w {
            assert_eq!(canvas.at(80 + x, 80 + y), d.pixels()[y * w + x], "logo pixel ({x}, {y})");
        }
    }
    assert_eq!(canvas.at(79, 80), 0, "cleared round it");
    assert_eq!(canvas.at(580, 372), 0);
    assert_eq!(
        m.live_palette().map(|p| *p.entries()),
        Some(*d.palette()),
        "the whole screen runs under the film's palette"
    );

    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    movie::start_up(&mut m);
    tick(&mut m, &mut g, &a);
    let mut canvas = l2_view::Canvas::screen();
    m.draw(&Ctx { game: &mut g, assets: &a }, &mut canvas);
    let d = decode_frame0(&a, "intro.smk");
    let (w, h) = d.size();
    assert_eq!((w, h), (560, 144));
    for row in 0..h {
        for x in 0..w {
            let want = d.pixels()[row * w + x];
            assert_eq!(canvas.at(40 + x, 80 + 2 * row), want, "intro ({x}, {row}) first copy");
            assert_eq!(canvas.at(40 + x, 81 + 2 * row), want, "intro ({x}, {row}) second copy");
        }
    }
}

/// **The last frame comes due and is never drawn.** `imptitle.smk` is 118
/// frames at 83.33 ms; frame 117 falls due at 117 × 8333 ≥ t × 1600, which is
/// tick 610. Ablation: finish on `due >= frames`.
#[test]
fn a_film_ends_on_the_tick_its_last_frame_falls_due() {
    let (_p, a) = install!();
    let mut g = Game::new(1);
    let mut m = Machine::new(ScreenId::Setup(setup::SetupPage::Title));
    m.push(ScreenId::Movie(Film::ImpTitle));
    tick(&mut m, &mut g, &a); // opens, frame 0
    for _ in 0..609 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::ImpTitle)), "609 ticks in, still playing");
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Movie(Film::Credits)), "tick 610 ends it");
}

/// One event-loop tick: update, then listen, as `App::tick` runs them.
fn tick_and_listen(m: &mut Machine, g: &mut Game, a: &Assets, audio: &mut Audio, d: &mut audio::Director) {
    tick(m, g, a);
    d.listen(audio, m, g);
}

fn buffer(audio: &mut Audio) -> Vec<f32> {
    let mut b = vec![0f32; 2 * 2048];
    audio.mix(&mut b);
    b
}

/// **A film stops the bed and the bed starts over after it** — the five
/// restart sites `docs/audio.json` now calls reproduced, heard rather than
/// read. The castle film's track plays in between. Ablation: make
/// `Scene::Film` answer the campaign track, and the music never stops.
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
