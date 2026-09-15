//! **The castle the player built, read out of `stnfield.pl8`** —
//! `Battlefield_BuildCastle` (`0x0047C4BA`) and `Battlefield_ReadStructureLayer`.


use l2_sim::castle::{self, CastleSheets};
use l2_sim::siege::{
    self, DOCK_WALL_ELEVATION, FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY,
    SURFACE_RAMPART_WALK, SURFACE_WATER,
};
use l2_sim::terrain::DIM;
use l2_sim::{BattleRunner, Muster, Troop};

fn layouts(dir: Option<std::path::PathBuf>) -> Option<CastleSheets> {
    let path = l2_testkit::find(&dir?, CastleSheets::FILE)?;
    CastleSheets::parse(&std::fs::read(path).ok()?)
}

macro_rules! sheets {
    () => {
        match layouts(l2_testkit::install_dir()) {
            Some(s) => s,
            None => l2_testkit::skip!(
                "no {} reachable ({} unset or the install is partial)",
                CastleSheets::FILE,
                l2_testkit::INSTALL_VAR
            ),
        }
    };
}

mod layout_tests;
pub use layout_tests::*;
mod siege_tests;
pub use siege_tests::*;


