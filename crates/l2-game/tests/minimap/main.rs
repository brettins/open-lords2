//! `Minimap_DrawOverlay` (`0x00410CBD`) and `FUN_00451BBA` are the two functions
//! under test; `docs/screens.md` §3.2 describes them.


use std::path::PathBuf;


fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no minimap raster to draw");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        let raster = assets.minimap(game.map_slot).expect("the slot has a MAPnn.PL8");
        (game, assets, raster)
    }};
}

mod helpers;
use helpers::*;
mod overlays;
mod ui;

