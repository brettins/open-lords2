#![allow(unused_imports)]
use super::*;
use super::triggers::*;
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

/// **The release of the click that ordered the castle cannot skip the film,
/// because when it arrives
///
/// `CastleBuild_Confirm` (`0x00436B59`) is record 0 of `g_castleBuildWidgets`
/// and the record is `Widget_Test` kind 5, so `Smk_Play` runs **twenty frames
/// after the press** — by which time the button has been let go and its release
/// was answered by the chooser. `CastleBuild_Confirm`'s own first call
/// (`FUN_004B18E3`) throws that click away too.
///
/// **This is what replaced a compensation.** `MovieScreen` used to carry a
/// `swallow_release` flag that made the castle film ignore one left release,
/// because `castle.rs` confirmed on the press and the release of that very
/// click then reached the film. The flag is gone: the timing it was papering
/// over is the original's now.
///
/// Ablations: move the release after the twentieth tick and the film is skipped
/// on the line that says it is not; declare the thumbs `Press` in
/// `castle::widgets` and the chooser answers on the press, which the screen's
/// own debug assertion refuses.
#[test]
fn the_release_of_the_ordering_click_is_answered_by_the_chooser_not_the_film() {
    let (_p, a) = install!();
    let (mut g, mut m) = castle_world();
    let ok = (castle::OK.x + 4, castle::OK.y + 4);
    order_the_castle(&mut m, &mut g, &a);
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Movie(Film::Castle(0))),
        "the twentieth frame orders the castle and plays the film"
    );

    // The next release is an ordinary one, and the film answers it.
    send(&mut m, &mut g, &a, Event::Release { x: ok.0, y: ok.1 });
    // **A skip is `Smk_OnFinished`, so it goes where the end of the film goes**
    // — `g_screenId = g_smkReturnScreen`, and `CastleBuild_Confirm` passed `0`.
    // This used to assert `Castle(1)`, which was our pop and not the original.
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "a skip lands where the end lands");
}

/// **`SmackToBuffer` at `Smk_Play`'s position, on black, under the film's
/// palette.** The logo is 500 × 292 at (80, 80); the intro is stored 144 rows
/// tall and drawn 288, **the even rows written and the odd ones left black** —
/// `_SmackToBuffer@28` (`0x403AF0`) with bit `0x10`. Ablation: copy the row, or
/// drop `live_palette`.
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
            assert_eq!(canvas.at(40 + x, 80 + 2 * row), want, "intro ({x}, {row}) written");
            assert_eq!(canvas.at(40 + x, 81 + 2 * row), 0, "intro ({x}, {row}) twin left black");
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

/// **The player's defect, with the real film: a castle film longer than twenty
/// frames used to strand him on the chooser under a tip.**
///
/// `castle1.smk` is 521 frames, so it runs far past `tip::DELAY` (`0x14`). The
/// old shape popped the film and left the chooser to pop itself on its next
/// `update`; [`Machine::update`] runs `run_tips` and `pump_messages` first, so
/// the castle advisor tip (`Tip_Update`'s `0x1B` arm) seated itself the moment
/// Measured at tick 523.
///
/// **Tips are deliberately left on**, because that is the whole point: with
/// `g_smkReturnScreen` built as a transition, the chooser is gone before the
/// `0x1B` arm can ever see it, so no tip of that screen's can seat itself.
///
/// Ablation: make [`Film::then`](movie::Film::then)'s castle arm `Pop` again
/// and the stack ends `[Campaign, Castle(1), Tip]`.
#[test]
fn a_real_castle_film_ends_on_the_map_however_long_it_runs() {
    let (_p, a) = install!();
    let (mut g, mut m) = castle_world();
    assert!(g.prefs.tip_screens, "the defect needs the advisor tips a new game has");
    m.push(ScreenId::Movie(Film::Castle(0)));
    let smk = a.films.open("castle1.smk").expect("the install's castle film");
    let frames = smk.frames();
    // `Player::tick` paces the film against [`l2_game::TICK_MS`], so the film is
    // this many ticks long — and then a tail for the machine after it.
    let ticks = frames as u64 * smk.header().period_10us() as u64
        / (l2_game::TICK_MS as u64 * 100)
        + 4 * l2_game::tip::DELAY as u64;
    assert!(
        ticks > l2_game::tip::DELAY as u64,
        "{frames} frames is {ticks} ticks, long enough to strand it"
    );

    for _ in 0..ticks {
        tick(&mut m, &mut g, &a);
        if m.ids() == vec![ScreenId::Campaign] {
            return;
        }
    }
    panic!("stranded off the map: {:?}", m.ids());
}

/// **The picture against the sound, on a wall clock that overshoots.**
///
/// `Smk_PlayLoop` (`0x0042DBC7`) steps a film only when `SmackWait` answers 0,
/// and `_SmackWait@4` is a `timeGetTime` comparison — the original's film runs
/// on real milliseconds, and so does the sound track a device plays out.
/// Ours steps on ticks, which is only the same clock while a tick is a true
/// 16 ms: [`l2_game::clock::Ticker`].
///
/// So this drives a real film's [`movie::Player`] over a synthetic wall clock
/// whose every wake overshoots its deadline by 400 µs — measured, with winit
/// 0.30's own waitable timer, as the middle of what a 16 ms wait really costs
/// here — and asks where the picture is when the sound has reached a given
/// sample. **One frame of tolerance**, which is 83 ms.
///
/// Ablation: schedule the ticks the way `main.rs` used to — a deadline of
/// `now + TICK` measured after the wait — and the same film falls behind by
/// **a fixed fraction of its own length**, which is the second half of this
/// test and the shape of the defect. It is a rate,
/// is 3.6 s long and ends 71 ms out, under one of its frames, while the intro
/// is 131 s long and ends three seconds out. That is why the complaint was
/// about the long films.
#[test]
fn a_film_keeps_step_with_its_own_sound_track() {
    let (_p, a) = install!();
    const OVER_NS: u64 = 400_000;
    let tick_ns = l2_game::clock::TICK_NS;

    for name in ["intro.smk", "credits.smk", "castle5.smk", "bat_win5.smk"] {
        let Some(smk) = a.films.open(name) else { continue };
        let track = smk.header().track(0).expect("every film here has track 0");
        let samples = smk.audio(0).expect("track 0 decodes").len() / track.channels() as usize;
        let sound_ns = samples as u64 * 1_000_000_000 / track.rate as u64;
        let period_ns = smk.header().period_10us() as u64 * 10_000;
        let frames = smk.frames() as u64;

        // The clock, and the film on it.
        let mut ticker = l2_game::clock::Ticker::new();
        let mut player = movie::Player::open(smk, false).expect("frame 0");
        let (mut now, mut wall_at_end) = (0u64, None);
        while now <= sound_ns {
            for _ in 0..ticker.due(now) {
                if wall_at_end.is_none() && player.tick() == Ok(movie::Step::Finished) {
                    wall_at_end = Some(now);
                }
            }
            // Where the picture is, against where the sound is.
            let shown = player.frame() as u64 * period_ns;
            assert!(
                shown.abs_diff(now.min((frames - 1) * period_ns)) <= period_ns,
                "{name}: at {} ms of sound the picture was at {} ms",
                now / 1_000_000,
                shown / 1_000_000
            );
            now = ticker.next_ns().unwrap_or(now + tick_ns) + OVER_NS;
        }
        // `Smk_PlayLoop` decodes the last frame and does not draw it, so a film
        // ends one frame short of its sound.
        let end = wall_at_end.expect("the film ended");
        assert!(
            end.abs_diff(sound_ns) <= 2 * period_ns,
            "{name}: picture ended at {} ms, sound at {} ms",
            end / 1_000_000,
            sound_ns / 1_000_000
        );

        // **The ablation**: the deadline measured from after the wait.
        let smk = a.films.open(name).expect("still there");
        let mut player = movie::Player::open(smk, false).expect("frame 0");
        let (mut now, mut next) = (0u64, 0u64);
        while now <= sound_ns {
            if now >= next {
                next = now + tick_ns;
                let _ = player.tick();
            }
            now = next + OVER_NS;
        }
        let behind = (frames - 1 - player.frame() as u64) * period_ns;
        // 400 µs lost per 16 ms tick is 2.4 % of every film; 1.5 % is the floor
        // that survives the frame the drift is quantised to.
        assert!(
            behind * 1000 >= sound_ns * 15,
            "{name}: the old rule was only {} ms behind {} ms of sound, so this test proves nothing",
            behind / 1_000_000,
            sound_ns / 1_000_000
        );
    }
}

/// **`Intro_DrawSubtitle` (`0x0041A166`) draws nothing for an English
/// `L2.eng`**, and this install's is English.
///
/// The function's first act is a `strcmp` of group 300 index 0 against
/// `"English"`; equal, it returns before it reaches group 301's eleven-line
/// ladder. So the shipped intro carries no text and the narration is the sound
/// track alone. `docs/formats/smk.md`.
///
/// Asserted where the player would see it: the canvas, over the first four
/// hundred ticks of the film — every cue up to frame `0x117` — in the rows
/// `Ui_DrawCentred(0x12D, n, 0, 400, …)` and its second line at `0x1A0` would
/// paint, which are below the picture and therefore the black
/// `FUN_004B1867` left.
///
/// **Ablation:** the tail of this test is a `Subtitles` cued the other way,
/// which fires at frame `0x1D`. Take the `is_english` test out of
/// `MovieScreen::open` and the rows fill.
#[test]
fn the_english_intro_carries_no_subtitles() {
    let (_p, a) = install!();
    assert!(
        movie::is_english(a.shell.text(300, 0)),
        "this install's L2.eng group 300/0 is the language tag FUN_0041A166 compares"
    );

    let mut g = realms();
    let mut m = Machine::new(ScreenId::Movie(Film::Intro));
    let mut canvas = l2_view::Canvas::screen();
    // The film is 40 x 80 and its rows stop above 400, so the two subtitle
    // rows are the cleared back buffer and nothing else.
    for tick in 0..400 {
        {
            let mut ctx = Ctx { game: &mut g, assets: &a };
            m.update(&mut ctx);
        }
        if m.top_id() != Some(ScreenId::Movie(Film::Intro)) {
            break;
        }
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut canvas);
        for y in 400..432usize {
            for x in 0..640usize {
                assert_eq!(canvas.at(x, y), 0, "tick {tick}: ink at ({x}, {y}) under the intro");
            }
        }
    }

    // The ablation, and the proof the ladder is alive: cued, frame `0x1D` puts
    // group 301 index 0 — *"1268 AD"* — on the first line.
    let mut cued = movie::Subtitles::new(true);
    cued.cue(0x1D);
    assert_eq!(cued.lines, [Some(0), None], "a translated L2.eng gets the first line");
}

