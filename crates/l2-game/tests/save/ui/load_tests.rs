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
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
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

/// Loads the named save through the box; returns the transition after the failure count.
fn load_through(screen: &mut SaveLoadScreen, game: &mut Game, assets: &Assets, name: &str) -> l2_game::Transition {
    let mut ctx = Ctx { game, assets };
    let row = screen.entries().iter().position(|e| e.name == name).expect("listed");
    let r = screen.row(row);
    l2_game::Screen::handle(screen, Event::Click { x: r.x + 2, y: r.y + 2 }, &mut ctx);
    let mut t = l2_game::Screen::handle(screen, Event::KeyDown(Key::Enter), &mut ctx);
    let frames = l2_game::screens::saveload::WORK_FRAMES as usize;
    for _ in 0..frames + usize::from(l2_game::screens::saveload::FAIL_TICKS) + 2 {
        t = l2_game::Screen::update(screen, &mut ctx);
        if t != l2_game::Transition::Stay {
            break;
        }
    }
    t
}

fn write_versioned(own: &Saves, name: &str, game: &Game, version: u32) {
    let mut bytes = save::encode(game);
    bytes[8..12].copy_from_slice(&version.to_le_bytes());
    std::fs::write(own.path.join(file(name)), &bytes).expect("write");
}

/// CNEW-save-compat: ablate the `Err(e) => match load_failure` arm back to `Pop` and it fails.
#[test]
fn an_in_game_load_of_another_format_says_so_and_keeps_the_box_up() {
    let own = Saves::new("newer");
    let (mut game, assets) = bare();
    write_versioned(&own, "NEWER", &game, save::VERSION + 1);
    let before = digest(&game.kingdom);
    let mut screen = SaveLoadScreen::new(Mode::Load);
    let t = load_through(&mut screen, &mut game, &assets, "NEWER");
    assert_eq!(t, l2_game::Transition::Stay, "the box stays up");
    // ablate the Mismatch guard in the latch's count end and the line goes blank.
    assert_eq!(
        screen.status(),
        &Status::Mismatch { found: save::VERSION + 1, expected: save::VERSION },
        "and the line outlives the failure count"
    );
    assert_eq!(digest(&game.kingdom), before);
}

#[test]
fn the_front_end_names_an_older_format_rather_than_file_error() {
    let own = Saves::new("older");
    let (mut game, assets) = bare();
    write_versioned(&own, "OLDER", &game, save::MIN_READABLE - 1);
    let mut screen = SaveLoadScreen::front_end();
    assert_eq!(load_through(&mut screen, &mut game, &assets, "OLDER"), l2_game::Transition::Stay);
    assert_eq!(
        screen.status(),
        &Status::Mismatch { found: save::MIN_READABLE - 1, expected: save::VERSION }
    );
}

#[test]
fn the_mismatch_line_says_older_or_newer_and_both_numbers() {
    use l2_game::screens::saveload::mismatch_lines;
    let [a, b] = mismatch_lines(7, 8);
    assert_eq!(a, "This save was made by an older build of the game");
    assert_eq!(b, "(save format 7; this build reads 8).");
    assert!(mismatch_lines(9, 8)[0].contains("a newer build"));
}

/// 40/4 stays the original's line for every failure that is not a version: SaveLoad_Tick 0x004AD9F0 :5791-5794.
#[test]
fn a_file_error_is_still_the_originals_line_and_the_in_game_box_still_closes() {
    use l2_game::screens::saveload::load_failure;
    let io = saves::Error::Io { path: PathBuf::from("x"), detail: "gone".into() };
    assert!(matches!(load_failure(&io, true), Some(Status::Failed(_))));
    assert_eq!(load_failure(&io, false), None);
    let world = saves::Error::Load {
        name: "w".into(),
        detail: LoadError::Kingdom(l2_kingdom::save::LoadError::UnsupportedVersion { found: 31, supported: 33 }),
    };
    assert_eq!(load_failure(&world, false), Some(Status::Mismatch { found: 31, expected: 33 }));
}

/// CNEW-save-compat's policy against every fixture of ours: a file inside
/// `MIN_READABLE..=VERSION` on both layers loads, and every other is refused on its version.
#[test]
fn every_fixture_either_loads_or_is_refused_on_its_version() {
    let Ok(root) = std::env::var("LORDS2_FIXTURES") else { return };
    let Ok(dir) = std::fs::read_dir(PathBuf::from(root).join("ours")) else { return };
    let (game_range, world_range) = (
        save::MIN_READABLE..=save::VERSION,
        l2_kingdom::save::MIN_READABLE..=l2_kingdom::save::VERSION,
    );
    // Scaffold: no world-format 32 fixture exists (all are 31); one inside the range must load.
    for path in dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "l2sav")) {
        let Err(e) = saves::read_path(&path, Tables::DEFAULT) else { continue };
        let (found, expected) = e.version_mismatch().unwrap_or_else(|| panic!("{}: {e}", path.display()));
        let range = if expected == save::VERSION { &game_range } else { &world_range };
        assert!(!range.contains(&found), "{}: format {found} is readable and was refused", path.display());
    }
}
