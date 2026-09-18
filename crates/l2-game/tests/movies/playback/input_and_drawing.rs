#![allow(unused_imports)]
use super::*;
use super::timing_and_sound::*;
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
fn a_film_ends_on_a_release_of_either_button_or_any_key_and_not_on_a_press() {
    let (_p, a) = install!();
    let mut g = Game::new(1);

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

/// `CastleBuild_Confirm` (`0x00436B59`) is record 0 of `g_castleBuildWidgets`
/// and the record is `Widget_Test` kind 5, so `Smk_Play` runs **twenty frames
/// after the press** — by which time the button has been let go and its release
/// was answered by the chooser. `CastleBuild_Confirm`'s own first call
/// (`FUN_004B18E3`) throws that click away too.
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

    send(&mut m, &mut g, &a, Event::Release { x: ok.0, y: ok.1 });
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

