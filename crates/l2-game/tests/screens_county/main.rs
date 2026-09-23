

#[macro_use]
#[path = "../common/mod.rs"]
mod common;

mod strip_and_sidebar;
mod panels;
mod drawing_and_emboss;
use drawing_and_emboss::*;
mod produce_and_pastures;
mod layout_and_labels;

use common::*;

use l2_game::game::Assets;
use l2_game::screen::Ctx;
use l2_game::screen::Screen;
use l2_game::Game;

fn run_turn<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    let mut done_at = None;
    for n in 1..2_000u32 {
        let mut ctx = Ctx { game, assets };
        screen.wind_turn(&mut ctx);
        screen.update(&mut ctx);
        if done_at.is_none() && game.kingdom.turn_count > before {
            done_at = Some(n);
        }
        if let Some(t) = done_at {
            if n >= t + l2_view::fade::PHASES as u32 {
                return;
            }
        }
    }
    panic!("the turn never came round");
}

const STRIP_BAD: u8 = l2_game::shell::font::HIGHLIGHT;

