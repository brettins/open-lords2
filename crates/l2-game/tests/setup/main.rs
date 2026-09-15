

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::setup::{
    SetupPage, SetupScreen, MAP_LIST_ROW, MAP_LIST_ROWS, MAP_LIST_X, MAP_LIST_Y, OPTION_CELLS,
    OPTION_LIST,
};
use l2_game::setup::{self, option, SetupOptions, COUNTY_STATUS, STARTING_GOLD, START_ARMOURY};
use l2_game::Game;
use l2_game::shell::{font, Pen};
use l2_view::Canvas;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_mods::Platform;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so no L2_maps.dat and no L2.eng");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

mod setup_tests;
pub use setup_tests::*;
mod skirmish_tests;
pub use skirmish_tests::*;
mod title_tests;
pub use title_tests::*;
mod held_tests;
pub use held_tests::*;

pub(crate) fn click(screen: &mut SetupScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

fn tick(screen: &mut SetupScreen, game: &mut Game, assets: &Assets) {
    let mut ctx = Ctx { game, assets };
    screen.update(&mut ctx);
}

fn choose(
    screen: &mut SetupScreen,
    game: &mut Game,
    assets: &Assets,
    i: usize,
    row: usize,
) {
    let (bx, by, _) = OPTION_CELLS[i];
    click(screen, game, assets, bx + 8, by + 8);
    assert_eq!(screen.page(), SetupPage::Dropdown, "option {i} opened its list");
    let (lx, mut ly, _) = OPTION_LIST[i];
    if i == option::NOBLES {
        let rows = SetupOptions::nobles_rows_for_map(screen.player_starts());
        ly -= (rows as i32 - 1) * 16;
    }
    click(screen, game, assets, lx + 8, ly + 16 + row as i32 * 16 + 8);
    assert_eq!(screen.page(), SetupPage::Custom, "and closed again");
    assert_eq!(screen.options().get(i), row, "option {i} took row {row}");
}

fn press_start(screen: &mut SetupScreen, game: &mut Game, assets: &Assets) -> Transition {
    click(screen, game, assets, 0xF3 + 20, 0xC6)
}

