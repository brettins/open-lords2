//! `Units_Tick` (`0x004650B0`) and the four phase kick-offs — **what
//! moves on the campaign map, and when**.
//!
//! `crate::movement` can take a unit one step and `crate::conquest` can resolve
//! the castle it arrives at, and between them there was still nothing that
//! *called* either during a turn. This module is the caller.
//!
//! # The finding that shapes the whole file: movement is not in a phase
//!
//! `docs/kingdom.md` §3.1's table says phase 2 is *"army movement, including
//! battle resolution"* and that it waits on unit type 1. Reading `Turn_Tick`
//! and `Units_Tick` end to end says something different, and it changes where
//! the code goes:
//!
//! ```c
//! /* the frame loop, 0x004B99C0 */
//! if ((g_battlePhase == 0) && (ticksDue != 0)) {
//!     FUN_0040490D();
//!     Turn_Tick();          /* the seven phases */
//!     Units_Tick();         /* every unit, every tick, whatever the phase */
//! }
//! ```
//!
//! **`Units_Tick` is a sibling of `Turn_Tick`, not a child of it**, it has
//! exactly one call site, and it never reads `g_turnPhase`. Every unit with
//! `moving == 2` takes a step on every tick of the whole turn. What the phases
//! do is *narrower* than the table suggests:
//!
//! | phase | what its first step really does | what it waits for |
//! |---:|---|---|
//! | 2 | `Siege_StartPhase` — validate garrison/besieger links. | the siege cursor sweep, `Siege_TickPhase() == 0` — **not** a unit predicate |
//! | 3 | re-target every transport at its cargo county's anchor and re-path it | no type-4 unit is moving |
//! | 5 | give every peasant mob a destination off a shared county cursor | no type-2 unit is moving |
//! | 6 | `Merchant_AdvanceAll` — the next leg of each merchant's route | no type-3 unit is moving |
//!
//! So the phases **originate** the game's own move orders and then wait for
//! them to finish; they never step anything themselves. A player's order is not
//! in that list at all — `Unit_OrderMove` sets the unit walking directly, and
//! it walks on the next tick whatever phase is current.
//!
//! Two consequences worth stating because a reimplementation gets them wrong by
//! default:
//!
//! * **`docs/kingdom.md` §3.1's phase-2 row is wrong**, and so was
//!   `Phase::ArmyMovement.wait()`. Phase 2 does not wait on armies. See
//!   [`crate::phase::Phase::wait`], which is corrected, and `docs/decisions.md`
//!   C35.
//! * **An army moving is not confined to a phase**, so [`Kingdom::tick_units`]
//!   is called on *every* tick by the turn driver, and the per-phase work is
//!   [`Kingdom::begin_unit_phase`].
//!
//! # One tile per tick, not a march
//!
//! `Unit_Step` (`0x00465D28`) loops only on the sub-tile animation code and
//! returns as soon as a tile is committed, so a unit enters **at most one tile
//! per tick**. With an army's 15 points that is five open-ground tiles or
//! fifteen road tiles a season; with the other three types' 10 it is three and
//! ten. [`crate::movement::march`] — which walks to exhaustion — is therefore
//! the right shape for a test and the wrong shape for the turn, because a
//! battle has to be able to interrupt the sweep between two tiles.
//!
//! # `moving` is really three states, and this crate has two
//!
//! `+0x14C` holds **0 idle, 1 ordered but not started, 2 stepping**. The phase
//! wait predicates (`FUN_004A4F5B`, `FUN_004A4E3D`) are not read-only queries:
//! they promote every 1 to a 2 *and* report that the phase is still busy, which
//! is how a phase both starts and waits for its units with one call.
//!
//! [`crate::unit::Unit::moving`] is a `bool`, and that is adequate here rather
//! than convenient. The 1 → 2 promotion always happens on the same tick
//! the order was given, in the same phase, before `Units_Tick` runs; nothing
//! observes a unit sitting at 1. The one place the distinction has teeth is a
//! *different* phase's units, and this driver steps every kind on every tick
//! exactly as the original's frame loop does, so there is nothing for the extra
//! state to gate. **`[D]`** — recorded because if a mob is ever seen stepping a
//! tick early, this is the paragraph that is wrong.

use crate::conquest::{self, Attack};
use crate::kingdom::Kingdom;
use crate::merchant;
use crate::movement::{self, Entry, Offence};
use crate::phase::Phase;
use crate::unit::{UnitKind, Units};

/// Two units met and neither will share the tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encounter {
    /// The unit that tried to move — the attacker.
    pub mover: usize,
    /// The unit already standing there — the defender.
    pub occupant: usize,
    /// The county the fight is in, taken from the **occupant**, which is what
    /// `FUN_004A7158` writes to `g_battleCounty`. The mover is still on its own
    /// tile, so its county is the wrong one whenever the pair straddle a
    /// border.
    pub county: u8,
}

/// Something that happened during a tick that the caller has to act on.
///
/// The variants are deliberately *reports*: a battle
/// needs `l2-sim`, which this crate must never depend on, and a county changing
/// hands needs to reach the interface. See [`Contact::Battle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contact {
    /// **Two armies met and a battle is due.** The caller fights it and brings
    /// the result back; nothing here resolves it.
    ///
    /// `FUN_004A7158` (`0x004A7158`) is the original's seam. It sets
    /// `g_battleIsSiege = 0`, `g_battleCounty` from the occupant, `g_battleArmyA`
    /// to the mover and `g_battleArmyB` to the occupant, clears both sides'
    /// battle scratch fields, and then either auto-resolves or opens the battle
    /// screen. On the interactive path it sets a latch that **abandons the rest
    /// of the unit sweep for that tick** and puts back the waypoint the step had
    /// already consumed, so the army resumes on the same tile afterwards.
    ///
    /// This driver reproduces the abandon — [`UnitsTick::battle`] stops the
    /// sweep — and does not need to reproduce the waypoint restore, because
    /// [`crate::movement::step`] returns [`Entry::Occupied`] *before* consuming
    /// anything.
    Battle(Encounter),
    /// An army reached a county's castle tile and [`conquest::attack_county`]
    /// said what came of it. A [`Attack::Battle`] here is the same handoff as
    /// [`Contact::Battle`] and is reported separately only because the caller
    /// has to change the county's owner afterwards.
    Castle { unit: usize, county: u8, outcome: Attack },
    /// **An army reached the castle building itself**, and
    /// [`conquest::reach_castle_building`] garrisoned it or laid siege.
    ///
    /// A different tile from [`Contact::Castle`] and a different rule: that one
    /// is the county *town* (plane-0 bit `0x40`) and takes the county; this is
    /// the castle's own 2×2 block (bit `0x80` with a castle standing on it) and
    /// is the only way into either a garrison or a siege.
    CastleBuilding { unit: usize, county: u8, arrival: conquest::CastleArrival },
    /// **Two of an AI realm's armies met and merged into one**, with no order
    /// and no prompt. The mover's slot is gone. See
/// [`Kingdom::merge_on_contact`], and a *person's* two armies do
    /// not do this.
    Merged { mover: usize, into: usize },
    /// A unit's next tile is held by somebody it will not fight — its own side,
/// an ally, or a merchant. The move ends.
    ///
    /// **A divergence, and a known one.** The original lets these through: the
    /// tile record carries a linked list of stacked units and
    /// `Unit_EnterOccupiedTile` returns the ordinary cost code, so the mover
    /// walks on and the pair stack. `crate::unit::Units` answers occupancy from
    /// the unit array and has no stack, so here the move stops instead. Nothing
    /// deadlocks — the unit is re-pathed by its phase next season, and a
    /// player's army can be re-ordered — but a friendly unit is an obstacle
    /// where the original has none. Modelling the stack is a `Units` change and
    /// is not this work.
    Blocked { mover: usize, occupant: usize },
}

/// What became of an army that walked onto its own castle — the three arms of
/// `Army_GarrisonApply` (`0x004A79A3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Garrison {
    /// The castle was empty and this army is now its garrison.
    Took,
    /// A garrison was already in place and the two were merged into it —
    /// `Army_Combine`, so the arriving slot is gone.
    Joined { into: usize },
    /// The two together would be over
    /// [`crate::industry::garrison_cap`], so nobody goes in. The army's
    /// mission is reset to [`crate::ai_army::Mission::SEEK_ENEMY`] and it
    /// stops — which is the original's own answer and is what stops a lord
    /// marching the same men at the same full castle every turn.
    TooMany,
}

/// **An army crossed into a county its owner does not own** — the comparison
/// `Unit_EnterCounty` (`0x004ABB36`) makes on every border crossing:
///
/// ```c
/// if (g_counties[county].owner != unit.owner) {
///     if (unit.owner == g_localPlayer) DAT_00553210 = 1;      /* the invasion tip */
///     …the taunt, when the county is owned and is the destination…
/// }
/// ```
///
/// A neutral county counts: its owner is 0 and no army's is. `Army_Tick`
/// (`0x0046521F`) is the only caller, so only armies cross this way — a
/// merchant, a cart or a mob never does. `County_GreetArmy` runs first and
/// only enqueues a reply, so the owner compared is the owner at the crossing.
///
/// **Reported, not stored.** The flag is `g_localPlayer`'s, which is a peer's
/// and not the world's, so the local-player test and the flag itself are
/// `l2_game::tip`'s; this carries the half that is the same on every peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Incursion {
    pub unit: usize,
    /// The army's owner at the crossing.
    pub owner: u8,
    pub county: u8,
}

/// **A `Msg_Enqueue` the unit sweep made**, in the order the sweep made it.
///
/// Two kinds, and they differ in who decides the recipient. A [`Posted::Letter`]
/// is fully addressed by the world — `County_GreetArmy` writes to the army's
/// owner and the invasion letter to the county's — so any peer's ring filter
/// keeps or drops it. A [`Posted::Capture`] is not: `County_ChangeOwner` chooses
/// its letter by comparing realms against `g_localPlayer`, so what is reported
/// is the facts, and the peer chooses. `l2_game::arrival` posts both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posted {
    /// `Unit_EnterCounty` (`0x004ABB36`): a greeting or an invasion letter.
    Letter(crate::diplomacy::Letter),
    /// `County_ChangeOwner` (`0x004A72FE`), reached from `Army_AttackCounty`.
    Capture(crate::conquest::Capture),
}

/// What one call to [`Kingdom::tick_units`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitsTick {
/// How many units entered a tile.
    pub stepped: usize,
    /// In the order they happened, which is ascending slot order.
    pub contacts: Vec<Contact>,
    /// Diplomatic hits earned by trampling, for a caller with a diplomacy
    /// layer. See [`crate::movement::Offence`].
    pub offences: Vec<Offence>,
    /// Armies that crossed into somebody else's county. See [`Incursion`].
    pub incursions: Vec<Incursion>,
    /// The letters the sweep posted, oldest first. See [`Posted`].
    pub posted: Vec<Posted>,
}

impl UnitsTick {
    /// The battle this tick raised, if any. At most one: the sweep stops on the
    /// first, exactly as `Units_Tick`'s latch does.
    pub fn battle(&self) -> Option<Encounter> {
        self.contacts.iter().find_map(|c| match c {
            Contact::Battle(e) => Some(*e),
            Contact::Castle { unit, outcome: Attack::Battle { defender, .. }, county } => {
                Some(Encounter { mover: *unit, occupant: *defender, county: *county })
            }
            _ => None,
        })
    }

    /// Counties that changed hands this tick.
    pub fn captures(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.contacts.iter().filter_map(|c| match c {
            Contact::Castle { unit, county, outcome: Attack::Captured(_) } => Some((*unit, *county)),
            _ => None,
        })
    }
}

impl Kingdom {
    /// `Army_GarrisonApply` (`0x004A79A3`) — put an army inside its own
    /// county's castle.
    ///
    /// ```c
    /// unit.needsDestination = 1;                       /* whatever happens */
    /// total = unit.men + (garrison ? garrison.men : 0);
    /// if (total > g_castleGarrisonCap[castleType]) { unit.mission = 2; unit.moving = 0; }
    /// else if (county.garrisonUnit == 0) {
    ///     county.garrisonUnit = unit;  unit.garrisonCounty = county;  unit.mission = 5;
    ///     unit.x = county.castleX;  unit.y = county.castleY;          /* a teleport */
    ///     unit.moving = 0;  unit.isPlayerDriven = 1;  unit.movesUsed += 5;
    ///     unit.destCounty = county;
    /// } else Army_Combine(county.garrisonUnit, unit);
    /// Army_RecountCountyTroops();
    /// ```
    ///
    /// Three things the shape decides:
    ///
    /// * **The cap is tested against the pair**, not against the new arrival,
    ///   so a castle 90 % full refuses an army that would have fitted in the
    ///   10 %. Reproduced.
    /// * **It is a teleport, not a step.** The army does not walk onto the
    ///   castle tile; it is placed on it and charged five moves. The tile it
    ///   was walking to is `County_FindCastleTile`'s `+0x74`/`+0x75`, which
    ///   this crate does not store — [`crate::ai_army::aim_tile`] recomputes it
    ///   off the map, which is what wrote that pair in the first place. `[I]`
    ///   on the substitution and `[V]` on there being one tile to find:
    ///   `battle-before.sav`'s garrison stands on county 2's stored pair and
    ///   that tile carries plane-0 `0x80` with terrain `0x15`.
/// * **A second army merges.** Two garrisons in one
///   castle: `garrisonUnit` is one slot.
    pub fn garrison_army(&mut self, unit: usize, county: u8) -> Garrison {
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = true;
        }
        let Some(c) = self.counties.get(county as usize) else { return Garrison::TooMany };
        let (castle_type, sitting_slot) = (c.castle_type, c.garrison_unit);
        let sitting = self.campaign.units.get(sitting_slot).map_or(0, |u| u.men);
        let arriving = self.campaign.units.get(unit).map_or(0, |u| u.men);
        if arriving + sitting > crate::industry::garrison_cap(&self.tables, castle_type) {
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = crate::ai_army::Mission::SEEK_ENEMY;
                u.moving = false;
            }
            return Garrison::TooMany;
        }
        // Already the garrison: nothing to join. See
        // [`crate::conquest::reach_castle_building`] for why this guard is here
        // and why the original has none.
        if sitting_slot == unit {
            return Garrison::Joined { into: sitting_slot };
        }
        if sitting_slot != 0 {
            let _ = crate::unit::combine(&mut self.campaign.units, sitting_slot, unit);
            let realms = self.realms.clone();
            self.campaign.units.recount_county_troops(&mut self.counties, &realms);
            return Garrison::Joined { into: sitting_slot };
        }
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let (x, y) = crate::ai_army::aim_tile(
            &self.campaign.map,
            &self.counties,
            from,
            county,
            crate::ai_army::Aim::Castle,
        );
        self.counties[county as usize].garrison_unit = unit;
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = crate::ai_army::Mission::GARRISON;
            u.garrison_county = county;
            u.x = x;
            u.y = y;
            u.county = county;
            u.moving = false;
            u.path.clear();
            u.player_driven = true;
            u.moves_used += GARRISON_MOVE_COST;
            u.dest_county = county;
            u.dest = Some((x, y));
        }
        let realms = self.realms.clone();
        self.campaign.units.recount_county_troops(&mut self.counties, &realms);
        Garrison::Took
    }

    /// `Units_Tick` (`0x004650B0`) — one step for every unit that is walking.
    ///
    /// Slots are walked in ascending order, every kind, whatever the phase.
    /// The sweep **stops at the first battle**, which is the original's
    /// `DAT_0050A49E` latch: `Units_Tick` returns the moment
    /// `Unit_EnterOccupiedTile` has opened a battle, leaving the higher slots
    /// unmoved until the battle is over. That is observable — an army in slot 9
    /// does not move on the tick an army in slot 4 picks a fight — so it is
/// reproduced.
    pub fn tick_units(&mut self) -> UnitsTick {
        let mut out = UnitsTick::default();
        // Every handler's *first* statement is its allowance, written whether
        // or not the unit is going anywhere. See [`refresh_allowances`] for why
// that is load-bearing.
        refresh_allowances(&mut self.campaign.units);
        for id in 1..crate::unit::MAX_UNITS {
            if !self.campaign.units.get(id).is_some_and(|u| u.moving) {
                continue;
            }
            // **`Unit_StepOnce`'s other arm**, and the one that decides how
            // fast anything on the campaign map goes. See [`cross_sub_tile`].
            if !cross_sub_tile(&mut self.campaign.units, id) {
                continue;
            }
            // **`Unit_Step` (`0x00465D28`)'s first statement inside its loop**,
            // and the fog of war's commonest writer:
            //
            // ```c
            // if (unit.field_0x14b & 1) {                       /* on a tile centre */
            //     if (unit.kind == 1 && unit.owner == g_localPlayer) {
            //         FUN_0046E067(unit.x, unit.y, 6);  Gfx_MarkAllDirty();
            //     }
            //     ...the stop tests, then Unit_StepOnce...
            // ```
            //
            // `at_tile_edge` is that latch, and it is set here and nowhere
            // else in this loop — on the tile a unit is spawned on, and on each
            // tile it finishes crossing into — so this is every tile centre the
            // original's loop reaches, before the budget test that may stop the
            // unit on it. An army only: a mob, a merchant and a transport walk
            // blind. `crate::explore` has why the realm stands in for
            // `g_localPlayer`.
            if let Some(u) = self.campaign.units.get(id) {
                if u.kind == crate::UnitKind::Army {
                    let (owner, x, y) = (u.owner, u.x as i32, u.y as i32);
                    self.campaign.explored.reveal_square(owner, x, y, crate::explore::ARMY_SIGHT);
                }
            }
            self.step_one(id, &mut out);
            if out.battle().is_some() {
                break;
            }
        }
        out
    }

    /// One unit, one tile.
    fn step_one(&mut self, id: usize, out: &mut UnitsTick) {
        let Some(step) = movement::step(
            &mut self.campaign.map,
            &mut self.counties,
            &self.realms,
            &mut self.campaign.units,
            id,
        ) else {
            // No path, or no moves left. `Unit_Step` writes `moving = 0` in
            // both cases; `movement::step` reports the refusal and leaves the
            // flag, so it is cleared here or the phase never settles.
            if let Some(u) = self.campaign.units.get_mut(id) {
                u.moving = false;
                if u.path.is_empty() {
                    u.needs_destination = true;
                }
            }
            return;
        };

        if step.moved {
            out.stepped += 1;
        }
        if let Some(o) = step.offence {
            // `Unit_CrossField` (`0x0046673C`) and `Unit_BurnDwelling`
            // (`0x00468AE2`) both call `Diplo_Offend` **inline**, in the middle
            // of the step, and this is the same place. `Step::offence` stays as
            // a report because the caller may want to say something about it;
            // what it no longer is, is the only thing that happens.
            crate::diplomacy::offend(
                &mut self.realms,
                o.against,
                o.by,
                o.amount.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
            );
            out.offences.push(o);
        }
        // `Army_Tick` and `PeasantMob_Tick` recount the county's troops on a
        // border crossing; `Merchant_Tick` and `Transport_Tick` do not, and
        // that asymmetry is `docs/armies.md` §2.1a's.
        if step.entered_county.is_some()
            && self.campaign.units.get(id).is_some_and(|u| u.kind.is_combatant())
        {
            self.campaign.units.recount_county_troops(&mut self.counties, &self.realms);
        }
        // `Unit_EnterCounty`, from `Army_Tick` and nothing else.
        if let (Some(county), Some(u)) = (step.entered_county, self.campaign.units.get(id)) {
            if u.kind == crate::UnitKind::Army {
                // Its owner test, for the invasion tip.
                if self.counties.get(county as usize).is_some_and(|c| c.owner != u.owner) {
                    out.incursions.push(Incursion { unit: id, owner: u.owner, county });
                }
                // Its two `Msg_Enqueue`s — a neutral county's greeting or a
                // lord's invasion letter — the second of which advances the
                // invader's voice rotation, which is simulation state.
                if let Some(letter) =
                    crate::arrival::enter_county(&self.counties, &mut self.realms, u, county)
                {
                    out.posted.push(Posted::Letter(letter));
                }
            }
        }

        // **Two branches implemented this rule and the other one is kept.**
        // `ai-lords-play` added a garrison-only handler here; the castles
        // branch added `conquest::reach_castle_building` below, which covers
        // the siege half too and is traced statement by statement to
        // `Army_GarrisonApply` (`0x004A79A3`) — including the teleport onto
        // the cached castle tile, which the other placed by search. The one
        // thing only the AI branch had, the mission reset that stops a lord
        // marching at a full castle for ever, now lives in that function.
        if let Some(county) = step.reached_castle {
            let restore = self.restore();
            let outcome = conquest::attack_county(
                &self.tables,
                &self.campaign.map,
                &mut self.counties,
                &mut self.realms,
                &mut self.campaign.units,
                &mut self.campaign.names,
                id,
                county,
                self.options.difficulty,
                self.year,
                &mut self.campaign.explored,
                restore,
            );
            // `County_ChangeOwner`'s letter, posted where the capture happens.
            if let Attack::Captured(capture) = outcome {
                out.posted.push(Posted::Capture(capture));
            }
            out.contacts.push(Contact::Castle { unit: id, county, outcome });
            return;
        }

        if let Some(county) = step.reached_castle_building {
            let arrival = conquest::reach_castle_building(
                &self.tables,
                &self.campaign.map,
                &mut self.counties,
                &self.realms,
                &mut self.campaign.units,
                id,
                county,
                self.season,
            );
            out.contacts.push(Contact::CastleBuilding { unit: id, county, arrival });
            return;
        }

        if let Entry::Occupied(occupant) = step.entry {
            if self.merge_on_contact(id, occupant) {
                out.contacts.push(Contact::Merged { mover: id, into: occupant });
                return;
            }
            out.contacts.push(self.classify_occupied(id, occupant));
        }
    }

    /// The **AI's automatic merge** — `Unit_EnterOccupiedTile`'s same-owner
    /// branch, which this driver had only half of.
    ///
    /// ```c
    /// if (mover.owner == occupant.owner) {
    ///     if (occupant.ownerIsHuman) {                 /* the half that was here */
    ///         if (mover.mergeOrder == occupant) Army_Combine(mover, occupant);
    ///         return ordinaryCode;
    ///     }
    ///     if (mover.mission    == 4) return ordinaryCode;   /* on a garrison errand */
    ///     if (occupant.mission == 4) return ordinaryCode;
    ///     Army_Combine(mover, occupant);                     /* unconditional */
    ///     return ordinaryCode;
    /// }
    /// ```
    ///
    /// **An AI's two armies merge the moment they touch**, with no order and no
/// prompt — so an AI realm ends a long game with a few large
/// armies. A **person's** two armies
    /// merge only on an explicit order, and that asymmetry is the original's:
    /// the test is on the *occupant's* `ownerIsHuman`, not on the mover's.
    ///
    /// The two exemptions are the reason [`crate::ai_army::Mission`] has to be
    /// modelled at all outside `ai_army`: an army walking to a castle to join
    /// its garrison is on a **dedicated errand** and is not to be absorbed by
    /// whatever it passes, in either direction. `[D]`
    ///
    /// Returns whether the pair were merged, in which case the mover's slot is
    /// gone and there is no contact to classify.
    fn merge_on_contact(&mut self, mover: usize, occupant: usize) -> bool {
        let units = &self.campaign.units;
        let (Some(m), Some(o)) = (units.get(mover), units.get(occupant)) else { return false };
        if m.kind != UnitKind::Army || o.kind != UnitKind::Army || m.owner != o.owner {
            return false;
        }
        if o.owner_is_human
            || m.mission == crate::ai_army::Mission::JOIN_GARRISON
            || o.mission == crate::ai_army::Mission::JOIN_GARRISON
        {
            return false;
        }
        crate::unit::combine(&mut self.campaign.units, occupant, mover).is_ok()
    }

    /// `Unit_EnterOccupiedTile` (`0x004658C1`), reduced to the question this
    /// driver has to answer: is that a fight?
    ///
    /// The original's ladder, in its own order:
    ///
    /// 1. the **mover** is a merchant or a transport — walk through;
    /// 2. the **occupant** is a merchant — walk through, so a merchant is never
    ///    attacked;
    ///
    /// **Rungs 1 and 2 no longer reach here.** They are not "no fight, but the
    /// move ends": the original returns the tile's ordinary Road or Open code
    /// and the mover steps onto the tile, which is a *movement* answer rather
    /// than a contact. [`crate::movement::pass_through`] gives it, so
    /// [`Entry::Occupied`] never reaches this function in those two cases and
    /// the rungs are kept below only because the ladder reads wrong without
    /// them. `docs/decisions.md` C40.
    ///
    /// 3. the occupant is an army or a mob:
    ///    * same owner — merge, but only on an explicit merge order;
    ///    * different owner — `Diplo_ActionAllowed` decides. An **ally** is
    ///      walked through (and has its grudge counter nudged); anyone else is
    ///      a battle;
    /// 4. the occupant is a transport of another realm — it is **seized**, not
    ///    fought.
    ///
    /// Everything that is not a battle comes back [`Contact::Blocked`] here,
    /// for the reason that variant documents.
    fn classify_occupied(&self, mover: usize, occupant: usize) -> Contact {
        let units = &self.campaign.units;
        let (Some(m), Some(o)) = (units.get(mover), units.get(occupant)) else {
            return Contact::Blocked { mover, occupant };
        };
        let hostile = m.kind.is_combatant()
            && o.kind.is_combatant()
            && m.owner != o.owner
            && !self.allied(m.owner, o.owner);
        if hostile {
            Contact::Battle(Encounter { mover, occupant, county: o.county })
        } else {
            Contact::Blocked { mover, occupant }
        }
    }

    /// `Diplo_ActionAllowed` (`0x004A0710`)'s ally test, and nothing else of it.
    /// Realm `+0x81` is one byte, so a realm has at most one ally, and the
    /// relation is read from both sides because the original stores it on both.
    fn allied(&self, a: u8, b: u8) -> bool {
        if a == b {
            return true;
        }
        let ally_of = |r: u8| self.realms.get(r as usize).map_or(0, |x| x.ally);
        ally_of(a) == b || ally_of(b) == a
    }

    /// The work a unit phase does on its **first** step — the one call that
    /// originates that phase's move orders.
    ///
    /// Returns how many units were set walking, which is only interesting to a
    /// test; the phase's wait reads [`Kingdom::units_moving`] instead.
    pub fn begin_unit_phase(&mut self, phase: Phase) -> usize {
        match phase {
            // `Siege_StartPhase` (`0x004A82B9`). Sieges are out of scope, and
            // the important half for this module is the negative: the function
            // touches only `besiegedBy`, `besiegingCounty` and `garrisonUnit`,
            // and never `moving`, `path`, `dest` or `movesUsed`. **Phase 2
            // originates no movement**, so there is nothing here to leave out.
            Phase::ArmyMovement => 0,
            Phase::SupplyTransports => self.retarget_transports(),
            Phase::PeasantMobs => self.retarget_mobs(),
            Phase::Merchants => merchant::advance_all(
                &self.campaign.routes,
                &self.campaign.map,
                &self.counties,
                &mut self.campaign.units,
                self.county_count,
            ),
            _ => 0,
        }
    }

    /// Whether any unit of a kind is still walking — the predicate
    /// `FUN_004A4F5B(kind)` answers for phases 3 and 6, and
    /// `FUN_004A4E3D(2, 6)` for phase 5.
    ///
    /// **Phase 5's is narrower and the difference is real**: it counts only
    /// mobs owned by realm **6** whose `ownerIsHuman` byte is clear — the
    /// ownerless rabble — so a type-2 unit belonging to a player would not hold
    /// the phase open. Nothing creates one, and the guard is reproduced anyway
    /// because a rule that only matters in a case that cannot arise is exactly
    /// the kind that stops being true later.
    pub fn units_moving(&self, kind: UnitKind) -> bool {
        self.campaign.units.iter().any(|(_, u)| {
            u.moving
                && u.kind == kind
                && (kind != UnitKind::PeasantMob || (u.owner == OWNERLESS && !u.owner_is_human))
        })
    }

    /// `FUN_004A4E3D(kind, realm)` — is that realm's own unit of this kind
    /// still walking?
    ///
    /// The AI's turn reads this and not [`Kingdom::units_moving`]: **the
    /// finish test for a realm is `15 + 2 × realmIndex <= aiStep` *and* this
    /// returning false** (`docs/armies.md` §3.2). A realm with armies still on
/// the road keeps stepping past its threshold, so an AI turn is
    /// not a fixed number of steps.
    ///
    /// The `ownerIsHuman` clause is the original's and is kept: the predicate
    /// only ever sees AI realms.
    pub fn realm_units_moving(&self, kind: UnitKind, realm: u8) -> bool {
        self.campaign
            .units
            .iter()
            .any(|(_, u)| u.moving && u.kind == kind && u.owner == realm && !u.owner_is_human)
    }

    /// `FUN_00429418` — **the first step of phase 3.**
    ///
    /// Every transport, every turn, unconditionally: point it at its cargo
    /// county's **anchor tile** and re-path. It does not test
    /// `needs_destination`, which is what makes a transport resume a journey it
    /// could not finish last season.
    ///
    /// The destination is the anchor itself, not a free tile near it — that is
    /// the merchant's rule, and the two are different. See
    /// [`crate::merchant::advance_all`].
    fn retarget_transports(&mut self) -> usize {
        let mut started = 0;
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::Transport)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            let Some(cargo) = self.campaign.units.get(id).map(|u| u.cargo_county) else { continue };
            let Some(county) = self.counties.get(cargo as usize) else { continue };
            let anchor = (county.anchor_x, county.anchor_y);
            if let Some(u) = self.campaign.units.get_mut(id) {
                u.dest = Some(anchor);
                u.dest_county = cargo;
                u.needs_destination = false;
            }
            if movement::order_move_by_road(&self.campaign.map, &mut self.campaign.units, id, anchor)
                .is_some()
            {
                started += 1;
            }
        }
        started
    }

    /// `FUN_004AC499` and its picker `FUN_004AC5BA` — **the first step of
    /// phase 5.**
    ///
    /// ```c
    /// if ((g_season == 2) || (unit.destCounty == 0)) {
    ///     if (++cursor > g_countyCount) cursor = 1;
    ///     if (unit.county == cursor && ++cursor > g_countyCount) cursor = 1;
    ///     unit.destCounty = cursor;
    /// }
    /// if (unit.destCounty != 0) {
    ///     unit.hasNoDestination = 0;
    ///     unit.destX = county[cursor].anchorX;      /* <- cursor, not destCounty */
    ///     unit.destY = county[cursor].anchorY;
    /// }
    /// ```
    ///
    /// Three things, and the third is a bug worth keeping:
    ///
    /// * **the cursor is one counter for the whole map**, not one per mob, so
    ///   consecutive mobs are sent to consecutive counties;
    /// * a mob is never sent to the county it is standing in — the second bump
    ///   is that guard, and note it can only skip **one**, so on a one-county
    ///   map it lands back where it started;
    /// * **the coordinates come from the cursor and the county id from
    ///   `destCounty`, and outside spring those are not the same county.** A mob
    ///   that kept last season's `destCounty` walks to *this* season's cursor
    ///   county's anchor while believing it is going somewhere else. Reproduced,
    ///   because it is what the original does and a lockstep peer that "fixed"
    ///   it would desync.
    ///
    /// Mobs are re-targeted wholesale in **season 2**; otherwise only those with
    /// no destination at all.
    fn retarget_mobs(&mut self) -> usize {
        let mut started = 0;
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::PeasantMob)
            .map(|(id, _)| id)
            .collect();
        if self.county_count == 0 {
            return 0;
        }
        for id in ids {
            let Some(u) = self.campaign.units.get(id) else { continue };
            let (here, mut dest_county) = (u.county, u.dest_county);
            if self.season == MOB_RETARGET_SEASON || dest_county == 0 {
                self.mob_cursor_next();
                if here as usize == self.campaign.mob_cursor {
                    self.mob_cursor_next();
                }
                dest_county = self.campaign.mob_cursor as u8;
            }
            if dest_county == 0 {
                continue;
            }
            // The anchor is the *cursor's* county, not `dest_county`. See above.
            let Some(county) = self.counties.get(self.campaign.mob_cursor) else { continue };
            let anchor = (county.anchor_x, county.anchor_y);
            if let Some(u) = self.campaign.units.get_mut(id) {
                u.dest_county = dest_county;
                u.dest = Some(anchor);
                u.needs_destination = false;
            }
            if movement::order_move(
                &self.campaign.map,
                &mut self.campaign.units,
                id,
                anchor,
                movement::Routing::Direct,
            )
            .is_some()
            {
                started += 1;
            }
        }
        started
    }

    fn mob_cursor_next(&mut self) {
        self.campaign.mob_cursor += 1;
        if self.campaign.mob_cursor > self.county_count {
            self.campaign.mob_cursor = 1;
        }
    }
}

/// Realm 6 — the owner byte merchants and peasant mobs carry. Not a realm: it
/// is one past the five, so `Units::realm_totals` and the wage bill
/// never see them.
/// What `Army_GarrisonApply` charges an army for walking into a castle.
pub const GARRISON_MOVE_COST: i32 = 5;

pub const OWNERLESS: u8 = 6;

/// The season every peasant mob is re-targeted in, whatever it was already
/// doing. `g_season == 2`, and season 2 is **spring** (`docs/kingdom.md` §3.3).
pub const MOB_RETARGET_SEASON: u8 = 2;

/// Whether a unit of this kind holds its phase open. A convenience for a
/// caller that has a [`Phase`].
pub fn kind_for_phase(phase: Phase) -> Option<UnitKind> {
    match phase.wait() {
        crate::phase::PhaseWait::Units(kind) => Some(kind),
        _ => None,
    }
}

/// Every unit of a kind, in ascending slot order. Used by the tests and by
/// anything that wants to ask a question of one type.
pub fn ids_of_kind(units: &Units, kind: UnitKind) -> Vec<usize> {
    units.iter().filter(|(_, u)| u.kind == kind).map(|(id, _)| id).collect()
}

/// Rebuild every unit's move allowance from its type, the way each tick handler
/// does before anything else it does.
///
/// `Army_Tick` writes **15** to `+0x154` as its first statement, and the other
/// three write **10** — *every tick*, unconditionally. The field is therefore
/// derived, never stored, and the England fixture proves it: all six merchants
/// in `england-turn1.sav` have `moveAllowance = 0` on disk because nothing had
/// ticked them since the load.
///
/// This matters for a unit that arrives from a save or from the levy without
/// one: it would otherwise have an allowance of zero and never take a step.
pub fn refresh_allowances(units: &mut Units) {
    for (_, u) in units.iter_mut() {
        u.move_allowance = u.kind.move_allowance();
    }
}

/// **One tick of walking across the tile the unit is on.** Answers whether the
/// far edge was reached, which is when — and only when — the next tile is
/// entered.
///
/// This is `Unit_StepOnce` (`0x0046634D`)'s **`(field_0x14b & 1) == 0` arm**,
/// and [`crate::movement::step`] is its other one. The original is one
/// function with two halves picked by a latch; here the halves are in the two
/// crates that already own them — the driver decides *when* a tile is entered,
/// the mover decides *what happens* when it is — and [`Unit::at_tile_edge`] is
/// the latch, in the unit record, where the original keeps it.
///
/// ```c
/// cVar1 = onRoad ? 0 : 3;
/// if (cVar1 < ++field_0x14a) {
///     field_0x14a = 0;
///     field_0x149 += (g_multiplayer == 0) ? 2 : 4;
///     if (field_0x149 >= 0x10) { field_0x14b |= 1; field_0x149 = 0; return 2; }
/// }
/// return 1;                       /* still crossing: no tile is entered */
/// ```
///
/// # What it costs a tile, and why this is the whole of the defect
///
/// Sixteen has to be reached in steps of two, so **eight admissions a tile**;
/// off a road only one tick in four is admitted and on a road every one is.
/// Single player, therefore:
///
/// | | admissions | ticks a tile |
/// |---|---:|---:|
/// | road | 8 | **8** |
/// | anything else | 8 | **32** |
///
/// A merchant on the England position walks a ten-tile road route, so its leg
/// takes eighty ticks.
/// tile every tick, which at our 16 ms tick is sixty-two tiles a second, and
/// what a player described as *"they move insanely fast"*. `docs/decisions.md`
/// **C134**.
///
/// **It must not be moved above this crate.**
/// The tick a unit arrives on decides which tick a battle starts on, which
/// county changes hands first, and when a phase's wait comes true; two peers
/// that disagreed about it would be playing different games
/// (`docs/netcode.md` §5). It is in the save and therefore in the digest.
///
/// # A merchant is interpolated exactly like an army — **[V]**, and asked
///
/// *Does he glide, or did the original step him tile to tile?* He glides, in
/// three places and none of them tests the kind byte:
///
/// * `Merchant_Tick` (`0x00465622`) and `Transport_Tick` (`0x00465761`) call
///   `Unit_Step` on every tick they are `moving == 2`, the same statement
///   `Army_Tick` (`0x0046521F`) ends on. `moveAllowance 10` is the turn's
///   budget, not a frame delay.
/// * `Unit_StepOnce` (`0x0046634D`) has no kind test at all: `+0x149` and the
///   road-keyed divider above are every unit's.
/// * `Map_DrawArmies` (`0x00408438`) reads its six `8 × 16` offset tables
///   **before** the first `kind` comparison; the only kind-dependent parts are
///   sheet B for a transport, the nudge and the mark-tile size.
///
/// His walk frames advance too, and faster than an army's: `Merchant_Tick`
/// writes `+0x07 = ((facing + 1) & 7) * 6 + g_merchantWalkFrames[+0x1B]` — six
/// frames against the army's three — off the counter bumped in the same branch
/// as `+0x149`. So there is nothing to take off this type.
fn cross_sub_tile(units: &mut Units, id: usize) -> bool {
    let Some(u) = units.get_mut(id) else { return false };
    // Already at the edge — the original leaves the latch set when a step is
    // refused, and the retry commits without re-crossing the tile.
    if u.at_tile_edge {
        return true;
    }
    u.sub_frame = u.sub_frame.wrapping_add(1);
    if u.sub_frame <= crate::tables::SUBTILE_DIVIDER[usize::from(u.on_road)] {
        return false;
    }
    u.sub_frame = 0;
    u.sub_tile = u.sub_tile.saturating_add(crate::tables::SUBTILE_STEP_SOLO);
    if u.sub_tile < crate::tables::SUBTILE_SPAN {
        return false;
    }
    u.at_tile_edge = true;
    u.sub_tile = 0;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
    use crate::unit::Unit;
    use crate::Options;

    /// Two counties either side of x = 32, a road along y = 10, and a realm 1
    /// that is human.
    fn kingdom() -> Kingdom {
        let mut k = Kingdom::new(0x2E5);
        assert!(k.set_county_count(2));
        k.season = 1;
        k.season_next = 2;
        k.year = 1268;
        k.options = Options { armies_eat: false, ..Options::default() };

        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
        }
        for x in 0..64u8 {
            map.set_flags(x, 10, flags::ROAD);
        }
        k.campaign.map = map;

        for id in 1..=2 {
            k.counties[id].population = 500;
            k.counties[id].happiness = 70;
        }
        k.counties[1].owner = 1;
        k.counties[1].anchor_x = 10;
        k.counties[1].anchor_y = 10;
        k.counties[2].anchor_x = 50;
        k.counties[2].anchor_y = 10;
        k.realms[1].in_play = true;
        k.realms[1].is_human = true;
        k
    }

    fn army(k: &mut Kingdom, owner: u8, x: u8, y: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, x, y);
        u.men = 100;
        u.troops[0] = 100;
        u.county = k.campaign.map.county_at(x, y);
        u.owner_is_human = k.realms[owner as usize].is_human;
        k.campaign.units.spawn(u).expect("a free slot")
    }

    /// A unit walks **one tile per crossing, and a crossing on a road is eight
    /// ticks** — not to exhaustion, and not one tile a tick.
    ///
    /// **This test used to be called `a_unit_enters_one_tile_a_tick` and it
    /// asserted the defect**, in its name, its doc comment and its numbers:
    /// *"fifteen points on a road is fifteen tiles, and it takes fifteen
    /// ticks."* Fifteen tiles is right and fifteen ticks was the whole of
    /// `docs/decisions.md` **C134** — the missing half of
    /// `Unit_StepOnce` (`0x0046634D`), which admits every tick on a road and
    /// needs **eight** admissions to cross a sixteen-wide tile. It is left
/// here because a test that has to be rewritten to
    /// make a fix pass is the strongest evidence that the fix is a change in
    /// behaviour and not a tidy-up.
    ///
    /// The first tile is entered on the tick the order is walked, because
    /// `Unit_Spawn` (`0x0046E1B0`) leaves `+0x14B` bit 0 set, so the arrivals
    /// fall on ticks 1, 9, 17 … and the fifteenth on 113.
    ///
    /// **The 8 is typed here, not read from [`crate::tables::SUBTILE_SPAN`] or
    /// [`crate::tables::SUBTILE_STEP_SOLO`]**, so that ablating either of those
    /// constants cannot move this probe with it — `docs/agents.md`, *ablating a
    /// constant while computing your probe from that same constant tests
    /// nothing at all*.
    #[test]
    fn a_unit_enters_one_tile_every_eight_ticks_on_a_road() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (20, 10), movement::Routing::Direct)
            .expect("a road runs the whole way");

        let mut ticks = 0usize;
        // (the tick it happened on, the tile it arrived at) — one entry per
// tile entered, so a stall then a sprint cannot satisfy it.
        let mut arrivals: Vec<(usize, u8)> = Vec::new();
        while k.units_moving(UnitKind::Army) && ticks < 1000 {
            let t = k.tick_units();
            ticks += 1;
            assert!(t.stepped <= 1, "one unit, one tile");
            let x = k.campaign.units.get(id).unwrap().x;
            if arrivals.last().map(|&(_, px)| px) != Some(x) {
                arrivals.push((ticks, x));
            }
        }
        assert_eq!(k.campaign.units.get(id).unwrap().x, 20);
        assert_eq!(
            arrivals.iter().map(|&(_, x)| x).collect::<Vec<u8>>(),
            (6..=20).collect::<Vec<u8>>(),
            "fifteen tiles, in order"
        );
        assert_eq!(
            arrivals.iter().map(|&(t, _)| t).collect::<Vec<usize>>(),
            (0..15).map(|n| 1 + n * 8).collect::<Vec<usize>>(),
            "eight ticks a road tile, and the first one free"
        );
        assert_eq!(k.campaign.units.get(id).unwrap().moves_used, 15);
    }

    /// **`Unit_EnterCounty`'s owner test**, as the report carries it: an army
    /// leaving its own county for a neutral one is an incursion, the same army
    /// walking home is not, and a merchant — whose tick never calls
    /// `Unit_EnterCounty` — crossing the same border is not either.
    ///
    /// Ablation: delete the `u.kind == UnitKind::Army` test and the merchant
    /// line goes red; flip `!=` and all three do.
    #[test]
    fn an_army_crossing_into_a_county_its_owner_does_not_hold_is_an_incursion() {
        let mut k = kingdom();
        let out = army(&mut k, 1, 30, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, out, (34, 10), movement::Routing::Direct)
            .unwrap();
        let mut seen = Vec::new();
        for _ in 0..200 {
            seen.extend(k.tick_units().incursions);
        }
        assert_eq!(seen, vec![Incursion { unit: out, owner: 1, county: 2 }]);

        movement::order_move(&k.campaign.map, &mut k.campaign.units, out, (30, 10), movement::Routing::Direct)
            .unwrap();
        let mut home = Vec::new();
        for _ in 0..200 {
            home.extend(k.tick_units().incursions);
        }
        assert_eq!(k.campaign.units.get(out).unwrap().county, 1, "it walked home");
        assert!(home.is_empty(), "county 1 is realm 1's: {home:?}");

        let mut trader = Unit::new(UnitKind::Merchant, 6, 30, 12);
        trader.county = 1;
        let t = k.campaign.units.spawn(trader).unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, t, (34, 12), movement::Routing::Direct)
            .unwrap();
        let mut carts = Vec::new();
        for _ in 0..400 {
            carts.extend(k.tick_units().incursions);
        }
        assert_eq!(k.campaign.units.get(t).unwrap().county, 2, "the merchant crossed");
        assert!(carts.is_empty(), "only Army_Tick calls Unit_EnterCounty: {carts:?}");
    }

    /// **The sweep reports `Unit_EnterCounty`'s letter at the crossing and
    /// `County_ChangeOwner`'s at the town**, in that order.
    ///
    /// Ablation: delete the `out.posted.push(Posted::Letter(..))` in `step_one`
    /// and the greeting goes; delete the capture's and the second entry does.
    #[test]
    fn crossing_into_a_neutral_county_posts_its_greeting_and_taking_its_town_posts_the_capture() {
        let mut k = kingdom();
        k.counties[2].happiness = 5;
        for (a, b) in [(1usize, 2u8), (2, 1)] {
            k.counties[a].neighbour_count = 1;
            k.counties[a].neighbours[0] = b;
        }
        k.realms[1].peak_counties = 1;
        k.campaign.map.set_flags(40, 10, flags::CASTLE);
        let out = army(&mut k, 1, 30, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, out, (40, 10), movement::Routing::Direct)
            .unwrap();
        let mut posted = Vec::new();
        for _ in 0..400 {
            posted.extend(k.tick_units().posted);
        }
        assert_eq!(posted.len(), 2, "{posted:?}");
        let Posted::Letter(greeting) = posted[0] else { panic!("{posted:?}") };
        assert_eq!((greeting.to, greeting.group, greeting.county), (1, 0x82, 2), "wretched");
        let Posted::Capture(capture) = posted[1] else { panic!("{posted:?}") };
        assert_eq!((capture.new_owner, capture.old_owner, capture.county), (1, 0, 2));
        assert_eq!((capture.held_before, capture.peak_before), (1, 1));
        assert_eq!(k.counties[2].owner, 1, "a wretched county surrenders");
    }

    /// The phase wait is answered by the unit array, and it goes false exactly
    /// when the last unit stops.
    #[test]
    fn the_wait_follows_the_units() {
        let mut k = kingdom();
        assert!(!k.units_moving(UnitKind::Army));
        let id = army(&mut k, 1, 5, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (8, 10), movement::Routing::Direct)
            .unwrap();
        assert!(k.units_moving(UnitKind::Army));
        // **This used to be `for _ in 0..10`**, which was long enough when a
// It is a `while`
        // bigger number on purpose: a fixed count that happens to be large
        // enough asserts nothing about *when* the wait drops, and the count is
        // what rots the next time the pacing moves. 5 → 8 is three road tiles
        // — the first on the tick the order is walked, eight for each of the
        // two after it — **and then eight more to cross the last one.**
        //
        // **This was 17, and 17 was the tick the last tile was *entered*.**
        // `Unit_Step` (`0x00465D28`) does not stop a unit on the commit that
        // empties its path: the commit returns 1, `moving` stays 2, the unit
        // crosses into the tile over the next eight admissions, and only at
        // that tile's edge does the latched arm find `field_0x1c == 0` and
        // write `moving = 0`. Ours stopped it on the commit, which left every
        // finished march parked at `+0x149 = 1` — one whole tile back from
        // where it stood, the moment the walk tables drew it. Typed, not
        // computed: 1 + 8 + 8 + 8.
        let mut ticks = 0usize;
        while k.units_moving(UnitKind::Army) && ticks < 500 {
            k.tick_units();
            ticks += 1;
        }
        assert!(!k.units_moving(UnitKind::Army));
        assert_eq!(ticks, 25, "the wait drops on the tick the last tile is crossed, not the tick it is entered");
        assert_eq!(k.campaign.units.get(id).unwrap().sub_tile, 0, "and the army stands at rest, not a tile back");
        assert_eq!(k.campaign.units.get(id).unwrap().tile(), (8, 10));
    }

/// Two enemy armies meet and the driver reports a battle
    /// resolving one — and the sweep stops there.
    #[test]
    fn two_enemy_armies_meeting_raise_a_battle_and_stop_the_sweep() {
        let mut k = kingdom();
        k.realms[2].in_play = true;
        let a = army(&mut k, 1, 5, 10);
        let _b = army(&mut k, 2, 6, 10);
        let c = army(&mut k, 1, 20, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, c, (24, 10), movement::Routing::Direct)
            .unwrap();

        let t = k.tick_units();
        let e = t.battle().expect("a battle is due");
        assert_eq!(e.mover, a);
        assert_eq!(e.occupant, _b);
        assert_eq!(e.county, 1);
        assert_eq!(k.campaign.units.get(a).unwrap().tile(), (5, 10), "and nobody moved onto it");
        assert_eq!(
            k.campaign.units.get(c).unwrap().tile(),
            (20, 10),
            "the higher slot did not move at all: the sweep was abandoned"
        );
    }

    /// Same owner is not a battle. It is not a merge here either — see
    /// `Contact::Blocked`.
    #[test]
    fn a_friendly_unit_blocks_rather_than_fights() {
        let mut k = kingdom();
        let a = army(&mut k, 1, 5, 10);
        let b = army(&mut k, 1, 6, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        let t = k.tick_units();
        assert_eq!(t.battle(), None);
        assert_eq!(t.contacts, vec![Contact::Blocked { mover: a, occupant: b }]);
    }

    /// An ally is walked into no more than an enemy is fought.
    #[test]
    fn an_ally_is_not_attacked() {
        let mut k = kingdom();
        k.realms[2].in_play = true;
        k.realms[1].ally = 2;
        let a = army(&mut k, 1, 5, 10);
        let b = army(&mut k, 2, 6, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        let t = k.tick_units();
        assert_eq!(t.battle(), None);
        assert_eq!(t.contacts, vec![Contact::Blocked { mover: a, occupant: b }]);
    }

    /// A merchant is never attacked, whoever it belongs to — the second rung of
    /// `Unit_EnterOccupiedTile`'s ladder.
    ///
    /// **Corrected, and the assertion reversed.** This used to expect
/// `Contact::Blocked`, on the reading that a merchant cannot be
    /// *fought*. The rung says `return local_8`, and `local_8` is the ordinary
    /// Road or Open code — so the army walks *through* the merchant and no
    /// contact is reported at all. [`crate::movement::pass_through`] is where
    /// that now happens, which makes rungs 1 and 2 of
/// [`UnitsTick::classify_occupied`] unreachable by construction
    /// by comment. `docs/decisions.md` C40.
    #[test]
    fn a_merchant_is_walked_through_rather_than_attacked() {
        let mut k = kingdom();
        k.realms[2].in_play = true;
        let a = army(&mut k, 1, 5, 10);
        let mut m = Unit::new(UnitKind::Merchant, OWNERLESS, 6, 10);
        m.county = 1;
        k.campaign.units.spawn(m).unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        let t = k.tick_units();
        assert_eq!(t.battle(), None);
        assert_eq!(t.contacts, vec![], "no fight, and no obstacle either");
        assert_eq!(k.campaign.units.get(a).unwrap().tile(), (6, 10), "the army is on the tile");
    }

    /// Phase 2 starts nothing. The whole point of the module's headline.
    #[test]
    fn phase_two_originates_no_movement() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        assert_eq!(k.begin_unit_phase(Phase::ArmyMovement), 0);
        assert!(!k.campaign.units.get(id).unwrap().moving);
    }

    /// Phase 3 re-targets a transport at its **cargo county's anchor**, every
    /// turn, whether or not it was already going somewhere.
    #[test]
    fn phase_three_sends_every_transport_to_its_cargo_county_anchor() {
        let mut k = kingdom();
        let mut t = Unit::new(UnitKind::Transport, 1, 5, 10);
        t.county = 1;
        t.cargo_county = 2;
        t.needs_destination = false;
        t.dest = Some((9, 9));
        let id = k.campaign.units.spawn(t).unwrap();

        assert_eq!(k.begin_unit_phase(Phase::SupplyTransports), 1);
        let u = k.campaign.units.get(id).unwrap();
        assert_eq!(u.dest, Some((50, 10)), "county 2's anchor, not a tile near it");
        assert_eq!(u.dest_county, 2);
        assert!(u.moving);
    }

    /// Phase 5's cursor is shared, so two mobs are sent to two different
    /// counties — and neither is sent to the county it is standing in.
    #[test]
    fn phase_five_walks_one_cursor_for_every_mob() {
        let mut k = kingdom();
        for (x, county) in [(5u8, 1u8), (50, 2)] {
            let mut m = Unit::new(UnitKind::PeasantMob, OWNERLESS, x, 10);
            m.county = county;
            m.men = 200;
            k.campaign.units.spawn(m).unwrap();
        }
        k.season = MOB_RETARGET_SEASON;
        k.begin_unit_phase(Phase::PeasantMobs);

        let dests: Vec<u8> =
            ids_of_kind(&k.campaign.units, UnitKind::PeasantMob)
                .iter()
                .map(|&id| k.campaign.units.get(id).unwrap().dest_county)
                .collect();
        assert_eq!(dests, vec![2, 1], "the cursor advanced between them");
    }

    /// Only realm 6's mobs hold phase 5 open — `FUN_004A4E3D(2, 6)`.
    #[test]
    fn a_mob_belonging_to_a_realm_does_not_hold_phase_five_open() {
        let mut k = kingdom();
        let mut m = Unit::new(UnitKind::PeasantMob, 1, 5, 10);
        m.county = 1;
        m.moving = true;
        k.campaign.units.spawn(m).unwrap();
        assert!(!k.units_moving(UnitKind::PeasantMob));

        let mut m = Unit::new(UnitKind::PeasantMob, OWNERLESS, 6, 10);
        m.county = 1;
        m.moving = true;
        k.campaign.units.spawn(m).unwrap();
        assert!(k.units_moving(UnitKind::PeasantMob));
    }

    /// A unit that cannot move has its flag cleared, or the phase waiting on it
    /// never ends. This is the failure mode the whole driver has to avoid.
    #[test]
    fn a_unit_with_no_path_stops_being_moving() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        k.campaign.units.get_mut(id).unwrap().moving = true;
        assert!(k.units_moving(UnitKind::Army));
        k.tick_units();
        assert!(!k.units_moving(UnitKind::Army), "no path is not a reason to wait for ever");
        assert!(k.campaign.units.get(id).unwrap().needs_destination);
    }

    /// The allowance is derived from the type on every tick, so a unit that
    /// arrives from a save with a zero in it still walks.
    #[test]
    fn the_allowance_is_rebuilt_from_the_type() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        k.campaign.units.get_mut(id).unwrap().move_allowance = 0;
        refresh_allowances(&mut k.campaign.units);
        assert_eq!(k.campaign.units.get(id).unwrap().move_allowance, 15);
    }
}
