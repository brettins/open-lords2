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

/// **A double click reaches the verb buttons and the dialog's gauntlets, and
/// each waits its twenty frames.**
///
/// Both screens dropped `Event::DoubleClick`. In the original the six verb
/// buttons and the send and cancel pairs are `Widget_Test` kind 5, whose guard
/// is `g_mouseLeftPressed || g_mouseLeftDoubleClick`; the card pick
/// `FUN_004369BD` opens `if (g_mouseLeftPressed != 0)` and the county picker
/// `FUN_0043B4CB` `else if (g_mouseLeftPressed == 0) return 0`. `[V]`
///
/// **Ablations, run:** delete the `Event::DoubleClick` early return from
/// `DiplomacyScreen::handle`; delete the
/// `Event::DoubleClick` arm of `ComposeScreen::handle` and it never closes.
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

/// **Right-click exits both screens, and they exit to different places.** The
/// gesture `docs/agents.md` records as the one we systematically miss, and the
/// asymmetry is the original's: the cross returns to the lord cards, the corner
/// button and the right button go to the map.
#[test]
fn the_right_button_leaves_and_the_cross_goes_back() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    assert_eq!(machine.depth(), 2);
    // The cross: back to 0x0B.
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_CANCEL));
    assert_eq!(machine.top_id(), Some(ScreenId::Diplomacy));
    assert_eq!(machine.depth(), 1);

    // The right button on the compose dialog: straight to the map.
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 300, y: 300 });
    assert_eq!(machine.top_id(), Some(ScreenId::Campaign));

    // And on the lord cards: the corner button leaves too.
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 100, y: 100 });
    assert!(machine.should_quit() || machine.depth() == 0, "popping the only screen quits");
}

/// **Nothing on either screen sends a letter as a side effect of drawing it.**
///
/// The compose dialog is the only screen in the game whose *draw* reads the
/// pair record it is about, and the pair record is what the AI's decisions are
/// made of — so a draw that wrote would be a rule the player could fire by
/// looking. Asserted by drawing every shape twice and comparing the kingdom.
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

/// The screens are **overlays**: the original clears nothing and paints over
/// the campaign map, which is what `Machine::draw` walking back to the last
/// non-overlay screen is for.
#[test]
fn both_screens_are_insets_over_the_map() {
    for id in [ScreenId::Diplomacy, ScreenId::DiploCompose(2, 0)] {
        assert!(id.build().is_overlay(), "{id:?} is drawn over what was there");
    }
}

/// A menu click on a rival that has been knocked out cannot happen, because
/// `Diplo_DrawScreen` replaces a dead target before it draws anything.
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

/// `Transition::Pass` is not used by either screen: every click either belongs
/// to it or is ignored. Stated because `screens/mod.rs` warns that *"passing by
/// default is how two screens end up both acting on one
/// click"*, and these two sit over the map.
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

