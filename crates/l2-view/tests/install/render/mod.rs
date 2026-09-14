#![allow(unused_imports)]

mod terrain_part;
pub use terrain_part::*;
mod battle_part;
pub use battle_part::*;
mod ui;
pub use ui::*;
mod sprites;
pub use sprites::*;

use super::*;
use super::corpus::*;
use super::oracle::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

fn load_assets(dir: &Path) -> BattleAssets {
    BattleAssets::load(
        |name| read(dir, name).ok_or_else(|| format!("{name} not found")),
        Colour::Red,
        Colour::Blue,
    )
    .expect("battle assets load")
}

