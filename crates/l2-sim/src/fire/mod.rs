//! **Fire on the battlefield**: a burning cell, a stream of boiling oil, a
//! bridge going up, a wood going up, and what standing in any of them does to a
//! man.
//!
//! Five routines of `Lords2.exe`, and one record shape carrying all of them:
//!
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
//! # A fire is a missile that does not move
//!
//! The original has no fire table. `FUN_00485675` allocates a slot of the
//! **same hundred** `g_missiles` records the arrows use, gives it class 5, no
//! sub-steps and a range of 20,000 ticks, **remembers the surface it burnt
//! over** in `+0x3E`, and writes surface 10 onto the cell. `Missile_UpdateAll`
//! counts its `+0x3C` down like any spent missile's, and on the frame the count
//! reads **2** it writes the remembered surface back — so the fire goes out by
//! itself and the ground is what it was, except where it was a bridge or a
//! wood, which burn away (see [`put_out`]).
//!
//! Two consequences a player meets. **A battle full of fire has fewer arrows**:
//! the slots are shared, and a hundred and first record is dropped. And **the
//! fire does not spread by itself**: a burning cell never sets its neighbour
//! alight. Three things spread fire, and each is a different routine with a
//! different rule — the oil stream paints a cross under itself as it flies, a
//! bridge fire walks along the bridge for five cells, and a wood fire floods the
//! whole wood one ring a frame.
//!
//! # Where a man burns, and where he does not
//!
//! `Battle_UpdateAllMen` calls [`burn`] for a figure standing on surface **10**
//! (oil, a bridge) or **0x11** (a wood that has caught). A cell of surface
//! **0x10** — a wood in its first frame alight — does **not** burn anybody:
//! the spread turns every 0x10 into 0x11 at the top of the next frame, and only
//! then is it fire.

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

/// Surface **7** — a bridge. `Battlefield_BuildCastle`'s structure code 7.
pub const SURFACE_BRIDGE: u8 = 7;
/// Surface **10** — a cell burning: [`ignite`] writes it.
pub const SURFACE_BURNING: u8 = 10;
/// Surface **0x0F** — woodland.
pub const SURFACE_WOODLAND: u8 = 0x0F;
/// Surface **0x10** — a wood cell in the frame it caught. Harmless for one
/// frame; [`spread_woodland`] turns it into [`SURFACE_WOOD_BURNING`].
pub const SURFACE_WOOD_CATCHING: u8 = 0x10;
/// Surface **0x11** — a wood burning. `BattleMan_BurnTick`'s second surface.
pub const SURFACE_WOOD_BURNING: u8 = 0x11;
/// Surface **5** — what a burnt-out bridge is left as. The bailey's value;
/// `Missile_UpdateAll` writes it, and nothing says why.
pub const SURFACE_BURNT_BRIDGE: u8 = 5;

/// `0x280` — a fire's life is this less the caller's argument.
/// `FUN_00485675` writes `+0x3C = 0x280 - param_3`.
pub const FIRE_LIFE: i16 = 0x280;
/// The count at which a fire writes its surface back. `Missile_UpdateAll`'s
/// class-5 arm tests `+0x3C == 2`
/// that is no longer burning.
pub const FIRE_RESTORE_AT: i16 = 2;
/// A fire's range, which nothing reaches: its countdown ends it first.
pub const FIRE_RANGE_TICKS: i16 = 20_000;
/// `+0x2F` on a fire record, 6, which only a flying record would read.
pub const FIRE_LAUNCH_ELEVATION: u8 = 6;

/// What `FUN_0048551D` passes for the first cell of a bridge fire: `0x78`, so
/// it burns for `0x280 - 0x78` = **520** frames.
pub const BRIDGE_FIRE_PARAM: i16 = 0x78;
/// How far a bridge fire walks from where it started: rings 1 to 5.
pub const BRIDGE_SPREAD_RADIUS: i32 = 5;
/// Each further bridge cell burns **seven frames longer** than the last, to a
/// ceiling of `0x78` more: the argument falls `0x78 - 0`, `- 7`, `- 14` …
pub const BRIDGE_SPREAD_STEP: i16 = 7;
/// What `Missile_Step` sets a missile's countdown to when it enters a bridge
/// cell: **8**, so the arrow, bolt, shot or oil that set it alight retires eight
/// frames later and hits nothing more.
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

/// **The cross a stream of oil burns under itself, every frame it flies** —
/// `Missile_UpdateAll`'s class-7 arm, in the original's order.
///
/// `(dx, dy, bias)`: each cell is set alight by [`ignite`] with argument
/// `(ticksFlown − 1) × 32 + bias`, so the far end of a pour burns shorter than
/// its head, and within one frame's cross the centre and the northern cell burn
/// 40 frames shorter than the southern one. A cell already of surface 7 or 10 is
/// skipped — a burning cell is never re-lit and a bridge is never lit by the
/// oil (the bridge is [`bridge_fire`]'s, from the stream's own sub-steps).
pub const OIL_CROSS: [(i32, i32, i16); 5] =
    [(0, 0, 0x28), (0, -1, 0x28), (0, 1, 0), (1, 0, 0x19), (-1, 0, 10)];

/// Figures of a human's more than this many on woodland, and an AI garrison's
/// arrows are fire arrows. `BattleMan_FireMissile`: `3 < DAT_005530E8`.
pub const FIRE_ARROWS_ABOVE: u32 = 3;

/// A fire record over one cell — what `Missile_Spawn(1, x, y, x, y)` and
/// `FUN_00485675`'s writes leave in the slot.
///
/// Owner **1**, whoever lit it, which is what makes a fire nobody's. The
/// position is nudged `−16, −24` sub-cell units for drawing; the cell is not
/// moved. Everything the record would need to fly is zero.
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
/// `BattleUnit_Order` (`0x00479E90`) writes `y * 0x280 + x * 8`.
pub const fn cell_byte_offset(x: i32, y: i32) -> i32 {
    (y * DIM as i32 + x) * 8
}

fn in_field(x: i32, y: i32) -> bool {
    (0..DIM as i32).contains(&x) && (0..DIM as i32).contains(&y)
}

