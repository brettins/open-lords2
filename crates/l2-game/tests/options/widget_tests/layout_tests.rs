#![allow(unused_imports)]
use super::*;
use super::interaction_tests::*;
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

/// Every page's window, close button and every widget box is on the 640 × 480
/// screen, and no two boxes on one page overlap.
///
/// The overlap half is the one that matters: two hotspots sharing a pixel is a
/// click whose meaning depends on the order of a `for` loop.
#[test]
fn every_box_is_on_screen_and_no_two_on_a_page_overlap() {
    for page in Page::ALL {
        let (x, y, cols, rows, _) = page.window();
        assert!(x >= 0 && y >= 0, "{page:?}");
        assert!(x + cols * 16 <= 640, "{page:?} is {} wide", x + cols * 16);
        assert!(y + rows * 16 <= 480, "{page:?} is {} tall", y + rows * 16);

        let close = page.close_hit();
        assert!(close.x + close.w <= 640 && close.y + close.h <= 480, "{page:?}'s close button");

        let mut boxes: Vec<l2_game::input::Rect> =
            page.rows().iter().map(|r| r.hit()).collect();
        boxes.push(close);
        for (i, a) in boxes.iter().enumerate() {
            for b in boxes.iter().skip(i + 1) {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{page:?}: two hotspots overlap at {a:?} and {b:?}");
            }
        }
    }
}

