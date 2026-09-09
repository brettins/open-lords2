//! The click-through demo: the front end, the thirteen setup pages, the
//! conquest screen and the shells.
//!
//! Two kinds of test, and the split is the point.
//!
//! * **Structure**, which needs no game: every page lays out, every hotspot is
//!   where the painter puts it, and the navigation graph reaches every screen
//!   and comes back. These run on a bare checkout.
//! * **Fidelity**, which needs the install: `L2.eng` really does say what the
//!   painters' `(group, index)` pairs claim, the two fonts really do map
//!   characters the way `g_glyphWidths` says, and that table really is the
//!   ninety-six bytes at `0x004D71F0` in the user's own `Lords2.exe`. Those
//!   skip without `LORDS2_DIR`.
//!
//! The second kind is what makes the first kind mean anything. A shell that
//! draws group 11 index 0 is only worth having if group 11 index 0 is
//! *"Lords of the Realm 2"*.

use std::env;
use std::path::{Path, PathBuf};

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::shells::{self, SHELLS};
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

fn eng() -> Option<Eng> {
    let dir = install()?;
    Eng::parse(std::fs::read(dir.join("L2.eng")).ok()?).ok()
}

/// A world and assets with no game behind them. Every structural test below
/// runs against this, which is what proves the layout does not depend on the
/// install being present.
fn bare() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

// ------------------------------------------------------------- the machine

#[test]
fn the_front_end_is_the_setup_screens_first_page() {
    let (mut game, assets) = bare();
    let mut m = Machine::new(ScreenId::Setup(SetupPage::Title));
    assert_eq!(m.ids(), vec![ScreenId::Setup(SetupPage::Title)]);

    // "Single player" opens "Your options"; that is one screen changing its
    // own page, not a push, exactly as `g_setupPage` is one screen.
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
        assert_eq!(m.top_id(), Some(ScreenId::Index), "{id:?} would not close");
        assert!(!m.should_quit());
    }
}

/// Every id the demo can build, which is also every row of the index.
fn every_screen() -> Vec<ScreenId> {
    let mut v = vec![
        ScreenId::Campaign,
        ScreenId::County(1),
        ScreenId::Conquest,
        ScreenId::Index,
    ];
    v.extend(SetupPage::ALL.iter().map(|p| ScreenId::Setup(*p)));
    v.extend(SHELLS.iter().map(|s| ScreenId::Shell(s.id)));
    v
}

#[test]
fn a_popup_is_drawn_over_what_was_underneath() {
    let (mut game, assets) = bare();
    game.kingdom.set_county_count(2);

    // The court is an overlay; the merchant, which loads its own 640 x 480
    // background, is not.
    let court = shells::find(0x09).unwrap();
    assert!(court.overlay);
    let merchant = shells::find(0x08).unwrap();
    assert!(!merchant.overlay);

    let mut m = Machine::new(ScreenId::Index);
    m.push(ScreenId::Shell(0x09));
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

#[test]
fn the_conquest_screen_says_all_three_of_the_things_it_can_say() {
    let (mut game, assets) = bare();
    let mut s = ConquestScreen::new();
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(s.outcome());
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        s.handle(Event::KeyDown(Key::Enter), &mut ctx);
    }
    seen.sort_by_key(|o| format!("{o:?}"));
    seen.dedup();
    assert_eq!(seen.len(), 3, "the three branches of FUN_0041E1DD");
}

// ------------------------------------------------------- against the install

#[test]
fn l2_eng_says_what_every_screen_in_the_table_claims_it_says() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    // The front end. If these three are right, page 1 is the front end and
    // `docs/screens-county.md`'s old row for 0x1C was wrong.
    assert_eq!(e.get(11, 0), Some("Lords of the Realm 2"));
    assert_eq!(e.get(11, 2), Some("Single player"));
    assert_eq!(e.get(11, 4), Some("Exit game"));
    assert_eq!(e.get(11, 5), Some("Your options"));
    assert_eq!(e.get(11, 6), Some("Play Now!"));

    // The conquest screen, which is what 0x1C actually is.
    assert_eq!(e.get(36, 0), Some("Congratulations!!"));
    assert_eq!(e.get(36, 1), Some("You have conquered"));
    assert_eq!(e.get(36, 4), Some("You have lost."));

    // The custom game's twelve options and the values they index.
    assert_eq!(e.get(102, 0), Some("Advanced Farming"));
    assert_eq!(e.get(102, 11), Some("Fight?"));
    assert_eq!(e.get(103, 0), Some("off"));
    assert_eq!(e.get(103, 45), Some("all"));
    assert_eq!(e.group(103).len(), 46, "the twelve runs cover 45 of these");

    // And every shell's group is a group that exists and has the index the
    // table draws.
    for s in SHELLS {
        for &(i, _, _) in s.lines {
            assert!(e.get(s.group, i).is_some(), "{}: group {} has no {i}", s.name, s.group);
        }
        if let Some((i, _, _)) = s.heading {
            assert!(e.get(s.group, i).is_some(), "{}: group {} has no {i}", s.name, s.group);
        }
    }
}

#[test]
fn the_option_runs_name_the_values_a_player_of_the_game_would_recognise() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    let values = |i: usize| -> Vec<&str> {
        (0..setup::OPTION_COUNT[i])
            .map(|v| e.get(103, setup::OPTION_BASE[i] + v).unwrap())
            .collect()
    };
    assert_eq!(values(0), vec!["off", "on"], "Advanced Farming");
    assert_eq!(values(2), vec!["two", "three", "four", "five"], "Nobles - never one");
    assert_eq!(values(4), vec!["easy", "normal", "hard", "impossible"], "Difficulty");
    assert_eq!(values(8), vec!["100", "500", "1000", "2500", "5000"], "Crowns");
    assert_eq!(values(11), vec!["humans", "all"], "Fight?");
    // The one string nothing reaches.
    assert_eq!(e.get(103, 4), Some("one"));
}

#[test]
fn the_glyph_map_is_the_table_in_the_users_own_executable() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let exe = std::fs::read(dir.join("Lords2.exe")).expect("Lords2.exe");
    // The image base is 0x400000 and there is no ASLR, so a virtual address is
    // a section offset away from a file offset. `.data` is found by walking the
    // section table rather than by hard-coding the delta.
    let pe = u32::from_le_bytes(exe[0x3C..0x40].try_into().unwrap()) as usize;
    let nsec = u16::from_le_bytes(exe[pe + 6..pe + 8].try_into().unwrap()) as usize;
    let opt = pe + 24;
    let opt_size = u16::from_le_bytes(exe[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let base = u32::from_le_bytes(exe[opt + 28..opt + 32].try_into().unwrap());
    let want = font::GLYPH_MAP_VA - base;
    let mut file_off = None;
    for i in 0..nsec {
        let o = opt + opt_size + i * 40;
        let va = u32::from_le_bytes(exe[o + 12..o + 16].try_into().unwrap());
        let vsize = u32::from_le_bytes(exe[o + 8..o + 12].try_into().unwrap());
        let roff = u32::from_le_bytes(exe[o + 20..o + 24].try_into().unwrap());
        if want >= va && want < va + vsize {
            file_off = Some((roff + (want - va)) as usize);
        }
    }
    let at = file_off.expect("0x004D71F0 is in a section");
    assert_eq!(
        &exe[at..at + 128],
        &font::GLYPH_MAP[..],
        "g_glyphWidths has moved, or the transcription is wrong"
    );
}

#[test]
fn the_fonts_send_descenders_to_the_frames_that_have_them() {
    let Some(dir) = install() else {
        eprintln!("skipping: no game install");
        return;
    };
    let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("the body font parses");
    let frame = |c: char| {
        let e = font::GLYPH_MAP[c as usize - 0x20];
        assert_ne!(e, 0, "{c} has no glyph");
        &pl8.frames[e as usize - 1]
    };
    // The five letters with descenders are taller than the five without, in
    // the same file, under the same map. Nothing but the right mapping does
    // that.
    for d in "gjpqy".chars() {
        for n in "acemn".chars() {
            assert!(
                frame(d).height > frame(n).height,
                "'{d}' should hang below '{n}'"
            );
        }
    }
    // And an uppercase letter is taller again than a lowercase one without an
    // ascender.
    assert!(frame('A').height > frame('a').height);
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

fn click((x, y): (i32, i32)) -> Event {
    Event::Click { x, y }
}
