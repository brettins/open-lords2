#![allow(unused_imports)]
use super::*;
use super::conquest::*;
use super::eng_part::*;
use super::font_part::*;
use super::glyph::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::confirm;
use l2_game::screens::county::Panel;
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::court::CourtScreen;
use l2_game::screens::ratings::RatingsScreen;
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

#[test]
fn the_front_end_is_the_setup_screens_first_page() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Title));
    assert_eq!(m.ids(), vec![ScreenId::Setup(SetupPage::Title)]);

    // "Single player" opens "Your options"; that is one screen changing its
// own page, as `g_setupPage` is one screen.
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(click(setup_item(0)), &mut ctx);
    assert_eq!(m.ids(), vec![ScreenId::Setup(SetupPage::Options)]);
    assert_eq!(m.depth(), 1, "a page change must not grow the stack");
}

#[test]
fn back_walks_the_page_graph_the_original_has() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Title));
    let mut ctx = Ctx { game: &mut game, assets: &assets };

    // Title -> Options -> Custom game -> back to Options -> back to Title.
    m.handle(click(setup_item(0)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Options)));
    m.handle(click(setup_item(3)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Custom)));
    // "Cancel" is the first of the custom page's bottom buttons.
    m.handle(click(custom_button(0)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Options)));
    m.handle(click(setup_item(4)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Title)));
}

#[test]
fn a_drop_down_opens_over_its_page_and_puts_the_value_back() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Custom));
    let mut ctx = Ctx { game: &mut game, assets: &assets };

    // Option 4 is Difficulty, which has four values.
    let (x, y, _) = setup::OPTION_CELLS[4];
    m.handle(click((x + 8, y + 8)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Dropdown)));

    // Its third value, "hard": the list starts one cell into the box.
    let (lx, ly, _) = setup::OPTION_LIST[4];
    m.handle(click((lx + 8, ly + 16 + 2 * 16 + 8)), &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Setup(SetupPage::Custom)));
    assert_eq!(
        m.ids().len(),
        1,
        "the drop-down is a page of the same screen, not a screen of its own"
    );
}

#[test]
fn every_screen_the_index_lists_opens_over_it_draws_and_closes_again() {
    let (mut game, assets) = bare();
    for id in every_screen() {
        let mut m = Machine::new(ScreenId::Index);
        m.push(id);
        assert_eq!(m.depth(), 2);

        let mut canvas = Canvas::screen();
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.draw(&ctx, &mut canvas);
            // Escape backs out. The setup screen is thirteen pages behind one
            // `ScreenId`, so it takes at most two presses — one to the title
            // page, one off the screen — and every other screen takes one.
            for _ in 0..3 {
                if m.top_id() == Some(ScreenId::Index) {
                    break;
                }
                m.handle(Event::KeyDown(Key::Escape), &mut ctx);
            }
        }
        assert!(canvas.count(0) < 640 * 480, "{id:?} drew nothing at all");
        // **The campaign map no longer backs out on Escape.** In a game the key
        // is `Menu_Quit` (`App_WndProc` `0x004B29BE`), so it raises the yes/no
        // box (`0x1E`) and the box's answer decides — `tests/confirm_box.rs`.
        // The index still reached it, drew it, and got a screen back.
        if id == ScreenId::Campaign {
            assert_eq!(m.top_id(), Some(ScreenId::Confirm(confirm::Ask::Quit)));
            assert!(!m.should_quit());
            continue;
        }
        assert_eq!(m.top_id(), Some(ScreenId::Index), "{id:?} would not close");
        assert!(!m.should_quit());
    }
}

/// Every id the demo can build, which is also every row of the index.
fn every_screen() -> Vec<ScreenId> {
    let mut v = vec![
        ScreenId::Campaign,
        ScreenId::County(1, Panel::Tax),
        ScreenId::Conquest,
        ScreenId::Index,
    ];
    v.extend(SetupPage::ALL.iter().map(|p| ScreenId::Setup(*p)));
    // The last seven shells, now seven screens.
    v.extend([
        ScreenId::About,
        ScreenId::Court,
        ScreenId::Diplomacy,
        ScreenId::Supplies(1),
        ScreenId::Ratings,
        ScreenId::Info(l2_game::screens::info::Target::Tile(0)),
        ScreenId::Info(l2_game::screens::info::Target::Unit(1)),
    ]);
    v
}

#[test]
fn a_popup_is_drawn_over_what_was_underneath() {
    let (mut game, assets) = bare();
    game.kingdom.set_county_count(2);

    // The court is an overlay; the battle master ratings, which load their own
    // 640 × 480 background, are not. Those are the two kinds of screen this
    // test is about.
    //
    // **This test outlived four of its own examples and then the whole
    // table.** The merchant stood here, then the armoury, then castle
    // building, then the court - every whole-picture shell the table had -
    // and the note here said that when the last one graduated this should go
// red and be deleted deliberately
    // set. It went red. This is that deliberate rewrite.
    //
// **an overlay
    // does not clear what is underneath it** - the property
    // `docs/decisions.md` C22 was written about
    // `Machine::draw` walks back to the last non-overlay screen. So it names
    // two graduated screens instead: the court, which paints over the map,
    // and the Battle Master ratings
    // The claim outlives its examples, which is what a claim is for.
    assert!(CourtScreen::new().is_overlay(), "the court paints over what opened it");
    assert!(!RatingsScreen::new().is_overlay(), "the ratings screen is a page of its own");

    let mut m = Machine::new(ScreenId::Index);
    m.push(ScreenId::Court);
    assert_eq!(m.depth(), 2);
    let mut alone = Machine::new(ScreenId::Index);
    let (mut over, mut under) = (Canvas::screen(), Canvas::screen());
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        m.draw(&ctx, &mut over);
        alone.draw(&ctx, &mut under);
    }

    assert!(over.diff_count(&under) > 0, "the popup must have drawn something");
    // The index's own title line is at y = 10 and the court's window starts at
    // y = 48, so the line above it must survive the popup.
    let untouched = (0..640).filter(|&x| over.at(x as usize, 10) == under.at(x as usize, 10)).count();
    assert_eq!(untouched, 640, "a popup must not clear what was underneath it");
}

#[test]
fn every_setup_page_and_every_shell_draws_without_the_game() {
    let (mut game, assets) = bare();
    for id in every_screen() {
        let mut screen = id.build();
        let mut canvas = Canvas::screen();
        screen.draw(&Ctx { game: &mut game, assets: &assets }, &mut canvas);
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert!(!screen.title(&ctx).is_empty(), "{id:?} has no window title");
    }
}

/// **`L2.eng` 295, *"What should I do each turn?"*, is eleven strings, and nine
/// of them read as missing.** Each of 295/2 … 295/10 opens with byte `0xB7`, and
/// `Eng::get` ran `from_utf8` on the bytes and returned `None` when it failed —
/// under a comment saying the file was Latin-1 and that `from_utf8` would reject
/// its accented bytes.
///
/// Ablated: `Eng::get` put back to `from_utf8(…).ok()` — red at the `expect` on
/// 295/9, which does not read back.
#[test]
fn the_each_turn_help_page_keeps_its_nine_bullets() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    assert_eq!(e.get(295, 0), Some("What should I do each turn?"));
    let bullet = e.get(295, 9).expect("295/9 reads back");
    assert!(bullet.starts_with('\u{B7}'), "{bullet:?}");
    assert!(bullet.ends_with("Adjust rations."), "{bullet:?}");
    assert_eq!(e.group(295).len(), 11, "a title and ten lines");
}

// ------------------------------------------------------------------ helpers

/// The n'th menu item of setup pages 1 and 2, in canvas coordinates.
fn setup_item(i: usize) -> (i32, i32) {
    (0xE0 + 8, 0x5B + i as i32 * 0x24 + 8)
}

/// The n'th bottom button of the custom-game page.
fn custom_button(i: usize) -> (i32, i32) {
    ([0xA5, 0xF3, 0x141, 399][i] + 8, 0xC6 + 4)
}

pub(crate) fn click((x, y): (i32, i32)) -> Event {
    Event::Click { x, y }
}

