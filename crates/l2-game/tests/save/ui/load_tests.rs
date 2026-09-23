#![allow(unused_imports)]
use super::*;
use super::save_tests::*;
use super::*;
use super::roundtrip::*;
use super::isolation::*;
use super::autosave::*;
use std::path::PathBuf;
use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

#[test]
fn a_click_on_the_load_screen_means_the_row_it_drew_even_if_a_save_arrived_since() {
    let _own = Saves::new("arrived");
    let (mut game, assets) = bare();
    let shown = furnished(0x5409);
    saves::write("b on screen", &shown).expect("write");
    let mut screen = SaveLoadScreen::new(Mode::Load);

    let arrived = furnished(0xA441);
    saves::write("a arrived later", &arrived).expect("write");
    assert_ne!(digest(&arrived.kingdom), digest(&shown.kingdom));
    assert_ne!(digest(&game.kingdom), digest(&shown.kingdom));

    let r = SaveLoadScreen::row_rect(0);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    let mut t = l2_game::Transition::Stay;
    for _ in 0..=l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
        if t != l2_game::Transition::Stay {
            break;
        }
    }
    assert_eq!(t, l2_game::Transition::Pop, "the load went through");
    assert_eq!(
        digest(&game.kingdom),
        digest(&shown.kingdom),
        "the click loaded a save that was not the row it landed on"
    );
}

/// g_battleCounty is `.bss`, outside Save_Read 0x004ADFE6's blocks [I]: a load keeps the process value.
/// Ablation: `game.kingdom.campaign.battle_county = ...` in the load arm, and this fails.
#[test]
fn a_load_keeps_the_battle_county_the_process_held() {
    let _own = Saves::new("battle-county");
    let (mut game, assets) = bare();
    saves::write("kept", &furnished(0x5409)).expect("write");
    game.kingdom.campaign.battle_county = 17;
    let mut screen = SaveLoadScreen::new(Mode::Load);
    let r = SaveLoadScreen::row_rect(0);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    let mut t = l2_game::Transition::Stay;
    for _ in 0..=l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
        if t != l2_game::Transition::Stay {
            break;
        }
    }
    assert_eq!(t, l2_game::Transition::Pop, "the load went through");
    assert_eq!(game.kingdom.campaign.battle_county, 17);
}

#[test]
fn the_screen_paints_inside_its_own_window_and_nothing_outside_it() {
    use l2_game::screens::saveload::{BOX_COLS, BOX_ROWS, BOX_X, BOX_Y};
    use l2_view::Canvas;

    let _own = Saves::new(&"a-save-directory-far-too-long-to-fit-in-the-box".repeat(2));
    assert!(saves::dir().unwrap().display().to_string().len() > 100, "long enough to overflow");
    let (mut game, assets) = bare();
    let mut screen = SaveLoadScreen::new(Mode::Save);
    let mut canvas = Canvas::screen();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        l2_game::Screen::draw(&mut screen, &ctx, &mut canvas);
    }
    let mut outside = 0usize;
    for y in 0..480i32 {
        for x in 0..640i32 {
            let inside = x >= BOX_X
                && x < BOX_X + BOX_COLS * 16
                && y >= BOX_Y
                && y < BOX_Y + BOX_ROWS * 16;
            if !inside && canvas.at(x as usize, y as usize) != 0 {
                outside += 1;
            }
        }
    }
    assert_eq!(outside, 0, "an overlay must not paint outside its own box");
    assert!(canvas.count(0) < 640 * 480, "and it must paint something inside it");
}

#[test]
fn a_load_that_cannot_read_its_file_closes_the_box_silently() {
    let own = Saves::new("corrupted");
    let name = "CORRUPTED";
    let (mut game, assets) = bare();

    let mut bytes = save::encode(&game);
    bytes[8..12].copy_from_slice(&(save::VERSION + 1).to_le_bytes());
    std::fs::write(own.path.join(file(name)), &bytes).expect("write");

    let before = digest(&game.kingdom);
    let mut screen = SaveLoadScreen::new(Mode::Load);
    let row = screen.entries().iter().position(|e| e.name == name).expect("listed");
    assert!(row < 30, "the test's own save must be on the first page, not row {row}");
    let r = SaveLoadScreen::row_rect(row);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    let mut t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    for _ in 0..l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
        if t != l2_game::Transition::Stay {
            break;
        }
    }

    // SaveLoad_Tick 0x004AD9F0 :5746, g_screenId restored before Save_Read.
    assert_eq!(t, l2_game::Transition::Pop, "a failed load closes the box");
    assert_eq!(digest(&game.kingdom), before, "and the world it refused to replace is untouched");
}




#[test]
fn a_load_of_a_name_with_no_file_shows_the_error_line_then_clears_it() {
    use l2_game::screens::saveload::FAIL_TICKS;
    let _own = Saves::new("latch-missing");
    let (mut game, assets) = bare();
    let before = digest(&game.kingdom);
    let mut screen = SaveLoadScreen::new(Mode::Load);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    for c in "NOSUCH".chars() {
        l2_game::Screen::handle(&mut screen, Event::Text(c), &mut ctx);
    }
    l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    // SaveLoad_Tick 0x004AD9F0 :5791-5794: the latch, and group 40/4 while the count runs.
    assert!(matches!(screen.status(), Status::Failed(_)), "the error line shows at once");
    for _ in 1..FAIL_TICKS {
        assert_eq!(l2_game::Screen::update(&mut screen, &mut ctx), l2_game::Transition::Stay);
        assert!(matches!(screen.status(), Status::Failed(_)), "for the whole count");
    }
    // :5768, the count ends by clearing the latch: no load, box open, line blank.
    assert_eq!(l2_game::Screen::update(&mut screen, &mut ctx), l2_game::Transition::Stay);
    assert_eq!(screen.status(), &Status::Idle);
    assert_eq!(digest(&game.kingdom), before);
}
