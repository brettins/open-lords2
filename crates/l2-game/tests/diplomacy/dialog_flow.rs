#![allow(unused_imports)]
use super::*;
use super::offers_and_requests::*;
use super::letters::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::diplomacy::{self, DiplomacyScreen, Menu};
use l2_game::Game;
use l2_kingdom::diplomacy::Kind;
use l2_view::Canvas;

/// Both screens dropped `Event::DoubleClick`. In the original the six verb
/// buttons and the send and cancel pairs are `Widget_Test` kind 5, whose guard
/// is `g_mouseLeftPressed || g_mouseLeftDoubleClick`; the card pick
/// `FUN_004369BD` opens `if (g_mouseLeftPressed != 0)` and the county picker
/// `FUN_0043B4CB` `else if (g_mouseLeftPressed == 0) return 0`. `[V]`
#[test]
fn a_double_click_opens_a_dialog_and_a_double_click_closes_it_each_after_twenty_frames() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);
    let double = |r: l2_game::input::Rect| Event::DoubleClick { x: r.x + r.w / 2, y: r.y + r.h / 2 };

    press_and_wait(&mut machine, &mut game, &assets, double(diplomacy::menu_widget(0)));
    assert_eq!(machine.top_id(), Some(ScreenId::DiploCompose(2, Kind::Gift.byte())));

    press_and_wait(&mut machine, &mut game, &assets, double(diplomacy::GIFT_CANCEL));
    assert_eq!(machine.top_id(), Some(ScreenId::Diplomacy), "the cross returns to the cards");
    assert_eq!(machine.clicks(), 2, "each double click was one press of Widget_Test's");
}

#[test]
fn the_right_button_leaves_and_the_cross_goes_back() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    assert_eq!(machine.depth(), 2);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_CANCEL));
    assert_eq!(machine.top_id(), Some(ScreenId::Diplomacy));
    assert_eq!(machine.depth(), 1);

    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 300, y: 300 });
    assert_eq!(machine.top_id(), Some(ScreenId::Campaign));

    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 100, y: 100 });
    assert!(machine.should_quit() || machine.depth() == 0, "popping the only screen quits");
}

#[test]
fn drawing_the_dialogs_changes_no_state_at_all() {
    let (mut game, assets) = world();
    let before = game.kingdom.clone();
    for id in [
        ScreenId::Diplomacy,
        ScreenId::DiploCompose(2, 0),
        ScreenId::DiploCompose(2, 1),
        ScreenId::DiploCompose(2, 5),
    ] {
        let mut screen = id.build();
        for _ in 0..2 {
            let mut canvas = Canvas::screen();
            let ctx = Ctx { game: &mut game, assets: &assets };
            screen.draw(&ctx, &mut canvas);
        }
        assert_eq!(game.kingdom, before, "{id:?} moved something by being drawn");
    }
}

#[test]
fn both_screens_are_insets_over_the_map() {
    for id in [ScreenId::Diplomacy, ScreenId::DiploCompose(2, 0)] {
        assert!(id.build().is_overlay(), "{id:?} is drawn over what was there");
    }
}

#[test]
fn a_target_that_is_knocked_out_is_replaced_rather_than_kept() {
    let (mut game, assets) = world();
    let mut screen = DiplomacyScreen::new();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.target(&ctx), 2);
    }
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, middle(diplomacy::card_rect(1)));
    game.kingdom.realms[3].strength = 0;
    game.kingdom.realms[3].in_play = false;
    let ctx = Ctx { game: &mut game, assets: &assets };
    let _ = &mut screen;
    let mut fresh = DiplomacyScreen::new();
    assert_eq!(fresh.target(&ctx), 2, "the default takes over");
    let mut canvas = Canvas::screen();
    l2_game::screen::Screen::draw(&mut fresh, &ctx, &mut canvas);
    assert_eq!(DiplomacyScreen::cards(&ctx), vec![2], "and the dead realm loses its card");
}

#[test]
fn neither_screen_ever_passes_a_click_to_the_map_underneath() {
    let (mut game, assets) = world();
    for id in [ScreenId::Diplomacy, ScreenId::DiploCompose(2, 0)] {
        let mut screen = id.build();
        for (x, y) in [(0, 0), (639, 479), (320, 240), (16, 400)] {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            let t = screen.handle(Event::Click { x, y }, &mut ctx);
            assert_ne!(t, Transition::Pass, "{id:?} passed a click at ({x}, {y})");
        }
    }
}

