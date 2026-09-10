//! **The castle on the battlefield**: the cell flags a siege turns on, the two
//! damage accumulators that bring a wall down, and the four progress counters
//! every siege order handler reads.
//!
//! `crate::ai` has held all seventeen order handlers since it was written, and
//! **fourteen of them have never once been dispatched**, because nothing in
//! this crate could produce a siege: `BattleRunner` had no castle to fight at,
//! no `is_siege`, and no way for a figure to attack a wall. This module is what
//! makes the siege tables reachable.
//!
//! # ⚠ The castle layout here is **ours**, and it is not the original's
//!
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
//! What *is* the original's, and is reproduced exactly:
//!
//! | | |
//! |---|---|
//! | the flags | `0x20` wall, `0x40` drawbridge, `0x08` the way in — all three from `Cell_TryEnter` |
//! | the accumulators | 5,000 per rampart patch, 20,000 for the gate, `docs/battle.md` §14.3 |
//! | the rates | 1 per frame per man, **20** for a battering ram |
//! | the surfaces | **8 an intact wall, 5 the bailey and what a smashed wall joins, 4 the rampart walk, 9 a collapsed wall, 2 the moat** — this row said the opposite and was wrong; see the table below the flags |
//! | the counters | approach, breach, attackers-on-wall, live engines |
//! | who may attack what | state 14 is reachable **only** by troop type 9 |
//!
//! # How a castle comes down
//!
//! Two counters, and they are not interchangeable:
//!
//! * a figure standing on **surface 5** — the bailey, or a stretch of wall
//!   already opened — feeds [`SiegeState::rampart_hits`]. At [`RAMPART_HITS`]
//!   [`smash_walls`] opens **every wall cell in the 9 × 9 around the attacker**,
//!   the counter **resets**, and another patch can be chewed through. A wall
//!   can be breached repeatedly, and each breach is nine cells wide.
//! * a figure standing anywhere else feeds [`SiegeState::gate_hits`]. At
//!   [`GATE_HITS`] the gate opens **once and for all**, the same 9 × 9 comes
//!   down, and both progress scores gain 4. There is exactly one of those in a
//!   battle.
//!
//! > **The 9 × 9 is the whole of why a besieger can win.** Until it was found,
//! > a breach was one cell wide, and 848 men queueing at a one-cell hole is
//! > indistinguishable from a besieger who cannot press an assault home. The
//! > original's two wall-attack states both call `Wall_Smash`
//! > (`FUN_0049694F`) with a radius of **4**, and so does the gate.
//!
//! A man on foot adds 1 a frame and a ram adds 20, so one ram opens a gate in a
//! thousand frames where a lone swordsman needs twenty thousand. The Readme's
//! *Battering Rams* note — *"Battering rams are only effective at attacking
//! either gatehouses and keeps"* — is the same statement from the other side: a
//! ram cannot climb onto a rampart, so the only counter it can ever feed is the
//! gate's.
//!
//! # The ditch, and the two numbers the county is billed
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
//! # And the garrison has a verb of its own
//!
//! [`lower_drawbridge`] — `FUN_00496B9F`, battlefield button 2. It is the only
//! thing in a siege that the *defender* initiates, and the price of it is that
//! it sets the same three globals a ram's twenty-thousandth blow sets: the
//! besieger's AI reads an open gate either way. That is the Readme's *"within a
//! siege, drawbridges can not be closed once they have been opened"*, from the
//! inside.
//!
//! # And the way in is not a counter at all
//!
//! Cell flag **`0x08`** is the third way a siege ends and it was in no document
//! before. `Cell_TryEnter` (`0x00490...`) refuses the step for **either** side
//! but, for a side-4 figure only, sets `DAT_00553F3C` on the way past — and
//! `Battle_CheckOutcome` reads that flag as *the besieger has won*. So an
//! attacker does not have to kill the garrison: **getting one man to the
//! keep's door ends the siege.** `[D]` for the mechanism, `[I]` for calling the
//! cell a door.

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
/// The shipped `Readme.txt` says which castles have one: *"Note that only the
/// Stone and Royal castles have drawbridges."* That is levels **3 and 4**, and
/// it explains why the routine is written as a search that can fail. `[V]` —
/// the errata and the code agree, from opposite ends.
///
/// **And the game names the flag itself.** The button that calls the routine,
/// `FUN_0043BBE7`, refuses with `L2.eng` group **111** *"No drawbridge!"* when
/// the castle is under level 3 and group **157** *"Drawbridge is down."* when
/// it has already fired. `[V]` — `docs/formats/eng.md` §5.
pub const FLAG_DRAWBRIDGE: u8 = 0x40;
/// Cell flag `0x08` — **the way in**. See the module header: a side-4 figure
/// reaching one wins the siege outright.
pub const FLAG_KEEP: u8 = 0x08;

// ---------------------------------------------------------------------------
// The surfaces — cell byte `+7`
// ---------------------------------------------------------------------------
//
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

/// Surface **8** — **an intact castle wall**, the cell that carries
/// [`FLAG_WALL`].
///
/// `Battlefield_BuildCastle`'s structure code 8 writes it together with flags
/// `0x20 | 0x04` and **elevation 1**, and `BattleMan_StateAttackWall` reads it:
/// the handler keeps swinging while `Cell_NeighbourHasSurface(x, y-1, off, 8)`
/// — or its two diagonal siblings — still finds one, and drops back to `dly
/// state` when none is left. That test is the whole reason this value has to be
/// distinguishable from what a smashed wall becomes. `[V]`.
pub const SURFACE_WALL: u8 = 8;
/// Surface **5** — **the bailey, and what an opened wall joins.**
///
/// Two writers and they mean the same thing. The castle build's flood
/// classifier seeds it from the keep's `0x08` door and from the bridge and
/// floods it across every low cell it can reach, so 5 is *the ground inside*.
/// [`smash_walls`] then writes it over every wall cell it opens, which puts the
/// hole in the same region as the courtyard behind it.
///
/// It is also the test that chooses between the two damage accumulators:
/// `BattleMan_StateAttackWall` charges [`RAMPART_HITS`] when **the attacker is
/// standing on a 5** and [`GATE_HITS`] otherwise. So a besieger outside chews
/// the gate at 20,000, and one who is already through chews the next wall at
/// 5,000 — which is what makes a breach spread. `[V]`.
pub const SURFACE_BAILEY: u8 = 5;
/// Surface **4** — **the rampart walk**, the raised ground the garrison posts
/// on and the only surface any order handler searches for by value.
///
/// `Siege_FindCellSurface4` (`0x00496566`) is that search, and its four callers
/// are `Order_ToNearestWallCell`, `Order_ToWallBelowKeep`,
/// `Order_ToWallNearPreferredTarget` and `Order_ToWallNearAvoidedTarget` —
/// every one of them a defender putting a unit *on the wall*. The classifier
/// writes it onto cells of non-zero elevation adjacent to the bailey. `[V]`.
///
/// > It used to be called `SURFACE_BREACH` and set to what a breach leaves
/// > behind. Nothing in the binary ever writes 4 outside the classifier.
pub const SURFACE_RAMPART_WALK: u8 = 4;
/// Surface **9** — **a wall cell a catapult brought down**, [`collapse_wall`].
/// Distinct from [`SURFACE_BAILEY`], which is what a *smashed* wall becomes:
/// the two routines are different and leave different marks, and only the
/// second joins the courtyard region the AI walks through.
pub const SURFACE_COLLAPSED: u8 = 9;
/// Surface **3** — ground-level ground outside the castle, and the surface
/// `Siege_LowerDrawbridge` writes over its 7 × 4 patch.
pub const SURFACE_GROUND: u8 = 3;
/// Surface **2** — water. `Formation_SendFigure` sends any non-knight ordered
/// onto one of these into the moat-fill state, and `FUN_00496768` — the search
/// behind `Order_ToBreachOrStaging`'s first arm — hunts for it.
pub const SURFACE_WATER: u8 = 2;
/// Surface **1** — the open field the two armies deploy on.
pub const SURFACE_FIELD: u8 = 1;
/// Surface **6** — the keep, and the cell that carries [`FLAG_KEEP`].
pub const SURFACE_KEEP: u8 = 6;
/// Surface **0x0B** — a **raised** drawbridge, the cell that carries
/// [`FLAG_DRAWBRIDGE`]. `Battlefield_BuildCastle`'s structure code 9 writes it
/// with `flags = 0x40` and elevation 0; both [`lower_drawbridge`] and
/// [`smash_walls`] overwrite it.
pub const SURFACE_DRAWBRIDGE: u8 = 0x0B;

/// **`g_siegeApproachScore` starts a fresh siege at 500 — unless the castle
/// has a moat, when it starts at 0.** Every siege order handler branches on it,
/// and the pair is the whole shape of the besieger's opening move.
///
/// Three writes in `Battlefield_BuildCastle`, in this order:
///
/// 1. `g_siegeApproachScore = 500`, before the layout raster is read;
/// 2. the raster's second pass hands terrain byte `0xEE` — a **moat** cell — to
///    `FUN_0047DCCE`, whose first statement is `g_siegeApproachScore = 0`. So
///    one ditch cell anywhere on the field puts it back to zero;
/// 3. as the last statement but one, `Siege_RestoreCastleDamage`
///    (`0x004787A4`), which restores the county's stored scores **only when
///    `castleDegraded == 2`** — only on a *repeat* assault. A first assault
///    keeps whatever 1 and 2 left.
///
/// Read against `Order_ToBreachOrStaging`'s three arms — under 16 hunt the
/// ditch, 16 to 400 fall back on staging, over 400 **do nothing at all** — the
/// two values are one design:
///
/// * **a moated castle opens at 0**, so the besieger's whole ladder is spent
///   shovelling, and `Moat_Fill`'s one-to-four points a cell is what
///   eventually carries it past the `approach_score < 3` gate and opens the
///   assault;
/// * **a dry castle opens at 500**, the approach is already *done*, and the
///   only thing holding the ladder shut is `breach_score == 0` — which the
///   siege engines are there to answer.
///
/// > `docs/battle.md` §16.1 says the restore "overwrites the fresh
/// > `g_siegeApproachScore = 500` that `Battle_Start` wrote a moment earlier".
/// > All three halves of that are wrong: the 500 is the castle builder's, the
/// > restore only fires on a repeat assault, and what actually overwrites it is
/// > the moat.
///
/// It is load-bearing rather than cosmetic. Four attacker handlers open with
/// `approach_score < 3 || breach_score == 0`; start a **dry** castle at 0 and
/// the besieger never leaves the approach ladder however long the battle runs,
/// and start a **moated** one at 500 and nobody ever fills the ditch. `[V]`.
pub const APPROACH_SCORE_START: i32 = 500;

/// [`APPROACH_SCORE_START`] or zero, by whether this battlefield has a ditch —
/// the two-line consequence of the three writes that constant documents.
pub fn approach_score_at_build(field: &Battlefield) -> i32 {
    if field.cells.iter().any(|c| c.surface == SURFACE_WATER) {
        0
    } else {
        APPROACH_SCORE_START
    }
}

/// **`_DAT_0055307C`'s starting value, by campaign castle level.**
///
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
/// [`smash_walls`] deliberately does **not** do this — the original leaves the
/// elevation alone there — which is why [`WALL_ELEVATION`] has to be 1.
/// `[V]` — the literal in the collapse routine.
pub const BREACH_ELEVATION: u8 = 0;

/// Hits one rampart patch absorbs before it becomes [`SURFACE_BAILEY`] and the
/// counter resets. `g_wallHitsRampart` (`0x00554034`).
pub const RAMPART_HITS: u32 = 5_000;
/// Hits the gate absorbs. `g_wallHitsGate` (`0x00568DA4`) — **one-shot**: it
/// never resets, so a siege gets exactly one gate breach.
pub const GATE_HITS: u32 = 20_000;
/// What a battering ram adds per frame. Everything else adds
/// [`WALL_HITS_PER_MAN`].
pub const RAM_HITS_PER_FRAME: u32 = 20;
/// What anything that is not a ram adds per frame, per figure.
pub const WALL_HITS_PER_MAN: u32 = 1;
/// Both progress scores gain this when the gate goes.
pub const GATE_BREACH_SCORE: i32 = 4;

// ---------------------------------------------------------------------------
// The moat
// ---------------------------------------------------------------------------

/// `g_moatFillSteps` — how many loads of earth one moat cell swallows before
/// [`fill_moat_cell`] turns it into ground. `0x0F`, written once at
/// `0x004975...`; the original keeps the running count in the cell's **terrain**
/// byte, which is why [`fill_moat_cell`] zeroes it on the way past. `[V]` — the
/// literal in the binary.
pub const MOAT_FILL_STEPS: u8 = 15;

/// Frames one figure spends on each of those fifteen loads.
///
/// **`BattleMan_StateFillMoat` (`0x00483FE1`) is faster for an AI.** The
/// original picks the threshold with `ownerIsHuman ? 100 : 0x50` and then tests
/// `threshold < counter`, so a human's man is 101 frames a load and an AI's 81.
/// That is a fifth ownerIsHuman asymmetry, on top of the four
/// `docs/battle.md` §6.2 lists, and it is reproduced rather than levelled.
/// `[V]`.
pub const MOAT_TICKS_PER_LOAD_HUMAN: u8 = 100;
pub const MOAT_TICKS_PER_LOAD_AI: u8 = 0x50;

/// Surface **1** — what a filled-in moat cell becomes. `FUN_0047DD86` writes
/// `surface = 1, flags = 0, frame &= 0x0F`.
pub const SURFACE_FILLED: u8 = 1;

// ---------------------------------------------------------------------------
// The drawbridge
// ---------------------------------------------------------------------------

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
/// **Read out of `Lords2.exe` rather than out of a listing** (`docs/decisions.md`
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

/// The siege half of a battle's state — the two accumulators, the two
/// one-shots, and the flag that says an attacker got in.
///
/// It lives beside `BattleRunner`'s other simulation state and is part of the
/// lockstep checksum for the same reason everything else there is: two peers
/// that disagree about whether the gate is open are watching different battles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SiegeState {
    /// Is this a siege at all? `g_battleIsSiege`.
    pub is_siege: bool,
    /// The castle level being fought, 0…4 — `g_castleLevel`
    /// (`DAT_0055401C`). `Battle_CheckOutcome` reads it to decide whether a
    /// repulsed assault repeats or ends the battle.
    pub castle_level: u8,
    /// `g_wallHitsRampart`. Resets every [`RAMPART_HITS`].
    pub rampart_hits: u32,
    /// `g_wallHitsGate`. Never resets.
    pub gate_hits: u32,
    /// `g_rampartCellsBreached` — how many patches have come down.
    pub ramparts_breached: u32,
    /// `g_gateBreached`.
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
    /// > `Battle_Start` zeroes it, `FUN_004787A4` restores it from the county,
    /// > and **`FUN_0047DD86` — the moat fill — adds one**. Nothing about a
    /// > breach touches it. It counts cells of water that were shovelled full,
    /// > which is why it costs the county **work and no materials**: five
    /// > man-seasons a cell to dig out again, and not a stick of wood.
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

/// What one siege did to the castle: the pair
/// [`SiegeState::moat_filled`] / [`SiegeState::wall_damage`], plus the two
/// progress scores and the two one-shots the county stores between assaults.
///
/// It exists so that `l2-kingdom` can bill the repair without depending on
/// `l2-sim` — `docs/plan.md`'s one-way rule — and so that the autocalc path can
/// hand over an honest **nothing**: `Battle_AutoResolve` never touches an
/// accumulator, so a siege that was calculated rather than fought does no
/// damage at all and `Siege_RecordCastleDamage` returns at its first `if`.
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
    /// Whether `Siege_RecordCastleDamage` would do anything at all:
    /// `if (g_battleIsSiege != 0 && (DAT_0057A0D8 != 0 || DAT_0056D648 != 0))`.
    pub fn any(&self) -> bool {
        self.moat_filled != 0 || self.wall_damage != 0
    }
}

impl SiegeState {
    /// A field battle: not a siege, nothing breached.
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

/// What one figure's blow against a wall did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallBlow {
    /// Absorbed by the running total.
    Absorbed,
    /// A rampart patch came down: [`smash_walls`] opens the 9 × 9 around the attacker.
    RampartBreached,
    /// The gate went. Both progress scores gain [`GATE_BREACH_SCORE`].
    GateBreached,
}

/// A figure hits the wall — `BattleMan_Step`'s state 6 and state 14.
///
/// `standing_on` is the **surface the attacker is standing on**, not the
/// surface it is hitting, and that is the whole of how the original chooses
/// between its two accumulators. `is_ram` is `troop == BatteringRams`; state 14
/// is reachable by nothing else, which `docs/battle.md` §14.3 establishes from
/// `Cell_TryEnterEngine` returning 6 only for `troopType == 9`.
pub fn strike_wall(state: &mut SiegeState, standing_on: u8, is_ram: bool) -> WallBlow {
    let hits = if is_ram { RAM_HITS_PER_FRAME } else { WALL_HITS_PER_MAN };
    if standing_on == SURFACE_BAILEY {
        state.rampart_hits += hits;
        if state.rampart_hits >= RAMPART_HITS {
            state.rampart_hits = 0;
            state.ramparts_breached += 1;
            return WallBlow::RampartBreached;
        }
        return WallBlow::Absorbed;
    }
    if state.gate_breached {
        // The counter never resets and the breach never un-happens, so a man
        // still swinging at an open gate achieves nothing. The original keeps
        // adding to a total nothing reads again; the effect is the same.
        state.gate_hits += hits;
        return WallBlow::Absorbed;
    }
    state.gate_hits += hits;
    if state.gate_hits >= GATE_HITS {
        state.gate_breached = true;
        return WallBlow::GateBreached;
    }
    WallBlow::Absorbed
}

/// **Open the wall** — `Wall_Smash`, `FUN_0049694F` (`0x0049694F`), which both
/// wall-attack states call with a radius of **4** the moment either
/// accumulator crosses its threshold.
///
/// ```c
/// Sound_PlayFile("bathit2.wav");
/// for every cell of the (2*r+1) square around (x, y), clipped at the field edge:
///     if (flags & 0x20) { flags &= ~0x20; surface = 5; frame[+0x280] += 0x10; }
///     if (flags & 0x40) { flags  =  0;    surface = 5; frame       += 0x28; }
/// Path_BuildTerrainTemplate(); Path_BuildStepCost();
/// ```
///
/// # This is the routine that makes an assault possible
///
/// A breach is **nine cells wide**, not one. Our `strike_castle` used to open
/// the single cell the attacker had walked into, and a single-cell gap in a
/// castle wall is a funnel that a few dozen figures cannot clear: they arrive,
/// the first one blocks it, the pathfinder marks a friendly-occupied cell 998,
/// and the rest stand outside for as long as you care to run the simulation.
/// Fought at scale that is exactly the observation the branch started from —
/// *848 men outside, two garrison figures alive, 400,000 frames*.
///
/// # Three details that are the original's and not obvious
///
/// * **The elevation is not touched.** A smashed wall stays at the height the
///   builder gave it, which is [`WALL_ELEVATION`] — one — and
///   `movement::can_step_elevation` allows exactly that. It is
///   [`collapse_wall`], the catapult's routine, that flattens a cell.
/// * **A wall and a drawbridge both become [`SURFACE_BAILEY`]**, so the hole
///   joins the courtyard region rather than becoming a category of its own.
///   That is what puts a besieger who is standing in it onto the *rampart*
///   accumulator, and it is why a breach spreads at 5,000 hits rather than
///   20,000.
/// * **The wall's graphic bump lands one row south** — `frame[+0x280] += 0x10`
///   — while the drawbridge's lands on the cell itself. Reproduced, because
///   the two are not the same offset in the original and there is no reason to
///   believe that is an accident. [`Cell`] carries no `flags2`, so the two
///   `|= 1` writes are dropped; nothing in this engine reads that byte.
///
/// Returns how many cells were opened, which is 0 when the square held no wall
/// at all.
pub fn smash_walls(field: &mut Battlefield, x: i32, y: i32, radius: i32) -> usize {
    let mut opened = 0;
    for cy in (y - radius).max(0)..=(y + radius).min(DIM as i32 - 1) {
        for cx in (x - radius).max(0)..=(x + radius).min(DIM as i32 - 1) {
            let c = cy as usize * DIM + cx as usize;
            let flags = field.cells[c].flags;
            if flags & FLAG_WALL != 0 {
                field.cells[c].flags &= !FLAG_WALL;
                field.cells[c].surface = SURFACE_BAILEY;
                // The frame bump is on the cell one row **south**, which is
                // where the original writes it.
                if cy + 1 < DIM as i32 {
                    let s = (cy as usize + 1) * DIM + cx as usize;
                    field.cells[s].gfx = field.cells[s].gfx.wrapping_add(0x10);
                }
                opened += 1;
            }
            if field.cells[c].flags & FLAG_DRAWBRIDGE != 0 {
                field.cells[c].flags = 0;
                field.cells[c].surface = SURFACE_BAILEY;
                field.cells[c].gfx = field.cells[c].gfx.wrapping_add(0x28);
                opened += 1;
            }
        }
    }
    opened
}

/// The radius `BattleMan_StateAttackWall` and `BattleMan_StateRamGate` both
/// pass to [`smash_walls`]. Four, so the square is 9 × 9. `[V]` — the literal
/// at all three call sites.
pub const SMASH_RADIUS: i32 = 4;

/// **A catapult brings a wall cell down** — `Wall_Collapse`, `FUN_0047DFE0`
/// (`0x0047DFE0`).
///
/// ```c
/// surface = 9; flags = 2; elevation = 0; flags2 = (flags2 & 0xE3) | 4;
/// for each of the four orthogonal neighbours still at surface 5:
///     g_siegeBreachScore++;  FUN_0048EE46(nb);  DAT_0056D648++;
/// ```
///
/// It is **not** [`smash_walls`] and leaves a different mark: surface
/// [`SURFACE_COLLAPSED`] rather than [`SURFACE_BAILEY`], the elevation
/// flattened to [`BREACH_ELEVATION`], and `flags` set to 2 outright rather than
/// having a bit cleared — so a collapsed cell carries neither [`FLAG_WALL`] nor
/// [`FLAG_DRAWBRIDGE`] and is passable.
///
/// Returns the number of neighbours billed, which is both the breach score
/// gained and the wall damage the county pays for. `docs/bugs.md` B69: those
/// two are the *same* count, written in one statement per neighbour.
pub fn collapse_wall(field: &mut Battlefield, state: &mut SiegeState, cell: usize) -> i32 {
    {
        let c = &mut field.cells[cell];
        c.surface = SURFACE_COLLAPSED;
        c.flags = 2;
        c.elevation = BREACH_ELEVATION;
    }
    let mut billed = 0;
    for n in orthogonal_neighbours(cell) {
        if field.cells[n].surface == SURFACE_BAILEY {
            billed += 1;
        }
    }
    state.wall_damage = state.wall_damage.saturating_add(billed as u16);
    billed
}

/// **Lower the drawbridge** — `FUN_00496B9F` (`0x00496B9F`), the whole of it.
///
/// The garrison's fifth verb, and the one the battlefield's third button
/// exists for. `FUN_0043BBE7` guards it four ways — a siege, the local player
/// owning army B, `g_castleLevel >= 3`, and the latch — and then this happens:
///
/// ```c
/// scan row-major for the first cell with flags & 0x40
/// for 7 rows: for 4 cells:
///     flags = 0; surface = 3; frame = DAT_004D9E18[i];
///     flags2 |= 1; flags2 &= 0xE3;
/// _DAT_00569588 = 1;                  /* the gate is open */
/// DAT_0052AF9C  = 1;                  /* and it stays open */
/// g_siegeApproachScore += 4;
/// g_siegeBreachScore   += 4;
/// Path_BuildTerrainTemplate(); Path_BuildElevation();
/// ```
///
/// # Three things this is not
///
/// * **It is not siege-engine placement.** The hand-off this was built from
///   said it was. The game names it itself: the button's two refusals are
///   `L2.eng` 111 *"No drawbridge!"* and 157 *"Drawbridge is down."*, the
///   guard is level 3 and up, and the shipped `Readme.txt` says *"only the
///   Stone and Royal castles have drawbridges."* Four sources, one verb.
///   `C79`.
/// * **It is not a hole in the wall for the besieger.** It sets the *same*
///   two globals a twenty-thousandth ram hit sets — `_DAT_00569588` and both
///   scores by 4 — so as far as every AI order handler is concerned **the
///   garrison has opened its own gate**. That is the price of a sally, and it
///   is why the Readme says a drawbridge cannot be closed again.
/// * **It does not fire when there is no drawbridge cell.** The latch is set
///   *inside* the `if`, so a level-3 castle whose layout happens to carry no
///   `0x40` cell leaves the button live. Reproduced.
///
/// > **The original's scan has a missing `break`.** `bVar1 = true; break;`
/// > leaves only the inner loop, and the outer one then re-tests the same cell
/// > for every remaining row, breaking immediately each time. The *answer* is
/// > unaffected — the offset stops at the first `0x40` cell either way — but
/// > `g_foundTileX` / `g_foundTileY` are left at `(0, 0x50)` rather than at the
/// > cell, which is a battlefield-wide scratch pair other routines read.
/// > Nothing was found that reads them between here and their next write.
/// > `docs/bugs.md` `B85`. `[V]` on the control flow, `[I]`
/// > that it is harmless.
///
/// Returns the anchor cell when it fired, so a caller can rebuild whatever it
/// derives from the field. `flags2` — cell byte `+2` — is **not** written:
/// [`Cell`] does not carry it, because nothing in this engine reads it.
pub fn lower_drawbridge(field: &mut Battlefield, state: &mut SiegeState) -> Option<usize> {
    if state.drawbridge_down {
        return None;
    }
    let anchor = field.cells.iter().position(|c| c.flags & FLAG_DRAWBRIDGE != 0)?;
    let (ax, ay) = (anchor % DIM, anchor / DIM);
    for row in 0..DRAWBRIDGE_ROWS {
        for col in 0..DRAWBRIDGE_COLS {
            let (x, y) = (ax + col, ay + row);
            // The original walks a flat offset with no bound check at all and
            // would run off the end of the array; we stop at the edge instead.
            // `docs/bugs.md` N4's reasoning: an overrun is not behaviour.
            if x >= DIM || y >= DIM {
                continue;
            }
            let c = &mut field.cells[y * DIM + x];
            c.flags = 0;
            c.surface = SURFACE_GROUND;
            c.gfx = DRAWBRIDGE_FRAMES[row * DRAWBRIDGE_COLS + col];
        }
    }
    state.drawbridge_down = true;
    state.gate_breached = true;
    Some(anchor)
}

/// **One moat cell is filled in** — `FUN_0047DD86` (`0x0047DD86`), reached from
/// `BattleMan_StateFillMoat` once a figure has tipped [`MOAT_FILL_STEPS`] loads
/// into it.
///
/// ```c
/// surface = 1; flags = 0; frame &= 0x0F;
/// for each of the four orthogonal neighbours:
///     if its surface is 3, 5 or 4  ->  g_siegeApproachScore += 1
/// DAT_0057A0D8 += 1;
/// ```
///
/// So the approach score is worth **up to four** for a cell that opens onto
/// castle ground on every side and nothing at all for one out in the water,
/// which is what makes the besieger's fill work inward. The accumulator goes up
/// by one whatever the neighbours say, and that is the number the county is
/// billed five man-seasons of digging for.
///
/// Returns the approach score gained.
pub fn fill_moat_cell(field: &mut Battlefield, state: &mut SiegeState, cell: usize) -> i32 {
    {
        let c = &mut field.cells[cell];
        c.surface = SURFACE_FILLED;
        c.flags = 0;
        c.gfx &= 0x0F;
        // The terrain byte was the fill counter; the original zeroes it at the
        // call site, immediately before this.
        c.terrain = crate::terrain::id::OPEN;
    }
    let mut score = 0;
    for n in orthogonal_neighbours(cell) {
        if matches!(
            field.cells[n].surface,
            SURFACE_GROUND | SURFACE_BAILEY | SURFACE_RAMPART_WALK
        ) {
            score += 1;
        }
    }
    state.moat_filled = state.moat_filled.saturating_add(1);
    score
}

/// The four orthogonal neighbours of a cell, clipped at the field edge — the
/// order `FUN_0047DD86` and `FUN_0047DFE0` both read them in: north, east,
/// south, west.
pub fn orthogonal_neighbours(cell: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((cell % DIM) as i32, (cell / DIM) as i32);
    [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)].into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        (nx >= 0 && ny >= 0 && nx < DIM as i32 && ny < DIM as i32)
            .then_some(ny as usize * DIM + nx as usize)
    })
}

// ---------------------------------------------------------------------------
// Our castle
// ---------------------------------------------------------------------------

/// **The name is a warning.** This is *our* castle, not the original's — see
/// the module header. It exists so that the fourteen siege order handlers have
/// somewhere to run, and so the damage model has a wall to eat.
///
/// One concentric keep, laid out around the defender's deployment marker:
///
/// ```text
///                 . . . . . . . . . . . . . .      the open field, surface 1
///             ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~ ~      the moat, surface 2, from level 2 up
///             # # # # # # # # # # # # # # # #      the curtain wall, surface 8, flag 0x20, elevation 1
///             # r r r r r r r r r r r r r r #      the rampart walk, surface 4, elevation 1
///             # r . . . . . . . . . . . . r #      the bailey, surface 5, elevation 0
///             # r . . . . K . . . . . . . r #      K: the way in, flag 0x08, surface 6
///             # # # # # G # # # # # # # # # #      G: the gatehouse, flag 0x40
///                       ^ the drawbridge, level 3 and up
/// ```
///
/// Size grows with the level, and two features are gated on it because the
/// game says so: the moat from level 2, and the drawbridge from level 3 —
/// *"only the Stone and Royal castles have drawbridges"*.
///
/// The two deployment markers are the field's: the garrison starts inside, the
/// besieger on the far side of the wall.
///
/// # The surfaces here were the inverse of the binary's, and it was fatal
///
/// This layout used to make the wall `surface = 5` at **elevation 2** and the
/// bailey `surface = 3` at elevation 1. Three things followed and all three
/// broke the siege:
///
/// * a wall two cells high is a wall nobody can step onto after it has been
///   opened — `movement::can_step_elevation` allows a difference of one, and
///   the original's builder writes [`WALL_ELEVATION`];
/// * `Siege_FindCellSurface4`, the only search any order handler runs by
///   surface value, had nothing to find, so **no defender ever posted on the
///   wall**;
/// * a besieger standing outside on a 5 fed the *rampart* accumulator at 5,000
///   rather than the gate's 20,000, which is the wrong one by a factor of four.
///
/// The layout is still ours. The five values in it are the binary's.
pub fn our_castle(level: u8) -> Battlefield {
    use crate::terrain::{flag, id};

    let level = level.min(4);
    // A palisade is a small ring and a royal castle a large one. These numbers
    // are ours; nothing in the binary says a castle is any particular size.
    let half = 6 + level as i32 * 2;
    let (cx, cy) = (40i32, 24i32);

    // The open field. Surface 1 is what the castle build's flood classifier
    // reaches from the two map corners, and it is what everything that is not
    // the castle ends up as.
    let mut cells = vec![
        Cell { terrain: id::OPEN, flags: 0, gfx: 0, elevation: 0, surface: SURFACE_FIELD };
        DIM * DIM
    ];
    let at = |x: i32, y: i32| (y as usize) * DIM + (x as usize);
    let inside = |x: i32, y: i32| (x - cx).abs() <= half && (y - cy).abs() <= half;
    let on_ring = |x: i32, y: i32| {
        inside(x, y) && ((x - cx).abs() == half || (y - cy).abs() == half)
    };

    // The moat, one ring outside the wall, from a Norman keep up.
    if level >= 2 {
        for y in (cy - half - 1)..=(cy + half + 1) {
            for x in (cx - half - 1)..=(cx + half + 1) {
                if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                    continue;
                }
                let ring = (x - cx).abs() == half + 1 || (y - cy).abs() == half + 1;
                if ring {
                    let c = &mut cells[at(x, y)];
                    c.terrain = id::WATER;
                    c.surface = SURFACE_WATER;
                    c.flags |= flag::IMPASSABLE;
                }
            }
        }
    }

    // A ring of ground-level apron immediately outside the moat — surface 3,
    // which is what the classifier writes for elevation-0 ground beside the
    // castle, and what `Moat_Fill` counts as one point of approach score for
    // each neighbour it opens onto.
    let apron = half + if level >= 2 { 2 } else { 1 };
    for y in (cy - apron)..=(cy + apron) {
        for x in (cx - apron)..=(cx + apron) {
            if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                continue;
            }
            if (x - cx).abs() == apron || (y - cy).abs() == apron {
                cells[at(x, y)].surface = SURFACE_GROUND;
            }
        }
    }

    // The bailey, the rampart walk, and the curtain wall.
    for y in (cy - half)..=(cy + half) {
        for x in (cx - half)..=(cx + half) {
            if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                continue;
            }
            // **Two cells in, not one.** The ring immediately inside the wall
            // has to be bailey, because `Wall_Collapse` scores a breach only
            // for the orthogonal neighbours of the collapsed cell that are at
            // [`SURFACE_BAILEY`] — put anything else against the wall's inner
            // face and a catapult can knock the whole curtain down for a
            // breach score of **zero**, which is a besieger whose engines
            // achieve nothing. Measured, not reasoned: four collapsed cells,
            // `breach_score` 0, 100,000 frames.
            let on_walk =
                (x - cx).abs() == half - 2 || (y - cy).abs() == half - 2;
            let c = &mut cells[at(x, y)];
            if on_ring(x, y) {
                c.surface = SURFACE_WALL;
                c.elevation = WALL_ELEVATION;
                c.flags |= FLAG_WALL;
            } else if on_walk {
                // Surface 4 — the one surface an order handler searches for.
                // Without it `Siege_FindCellSurface4` never finds anything and
                // four defender actions are dead.
                c.surface = SURFACE_RAMPART_WALK;
                c.elevation = WALL_ELEVATION;
            } else {
                c.surface = SURFACE_BAILEY;
                c.elevation = 0;
            }
        }
    }

    // **The gatehouse**, in the middle of the wall facing the besieger, where
    // the level has one — *"only the Stone and Royal castles have
    // drawbridges."*
    //
    // It is `DRAWBRIDGE_COLS` wide and three deep so that the patch
    // [`lower_drawbridge`] lays down lands on it: the routine anchors on the
    // **first** `0x40` cell in row-major order and runs 7 × 4 south and east
    // from there, so the block's north-west corner has to be the north-west
    // corner of the patch. Four wall cells, then the moat, then three cells of
    // open ground — which is a bridge across the ditch and a hole in the wall,
    // in one stroke, and is exactly what the four bytes the original writes
    // amount to.
    //
    // While it is up the cells are flagged `0x40`, which `Cell_TryEnter`
    // refuses to **both** sides: a raised drawbridge is a shut gate.
    if level >= 3 {
        let gate_x = cx - DRAWBRIDGE_COLS as i32 / 2;
        for dy in 0..3i32 {
            for dx in 0..DRAWBRIDGE_COLS as i32 {
                let (x, y) = (gate_x + dx, cy + half + dy);
                if !(0..DIM as i32).contains(&x) || !(0..DIM as i32).contains(&y) {
                    continue;
                }
                let c = &mut cells[at(x, y)];
                c.flags = FLAG_DRAWBRIDGE;
                c.terrain = id::OPEN;
                c.surface = SURFACE_DRAWBRIDGE;
                c.elevation = 0;
            }
        }
    }

    // **The way in** — one cell, in the middle of the bailey, and it is left at
    // the bailey's own elevation.
    //
    // > It used to stand at 3 against a bailey of 1, and that made the third
    // > way a siege can end unreachable. `Formation_SlotIsUsable` rejects a
    // > slot more than one below the destination's elevation and
    // > `Formation_RectIsClear` rejects a rectangle whose slots are not *at*
    // > it, so an order onto a keep two levels above its own courtyard is an
    // > order no figure is ever given — the besiegers walk into the bailey,
    // > find nowhere to stand, and the battle runs for ever with the gate open
    // > and two men of the garrison alive in a corner. Found by fighting one to
    // > the end, which nothing had done. The elevation was ours to begin with;
    // > `docs/decisions.md` `C82`.
    cells[at(cx, cy)].flags |= FLAG_KEEP;
    cells[at(cx, cy)].surface = SURFACE_KEEP;

    let mut field = Battlefield {
        cells,
        deploy_side0: [(0, 0); 12],
        deploy_side4: [(0, 0); 12],
        // The garrison deploys inside, on the bailey; the besieger on the open
        // ground well south of the moat.
        home_side0: (cx as u8, (cy + half / 2) as u8),
        home_side4: (cx as u8, 64),
    };
    for (slot, (dx, dy)) in crate::terrain::DEPLOY_OFFSETS.iter().enumerate() {
        let clamp = |v: i32| v.clamp(1, DIM as i32 - 2) as u8;
        // Side 0's slots are squeezed to fit inside the bailey; side 4's are
        // the field's own spread.
        let sx = (dx / 3).clamp(-half + 2, half - 2);
        let sy = (dy / 3).clamp(-half + 2, half - 2);
        field.deploy_side0[slot] =
            (clamp(field.home_side0.0 as i32 + sx), clamp(field.home_side0.1 as i32 + sy));
        field.deploy_side4[slot] =
            (clamp(field.home_side4.0 as i32 + dx), clamp(field.home_side4.1 as i32 + dy));
    }
    field
}

/// The positional tables the siege handlers read, derived from [`our_castle`]'s
/// geometry.
///
/// **Also ours.** In the original these are filled by
/// `Battlefield_BuildCastle` from the layout raster, and `crate::ai`'s
/// [`AiField`](crate::AiField) documents each of them as `[I]` for exactly that
/// reason. What is *not* invented is their meaning and their arity — sixteen
/// wall slots a group, three groups, twenty defence posts, four approach lanes
/// — all of which are read out of the binary and all of which this fills
/// honestly.
pub fn our_castle_ai_field(field: &Battlefield, level: u8) -> crate::AiField {
    let mut f = crate::runner::ai_field_for(field);
    let level = level.min(4);
    let half = 6 + level as i32 * 2;
    let (cx, cy) = (40i16, 24i16);
    let h = half as i16;

    f.castle_ref = (cx, cy);
    f.castle_objective = [
        cy as usize * DIM + cx as usize,
        (cy + h) as usize * DIM + cx as usize,
    ];
    f.castle_index = 13;
    f.layout = 1;
    f.castle_layout_flag = level >= 3;

    // Four approach lanes onto the gate wall, at six stand-off distances.
    //
    // **The furthest of these has to stay inside the ditch search's reach**,
    // and it did not. `Order_ToBreachOrStaging`'s first arm hunts for a
    // surface-2 cell within a radius of 12 to 19 of the unit and **does nothing
    // at all** when it finds none — so a unit parked further out than that from
    // the moat never shovels, never raises the approach score, never passes the
    // `approach_score < 3` gate, and never assaults. The ladder alternates
    // staging with the ditch hunt precisely on the assumption that staging is
    // close enough, and ours was 21 cells beyond the water: measured, 848 men
    // sat in the field for 200,000 frames with a full ditch in front of them.
    // Six rings two cells apart, the outermost `h + 12` from the centre, keeps
    // every rung of the ladder inside the search.
    for (step, row) in f.castle_approach.iter_mut().enumerate() {
        let back = h + 2 + (5 - step as i16) * 2;
        for (lane, point) in row.iter_mut().enumerate() {
            *point = (cx + (lane as i16 - 2) * (h / 2).max(1), cy + back);
        }
    }
    for (lane, point) in f.staging.iter_mut().enumerate() {
        *point = (cx + (lane as i16 - 2) * (h / 2).max(1), cy + h + 4);
    }

    // Wall slots: group 0 walks the rampart, group 1 the inner face, group 2
    // the two corners nearest the gate.
    for slot in 0..16i16 {
        let along = -h + (slot * (2 * h)) / 15;
        f.wall_slot[0][slot as usize] = (cx + along, cy + h);
        f.wall_slot[1][slot as usize] = (cx + along, cy + h - 1);
        f.wall_slot[2][slot as usize] = (cx + (if slot % 2 == 0 { -h } else { h }), cy + along);
    }
    // **The defence posts start empty, and that is the original's.** This used
    // to spread twenty of them along the gate wall. `FUN_0048EE46` — the
    // twenty-entry table's only appender in the whole binary — is called from
    // `Wall_Collapse` and from nowhere else, once per rampart neighbour left
    // hanging by a catapult shot. So a castle nobody has bombarded has no
    // defence posts, `Siege_ClaimDefencePost` returns 0 for every unit, and the
    // garrison's handlers take their `cellOffset == 0` arm — the wall slots —
    // for the whole of that siege. The posts are *the holes*, and they arrive
    // when the holes do.
    f.defence_posts = [0; 20];
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two accumulators, and that they are chosen by where the *attacker*
    /// stands rather than by what it hits.
    #[test]
    fn a_man_on_the_rampart_chews_the_wall_and_a_man_on_the_ground_chews_the_gate() {
        // Level 1: one of the three campaign levels that ship with no gap in
        // the wall, so the counter starts at zero. See
        // [`RAMPART_GAP_AT_BUILD`].
        let mut s = SiegeState::castle(1);
        assert_eq!(s.ramparts_breached, 0);
        for _ in 0..RAMPART_HITS - 1 {
            assert_eq!(strike_wall(&mut s, SURFACE_BAILEY, false), WallBlow::Absorbed);
        }
        assert_eq!(strike_wall(&mut s, SURFACE_BAILEY, false), WallBlow::RampartBreached);
        assert_eq!(s.rampart_hits, 0, "the rampart counter resets");
        assert_eq!(s.ramparts_breached, 1);
        assert_eq!(s.gate_hits, 0, "and the gate is untouched");

        // A wall can be chewed through repeatedly; the gate cannot.
        for _ in 0..RAMPART_HITS {
            strike_wall(&mut s, SURFACE_BAILEY, false);
        }
        assert_eq!(s.ramparts_breached, 2);
    }

    /// **`_DAT_0055307C` is seeded from the castle level, and it is not the
    /// moat flag.** `Battlefield_BuildCastle` writes 1 at campaign levels 0 and
    /// 3 and 0 at the rest — which is not the level ordering a moat would give
    /// and not the ordering castle size would give either.
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
        // And it moves in the *opposite* direction to the moat, which is what
        // rules out the name it used to carry: the moat appears from level 2
        // up, the gap at 0 and 3.
        for level in 0..=4u8 {
            let moated = our_castle(level).cells.iter().any(|c| c.surface == SURFACE_WATER);
            assert_eq!(moated, level >= 2, "level {level} moat");
        }
        assert!(RAMPART_GAP_AT_BUILD[0] == 1 && RAMPART_GAP_AT_BUILD[2] == 0);
    }

    /// A ram is worth twenty men, and that is the whole reason to build one.
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

    /// The gate is one-shot. The rampart is not.
    #[test]
    fn the_gate_breaks_once_and_never_again() {
        let mut s = SiegeState::castle(0);
        for _ in 0..GATE_HITS * 3 {
            strike_wall(&mut s, SURFACE_GROUND, false);
        }
        assert!(s.gate_breached);
        // Nothing further is reported, however long they keep swinging.
        assert_eq!(strike_wall(&mut s, SURFACE_GROUND, true), WallBlow::Absorbed);
    }

    /// The Readme: *"only the Stone and Royal castles have drawbridges."*
    #[test]
    fn only_the_two_largest_castles_have_a_drawbridge_cell() {
        for level in 0..=4u8 {
            let field = our_castle(level);
            let has = field.cells.iter().any(|c| c.flags & FLAG_DRAWBRIDGE != 0);
            assert_eq!(has, level >= 3, "level {level}");
        }
    }

    /// And a moat from a Norman keep up, which is what makes the moat-fill
    /// state reachable at all.
    #[test]
    fn a_moat_appears_from_the_middle_castle_upward_and_is_water() {
        for level in 0..=4u8 {
            let field = our_castle(level);
            let moat = field.cells.iter().filter(|c| c.surface == SURFACE_WATER).count();
            assert_eq!(moat > 0, level >= 2, "level {level}");
        }
    }

    /// Every castle has one wall, one gate side, and exactly one way in.
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
        // And a bigger castle is a bigger castle.
        let small = our_castle(0).cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
        let large = our_castle(4).cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
        assert!(large > small);
    }
}
