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

/// **A click means the row that was drawn, whatever the directory holds now.**
///
/// The screen reads the directory once, when it opens, and both what it paints
/// and what a click resolves against are that one read. So a save that appears
/// while the screen is up — a second copy of the game saving, a file copied in
/// by hand — cannot move a row out from under the pointer. The test above used
/// to get this wrong in its own code, by resolving a click against a second
/// read; this is the property that kept the screen itself from ever doing so.
///
/// Make the click arm re-read the directory and this loads the save that
/// arrived.
#[test]
fn a_click_on_the_load_screen_means_the_row_it_drew_even_if_a_save_arrived_since() {
    let _own = Saves::new("arrived");
    let (mut game, assets) = bare();
    let shown = furnished(0x5409);
    saves::write("b on screen", &shown).expect("write");
    let mut screen = SaveLoadScreen::new(Mode::Load);

    // One that sorts first, written after the screen opened.
    let arrived = furnished(0xA441);
    saves::write("a arrived later", &arrived).expect("write");
    assert_ne!(digest(&arrived.kingdom), digest(&shown.kingdom));
    assert_ne!(digest(&game.kingdom), digest(&shown.kingdom));

    // Row 0: the screen opened on a directory with one save in it.
    let r = SaveLoadScreen::row_rect(0);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    l2_game::Screen::handle(&mut screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    // Enter is `Edit_Confirm`, the same latch as the thumb up: the load runs when
    // `SaveLoad_Tick`'s count has run out.
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

/// It paints inside the window the painter opens and **nowhere else**, on a
/// machine with no artwork at all — which is what proves the layout does not
/// depend on the install being present.
///
/// **With a save directory too long to fit**, on every machine. The screen
/// writes its directory along the bottom of the box, and this test used to run
/// with a short temporary one — so it passed while a long directory ran out of
/// the side, and went red only when its own directory happened to get longer.
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
fn the_load_screen_refuses_a_file_it_cannot_read_and_stays_open() {
    let own = Saves::new("corrupted");
    let name = "CORRUPTED";
    let (mut game, assets) = bare();

    // A file with our magic and a version from the future.
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
    // Enter arms `SaveLoad_Tick`'s latch; the load is attempted when it runs out.
    let mut t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    for _ in 0..l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
    }

    assert_eq!(t, l2_game::Transition::Stay, "a refused load leaves the screen open");
    assert!(matches!(screen.status(), Status::Failed(_)));
    assert_eq!(digest(&game.kingdom), before, "and the world it refused to replace is untouched");
}

// --- the rolling autosave --------------------------------------------------


