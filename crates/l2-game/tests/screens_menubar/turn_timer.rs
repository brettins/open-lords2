#![allow(unused_imports)]
use super::*;
use super::menu_bar::*;
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

/// `FUN_0041A639` (`0x0041A639`) draws `Misc_cty` frame `0x60` at (404, 430)
/// and `DAT_005440C8` centred in fifty pixels at (424, 442) under
///
/// ```c
/// 0 < DAT_005440C8 && (DAT_0055403C < 1 || g_realms[g_localPlayer].aiStep == 999)
///                  && 0 < g_optTimeLimit && DAT_004D2E80[g_screenId] == 0
/// ```
#[test]
fn the_turn_timer_is_up_for_the_players_own_turn_and_ends_it_when_it_runs_out() {
    let (mut game, assets) = world!();
    let ch = assets.chrome.as_ref().expect("the install has Misc_cty.pl8");
    game.kingdom.options.time_limit = 30;
    // Turn_Tick 0x0049A010 counts the timer in phase 4 only; the save opens in phase 1.
    l2_game::turn::reach_players_turn(&mut game).expect("phases 1-3 settle");
    let human = game.player as usize;
    for id in 1..game.kingdom.realms.len() {
        game.kingdom.realms[id].ai_step = l2_kingdom::AI_STEP_DONE + 1;
    }
    game.kingdom.realms[human].ai_step = 1;
    let mut m = Machine::new(ScreenId::Campaign);

    tick_stack(&mut m, &mut game, &assets, 1);
    assert!(!l2_game::turn::turn_in_flight(&game));
    let first = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        timer_digits(&first, &assets, 30),
        Some(timer_digits_expected(&assets, 30)),
        "30 seconds, centred in Ui_DrawNumberRight's fifty pixels from (424, 442), during the \
         person's own turn - with his counter at 1 and every other at 1000",
    );

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

    tick_stack(&mut m, &mut game, &assets, 1936);
    assert!(!l2_game::turn::turn_in_flight(&game), "0 does not end the turn");
    let at_zero = draw_stack(&mut m, &mut game, &assets);
    assert!(
        at_zero.pixels == draw_stack_without_the_timer(&mut m, &mut game, &assets).pixels,
        "0 < DAT_005440C8 fails at 0, so nothing is drawn for the last second",
    );

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

    let mut n = 0;
    while l2_game::turn::turn_in_flight(&game) {
        game.kingdom.realms[human].ai_step = l2_kingdom::AI_STEP_DONE;
        tick_stack(&mut m, &mut game, &assets, 1);
        n += 1;
        assert!(n < 4000, "the turn the timer ended never came round");
    }
    // FUN_004B0CB4's fade bytes 0x23/0x24 withhold Turn_Tick (DAT_004E31B0), so the restart waits for it [V].
    let mut fade = 0;
    while game.turn_clock.remaining() != 30 {
        game.kingdom.realms[human].ai_step = l2_kingdom::AI_STEP_DONE;
        tick_stack(&mut m, &mut game, &assets, 1);
        fade += 1;
        assert!(fade < 400, "the count never restarted");
    }
    let next = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        timer_digits(&next, &assets, 30),
        Some(timer_digits_expected(&assets, 30)),
        "the next turn begins at the whole limit again, with every counter at 999 or more",
    );
}

/// `DAT_004D2E80` is 1 for `0x0B`, the other lords, so nothing is drawn over
/// that page — but `Turn_Tick` does not ask the screen, so the count goes on
/// under it. The village (`0x02`) is drawn over; its turn-ended guard never holds in single
/// player unless F12 shut it (DAT_00553018 = 1, 00400000.c:8248; 004b0000.c:2055), so it outlives the clock [V].
#[test]
fn running_out_leaves_the_village_open_and_a_page_hides_the_timer_without_stopping_it() {
    let (mut game, assets) = world!();
    game.kingdom.options.time_limit = 30;
    let county = game
        .kingdom
        .county_ids()
        .find(|&c| game.kingdom.counties[c].owner == game.player)
        .expect("the person holds a county") as u8;

    let mut m = over_the_map(ScreenId::Diplomacy);
    tick_stack(&mut m, &mut game, &assets, 1);
    assert!(
        draw_stack(&mut m, &mut game, &assets).pixels
            == draw_stack_without_the_timer(&mut m, &mut game, &assets).pixels,
        "DAT_004D2E80[0x0B] is 1",
    );

    let mut m = over_the_map(ScreenId::Village(county));
    tick_stack(&mut m, &mut game, &assets, 1);
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert_eq!(
        timer_digits(&canvas, &assets, 30),
        Some(timer_digits_expected(&assets, 30)),
        "DAT_004D2E80[0x02] is 0, and the count is still 30 after two ticks",
    );

    tick_stack(&mut m, &mut game, &assets, 1935);
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Village(county)]);
    assert!(!l2_game::turn::turn_in_flight(&game));
    tick_stack(&mut m, &mut game, &assets, 1);
    // App_WinMain 00400000.c:8248 DAT_00553018 = 1, no F12: no force-close. C249.
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Village(county)], "the village outlives the clock");
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


/// FUN_004BB978 0x004BB978 withholds Turn_Tick under a tip (`DAT_004E31B0[0x27]` = 1), so
/// the count freezes and the turn cannot end; `_DAT_00568D9C` stands, so the first tick
/// after the dismissal ends it [V] 00490000.c:4787. Ablate: run_turn_clock's `if withheld`.
#[test]
fn a_tip_freezes_the_count_and_the_turn_ends_on_the_first_tick_after_it() {
    let (mut game, assets) = world!();
    game.kingdom.options.time_limit = 30;
    l2_game::turn::reach_players_turn(&mut game).expect("phases 1-3 settle");
    let human = game.player as usize;
    for id in 1..game.kingdom.realms.len() {
        game.kingdom.realms[id].ai_step = l2_kingdom::AI_STEP_DONE + 1;
    }
    game.kingdom.realms[human].ai_step = 1;
    let mut m = Machine::new(ScreenId::Campaign);
    tick_stack(&mut m, &mut game, &assets, 1000);
    assert_eq!(game.turn_clock.remaining(), 14, "16 s of a 30 s limit");

    assert!(l2_game::tip::show(&mut game, 200), "a tip is up: g_screenId is 0x27");
    tick_stack(&mut m, &mut game, &assets, 1000);
    assert!(game.tips.hosting());
    assert_eq!(game.turn_clock.remaining(), 14, "Turn_Tick did not run under the tip");
    assert!(!l2_game::turn::players_turn_ended(&game), "32 s, and the turn stands");

    l2_game::message::dismiss(&mut game);
    tick_stack(&mut m, &mut game, &assets, 1);
    assert!(l2_game::turn::players_turn_ended(&game), "the time under the tip counted");
}
