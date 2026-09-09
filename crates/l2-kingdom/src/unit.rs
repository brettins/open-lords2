//! **Everything that moves on the campaign map** — `docs/armies.md`.
//!
//! The headline of that document is the design of this module: *an army is a
//! unit*. Armies, revolting peasants, merchants and supply transports are one
//! 151-record array, `g_units` (`0x0052F0B0`, stride `0x1A4`), told apart by a
//! type byte at `+0x08`. Four separate Rust types would be four separate
//! answers to "how many units are standing in this county", and the original
//! only has one — [`Units::recount_county_troops`] walks types **1 and 2**
//! together, [`wages_for_realm`] walks type 1 alone, and the movement stepper
//! walks all four.
//!
//! As everywhere else in this crate the *semantics* are reproduced and the byte
//! layout is not; each field carries the offset it was identified at so a
//! differential test against the original can find it again.
//!
//! # What is here and what is next door
//!
//! * this module — the record, the array, and the operations that are pure
//!   record arithmetic: merging, desertion, destruction, the strength score,
//!   and the two county/realm rollups.
//! * [`crate::map`] — the campaign map the units stand on, and the movement
//!   cost of every tile.
//! * [`crate::movement`] — the pathfinder and the stepper, including what a
//!   step does to a field or a resource site.
//! * [`crate::levy`] — raising an army out of a county's people.
//! * [`crate::mercenary`] — the twelve bands and their walk.
//!
//! # Determinism
//!
//! The array is fixed-size and walked by ascending index everywhere, exactly as
//! the original's is. Nothing here allocates on a decision, branches on a
//! pointer or iterates a hash (`docs/netcode.md` §3).

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
/// before a battle — but the campaign only ever writes the first seven, and
/// `+0x182`, where the siege records begin, is exactly where the eleven end.
/// Siege engines are out of this module's scope, so seven is what is stored.
pub const TROOP_TYPES: usize = 7;

/// The path array on a unit record: 150 `(x, y)` pairs at `+0x1D`.
///
/// `[V]` and the arithmetic closes: `0x1D + 150 * 2 = 0x149`, and `+0x149` is
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
/// the dispatcher accepts types up to 5, so a type-5 unit would call address 0.
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
/// may not depend on `l2-sim`, so the correspondence is stated rather than
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
/// to [`Unit::strength_score`], which is what the AI and the autocalc compare.
///
/// `docs/armies.md` §5.1 notes that the mercenary prices do *not* track these:
/// price ÷ weight comes out 2.0, 1.54, 2.1, 1.875, 1.56, 2.5 across the six
/// equipped types, so the prices are hand-authored and these are not a price
/// list.
pub const TROOP_STRENGTH_WEIGHT: [i32; TROOP_TYPES] = [2, 16, 8, 13, 9, 13, 22];

/// The mercenary band riding with an army — record fields `+0x195`, `+0x196`
/// and `+0x197`.
///
/// **A band is atomic**, which is why it is one field rather than seven added
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
    /// `+0x14F` *sh* — index into `L2.eng` group `93 + owner`, 0…23. For a
    /// merchant the same byte is the route number.
    pub name_index: u8,
    /// `+0x150` *sh* — idle, no orders.
    pub needs_destination: bool,
    /// `+0x151` *sh* — the county the current order leads to. It is what makes
    /// *"Invasion of"* fire on arrival rather than on passing through.
    pub dest_county: u8,
    /// `+0x153` — moves spent this season. The panel prints
    /// `move_allowance - moves_used` as `L2.eng` 31/22 *"moves left."*
    pub moves_used: i32,
    /// `+0x154` — 15 for an army, 10 for the other three. Rewritten every tick
    /// by the type's handler, so it is derived rather than stored; it is here
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
    /// cap all read this rather than summing [`Unit::troops`].
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
    /// `+0x198 = 0` and `+0x199 = 4`, and the garrison it is besieging carries
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
    /// `+0x167` — **the county-defence mark**, and the field that decides
    /// whether winning a battle also wins the county.
    ///
    /// `Army_AttackCounty` writes it when it settles who defends: **1** for a
    /// defence levied on the spot, **2** for an existing army pressed into the
    /// role. [`crate::battle::return_to_campaign`] reads it to decide whether
    /// the county changes hands, and
    /// [`crate::battle::disband_defence`] (`Defence_Disband`, `0x004ABA5A`)
    /// reads it afterwards: a **1** goes back into the county's people and the
    /// unit is destroyed; a **2** simply has the mark cleared and the army
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
    /// > **Two fields rather than one**, because we are not byte-compatible
    /// > with the original's record and nothing is gained by aliasing them: an
    /// > army has no cargo and a transport is never a county's defence, so
    /// > keeping them apart means no code can read the wrong one. The cost is
    /// > one byte per unit in the save.
    pub cargo_county: u8,
    /// `+0x1A` — **the mission byte**: what this unit is currently trying to
    /// do, and the thing AI turn step 11 dispatches on.
    ///
    /// [`crate::ai_army::Mission`] is the enumeration and carries what each
    /// value means; the field is kept as the raw byte because the original's
    /// dispatcher has an `else` arm that rewrites any unrecognised value to
    /// [`crate::ai_army::Mission::SEEK_ENEMY`], and a Rust enum would have
    /// nowhere to put the value that provoked it.
    ///
    /// **It is not a player's order.** A human's army is steered by
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

impl Unit {
    /// A blank unit of a kind, at a tile, owned by a realm.
    pub fn new(kind: UnitKind, owner: u8, x: u8, y: u8) -> Unit {
        Unit {
            owner,
            owner_is_human: false,
            shield: 0,
            player_driven: false,
            kind,
            facing: 0,
            x,
            y,
            county: 0,
            home_county: 0,
            dest: None,
            path: Vec::new(),
            moving: false,
            on_road: false,
            name_index: 0,
            needs_destination: true,
            dest_county: 0,
            moves_used: 0,
            move_allowance: kind.move_allowance(),
            starvation: 0,
            wages: 0,
            year_formed: 0,
            morale: 0,
            men: 0,
            troops: [0; TROOP_TYPES],
            mercenaries: None,
            garrison_county: 0,
            besieging_county: 0,
            besieged_by: 0,
            engines: [crate::siege::EngineBuild::default(); 3],
            siege_seasons_left: 0,
            defence_mark: 0,
            cargo_county: 0,
            mission: 0,
            mission_county: 0,
        }
    }

    pub fn tile(&self) -> (u8, u8) {
        (self.x, self.y)
    }

    /// Moves the unit has left this season, never negative — a trample can
    /// overshoot the allowance and the panel would otherwise print a negative.
    pub fn moves_left(&self) -> i32 {
        (self.move_allowance - self.moves_used).max(0)
    }

    pub fn is_garrisoned(&self) -> bool {
        self.garrison_county != 0
    }

    /// The men in the band, or 0. Mercenaries are **already inside**
    /// [`Unit::men`] — `Mercenary_Hire` adds them there — so this is not extra
    /// strength, it is the part of the total that leaves in one piece.
    pub fn mercenary_men(&self) -> i32 {
        self.mercenaries.map_or(0, Mercenaries::men)
    }

    /// The sprite bank `Army_Tick` picks: 0 under 301 men, 1 under 601, 2 above.
    ///
    /// `[V]` on the thresholds and the arithmetic — the banks `0x48`, `0x60`
    /// and `0x78` are 24 apart, which is 8 facings × 3 walk frames, and the
    /// instructions are `CMP …, 300` / `CMP …, 600` with `JG`. **`[I]` that the
    /// three banks are literally one, two and three figures**; nobody has
    /// looked at the sheet.
    pub fn size_class(&self) -> usize {
        let [small, medium] = crate::tables::ARMY_SIZE_CLASS_MAX;
        if self.men <= small {
            0
        } else if self.men <= medium {
            1
        } else {
            2
        }
    }

    /// **Which sprite sheet this unit is drawn from** — 0 for
    /// `g_spriteSheetA` (`Sprite1a.pl8` / `Sprite2a.pl8`), 1 for
    /// `g_spriteSheetB`.
    ///
    /// `Map_DrawArmies` (`0x00408438`) makes this choice in one line:
    /// `if (kind == 4) sheet = B;`. A merchant is drawn from **sheet A**, the
    /// same file as the armies — only a transport uses B. `[D]`
    pub fn sprite_sheet(&self) -> usize {
        usize::from(self.kind == UnitKind::Transport)
    }

    /// **The frame the unit's figure is drawn with** — unit record `+0x07`,
    /// which the type's tick handler writes and `Map_DrawArmies` reads
    /// unmodified.
    ///
    /// ```c
    /// Army_Tick     / Mob_Tick:  frame = bank + 3 * ((facing + 1) & 7) + walk[phase];
    /// Merchant_Tick / Transport_Tick: frame =  6 * ((facing + 1) & 7) + phase;
    /// ```
    ///
    /// with `walk` = `g_unitWalkFrames` (`0x004D6A78`) = `[0, 1, 2, 1]` and the
    /// merchant's `g_merchantWalkFrames` (`0x004D6AB8`) = `[0, 1, 2, 3, 4, 5]`.
    /// The bank is [`SPRITE_BANKS`] by [`Unit::size_class`] for an army and
    /// [`MOB_SPRITE_BANK`] for a mob; a merchant and a transport have no bank
    /// at all, because their sheets hold nothing else.
    ///
    /// **The rotation is `facing + 1`, not `facing`** — all four handlers, and
    /// `docs/screens.md` §5 had it as `3*facing`.
    ///
    /// The counts close against the shipped sheets: 8 facings × 3 walk frames =
    /// 24, which is the spacing of the three army banks (`0x48`, `0x60`,
    /// `0x78`) and of the mob's `0x90`; 8 × 6 = 48, which is exactly the
    /// 40 × 32 run at the front of `Sprite1a.pl8` and the whole of
    /// `Sprite1b.pl8`.
    pub fn sprite_frame(&self, phase: usize) -> usize {
        let dir = ((self.facing as usize) + 1) & 7;
        match self.kind {
            UnitKind::Merchant | UnitKind::Transport => {
                dir * MERCHANT_WALK_FRAMES.len() + MERCHANT_WALK_FRAMES[phase % MERCHANT_WALK_FRAMES.len()]
            }
            UnitKind::Army => {
                SPRITE_BANKS[self.size_class()] + dir * 3 + UNIT_WALK_FRAMES[phase % UNIT_WALK_FRAMES.len()]
            }
            UnitKind::PeasantMob => {
                MOB_SPRITE_BANK + dir * 3 + UNIT_WALK_FRAMES[phase % UNIT_WALK_FRAMES.len()]
            }
        }
    }

    /// The `(x, y)` `Map_DrawArmies` adds for this kind before it centres the
    /// figure on the tile's bottom vertex. `[D]`
    pub fn sprite_nudge(&self) -> (i32, i32) {
        match self.kind {
            UnitKind::Army | UnitKind::PeasantMob => (0, -4),
            UnitKind::Merchant | UnitKind::Transport => (-4, -2),
        }
    }

    /// `Army_StrengthScore` (`0x004AB2AA`) — what the AI and the autocalc
    /// compare.
    ///
    /// ```text
    /// score = Σ troops[t] * weight[t]  +  band.men * weight[band.troop]
    /// if (score < 1) score = 1; else score += 20;
    /// ```
    ///
    /// **`docs/armies.md` §7 said *"+ 20 if non-zero"* and missed the floor.**
    /// An army with no men at all scores **1**, not 0, so "stronger than
    /// nothing" is never free: the AI's ratio comparisons cannot divide by
    /// zero, and there is a 20-point step between an empty army and an army of
    /// one peasant (1 versus 22). Corrected in the document. `[D]`
    pub fn strength_score(&self) -> i32 {
        let mut score: i64 = 0;
        for t in ALL_TROOP_TYPES {
            score += self.troops[t.index()] as i64 * TROOP_STRENGTH_WEIGHT[t.index()] as i64;
        }
        if let Some(m) = self.mercenaries {
            score += m.men() as i64 * TROOP_STRENGTH_WEIGHT[m.troop.index()] as i64;
        }
        if score < 1 {
            1
        } else {
            (score + STRENGTH_SCORE_BONUS as i64) as i32
        }
    }

    /// The sum of the seven counts plus the band. **Not** what any rule uses —
    /// every rule reads [`Unit::men`] — but the invariant `Army_Create` and
    /// `Army_Combine` maintain, and therefore worth being able to assert.
    pub fn troop_total(&self) -> i32 {
        self.troops.iter().sum::<i32>() + self.mercenary_men()
    }

    /// `Army_Desert` (`0x004AD16C`) — take [`DESERTION_PCT`] off each of the
    /// seven counts, but **only from a count that exceeds
    /// [`DESERTION_MIN_TROOPS`]**, and subtract the same total from
    /// [`Unit::men`].
    ///
    /// The floor is what stops a starving army from vanishing: ten men of a
    /// type never desert, so an army of seven tens shrinks to nothing slowly
    /// and then stops. The same function is `docs/kingdom.md` §7.4's
    /// bankruptcy penalty, which is why it lives on the record rather than in
    /// [`starve`].
    ///
    /// Returns the men lost.
    ///
    /// [`DESERTION_PCT`]: crate::tables::DESERTION_PCT
    /// [`DESERTION_MIN_TROOPS`]: crate::tables::DESERTION_MIN_TROOPS
    pub fn desert(&mut self) -> i32 {
        let mut lost = 0;
        for t in 0..TROOP_TYPES {
            if self.troops[t] > crate::tables::DESERTION_MIN_TROOPS {
                let gone = crate::math::pct(self.troops[t], crate::tables::DESERTION_PCT);
                self.troops[t] -= gone;
                lost += gone;
            }
        }
        self.men -= lost;
        lost
    }
}

/// `Army_StrengthScore`'s bonus for being an army at all — the `+ 20` on any
/// score that reached 1.
pub const STRENGTH_SCORE_BONUS: i32 = 20;

/// The 151-slot array, and the operations that walk it.
///
/// Slot 0 is never a unit, exactly as in the original. A free slot is `None`
/// rather than an owner byte of 0, so "is this slot in use" cannot be asked two
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

impl Units {
    pub fn new() -> Units {
        Units { slots: core::array::from_fn(|_| None) }
    }

    pub fn get(&self, id: usize) -> Option<&Unit> {
        self.slots.get(id).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut Unit> {
        self.slots.get_mut(id).and_then(Option::as_mut)
    }

    /// `(slot, unit)` for every occupied slot, in ascending slot order —
    /// which is the order every loop in the original walks, and therefore part
    /// of the specification rather than a convenience.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Unit)> {
        self.slots.iter().enumerate().filter_map(|(i, u)| u.as_ref().map(|u| (i, u)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut Unit)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, u)| u.as_mut().map(|u| (i, u)))
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|u| u.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The lowest free slot, 1…150. `Unit_Spawn` (`0x0046E1B0`) takes the
    /// first free one, so which slot a unit lands in is deterministic and
    /// reproducible — and the AI's "first army in this county" is really "the
    /// lowest-numbered one".
    pub fn free_slot(&self) -> Option<usize> {
        (1..MAX_UNITS).find(|&i| self.slots[i].is_none())
    }

    /// Put a unit in the lowest free slot. `None` when all 150 are taken; the
    /// original silently does nothing in that case, and so does the caller.
    pub fn spawn(&mut self, unit: Unit) -> Option<usize> {
        let slot = self.free_slot()?;
        self.slots[slot] = Some(unit);
        Some(slot)
    }

    /// Take a unit out of the array, returning it. The realm's name counter
    /// and wage bill are the caller's to fix up — [`destroy`] does the whole
    /// job.
    pub fn remove(&mut self, id: usize) -> Option<Unit> {
        self.slots.get_mut(id).and_then(Option::take)
    }

    /// Put a unit in a **named** slot, for [`crate::save`] alone.
    ///
    /// Everything else uses [`Units::spawn`], which takes the lowest free slot
    /// the way `Unit_Spawn` does. A save has to restore the slot a unit was
    /// actually in, because slot numbers are referenced by county
    /// `garrison_unit`, by `besieged_by`, and by a band's `hired_by` — renumber
    /// them on load and the links point at the wrong armies.
    pub fn put(&mut self, id: usize, unit: Unit) {
        if let Some(slot) = self.slots.get_mut(id) {
            *slot = Some(unit);
        }
    }

    /// The unit standing on a tile, if any. The original keeps this in the
    /// runtime tile record's byte `+5`; recomputing it from the array is the
    /// same answer without a second place for it to be wrong.
    pub fn at(&self, x: u8, y: u8) -> Option<usize> {
        self.iter().find(|(_, u)| u.x == x && u.y == y).map(|(i, _)| i)
    }

    /// `Units_ResetMoves` (`0x004651B9`) — turn phase 7, for **all 150 slots
    /// regardless of type or owner**: `moveState = 0` and `movesUsed = 0`.
    ///
    /// `[D]`, and the loop really is unconditional: it does not skip garrisons,
    /// besiegers or free slots, and it does not touch the allowance, which each
    /// type's tick handler rewrites anyway.
    pub fn reset_moves(&mut self) {
        for (_, u) in self.iter_mut() {
            u.moving = false;
            u.moves_used = 0;
        }
    }

    /// `Army_RecountCountyTroops` (`0x004AD6C0`) — rebuild every county's
    /// `friendly_troops` / `enemy_troops`, which is what
    /// [`crate::ration::people_to_feed`] adds to the food requirement when
    /// *Army foraging* is on.
    ///
    /// ```text
    /// for c in 1..=16:  c.friendly = c.enemy = 0
    /// for unit in 1..=150 where type in {1,2} and garrisonCounty == 0:
    ///     county = unit.county
    ///     if county.owner == unit.owner                    county.friendly += unit.men
    ///     else if realm[unit.owner].ally == county.owner   county.friendly += unit.men
    ///     else                                             county.enemy    += unit.men
    /// ```
    ///
    /// Three things worth stating because they are easy to get wrong:
    ///
    /// * **revolting peasants are counted**, not just armies — the loop tests
    ///   type 1 *or* 2;
    /// * **a garrison is excluded**, so walking your army into your own castle
    ///   takes it off the county's food bill entirely;
    /// * a **besieging** army is *not* excluded, so it eats in the county whose
    ///   castle it is sitting outside.
    ///
    /// The original clears and fills all sixteen county slots regardless of how
    /// many the map has; so does this, because a unit standing on a tile whose
    /// county byte is above `county_count` would otherwise leave a stale count
    /// behind. `[D]`
    pub fn recount_county_troops(&self, counties: &mut [County; MAX_COUNTIES], realms: &[Realm; MAX_REALMS]) {
        for c in counties.iter_mut().skip(1) {
            c.friendly_troops = 0;
            c.enemy_troops = 0;
        }
        for (_, u) in self.iter() {
            if !u.kind.is_combatant() || u.is_garrisoned() {
                continue;
            }
            let Some(county) = counties.get_mut(u.county as usize) else { continue };
            let ally = realms.get(u.owner as usize).map_or(0, |r| r.ally);
            if county.owner == u.owner || (ally != 0 && ally == county.owner) {
                county.friendly_troops += u.men;
            } else {
                county.enemy_troops += u.men;
            }
        }
    }

    /// The men and the armies a realm has, which are two of `Score_RankRealms`'
    /// six inputs (realm `+0x54` and `+0x2C`).
    pub fn realm_totals(&self, realm: u8) -> (u8, i32) {
        let mut armies = 0u8;
        let mut men = 0i32;
        for (_, u) in self.iter() {
            if u.owner == realm && u.kind == UnitKind::Army {
                armies = armies.saturating_add(1);
                men += u.men;
            }
        }
        (armies, men)
    }
}

/// `Wages_ForRealm` (`0x004AD495`) — sum `Wages_ForUnit` over every **type-1**
/// unit this realm owns.
///
/// Garrisoned armies are included, besieging armies are included, and the
/// mercenary band is included because it is part of `men`. Revolting peasants
/// are not, because they are type 2 — a realm does not pay a mob that is
/// rebelling against it.
///
/// `docs/armies.md` §5.1: `g_mercWage` is `price / 10`, is copied onto the
/// band record, and is **never read anywhere**; the raise-army screen prints
/// `men / 2`; and this is what is actually charged. Three numbers for the same
/// thing, of which only this one is spent.
pub fn wages_for_realm(t: &Tables, units: &Units, realms: &[Realm; MAX_REALMS], realm: u8, difficulty: u8) -> i32 {
    let Some(r) = realms.get(realm as usize) else { return 0 };
    let mut total = 0;
    for (_, u) in units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            total += r.wage_for_unit(t, u.men, difficulty);
        }
    }
    total
}

/// Write each of a realm's armies' own wage into `Unit::wages`, and return the
/// bill.
///
/// The per-unit number is `L2.eng` 31/8 *"Wages"* on the army panel, and it is
/// the second source that makes `+0x15C` `[V]`. Kept in step with the realm
/// total by being computed in the same pass.
pub fn refresh_wages(t: &Tables, units: &mut Units, realms: &mut [Realm; MAX_REALMS], realm: u8, difficulty: u8) -> i32 {
    let Some(r) = realms.get(realm as usize) else { return 0 };
    let mut total = 0;
    let mut per_unit: Vec<(usize, i32)> = Vec::new();
    for (i, u) in units.iter() {
        if u.owner == realm && u.kind == UnitKind::Army {
            let w = r.wage_for_unit(t, u.men, difficulty);
            per_unit.push((i, w));
            total += w;
        }
    }
    for (i, w) in per_unit {
        if let Some(u) = units.get_mut(i) {
            u.wages = w;
        }
    }
    realms[realm as usize].wages = total;
    total
}

/// What one army's starvation check did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Starvation {
    /// Fed, or garrisoned, or the option is off. The counter is back to 0.
    Fed,
    /// The counter stepped to 1: a warning and nothing else.
    Warned,
    /// The counter is 2…4: a tenth of every troop count over ten walks off.
    Deserted { lost: i32 },
    /// The counter reached 5. The army is gone.
    Perished,
}

/// `Army_Starve` (`0x004ACE5E`), once a season, from inside `Wages_PayAll` and
/// **before** anyone is paid — so an army can desert from hunger and be billed
/// the reduced wage in the same season.
///
/// ```c
/// for unit in 1..=150 where owner != 0 and type == 1:
///     unit.orderFlags = 0, 0, 0;                     /* unconditionally */
///     if (!g_optArmiesEat)                        unit.starvation = 0;
///     else if (unit.men < Food_Available(county))  unit.starvation = 0;   /* fed */
///     else if (unit.garrisonCounty != 0)          unit.starvation = 0;   /* the castle feeds it */
///     else {
///         unit.starvation++;
///         if (unit.starvation == 1)      warn;
///         else if (unit.starvation < 5)  { Army_Desert(unit); warn louder; }
///         else                           { warn; Army_Destroy(unit); }
///     }
/// ```
///
/// Four things worth pinning down, all `[D]`:
///
/// * **the fed test is a strict `<`**, so an army of exactly the county's
///   available food starves;
/// * the food it is compared against is **not** what is left after the peasants
///   ate — see [`crate::ration::food_available`] — and takes no account of
///   other armies in the same county, so an army of 400 in a county with 3,000
///   available is fed however many armies stand beside it;
/// * a **garrison never starves**, whatever the county holds;
/// * in the desert case the men leave *before* the message; in the destroy case
///   the message is raised *before* the army goes. That ordering is not
///   cosmetic — it decides the order two lockstep peers append to the report.
///
/// `armies_eat` is `g_optArmiesEat`, `L2.eng` group 50 index 2 — the advanced
/// option the game itself calls *"Army foraging"*, and it is **off in the
/// shipped save**, in which case this only ever clears the counters.
pub fn starve(
    t: &Tables,
    units: &mut Units,
    counties: &[County; MAX_COUNTIES],
    armies_eat: bool,
    out: &mut Vec<crate::report::Message>,
) -> Vec<(usize, Starvation)> {
    let ids: Vec<usize> = units
        .iter()
        .filter(|(_, u)| u.kind == UnitKind::Army && u.owner != 0)
        .map(|(i, _)| i)
        .collect();
    let mut outcomes = Vec::new();
    let mut doomed = Vec::new();

    for id in ids {
        let Some(u) = units.get(id) else { continue };
        let (county_id, men, garrisoned, realm) = (u.county, u.men, u.is_garrisoned(), u.owner);
        let food = counties
            .get(county_id as usize)
            .map_or(0, |c| crate::ration::food_available(t, c));

        let fed = !armies_eat || men < food || garrisoned;
        let u = units.get_mut(id).expect("still there");
        if fed {
            u.starvation = 0;
            outcomes.push((id, Starvation::Fed));
            continue;
        }
        u.starvation += 1;
        let stage = u.starvation;
        if stage == 1 {
            out.push(crate::report::Message::ArmyStarving { realm, unit: id, county: county_id, stage });
            outcomes.push((id, Starvation::Warned));
        } else if stage < crate::tables::STARVATION_LIMIT {
            let lost = u.desert();
            out.push(crate::report::Message::ArmyStarving { realm, unit: id, county: county_id, stage });
            outcomes.push((id, Starvation::Deserted { lost }));
        } else {
            out.push(crate::report::Message::ArmyStarving { realm, unit: id, county: county_id, stage });
            outcomes.push((id, Starvation::Perished));
            doomed.push(id);
        }
    }
    for id in doomed {
        units.remove(id);
    }
    outcomes
}

/// Why [`combine`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombineRefusal {
    /// Together they would exceed [`crate::tables::ARMY_MAX_MEN`]. The original
    /// says nothing at all in this case — no message is raised.
    TooMany,
    /// Both carry a mercenary band. `L2.eng` **167**: *"Cannot combine armies.
    /// The mercenaries in these armies will not fight together."*
    TwoMercenaryBands,
    /// One of the slots is empty, or is not an army.
    NotAnArmy,
}

/// `Army_Combine` (`0x004AA181`) — merge `from` into `into` and destroy `from`.
///
/// ```text
/// if (men[into] + men[from] < 0x5DD)            /* 1501: at most 1500 merged */
///     if (merc[into] == 0 || merc[from] == 0)   /* two bands refuse */
///         ...merge...
///     else message 0xA7                          /* group 167 */
/// ```
///
/// **`[V]` 1500 is exact**, and it is the player's *"maximum army size is about
/// 1500"*. What the merge takes:
///
/// * `men` and the seven troop counts are summed;
/// * `movesUsed` takes **the higher of the two**, so a fresh army that merges
///   into a spent one is spent;
/// * the band, if only one side has it, moves across whole;
/// * the siege links move across, and the garrison's back-pointer is repaired.
///
/// Returns the men in the merged army, or why it refused. The caller destroys
/// `from`: this function only moves what is on the two records, because
/// `Army_Destroy` needs the realm array and this does not.
pub fn combine(units: &mut Units, into: usize, from: usize) -> Result<i32, CombineRefusal> {
    let (a, b) = match (units.get(into), units.get(from)) {
        (Some(a), Some(b)) if a.kind == UnitKind::Army && b.kind == UnitKind::Army => (a, b),
        _ => return Err(CombineRefusal::NotAnArmy),
    };
    if a.men + b.men > crate::tables::ARMY_MAX_MEN {
        return Err(CombineRefusal::TooMany);
    }
    if a.mercenaries.is_some() && b.mercenaries.is_some() {
        return Err(CombineRefusal::TwoMercenaryBands);
    }
    let absorbed = units.remove(from).expect("checked just above");
    let into_unit = units.get_mut(into).expect("checked just above");
    // **The higher, not the lower.** `docs/armies.md` §2.7 says the merge
    // "takes the *lower* of the two `movesUsed`", which reads as a refund and
    // is the opposite of what the code does:
    //
    // ```c
    // if ((char)movesUsed[from] < (char)movesUsed[into]) v = movesUsed[into];
    // else                                               v = movesUsed[from];
    // movesUsed[into] = v;
    // ```
    //
    // Both arms select the maximum. Merging a fresh army into a spent one
    // leaves the result spent, so a reinforcement cannot buy back movement —
    // which is a real tactical rule, and the inverse of the documented one.
    // Corrected in the document. `[D]`
    into_unit.moves_used = into_unit.moves_used.max(absorbed.moves_used);
    if into_unit.besieging_county == 0 {
        into_unit.besieging_county = absorbed.besieging_county;
    }
    if into_unit.besieged_by == 0 {
        into_unit.besieged_by = absorbed.besieged_by;
    }
    if into_unit.mercenaries.is_none() {
        into_unit.mercenaries = absorbed.mercenaries;
    }
    into_unit.men += absorbed.men;
    for t in 0..TROOP_TYPES {
        into_unit.troops[t] += absorbed.troops[t];
    }
    Ok(into_unit.men)
}

/// `Army_Destroy` (`0x004AA039`) — free the slot and put the realm back in
/// order.
///
/// Three things happen besides the slot being cleared, and each is a place a
/// naive `remove` would leave the kingdom wrong:
///
/// 1. the garrison or siege link is broken, in whichever direction it points;
/// 2. the realm's name counter at `+0x2D + nameIndex` is **decremented by
///    one**;
/// 3. the realm's wage bill is recomputed from what is left.
///
/// > **`docs/armies.md` §6.3 says `Army_Destroy` *"reverses the name
/// > counter"*. It does not.** `Army_PickName` adds **2** and this subtracts
/// > **1**, so every army a realm has ever raised leaves a permanent +1 on its
/// > name's counter. The effect is real and visible: names are not recycled
/// > evenly forever — a name that has been used and lost is still slightly
/// > less likely to come up again than one that has never been used.
/// > Corrected in the document. `[D]`
pub fn destroy(t: &Tables, units: &mut Units, realms: &mut [Realm; MAX_REALMS], names: &mut ArmyNames, id: usize, difficulty: u8) -> Option<Unit> {
    let unit = units.get(id)?.clone();
    // Break the links in whichever direction they point. A garrison names its
    // county; a besieger names the county whose garrison points back at it.
    if unit.garrison_county == 0 {
        if unit.besieging_county != 0 {
            for (_, other) in units.iter_mut() {
                if other.besieged_by == id as u8 {
                    other.besieged_by = 0;
                }
            }
        }
    } else {
        for (_, other) in units.iter_mut() {
            if other.besieging_county == unit.garrison_county {
                other.besieging_county = 0;
            }
        }
    }
    units.remove(id);
    names.release(unit.owner, unit.name_index);
    refresh_wages(t, units, realms, unit.owner, difficulty);
    Some(unit)
}

/// The twenty-four name counters a realm keeps at `+0x2D`, one per name slot.
///
/// `Army_PickName` (`0x004A9F72`) picks the **first** slot holding the lowest
/// count and adds 2 to it, so a name repeats only after every other has been
/// used — and, because [`destroy`] gives back only 1, the counters drift
/// upwards over a long game rather than returning to zero.
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
        // Strictly less than, so the *first* slot at the minimum wins and the
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

    /// The four types and the two budgets, which is the whole of
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
    /// scores 1, not 0, and the +20 is only on a score that reached 1.
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

    /// Twenty-four names, `+2` a use, so a lord runs through all of them before
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

    /// The loop tests type 1 **or 2**, so a peasant mob is a mouth too.
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
