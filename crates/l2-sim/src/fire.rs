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
/// class-5 arm tests `+0x3C == 2`, so a fire's last two frames are over ground
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
        fire_arrow: false,
    }
}

fn in_field(x: i32, y: i32) -> bool {
    (0..DIM as i32).contains(&x) && (0..DIM as i32).contains(&y)
}

/// **Set one cell burning** — `FUN_00485675` (`0x00485675`).
///
/// ```c
/// Missile_Spawn(1, x, y, x, y);  class = 5;  x -= 0x10;  y -= 0x18;
/// +0x3C = 0x280 - param_3;  +0x38 = 20000;  +0x2F = 6;
/// +0x3E = cell.surface;  cell.surface = 10;
/// if (+0x3E == 7) { cell.flags = 0; cell.elevation = 0; flags2 = …; cell.frame = 0; }
/// ```
///
/// A bridge that catches is **flattened and cleared on the spot**: elevation 0,
/// every flag gone. So a burning bridge is passable ground at the height of the
/// water, for as long as it burns and after.
///
/// **The slot is not checked**, and that is the original's defect rather than
/// ours: `Missile_Spawn` returns 0 with a hundred records in flight, and the
/// writes land on a hundred-and-first record past the array. The cell's own
/// writes still happen — so **a cell set alight with the array full burns for
/// the rest of the battle**, with no record to put it out. Reproduced for the
/// cell; the overrun itself is not behaviour (`docs/bugs.md` N4's reasoning)
/// and is not reproduced. `docs/bugs.md` `B102`.
///
/// Returns the cell, for the caller to re-derive what it keeps from the field.
pub fn ignite(field: &mut Battlefield, missiles: &mut Missiles, x: i32, y: i32, param: i16) -> usize {
    let cell = y as usize * DIM + x as usize;
    let saved = field.cells[cell].surface;
    if let Some(slot) = missiles.alloc() {
        *missiles.get_mut(slot) = fire_record(x, y, FIRE_LIFE.wrapping_sub(param), saved);
    }
    let c = &mut field.cells[cell];
    c.surface = SURFACE_BURNING;
    if saved == SURFACE_BRIDGE {
        c.flags = 0;
        c.elevation = 0;
        c.gfx = 0;
    }
    cell
}

/// **Set one woodland cell catching** — `FUN_00485861` (`0x00485861`).
///
/// Unlike [`ignite`] this one **does** check the slot, and does nothing at all
/// without one. The life is `0x280 − 10 × ((x + y) & 0x1F)`, so a wood burns
/// out in a diagonal stripe pattern over 330 to 640 frames rather than all at
/// once, and the remembered surface is always woodland.
///
/// Returns the cell when it caught.
pub fn ignite_woodland(field: &mut Battlefield, missiles: &mut Missiles, x: i32, y: i32) -> Option<usize> {
    let slot = missiles.alloc()?;
    let cell = y as usize * DIM + x as usize;
    let life = FIRE_LIFE - 10 * (((x + y) & 0x1F) as i16);
    *missiles.get_mut(slot) = fire_record(x, y, life, SURFACE_WOODLAND);
    field.cells[cell].surface = SURFACE_WOOD_CATCHING;
    Some(cell)
}

/// **A fire goes out** — `Missile_UpdateAll`'s class-5 arm, on the frame its
/// countdown reads [`FIRE_RESTORE_AT`].
///
/// ```c
/// if (+0x3E == 7)    { surface = 5; flags = 0; elevation = 0; frame = 0; }
/// else if (+0x3E == 0x0F) { surface = 0; flags = 0; frame = 0; }
/// else               { surface = +0x3E; }
/// ```
///
/// So what burns away is exactly **a bridge** — left as surface 5 at the
/// height of the water — and **a wood**, left as surface 0, which no zone of
/// the battlefield classifier ever writes. Everything else comes back.
pub fn put_out(field: &mut Battlefield, fire: &Missile) -> usize {
    let cell = fire.cell_y as usize * DIM + fire.cell_x as usize;
    let c = &mut field.cells[cell];
    match fire.saved_surface {
        SURFACE_BRIDGE => {
            c.surface = SURFACE_BURNT_BRIDGE;
            c.flags = 0;
            c.elevation = 0;
            c.gfx = 0;
        }
        SURFACE_WOODLAND => {
            c.surface = 0;
            c.flags = 0;
            c.gfx = 0;
        }
        s => c.surface = s,
    }
    cell
}

/// `Cell_NeighbourHasSurface` (`0x00496F72`): north, east, south, west, each
/// clipped at the field's edge.
pub fn neighbour_has_surface(field: &Battlefield, x: i32, y: i32, surface: u8) -> bool {
    let at = |x: i32, y: i32| field.cells[y as usize * DIM + x as usize].surface;
    (y >= 1 && at(x, y - 1) == surface)
        || (x < DIM as i32 - 1 && at(x + 1, y) == surface)
        || (y < DIM as i32 - 1 && at(x, y + 1) == surface)
        || (x >= 1 && at(x - 1, y) == surface)
}

/// **A bridge goes up** — `FUN_0048551D` (`0x0048551D`), less the
/// `Sound_PlayFile("dest_ind.wav", 0, 0)` it opens with, which is the caller's
/// cue.
///
/// ```c
/// FUN_00485675(x, y, 0x78);
/// c = 0;
/// for (r = 1; r < 6; r++)
///   for each cell of the (2r+1) square round (x, y), rows top to bottom:
///     if (Cell_NeighbourHasSurface(cell, 10)) {
///       if (cell.surface == 7) { FUN_00485675(cell, 0x78 - c); c += 7; if (c > 0x78) c = 0x78; }
///       else if (cell.surface == 5) FUN_00485B47(cell);     /* scorch the ground */
///     }
/// ```
///
/// **The fire walks along the bridge, and only along the bridge**, one ring at
/// a time for five rings: a bridge cell beside something burning catches, and
/// so does the one beside *it* on the next ring — so a straight bridge burns
/// five cells either way of where it caught and no further. **Anything burning
/// counts as a neighbour**, oil included. A cell of surface 5 beside the fire
/// is only scorched: `FUN_00485B47` masks its frame to the low nibble and
/// touches no rule.
///
/// The original walks the square with no bound at the field's edge, so a fire
/// within five of it reads cells of the neighbouring row. That is not
/// behaviour and is not reproduced. Returns every cell it wrote.
pub fn bridge_fire(field: &mut Battlefield, missiles: &mut Missiles, x: i32, y: i32) -> Vec<usize> {
    let mut touched = vec![ignite(field, missiles, x, y, BRIDGE_FIRE_PARAM)];
    let mut c: i16 = 0;
    for r in 1..=BRIDGE_SPREAD_RADIUS {
        for cy in (y - r)..=(y + r) {
            for cx in (x - r)..=(x + r) {
                if !in_field(cx, cy) || !neighbour_has_surface(field, cx, cy, SURFACE_BURNING) {
                    continue;
                }
                let cell = cy as usize * DIM + cx as usize;
                match field.cells[cell].surface {
                    SURFACE_BRIDGE => {
                        touched.push(ignite(field, missiles, cx, cy, BRIDGE_FIRE_PARAM - c));
                        c = (c + BRIDGE_SPREAD_STEP).min(BRIDGE_FIRE_PARAM);
                    }
                    // Surface 5 — the bailey, and a bridge that has already
                    // burnt out, which is written as the same value.
                    crate::siege::SURFACE_BAILEY => {
                        field.cells[cell].gfx &= 0x0F;
                        touched.push(cell);
                    }
                    _ => {}
                }
            }
        }
    }
    touched
}

/// **The wood fire, one frame of it** — `FUN_004859E5` (`0x004859E5`), which
/// `Battle_Frame` runs after the unit sweep.
///
/// ```c
/// if (0 < DAT_0053E9D0) {
///     DAT_0053E9D0 = 0;
///     for every cell: if (surface == 0x10) { surface = 0x11; DAT_0053E9D0 = 1; }
///     for every cell: if (surface == 0x0F && Cell_NeighbourHasSurface(cell, 0x11)) FUN_00485861(cell);
/// }
/// ```
///
/// **One ring a frame, through the whole of a connected wood**, and it stops
/// only when a frame turns nothing from catching to burning. The second pass
/// lights cells beside `0x11` and writes `0x10`, so a cell lit in this pass
/// cannot light its own neighbour until the next frame — the ring is exact.
/// `spreading` is `DAT_0053E9D0`. Returns every cell it wrote.
pub fn spread_woodland(field: &mut Battlefield, missiles: &mut Missiles, spreading: &mut bool) -> Vec<usize> {
    let mut touched = Vec::new();
    if !*spreading {
        return touched;
    }
    *spreading = false;
    for (cell, c) in field.cells.iter_mut().enumerate() {
        if c.surface == SURFACE_WOOD_CATCHING {
            c.surface = SURFACE_WOOD_BURNING;
            *spreading = true;
            touched.push(cell);
        }
    }
    for y in 0..DIM as i32 {
        for x in 0..DIM as i32 {
            let cell = y as usize * DIM + x as usize;
            if field.cells[cell].surface == SURFACE_WOODLAND
                && neighbour_has_surface(field, x, y, SURFACE_WOOD_BURNING)
            {
                if let Some(c) = ignite_woodland(field, missiles, x, y) {
                    touched.push(c);
                }
            }
        }
    }
    touched
}

/// **A stream of boiling oil, as `FUN_0047A814` (`0x0047A814`) leaves the
/// record** before its four launch steps.
///
/// ```c
/// Missile_Spawn(pot.owner, pot.mapX, pot.mapY, x, y);
/// +0x06 = pot;  class = 7;  sprite = 0;  +0x32 = +0x31 = 0x10;
/// power = 0;  range = 0x10;  +0x2F = 4;  blockedTicks = 0;  blocked = 0;
/// ```
///
/// **Zero power, and no class test in `Missile_Step` that could hurt a man with
/// it**: oil kills by the fire it leaves, not by landing. Its `+0x3C` is left at
/// the zeroed slot's 0, which is what lets it set a bridge alight as it crosses.
pub fn oil_record(owner: u8, pot: usize, from: (u8, u8), to: (u8, u8)) -> Missile {
    let mut m = Missile {
        owner,
        class: CLASS_OIL,
        shooter: pot as u16,
        x: from.0 as i16 * SUB_CELL,
        y: from.1 as i16 * SUB_CELL,
        target_x: to.0 as i16 * SUB_CELL,
        target_y: to.1 as i16 * SUB_CELL,
        cell_x: from.0 as i16,
        cell_y: from.1 as i16,
        dir: crate::facing::facing_from_delta(
            to.0 as i32 - from.0 as i32,
            to.1 as i32 - from.1 as i32,
        )
        .unwrap_or(8),
        launch_elevation: OIL_LAUNCH_ELEVATION,
        blocked_ticks: 0,
        sub_steps: OIL_SUB_STEPS,
        range_ticks: OIL_RANGE_TICKS,
        ..Missile::default()
    };
    m.setup_line();
    m
}

/// The facing `FUN_0047A814` turns a pot to as it pours: the axis the pour
/// travels further along, with **x winning a tie** —
/// `if (absDx < absDy) { y < ty ? 4 : 0 } else { x < tx ? 2 : 6 }`.
pub fn pour_facing(from: (u8, u8), to: (u8, u8)) -> u8 {
    let (dx, dy) = (to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32);
    if dx.abs() < dy.abs() {
        if from.1 < to.1 {
            4
        } else {
            0
        }
    } else if from.0 < to.0 {
        2
    } else {
        6
    }
}

/// Where a pot of oil **pours when its unit is ordered**, from `BattleUnit_Order`
/// (`0x00479E90`)'s oil loop: the pot's own surface against the destination's.
///
/// ```c
/// if      (pot == 6 && dest < 6) FUN_0047a814(pot, x, y);   /* from the keep     */
/// else if (pot == 4 && dest < 4) FUN_0047a814(pot, x, y);   /* from the rampart  */
/// else if (pot == 5 && dest < 4) FUN_0047a814(pot, x, y);   /* from the bailey   */
/// ```
///
/// So an order *down* pours and an order *along* moves: a pot on the rampart
/// walk told to go to another stretch of walk walks there, and told to go to
/// the field below tips its oil where it stands. **An order is a pour, not a
/// walk to a place to pour from.**
pub fn order_pours(pot_surface: u8, dest_surface: u8) -> bool {
    matches!((pot_surface, dest_surface), (6, d) if d < 6)
        || matches!((pot_surface, dest_surface), (4, d) if d < 4)
        || matches!((pot_surface, dest_surface), (5, d) if d < 4)
}

/// **A man stands in fire** — `BattleMan_BurnTick` (`0x0049459A`), one frame.
///
/// | size class | a man's figure | a siege engine (7, 8, 9) |
/// |---:|---:|---:|
/// | 0, 1 | 3 | 1 |
/// | 2 | 6 | 3 |
/// | 3 | 9 | 5 |
/// | 4 and up | 12 | 7 |
/// | a human's, add | 1 | 2 |
///
/// …against a threshold of **100** hits a man, or **160** for an engine, and
/// **one** man a frame at most: `hits -= threshold; men -= 1`. The remainder is
/// carried. `engine` is the figure's `+0x194`, which `BattleUnit_Create` sets
/// for troop types 7, 8 and 9 — **not 10**: a pot of oil burns as a man does,
/// at a man's threshold, whatever its own table says.
///
/// Both thresholds are literals in the body, so a ruleset's
/// `hits_per_casualty` does not reach this — as it does not reach the
/// original's. Returns `true` when the figure's last man died of it, which is
/// when the original plays the dying figure's side's cry.
pub fn burn(f: &mut Figure, size_class: u8, engine: bool) -> bool {
    if f.state == State::Dead {
        return false;
    }
    const MAN: [u16; 4] = [3, 6, 9, 12];
    const ENGINE: [u16; 4] = [1, 3, 5, 7];
    let rung = match size_class {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        _ => 3,
    };
    let (threshold, add, human) =
        if engine { (160u16, ENGINE[rung], 2u16) } else { (100u16, MAN[rung], 1u16) };
    f.hits = f.hits.saturating_add(add);
    if f.owner_is_human {
        f.hits = f.hits.saturating_add(human);
    }
    if f.hits >= threshold {
        f.hits -= threshold;
        f.men = f.men.saturating_sub(1);
    }
    if f.men < 1 {
        f.state = State::Dead;
        f.opponent = None;
        return true;
    }
    false
}

/// Whether a troop's figure carries `+0x194`, `isSiegeEngine` —
/// `BattleUnit_Create`: `6 < troopType && troopType < 10`. Oil is not one.
pub fn is_engine(troop: crate::Troop) -> bool {
    (7..=9).contains(&troop.index())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};
    use crate::runner::blank_field;
    use crate::Troop;

    /// **The burn table**, pinned from the literals in `BattleMan_BurnTick`
    /// rather than computed: one frame per size class, a man and an engine, an
    /// AI's and a human's.
    ///
    /// Ablation: swap the `6` and `9` in `MAN` and the class-2 row reads 9.
    #[test]
    fn a_frame_in_fire_costs_three_six_nine_or_twelve_hits_and_an_engine_less() {
        for (class, man, engine) in [(0u8, 3u16, 1u16), (1, 3, 1), (2, 6, 3), (3, 9, 5), (4, 12, 7), (8, 12, 7)] {
            let mut f = Figure::new(Troop::Peasants, SIDE_A, 4);
            burn(&mut f, class, false);
            assert_eq!(f.hits, man, "a man at class {class}");
            let mut h = Figure::new(Troop::Peasants, SIDE_A, 4);
            h.owner_is_human = true;
            burn(&mut h, class, false);
            assert_eq!(h.hits, man + 1, "a human's man at class {class}");
            let mut e = Figure::new(Troop::SiegeTowers, SIDE_B, 4);
            burn(&mut e, class, true);
            assert_eq!(e.hits, engine, "an engine at class {class}");
            e.owner_is_human = true;
            burn(&mut e, class, true);
            assert_eq!(e.hits, 2 * engine + 2, "a human's engine at class {class}");
        }
    }

    /// **One man a frame, the remainder carried, and the last man's death
    /// reported once.** At three hits a frame the hundredth hit lands on the
    /// thirty-fourth frame, with two carried.
    #[test]
    fn a_man_in_fire_dies_every_thirty_four_frames_and_the_figure_once() {
        let mut f = Figure::new(Troop::Archers, SIDE_B, 2);
        let mut died = 0;
        for frame in 1..=68 {
            if burn(&mut f, 0, false) {
                died += 1;
            }
            if frame == 33 {
                assert_eq!((f.men, f.hits), (2, 99));
            }
            if frame == 34 {
                assert_eq!((f.men, f.hits), (1, 2), "the remainder is carried");
            }
        }
        assert_eq!(f.men, 0);
        assert_eq!(f.state, State::Dead);
        assert_eq!(died, 1, "the death is reported once");
        assert!(!burn(&mut f, 0, false), "a corpse does not burn");
    }

    /// **Oil burns at a man's threshold**, because `+0x194` is set for 7, 8
    /// and 9 and not for 10 — although our troop table gives oil 160.
    #[test]
    fn a_pot_of_oil_burns_as_a_man_does_not_as_an_engine() {
        assert!(!is_engine(Troop::Oil));
        assert!(is_engine(Troop::Catapults) && is_engine(Troop::SiegeTowers) && is_engine(Troop::BatteringRams));
        assert_eq!(Troop::Oil.hits_per_casualty(), 160, "the table's number, which the burn ignores");
        let mut pot = Figure::new(Troop::Oil, SIDE_A, 4);
        for _ in 0..34 {
            burn(&mut pot, 0, is_engine(Troop::Oil));
        }
        assert_eq!(pot.men, 3, "100 hits, not 160");
    }

    /// **A fire remembers what it burnt and gives it back**, and what burns
    /// away is a bridge and a wood.
    #[test]
    fn a_fire_puts_back_the_ground_except_a_bridge_and_a_wood() {
        let mut field = blank_field();
        let mut ms = Missiles::new();
        let at = |x: usize, y: usize| y * DIM + x;
        field.cells[at(10, 10)].surface = 4;
        field.cells[at(11, 10)].surface = SURFACE_BRIDGE;
        field.cells[at(11, 10)].elevation = 2;
        field.cells[at(11, 10)].flags = 0x10;
        field.cells[at(11, 10)].gfx = 150;

        ignite(&mut field, &mut ms, 10, 10, 0x78);
        ignite(&mut field, &mut ms, 11, 10, 0);
        assert_eq!(field.cells[at(10, 10)].surface, SURFACE_BURNING);
        let bridge = field.cells[at(11, 10)];
        assert_eq!(
            (bridge.surface, bridge.flags, bridge.elevation, bridge.gfx),
            (SURFACE_BURNING, 0, 0, 0),
            "a bridge is flattened and cleared the moment it catches"
        );
        let (a, b) = (*ms.get(1), *ms.get(2));
        assert_eq!((a.class, a.ttl, a.saved_surface, a.owner), (CLASS_FIRE, 520, 4, 1));
        assert_eq!((b.ttl, b.saved_surface), (640, SURFACE_BRIDGE));
        assert_eq!((a.sub_steps, a.range_ticks), (0, 20_000), "a fire does not fly");

        put_out(&mut field, &a);
        put_out(&mut field, &b);
        assert_eq!(field.cells[at(10, 10)].surface, 4, "the rampart comes back");
        assert_eq!(field.cells[at(11, 10)].surface, SURFACE_BURNT_BRIDGE, "the bridge does not");

        let mut wood = blank_field();
        wood.cells[at(3, 4)].surface = SURFACE_WOODLAND;
        let cell = ignite_woodland(&mut wood, &mut ms, 3, 4).unwrap();
        let fire = *ms.iter().find(|(_, m)| m.cell_x == 3 && m.cell_y == 4).unwrap().1;
        assert_eq!(fire.ttl, 640 - 10 * 7, "0x280 - 10 * ((x + y) & 0x1F)");
        assert_eq!(wood.cells[cell].surface, SURFACE_WOOD_CATCHING);
        put_out(&mut wood, &fire);
        assert_eq!(wood.cells[cell].surface, 0, "a burnt wood is surface 0");
    }

    /// **A bridge fire walks five cells along the bridge and no further**, each
    /// cell seven frames longer-lived than the last, and a cell of the bailey
    /// beside it is only scorched.
    ///
    /// The durations are pinned as the literals `FUN_0048551D` produces: 520,
    /// then `0x280 − (0x78 − 7k)`. Ablation: drop the `c` accumulation and every
    /// spread cell reads 520.
    #[test]
    fn a_bridge_burns_five_cells_either_way_and_stops() {
        let mut field = blank_field();
        let mut ms = Missiles::new();
        for y in 30..=45usize {
            field.cells[y * DIM + 40].surface = SURFACE_BRIDGE;
        }
        field.cells[38 * DIM + 41].surface = SURFACE_BURNT_BRIDGE;
        field.cells[38 * DIM + 41].gfx = 0x5A;

        bridge_fire(&mut field, &mut ms, 40, 38);

        let burning: Vec<usize> =
            (30..=45).filter(|&y| field.cells[y * DIM + 40].surface == SURFACE_BURNING).collect();
        assert_eq!(burning, (33..=43).collect::<Vec<_>>(), "five either way of row 38");
        assert_eq!(field.cells[32 * DIM + 40].surface, SURFACE_BRIDGE);
        assert_eq!(field.cells[44 * DIM + 40].surface, SURFACE_BRIDGE);
        assert_eq!(field.cells[38 * DIM + 41].gfx, 0x0A, "scorched, FUN_00485B47");

        let life = |y: i16| ms.iter().find(|(_, m)| m.cell_y == y && m.cell_x == 40).unwrap().1.ttl;
        assert_eq!(life(38), 520);
        // Ring 1 is scanned top row first: row 37 catches before row 39.
        assert_eq!(life(37), 520);
        assert_eq!(life(39), 527);
        assert_eq!(life(36), 534);
        assert_eq!(life(40), 541);
        assert_eq!(life(33), 576);
        assert_eq!(life(43), 583);
        assert_eq!(ms.live(), 11);
    }

    /// **A wood fire is one ring a frame**, and a cell that caught this frame
    /// does not light its neighbour until the next.
    #[test]
    fn a_wood_fire_floods_the_wood_one_ring_a_frame() {
        let mut field = blank_field();
        let mut ms = Missiles::new();
        for x in 10..=20usize {
            field.cells[10 * DIM + x].surface = SURFACE_WOODLAND;
        }
        ignite_woodland(&mut field, &mut ms, 15, 10).unwrap();
        let mut spreading = true;
        let row = |f: &Battlefield| (10..=20).map(|x| f.cells[10 * DIM + x].surface).collect::<Vec<_>>();

        spread_woodland(&mut field, &mut ms, &mut spreading);
        let r = row(&field);
        assert_eq!(r[5], SURFACE_WOOD_BURNING);
        assert_eq!((r[4], r[6]), (SURFACE_WOOD_CATCHING, SURFACE_WOOD_CATCHING));
        assert_eq!((r[3], r[7]), (SURFACE_WOODLAND, SURFACE_WOODLAND), "one ring, not two");
        assert!(spreading);

        for _ in 0..5 {
            spread_woodland(&mut field, &mut ms, &mut spreading);
        }
        assert!(row(&field).iter().all(|&s| s == SURFACE_WOOD_BURNING));
        spread_woodland(&mut field, &mut ms, &mut spreading);
        assert!(!spreading, "a frame that turns nothing from catching to burning ends it");
    }

    /// The pour's two tables, from `BattleUnit_Order` and `FUN_0047A814`.
    #[test]
    fn an_order_downhill_pours_and_the_pot_turns_along_the_longer_axis() {
        assert!(order_pours(4, 1) && order_pours(4, 3) && !order_pours(4, 4));
        assert!(order_pours(5, 3) && !order_pours(5, 4), "the bailey pours only below the rampart");
        assert!(order_pours(6, 5) && !order_pours(6, 6));
        assert!(!order_pours(1, 0), "a pot on the field never pours from an order");
        assert_eq!(pour_facing((10, 10), (10, 20)), 4);
        assert_eq!(pour_facing((10, 10), (10, 0)), 0);
        assert_eq!(pour_facing((10, 10), (15, 15)), 2, "a tie goes to x");
        assert_eq!(pour_facing((10, 10), (5, 12)), 6);
        let m = oil_record(2, 7, (10, 10), (10, 20));
        assert_eq!((m.class, m.sub_steps, m.range_ticks, m.power, m.launch_elevation), (7, 16, 16, 0, 4));
        assert_eq!((m.shooter, m.dir, m.ttl), (7, 4, 0));
    }
}
