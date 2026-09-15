#![allow(unused_imports)]
use super::*;
use super::input_and_drawing::*;
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

#[test]
fn a_real_castle_film_ends_on_the_map_however_long_it_runs() {
    let (_p, a) = install!();
    let (mut g, mut m) = castle_world();
    assert!(g.prefs.tip_screens, "the defect needs the advisor tips a new game has");
    m.push(ScreenId::Movie(Film::Castle(0)));
    let smk = a.films.open("castle1.smk").expect("the install's castle film");
    let frames = smk.frames();
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

/// `Smk_PlayLoop` (`0x0042DBC7`) steps a film only when `SmackWait` answers 0,
/// and `_SmackWait@4` is a `timeGetTime` comparison — the original's film runs
/// on real milliseconds, and so does the sound track a device plays out.
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

        let mut ticker = l2_game::clock::Ticker::new();
        let mut player = movie::Player::open(smk, false).expect("frame 0");
        let (mut now, mut wall_at_end) = (0u64, None);
        while now <= sound_ns {
            for _ in 0..ticker.due(now) {
                if wall_at_end.is_none() && player.tick() == Ok(movie::Step::Finished) {
                    wall_at_end = Some(now);
                }
            }
            let shown = player.frame() as u64 * period_ns;
            assert!(
                shown.abs_diff(now.min((frames - 1) * period_ns)) <= period_ns,
                "{name}: at {} ms of sound the picture was at {} ms",
                now / 1_000_000,
                shown / 1_000_000
            );
            now = ticker.next_ns().unwrap_or(now + tick_ns) + OVER_NS;
        }
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


