#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use super::timing_tests::*;
use super::sound_tests::*;
use super::close_tests::*;
use super::*;
use super::page_quirks::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

#[test]
fn each_widget_toggles_its_own_row_and_only_its_own() {
    for page in ORIGINALS {
        for row in page.rows() {
            let (mut game, assets) = world();
            let before: Vec<bool> =
                page.rows().iter().map(|r| value(r.setting, &mut game, &assets)).collect();

            let mut screen = OptionsScreen::new(page);
            let (x, y) = mid(row.hit());
            let t = press_and_wait(&mut screen, &mut game, &assets, x, y);
            let leaves = row.setting == Setting::FullScreen;
            assert_eq!(t == Transition::Pop, leaves, "{page:?} {:?} answered {t:?}", row.setting);

            let after: Vec<bool> =
                page.rows().iter().map(|r| value(r.setting, &mut game, &assets)).collect();

            for (i, r) in page.rows().iter().enumerate() {
                if r.setting == row.setting {
                    if row.supported() {
                        assert_ne!(before[i], after[i], "{page:?} {:?} did not move", r.setting);
                    } else {
                        assert_eq!(before[i], after[i], "{page:?} {:?} is not ours to honour", r.setting);
                    }
                } else {
                    assert_eq!(before[i], after[i], "{page:?} {:?} moved and should not have", r.setting);
                }
            }
        }
    }
}

#[test]
fn a_near_miss_neither_toggles_nor_closes() {
    for page in ORIGINALS {
        for row in page.rows() {
            let (mut game, assets) = world();
            let before = value(row.setting, &mut game, &assets);
            let mut screen = OptionsScreen::new(page);
            let r = row.hit();
            for (x, y) in [
                (r.x - 1, r.y + r.h / 2),
                (r.x + r.w, r.y + r.h / 2),
                (r.x + r.w / 2, r.y - 1),
                (r.x + r.w / 2, r.y + r.h),
            ] {
                assert_eq!(
                    press_and_wait(&mut screen, &mut game, &assets, x, y),
                    Transition::Stay,
                    "{page:?}: a click at ({x}, {y}) closed the panel"
                );
                let now = value(row.setting, &mut game, &assets);
                assert_eq!(before, now, "{page:?}: a click at ({x}, {y}) toggled {:?}", row.setting);
                assert!(screen.pressed_rows().is_empty(), "{page:?}: a miss at ({x}, {y}) went down");
            }
        }
    }
}

#[test]
fn no_click_anywhere_is_ever_passed_to_the_screen_underneath() {
    for page in Page::ALL {
        let (mut game, assets) = world();
        let mut screen = OptionsScreen::new(page);
        for x in (0..640).step_by(37) {
            for y in (0..480).step_by(31) {
                let t = click(&mut screen, &mut game, &assets, x, y);
                assert_ne!(t, Transition::Pass, "{page:?}: ({x}, {y}) fell through to the map");
            }
        }
    }
}


