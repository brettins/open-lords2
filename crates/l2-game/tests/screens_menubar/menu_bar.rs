#![allow(unused_imports)]
use super::*;
use super::turn_timer::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screens::map::MapScreen;
use l2_game::screens::menubar;
use l2_game::screens::options::Page as OptionsPage;
use l2_game::screens::saveload::Mode as SaveLoadMode;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::chrome;
use l2_view::Canvas;

/// Install-gated by `world!`, so the widths are the shipped `Fntl2_14.pl8`'s
/// Through the shipped `L2.eng`'s own captions.
#[test]
fn the_menu_bar_titles_are_their_own_words_wide_with_a_dead_gap_between_them() {
    let (mut game, assets) = world!();
    let ctx = Ctx { game: &mut game, assets: &assets };
    let t = menubar::titles(&ctx);

    assert_eq!(t[0].x, 10, "g_menuBarItems[0].x");
    for r in &t {
        assert_eq!(r.y, 6, "every record's y");
        assert_eq!(r.h, 12, "Menu_HitTitle's fixed 12-pixel height");
        assert!(r.w > 0 && r.w < 120, "a caption, not a rectangle: {r:?}");
    }
    assert_eq!(t[1].x - (t[0].x + t[0].w), 32, "g_penAdvance += 0x20");
    assert_eq!(t[2].x - (t[1].x + t[1].w), 32);

    let gap = t[0].x + t[0].w + 8;
    assert!(menubar::title_at(&ctx, gap, 10).is_none(), "the gap hits nothing");
    assert_eq!(menubar::title_at(&ctx, t[1].x + 1, 10), Some(1));
    assert!(menubar::title_at(&ctx, t[1].x + 1, 18).is_none(), "y 18 is past 6 + 12");
}

#[test]
fn the_menu_bar_opens_a_dropdown_and_its_items_reach_their_screens() {
    let (mut game, assets) = world!();
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };

    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[0].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)), "the File menu is open");

    // FUN_0040DD92's button-up half: the pointer on another title switches it.
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: titles[2].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(2)), "sliding onto Help switches the menu");
    send_stack(&mut m, &mut game, &assets, Event::Pointer { x: titles[0].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)));

    // FUN_0040E099 and the pick: File's second item is Load, screen 0x35.
    let row = menubar::item_rect(&titles, 0, 1);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: row.x + 4, y: row.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::SaveLoad(SaveLoadMode::Load)), "File / Load");
    assert_eq!(m.depth(), 2, "and it landed where the drop-down was, over the map");

    // FUN_0040DF62: a press that is on no row closes and does nothing else, and
    // the five pixels between two rows belong to nothing at all.
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[1].x + 2, y: 10 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(1)));
    let dead = menubar::item_rect(&titles, 1, 0);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: dead.x + 4, y: dead.y + dead.h + 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "a press between two rows closes the menu");

    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[1].x + 2, y: 10 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 300, y: 300 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

#[test]
fn the_options_menu_reaches_the_four_option_screens() {
    let (mut game, assets) = world!();
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };
    for (menu, item, page) in [
        (1usize, 0usize, OptionsPage::Advanced),
        (1, 1, OptionsPage::Sound),
        (1, 2, OptionsPage::Display),
        (2, 0, OptionsPage::Help),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[menu].x + 2, y: 10 });
        let row = menubar::item_rect(&titles, menu, item);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: row.x + 4, y: row.y + 4 });
        assert_eq!(m.top_id(), Some(ScreenId::Options(page)), "menu {menu} item {item}");
    }
}

/// **Help > *"How do I…"* and its four siblings put a help message on the
/// ring**, which is what `Menu_HelpHowDoI` (`0x0043480C`) does:
///
/// **`l2help.hlp` is a different mechanism and is out of scope.** The only
/// `WinHelpA` in the interface is `Opt_GameHelpContents` (`0x00434942`) on the
/// help-options page, and a `winit` window cannot open a Windows 3.1 help
/// file. These five are `L2.eng`, not the help file.
#[test]
fn the_five_help_topics_put_their_message_on_the_ring() {
    let (mut game, assets) = world!();
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };
    for (row, id) in (1..=5usize).zip(0x123u16..=0x127) {
        let mut m = Machine::new(ScreenId::Campaign);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[2].x + 2, y: 10 });
        assert_eq!(m.top_id(), Some(ScreenId::MenuBar(2)), "the Help menu is open");
        let r = menubar::item_rect(&titles, 2, row);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: r.x + 4, y: r.y + 4 });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the menu closed behind it");
        let posted = *game.messages.waiting().last().expect("a help message");
        assert_eq!(posted.group, id);
        assert_eq!(posted.category, l2_game::message::category::HELP);
        assert_eq!(posted.to, game.player);
    }
}

/// A player: *"I think in the original game the shield icons at the top meant
/// that players hadn't ended their turn."* They did. `Screen_DrawMenuBar`
/// (`0x00419C78`):
#[test]
fn the_menu_bar_shields_are_the_realms_still_to_move() {
    let (mut game, assets) = world!();
    let ch = assets.chrome.as_ref().expect("the install has Panels.pl8 and Misc_cty.pl8");
    let mut screen = MapScreen::new();

    let human = game.player as usize;
    let live: Vec<usize> =
        (1..game.kingdom.realms.len()).filter(|&id| game.kingdom.realms[id].in_play).collect();
    let ais: Vec<usize> = live.iter().copied().filter(|&id| id != human).collect();
    assert!(ais.len() >= 2, "England seats more than one lord, or this test asserts nothing");
    let colours = game.realm_colour;

    let band = |c: &Canvas| -> Vec<u8> {
        let mut out = Vec::new();
        for y in 4..20usize {
            for x in 270..350usize {
                out.push(c.at(x, y));
            }
        }
        out
    };
    let expected = |realms: &[usize]| -> Vec<u8> {
        let mut c = Canvas::screen();
        ch.draw_menu_bar_background(&mut c);
        for (slot, &id) in realms.iter().enumerate() {
            let frame = 0x55 + chrome::realm_colour(colours[id]) as usize;
            assert!(ch.draw_misc(&mut c, frame, 0x10E + 0x10 * slot as i32, 4), "frame {frame}");
        }
        band(&c)
    };

    game.kingdom.realms[human].ai_step = l2_kingdom::AI_STEP_DONE;
    for &id in &ais {
        game.kingdom.realms[id].ai_step = l2_kingdom::AI_STEP_DONE + 1;
    }
    assert!(!l2_game::turn::turn_in_flight(&game));
    let idle = draw(&mut screen, &mut game, &assets);
    assert!(
        band(&idle) == expected(&live),
        "during the person's own turn every living realm's shield is up, in realm order \
         ({live:?}) - a literal `aiStep < 999` draws none here, because a finished turn \
         leaves every counter at 999 or 1000",
    );

    send(&mut screen, &mut game, &assets, Event::Click { x: 500, y: 470 });
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert!(
        l2_game::turn::turn_in_flight(&game),
        "the click should have started a turn, or this test asserts nothing",
    );
    let still = *ais.last().expect("checked");
    for &id in &ais {
        game.kingdom.realms[id].ai_step =
            if id == still { 3 } else { l2_kingdom::AI_STEP_DONE + 1 };
    }
    game.kingdom.realms[human].ai_step = 0;
    let running = draw(&mut screen, &mut game, &assets);
    assert!(
        band(&running) == expected(&[still]),
        "with End Turn pressed only realm {still}'s shield is up, and it is in slot 0 - \
         the person's has gone although his record's counter reads 0, and the row closes \
         up over every realm that has finished",
    );

    game.kingdom.realms[still].ai_step = l2_kingdom::AI_STEP_DONE;
    let finished = draw(&mut screen, &mut game, &assets);
    assert!(band(&finished) == expected(&[]), "every realm has finished, so no shield is up");
}

