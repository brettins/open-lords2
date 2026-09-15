//! `Battlefield_BuildCastle` (`0x0047C4BA`) reads a stock layout out of the
//! shipped art — a raster per castle type, translated cell by cell through a
//! 256-entry table at `0x004D7D80` whose first byte is either a height or one
//! of the structure codes 5…12 (`docs/battle.md` §3.0.1). We have read the
//! *translation*; we have not read the *rasters*, and the rasters are the
//! castle. [`our_castle`] therefore builds a plain concentric keep of our own
//! design that has one of everything the rules need, and says so in its name
//! and everywhere it is drawn. **Nothing here should be taken as a statement
//! about what a Lords of the Realm II castle looks like.**
//!
//! > **The 9 × 9 is the whole of why a besieger can win.** Until it was found,
//! > a breach was one cell wide, and 848 men queueing at a one-cell hole is
//! > indistinguishable from a besieger who cannot press an assault home. The
//! > original's two wall-attack states both call `Wall_Smash`
//! > (`FUN_0049694F`) with a radius of **4**, and so does the gate.
//!
//! A besieger who cannot cross the moat cannot touch the wall, so the first
//! thing a siege does is shovel the ditch full — [`fill_moat_cell`],
//! `FUN_0047DD86`, once per cell and **four loads a cell**, because the fill
//! counter lives in the cell's own terrain byte and the castle builder seeded
//! that with the water id 11 against a threshold of 15.
//!
//! Filling it feeds [`SiegeState::moat_filled`]; a catapult bringing a wall
//! down feeds [`SiegeState::wall_damage`]. Those two are everything
//! `Siege_RecordCastleDamage` (`0x004784CA`) bills a repair from, and they are
//! **not** billed alike: the ditch costs the defender five man-seasons of
//! digging a cell and no materials at all, while the wall costs fifteen *and*
//! the wood or stone the castle is made of. `docs/bugs.md` B69.
//!
//! [`lower_drawbridge`] — `FUN_00496B9F`, battlefield button 2. It is the only
//! thing in a siege that the *defender* initiates, and the price of it is that
//! it sets the same three globals a ram's twenty-thousandth blow sets: the
//! besieger's AI reads an open gate either way. That is the Readme's *"within a
//! siege, drawbridges can not be closed once they have been opened"*, from the
//! inside.
//!
//! Cell flag **`0x08`** is the third way a siege ends and it was in no document
//! before. `Cell_TryEnter` (`0x00490...`) refuses the step for **either** side
//! but, for a side-4 figure only, sets `DAT_00553F3C` on the way past — and
//! `Battle_CheckOutcome` reads that flag as *the besieger has won*. So an
//! attacker does not have to kill the garrison: **getting one man to the
//! keep's door ends the siege.** `[D]` for the mechanism, `[I]` for calling the
//! cell a door.

mod code_part;
pub use code_part::*;

mod castle;
pub use castle::*;
mod damage;
pub use damage::*;
mod drawbridge;
pub use drawbridge::*;
mod moat;
pub use moat::*;
mod siegetower;
pub use siegetower::*;

use crate::terrain::{Battlefield, Cell, DIM};

/// Cell flag `0x20` — **a castle wall**. `Cell_TryEnter` returns 5 for it
/// against a non-zero side and 1 (pass) for side 0, so the garrison walks
/// through its own walls and the besieger does not. `[V]` — `docs/battle.md`
/// §14.2, where state 6 was corrected from *"blocked"* to *"hitting the wall
/// it just walked into"*.
pub const FLAG_WALL: u8 = 0x20;
/// Cell flag `0x40` — **the drawbridge**. [`lower_drawbridge`]
/// (`FUN_00496B9F`, `0x00496B9F`) scans for one of these and does nothing at
/// all if none exists.
///
/// The shipped `Readme.txt` says which castles have one: *"only the
/// Stone and Royal castles have drawbridges."* That is levels **3 and 4**, and
/// it explains why the routine is written as a search that can fail. `[V]` —
/// the errata and the code agree, from opposite ends.
///
/// **And the game names the flag itself.** The button that calls the routine,
/// `FUN_0043BBE7`, refuses with `L2.eng` group **111** *"No drawbridge!"* when
/// the castle is under level 3 and group **157** *"Drawbridge is down."* when
/// it has already fired. `[V]` — `docs/formats/eng.md` §5.
pub const FLAG_DRAWBRIDGE: u8 = 0x40;
pub const FLAG_KEEP: u8 = 0x08;

// **This block was inverted, and every constant in it has moved.** The reading
// it replaces was that 5 was "the rampart" and 4 "what a breach leaves
// behind"; the binary says the opposite of the second half and something else
// again about the first. `docs/decisions.md` `C99` has the whole of
// it. What settled it was an exhaustive search for **writers** of `surface` —
// there are 36 in the binary and they fall into three groups: the castle
// builder's escape codes, the flood classifier that runs immediately after it,
// and the three routines a siege runs on a wall.
//
// | surface | written by | meaning |
// |---:|---|---|
// | 1 | classifier `FUN_0047E7B2`, flooded from the two map corners | the open field |
// | 2 | `Battlefield_BuildCastle`, terrain `0xEE` | water — the moat |
// | 3 | classifier `FUN_0047E668`; `Siege_LowerDrawbridge` | ground-level ground outside the castle, and the bridge patch |
// | **4** | classifier `FUN_0047E52D` — raised ground beside a 5 | **the rampart walk.** What `Siege_FindCellSurface4` hunts |
// | **5** | classifier `FUN_0047E387`, flooded from the keep door and the bridge — **and `Wall_Smash`** | the bailey, and what an opened wall joins |
// | 6 | build code 6, classifier `FUN_0047E263` | the keep, and the `0x08` way in |
// | 7 | build code 7 | the bridge |
// | **8** | build code 8, with flag `0x20` | **an intact, breakable wall** |
// | **9** | `Wall_Collapse` | a wall cell a catapult brought down |
// | 0x0B | build code 9, with flag `0x40` | the raised drawbridge |
// | 0x0E | build code 10 | the classifier's placeholder; nothing survives it |
// | 10, 0x11 | the oil and fire effects | burning |

/// the handler keeps swinging while `Cell_NeighbourHasSurface(x, y-1, off, 8)`
/// — or its two diagonal siblings — still finds one, and drops back to `dly
/// state` when none is left. That test is the whole reason this value has to be
/// distinguishable from what a smashed wall becomes. `[V]`.
pub const SURFACE_WALL: u8 = 8;
/// `BattleMan_StateAttackWall` charges [`RAMPART_HITS`] when **the attacker is
/// standing on a 5** and [`GATE_HITS`] otherwise. So a besieger outside chews
/// the gate at 20,000, and one who is already through chews the next wall at
/// 5,000 — which is what makes a breach spread. `[V]`.
pub const SURFACE_BAILEY: u8 = 5;
/// `Siege_FindCellSurface4` (`0x00496566`) is that search, and its four callers
/// are `Order_ToNearestWallCell`, `Order_ToWallBelowKeep`,
/// `Order_ToWallNearPreferredTarget` and `Order_ToWallNearAvoidedTarget` —
/// every one of them a defender putting a unit *on the wall*. The classifier
/// writes it onto cells of non-zero elevation adjacent to the bailey. `[V]`.
pub const SURFACE_RAMPART_WALK: u8 = 4;
pub const SURFACE_COLLAPSED: u8 = 9;
pub const SURFACE_GROUND: u8 = 3;
/// Surface **2** — water. `Formation_SendFigure` sends any non-knight ordered
/// onto one of these into the moat-fill state, and `FUN_00496768` — the search
/// behind `Order_ToBreachOrStaging`'s first arm — hunts for it.
pub const SURFACE_WATER: u8 = 2;
pub const SURFACE_FIELD: u8 = 1;
pub const SURFACE_KEEP: u8 = 6;
pub const SURFACE_DRAWBRIDGE: u8 = 0x0B;

/// 1. `g_siegeApproachScore = 500`, before the layout raster is read;
/// 2. the raster's second pass hands terrain byte `0xEE` — a **moat** cell — to
///    `FUN_0047DCCE`, whose first statement is `g_siegeApproachScore = 0`. So
///    one ditch cell anywhere on the field puts it back to zero;
/// 3. as the last statement but one, `Siege_RestoreCastleDamage`
///    (`0x004787A4`), which restores the county's stored scores **only when
///    `castleDegraded == 2`** — only on a *repeat* assault. A first assault
///    keeps whatever 1 and 2 left.
///
/// It is load-bearing. Four attacker handlers open with
/// `approach_score < 3 || breach_score == 0`; start a **dry** castle at 0 and
/// the besieger never leaves the approach ladder however long the battle runs,
/// and start a **moated** one at 500 and nobody ever fills the ditch. `[V]`.
pub const APPROACH_SCORE_START: i32 = 500;

/// `Battlefield_BuildCastle` writes it as
/// `(g_castleLevel == 0 || g_castleLevel == 3) ? 1 : 0` — so **the palisade
/// and the stone castle ship with a gap in the outer wall and the other three
/// do not**, which is a statement about two of the five layout rasters and not
/// about castle size. The skirmish path reads the same quantity out of a
/// twenty-entry table at `0x004D4A98`, one entry per skirmish castle, holding
/// `1 0 0 0 0 1 0 0 1 0 1 0 0 0 0 0 0 1 1 1`. Seven of twenty.
///
/// `[V]` on the values. Our own castle has no such gap — [`our_castle`] draws
/// an unbroken ring — so seeding this is the layout property arriving without
/// the layout, and it is seeded anyway because the *counter* is the original's
/// and the AI reads it.
pub const RAMPART_GAP_AT_BUILD: [u8; 5] = [1, 0, 0, 1, 0];

/// **A wall stands one cell high.** `Battlefield_BuildCastle`'s structure code
/// 8 writes `elevation = 1`, and it is a named constant because
/// `movement::can_step_elevation` allows a difference of exactly one: a wall
/// built two high is a wall nobody can walk onto even after it has been
/// smashed open, and the siege then runs for ever with a hole in it. `[V]` —
/// the literal in the builder.
pub const WALL_ELEVATION: u8 = 1;

/// **What a *collapsed* wall is left standing at: nothing.** `FUN_0047DFE0`,
/// the routine a catapult shot runs when its fourth hit brings a wall cell
/// down, writes `elevation = 0` over that cell.
///
/// `[V]` — the literal in the collapse routine.
pub const BREACH_ELEVATION: u8 = 0;

/// Hits one rampart patch absorbs before it becomes [`SURFACE_BAILEY`] and the
/// counter resets. `g_wallHitsRampart` (`0x00554034`).
pub const RAMPART_HITS: u32 = 5_000;
/// Hits the gate absorbs. `g_wallHitsGate` (`0x00568DA4`) — **one-shot**: it
/// never resets, so a siege gets exactly one gate breach.
pub const GATE_HITS: u32 = 20_000;
pub const RAM_HITS_PER_FRAME: u32 = 20;
pub const WALL_HITS_PER_MAN: u32 = 1;
pub const GATE_BREACH_SCORE: i32 = 4;


/// `g_moatFillSteps` — how many loads of earth one moat cell swallows before
/// [`fill_moat_cell`] turns it into ground. `0x0F`, written once at
/// `0x004975...`; the original keeps the running count in the cell's **terrain**
/// byte, so [`fill_moat_cell`] zeroes it on the way past. `[V]` — the
/// literal in the binary.
pub const MOAT_FILL_STEPS: u8 = 15;

/// **`BattleMan_StateFillMoat` (`0x00483FE1`) is faster for an AI.** The
/// original picks the threshold with `ownerIsHuman ? 100 : 0x50` and then tests
/// `threshold < counter`, so a human's man is 101 frames a load and an AI's 81.
///
/// `[V]`.
pub const MOAT_TICKS_PER_LOAD_HUMAN: u8 = 100;
pub const MOAT_TICKS_PER_LOAD_AI: u8 = 0x50;

/// Surface **1** — what a filled-in moat cell becomes. `FUN_0047DD86` writes
/// `surface = 1, flags = 0, frame &= 0x0F`.
pub const SURFACE_FILLED: u8 = 1;


/// The patch `FUN_00496B9F` lays down: **7 rows of 4 cells**, anchored at the
/// first `flags & FLAG_DRAWBRIDGE` cell in row-major order and running south
/// and east from it. The inner loop steps one cell east four times and the
/// outer adds `0x260` — 76 cells — which is exactly one row on an 80-wide
/// field. `[V]`.
pub const DRAWBRIDGE_ROWS: usize = 7;
pub const DRAWBRIDGE_COLS: usize = 4;

/// `DAT_004D9E18` — the 28 tile frames the drawbridge patch is drawn with,
/// row-major, one per cell of the 7 × 4.
///
/// **Read out of `Lords2.exe`** (`docs/decisions.md`
/// C3, and the reading-comprehension failure recorded in `docs/battle.md`
/// §8.2a): the original indexes it `(&DAT_004D9E18)[i * 4]`, so it is 28
/// **`i32`s**, 112 bytes, at file offset `0xD8018`. Frame 197 is the filler —
/// eighteen of the twenty-eight cells are it — and the other six trace the
/// bridge's shape into the corner nearest the gate. A table whose values were
/// *all* distinct, or all the same, would both have refuted the reading; this
/// one does neither. `[V]`.
pub const DRAWBRIDGE_FRAMES: [u8; DRAWBRIDGE_ROWS * DRAWBRIDGE_COLS] = [
    198, 198, 198, 198, //
    198, 198, 197, 197, //
    198, 196, 197, 197, //
    198, 195, 197, 197, //
    193, 194, 197, 197, //
    192, 197, 197, 197, //
    197, 197, 197, 197, //
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SiegeState {
    pub is_siege: bool,
    /// The castle level being fought, 0…4 — `g_castleLevel`
    /// (`DAT_0055401C`). `Battle_CheckOutcome` reads it to decide whether a
    /// repulsed assault repeats or ends the battle.
    pub castle_level: u8,
    pub rampart_hits: u32,
    pub gate_hits: u32,
    pub ramparts_breached: u32,
    pub gate_breached: bool,
    /// `DAT_00553F3C` — a side-4 figure reached a [`FLAG_KEEP`] cell.
    pub broke_in: bool,
    /// `DAT_0052AF9C` — whether [`lower_drawbridge`] has already fired. It is a
    /// one-shot latch and the Readme says so: *"Within a siege, drawbridges can
    /// not be closed once they have been opened."*
    pub drawbridge_down: bool,
    /// **`DAT_0057A0D8` — moat cells filled in**, and the first of the two
    /// numbers `Siege_RecordCastleDamage` (`0x004784CA`) bills the repair from.
    ///
    /// > **`docs/symbols.md` calls this `breachDamage` and that is a
    /// > misnomer.** The whole binary holds three writers of `DAT_0057A0D8`:
    ///
    /// > `Battle_Start` zeroes it, `FUN_004787A4` restores it from the county,
    /// > and **`FUN_0047DD86` — the moat fill — adds one**. Nothing about a
    /// > breach touches it. It counts cells of water that were shovelled full,
/// > so it costs the county **work and no materials**: five
    /// > man-seasons a cell to dig out again, and not a stick of wood.
    ///
    /// > `[V]` — an exhaustive search for writers.
    pub moat_filled: u16,
    /// **`DAT_0056D648` — rampart cells left hanging by a collapse**, and the
    /// second number in the bill. `FUN_0047DFE0` adds one for each of the four
    /// orthogonal neighbours of a collapsing wall cell that is still
    /// [`SURFACE_BAILEY`], so a shot into the middle of a wall costs the
    /// defender twice what a shot into its end does. This is the number the
    /// repair is charged in **wood or stone** — `docs/bugs.md` B69. `[V]`.
    pub wall_damage: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CastleDamage {
    /// [`SiegeState::moat_filled`] — `+0x1E4`.
    pub moat_filled: u16,
    /// [`SiegeState::wall_damage`] — `+0x1E6`.
    pub wall_damage: u16,
    /// `g_siegeBreachScore` — `+0x1E8`.
    pub breach_score: i32,
    /// `g_siegeApproachScore` — `+0x1EC`.
    pub approach_score: i32,
    /// `_DAT_0055307C` — `+0x1F0`, how many rampart patches have come down.
    pub ramparts_breached: u8,
    /// `_DAT_00569588` — `+0x1F1`, the gate is open. Set by the twenty-thousandth
    /// hit **and by [`lower_drawbridge`]**, which is the same state reached from
    /// the inside.
    pub gate_open: bool,
}

impl CastleDamage {
    /// `if (g_battleIsSiege != 0 && (DAT_0057A0D8 != 0 || DAT_0056D648 != 0))`.
    pub fn any(&self) -> bool {
        self.moat_filled != 0 || self.wall_damage != 0
    }
}

impl SiegeState {
    pub fn field() -> SiegeState {
        SiegeState::default()
    }

    pub fn castle(level: u8) -> SiegeState {
        SiegeState {
            is_siege: true,
            castle_level: level,
            ramparts_breached: u32::from(RAMPART_GAP_AT_BUILD[level.min(4) as usize]),
            ..SiegeState::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallBlow {
    Absorbed,
    RampartBreached,
    GateBreached,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_man_on_the_rampart_chews_the_wall_and_a_man_on_the_ground_chews_the_gate() {
        let mut s = SiegeState::castle(1);
        assert_eq!(s.ramparts_breached, 0);
        for _ in 0..RAMPART_HITS - 1 {
            assert_eq!(strike_wall(&mut s, SURFACE_BAILEY, false), WallBlow::Absorbed);
        }
        assert_eq!(strike_wall(&mut s, SURFACE_BAILEY, false), WallBlow::RampartBreached);
        assert_eq!(s.rampart_hits, 0, "the rampart counter resets");
        assert_eq!(s.ramparts_breached, 1);
        assert_eq!(s.gate_hits, 0, "and the gate is untouched");

        for _ in 0..RAMPART_HITS {
            strike_wall(&mut s, SURFACE_BAILEY, false);
        }
        assert_eq!(s.ramparts_breached, 2);
    }

    #[test]
    fn two_of_the_five_castles_start_with_a_gap_in_the_wall() {
        assert_eq!(RAMPART_GAP_AT_BUILD, [1, 0, 0, 1, 0]);
        for level in 0..=4u8 {
            assert_eq!(
                SiegeState::castle(level).ramparts_breached,
                u32::from(RAMPART_GAP_AT_BUILD[level as usize]),
                "level {level}"
            );
        }
        for level in 0..=4u8 {
            let moated = our_castle(level).cells.iter().any(|c| c.surface == SURFACE_WATER);
            assert_eq!(moated, level >= 2, "level {level} moat");
        }
        assert!(RAMPART_GAP_AT_BUILD[0] == 1 && RAMPART_GAP_AT_BUILD[2] == 0);
    }

    #[test]
    fn one_ram_opens_a_gate_in_a_thousand_frames_and_a_swordsman_needs_twenty_thousand() {
        let mut ram = SiegeState::castle(4);
        let mut frames = 0;
        while !ram.gate_breached {
            strike_wall(&mut ram, SURFACE_GROUND, true);
            frames += 1;
        }
        assert_eq!(frames, 1_000);

        let mut man = SiegeState::castle(4);
        let mut frames = 0;
        while !man.gate_breached {
            strike_wall(&mut man, SURFACE_GROUND, false);
            frames += 1;
        }
        assert_eq!(frames, 20_000);
        assert_eq!(frames / 1_000, RAM_HITS_PER_FRAME as i32);
    }

    #[test]
    fn the_gate_breaks_once_and_never_again() {
        let mut s = SiegeState::castle(0);
        for _ in 0..GATE_HITS * 3 {
            strike_wall(&mut s, SURFACE_GROUND, false);
        }
        assert!(s.gate_breached);
        assert_eq!(strike_wall(&mut s, SURFACE_GROUND, true), WallBlow::Absorbed);
    }

    #[test]
    fn only_the_two_largest_castles_have_a_drawbridge_cell() {
        for level in 0..=4u8 {
            let field = our_castle(level);
            let has = field.cells.iter().any(|c| c.flags & FLAG_DRAWBRIDGE != 0);
            assert_eq!(has, level >= 3, "level {level}");
        }
    }

    #[test]
    fn a_moat_appears_from_the_middle_castle_upward_and_is_water() {
        for level in 0..=4u8 {
            let field = our_castle(level);
            let moat = field.cells.iter().filter(|c| c.surface == SURFACE_WATER).count();
            assert_eq!(moat > 0, level >= 2, "level {level}");
        }
    }

    #[test]
    fn our_castle_has_a_wall_a_bailey_and_a_single_keep_cell() {
        for level in 0..=4u8 {
            let field = our_castle(level);
            let wall = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
            let rampart = field.cells.iter().filter(|c| c.surface == SURFACE_WALL).count();
            let keep = field.cells.iter().filter(|c| c.flags & FLAG_KEEP != 0).count();
            assert_eq!(wall, rampart, "every wall cell is a rampart and vice versa");
            assert!(wall > 0, "level {level} has a wall");
            assert_eq!(keep, 1, "level {level} has exactly one way in");
        }
        let small = our_castle(0).cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
        let large = our_castle(4).cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
        assert!(large > small);
    }
}


