//! **Everything that moves on the campaign map** — `docs/armies.md`.
//!
//! The headline of that document is the design of this module: *an army is a
//! unit*. Armies, revolting peasants, merchants and supply transports are one
//! 151-record array, `g_units` (`0x0052F0B0`, stride `0x1A4`), told apart by a
//! type byte at `+0x08`. Four separate Rust types would be four separate
//! answers to "how many units are standing in this county"
//! only has one — [`Units::recount_county_troops`] walks types **1 and 2**
//! together, [`wages_for_realm`] walks type 1 alone
//! walks all four.
//!
//! As everywhere else in this crate the *semantics* are reproduced and the byte
//! layout is not; each field carries the offset it was identified at so a
//! differential test against the original can find it again.
//!
//! # What is here and what is next door
//!
//! * this module — the record, the array
//!   record arithmetic: merging, desertion, destruction, the strength score,
//! and the two county/realm rollups.
//! * [`crate::map`] — the campaign map the units stand on
//!   cost of every tile.
//! * [`crate::movement`] — the pathfinder and the stepper
//!   step does to a field or a resource site.
//! * [`crate::levy`] — raising an army out of a county's people.
//! * [`crate::mercenary`] — the twelve bands and their walk.
//!
//! # Determinism
//!
//! The array is fixed-size and walked by ascending index everywhere
//! the original's is. Nothing here allocates on a decision, branches on a
//! pointer or iterates a hash (`docs/netcode.md` §3).

mod army;
pub use army::*;

use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

/// `g_units` is **151 records and slot 0 is never a unit**: every loop in the
/// original runs `for (i = 1; i < 0x97; i++)`.
///
/// `[V]` — `g_saveBlocks` row 5 is 151 × 0x1A4 = 63,420 bytes.
pub const MAX_UNITS: usize = 151;

/// The highest usable slot, 150.
pub const MAX_UNIT_ID: usize = MAX_UNITS - 1;

/// The seven troop types a **campaign** unit record carries.
///
/// The record's array is eleven wide — `Battle_RaiseSide` walks eleven types
/// and `Army_PrepareForBattle` fills 7…10 from the siege records immediately
/// before a battle — but the campaign only ever writes the first seven
/// `+0x182`, where the siege records begin, is exactly where the eleven end.
/// Siege engines are out of this module's scope, so seven is what is stored.
pub const TROOP_TYPES: usize = 7;

/// The path array on a unit record: 150 `(x, y)` pairs at `+0x1D`.
///
/// `[V]` and the arithmetic closes: `0x1D + 150 * 2 = 0x149`
/// the next offset anything in the binary references. The 150 is the loop bound
/// in `Path_CopyToUnit` (`0x004707BE`).
pub const MAX_PATH: usize = 150;

/// The twenty-four army-name slots at realm `+0x2D`, one counter each.
/// `L2.eng` groups 94…98 hold twenty-four names a lord.
pub const ARMY_NAME_SLOTS: usize = 24;

/// The three army sprite banks `Army_Tick` picks between, by
/// [`Unit::size_class`]. Twenty-four frames apart, which is eight facings times
/// three walk frames. `[V]`
pub const SPRITE_BANKS: [usize; 3] = [0x48, 0x60, 0x78];

/// The revolting-peasants bank, one step past the last army bank. `[V]` — the
/// twenty-four frames at `0x90` are the last of `Sprite1a.pl8`'s 168.
pub const MOB_SPRITE_BANK: usize = 0x90;

/// `g_unitWalkFrames` (`0x004D6A78`), read out of `Lords2.exe`: four `i32`,
/// `0, 1, 2, 1` — a three-frame walk played there and back.
pub const UNIT_WALK_FRAMES: [usize; 4] = [0, 1, 2, 1];

/// `g_merchantWalkFrames` (`0x004D6AB8`): six `i32`, `0 … 5` — a plain
/// six-frame cycle, which is what makes a merchant's facing block six frames
/// wide where an army's is three.
pub const MERCHANT_WALK_FRAMES: [usize; 6] = [0, 1, 2, 3, 4, 5];

/// The kind of unit a record holds — the type byte at `+0x08`.
///
/// `[V]` — `L2.eng` group 31 names all four, and `g_unitTickTable`
/// (`0x004D6A50`) has one handler each. **Slot 5 of that table is NULL** while
/// the dispatcher accepts types up to 5
/// Nothing spawns one, and nothing here can: the type is an enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum UnitKind {
    Army = 1,
    PeasantMob = 2,
    Merchant = 3,
    Transport = 4,
}

impl UnitKind {
    pub fn from_byte(b: u8) -> Option<UnitKind> {
        match b {
            1 => Some(UnitKind::Army),
            2 => Some(UnitKind::PeasantMob),
            3 => Some(UnitKind::Merchant),
            4 => Some(UnitKind::Transport),
            _ => None,
        }
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Merchants and transports are non-combatants: `Unit_EnterOccupiedTile`
    /// returns immediately for them, `Unit_CrossField` and `Unit_TrampleTile`
    /// both skip them, and neither is counted as troops in a county.
    pub fn is_combatant(self) -> bool {
        matches!(self, UnitKind::Army | UnitKind::PeasantMob)
    }

    /// `Army_Tick` (`0x0046521F`) writes **15** to `+0x154` every tick; the
    /// other three handlers write **10**. `[V]`, four functions.
    pub fn move_allowance(self) -> i32 {
        match self {
            UnitKind::Army => crate::tables::MOVE_ALLOWANCE_ARMY,
            _ => crate::tables::MOVE_ALLOWANCE_OTHER,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UnitKind::Army => "Army",
            UnitKind::PeasantMob => "Revolting peasants",
            UnitKind::Merchant => "Merchant",
            UnitKind::Transport => "Transport",
        }
    }
}

/// The seven campaign troop types, in record order.
///
/// **`[V]`, seven for seven from two sources that know nothing about each
/// other.** `L2.eng` group 8 at `type * 2 + 52` names them — *Peasant,
/// Crossbowman, Maceman, Swordsman, Pikeman, Archer, Knight* — and
/// `docs/kingdom.md` §7.4's [`crate::tables::WEAPON_NAMES`] order is *crossbow,
/// mace, sword, pike, bow, armour*, which lands one for one on types 1…6 with
/// the unequipped levy left over as type 0. A knight is a man in mail.
///
/// The same seven are the first seven of `l2_sim::Troop`'s eleven; this crate
/// may not depend on `l2-sim`, so the correspondence is stated
/// shared. Types 7…10 (catapult, siege tower, ram, oil) exist only inside a
/// battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum TroopType {
    Peasant = 0,
    Crossbowman = 1,
    Maceman = 2,
    Swordsman = 3,
    Pikeman = 4,
    Archer = 5,
    Knight = 6,
}

/// The seven types in record order, for walking `troops`.
pub const ALL_TROOP_TYPES: [TroopType; TROOP_TYPES] = [
    TroopType::Peasant,
    TroopType::Crossbowman,
    TroopType::Maceman,
    TroopType::Swordsman,
    TroopType::Pikeman,
    TroopType::Archer,
    TroopType::Knight,
];

impl TroopType {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<TroopType> {
        ALL_TROOP_TYPES.get(i).copied()
    }

    /// The weapon slot this type is equipped from, or `None` for the peasant,
    /// who carries whatever is to hand. Slot `t - 1` of
    /// [`crate::tables::WEAPON_NAMES`].
    pub fn weapon_slot(self) -> Option<usize> {
        match self {
            TroopType::Peasant => None,
            other => Some(other.index() - 1),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TroopType::Peasant => "Peasant",
            TroopType::Crossbowman => "Crossbowman",
            TroopType::Maceman => "Maceman",
            TroopType::Swordsman => "Swordsman",
            TroopType::Pikeman => "Pikeman",
            TroopType::Archer => "Archer",
            TroopType::Knight => "Knight",
        }
    }
}

/// `g_troopStrengthWeight` (`0x004D6A18`) — what one man of each type is worth
/// to [`Unit::strength_score`]
///
/// `docs/armies.md` §5.1 notes that the mercenary prices do *not* track these:
/// price ÷ weight comes out 2.0, 1.54, 2.1, 1.875, 1.56, 2.5 across the six
/// equipped types, so the prices are hand-authored and these are not a price
/// list.
pub const TROOP_STRENGTH_WEIGHT: [i32; TROOP_TYPES] = [2, 16, 8, 13, 9, 13, 22];

/// The mercenary band riding with an army — record fields `+0x195`, `+0x196`
/// and `+0x197`.
///
/// **A band is atomic**, so it is one field
/// into `troops`: it is raised as a single extra battle unit, it blocks a merge
/// with another army that also carries one, and bankruptcy walks it off in one
/// piece. `docs/armies.md` §5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mercenaries {
    /// `+0x197` — the band id, 1…12. Indexes both the live band table and
    /// `L2.eng` group 16, the nationality.
    pub band: u8,
    /// `+0x195` — 0…6, the same numbering as [`TroopType`].
    pub troop: TroopType,
    /// `+0x196` — **a byte in the original**, and nothing clamps it; the
    /// largest shipped band is 250, so it fits.
    pub men: u8,
}

impl Mercenaries {
    pub fn men(self) -> i32 {
        self.men as i32
    }
}

/// One campaign unit.
///
/// Every field carries the offset in the original's `0x1A4`-byte record it was
/// identified at. Offsets shared with merchants and transports are marked *sh*
/// in `docs/armies.md` §1; two of those carry a different meaning per unit type
/// and are named for the army meaning here, because this crate has no merchant
/// model yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// `+0x00` — realm 1…5. In the original **0 means the slot is free**;
    /// here a free slot is `None` in [`Units`], so this is always a real realm
    /// (or 6, an ownerless unit).
    pub owner: u8,
    /// `+0x01` — a copy of realm `+0x05` taken at creation, not a live read.
    /// It gates a sound, the AI's siege-engine defaults, the post-battle move
    /// penalty, and — the one that matters here — whether trampling a field
    /// costs the trampler any diplomacy at all. See
    /// [`crate::movement::cross_field`].
    pub owner_is_human: bool,
    /// `+0x02` — a copy of realm `+0x0A`, the banner.
    pub shield: u8,
    /// `+0x06` — 1 for armies made by `Army_Create`; suppresses the automatic
    /// re-path at the end of a move.
    pub player_driven: bool,
    /// `+0x08`.
    pub kind: UnitKind,
    /// `+0x09` — 0…7, the direction of the last step.
    pub facing: u8,
    /// `+0x0A`, `+0x0B` — tile coordinates 0…63.
    pub x: u8,
    pub y: u8,
    /// `+0x10` — the county the unit is standing in. `Army_Tick` keeps it
    /// equal to the tile's county byte.
    pub county: u8,
    /// `+0x11` — where it was raised. `L2.eng` 31/9 *"An army from"*.
    pub home_county: u8,
    /// `+0x16`, `+0x17` — the end of the ordered path.
    pub dest: Option<(u8, u8)>,
    /// `+0x1C` and `+0x1D…+0x148` — the remaining steps, **in travel order**.
    ///
    /// The original stores them backwards and counts `pathLen` down to zero;
    /// storing them forwards and popping the front is the same walk with the
    /// index arithmetic left out. Capped at [`MAX_PATH`] because the original's
    /// array is.
    pub path: Vec<(u8, u8)>,
    /// `+0x14C` — 0 idle, 2 moving.
    pub moving: bool,
    /// `+0x14D` — the tile just entered was a road, which is what makes the
    /// step cost 1 instead of 3.
    pub on_road: bool,
    /// `+0x149` — **how far across the current tile the unit is**, 0…16.
    ///
    /// `Unit_StepOnce` (`0x0046634D`) adds [`SUBTILE_STEP_SOLO`] to this on
    /// every tick it is admitted by [`Unit::sub_frame`], and only when it
/// reaches [`SUBTILE_SPAN`] is the next tile entered. It is
    /// **not** a display value: it is the whole of a unit's speed, and without
    /// it a unit crosses one tile per tick. See [`crate::units_tick`].
    pub sub_tile: u8,
    /// `+0x14A` — the divider in front of [`Unit::sub_tile`], reset every time
    /// it admits a tick.
    ///
    /// `Unit_StepOnce` admits one when `sub_frame` exceeds 0 on a road and 3
    /// off one, which is where a road's fourfold speed comes from — the
    /// **cost** difference (1 against 3) is a separate rule and is charged in
    /// [`crate::movement::step`].
    pub sub_frame: u8,
    /// `+0x14B` bit 0 — **the unit is standing on the edge of the next tile**
    /// and the coming tick commits it.
    ///
    /// `Unit_Step`'s loop tests this before the budget check and the waypoint
/// advance
/// tile boundary.
    ///
/// **It starts set.** `Unit_Spawn`
    /// (`0x0046E1B0`) ends with `field_0x14b |= 1` on every unit it creates,
    /// and `Army_Split` (`0x00437FD7`) sets it again on the half it makes — so
    /// the *first* admitted tick of a unit's life commits a tile immediately
    /// and only the tiles after it cost the full crossing. Started clear
    /// instead, every unit in the game spends its first eight (or thirty-two)
    /// ticks standing still, and six unit tests of the driver read that as a
    /// stalled sweep. `docs/decisions.md` **C134**.
    pub at_tile_edge: bool,
    /// `+0x14F` *sh* — index into `L2.eng` group `93 + owner`, 0…23. For a
    /// merchant the same byte is the route number.
    pub name_index: u8,
    /// `+0x150` *sh* — idle, no orders.
    pub needs_destination: bool,
    /// `+0x151` *sh* — the county the current order leads to. It is what makes
/// *"Invasion of"* fire on arrival.
    pub dest_county: u8,
    /// `+0x153` — moves spent this season. The panel prints
    /// `move_allowance - moves_used` as `L2.eng` 31/22 *"moves left."*
    pub moves_used: i32,
    /// `+0x154` — 15 for an army, 10 for the other three. Rewritten every tick
/// by the type's handler, so it is derived; it is here
    /// because the panel subtracts from it.
    pub move_allowance: i32,
    /// `+0x155` — 0…5, drawn as `L2.eng` 31/(27 + value): *healthy*, *ill,
    /// will perish in 4 seasons*, …, *dying, will perish if not fed*.
    pub starvation: i32,
    /// `+0x15C` — this unit's share of its realm's wage bill, `L2.eng` 31/8.
    pub wages: i32,
    /// `+0x164` *sh* — `L2.eng` 31/20 *"Formed"*. For a merchant the same
    /// field is the route cursor.
    pub year_formed: i32,
    /// `+0x166` — `L2.eng` 31/21 *"Morale"*. Copied from the county's
    /// happiness when the army is raised; **nothing was found that changes it
    /// afterwards** (`docs/armies.md` §1.2).
    pub morale: i32,
    /// `+0x168` — the total. Wages, the sprite class, starvation and the 1500
/// cap all read this.
    pub men: i32,
    /// `+0x16C + t*2` — the seven counts, indexed by [`TroopType`].
    pub troops: [i32; TROOP_TYPES],
    /// `+0x195…+0x197`.
    pub mercenaries: Option<Mercenaries>,
    /// `+0x198` — non-zero means this unit is inside that county's castle. A
    /// garrison is excluded from the county troop count and never starves.
    pub garrison_county: u8,
    /// `+0x199` — non-zero means camped outside that county's castle building
    /// engines. [`crate::siege`] is the whole of what it means;
    /// [`Units::recount_county_troops`], [`wages_for_realm`] and [`starve`]
    /// each have to know about it because a besieger forages in the county it
    /// is camped in.
    ///
    /// **It is not `garrison_county` and the two are never both set.**
    /// `siege-sieging.sav` is the position: the besieging army carries
    /// `+0x198 = 0` and `+0x199 = 4`
    /// `+0x198 = 4` and `+0x19A = 5`. `[V]`
    pub besieging_county: u8,
    /// `+0x19A` — on a garrison, the slot of its besieger.
    pub besieged_by: u8,
    /// `+0x182 + e*6` — the three siege-engine build records, indexed by
    /// [`crate::siege::Engine`]. See [`crate::siege`].
    pub engines: [crate::siege::EngineBuild; 3],
    /// `+0x19C` — **seasons until the ordered engines are ready**, the number
    /// the siege-preparation screen prints beside `L2.eng` 83/4 *"Siege will
    /// take"*.
    ///
    /// `ceil(remaining man-seasons / men)`, rewritten by
    /// [`crate::siege::recompute_build_time`] and
    /// [`crate::siege::build_tick`]. Verified against four snapshots of one
    /// siege in `E:\dev\lords2-fixtures`: a 43-man army building one catapult
    /// reads 3, 2, 1, 0 as its work record climbs 86, 129, 172, 200. `[V]`
    pub siege_seasons_left: u8,
    /// `+0x167` — **the county-defence mark**
    /// whether winning a battle also wins the county.
    ///
    /// `Army_AttackCounty` writes it when it settles who defends: **1** for a
    /// defence levied on the spot, **2** for an existing army pressed into the
    /// role. [`crate::battle::return_to_campaign`] reads it to decide whether
    /// the county changes hands, and
    /// [`crate::battle::disband_defence`] (`Defence_Disband`, `0x004ABA5A`)
    /// reads it afterwards: a **1** goes back into the county's people and the
/// unit is destroyed; a **2** has the mark cleared and the army
    /// stays.
    ///
    /// **The 2 is written on the AI branch and not on the human one.** An
    /// existing army defending a *human's* county is never marked at all, so
    /// `Defence_Disband` never touches it — which is the same outcome by a
    /// different route, and is quoted verbatim in [`crate::conquest`].
    ///
    /// A merchant or transport carries a county in the same byte; that is a
    /// different meaning for a different unit type, like `+0x14F` and `+0x164`.
    /// See [`Unit::cargo_county`], which is that meaning and arrived from the
    /// other side — the two readings were traced independently and agree that
    /// the byte is per-type. `docs/armies.md` §8.1. `[V]` — written by one
    /// site, read by three, and `battle-during.sav` slot 6 carries a 1.
    pub defence_mark: u8,
    /// `+0x167` on a **transport** — where the load is going.
    ///
    /// Not the same field as [`Unit::dest_county`], which is where the *current
    /// path* ends and changes every leg. This one is set once when the
    /// transport is created and two functions read it: the phase-3 re-target
    /// (`FUN_00429418`) points the transport at this county's anchor every
    /// single turn, and `Transport_Deliver` (`0x00429436`) unloads only when
    /// `cargo_county == county` and then destroys the unit.
    ///
    /// > **The same byte, and [`Unit::defence_mark`] is the other meaning.**
    /// > The two were traced independently — one from
    /// > `Army_AttackCounty` and `Battle_ReturnToCampaign`, one from the
    /// > phase-3 transport re-target — and they agree that `+0x167` is a
    /// > per-type reuse of the kind `docs/armies.md` §1 marks *sh*. It is the
    /// > third such byte; `+0x14F` and `+0x164` are the two that document
    /// > already lists, and §1.5 has `+0x167` among the offsets *"not
    /// > traced"*. It is traced twice over now, once per type.
    /// >
/// > **Two fields**, because we are not byte-compatible
    /// > with the original's record and nothing is gained by aliasing them: an
    /// > army has no cargo and a transport is never a county's defence, so
    /// > keeping them apart means no code can read the wrong one. The cost is
    /// > one byte per unit in the save.
    pub cargo_county: u8,
    /// `+0x1A` — **the mission byte**: what this unit is currently trying to
    /// do
    ///
    /// [`crate::ai_army::Mission`] is the enumeration and carries what each
    /// value means; the field is kept as the raw byte because the original's
    /// dispatcher has an `else` arm that rewrites any unrecognised value to
    /// [`crate::ai_army::Mission::SEEK_ENEMY`], and a Rust enum would have
    /// nowhere to put the value that provoked it.
    ///
/// A human's army is steered by
    /// [`Unit::dest`] and [`Unit::path`]; this byte is only ever read for a
    /// unit whose realm the AI is driving. It is written by five sites in the
    /// original, all of them AI, and by one that is not — joining a castle
    /// garrison sets it to [`crate::ai_army::Mission::GARRISON`] whoever
    /// ordered it. `docs/records.json` does not carry `+0x1A` at all.
    pub mission: u8,
    /// `+0x19B` — **the county the mission is about**, where that is not the
    /// same as the county the current path ends in.
    ///
    /// Written by `FUN_0049FDA5` from the realm's [`Realm::target_county`] —
    /// the county an ally has asked this realm to march on. The distinction
    /// matters for [`crate::ai_army::Mission::ASSIST_ALLY`]: the army's
    /// [`Unit::dest_county`] is where it is walking *now*, which may be an
    /// intermediate friendly county, and this is what it was sent to do.
    pub mission_county: u8,
}

/// The 151-slot array
///
/// Slot 0 is never a unit. A free slot is `None`
/// so "is this slot in use" cannot be asked two
/// different ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Units {
    slots: [Option<Unit>; MAX_UNITS],
}

impl Default for Units {
    fn default() -> Self {
        Units::new()
    }
}

/// The twenty-four name counters a realm keeps at `+0x2D`, one per name slot.
///
/// `Army_PickName` (`0x004A9F72`) picks the **first** slot holding the lowest
/// count and adds 2 to it
/// used — and, because [`destroy`] gives back only 1, the counters drift
/// upwards over a long game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmyNames {
    counters: [[u8; ARMY_NAME_SLOTS]; MAX_REALMS],
}

impl Default for ArmyNames {
    fn default() -> Self {
        ArmyNames::new()
    }
}

impl ArmyNames {
    pub fn new() -> ArmyNames {
        ArmyNames { counters: [[0; ARMY_NAME_SLOTS]; MAX_REALMS] }
    }

    pub fn counters(&self, realm: u8) -> &[u8; ARMY_NAME_SLOTS] {
        &self.counters[(realm as usize).min(MAX_REALMS - 1)]
    }

    /// Restore one realm's row, for [`crate::save`].
    pub fn set_counters(&mut self, realm: u8, row: [u8; ARMY_NAME_SLOTS]) {
        self.counters[(realm as usize).min(MAX_REALMS - 1)] = row;
    }

    /// `Army_PickName`. Realms outside 1…5 get slot 0 without touching a
    /// counter, which is the original's own guard.
    pub fn pick(&mut self, realm: u8) -> u8 {
        if realm < 1 || realm as usize >= MAX_REALMS {
            return 0;
        }
        let row = &mut self.counters[realm as usize];
        let mut best = 0usize;
        // Strictly less than
        // choice is deterministic.
        for slot in 1..ARMY_NAME_SLOTS {
            if row[slot] < row[best] {
                best = slot;
            }
        }
        row[best] = row[best].saturating_add(2);
        best as u8
    }

    /// The one the destroyed army gives back. See [`destroy`].
    pub fn release(&mut self, realm: u8, name: u8) {
        if realm < 1 || realm as usize >= MAX_REALMS {
            return;
        }
        let slot = (name as usize).min(ARMY_NAME_SLOTS - 1);
        let row = &mut self.counters[realm as usize];
        row[slot] = row[slot].saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Tables;

    const T: &Tables = &Tables::DEFAULT;

    fn army(owner: u8, men: i32) -> Unit {
        let mut u = Unit::new(UnitKind::Army, owner, 10, 10);
        u.men = men;
        u.troops[TroopType::Peasant.index()] = men;
        u
    }

    #[test]
    fn slot_zero_is_never_a_unit_and_the_array_holds_a_hundred_and_fifty() {
        let mut units = Units::new();
        assert_eq!(units.free_slot(), Some(1), "the first free slot is 1, not 0");
        for i in 1..=MAX_UNIT_ID {
            assert_eq!(units.spawn(army(1, 50)), Some(i));
        }
        assert_eq!(units.free_slot(), None);
        assert_eq!(units.spawn(army(1, 50)), None, "the 151st is refused, not grown");
        assert_eq!(units.len(), MAX_UNIT_ID);
    }

    #[test]
    fn a_freed_slot_is_reused_by_the_next_spawn() {
        let mut units = Units::new();
        let a = units.spawn(army(1, 50)).unwrap();
        let b = units.spawn(army(1, 60)).unwrap();
        units.remove(a);
        assert_eq!(units.spawn(army(2, 70)), Some(a), "the lowest free slot wins");
        assert!(units.get(b).is_some());
    }

    /// The four types and the two budgets
    /// `docs/armies.md` §0's movement row.
    #[test]
    fn an_army_gets_fifteen_moves_and_everything_else_gets_ten() {
        assert_eq!(UnitKind::Army.move_allowance(), 15);
        for k in [UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
            assert_eq!(k.move_allowance(), 10, "{}", k.name());
        }
        for b in 1..=4u8 {
            assert_eq!(UnitKind::from_byte(b).map(UnitKind::byte), Some(b));
        }
        assert_eq!(UnitKind::from_byte(0), None);
        assert_eq!(UnitKind::from_byte(5), None, "the NULL tick handler is unreachable");
    }

    #[test]
    fn only_armies_and_mobs_are_combatants() {
        assert!(UnitKind::Army.is_combatant());
        assert!(UnitKind::PeasantMob.is_combatant());
        assert!(!UnitKind::Merchant.is_combatant());
        assert!(!UnitKind::Transport.is_combatant());
    }

    /// The seven troop types line up with the six weapons plus the unequipped
    /// levy — `docs/armies.md` §6.2's cross-check, as an assertion.
    #[test]
    fn a_knight_is_a_man_in_armour_and_a_peasant_carries_nothing() {
        assert_eq!(TroopType::Peasant.weapon_slot(), None);
        for t in ALL_TROOP_TYPES.iter().skip(1) {
            let slot = t.weapon_slot().expect("every equipped type has a weapon");
            assert!(slot < crate::tables::WEAPON_TYPE_COUNT);
        }
        assert_eq!(TroopType::Knight.weapon_slot(), Some(5));
        assert_eq!(crate::tables::WEAPON_NAMES[5], "Armour");
        assert_eq!(TroopType::Crossbowman.weapon_slot(), Some(0));
        assert_eq!(crate::tables::WEAPON_NAMES[0], "Crossbow");
    }

    /// The correction this module makes to `docs/armies.md` §7: an empty army
    /// scores 1, not 0
    #[test]
    fn an_empty_army_scores_one_and_a_single_peasant_scores_twenty_two() {
        let empty = Unit::new(UnitKind::Army, 1, 0, 0);
        assert_eq!(empty.strength_score(), 1);

        let mut one = Unit::new(UnitKind::Army, 1, 0, 0);
        one.troops[TroopType::Peasant.index()] = 1;
        one.men = 1;
        assert_eq!(one.strength_score(), 2 + STRENGTH_SCORE_BONUS);
    }

    #[test]
    fn the_strength_score_weights_each_type_and_adds_the_band() {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.troops[TroopType::Knight.index()] = 10;
        u.men = 10;
        assert_eq!(u.strength_score(), 10 * 22 + STRENGTH_SCORE_BONUS);

        u.mercenaries = Some(Mercenaries { band: 11, troop: TroopType::Knight, men: 50 });
        u.men += 50;
        assert_eq!(u.strength_score(), (10 + 50) * 22 + STRENGTH_SCORE_BONUS);
    }

    /// `Army_Desert` takes a tenth of every count that *exceeds* ten, so an
    /// army of small detachments stops shrinking instead of dying out.
    #[test]
    fn desertion_skips_any_troop_type_of_ten_or_fewer() {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        u.troops = [100, 11, 10, 9, 0, 250, 1];
        u.men = u.troops.iter().sum();
        let before = u.men;

        let lost = u.desert();
        assert_eq!(u.troops, [90, 10, 10, 9, 0, 225, 1]);
        assert_eq!(lost, 10 + 1 + 25);
        assert_eq!(u.men, before - lost);

        // And a second pass takes nothing more from the ones already at the
        // floor.
        let mut small = Unit::new(UnitKind::Army, 1, 0, 0);
        small.troops = [10; TROOP_TYPES];
        small.men = 70;
        assert_eq!(small.desert(), 0);
        assert_eq!(small.men, 70, "seventy men in sevens never desert at all");
    }

    #[test]
    fn the_sprite_class_breaks_at_three_hundred_and_one_and_six_hundred_and_one() {
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        for (men, class) in [(1, 0), (300, 0), (301, 1), (600, 1), (601, 2), (1500, 2)] {
            u.men = men;
            assert_eq!(u.size_class(), class, "{men} men");
        }
    }

    // --- combining ---------------------------------------------------------

    #[test]
    fn two_armies_totalling_fifteen_hundred_merge_and_fifteen_hundred_and_one_does_not() {
        let mut units = Units::new();
        let a = units.spawn(army(1, 750)).unwrap();
        let b = units.spawn(army(1, 750)).unwrap();
        assert_eq!(combine(&mut units, a, b), Ok(1500));
        assert!(units.get(b).is_none(), "the absorbed army is gone");
        assert_eq!(units.get(a).unwrap().men, 1500);

        let mut units = Units::new();
        let a = units.spawn(army(1, 751)).unwrap();
        let b = units.spawn(army(1, 750)).unwrap();
        assert_eq!(combine(&mut units, a, b), Err(CombineRefusal::TooMany));
        assert!(units.get(b).is_some(), "a refused merge leaves both armies");
    }

    /// `L2.eng` 167: *"The mercenaries in these armies will not fight
    /// together."*
    #[test]
    fn two_armies_both_carrying_mercenaries_refuse_to_merge() {
        let band = Mercenaries { band: 9, troop: TroopType::Maceman, men: 150 };
        let mut units = Units::new();
        let mut one = army(1, 200);
        one.mercenaries = Some(band);
        let mut two = army(1, 200);
        two.mercenaries = Some(band);
        let a = units.spawn(one).unwrap();
        let b = units.spawn(two).unwrap();
        assert_eq!(combine(&mut units, a, b), Err(CombineRefusal::TwoMercenaryBands));

        // One band between them is fine, and it travels with the merge.
        units.get_mut(b).unwrap().mercenaries = None;
        assert!(combine(&mut units, a, b).is_ok());
        assert_eq!(units.get(a).unwrap().mercenaries, Some(band));
    }

    /// The correction to `docs/armies.md` §2.7: the merge takes the **higher**
    /// of the two move counts, in both directions, so it never refunds
    /// movement.
    #[test]
    fn a_merge_takes_the_higher_of_the_two_move_counts_in_both_directions() {
        for (a_used, b_used) in [(12, 2), (2, 12)] {
            let mut units = Units::new();
            let mut one = army(1, 100);
            one.moves_used = a_used;
            let mut two = army(1, 100);
            two.moves_used = b_used;
            let a = units.spawn(one).unwrap();
            let b = units.spawn(two).unwrap();
            assert!(combine(&mut units, a, b).is_ok());
            assert_eq!(
                units.get(a).unwrap().moves_used,
                12,
                "merging {a_used} and {b_used} must leave the army spent"
            );
        }
    }

    #[test]
    fn a_merge_sums_the_seven_troop_counts() {
        let mut units = Units::new();
        let mut one = Unit::new(UnitKind::Army, 1, 0, 0);
        one.troops = [10, 20, 30, 40, 50, 60, 70];
        one.men = one.troops.iter().sum();
        let mut two = Unit::new(UnitKind::Army, 1, 0, 0);
        two.troops = [1, 2, 3, 4, 5, 6, 7];
        two.men = two.troops.iter().sum();
        let a = units.spawn(one).unwrap();
        let b = units.spawn(two).unwrap();
        assert!(combine(&mut units, a, b).is_ok());
        assert_eq!(units.get(a).unwrap().troops, [11, 22, 33, 44, 55, 66, 77]);
        assert_eq!(units.get(a).unwrap().men, units.get(a).unwrap().troop_total());
    }

    // --- names -------------------------------------------------------------

    /// Twenty-four names, `+2` a use
    /// repeating.
    #[test]
    fn a_realm_uses_every_name_before_repeating_one() {
        let mut names = ArmyNames::new();
        let mut seen = Vec::new();
        for _ in 0..ARMY_NAME_SLOTS {
            seen.push(names.pick(1));
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), ARMY_NAME_SLOTS, "all twenty-four before any repeat");
        assert_eq!(names.pick(1), 0, "and then round again from the first");
    }

    /// The correction: `Army_PickName` adds 2 and `Army_Destroy` gives back 1.
    #[test]
    fn a_destroyed_army_gives_back_only_half_of_what_its_name_cost() {
        let mut names = ArmyNames::new();
        let n = names.pick(1);
        assert_eq!(names.counters(1)[n as usize], 2);
        names.release(1, n);
        assert_eq!(names.counters(1)[n as usize], 1, "not back to zero");
        // So the next pick avoids it, even though nothing is using it.
        assert_ne!(names.pick(1), n);
    }

    #[test]
    fn a_realm_outside_one_to_five_never_touches_a_counter() {
        let mut names = ArmyNames::new();
        assert_eq!(names.pick(0), 0);
        assert_eq!(names.pick(9), 0);
        assert_eq!(names.counters(0), &[0; ARMY_NAME_SLOTS]);
    }

    // --- the two rollups ---------------------------------------------------

    fn kingdom_bits() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let counties = core::array::from_fn(|_| County::new());
        let realms = core::array::from_fn(|_| Realm::new());
        (counties, realms)
    }

    #[test]
    fn the_troop_recount_splits_friendly_from_enemy_by_the_countys_owner() {
        let (mut counties, mut realms) = kingdom_bits();
        counties[1].owner = 1;
        counties[2].owner = 2;
        realms[1].in_play = true;
        realms[2].in_play = true;

        let mut units = Units::new();
        let mut mine = army(1, 300);
        mine.county = 1;
        let mut invading = army(1, 200);
        invading.county = 2;
        units.spawn(mine);
        units.spawn(invading);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!((counties[1].friendly_troops, counties[1].enemy_troops), (300, 0));
        assert_eq!((counties[2].friendly_troops, counties[2].enemy_troops), (0, 200));
    }

    /// An ally's army eats as a friendly. Realm `+0x81` read as an alliance
    /// flag is `[I]` — this is the only rule that reads it that way.
    #[test]
    fn an_allys_army_counts_as_friendly() {
        let (mut counties, mut realms) = kingdom_bits();
        counties[1].owner = 2;
        realms[1].ally = 2;

        let mut units = Units::new();
        let mut u = army(1, 400);
        u.county = 1;
        units.spawn(u);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!(counties[1].friendly_troops, 400);
        assert_eq!(counties[1].enemy_troops, 0);
    }

    /// A garrison is off the books entirely, and a besieger is not.
    #[test]
    fn a_garrison_is_excluded_from_the_recount_but_a_besieger_is_not() {
        let (mut counties, realms) = kingdom_bits();
        counties[1].owner = 1;

        let mut units = Units::new();
        let mut inside = army(1, 500);
        inside.county = 1;
        inside.garrison_county = 1;
        let mut outside = army(2, 300);
        outside.county = 1;
        outside.besieging_county = 1;
        units.spawn(inside);
        units.spawn(outside);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!(counties[1].friendly_troops, 0, "the garrison eats out of the castle");
        assert_eq!(counties[1].enemy_troops, 300, "the besieger forages the county");
    }

    /// The loop tests type 1 **or 2**
    #[test]
    fn revolting_peasants_are_counted_and_merchants_are_not() {
        let (mut counties, realms) = kingdom_bits();
        counties[1].owner = 1;

        let mut units = Units::new();
        let mut mob = Unit::new(UnitKind::PeasantMob, 6, 5, 5);
        mob.county = 1;
        mob.men = 90;
        let mut trader = Unit::new(UnitKind::Merchant, 1, 6, 6);
        trader.county = 1;
        trader.men = 1000;
        units.spawn(mob);
        units.spawn(trader);

        units.recount_county_troops(&mut counties, &realms);
        assert_eq!(counties[1].enemy_troops, 90);
        assert_eq!(counties[1].friendly_troops, 0, "a merchant is not troops");
    }

    #[test]
    fn a_recount_clears_the_counts_it_is_about_to_rebuild() {
        let (mut counties, realms) = kingdom_bits();
        counties[3].friendly_troops = 999;
        counties[3].enemy_troops = 999;
        Units::new().recount_county_troops(&mut counties, &realms);
        assert_eq!((counties[3].friendly_troops, counties[3].enemy_troops), (0, 0));
    }

    /// The wage bill walks type-1 units only, and mercenaries cost `men / 4`
    /// like everyone else because they are part of `men`.
    #[test]
    fn the_wage_bill_covers_armies_only_and_charges_a_quarter_a_man() {
        let (_, mut realms) = kingdom_bits();
        realms[1].is_human = true;
        realms[1].in_play = true;

        let mut units = Units::new();
        units.spawn(army(1, 250));
        units.spawn(army(1, 252));
        let mut mob = Unit::new(UnitKind::PeasantMob, 1, 3, 3);
        mob.men = 1000;
        units.spawn(mob);
        units.spawn(army(2, 400));

        assert_eq!(wages_for_realm(T, &units, &realms, 1, 0), 62 + 63);
        let bill = refresh_wages(T, &mut units, &mut realms, 1, 0);
        assert_eq!(bill, 62 + 63);
        assert_eq!(realms[1].wages, bill);
        assert_eq!(units.get(1).unwrap().wages, 62);
        assert_eq!(units.get(2).unwrap().wages, 63);
        assert_eq!(units.get(3).unwrap().wages, 0, "a mob is not paid");
    }

    #[test]
    fn a_garrisoned_army_is_still_paid() {
        let (_, mut realms) = kingdom_bits();
        realms[1].is_human = true;
        let mut units = Units::new();
        let mut inside = army(1, 400);
        inside.garrison_county = 3;
        units.spawn(inside);
        assert_eq!(refresh_wages(T, &mut units, &mut realms, 1, 0), 100);
    }

    #[test]
    fn resetting_moves_touches_every_slot_regardless_of_type() {
        let mut units = Units::new();
        for kind in [UnitKind::Army, UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport] {
            let mut u = Unit::new(kind, 1, 0, 0);
            u.moves_used = 9;
            u.moving = true;
            units.spawn(u);
        }
        units.reset_moves();
        for (_, u) in units.iter() {
            assert_eq!(u.moves_used, 0, "{}", u.kind.name());
            assert!(!u.moving);
        }
    }

    #[test]
    fn destroying_an_army_frees_the_slot_and_rebuilds_the_wage_bill() {
        let (_, mut realms) = kingdom_bits();
        realms[1].is_human = true;
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = units.spawn(army(1, 400)).unwrap();
        let b = units.spawn(army(1, 800)).unwrap();
        units.get_mut(a).unwrap().name_index = names.pick(1);
        refresh_wages(T, &mut units, &mut realms, 1, 0);
        assert_eq!(realms[1].wages, 100 + 200);

        let gone = destroy(T, &mut units, &mut realms, &mut names, a, 0).unwrap();
        assert_eq!(gone.men, 400);
        assert!(units.get(a).is_none());
        assert_eq!(realms[1].wages, 200, "the bill is recomputed from what is left");
        assert!(units.get(b).is_some());
    }

    #[test]
    fn destroying_a_besieger_clears_the_garrisons_back_pointer() {
        let (_, mut realms) = kingdom_bits();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let garrison = units.spawn(army(1, 200)).unwrap();
        let besieger = units.spawn(army(2, 400)).unwrap();
        units.get_mut(garrison).unwrap().garrison_county = 4;
        units.get_mut(besieger).unwrap().besieging_county = 4;
        units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

        destroy(T, &mut units, &mut realms, &mut names, besieger, 0);
        assert_eq!(units.get(garrison).unwrap().besieged_by, 0);

        // And the other way round: losing the garrison lifts the siege.
        let besieger = units.spawn(army(2, 400)).unwrap();
        units.get_mut(besieger).unwrap().besieging_county = 4;
        destroy(T, &mut units, &mut realms, &mut names, garrison, 0);
        assert_eq!(units.get(besieger).unwrap().besieging_county, 0);
    }

    #[test]
    fn realm_totals_count_armies_and_their_men() {
        let mut units = Units::new();
        units.spawn(army(1, 100));
        units.spawn(army(1, 250));
        units.spawn(army(2, 900));
        let mut mob = Unit::new(UnitKind::PeasantMob, 1, 0, 0);
        mob.men = 40;
        units.spawn(mob);
        assert_eq!(units.realm_totals(1), (2, 350));
        assert_eq!(units.realm_totals(2), (1, 900));
        assert_eq!(units.realm_totals(3), (0, 0));
    }

    #[test]
    fn a_unit_is_found_by_the_tile_it_stands_on() {
        let mut units = Units::new();
        let a = units.spawn(army(1, 100)).unwrap();
        assert_eq!(units.at(10, 10), Some(a));
        assert_eq!(units.at(11, 10), None);
    }
}

