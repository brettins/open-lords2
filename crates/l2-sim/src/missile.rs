//! Missile combat: the damage a hit does, and **the arrow that carries it**.
//!
//! Reimplemented from `docs/battle.md` §6.2 and §14.7. Three things distinguish
//! the damage from melee, and all three are easy to lose by accident:
//!
//! * **Armour is a flat subtraction, and applies here only.** It is never read
//!   during a melee exchange.
//! * **High ground protects the target, and only against missiles.** Nothing in
//!   melee reads elevation.
//! * **A missile kill discards the leftover damage; a melee kill carries it.**
//!
//! Integer arithmetic throughout, in the original's order of operations —
//! division before the armour subtraction, clamping after. Reordering changes
//! results, so the order is part of the specification.
//!
//! # The shot is an object, not an outcome
//!
//! This module used to stop at [`resolve_power`]: a hit resolved and nothing
//! flew, so sixty archers fought as sixty men carrying bows. The question that
//! had to be settled before any of this could be written was whether the
//! original resolves a shot at launch and merely animates it, or whether the
//! arrow genuinely traverses. **It genuinely traverses**, and the evidence is
//! one line of `Missile_Step` (`0x00492C8B`):
//!
//! ```c
//! g_otherBattleMan = g_battlefield[missile.cellOffset].figure;
//! ```
//!
//! The victim is read out of the **cell the missile has just entered**, fresh,
//! every sub-step. Nothing anywhere on the 0x4C-byte record remembers who the
//! shot was aimed at — `+0x06` is the *shooter* — so an arrow cannot check
//! whether it hit the right man, and does not. Four consequences, all of them
//! visible in play and all of them reproduced here:
//!
//! * **A body in the way takes the arrow.** Anyone who has walked into the
//!   flight path is hit instead, which is what makes a screening line work.
//! * **A miss keeps flying.** When the Bresenham line is exhausted the missile
//!   switches to [`Missile::step_octant`] and coasts on in its launch
//!   direction until it runs out of range, hits somebody else or leaves the
//!   map. Overshoot can kill a second rank.
//! * **A target that dies or walks away is not tracked.** The impact point is
//!   frozen at launch; the arrow arrives at an empty cell and carries on.
//! * **Friendly fire cannot happen**, because the test is on the *owner* byte
//!   rather than the side — and a friendly body does not stop the arrow either.
//!
//! # The timing is one number wearing two hats
//!
//! A missile advances [`SUB_STEPS`] sub-steps a tick and one sub-step is
//! [`SUB_CELL`]⁻¹ of a cell, so a tick is exactly ⅛ of a cell. The range in
//! `g_missileStats` is stored in **eighths of a cell**, so the same number is
//! both the distance and the tick budget, and `range >> 3` is the range in
//! cells with no conversion anywhere. That is why [`MissileStats::range_ticks`]
//! and [`MissileStats::range`] are the same fact twice.
//!
//! # What is not here
//!
//! Missile classes **4** (catapult debris), **5** (a burning cell) and **7**
//! (the boiling-oil stream) share the same 100-slot array in the original.
//! Debris is modelled as a spent shot that counts down; fire and oil are not,
//! and `l2-sim` has no burning-cell model for them to feed. `docs/battle.md`
//! §0 calls class 7 *"falling men"*; it is boiling oil, and the only spawner in
//! the binary is the oil path.

use crate::facing::facing_from_delta;
use crate::figure::Figure;
use crate::troop::Troop;

/// Sub-cell units a cell is divided into. A missile's position is in these.
/// **[V]** `Missile_Spawn` writes `x << 5` and `Missile_Step` recovers
/// `(x + 8) / 32`.
pub const SUB_CELL: i16 = 32;

/// Sub-steps a missile takes per tick — `g_missileStats[class][2]`, **4 for
/// every weapon class**. One sub-step is one unit of [`SUB_CELL`], so a tick is
/// an eighth of a cell.
pub const SUB_STEPS: i8 = 4;

/// The array bound: `Missile_UpdateAll` runs `for (i = 1; i < 0x65; i++)`, and
/// slot 0 is the "none" value. **A hundred and first arrow is silently
/// dropped**, exactly as an eighty-first figure is.
pub const MAX_MISSILES: usize = 100;

/// **A missile is born a whole cell out from its shooter.**
/// `BattleMan_FireMissile` runs eight `Missile_Step`s on the spot before the
/// missile is ever linked to a cell or drawn — eight ticks, thirty-two
/// sub-steps, one cell — and those eight come out of the range budget. So a
/// shot can already have hit something before anybody sees it.
pub const LAUNCH_STEPS: u32 = 8;

/// Ticks a **blocked** missile keeps trying before it is given up on. Past this
/// the shooter is marked engaged and the arrow is discarded.
pub const BLOCKED_LIMIT: u8 = 0x20;

/// What a hit sets the missile's countdown to. `Missile_UpdateAll` decrements
/// it, so an arrow lives exactly one more tick after impact — which is what
/// makes **one hit per missile** structural rather than a rule anybody wrote.
pub const HIT_TTL: i16 = 2;

/// What a catapult shot striking a wall sets its countdown to, as it becomes
/// class 4 debris.
pub const DEBRIS_TTL: i16 = 0x78;

/// Catapult hits one wall cell absorbs before it collapses. The original counts
/// them in cell byte `+0` and collapses at `> 0x0F`, so the sixteenth hit is
/// the one that lands.
pub const WALL_HITS_PER_COLLAPSE: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    Bow = 1,
    Crossbow = 2,
    Catapult = 3,
}

/// The missile classes that are not weapons — the other three users of the same
/// array. Only [`CLASS_DEBRIS`] is modelled.
pub const CLASS_DEBRIS: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissileStats {
    /// Range in cells. Stored in the original as sub-cell units of 8.
    pub range: u16,
    /// Ticks between shots.
    pub reload: u16,
    /// Base damage before band scaling, elevation and armour.
    pub damage: u16,
}

impl MissileStats {
    /// **The same number as [`MissileStats::range`], in the units the original
    /// stores it in** — eighths of a cell — which is also the missile's tick
    /// budget, because a tick is an eighth of a cell. `g_missileStats[class][0]`
    /// is 120, 64 and 160.
    ///
    /// ```
    /// # use l2_sim::WeaponClass;
    /// assert_eq!(WeaponClass::Bow.stats().range_ticks(), 120);
    /// assert_eq!(WeaponClass::Crossbow.stats().range_ticks(), 64);
    /// assert_eq!(WeaponClass::Catapult.stats().range_ticks(), 160);
    /// ```
    pub fn range_ticks(&self) -> i16 {
        (self.range * 8) as i16
    }
}

/// **When a shot's target is chosen.** `BattleMan_FireMissile` acquires ten
/// ticks before the reload expires, so a figure that loses its target in the
/// last ten ticks of a cycle does not shoot at all.
pub const ACQUIRE_LEAD: u16 = 10;

/// What `BattleMan_FireMissile` resets the reload counter to when
/// `Missile_FindTarget` finds nothing: the figure does **not** wait a full
/// cycle before looking again.
pub const NO_TARGET_RESET: u16 = 4;

impl WeaponClass {
    pub fn stats(self) -> MissileStats {
        match self {
            WeaponClass::Bow => MissileStats { range: 15, reload: 50, damage: 50 },
            WeaponClass::Crossbow => MissileStats { range: 8, reload: 100, damage: 200 },
            WeaponClass::Catapult => MissileStats { range: 20, reload: 100, damage: 200 },
        }
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    /// Whether a missile of this class can hurt a **figure**.
    ///
    /// `Missile_Step`'s hit test is gated on `class < 3`, so a catapult shot
    /// passing straight through a crowd does nothing to the men in it at all.
    /// A catapult is a wall-breaker and only a wall-breaker. **[V]**
    pub fn hits_men(self) -> bool {
        (self as u8) < 3
    }

    /// Which troops carry a missile weapon. Everything else never enters the
    /// firing state at all.
    pub fn for_troop(troop: Troop) -> Option<WeaponClass> {
        match troop {
            Troop::Archers => Some(WeaponClass::Bow),
            Troop::Crossbowmen => Some(WeaponClass::Crossbow),
            Troop::Catapults => Some(WeaponClass::Catapult),
            _ => None,
        }
    }
}

/// **One missile in flight** — the original's `g_missiles` record
/// (`0x0057A100`, stride `0x4C`), reduced to the fields that decide anything.
///
/// The dropped fields are all presentation or dead: the sprite sheet pointer,
/// the sprite frame and base, the per-cell draw list link, the burnt-surface
/// save slot, and `+0x32`, which the original writes and never reads.
///
/// **`owner` is the free-slot marker**, exactly as it is for a unit and a
/// figure: zero means the slot is empty, and [`Missiles::alloc`] finds the
/// lowest such slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Missile {
    /// Realm that fired it. **Zero means a free slot.** The hit test compares
    /// this against a figure's *owner*, not its side, which is why an arrow
    /// passes through a friendly body without being consumed.
    pub owner: u8,
    /// 1 bow, 2 crossbow, 3 catapult, [`CLASS_DEBRIS`] what a catapult shot
    /// becomes when it strikes a wall.
    pub class: u8,
    /// **The firing figure**, not the target. There is no target field on the
    /// record at all — see the module header.
    pub shooter: u16,
    /// Position, in [`SUB_CELL`]ths of a cell.
    pub x: i16,
    pub y: i16,
    /// The impact point, **frozen at launch**. A target that moves is not
    /// followed.
    pub target_x: i16,
    pub target_y: i16,
    /// The cell the missile is over, `(pos + 8) / 32`. The `+ 8` is the
    /// original's: the cell index flips a quarter of a cell late.
    pub cell_x: i16,
    pub cell_y: i16,
    /// Bresenham's remaining counts, in sub-cell units, decremented as the line
    /// is walked. When both reach zero the line is spent and the missile
    /// coasts.
    pub dx: i32,
    pub dy: i32,
    pub err: i32,
    /// 1 when x is the major axis, 2 when y is.
    pub major_axis: u8,
    /// Launch direction, 0…7 — what the missile coasts along once its line is
    /// spent.
    pub dir: u8,
    /// Cell elevation under the shooter. Ground more than one level above this
    /// blocks the shot.
    pub launch_elevation: u8,
    /// Ticks spent blocked. Seeded per shooter so a volley does not all give up
    /// on the same frame.
    pub blocked_ticks: u8,
    /// Sub-steps this tick — [`SUB_STEPS`] normally, 1 for debris.
    pub sub_steps: i8,
    /// Ticks flown, against [`Missile::range_ticks`].
    pub ticks_flown: i16,
    /// The tick budget, which is the range in eighths of a cell.
    pub range_ticks: i16,
    /// Sticky: high ground or a siege engine is in the way.
    pub blocked: bool,
    /// Non-zero **suppresses every impact test**, and the countdown to
    /// retirement. This is what makes one hit per missile structural.
    pub ttl: i16,
    /// Damage, band-scaled and frozen at launch. Elevation, armour and the
    /// engine caps are applied at impact by [`resolve_power`].
    pub power: u16,
}

impl Missile {
    pub fn is_live(&self) -> bool {
        self.owner != 0
    }

    /// `Missile_SetupLine` (`0x00493CB9`) — `|dx|`, `|dy|`, `2 * min − max`,
    /// and the octant snap.
    ///
    /// The snap moves the *direction* only, never the position: a near-vertical
    /// diagonal is drawn as vertical. It matters here because `dir` is what the
    /// missile coasts along once its line is spent.
    fn setup_line(&mut self) {
        let dx = (self.target_x - self.x).unsigned_abs() as i32;
        let dy = (self.target_y - self.y).unsigned_abs() as i32;
        self.dx = dx;
        self.dy = dy;
        // `2 * min − max`, and **0 when the two are equal** — the perfect
        // diagonal, where either major axis walks the same line.
        match dx.cmp(&dy) {
            core::cmp::Ordering::Greater => {
                self.major_axis = 1;
                self.err = 2 * dy - dx;
            }
            core::cmp::Ordering::Less => {
                self.major_axis = 2;
                self.err = 2 * dx - dy;
            }
            core::cmp::Ordering::Equal => {
                self.major_axis = 1;
                self.err = 0;
            }
        }
        // **The octant snap is already done.** `Missile_SetupLine` pulls a
        // diagonal `dir` to the nearer cardinal when one axis is under half the
        // other — and that is the same partition
        // [`crate::facing::facing_from_delta`] applies, which is where `dir`
        // came from. Applying it twice would change nothing; applying it here
        // as well would only invite somebody to change one copy.
    }

    /// `Missile_StepError` (`0x00493B61`) — one Bresenham error update, over the
    /// **remaining** counts rather than the original ones, and one decrement of
    /// the major axis.
    ///
    /// Using the remaining counts is the original's and is not a mistake: they
    /// shrink together, so the slope it re-derives each step is the same slope.
    /// **[I]** on whether the decrement precedes or follows the error update —
    /// the path is identical either way, and nothing else reads the counts.
    fn step_error(&mut self) {
        let (major, minor) =
            if self.major_axis == 2 { (self.dy, self.dx) } else { (self.dx, self.dy) };
        if self.err < 0 {
            self.err += 2 * minor;
        } else {
            self.err += 2 * (minor - major);
        }
        if self.major_axis == 2 {
            self.dy -= 1;
        } else {
            self.dx -= 1;
        }
    }

    /// One unit toward the frozen impact point, on one axis.
    fn step_toward_x(&mut self) {
        match self.x.cmp(&self.target_x) {
            core::cmp::Ordering::Less => self.x += 1,
            core::cmp::Ordering::Greater => self.x -= 1,
            core::cmp::Ordering::Equal => {}
        }
    }

    fn step_toward_y(&mut self) {
        match self.y.cmp(&self.target_y) {
            core::cmp::Ordering::Less => self.y += 1,
            core::cmp::Ordering::Greater => self.y -= 1,
            core::cmp::Ordering::Equal => {}
        }
    }

    /// **The overshoot.** `FUN_00494265`: once the line is spent the missile
    /// carries on one unit a sub-step along its launch direction, until range,
    /// the map edge or somebody else stops it.
    ///
    /// This is the behaviour that makes a miss dangerous, and it is why an
    /// arrow that passes its target keeps travelling instead of vanishing.
    pub fn step_octant(&mut self) {
        let (dx, dy) = crate::facing::FACING_DELTA[(self.dir & 7) as usize];
        self.x += dx as i16;
        self.y += dy as i16;
    }

    /// `Missile_OffMap` — the battlefield bound again, `0 … 79` in either axis.
    pub(crate) fn off_map(&self) -> bool {
        self.cell_x < 0
            || self.cell_y < 0
            || self.cell_x >= crate::terrain::DIM as i16
            || self.cell_y >= crate::terrain::DIM as i16
    }

    /// Recompute the cell from the position. The `+ 8` is the original's, and
    /// it means the cell index changes a quarter of a cell after the geometric
    /// boundary.
    pub(crate) fn resync_cell(&mut self) {
        self.cell_x = (self.x + 8) / SUB_CELL;
        self.cell_y = (self.y + 8) / SUB_CELL;
    }

    /// One sub-step of the line, or of the coast past its end.
    pub(crate) fn sub_step(&mut self) {
        if self.dx + self.dy < 1 {
            self.step_octant();
        } else {
            self.step_error();
            if self.major_axis == 2 {
                self.step_toward_y();
                if self.err >= 0 {
                    self.dx -= 1;
                    self.step_toward_x();
                }
            } else {
                self.step_toward_x();
                if self.err >= 0 {
                    self.dy -= 1;
                    self.step_toward_y();
                }
            }
        }
        self.resync_cell();
    }
}

/// **The missile array** — a hundred fixed slots and no allocation, exactly as
/// the original has it.
///
/// Slot 0 is never used; it is the "none" value. Allocation always restarts the
/// scan at 1 and takes the lowest free slot, which is what makes it
/// deterministic enough for lockstep: two peers with the same state allocate the
/// same index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missiles {
    slots: Vec<Missile>,
}

impl Default for Missiles {
    fn default() -> Self {
        Missiles::new()
    }
}

impl Missiles {
    pub fn new() -> Missiles {
        Missiles { slots: vec![Missile::default(); MAX_MISSILES + 1] }
    }

    /// `Missile_Spawn`'s allocator: the lowest free slot, or `None` once all
    /// hundred are in flight. **The original drops the shot silently**, and so
    /// does this.
    pub fn alloc(&mut self) -> Option<usize> {
        (1..=MAX_MISSILES).find(|&i| !self.slots[i].is_live())
    }

    /// Zero a slot — `FUN_0046EB78`, which clears all `0x4C` bytes.
    pub fn free(&mut self, i: usize) {
        self.slots[i] = Missile::default();
    }

    pub fn get(&self, i: usize) -> &Missile {
        &self.slots[i]
    }

    pub fn get_mut(&mut self, i: usize) -> &mut Missile {
        &mut self.slots[i]
    }

    /// How many are in the air. For tests and for a renderer; nothing branches
    /// on it.
    pub fn live(&self) -> usize {
        self.slots.iter().filter(|m| m.is_live()).count()
    }

    /// Every live missile with its slot, ascending — the order
    /// `Missile_UpdateAll` walks and the only order anything may walk them in.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Missile)> {
        self.slots.iter().enumerate().skip(1).filter(|(_, m)| m.is_live())
    }
}

/// **Launch one missile.** `Missile_Spawn` (`0x0046E767`) writes geometry only;
/// everything else is written by the caller through `g_lastMissile`, which is
/// why this takes the whole record's worth of arguments.
///
/// Returns the slot, or `None` when all hundred are in flight.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    missiles: &mut Missiles,
    owner: u8,
    class: WeaponClass,
    shooter: usize,
    from: (u8, u8),
    to: (u8, u8),
    power: u16,
    launch_elevation: u8,
) -> Option<usize> {
    let slot = missiles.alloc()?;
    let m = missiles.get_mut(slot);
    *m = Missile {
        owner,
        class: class.index(),
        shooter: shooter as u16,
        x: from.0 as i16 * SUB_CELL,
        y: from.1 as i16 * SUB_CELL,
        target_x: to.0 as i16 * SUB_CELL,
        target_y: to.1 as i16 * SUB_CELL,
        cell_x: from.0 as i16,
        cell_y: from.1 as i16,
        dir: facing_from_delta(to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32)
            .unwrap_or(8),
        launch_elevation,
        // `(shooterIndex & 0x10) + 4` — 4 or 20, so a blocked volley gives up
        // raggedly rather than all at once.
        blocked_ticks: ((shooter as u8) & 0x10) + 4,
        sub_steps: SUB_STEPS,
        range_ticks: class.stats().range_ticks(),
        power,
        ..Missile::default()
    };
    m.setup_line();
    Some(slot)
}

/// The strength band's effect on a shot: `×1`, `×4/5`, `×3/4`, `×1/2`.
///
/// `docs/battle.md` §5.3. The melee column of the same table is not wired up —
/// [`crate::melee`] still swings at band 0 — so this is the first place a
/// figure's losses make it weaker.
pub fn band_scaled(damage: u16, band: u8) -> u16 {
    match band {
        0 => damage,
        1 => damage * 4 / 5,
        2 => damage * 3 / 4,
        _ => damage / 2,
    }
}

/// Damage a missile delivers, before it is applied.
///
/// `elevation_delta` is the target's cell elevation minus the shooter's, so a
/// positive value means the target holds the high ground. It is always zero on
/// skirmish maps, which carry no elevation at all.
pub fn resolve_power(
    weapon: WeaponClass,
    band_scaled_power: u16,
    elevation_delta: i32,
    target: &Figure,
    size_class: u16,
) -> u16 {
    let mut power = band_scaled_power as i32;

    // 1. High ground. The human-owner case gets *less* protection, which is
    //    real in the original and unexplained - see docs/battle.md §6.2.
    if target.owner_is_human {
        if elevation_delta > 3 {
            power /= 4;
        } else if elevation_delta > 1 {
            power /= 2;
        }
    } else if elevation_delta > 3 {
        power /= 5;
    } else if elevation_delta > 1 {
        power /= 3;
    }

    // 2. Armour, as a flat subtraction. This is why a crossbow bolt still kills
    //    through plate and an arrow largely does not.
    power -= target.armour() as i32;

    // 3. Siege engines are a special case in several directions at once.
    if target.troop.is_siege() {
        match weapon {
            WeaponClass::Crossbow => {
                power /= if target.owner_is_human { 2 } else { 3 };
            }
            WeaponClass::Bow if power < 1 => {
                power = if target.owner_is_human { 6 } else { 4 };
            }
            _ => {}
        }
        let cap = 2 * (size_class as i32 * 5 + if target.owner_is_human { 10 } else { 5 });
        if power > cap {
            power = cap;
        }
    }

    // 4. A hit always lands for something. Nothing is immune.
    power.max(2) as u16
}

/// Apply a resolved missile hit. Returns men killed.
///
/// Unlike melee, leftover damage is thrown away, and a single hit can take up
/// to four men at once.
pub fn apply_hit(target: &mut Figure, power: u16) -> u16 {
    let threshold = target.hits_per_casualty;
    target.hits = target.hits.saturating_add(power);
    if target.hits < threshold {
        return 0;
    }
    let multiples = (target.hits / threshold).min(4);
    let killed = multiples.min(target.men);
    target.men -= killed;
    // Discarded, not carried. The melee path keeps its remainder.
    target.hits = 0;
    if target.men == 0 {
        target.state = crate::figure::State::Dead;
        target.opponent = None;
    }
    killed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::SIDE_B;

    /// The manual states this in one sentence: archers have greater range and a
    /// faster rate of fire than crossbowmen, but do less damage per shot. Three
    /// independent agreements from a source nobody here wrote.
    #[test]
    fn bows_out_range_and_out_shoot_crossbows_but_hit_softer() {
        let bow = WeaponClass::Bow.stats();
        let xbow = WeaponClass::Crossbow.stats();
        assert!(bow.range > xbow.range, "greater range");
        assert!(bow.reload < xbow.reload, "faster rate of fire");
        assert!(bow.damage < xbow.damage, "less damage per shot");
    }

    #[test]
    fn only_three_troop_types_carry_a_missile_weapon() {
        assert_eq!(WeaponClass::for_troop(Troop::Archers), Some(WeaponClass::Bow));
        assert_eq!(WeaponClass::for_troop(Troop::Crossbowmen), Some(WeaponClass::Crossbow));
        assert_eq!(WeaponClass::for_troop(Troop::Catapults), Some(WeaponClass::Catapult));
        for t in [Troop::Swordsmen, Troop::Knights, Troop::Pikemen, Troop::Peasants] {
            assert_eq!(WeaponClass::for_troop(t), None);
        }
    }

    /// Armour subtracts flatly, which is the whole reason crossbows are the
    /// anti-armour weapon: an arrow mostly bounces off plate, a bolt does not.
    #[test]
    fn armour_is_a_flat_subtraction_not_a_ratio() {
        let armoured = Figure::new(Troop::Swordsmen, SIDE_B, 4); // armour 35
        let soft = Figure::new(Troop::Peasants, SIDE_B, 4); // armour 0

        let arrow_vs_plate = resolve_power(WeaponClass::Bow, 50, 0, &armoured, 0);
        let arrow_vs_soft = resolve_power(WeaponClass::Bow, 50, 0, &soft, 0);
        let bolt_vs_plate = resolve_power(WeaponClass::Crossbow, 200, 0, &armoured, 0);

        assert_eq!(arrow_vs_plate, 15, "50 - 35");
        assert_eq!(arrow_vs_soft, 50, "nothing subtracted");
        assert_eq!(bolt_vs_plate, 165, "200 - 35");
        // An arrow needs about seven hits to kill one swordsman; a bolt under two.
        assert!(100 / arrow_vs_plate >= 6);
        assert!(100 / bolt_vs_plate < 1);
    }

    #[test]
    fn a_hit_always_lands_for_at_least_two() {
        // Oil has the heaviest armour in the game, far above an arrow's damage.
        let mut oil = Figure::new(Troop::Oil, SIDE_B, 4);
        oil.owner_is_human = false;
        let p = resolve_power(WeaponClass::Bow, 50, 0, &oil, 0);
        assert!(p >= 2, "nothing is immune, got {p}");
    }

    #[test]
    fn high_ground_protects_the_target_and_only_against_missiles() {
        let target = Figure::new(Troop::Peasants, SIDE_B, 4);
        let level = resolve_power(WeaponClass::Crossbow, 200, 0, &target, 0);
        let slight = resolve_power(WeaponClass::Crossbow, 200, 2, &target, 0);
        let commanding = resolve_power(WeaponClass::Crossbow, 200, 4, &target, 0);
        assert!(commanding < slight && slight < level);
        assert_eq!(commanding, 200 / 5, "a fifth from four levels up");
    }

    /// The asymmetry documented but not explained: a human-owned figure gets
    /// *less* protection from high ground.
    #[test]
    fn human_owned_targets_get_less_high_ground_protection() {
        let mut ai = Figure::new(Troop::Peasants, SIDE_B, 4);
        ai.owner_is_human = false;
        let mut human = ai;
        human.owner_is_human = true;

        let ai_power = resolve_power(WeaponClass::Crossbow, 200, 4, &ai, 0);
        let human_power = resolve_power(WeaponClass::Crossbow, 200, 4, &human, 0);
        assert!(human_power > ai_power, "human takes more: {human_power} vs {ai_power}");
    }

    #[test]
    fn a_missile_kill_discards_the_remainder_but_melee_carries_it() {
        let mut by_missile = Figure::new(Troop::Peasants, SIDE_B, 4);
        apply_hit(&mut by_missile, 150);
        assert_eq!(by_missile.men, 3);
        assert_eq!(by_missile.hits, 0, "missile throws the extra 50 away");

        let mut by_melee = Figure::new(Troop::Peasants, SIDE_B, 4);
        by_melee.take_hits(150);
        assert_eq!(by_melee.men, 3);
        assert_eq!(by_melee.hits, 50, "melee carries the extra 50 forward");
    }

    #[test]
    fn one_missile_can_take_up_to_four_men_but_no_more() {
        let mut f = Figure::new(Troop::Peasants, SIDE_B, 8);
        assert_eq!(apply_hit(&mut f, 1000), 4, "capped at four");
        assert_eq!(f.men, 4);
    }

    #[test]
    fn a_missile_cannot_kill_more_men_than_are_present() {
        let mut f = Figure::new(Troop::Peasants, SIDE_B, 2);
        assert_eq!(apply_hit(&mut f, 1000), 2);
        assert_eq!(f.men, 0);
        assert_eq!(f.state, crate::figure::State::Dead);
    }
}
