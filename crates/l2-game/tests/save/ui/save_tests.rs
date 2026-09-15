#![allow(unused_imports)]
use super::*;
use super::load_tests::*;
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

/// **The thumb up clicks on the press
/// 150 frames later.**
///
/// A player, twice: *"no sound on clicking the yes/no on save, and it is still
/// on mousedown instead of mouseup"*. Both halves are the binary's:
/// `g_saveLoadWidgets`' records are `Widget_Test` **kind 4**, which plays
/// `Sound_RestartSlot(1)` and calls the handler on the press
/// `FUN_004342F3`, is only `DAT_005CD41C = 100` — `SaveLoad_Tick` sets
/// `DAT_0057D3C4 = 0x96` and writes when that runs out. Ours answered a raw
/// click and wrote on the spot.
///
/// **Ablations, run:** make `SaveLoadScreen::begin` arm a one-frame wait and
/// *"written on frame 1, before SaveLoad_Tick's 150"* goes red; delete
/// `self.click()` from `Press::press` and the first click count reads 0.
#[test]
fn the_save_thumb_clicks_on_the_press_and_writes_a_hundred_and_fifty_frames_later() {
    use l2_game::screens::saveload::{CONFIRM, WORK_FRAMES};

    let own = Saves::new("latch");
    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> = "LATCH".chars().map(Event::Text).collect();
    // Pressed and let go. **A thumb held down never saves**: it is kind 4, its
    // repeat runs `FUN_004342F3` again, and each run restarts the 150 frames.
    events.push(click_widget(CONFIRM));
    events.push(release_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &events);

    assert_eq!(m.clicks(), 1, "Widget_Test kind 4 plays Sound_RestartSlot(1) on the press");
    assert!(own.files().is_empty(), "nothing is written on the press");
    assert_eq!(m.depth(), 1, "and the box is still up");
    for t in 1..WORK_FRAMES {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
        assert!(own.files().is_empty(), "written on frame {t}, before SaveLoad_Tick's {WORK_FRAMES}");
    }
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.update(&mut ctx);
    assert_eq!(own.files(), vec![file("latch")], "written on frame {WORK_FRAMES}");
    assert!(m.should_quit() || m.depth() == 0, "and the box closed");
    assert_eq!(m.clicks(), 1, "the save itself is silent");
}

#[test]
fn the_save_screen_writes_a_file_and_the_load_screen_reads_it_back() {
    use l2_game::screens::saveload::{CONFIRM, LIST};

    let own = Saves::new("screens");
    // Four saves that sort before the one this test makes, all of a different
    // game
    // the digest says so.
    let decoy = furnished(0xDEC0);
    let decoys = ["a decoy", "b decoy", "c decoy", "d decoy"];
    for n in decoys {
        saves::write(n, &decoy).expect("write");
    }

    // **What is typed and what is saved are different strings now**
    // difference is the point. The save box is `Edit_Begin(&DAT_004EA130, 8,
    // 0xA0, 1)` — **kind 1**, a DOS file name — so `Edit_TypeChar` runs
    // `A`–`Z` through `0x004011B0` and lower-cases them. Typing `SCREENTEST`
    // gives `screentest`, which is what the original's own file box does and
    // what its file list shows.
    let typed_name = "SCREEN TEST";
    let name = "screen test";

    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let before = digest(&game.kingdom);
    assert_ne!(digest(&decoy.kingdom), before);

    // Save: type a name, then press the tick. Nothing here reaches into the
    // screen's fields; it is clicks and keys.
    //
    // **`Event::Text`, not `Event::KeyDown`.** They are `WM_CHAR` and
    // `WM_KEYDOWN` and the field reads the first
    // does — `Key::Char` is folded to upper case for the hotkey matchers
    // field fed from it could never produce a lower-case letter at all. This
    // test drove the old hand-rolled field through the hotkey message and is
// the reason that distinction is now enforced. The
    // space in the middle is deliberate: a space is a character here, not a
    // confirm, and driving it proves the field takes it.
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut typed: Vec<Event> = typed_name.chars().map(Event::Text).collect();
    typed.push(click_widget(CONFIRM));
    typed.push(release_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &typed);
    settle(&mut m, &mut game, &assets);
    assert!(m.should_quit() || m.depth() == 0, "the save screen closes when it has saved");
    let mut expected: Vec<String> = decoys.iter().map(|n| file(n)).collect();
    expected.push(file(name));
    assert_eq!(own.files(), expected, "the file is on disk, and nothing else was written");

    // Now change the world, and load it back.
    let mut game2 = furnished(0xDEAD);
    game2.prefs.tip_screens = false;
    assert_ne!(digest(&game2.kingdom), before);
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Load));
    // **Row 4, and it is a literal.** The directory holds exactly the five
    // files asserted above
    // so the list this screen opened on has `screen test` fifth — the middle
    // column of the second line, which exercises both halves of the geometry.
    //
    // It used to be found by taking a *second* `saves::list()` after the
    // screen had opened. That answered for the directory as it stood at that
    // statement, not as the screen had read it, and in a directory shared with
    // other tests those were sometimes different lists.
    const ROW: usize = 4;
    let r = SaveLoadScreen::row_rect(ROW);
    drive(
        &mut m,
        &mut game2,
        &assets,
        &[Event::Click { x: r.x + 2, y: r.y + 2 }, click_widget(CONFIRM), release_widget(CONFIRM)],
    );
    settle(&mut m, &mut game2, &assets);
    assert_eq!(digest(&game2.kingdom), before, "the loaded game is the saved one");
    assert_eq!(LIST.1, r.y - (ROW / 3) as i32 * 16, "the row geometry is the painter's");
}

/// **The words on these two screens are the game's**, and this is what makes
/// that claim checkable: `Mode::heading_index` returns the
/// painter's own `saving` argument, and `Eng_DrawString(40, saving, …)` uses it
/// as a string index, so index 0 and index 1 have to be those two sentences in
/// the user's own `L2.eng`.
#[test]
fn the_headings_and_the_status_line_are_l2_engs_own_words() {
    use l2_game::screens::saveload::{ERROR_INDEX, GROUP};
    use l2_game::shell::Eng;

    let Some(dir) = l2_testkit::install_dir() else {
        eprintln!("SKIP the_headings_and_the_status_line_are_l2_engs_own_words: no game install");
        return;
    };
    let e = Eng::parse(std::fs::read(dir.join("L2.eng")).expect("L2.eng")).expect("parses");

    assert_eq!(e.get(GROUP, Mode::Load.heading_index()), Some("Loading a conquest."));
    assert_eq!(e.get(GROUP, Mode::Save.heading_index()), Some("Saving a conquest."));
    assert_eq!(e.get(GROUP, Mode::Load.working_index()), Some("Loading game. Please wait."));
    assert_eq!(e.get(GROUP, Mode::Save.working_index()), Some("Saving game. Please wait."));
    assert_eq!(e.get(GROUP, ERROR_INDEX), Some("File error. Operation canceled."));
}

#[test]
fn the_cancel_cross_closes_without_writing_anything() {
    use l2_game::screens::saveload::CANCEL;

    let own = Saves::new("cancel");
    let (mut game, assets) = bare();
    // **A name that would be written if the cross confirmed**, typed the way
    // the field reads it. This used to type through `Event::KeyDown`, which the
    // field ignores, and then look for `CANCELLED` — a name the file-name field
    // would have lower-cased — so a cross that saved could not have turned it
    // red twice over. An empty directory of its own is the whole assertion now.
    let mut m = Machine::new(ScreenId::SaveLoad(Mode::Save));
    let mut events: Vec<Event> = "CANCELLED".chars().map(Event::Text).collect();
    events.push(click_widget(CANCEL));
    drive(&mut m, &mut game, &assets, &events);
    assert!(m.should_quit() || m.depth() == 0, "the cross closes the screen");
    assert_eq!(own.files(), Vec::<String>::new(), "cancelling must not write a file");
}

#[test]
fn a_save_name_the_file_system_would_choke_on_is_reported_and_not_written() {
    let own = Saves::new("refused-name");
    let (mut game, assets) = bare();
    let mut screen = SaveLoadScreen::new(Mode::Save);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    // The name field cannot hold a separator — `Key::Char` never carries one —
    // so the refusal is reached the way a player would reach it: an empty name.
    // Enter arms `SaveLoad_Tick`'s latch
    let mut t = l2_game::Screen::handle(&mut screen, Event::KeyDown(Key::Enter), &mut ctx);
    for _ in 0..l2_game::screens::saveload::WORK_FRAMES {
        t = l2_game::Screen::update(&mut screen, &mut ctx);
    }
    assert_eq!(t, l2_game::Transition::Stay, "a refused save leaves the screen open");
    assert!(matches!(screen.status(), Status::Failed(_)), "and it says so");
    assert_eq!(own.files(), Vec::<String>::new(), "and nothing was written");
}


/// **The save the player actually makes: from the menu bar, over the campaign
/// map.** A player: *"it says saving game please wait, then stays on that
/// screen … the new save isn't there"*.
///
/// Every other test on this screen builds a `Machine` whose *only* screen is
/// the box, so nothing is under it and [`Machine::update`] reaches the box on
/// every frame. In the game the box is pushed over screen `0x24`, and
/// `Machine::wind_turn` — `Battle_Frame`'s `Turn_Tick(); Units_Tick();` at
/// `0x004B99C0` — runs the map underneath first. When that wind-on moves the
/// stack the machine returns early and the top screen gets no tick at all, so
/// `SaveLoad_Tick`'s `DAT_0057D3C4` never counts down and the box sits on
/// *"Saving game. Please wait."* for ever.
#[test]
fn a_save_from_the_menu_bar_over_the_campaign_map_finishes_and_closes() {
    use l2_game::screens::menubar;
    use l2_game::screens::saveload::{CONFIRM, WORK_FRAMES};

    let own = Saves::new("menubar-save");
    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    // **The player has been sitting on the map**, so the AI realms have taken
    // their `AI_RunTurnStep` steps and `players_turn_open` is set — the state
    // `turn::tick_ai_frame` leaves behind and the state a real save is made in.
    for _ in 0..400 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the map is his to save from");
    let titles = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        menubar::titles(&ctx)
    };
    // File, then its third item — `MENUS[0].items[2]` is `Item::Save`.
    let row = menubar::item_rect(&titles, 0, 2);
    drive(
        &mut m,
        &mut game,
        &assets,
        &[Event::Click { x: titles[0].x + 2, y: 10 }, Event::Click { x: row.x + 4, y: row.y + 4 }],
    );
    assert_eq!(m.top_id(), Some(ScreenId::SaveLoad(Mode::Save)), "File / Save");
    assert_eq!(m.depth(), 2, "and the map is still under it");

    let mut events: Vec<Event> = "OVERMAP".chars().map(Event::Text).collect();
    events.push(click_widget(CONFIRM));
    events.push(release_widget(CONFIRM));
    drive(&mut m, &mut game, &assets, &events);
    for _ in 0..=WORK_FRAMES {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert_eq!(own.files(), vec![file("overmap")], "the file the player asked for");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the box is gone");
}
