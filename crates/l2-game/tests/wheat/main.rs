//! A player, on the build that carried C124's fix: *"wheat fields still not
//! showing the different stages of wheat growth."* **"Still"** — the fix had a
//! test at the pixel, and the test painted the terrain byte by hand. Nothing
//! ever asked what a season writes there, so the fix could band the wrong crop
//! word for three seasons in four and stay green. `docs/decisions.md`
//! C195.
//!
//! * **The frame** is a literal: `Terrain_Set` (`0x0046D7F4`) writes
//!   `((frame - oldBase) & 3) + 'X' + variant * 4` for terrain `2 … 0x12`, and
//!   `'X'` is `0x58`. [`WHEAT_BASE`].
//!
//! * **The variant** is [`originals_variant`], a transcription of
//!   `Grain_SeasonTick` (`0x0044C8AE`) and `FUN_0044CF6F` with their literal
//!   thresholds, reading the county's crop words and `+0x206` — which the four
//!   End Turns wrote, not this file.
//!
//! # Stated ablations, and what each did
//!
//! | ablated | this test | unit test |
//! |---|---|---|
//! | the band reads `crop[2]` in Spring, Summer, Autumn — C124's word | **red** | red |
//! | the repaint call in `Kingdom::grain_season_tick` | **red** | — |
//! | `+ field_variant(terrain) * 4` in `campaign::field_frame` | **red** | — |
//! | the fog arm darkens every tile, so the field is fogged | **red** (the four crops stop being four pictures) | — |
//! | sowing does not write `+0x206` | **red** — after the fix below | red |
//! | the band divides by `fieldsGrain`, not `+0x206` | *green* | red |
//! | Winter reads `crop[1]`, not the harvest | *green* | red |
//! | `FUN_00469D21`'s shortfall arm | *green* | red |
//!
//! **The three greens are findings about this year, not about the lines.** No
//! field is destroyed and none painted after sowing, so `fieldsGrain` equals
//! `+0x206` all year; nobody is short of reapers, so the harvest equals the
//! standing crop; and the seed covers a sack a field, so the shortfall never
//! fires. Each line is pinned by a unit test in `l2_kingdom::land` instead.
//!
//! **And one was green for a reason that was this file's fault.** The first
//! version read `+0x206` back from the county to compute its expectation, so
//! deleting the line that writes it moved the expectation with the picture —
//! `docs/agents.md`'s probe computed from the thing being ablated. It is pinned
//! from what this file sowed now, and that ablation is red.


use std::path::PathBuf;

use l2_formats::maps::Plane;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::map::MapScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::map::{coords, index, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::{campaign, Canvas};

const WHEAT_BASE: u8 = 0x58;

const ROADS_BANK_BYTE: u8 = 0x09;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no wheat to draw");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

mod helpers;
pub use helpers::*;
mod wheat_test;
pub use wheat_test::*;

