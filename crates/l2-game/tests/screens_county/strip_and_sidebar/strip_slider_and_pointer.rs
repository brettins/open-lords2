#![allow(unused_imports)]
use super::*;
use super::strip_quadrants_and_sidebar_buttons::*;
use super::strip_closing_and_numbers::*;
use super::*;
use super::panels::*;
use super::drawing_and_emboss::*;
use super::produce_and_pastures::*;
use super::layout_and_labels::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// **The farm/industry split slider moves peasants.** `FUN_00439122` is the one
/// control on the campaign screen that reallocates labour in bulk, and it was
/// not wired at all — which is a fair part of *"I can't assign peasants"*.
#[test]
fn the_sidebar_split_slider_moves_the_countys_labour_between_farm_and_industry() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let before = game.kingdom.counties[8].industry_share;

    // x 533 on the track is ((533 - 531) * 2) & 0xFC = 4.
    send(&mut screen, &mut game, &assets, Event::Click { x: 533, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 4);
    assert_ne!(4, before, "and that is not where it started");

    // The far end of the track is 100 per cent industry.
    send(&mut screen, &mut game, &assets, Event::Click { x: 581, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 100);

    // The allocation follows it.
    // split: with everybody in the mines, the farm jobs empty.
    let farm: i32 = (0..3).map(|j| game.kingdom.counties[8].labour[j]).sum();
    send(&mut screen, &mut game, &assets, Event::Click { x: 531, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 0);
    let farm_after: i32 = (0..3).map(|j| game.kingdom.counties[8].labour[j]).sum();
    assert!(farm_after > farm, "0% industry puts more people on the land than 100% did");
}

/// **The blue outline appears, and only when it should.**
///
/// Two separate signals with two separate tests, which is the thing worth
/// pinning down: the **slider's** thumb gains its ring on
/// `county.labour[8].workers != 0` — anybody idle at all — and each **produce
/// icon** gains one on `labour[slot].useful < labour[slot].workers` — too many
/// people on *that* job. A player described both and thought they were the same
/// signal; they are the same picture and different tests.
///
/// Measured by counting pixels of the ring's own three palette entries inside
/// the sidebar, so it is the artwork being asserted and not a description of
/// it.
#[test]
fn the_strip_draws_the_blue_ring_on_the_slider_and_on_the_overstaffed_job() {
    use l2_view::chrome::misc_cty::RING_COLOURS;
    let (mut game, assets) = world!();
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
    let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;

    // Count the ring's colours in the sidebar column only.
    let ring_pixels = |canvas: &Canvas| -> usize {
        let mut n = 0;
        for y in 156..430usize {
            for x in 478..640usize {
                if RING_COLOURS.contains(&canvas.at(x, y)) {
                    n += 1;
                }
            }
        }
        n
    };

    // Nobody idle, and the dairy inside its ceiling.
    {
        let c = &mut game.kingdom.counties[8];
        c.labour[idle] = 0;
        c.labour[cattle] = 100;
        c.labour_wanted[cattle] = -1;
        c.labour_useful[cattle] = 200;
        c.herd = 400;
        c.fields_cattle = 4;
    }
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let quiet = ring_pixels(&draw(&mut screen, &mut game, &assets));

    // One idle townsman: the slider's thumb becomes frame 0x55.
    game.kingdom.counties[8].labour[idle] = 1;
    let with_slider = ring_pixels(&draw(&mut screen, &mut game, &assets));
    assert!(
        with_slider > quiet,
        "the slider's thumb gains its ring: {quiet} -> {with_slider} ring pixels"
    );

    // And more people milking than the herd can use: the cow gains one too.
    game.kingdom.counties[8].labour_useful[cattle] = 50;
    let with_both = ring_pixels(&draw(&mut screen, &mut game, &assets));
    assert!(
        with_both > with_slider,
        "the dairy icon gains its own ring: {with_slider} -> {with_both} ring pixels"
    );

    // Putting the ceiling back takes the cow's ring away again and leaves the
    // Slider's: two tests.
    game.kingdom.counties[8].labour_useful[cattle] = 200;
    assert_eq!(ring_pixels(&draw(&mut screen, &mut game, &assets)), with_slider);
}

/// **And it is a drag, not a click.** A player reported *"the peasant slider of
/// industry isn't draggable, should be"*, and `FUN_00439122` agrees: it acts
/// while `DAT_004E65CC` — the button's *level* — is set and `DAT_004EA4B0` says
/// the pointer moved, and does nothing at all on the release. So the value
/// follows the pointer for as long as the button is held, and stops the moment
/// it is let go.
///
/// The whole gesture as a sequence of values, which is the only way to test a
/// drag without an input queue.
#[test]
fn the_split_slider_tracks_the_pointer_while_the_button_is_held() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let share = |g: &Game| g.kingdom.counties[8].industry_share;

    // Press on the track at x = 533, then travel along it without letting go.
    send(&mut screen, &mut game, &assets, Event::Click { x: 533, y: 270 });
    assert_eq!(share(&game), 4, "the press itself sets the value");
    for (x, want) in [(541, 20), (561, 60), (581, 100), (551, 40)] {
        send(&mut screen, &mut game, &assets, Event::Pointer { x, y: 270 });
        assert_eq!(share(&game), want, "held and moved to x = {x}");
    }

    // Off the sidebar entirely and the slider stops, without the drag ending —
// the original re-tests the rectangle every frame and skips.
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 200, y: 270 });
    assert_eq!(share(&game), 40, "outside the rectangle nothing moves");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 561, y: 270 });
    assert_eq!(share(&game), 60, "and coming back resumes the same drag");

    // Let go. Now the same movement does nothing.
    send(&mut screen, &mut game, &assets, Event::Release { x: 561, y: 270 });
    assert_eq!(share(&game), 60, "the release itself changes nothing");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 533, y: 270 });
    assert_eq!(share(&game), 60, "and a bare pointer move is not a drag");

    // Off the track, each move steps by four.
    send(&mut screen, &mut game, &assets, Event::Click { x: 600, y: 270 });
    assert_eq!(share(&game), 64, "right of the track: +4");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 601, y: 270 });
    assert_eq!(share(&game), 68, "and again on the next move");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 500, y: 270 });
    assert_eq!(share(&game), 64, "left of the track: -4");
}

