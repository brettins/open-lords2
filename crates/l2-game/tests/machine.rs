
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::confirm::Ask;
use l2_game::screens::county::Panel;
use l2_game::screens::menu::MenuScreen;
use l2_game::Game;
use l2_view::Canvas;

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(3);
    l2_testkit::chain_neighbours!(g.kingdom);
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].strength = 3;
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[2].in_play = true;
    g.kingdom.realms[2].strength = 3;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[1].population = 400;
    g.kingdom.counties[2].owner = 0;
    g.kingdom.counties[2].population = 400;
    g.kingdom.counties[3].owner = 2;
    g.kingdom.counties[3].population = 400;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

fn tick(machine: &mut Machine, game: &mut Game, assets: &Assets) {
    let mut ctx = Ctx { game, assets };
    machine.update(&mut ctx);
}

fn run_turn(machine: &mut Machine, game: &mut Game, assets: &Assets) -> u32 {
    let before = game.kingdom.turn_count;
    let mut phase_ticks = None;
    for n in 1..2_000u32 {
        tick(machine, game, assets);
        if phase_ticks.is_none() && game.kingdom.turn_count > before {
            phase_ticks = Some(n);
        }
        if let Some(t) = phase_ticks {
            if n >= t + l2_view::fade::PHASES as u32 {
                return t;
            }
        }
    }
    panic!("the turn never came round");
}

pub(crate) fn draw(machine: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    machine.draw(&ctx, &mut canvas);
    canvas
}

#[test]
fn the_menu_starts_a_campaign_and_the_map_sits_on_top_of_it() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Menu);
    assert_eq!(m.ids(), vec![ScreenId::Menu]);

    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(m.ids(), vec![ScreenId::Menu, ScreenId::Campaign]);
    assert!(!m.should_quit());
}

/// **Escape on the map asks first.** `App_WndProc` (`0x004B29BE`) `VK_ESCAPE`
/// is `Menu_Quit()` inside a game, which is `Ui_OpenConfirm(0, …)` — the box,
/// not the exit. `tests/confirm_box.rs` drives its two answers; here it is the
/// stack that matters. Escape on the *menu* is ours and quits.
#[test]
fn escape_from_the_map_asks_the_exit_box_and_escape_on_the_menu_quits() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Menu);
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Enter));
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    assert_eq!(
        m.ids(),
        vec![ScreenId::Menu, ScreenId::Campaign, ScreenId::Confirm(Ask::Quit)],
        "the map is still under the question"
    );
    assert!(!m.should_quit());

    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    assert!(!m.should_quit(), "the box has no keyboard arm — the original's has none either");
}

#[test]
fn the_menus_second_item_quits_by_keyboard_and_by_click() {
    for by_mouse in [false, true] {
        let (mut game, assets) = world();
        let mut m = Machine::new(ScreenId::Menu);
        if by_mouse {
            let r = MenuScreen::item_rect(1);
            send(&mut m, &mut game, &assets, Event::Click { x: r.centre_x(), y: r.y + 4 });
        } else {
            send(&mut m, &mut game, &assets, Event::KeyDown(Key::Down));
            send(&mut m, &mut game, &assets, Event::KeyDown(Key::Enter));
        }
        assert!(m.should_quit(), "by_mouse = {by_mouse}");
    }
}

#[test]
fn a_click_that_lands_on_no_menu_item_does_nothing_at_all() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Menu);
    send(&mut m, &mut game, &assets, Event::Click { x: 5, y: 5 });
    assert_eq!(m.ids(), vec![ScreenId::Menu]);
    assert!(!m.should_quit());
}

#[test]
fn a_screen_returns_its_transition_and_has_no_way_to_perform_one() {
    let (mut game, assets) = world();
    let mut screen = MenuScreen::new();
    let mut ctx = Ctx { game: &mut game, assets: &assets };

    let t = screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
    assert_eq!(t, Transition::Push(ScreenId::Campaign));

    let t = screen.handle(Event::KeyDown(Key::Down), &mut ctx);
    assert_eq!(t, Transition::Stay, "moving the selection is not a transition");

    let t = screen.handle(Event::KeyDown(Key::Enter), &mut ctx);
    assert_eq!(t, Transition::Quit);
}

#[test]
fn only_the_top_screen_is_offered_input() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    let before = game.kingdom.turn_count;

    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Char('E')));
    assert_eq!(
        game.kingdom.turn_count, before,
        "E starts the turn; it does not finish it inside the keystroke",
    );
    run_turn(&mut m, &mut game, &assets);
    let after_map = game.kingdom.turn_count;
    assert!(after_map > before, "the map screen ends the turn on E");

    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(m.top_id(), Some(ScreenId::County(1, Panel::Tax)));
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Char('E')));
    for _ in 0..64 {
        tick(&mut m, &mut game, &assets);
    }
    assert_eq!(
        game.kingdom.turn_count, after_map,
        "the map is underneath and must not see the key"
    );
}

#[test]
fn the_county_panel_sets_the_tax_rate_of_the_players_county_and_not_of_another() {
    let (mut game, assets) = world();
    game.selected = 1;
    let mut m = Machine::new(ScreenId::County(1, Panel::Tax));
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Right));
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[1].tax_rate, 2);
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Left));
    assert_eq!(game.kingdom.counties[1].tax_rate, 1);

    let mut m = Machine::new(ScreenId::County(2, Panel::Tax));
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[2].tax_rate, 0);
}

#[test]
fn the_county_panels_second_row_sets_the_ration_level() {
    let (mut game, assets) = world();
    game.kingdom.counties[1].ration_wanted = 3;
    let mut m = Machine::new(ScreenId::County(1, Panel::Tax));
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Down)); // to the ration row
    send(&mut m, &mut game, &assets, Event::KeyDown(Key::Right));
    assert_eq!(game.kingdom.counties[1].ration_wanted, 4);
    assert_eq!(game.kingdom.counties[1].tax_rate, 0, "the tax row must not have moved");

    for _ in 0..5 {
        send(&mut m, &mut game, &assets, Event::KeyDown(Key::Right));
    }
    assert_eq!(game.kingdom.counties[1].ration_wanted, 5);
}

#[test]
fn every_screen_paints_the_whole_canvas_rather_than_leaving_it_blank() {
    let (mut game, assets) = world();
    for id in [ScreenId::Menu, ScreenId::Campaign, ScreenId::County(1, Panel::Tax)] {
        let mut m = Machine::new(id);
        let canvas = draw(&mut m, &mut game, &assets);
        let ink = &assets.ink;
        assert!(
            canvas.count(ink.text) > 0,
            "{id:?} drew no text at all"
        );
        assert!(
            canvas.pixels.iter().any(|&p| p != ink.background),
            "{id:?} drew nothing but background"
        );
    }
}

#[test]
fn the_window_title_is_the_top_screens_and_follows_the_clock() {
    let (mut game, assets) = world();
    game.kingdom.season = 4;
    game.kingdom.year = 1268;
    let mut m = Machine::new(ScreenId::Menu);

    let title = {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let t = m.title(&ctx);
        m.handle(Event::KeyDown(Key::Enter), &mut ctx);
        t
    };
    assert_eq!(title, "Lords of the Realm II", "the menu names the game and nothing else");

    let ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(m.title(&ctx), "Lords of the Realm II - Winter 1268");
}

#[test]
fn the_machine_reports_dirty_after_input_and_clean_after_a_quiet_tick() {
    let (mut game, assets) = world();
    let mut m = Machine::new(ScreenId::Menu);
    assert!(m.take_dirty(), "the first frame always draws");
    assert!(!m.take_dirty(), "and nothing has happened since");

    send(&mut m, &mut game, &assets, Event::Pointer { x: 1, y: 1 });
    assert!(m.take_dirty());

    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert!(!m.take_dirty(), "a tick in which no screen asked for anything is not a repaint");
}
