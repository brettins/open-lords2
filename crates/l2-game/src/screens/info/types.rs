#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::screen::*;
use super::painters::*;
use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

/// What the right click resolved to. **Part of the screen's identity**, because
/// the original keeps it in `g_pickedTileUnit`/`DAT_0056795C` and picks the
/// painter from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// `g_pickedTileUnit != 0` — the unit half.
    Unit(usize),
    /// A tile index — the tile half.
    Tile(usize),
}

/// `L2.eng` group 31 — the unit panel's own strings, 32 of them.
pub const UNIT_GROUP: usize = 31;
/// `L2.eng` group 30 — the tile panel's, 88 of them.
pub const TILE_GROUP: usize = 30;
/// Group 100, county names, twenty per scenario.
pub const COUNTY_GROUP: usize = 100;

/// **The ten strings of group 31 that nothing in the binary draws**, with 21
/// among them.
///
/// Enumerated: group 31 has two consumers and their
/// reachable index sets were listed. 6 is the interesting one — it is *assigned*
/// to the heading local for an army and then explicitly suppressed by
/// `else if (local_20 != 6)`.
pub const DEAD_LABELS: [usize; 10] = [1, 3, 4, 6, 7, 10, 11, 19, 21, 26];

/// The panel's top row in 16-pixel cells, with the head-room the tile half
/// grants a county tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `DAT_00553D2C`.
    pub row: i32,
    /// `DAT_005651C8` — 2 when the tile belongs to a county, else 0.
    pub headroom: i32,
}

impl Layout {
    /// `Ui_DrawBox(8, (row − headroom) * 16 + 32, 0x1C, (0x1B − row) + headroom)`
    /// — **and `top + height` is 464 for every one of them.**
    pub fn box_at(self) -> Rect {
        let top = (self.row - self.headroom) * 16 + 32;
        let rows = (0x1B - self.row) + self.headroom;
        Rect::new(8, top, 0x1C * 16, rows * 16)
    }

    /// Where a y written as `row * 16 + k` in the decompilation lands.
    pub fn y(self, k: i32) -> i32 {
        self.row * 16 + k
    }

    /// Every value the two painters take, with what produces it. The unit
    /// half's three never grant head-room; the tile half's grant it except for
    /// sea and a village.
    pub const ALL: [(&'static str, Layout); 11] = [
        ("army, yours", Layout { row: 2, headroom: 0 }),
        ("farmland, your county, a real field", Layout { row: 5, headroom: 2 }),
        ("castle degraded, your county", Layout { row: 0x0A, headroom: 2 }),
        ("farmland, your county, waste or reclaiming", Layout { row: 0x0C, headroom: 2 }),
        ("castle intact, no build in progress", Layout { row: 0x0E, headroom: 2 }),
        ("county town with a mercenary offer", Layout { row: 0x0F, headroom: 2 }),
        ("peasants, merchant or transport", Layout { row: 0x0F, headroom: 0 }),
        ("a village", Layout { row: 0x10, headroom: 0 }),
        ("the fallback: scrub, road, mountain, an enemy's anything", Layout { row: 0x11, headroom: 2 }),
        ("sea", Layout { row: 0x11, headroom: 0 }),
        ("army, not yours", Layout { row: 0x12, headroom: 0 }),
    ];
}

/// `TileInfo_Draw`'s ladder, as far as it selects a *layout*. The eighty-eight
/// group 30 strings are the player's data and not a table of ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Road,
    Sea,
    Village,
    /// The `0x10` arm's other half — a dwelling plot whose terrain byte is not
    /// [`VILLAGE_GRAPHIC`]. `Map_ResolvePick` blanks the flags when it is
    /// *zero*, so this is a plot that once held something.
    RuinedVillage,
    Mountain,
    Woodland,
    Farmland,
    CountyTown,
    Industry,
    Castle,
    Scrubland,
}

