#![allow(unused_imports)]
use super::*;
use super::county_name_tests::*;
use super::panel_tests::*;
use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
use super::produce_and_pastures::*;
use super::layout_and_labels::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

fn body_mask(assets: &Assets, s: &str) -> Vec<(i32, i32)> {
    let font = assets.shell.body.as_ref().expect("the body font");
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let (w, h) = (font.width(s).max(1), font.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect()
}

pub(crate) fn emboss_at(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(u8, u8)> {
    let mask = body_mask(assets, s);
    let (ox, oy) = find_body(canvas, assets, s, colour)?;
    let inside = |dx: i32, dy: i32| mask.contains(&(dx, dy));
    let mut up: Option<u8> = None;
    let mut down: Option<u8> = None;
    for &(mx, my) in &mask {
        if !inside(mx, my + 1) {
            let got = canvas.at((ox + mx) as usize, (oy + my + 1) as usize);
            if *down.get_or_insert(got) != got {
                return None;
            }
        }
        if !inside(mx, my - 1) && !inside(mx, my - 2) {
            let got = canvas.at((ox + mx) as usize, (oy + my - 1) as usize);
            if *up.get_or_insert(got) != got {
                return None;
            }
        }
    }
    Some((up?, down?))
}

