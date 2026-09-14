//! The menu bar, the options menu and the turn timer.
//!
//! Split out of `tests/screens.rs`; the shared helpers are in `tests/common/`.

#[macro_use]
mod common;

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

/// **`Menu_HitTitle` measures the words**, so the 32-pixel gap between two
/// titles is dead bar.
///
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

/// **The menu bar opens, hovers, picks and closes** - the four arms of screen
/// `0x32`, driven as a player drives them.
#[test]
fn the_menu_bar_opens_a_dropdown_and_its_items_reach_their_screens() {
    let (mut game, assets) = world!();
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };

    // Menu_OpenDropdown: a press on a title.
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

    // And the right button closes it.
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: titles[1].x + 2, y: 10 });
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 300, y: 300 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

/// **The Options and Help menus reach the four option screens**, which had no
/// route into them but the index screen.
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
/// `Msg_Enqueue(0, g_localPlayer, 0x123, 0, 0x13, 0, 0, 0)` then
/// `g_screenId = g_menuPrevScreen`. Five consecutive ids, `0x123` … `0x127`,
/// all category `0x13`.
///
/// **`l2help.hlp` is a different mechanism and is out of scope.** The only
/// `WinHelpA` in the interface is `Opt_GameHelpContents` (`0x00434942`) on the
/// help-options page, and a `winit` window cannot open a Windows 3.1 help
/// file. These five are `L2.eng`, not the help file.
///
/// **Ablation, run:** return `Transition::Stay` without the enqueue and the
/// ring stays empty.
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

/// **The menu bar's shields are the realms still to move.**
///
/// A player: *"I think in the original game the shield icons at the top meant
/// that players hadn't ended their turn."* They did. `Screen_DrawMenuBar`
/// (`0x00419C78`):
///
/// ```c
/// for (i = 1; i < 6; i++)
///     if ((g_realms[i].strength != 0) && (g_realms[i].aiStep < 999)) {
///         Pl8_DrawFrame(g_miscCtySheet, g_realms[i].shieldIndex + 0x55, slot * 0x10 + 0x10e, 4);
///         slot++;
///     }
/// ```
///
/// The expected band is built **out of those literals** — frame `0x55 +
/// shield`, `x = 0x10E + 0x10 * slot`, `y = 4` — on a bare menu-bar
/// background, and compared with what the campaign map drew. Nothing in the
/// probe goes through `Chrome::draw_banner` or `turn::realm_turn_ended`.
///
/// Three states:
///
/// 1. **the person's own turn — every living realm's shield is up**, even with
///    every counter at or past 999, which is where a finished turn leaves them.
///    This is the inversion trap: a literal `ai_step < 999` draws none.
/// 2. **End Turn pressed** — the person's shield is gone even with his counter
///    at 0, and of the AI realms only the one still stepping remains, **closed
///    up into slot 0** although it is the highest-numbered realm;
/// 3. **everybody finished** — the bar is empty.
///
/// Ablations, each observed red: `ai_step < 999` literally in `draw_menu_bar`
/// (claim 1); the `realm_turn_ended` clause deleted (claim 2); `slot`
/// Advanced for every realm (claim 2).
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

    // The five slots: 13 x 16 frames from x 270 at 16-pixel steps, y 4.
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

    // 1 — the person's own turn, with every counter where the last turn left it.
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

    // 2 — End Turn, from the map, with the mouse.
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

    // 3 — and when the last realm finishes, the bar is empty.
    game.kingdom.realms[still].ai_step = l2_kingdom::AI_STEP_DONE;
    let finished = draw(&mut screen, &mut game, &assets);
    assert!(band(&finished) == expected(&[]), "every realm has finished, so no shield is up");
}

/// `n` fixed ticks of a whole [`Machine`] — which is where the turn timer
/// counts, because the original counts it in `Turn_Tick` and not in a screen.
fn tick_stack(m: &mut Machine, game: &mut Game, assets: &Assets, n: u32) {
    for _ in 0..n {
        let mut ctx = Ctx { game: &mut *game, assets };
        m.update(&mut ctx);
    }
}

/// The same stack drawn with the time limit taken away for the one frame, so
/// that `with == without` says *the timer drew nothing* as an equality, with no
/// threshold and no knowledge of what else is on the screen.
fn draw_stack_without_the_timer(m: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let limit = game.kingdom.options.time_limit;
    game.kingdom.options.time_limit = 0;
    let canvas = draw_stack(m, game, assets);
    game.kingdom.options.time_limit = limit;
    canvas
}

/// Where the digits of `Ui_DrawNumberRight(v, ' ', &DAT_004D41D0, 0x1A8, 0x1BA,
/// 0x32, &g_fontBody, 0x3F)` are, searched for **inside that box only**.
fn timer_digits(canvas: &Canvas, assets: &Assets, v: i32) -> Option<(i32, i32)> {
    let window = crop(canvas, 0x1A8, 0x1BA, 0x32 + 8, 20);
    find_body(&window, assets, &v.to_string(), font::TEXT)
}

/// Where they belong, in the box's own coordinates.
///
/// The buffer is lead, digits and suffix, centred whole. **The suffix is under
/// test and the expectation does not go through it**: `&DAT_004D41D0` is `20 00`
/// in the image, one space, and it is written here as one more space's advance
/// Not read from `turn_clock::SUFFIX`; emptying the constant moves
/// the picture two pixels and leaves this where it is.
fn timer_digits_expected(assets: &Assets, v: i32) -> (i32, i32) {
    let space = body_width(assets, " ");
    let buffer = body_width(assets, &format!(" {v}")) + space;
    (((0x32 - buffer) / 2).max(0) + space, 0)
}

/// **The turn timer is up for the person's own turn, counts it down, ends it,
/// and is back at the whole limit when his next turn begins.**
///
/// `FUN_0041A639` (`0x0041A639`) draws `Misc_cty` frame `0x60` at (404, 430)
/// and `DAT_005440C8` centred in fifty pixels at (424, 442) under
///
/// ```c
/// 0 < DAT_005440C8 && (DAT_0055403C < 1 || g_realms[g_localPlayer].aiStep == 999)
///                  && 0 < g_optTimeLimit && DAT_004D2E80[g_screenId] == 0
/// ```
///
/// and `Turn_Tick`'s phase-4 arm counts `g_optTimeLimit - elapsed / 1000` and
/// calls `Turn_End` below zero. Every literal below — the frame, the two
/// positions, the box, the tick numbers — is written from the decompilation and
/// none goes through `crate::turn_clock`.
///
/// **The trap is the first assertion.** Every realm's counter is where a
/// finished turn leaves it and the person's is 1, `AI_RunTurnStep`'s step 0, so
/// a port of the guard as `docs/draws-map.md` §5.11 used to write it —
/// `g_optTimeLimit > 0 && aiStep == 999` — draws nothing during the person's
/// own turn, which is the only time a countdown is for.
///
/// Ablations, each observed red: the guard as documented, the person's
/// `ai_step == 999` alone (claim 1); the suffix emptied (claim 1, two pixels);
/// the `Turn_End` request `Turn_Tick` makes deleted (claim 4); the person's turn
/// Read off his `ai_step`.
/// (claim 5, and nothing earlier).
#[test]
fn the_turn_timer_is_up_for_the_players_own_turn_and_ends_it_when_it_runs_out() {
    let (mut game, assets) = world!();
    let ch = assets.chrome.as_ref().expect("the install has Misc_cty.pl8");
    // "30 secs" — `g_timeLimitSeconds[0]`.
    game.kingdom.options.time_limit = 30;
    let human = game.player as usize;
    for id in 1..game.kingdom.realms.len() {
        game.kingdom.realms[id].ai_step = l2_kingdom::AI_STEP_DONE + 1;
    }
    game.kingdom.realms[human].ai_step = 1;
    let mut m = Machine::new(ScreenId::Campaign);

    // 1 — the person's own turn, on its first tick: the whole limit, in its box.
    tick_stack(&mut m, &mut game, &assets, 1);
    assert!(!l2_game::turn::turn_in_flight(&game));
    let first = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        timer_digits(&first, &assets, 30),
        Some(timer_digits_expected(&assets, 30)),
        "30 seconds, centred in Ui_DrawNumberRight's fifty pixels from (424, 442), during the \
         person's own turn - with his counter at 1 and every other at 1000",
    );

    // 2 — and the plate under it: every pixel frame 0x60 writes at (404, 430),
    // outside the number's box, is the frame's.
    let mut on_black = Canvas::screen();
    let mut on_white = Canvas::screen();
    on_white.fill_rect(0, 0, 640, 480, 1);
    assert!(ch.draw_misc(&mut on_black, 0x60, 0x194, 0x1AE), "Misc_cty has a frame 0x60");
    ch.draw_misc(&mut on_white, 0x60, 0x194, 0x1AE);
    let (fw, fh) = chrome::frame_size(ch.misc_cty(), 0x60).expect("frame 0x60");
    let mut plate = 0;
    for y in 0x1AE..(0x1AE + fh as usize).min(480) {
        for x in 0x194..(0x194 + fw as usize).min(640) {
            let in_box = (0x1A8 - 2..0x1A8 + 0x32 + 2).contains(&x) && (0x1BA..0x1BA + 20).contains(&y);
            if in_box || on_black.at(x, y) != on_white.at(x, y) {
                continue;
            }
            plate += 1;
            assert_eq!(first.at(x, y), on_black.at(x, y), "frame 0x60's pixel at ({x}, {y})");
        }
    }
    assert!(plate > 0, "the plate writes pixels outside the number's box, or this measured nothing");

    // 3 — 1,937 ticks is 30.992 seconds: the count reads 0, which is not below
    // zero, and 0 is not drawn.
    tick_stack(&mut m, &mut game, &assets, 1936);
    assert!(!l2_game::turn::turn_in_flight(&game), "0 does not end the turn");
    let at_zero = draw_stack(&mut m, &mut game, &assets);
    assert!(
        at_zero.pixels == draw_stack_without_the_timer(&mut m, &mut game, &assets).pixels,
        "0 < DAT_005440C8 fails at 0, so nothing is drawn for the last second",
    );

    // 4 — tick 1,938 is 31.008 seconds, and `Turn_End` begins the turn.
    tick_stack(&mut m, &mut game, &assets, 1);
    assert!(
        l2_game::turn::players_turn_ended(&game),
        "the tick that passes 31 seconds ends the person's turn, with nobody pressing anything",
    );
    let running = draw_stack(&mut m, &mut game, &assets);
    assert!(
        running.pixels == draw_stack_without_the_timer(&mut m, &mut game, &assets).pixels,
        "the count is -1 while the turn it ended runs, and -1 is not drawn",
    );

    // 5 — the turn comes round. The person's counter is held at 999 throughout,
    // which is where `Turn_End` puts it; a restart that tested it would never fire.
    let mut n = 0;
    while l2_game::turn::turn_in_flight(&game) {
        game.kingdom.realms[human].ai_step = l2_kingdom::AI_STEP_DONE;
        tick_stack(&mut m, &mut game, &assets, 1);
        n += 1;
        assert!(n < 4000, "the turn the timer ended never came round");
    }
    // The tick above ended the turn after the clock had counted it, so the
    // restart is the next one: `Turn_Tick`'s phase-4 arm, on the first frame of
    // the person's next turn.
    game.kingdom.realms[human].ai_step = l2_kingdom::AI_STEP_DONE;
    tick_stack(&mut m, &mut game, &assets, 1);
    let next = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        timer_digits(&next, &assets, 30),
        Some(timer_digits_expected(&assets, 30)),
        "the next turn begins at the whole limit again, with every counter at 999 or more",
    );
}

/// **Running out closes the village, and a page hides the timer without
/// stopping it.**
///
/// `DAT_004D2E80` is 1 for `0x0B`, the other lords, so nothing is drawn over
/// that page — but `Turn_Tick` does not ask the screen, so the count goes on
/// under it. The village (`0x02`) is drawn over, and its `Screen_FrameInput`
/// arm opens with the turn-ended guard, `g_screenId = 0` — so the tick the
/// count runs out on takes the village down and the map starts the turn in the
/// same frame.
///
/// Ablations, each observed red: `0x0B`'s byte made 0 (claim 1); the pop loop
/// in `Machine::run_turn_clock` deleted (claim 3).
#[test]
fn running_out_closes_the_village_and_a_page_hides_the_timer_without_stopping_it() {
    let (mut game, assets) = world!();
    game.kingdom.options.time_limit = 30;
    let county = game
        .kingdom
        .county_ids()
        .find(|&c| game.kingdom.counties[c].owner == game.player)
        .expect("the person holds a county") as u8;

    // 1 — over the other lords' page the timer is not drawn, and it is counting.
    let mut m = over_the_map(ScreenId::Diplomacy);
    tick_stack(&mut m, &mut game, &assets, 1);
    assert!(
        draw_stack(&mut m, &mut game, &assets).pixels
            == draw_stack_without_the_timer(&mut m, &mut game, &assets).pixels,
        "DAT_004D2E80[0x0B] is 1",
    );

    // 2 — over the village it is.
    let mut m = over_the_map(ScreenId::Village(county));
    tick_stack(&mut m, &mut game, &assets, 1);
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        timer_digits(&canvas, &assets, 30),
        Some(timer_digits_expected(&assets, 30)),
        "DAT_004D2E80[0x02] is 0, and the count is still 30 after two ticks",
    );

    // 3 — tick 1,938 of the count, two of which were spent above.
    tick_stack(&mut m, &mut game, &assets, 1935);
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Village(county)]);
    assert!(!l2_game::turn::turn_in_flight(&game));
    tick_stack(&mut m, &mut game, &assets, 1);
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "the turn-ended guard closes the village");
    assert!(l2_game::turn::players_turn_ended(&game), "and the map began the turn in the same frame");
}

/// **`DAT_004D2E80` and `&DAT_004D41D0`, against the image.** The transcription
/// in `crate::turn_clock` is two artefacts maintained by one person; this is
/// the check that is not.
#[test]
fn the_turn_timers_screen_table_and_suffix_are_the_images_own() {
    let exe = l2_testkit::executable!();
    let u32_at = |o: usize| u32::from_le_bytes(exe[o..o + 4].try_into().expect("four bytes"));
    let u16_at = |o: usize| u16::from_le_bytes(exe[o..o + 2].try_into().expect("two bytes"));
    let pe = u32_at(0x3C) as usize;
    let sections = u16_at(pe + 6) as usize;
    let table = pe + 24 + u16_at(pe + 20) as usize;
    let bytes_at = |va: u32, n: usize| -> Vec<u8> {
        let rva = va - 0x0040_0000;
        for i in 0..sections {
            let s = table + i * 40;
            let (vsize, vaddr, raw) = (u32_at(s + 8), u32_at(s + 12), u32_at(s + 20));
            if rva >= vaddr && rva < vaddr + vsize {
                let o = (raw + (rva - vaddr)) as usize;
                return exe[o..o + n].to_vec();
            }
        }
        panic!("{va:#X} is in no section");
    };
    assert_eq!(bytes_at(0x004D_2E80, 0x46), l2_game::turn_clock::SCREENS.to_vec(), "DAT_004D2E80");
    assert_eq!(bytes_at(0x004D_41D0, 2), b" \0".to_vec(), "&DAT_004D41D0 is one space");
    assert_eq!(l2_game::turn_clock::SUFFIX, " ");
}
