//! | routine | what it is | what writes it here |
//! |---|---|---|
//! | `FUN_00485675` | set one cell burning — surface **10** | [`ignite`] |
//! | `FUN_00485861` | set one woodland cell catching — surface **0x10** | [`ignite_woodland`] |
//! | `FUN_004859E5` | the wood fire's spread, once a frame | [`spread_woodland`] |
//! | `FUN_0048551D` | a bridge goes up | [`bridge_fire`] |
//! | `FUN_0047A814` | a pot of oil is poured — the stream's record | [`oil_record`] |
//! | `BattleMan_BurnTick` (`0x0049459A`) | a man stands in fire | [`burn`] |
//!
//! The runner ([`crate::runner`]) owns *when* each happens; this module owns
//! what each does to a cell, a record and a figure. Every constant below is a
//! literal out of those bodies. `[V]` throughout unless a line says otherwise.
//!
//! The original has no fire table. `FUN_00485675` allocates a slot of the
//! **same hundred** `g_missiles` records the arrows use, gives it class 5, no
//! sub-steps and a range of 20,000 ticks, **remembers the surface it burnt
//! over** in `+0x3E`, and writes surface 10 onto the cell. `Missile_UpdateAll`
//! counts its `+0x3C` down like any spent missile's, and on the frame the count
//! reads **2** it writes the remembered surface back — so the fire goes out by
//! itself and the ground is what it was, except where it was a bridge or a
//! wood, which burn away (see [`put_out`]).

mod ignite_part;
pub use ignite_part::*;
mod woodland;
pub use woodland::*;
mod burn_part;
pub use burn_part::*;
mod tests_part;
pub use tests_part::*;

use crate::figure::{Figure, State};
use crate::missile::{Missile, Missiles, CLASS_FIRE, CLASS_OIL, SUB_CELL};
use crate::terrain::{Battlefield, DIM};

pub const SURFACE_BRIDGE: u8 = 7;
pub const SURFACE_BURNING: u8 = 10;
pub const SURFACE_WOODLAND: u8 = 0x0F;
pub const SURFACE_WOOD_CATCHING: u8 = 0x10;
pub const SURFACE_WOOD_BURNING: u8 = 0x11;
pub const SURFACE_BURNT_BRIDGE: u8 = 5;

/// `FUN_00485675` writes `+0x3C = 0x280 - param_3`.
pub const FIRE_LIFE: i16 = 0x280;
/// The count at which a fire writes its surface back. `Missile_UpdateAll`'s
/// class-5 arm tests `+0x3C == 2`
/// that is no longer burning.
pub const FIRE_RESTORE_AT: i16 = 2;
pub const FIRE_RANGE_TICKS: i16 = 20_000;
/// `+0x2F` on a fire record, 6, which only a flying record would read.
pub const FIRE_LAUNCH_ELEVATION: u8 = 6;

/// What `FUN_0048551D` passes for the first cell of a bridge fire: `0x78`, so
/// it burns for `0x280 - 0x78` = **520** frames.
pub const BRIDGE_FIRE_PARAM: i16 = 0x78;
pub const BRIDGE_SPREAD_RADIUS: i32 = 5;
pub const BRIDGE_SPREAD_STEP: i16 = 7;
pub const MISSILE_ON_BRIDGE_TTL: i16 = 8;

/// A stream of oil moves **16** sub-steps a frame — half a cell — where an
/// arrow moves four. `FUN_0047A814` writes `+0x31 = +0x32 = 0x10`.
pub const OIL_SUB_STEPS: i8 = 16;
/// …for **16** frames. `+0x38 = 0x10`, which at half a cell a frame is eight
/// cells from the pot.
pub const OIL_RANGE_TICKS: i16 = 16;
/// `+0x2F = 4`, so high ground never stops a pour.
pub const OIL_LAUNCH_ELEVATION: u8 = 4;
/// The steps `FUN_0047A814` runs on the spot, before the stream is linked to a
/// cell: **four**, where an arrow gets eight. Two cells.
pub const OIL_LAUNCH_STEPS: u32 = 4;

pub const OIL_CROSS: [(i32, i32, i16); 5] =
    [(0, 0, 0x28), (0, -1, 0x28), (0, 1, 0), (1, 0, 0x19), (-1, 0, 10)];

/// Figures of a human's more than this many on woodland, and an AI garrison's
/// arrows are fire arrows. `BattleMan_FireMissile`: `3 < DAT_005530E8`.
pub const FIRE_ARROWS_ABOVE: u32 = 3;

/// A fire record over one cell — what `Missile_Spawn(1, x, y, x, y)` and
/// `FUN_00485675`'s writes leave in the slot.
pub fn fire_record(x: i32, y: i32, life: i16, saved_surface: u8) -> Missile {
    Missile {
        owner: 1,
        class: CLASS_FIRE,
        shooter: 0,
        x: (x as i16) * SUB_CELL - 0x10,
        y: (y as i16) * SUB_CELL - 0x18,
        target_x: (x as i16) * SUB_CELL,
        target_y: (y as i16) * SUB_CELL,
        cell_x: x as i16,
        cell_y: y as i16,
        dx: 0,
        dy: 0,
        err: 0,
        major_axis: 1,
        dir: 8,
        launch_elevation: FIRE_LAUNCH_ELEVATION,
        blocked_ticks: 0,
        sub_steps: 0,
        ticks_flown: 0,
        range_ticks: FIRE_RANGE_TICKS,
        blocked: false,
        ttl: life,
        power: 0,
        saved_surface,
        fire_arrow: 0,
    }
}

/// A battlefield cell as the original's **byte offset**, `(y * 80 + x) * 8` —
/// the form unit `+0x30` (`targetCell`) and missile `+0x44` both carry.
///
/// `BattleUnit_Order` (`0x00479E90`) writes `y * 0x280 + x * 8`.
pub const fn cell_byte_offset(x: i32, y: i32) -> i32 {
    (y * DIM as i32 + x) * 8
}

fn in_field(x: i32, y: i32) -> bool {
    (0..DIM as i32).contains(&x) && (0..DIM as i32).contains(&y)
}

