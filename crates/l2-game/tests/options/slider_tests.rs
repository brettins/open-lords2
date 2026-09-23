use super::*;
use l2_game::screens::slider::{SliderScreen, Spin, DOWN, OK, UP};

fn send(s: &mut SliderScreen, game: &mut Game, assets: &Assets, e: Event) -> Transition {
    s.handle(e, &mut Ctx { game, assets })
}

/// `Ui_SliderUp` 0x00434E9E is `if (*v < max) *v += step` with `Menu_ScrollSpeed`'s
/// `(step 10, max 100)`; `Ui_SliderDown` 0x00434EC9 `if (min < *v) *v -= step`. Ablated by
/// swapping `widget == 0` for `widget == 1` in `SliderScreen::fire`: 60 goes to 50.
#[test]
fn the_spinner_steps_g_opt_scroll_speed_by_ten_and_stops_at_its_ends() {
    let (mut game, assets) = world();
    let mut s = SliderScreen::new(Spin::ScrollSpeed);
    assert_eq!(game.prefs.scroll_speed, 60, "Options_SetDefaults");
    let (x, y) = mid(UP);
    send(&mut s, &mut game, &assets, Event::Click { x, y });
    send(&mut s, &mut game, &assets, Event::Release { x, y });
    assert_eq!(game.prefs.scroll_speed, 70);
    game.prefs.scroll_speed = 100;
    send(&mut s, &mut game, &assets, Event::Click { x, y });
    send(&mut s, &mut game, &assets, Event::Release { x, y });
    assert_eq!(game.prefs.scroll_speed, 100, "a gate, not a clamp");
    let (x, y) = mid(DOWN);
    send(&mut s, &mut game, &assets, Event::Click { x, y });
    send(&mut s, &mut game, &assets, Event::Release { x, y });
    assert_eq!(game.prefs.scroll_speed, 90);
    assert_eq!(game.prefs.game_speed, 90, "the other global is untouched");
}

/// Screen_FrameInput's `0x21` arm: the right release, or `Ui_OkButtonClicked` on the left
/// release, both `g_screenId = g_sliderPrevScreen`. A left press on the corner does not.
#[test]
fn the_spinner_leaves_on_the_right_release_or_the_corner_release() {
    let (mut game, assets) = world();
    let mut s = SliderScreen::new(Spin::GameSpeed);
    let (x, y) = mid(OK);
    assert_eq!(send(&mut s, &mut game, &assets, Event::Click { x, y }), Transition::Stay);
    assert_eq!(send(&mut s, &mut game, &assets, Event::Release { x, y }), Transition::Pop);
    let mut s = SliderScreen::new(Spin::GameSpeed);
    assert_eq!(send(&mut s, &mut game, &assets, Event::RightClick { x: 5, y: 5 }), Transition::Pop);
}

/// `Menu_GameSpeed` 0x00434CBE / `Menu_ScrollSpeed` 0x00434CEE open screen 0x21 with prompts 12/1 and 12/2.
#[test]
fn the_two_speed_items_open_screen_0x21() {
    assert_eq!(Spin::GameSpeed.prompt(), 1);
    assert_eq!(Spin::ScrollSpeed.prompt(), 2);
    assert!(ScreenId::Slider(Spin::GameSpeed).build().is_overlay());
}
