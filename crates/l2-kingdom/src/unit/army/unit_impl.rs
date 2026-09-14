#![allow(unused_imports)]
use super::*;
use super::units_impl::*;
use super::wages::*;
use super::starvation::*;
use super::combine_part::*;
use super::destroy_part::*;
use super::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

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
            sub_tile: 0,
            sub_frame: 0,
            // `Unit_Spawn` (`0x0046E1B0`): `field_0x14b |= 1`.
            at_tile_edge: true,
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
    /// `[V]` on the thresholds and the arithmetic — the banks `0x48`, `0x60`格
    /// and `0x78` are 24 apart
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

    /// **`+0x1B`, the walk phase — derived, because in every state the
    /// original can reach it is `+0x149` halved.**
    ///
    /// `Unit_StepOnce` (`0x0046634D`) is the only writer of either byte and it
    /// moves them together: an admitted tick adds 1 to `+0x1B` and
    /// [`SUBTILE_STEP_SOLO`](crate::tables::SUBTILE_STEP_SOLO) to `+0x149`;
    /// reaching the tile edge zeroes both; and the commit writes `+0x149 = 1`
    /// with `+0x1B` already 0 — from that edge, or from `Unit_Spawn`'s cleared
    /// record, which is the only other way the latch it commits from is ever
    /// set. So a crossing reads `(1, 0), (3, 1) … (15, 7)` and a unit at rest
    /// `(0, 0)`, and a stored byte would be a second copy of a number the record
    /// already holds. **[D]** — and "only writer" is a search of the whole
    /// corpus: `+0x149` appears in `Unit_StepOnce` and in `Map_DrawArmies`,
    /// which only reads it.
    ///
    /// Single player. The network game's `+4` would make it `+0x149 / 4`, and
    /// that step is not selectable yet — see
    /// [`SUBTILE_STEP_NET`](crate::tables::SUBTILE_STEP_NET).
    ///
    /// **No rule reads it.** The four tick handlers turn it into the figure's
    /// frame through [`UNIT_WALK_FRAMES`] or [`MERCHANT_WALK_FRAMES`] — see
    /// [`Unit::sprite_frame`] — and nothing else in the binary looks at it.
    pub fn walk_phase(&self) -> usize {
        usize::from(self.sub_tile / crate::tables::SUBTILE_STEP_SOLO)
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
/// bankruptcy penalty, so it lives on the record
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

