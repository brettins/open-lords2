#![allow(unused_imports)]
use super::*;
use super::population::*;
use super::tax::*;
use super::turns_and_sidebar::*;
use super::*;
use super::strip_and_sidebar::*;
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

/// The ration panel's slider is the third order the original's county panels
/// give, and the one we never had: `Ration_SliderClick` jumps the split to
/// `mouseX - 224`, and the two caps step it by one.
#[test]
fn the_ration_split_slider_sets_the_field_the_original_sets() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    screen.open(Panel::Ration);

    let track = county::split_track();
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 37, y: track.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 37, "the track jumps to mouseX - 224");

    let down = county::split_down_button();
    send(&mut screen, &mut game, &assets, Event::Click { x: down.centre_x(), y: down.y + 8 });
    assert_eq!(game.kingdom.counties[8].ration_split, 36, "the left cap steps down one");

    let up = county::split_up_button();
    for _ in 0..3 {
        send(&mut screen, &mut game, &assets, Event::Click { x: up.centre_x(), y: up.y + 8 });
    }
    assert_eq!(game.kingdom.counties[8].ration_split, 39);

    // The knob follows: two different splits must not draw the same picture.
    let a = draw(&mut screen, &mut game, &assets);
    game.kingdom.counties[8].ration_split = 90;
    let b = draw(&mut screen, &mut game, &assets);
    assert!(a.diff_count(&b) > 0, "the knob moves with the value");
}

/// **The slider's effect has to be visible in the same frame, and it was not.**
///
/// A player reported *"rations slider moves but is inoperable"*. It was writing
/// `ration_split` correctly the whole time — a test three functions up asserted
/// exactly that and passed — and every number on the panel stayed where it was
/// until the turn ended, because `Game::set_ration_split` wrote the field and
/// stopped. `Ration_SetSplit` re-runs the food pass, reallocates the county
/// twice and repaints.
///
/// So this asserts the *effect* and not the fixture: draw the panel, move the
/// slider, draw it again, and require pixels to differ **outside the slider's
/// own rectangle**. Masking the slider out is the whole point — a thumb that
/// moves is what the player could already see, and it is not evidence of
/// anything.
///
/// Ablation, which was run: replace `Kingdom::set_ration_split`'s body with the
/// old one-line write and this fails with zero pixels changed.
#[test]
fn moving_the_ration_slider_changes_a_number_on_the_panel_in_the_same_frame() {
    let (mut game, assets) = world!();
    let county = 8; // the player's, in the England turn-one fixture
    assert_eq!(game.kingdom.counties[county].owner, game.player);

    // **The fixture's own county cannot demonstrate this.
    // rule worth knowing.** County 8 at England turn one has 435 people and
    // 101 head; the standing herd feeds five people a head *without being
    // slaughtered*, so 505 mouths' worth of dairy covers 435 and the county eats
    // nothing at all. `herd_eaten` and `grain_eaten` are 0 at **every** split,
    // so the slider has nothing to divide and the panel is inert — in the
    // original as much as here. That is very likely what the player was looking
    // At. `docs/rules.md` explains this.
    //
    // So the herd is cut to something the county has to eat *around*. The state
    // is the input and the screen is the subject; asserting on a county whose
    // numbers cannot move would be a test that passes for the wrong reason,
    // which is the whole family `docs/agents.md` catalogues.
    game.kingdom.counties[county].herd = 20;
    game.kingdom.counties[county].grain = 400;

    let mut screen = CountyScreen::new(county as u8, Panel::Ration);
    let track = county::split_track();

    // Start at one end so the move is as large as the control allows.
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x, y: track.y + 8 });
    let before = draw(&mut screen, &mut game, &assets);
    let split_before = game.kingdom.counties[county].ration_split;

    // A drag: the button goes down on the track and the pointer walks to the
    // far end. `Ration_SliderClick` fires on held-and-moved, not on the click.
    send(&mut screen, &mut game, &assets, Event::Click { x: track.x + 4, y: track.y + 8 });
    for step in (4..=track.w).step_by(8) {
        send(
            &mut screen,
            &mut game,
            &assets,
            Event::Pointer { x: track.x + step, y: track.y + 8 },
        );
    }
    send(&mut screen, &mut game, &assets, Event::Release { x: track.x + track.w, y: track.y + 8 });
    let after = draw(&mut screen, &mut game, &assets);

    assert_ne!(
        game.kingdom.counties[county].ration_split, split_before,
        "the drag did not reach the field at all",
    );

    // Everything except the slider's own row. `split_track` is the track; the
    // two caps sit either side of it on the same row, so the mask is the whole
    // band.
    let masked = |c: &l2_view::Canvas, other: &l2_view::Canvas| {
        let mut n = 0usize;
        for y in 0..l2_view::canvas::HEIGHT {
            for x in 0..l2_view::canvas::WIDTH {
                let (xi, yi) = (x as i32, y as i32);
                let in_slider = yi >= track.y - 8
                    && yi < track.y + track.h + 8
                    && xi >= track.x - 64
                    && xi < track.x + track.w + 64;
                if !in_slider && c.at(x, y) != other.at(x, y) {
                    n += 1;
                }
            }
        }
        n
    };
    let changed = masked(&before, &after);
    assert!(
        changed > 0,
        "the slider moved and nothing else on the panel did. The thumb is not the \
         effect: Ration_SetSplit runs the food pass on the spot, so `Eaten`, `Achieved` \
         and the happiness deltas move with it. docs/decisions.md C118.",
    );
}

