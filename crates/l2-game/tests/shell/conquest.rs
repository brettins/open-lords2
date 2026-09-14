#![allow(unused_imports)]
use super::*;
use super::navigation::*;
use super::eng_part::*;
use super::font_part::*;
use super::glyph::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::court::CourtScreen;
use l2_game::screens::ratings::RatingsScreen;
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

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

