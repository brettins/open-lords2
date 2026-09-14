#![allow(unused_imports)]
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

/// `Army_StrengthScore`'s bonus for being an army at all — the `+ 20` on any
/// score that reached 1.
pub const STRENGTH_SCORE_BONUS: i32 = 20;

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
/// of the specification.
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
/// in, because slot numbers are referenced by county
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
    ///
    /// # A garrison is not standing on its tile
    ///
    /// **`Unit_LinkToTile` (`0x0046EDDF`) opens `if (kind != 1 || garrisonCounty
    /// == 0)` and does nothing for anything else**, and `Army_GarrisonApply`
    /// calls `Unit_UnlinkFromTile` on its way in. So an army inside a castle is
    /// deliberately kept **out** of the tile's occupancy chain: two functions
    /// agree
    /// instead of the unit.
    ///
    /// It is not cosmetic. `Unit_TryEnterTile` tests occupancy *before* it tests
    /// any tile flag
    /// every siege into a field battle fought on open ground — which is exactly
    /// what happened here until this line existed: the besieger walked up, met
    /// the garrison as an obstacle.
    /// `[V]`
    pub fn at(&self, x: u8, y: u8) -> Option<usize> {
        self.iter()
            .find(|(_, u)| u.x == x && u.y == y && !u.is_garrisoned())
            .map(|(i, _)| i)
    }

    /// `Units_ResetMoves` (`0x004651B9`) — turn phase 7, for **all 150 slots
    /// regardless of type or owner**: `moveState = 0` and `movesUsed = 0`.
    ///
    /// `[D]`
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
/// * **revolting peasants are counted** — the loop tests
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
/// Garrisoned armies are included, besieging armies are included
/// mercenary band is included because it is part of `men`. Revolting peasants
/// are not, because they are type 2 — a realm does not pay a mob that is
/// rebelling against it.
///
/// `docs/armies.md` §5.1: `g_mercWage` is `price / 10`, is copied onto the
/// band record, and is **never read anywhere**; the raise-army screen prints
/// `men / 2`; and this is what is charged. Three numbers for the same
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
/// shipped save**
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
/// * `movesUsed` takes **the higher of the two**
///   into a spent one is spent;
/// * the band, if only one side has it, moves across whole;
/// * the siege links move across
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
    // leaves the result spent
    // which is a real tactical rule
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
/// Three things happen besides the slot being cleared
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

