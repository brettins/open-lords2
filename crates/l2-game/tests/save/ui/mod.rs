#![allow(unused_imports)]

mod save_tests;
pub use save_tests::*;
mod load_tests;
pub use load_tests::*;

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


pub(super) fn bare() -> (Game, Assets) {
    (played(1), Assets::placeholder())
}

pub(super) fn drive(m: &mut Machine, game: &mut Game, assets: &Assets, events: &[Event]) {
    for e in events {
        let mut ctx = Ctx { game, assets };
        m.handle(*e, &mut ctx);
    }
}

fn click_widget(w: (i32, i32, usize, i32)) -> Event {
    Event::Click { x: w.0 + w.3 / 2, y: w.1 + w.3 / 2 }
}

fn release_widget(w: (i32, i32, usize, i32)) -> Event {
    Event::Release { x: w.0 + w.3 / 2, y: w.1 + w.3 / 2 }
}

fn settle(m: &mut Machine, game: &mut Game, assets: &Assets) {
    for _ in 0..=l2_game::screens::saveload::WORK_FRAMES {
        if m.depth() == 0 || m.should_quit() {
            return;
        }
        let mut ctx = Ctx { game, assets };
        m.update(&mut ctx);
    }
}

