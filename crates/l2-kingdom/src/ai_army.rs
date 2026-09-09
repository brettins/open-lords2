//! **The AI's war** — turn steps 4, 7, 9, 10 and 11, and the six unit missions
//! step 11 dispatches on.
//!
//! [`crate::ai`] is the AI's *economy*: taxes, grants, castles, industry. This
//! is everything the AI does with an army, and until it existed no AI realm
//! had ever raised one.
//!
//! # The constraint that had expired
//!
//! `ai.rs` used to say of these steps that they *"drive armies, merchants,
//! diplomacy and map tiles, none of which `l2-kingdom` owns — reproducing them
//! here would mean inventing a unit model to hang them on"*. The crate owns
//! [`crate::unit`], [`crate::movement`], [`crate::levy`], [`crate::map`] and
//! [`crate::merchant`], so there is nothing left to invent; the sentence was a
//! decision that had become a description of a world that had gone.
//! `docs/plan.md` §2.4.
//!
//! # The five steps
//!
//! | step | address | what it is | here |
//! |---:|---|---|---|
//! | 4 | `0x0049E1BF` | what the realm wants to buy | [`resource_wants`] |
//! | 7 | `0x0049F93D` | three passes: pick the muster county, top up the castle garrisons, hold or abandon the frontier | [`Kingdom::run_ai_armies`] |
//! | 9 | `0x0049F977` | raise the main army and aim it | [`Kingdom::run_ai_raise_army`] |
//! | 10 | `0x004A0015` | send a raiding party at somebody's crops | [`Kingdom::run_ai_raid`] |
//! | 11 | `0x004A5667` | run every army's mission and re-path it | [`Kingdom::run_ai_move_armies`] |
//!
//! # `docs/kingdom.md` §3.2 is wrong about step 10, and it is one digit
//!
//! §3.2 calls step 10 *"create a **type-7 unit** and send it somewhere"*, and
//! `ai.rs` copied that as *"needs the unit **mission** byte"* without noticing
//! the two were the same claim. **There is no unit kind 7** —
//! [`crate::unit::UnitKind`] has four and `g_unitTickTable` has four handlers.
//! `FUN_004A0015` calls `FUN_004A5003`, which calls `Army_Create`, which
//! spawns a **type-1 army**; the 7 goes into `+0x1A`, the *mission*. `[D]`
//!
//! # The mission byte, and what `docs/armies.md` had closed
//!
//! `docs/armies.md` §1.5 and §8b.2 close unit `+0x1A` as *"the garrison
//! feedback code"*, on the evidence that `Army_GarrisonApply` writes 5 on
//! success and 2 on refusal and that `battle-before.sav`'s garrison carries a
//! 5. Every one of those observations is right and the conclusion is too
//! narrow: `+0x1A` is a **six-value mission enum** that the AI's whole army
//! driver dispatches on, and garrisoning is two of the six values.
//! [`Mission`] is the enum. `[D]`
//!
//! # Five personality fields traced
//!
//! `docs/diplomacy.md` §8.4 lists eleven personality fields that *"hold
//! plausible per-lord values and were not traced"*. Reading these five steps
//! closes three more of them — `+0x70`, `+0x74` and `+0x9C` — and turns two
//! that were named on a guess (`+0x28`, `+0x68`) into claims with a reader.
//! See [`crate::tables::AI_PERSONALITY_GARRISON_MIN_POPULATION`] and its
//! neighbours. It also corrects §8.4's *"a threshold on realm `+0x38`"*: the
//! field is `+0x138`, the realm's **total weapon stock**, and `+0x38` is
//! inside the army-name counters.
//!
//! # Two seams, named rather than silently skipped
//!
//! * **The evacuation.** Step 7's third pass ships a written-off county's
//!   grain and herd to the muster county with `Transport_Spawn`, or sells them
//!   where the county has a merchant stall. This crate has neither
//!   `Transport_Deliver` nor a county stall field, and a transport that can
//!   never unload would **delete** the county's food from the game — a worse
//!   reproduction than not evacuating. So the pass returns what it wanted to
//!   move, as [`Evacuation`], and moves nothing. Everything else in the pass —
//!   the levy, the punitive tax rate, the industry share — happens.
//! * **The garrison eviction's battle.** `FUN_00437535` starts a battle when
//!   the evicted garrison had a besieger. [`Kingdom::run_ai_move_armies`]
//!   reports the pair instead of starting one, for the same reason
//!   [`crate::ai::taunt`] returns its letters.
//!
//! # Determinism
//!
//! Every scan here is an ascending index or an ascending tile offset with a
//! **strict** `<` comparison, so a tie goes to the lowest id — which is the
//! original's own tie-break and is what `docs/netcode.md` §5 requires anyway.
//! Nothing draws a random number: the AI's war is entirely deterministic in
//! the shipped game, which is worth knowing before anyone reaches for the
//! generator to make it less predictable.

use crate::county::{County, MAX_COUNTIES};
use crate::kingdom::Kingdom;
use crate::levy::{self, LevyBasket, Muster};
use crate::map::{flags, CampaignMap, MAP_DIM};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{TroopType, UnitKind, Units};

/// Unit `+0x1A` — **what an army is currently trying to do.**
///
/// `FUN_004A57AC` dispatches on it and rewrites **any** value it does not
/// recognise to [`Mission::SEEK_ENEMY`], returning 0 that turn — so a unit
/// with mission 0 or 1 loses one turn and then behaves as an attacker. That
/// normalisation is why [`crate::unit::Unit::mission`] is a raw byte and not a
/// Rust enum.
pub struct Mission;

impl Mission {
    /// **2 — seek the enemy.** The default and the one every unrecognised
    /// value becomes: intercept an enemy army within 5 tiles, else march on a
    /// hostile county. `FUN_004A5B1F`.
    pub const SEEK_ENEMY: u8 = 2;
    /// **3 — hold the home county.** Intercept anything hostile standing in
    /// [`crate::unit::Unit::home_county`] within 30 tiles, else walk back to it.
    /// `FUN_004A5F0A`.
    pub const HOLD_HOME: u8 = 3;
    /// **4 — go and join a garrison.** Walk to a friendly castle with room and
    /// step onto its tile. `FUN_004A6270`.
    pub const JOIN_GARRISON: u8 = 4;
    /// **5 — in garrison.** `Army_GarrisonApply` writes this on success and
    /// [`Mission::SEEK_ENEMY`] on refusal, which is the pair `docs/armies.md`
    /// §8b.2 read as the whole meaning of the byte. `FUN_004A60B9`.
    pub const GARRISON: u8 = 5;
    /// **6 — relieve an ally.** March on the county the ally asked about,
    /// while the ally still owns it and there are still enemies in it.
    /// `FUN_004A599D`.
    pub const ASSIST_ALLY: u8 = 6;
    /// **7 — raid.** Walk at the nearest **standing crop** in the target
    /// county, which is what makes this a raid: `Unit_TrampleTile` fires on
    /// every field an army crosses in a county it does not own.
    /// `FUN_004A58F4`.
    pub const RAID: u8 = 7;
}

/// The intercept radius of [`Mission::SEEK_ENEMY`] — Chebyshev `< 6`.
pub const SEEK_ENEMY_RADIUS: i32 = 6;
/// The intercept radius of [`Mission::HOLD_HOME`] — Chebyshev `< 31`. Five
/// times the attacker's, which is what makes a defender leave its post for
/// something it would otherwise ignore.
pub const HOLD_HOME_RADIUS: i32 = 31;
/// The radius [`Mission::ASSIST_ALLY`] closes on the enemy in the ally's
/// county — Chebyshev `< 21`.
pub const ASSIST_ALLY_RADIUS: i32 = 21;

/// The distance both county choosers start from, so a county scoring `0x80` or
/// worse is never chosen at all.
pub const TARGET_SCORE_CEILING: i32 = 0x80;
/// What a **defended** castle adds to a county's target score. It is 40 on a
/// Chebyshev distance across a 64-tile map, so it is very nearly a veto.
pub const DEFENDED_CASTLE_PENALTY: i32 = 40;

/// The smallest army [`Kingdom::run_ai_raise_army`] will divert instead of
/// raising a new one — `FUN_004A0917`'s third argument.
pub const DIVERT_MIN_MEN: i32 = 200;
/// What standing in `FUN_004A0917`'s score being in the wanted county is worth.
pub const DIVERT_IN_COUNTY: i32 = 300;
/// …and being next door to it.
pub const DIVERT_NEXT_DOOR: i32 = 100;

/// The muster county must have more than this many people, and more than
/// [`RAID_MIN_HAPPINESS`] happiness, before a raid is sent from it.
pub const RAID_MIN_POPULATION: i32 = 399;
/// See [`RAID_MIN_POPULATION`].
pub const RAID_MIN_HAPPINESS: i32 = 24;
/// How many men a raiding party is — `PctOf(50, population)` men, which is
/// **fifty men expressed as a percentage of the county** and then taken back
/// out of it, so integer truncation makes it fifty-ish whatever the county's
/// size. `FUN_004A5003`.
pub const RAID_MEN: i32 = 50;

/// A rival must be **strictly below** this standing before the AI will raid
/// it, and the lowest such standing wins. `FUN_004A0309` seeds its running
/// best with the threshold itself, which is what makes it strict.
pub const RAID_STANDING_THRESHOLD: i8 = -10;

/// The share of the map the leading realm must hold before the others treat it
/// as *the* threat — `FUN_004A01EA`'s `0x28`.
pub const THREAT_SHARE_PCT: i32 = 40;

/// A castle's garrison must be short by more than this before the AI raises
/// men for it, and the shortfall must be under
/// [`GARRISON_GAP_ABANDON_PCT`] of the county's population.
pub const GARRISON_GAP_MIN: i32 = 19;
/// A shortfall this large a share of the county is not attempted at all.
pub const GARRISON_GAP_ABANDON_PCT: i32 = 80;
/// The realm needs more than this many bows *plus* crossbows before it will
/// garrison anything — `weapons[4] + weapons[0] > 0x13`.
pub const GARRISON_MIN_MISSILE_STOCK: i32 = 19;

/// `FUN_0049F431`'s two thresholds: an enemy force in one of the realm's
/// counties is only *noticed* above this…
pub const FRONTIER_ENEMY_MIN: i32 = 99;
/// …and only acted on when the realm's own men there fall this far behind it.
pub const FRONTIER_DEFICIT: i32 = 100;
/// A county below this population is written off outright — the branch that
/// forces the comparison threshold to zero.
pub const FRONTIER_MIN_POPULATION: i32 = 200;
/// The share of the county's people either branch of the frontier pass levies.
pub const FRONTIER_LEVY_PCT: i32 = 50;
/// The population floor `FUN_0049F431` passes `FUN_004A50AE`. **Not
/// [`crate::levy::DEFENCE_MIN_POPULATION`]**, which is the 40 the *invasion*
/// path passes: the same function, a different argument, and hardcoding it was
/// this crate's simplification rather than the original's rule.
pub const FRONTIER_LEVY_MIN_POPULATION: i32 = 100;

/// The order `FUN_004A5389` hands weapons out in: bows, crossbows, swords,
/// maces, mail — **and never pikes.**
///
/// This is a *greedy* fill and not [`LevyBasket::auto_equip`]'s round-robin:
/// each type in turn arms as many still-unarmed levies as the stock allows, so
/// a realm with a thousand bows sends a garrison of pure archers. Pikemen are
/// the one equipped type the pass skips, which reads as doctrine — a garrison
/// wants missiles and a pike is a field weapon against cavalry — and is stated
/// as an observation rather than an explanation. `[D]`
pub const GARRISON_EQUIP_ORDER: [TroopType; 5] = [
    TroopType::Archer,
    TroopType::Crossbowman,
    TroopType::Swordsman,
    TroopType::Maceman,
    TroopType::Knight,
];

// ---------------------------------------------------------------------------
// Step 4 — what the realm wants to buy
// ---------------------------------------------------------------------------

/// `FUN_0049D5E0` — the realm's **lowest-numbered** county, or 0.
pub fn first_owned_county(counties: &[County; MAX_COUNTIES], county_count: usize, realm: u8) -> u8 {
    (1..=county_count.min(MAX_COUNTIES - 1))
        .find(|&id| counties[id].owner == realm)
        .unwrap_or(0) as u8
}

/// Step 4 — `FUN_0049E1BF`, which fills realm `+0x70 … +0x7C`.
///
/// ```c
/// for (i = 0; i < 4; i++) realm.want[i] = 0;
/// if (realm.countyCount > 1
///     || ((c = firstOwnedCounty(realm)) > 0 && c.happiness > 14 && c.healthMeter > 19)) {
///     if (realm.iron < 50)  realm.want[iron] = 50;
///     if (realm.wood < 100) realm.want[wood] = 100;
///     for every owned county with a castle and work in progress:
///         realm.want[wood]  += county[+0x1D4];   /* wood the work still needs */
///         realm.want[stone] += county[+0x1D0];   /* stone */
/// }
/// ```
///
/// Two things worth having in the model rather than the head:
///
/// * **The gate is the whole point.** A realm with **one** county buys nothing
///   at all unless that county is above 14 happiness *and* above 19 on the
///   health meter — a realm down to one starving county stops shopping, which
///   is the same shape as the resource grants
///   ([`crate::ai::grant_resources`]) abandoning a realm that is losing.
/// * **Slot 2 is zeroed every turn and never written.** The loop that clears
///   four words is the only thing that touches it. Reproduced.
///
/// **One seam.** The per-county term reads the wood and stone a castle build
/// still owes, and this crate has no such counter:
/// [`crate::industry::order_castle`] debits the whole cost up front, exactly
/// as [`crate::ai::choose_industry`] records for the same pair of fields. So
/// the term evaluates to zero here and the wants are the two floors. `[I]`, and
/// it is the up-front debit that makes it so rather than anything read out of
/// the binary.
///
/// The consumer is `Ai_TradeForCounty` (`0x0049E39B`), which
/// [`crate::ai_farm`] records as unimplemented — so this step changes no
/// behaviour today. It is here because it is one of the fourteen and because
/// the *gate* is a rule, not because anything reads the answer yet.
pub fn resource_wants(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realms: &mut [Realm; MAX_REALMS],
    realm_id: u8,
) {
    let _ = t;
    let Some(realm) = realms.get_mut(realm_id as usize) else { return };
    realm.want = [0; 4];
    if realm.county_count <= 1 {
        let first = first_owned_county(counties, county_count, realm_id);
        if first == 0 {
            return;
        }
        let c = &counties[first as usize];
        if c.happiness <= WANT_MIN_HAPPINESS || c.health_meter <= WANT_MIN_HEALTH {
            return;
        }
    }
    if realm.iron < WANT_IRON_FLOOR {
        realm.want[WANT_IRON] = WANT_IRON_FLOOR;
    }
    if realm.wood < WANT_WOOD_FLOOR {
        realm.want[WANT_WOOD] = WANT_WOOD_FLOOR;
    }
    // The per-county term is the seam above: with the castle cost debited up
    // front there is nothing outstanding to add.
}

/// Realm `+0x70` — wood.
pub const WANT_WOOD: usize = 0;
/// Realm `+0x74` — iron.
pub const WANT_IRON: usize = 1;
/// Realm `+0x78` — **zeroed every turn and never written.**
pub const WANT_UNUSED: usize = 2;
/// Realm `+0x7C` — stone.
pub const WANT_STONE: usize = 3;
/// The iron floor below which a one-county realm starts buying.
pub const WANT_IRON_FLOOR: i32 = 50;
/// The wood floor.
pub const WANT_WOOD_FLOOR: i32 = 100;
/// A one-county realm below this happiness buys nothing.
pub const WANT_MIN_HAPPINESS: i32 = 14;
/// …or below this on the health meter.
pub const WANT_MIN_HEALTH: i32 = 19;

// ---------------------------------------------------------------------------
// Step 7, pass 1 — where to muster
// ---------------------------------------------------------------------------

/// `FUN_004A0AAA`'s score for one county, as `(muster, raid)`.
///
/// ```c
/// s = (population / 500) * 2;
/// if      (happiness < 40) s -= 1;
/// else if (happiness > 80) s += 1;
/// if (foodAvailable < population)   { r = s + 2; s -= 2; }
/// else if (population * 4 < food)   { r = s - 2; s += 1; }
/// else                                r = s;
/// ```
///
/// **The two outputs take the food term in opposite directions**, and that is
/// the finding. The county the realm musters its main army from is the one
/// with **food to spare**; the county it raids *out of* is the one whose
/// larder is already short. A hungry county is where you send the surplus
/// mouths, and a fat one is where you raise the army that has to eat.
///
/// The population term is a *step*: `(population / 500) * 2` is 0 below 500,
/// 2 below 1000, 4 below 1500. A county of 999 people scores exactly what a
/// county of 500 does.
pub fn muster_score(t: &Tables, county: &County) -> (i32, i32) {
    let food = crate::ration::food_available(t, county);
    let mut s = (county.population / 500) * 2;
    if county.happiness < 40 {
        s -= 1;
    } else if county.happiness > 80 {
        s += 1;
    }
    let r;
    if food < county.population {
        r = s + 2;
        s -= 2;
    } else if county.population * 4 < food {
        r = s - 2;
        s += 1;
    } else {
        r = s;
    }
    (s, r)
}

/// Step 7's first pass — `FUN_004A0AAA`, which writes realm `+0xE5` and
/// `+0xE6`.
///
/// Returns `(muster county, raid county)`, either of which is **0 when no
/// county scores above zero**: both running maxima start at 0 and the
/// comparison is `>=`, so a realm every one of whose counties scores negative
/// musters nowhere. That is reachable — one small unhappy county with an empty
/// larder scores `0 - 1 - 2 = -3` — and it is how a realm on its last legs
/// stops raising armies.
///
/// The `>=` also means the **last** county on the highest score wins, where
/// every other scan in this module gives it to the first. Reproduced.
pub fn choose_muster_counties(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realm_id: u8,
) -> (u8, u8) {
    let (mut best, mut best_score) = (0u8, 0i32);
    let (mut raid, mut raid_score) = (0u8, 0i32);
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        if counties[id].owner != realm_id {
            continue;
        }
        let (s, r) = muster_score(t, &counties[id]);
        if s >= best_score {
            best_score = s;
            best = id as u8;
        }
        if r >= raid_score {
            raid_score = r;
            raid = id as u8;
        }
    }
    (best, raid)
}

// ---------------------------------------------------------------------------
// Step 7, pass 2 — the castle garrisons
// ---------------------------------------------------------------------------

/// `FUN_0049F12F`'s sizing ladder: how much of a castle's garrison shortfall
/// the AI raises in one go, given the shortfall as a percentage of the
/// county's population.
///
/// `None` means the pass gives up on the county this turn.
///
/// | shortfall as % of the county | raised |
/// |---|---|
/// | 80 or more | **nothing at all** |
/// | 65 … 79 | a fifth of it |
/// | 50 … 64 | a quarter |
/// | 30 … 49 | a third |
/// | 15 … 29 | a half |
/// | 14 or less | all of it |
///
/// The ladder is not a taper — it is a **cap**. Feed it any shortfall and the
/// number that comes out is between 13 % and 15 % of the county's population,
/// so a castle whose garrison is nearly empty is filled a slice at a time over
/// several turns rather than by conscripting the whole county at once.
pub fn garrison_levy_share(gap: i32, population: i32) -> Option<i32> {
    let share = crate::industry::pct_of(gap, population);
    if share >= GARRISON_GAP_ABANDON_PCT {
        return None;
    }
    Some(if share >= 65 {
        gap / 5
    } else if share >= 50 {
        gap / 4
    } else if share >= 30 {
        gap / 3
    } else if share >= 15 {
        gap / 2
    } else {
        gap
    })
}

// ---------------------------------------------------------------------------
// Step 7, pass 3 — hold the frontier or write it off
// ---------------------------------------------------------------------------

/// The men standing in a county that belong to `realm`, or that do not belong
/// to `realm` or its ally — `FUN_0049F850` and `FUN_0049F73F`.
///
/// Both walk every unit slot, take only **armies** (`kind == 1`) with a
/// non-zero owner, and ignore whether the unit is garrisoned or besieging —
/// unlike [`crate::unit::Units::recount_county_troops`], which excludes a
/// garrison. So a castle's garrison counts toward its own realm's total here.
/// `[D]`
pub fn men_in_county(units: &Units, realms: &[Realm; MAX_REALMS], county: u8, realm: u8, hostile: bool) -> i32 {
    let ally = realms.get(realm as usize).map_or(0, |r| r.ally);
    let mut men = 0;
    for (_, u) in units.iter() {
        if u.kind != UnitKind::Army || u.owner == 0 || u.county != county {
            continue;
        }
        let theirs = u.owner != realm && u.owner != ally;
        if theirs == hostile {
            men += u.men;
        }
    }
    men
}

/// What step 7's third pass wanted to move out of a county it has written off.
///
/// **Nothing is moved.** See the module documentation's *seams*: without
/// `Transport_Deliver` a spawned transport would delete the county's food from
/// the game, and without a county stall field the sale branch has no market.
/// The pass returns its intent so that neither is silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evacuation {
    /// The county being written off.
    pub from: u8,
    /// Where the goods were bound — the realm's muster county.
    pub to: u8,
    pub grain: i32,
    pub herd: i32,
}

/// The evacuation only happens at all when there is something worth moving:
/// `grain > 10 || herd > 5`.
pub fn worth_evacuating(county: &County) -> bool {
    county.grain > 10 || county.herd > 5
}

// ---------------------------------------------------------------------------
// Step 9 — the main army
// ---------------------------------------------------------------------------

/// `FUN_004A01EA` — realm `+0x48`, **who the realm has decided is the threat**.
///
/// ```c
/// realm.threat = 0;
/// if (realm.warTarget != 0) { realm.threat = realm.warTarget; return; }
/// for (i = 1; i < 6; i++) if (realms[i].rank < 2) {
///     if (i == self)                       return;
///     if (realms[i].strength == 0)         return;
///     if (realms[i].shareOfMapPct < 40)    return;
///     realm.threat = i;
/// }
/// ```
///
/// **`rank < 2` is true of rank 0 as well as rank 1**, and rank 0 is what
/// [`crate::ai::rank_realms`] leaves on a realm that is *not in play*. So the
/// loop stops at the first eliminated realm below the leader's index and the
/// realm ends up with no threat at all. On a map where realm 1 is knocked out
/// early, **nobody ever gangs up on the leader again.** Reproduced, and
/// stated here because it looks like a transcription slip and is not: the
/// early `return`s are in the disassembly.
///
/// Note also that the threat is only ever the realm ranked **first**, and only
/// while it holds at least [`THREAT_SHARE_PCT`] of the map. Below that the AI
/// has no preferred enemy and takes whatever is nearest.
pub fn pick_threat(realms: &[Realm; MAX_REALMS], realm_id: u8) -> u8 {
    let Some(me) = realms.get(realm_id as usize) else { return 0 };
    if me.war_target != 0 {
        return me.war_target;
    }
    let mut threat = 0;
    for i in 1..MAX_REALMS.min(6) {
        if realms[i].rank >= 2 {
            continue;
        }
        if i as u8 == realm_id || realms[i].strength == 0 || realms[i].share_of_map_pct < THREAT_SHARE_PCT
        {
            return threat;
        }
        threat = i as u8;
    }
    threat
}

/// What one county is worth attacking, from `from`'s anchor. Lower is better.
///
/// ```c
/// score = Chebyshev(from.anchor, cand.anchor);
/// if (cand has a castle AND a garrison)  score += 40;
/// herd  >= 201 -> -20 | 101..200 -> -10
/// grain: the two choosers differ, and that is the ONLY difference between them
/// ```
///
/// `wide_grain` picks which grain ladder: **true** is `FUN_004A03F2`'s, used by
/// step 9 and step 10, and **false** is `FUN_004A0649`'s, used by
/// [`Mission::SEEK_ENEMY`] when an army picks its own next target.
///
/// | grain | `FUN_004A03F2` | `FUN_004A0649` |
/// |---|---:|---:|
/// | 2501+ | −20 | −10 |
/// | 1201 … 2500 | −10 | −10 |
/// | 1001 … 1200 | −5 | −10 |
/// | 501 … 1000 | −5 | −5 |
///
/// So a **realm** planning a campaign values a full granary twice as much as a
/// wandering army does. The livestock ladder is identical in both.
pub fn target_score(from: &County, cand: &County, wide_grain: bool) -> i32 {
    let mut score = chebyshev(
        (from.anchor_x, from.anchor_y),
        (cand.anchor_x, cand.anchor_y),
    );
    if cand.castle_type != 0 && cand.garrison_unit != 0 {
        score += DEFENDED_CASTLE_PENALTY;
    }
    if cand.herd >= 201 {
        score -= 20;
    } else if cand.herd > 100 {
        score -= 10;
    }
    if wide_grain {
        if cand.grain >= 2501 {
            score -= 20;
        } else if cand.grain >= 1201 {
            score -= 10;
        } else if cand.grain > 500 {
            score -= 5;
        }
    } else if cand.grain >= 1001 {
        score -= 10;
    } else if cand.grain > 500 {
        score -= 5;
    }
    score
}

/// `Dist_Chebyshev` — the larger of the two axis distances.
pub fn chebyshev((ax, ay): (u8, u8), (bx, by): (u8, u8)) -> i32 {
    let dx = (ax as i32 - bx as i32).abs();
    let dy = (ay as i32 - by as i32).abs();
    dx.max(dy)
}

/// `Dist_Manhattan` — the sum of the two, which is what the three
/// destination-tile finders measure with. The choosers above use Chebyshev and
/// the tile finders use Manhattan; they are not interchangeable and the
/// original really does use both.
pub fn manhattan((ax, ay): (u8, u8), (bx, by): (u8, u8)) -> i32 {
    (ax as i32 - bx as i32).abs() + (ay as i32 - by as i32).abs()
}

/// `Diplo_ActionAllowed` (`0x004A16F7`) — may `actor` act against `target`?
///
/// ```c
/// if (target == 0)                       return 1;   /* nobody owns it */
/// if (realms[actor].ally == target)    { realms[actor].pairs[target].grudge++; return 0; }
/// if (actor == target)                   return 0;
/// return 1;
/// ```
///
/// **It is not a predicate, it mutates**, and that is the interesting part:
/// every time the AI's target search so much as *considers* a county its ally
/// owns, the actor's own grudge against that ally goes up by one. The search
/// runs once per county per army per turn, so an AI hemmed in by its ally
/// accumulates grudge purely by looking at the map, and
/// [`crate::tables::AI_PERSONALITY_GRUDGE_TOLERANCE`] eventually breaks the
/// alliance on its own. The Knight tolerates 5.
///
/// Note what is **not** in it: neither `atWar` nor standing. A realm may
/// attack anybody it is not allied to, at any standing at all.
pub fn action_allowed(realms: &mut [Realm; MAX_REALMS], actor: u8, target: u8) -> bool {
    if target == 0 {
        return true;
    }
    let Some(me) = realms.get_mut(actor as usize) else { return false };
    if me.ally == target {
        let pair = me.pair_mut(target);
        pair.grudge = pair.grudge.wrapping_add(1);
        return false;
    }
    actor != target
}

/// `FUN_00467EB0` — does any of `county`'s neighbours belong to `realm`?
///
/// This is the adjacency gate on every target the AI picks for its **main**
/// army: it will only march on a county that touches its own territory. A
/// raid ([`Kingdom::run_ai_raid`]) skips this gate entirely, which is why a
/// raiding party can appear a long way from the raider's border.
pub fn county_borders_realm(counties: &[County; MAX_COUNTIES], county: u8, realm: u8) -> bool {
    let Some(c) = counties.get(county as usize) else { return false };
    c.neighbours()
        .iter()
        .take_while(|&&n| n != 0)
        .any(|&n| counties.get(n as usize).is_some_and(|c| c.owner == realm))
}

/// `AI_EmergencyWeapons` (`0x0049FCC5`) — the charity that lets a cornered
/// lord muster anyway.
///
/// A realm down to **fewer than two counties**, before year **1273**, is
/// handed 100 pikes and 100 bows if its lord is the Bishop, or 100 crossbows
/// if it is the Countess. **The Knight and the Baron get nothing**, so two of
/// the four lords cannot be rescued at all and — because the grant's return
/// value is what bypasses
/// [`crate::tables::AI_PERSONALITY_MUSTER_ARMS`] — two of the four also cannot
/// muster below their weapon threshold. `[D]`, and `symbols.json` marks it
/// `[inferred]`; this is a second reader agreeing rather than new evidence.
///
/// Returns whether anything was granted.
pub fn emergency_weapons(realm: &mut Realm, year: i32) -> bool {
    if realm.county_count >= 2 || year >= EMERGENCY_WEAPONS_UNTIL_YEAR {
        return false;
    }
    match realm.lord {
        4 => {
            realm.weapons[3] += EMERGENCY_WEAPONS_GRANT;
            realm.weapons[4] += EMERGENCY_WEAPONS_GRANT;
            true
        }
        3 => {
            realm.weapons[0] += EMERGENCY_WEAPONS_GRANT;
            true
        }
        _ => false,
    }
}

/// `g_year < 0x4F9`.
pub const EMERGENCY_WEAPONS_UNTIL_YEAR: i32 = 1273;
/// How many of each type the grant hands over.
pub const EMERGENCY_WEAPONS_GRANT: i32 = 100;

/// `FUN_0049FF71` — is the ally's request still worth marching on?
///
/// Returns the county, or 0 to cancel: **no ally** cancels it, a county the
/// asking realm no longer holds *with no enemy standing in it* cancels it, and
/// a county that has become the asked realm's **own** cancels it. The middle
/// test reads [`crate::county::County::enemy_troops`] (`+0x19C`), which
/// `Army_RecountCountyTroops` maintains — so it is literally *"my ally still
/// owns it and there are still enemies in it"*.
pub fn ally_request_still_stands(
    counties: &[County; MAX_COUNTIES],
    realms: &[Realm; MAX_REALMS],
    county: u8,
    realm: u8,
) -> u8 {
    let (Some(c), Some(me)) = (counties.get(county as usize), realms.get(realm as usize)) else {
        return 0;
    };
    if me.ally == 0 {
        return 0;
    }
    if me.ally == c.owner {
        if c.enemy_troops == 0 {
            return 0;
        }
    } else if c.owner == realm {
        return 0;
    }
    county
}

/// `FUN_004A0309` — who to raid: the declared war target, else the rival this
/// realm thinks worst of, provided that is **strictly below**
/// [`RAID_STANDING_THRESHOLD`].
pub fn pick_raid_victim(realms: &[Realm; MAX_REALMS], realm_id: u8) -> u8 {
    let Some(me) = realms.get(realm_id as usize) else { return 0 };
    if me.war_target != 0 {
        return me.war_target;
    }
    let mut worst = RAID_STANDING_THRESHOLD;
    let mut victim = 0;
    for i in 1..MAX_REALMS.min(6) {
        if i as u8 == realm_id || realms[i].strength == 0 {
            continue;
        }
        let standing = me.pair(i as u8).standing;
        if standing < worst {
            worst = standing;
            victim = i as u8;
        }
    }
    victim
}

// ---------------------------------------------------------------------------
// The three destination-tile finders
// ---------------------------------------------------------------------------

/// Which tile of a county an order aims at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    /// `FUN_004A6735` — the **county town**: plane-0 `0x40`. Stepping onto it
    /// is `Army_AttackCounty`, so this is how a county is taken.
    Town,
    /// `FUN_004A65A3` — the **castle**: plane-0 `0x80` with terrain `0x15 …
    /// 0x19`, which is the five built castle types and **not** the empty
    /// `0x14` plot. Stepping onto it garrisons the army when the county is the
    /// realm's own and lays a siege when it is not.
    Castle,
    /// `FUN_004A689D` — the nearest **standing crop**: plane-0 `0x20` with
    /// terrain `3 … 22`, which is exactly
    /// [`crate::map::terrain::FIELD_STANDING_FROM`] `+1` up to
    /// [`crate::map::terrain::FIELD_STANDING_TO`] `−1`. Every field an army
    /// crosses in a county it does not own is trampled, so **a raid is aimed
    /// at the harvest** and does its damage on the way in.
    StandingCrop,
}

impl Aim {
    fn accepts(self, terrain: u8, tile_flags: u8) -> bool {
        match self {
            Aim::Town => tile_flags & flags::CASTLE != 0,
            Aim::Castle => tile_flags & flags::SETTLEMENT != 0 && (0x15..=0x19).contains(&terrain),
            Aim::StandingCrop => {
                tile_flags & flags::FARMLAND != 0 && terrain > 2 && terrain < 0x17
            }
        }
    }
}

/// The body all three finders share: the tile of `county` matching `aim` that
/// is **nearest by Manhattan distance** to `from`, or the county's anchor.
///
/// Two reproduced details:
///
/// * **The scan is the whole 64 × 64 map**, row by row, and the comparison is
///   strict — so a tie goes to the lowest `y`, then the lowest `x`.
/// * **Tile (0, 0) can never be chosen.** The original keeps its best as a
///   byte *offset* into the tile array and uses 0 for *"nothing found"*, so
///   offset 0 — tile (0, 0) — is indistinguishable from failure. Harmless on
///   any real map and reproduced anyway, because the alternative is a
///   difference nobody would ever find.
///
/// The fallback differs from the original in one place and it is stated: for
/// [`Aim::Castle`] the original falls back to the county's stored castle tile
/// (`+0x74`/`+0x75`, `County_FindCastleTile`'s output), which this crate does
/// not carry. The anchor is used instead. `[I]`, and it only fires when the
/// county has no castle tile on the map at all — in which case the stored pair
/// would be stale.
pub fn aim_tile(
    map: &CampaignMap,
    counties: &[County; MAX_COUNTIES],
    from: (u8, u8),
    county: u8,
    aim: Aim,
) -> (u8, u8) {
    let mut best = 1000;
    let mut found: Option<(u8, u8)> = None;
    for y in 0..MAP_DIM as u8 {
        for x in 0..MAP_DIM as u8 {
            if x == 0 && y == 0 {
                continue; // the offset-0 sentinel
            }
            if map.county_at(x, y) != county {
                continue;
            }
            if !aim.accepts(map.terrain_at(x, y), map.flags_at(x, y)) {
                continue;
            }
            let d = manhattan(from, (x, y));
            if d < best {
                best = d;
                found = Some((x, y));
            }
        }
    }
    found.unwrap_or_else(|| {
        counties
            .get(county as usize)
            .map_or((0, 0), |c| (c.anchor_x, c.anchor_y))
    })
}

/// `FUN_004A64CA` — the choice missions 2 and 3 make between the town and the
/// castle.
///
/// > *"This shire contains a garrisoned castle my lord. We must lay siege to
/// > that, to gain control of the county."* — `L2.eng` group 286, which is the
/// > game telling the **player** the rule this function is the AI's half of.
/// > `[V]`
///
/// A county with no castle, or a castle with nobody in it, is entered at the
/// town; anything else is approached at the castle, which begins a siege.
pub fn aim_for_county(counties: &[County; MAX_COUNTIES], county: u8) -> Aim {
    match counties.get(county as usize) {
        Some(c) if c.castle_type != 0 && c.garrison_unit != 0 => Aim::Castle,
        _ => Aim::Town,
    }
}

// ---------------------------------------------------------------------------
// The passes, over a whole kingdom
// ---------------------------------------------------------------------------

/// What one turn of the AI's army handling did, for a caller that wants to
/// show it or a test that wants to assert on it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArmyReport {
    /// Armies raised to fill a castle garrison — step 7's second pass.
    pub garrisons_raised: Vec<usize>,
    /// Armies raised on a threatened frontier county — step 7's third pass.
    pub frontier_raised: Vec<usize>,
    /// Counties written off, and what the pass wanted to carry out of them.
    pub evacuations: Vec<Evacuation>,
}

impl Kingdom {
    /// AI step 4 — [`resource_wants`].
    pub fn run_ai_resource_wants(&mut self, realm_id: u8) {
        resource_wants(
            &self.tables,
            &self.counties,
            self.county_count,
            &mut self.realms,
            realm_id,
        );
    }

    /// AI step 7 — `FUN_0049F93D`, which is three passes in a row.
    pub fn run_ai_armies(&mut self, realm_id: u8) -> ArmyReport {
        let (muster, raid) = choose_muster_counties(&self.tables, &self.counties, self.county_count, realm_id);
        if let Some(r) = self.realms.get_mut(realm_id as usize) {
            r.muster_county = muster;
            r.raid_county = raid;
        }
        let garrisons_raised = self.run_ai_garrisons(realm_id);
        let (frontier_raised, evacuations) = self.run_ai_frontier(realm_id);
        ArmyReport { garrisons_raised, frontier_raised, evacuations }
    }

    /// Step 7's **second** pass — `FUN_0049F12F`, the castle garrisons.
    ///
    /// For every county the realm holds that has a castle which is standing,
    /// not ruined and not under construction, is at or above the lord's
    /// [`crate::tables::AI_PERSONALITY_GARRISON_MIN_POPULATION`], **and** while
    /// the realm holds more than [`GARRISON_MIN_MISSILE_STOCK`] bows plus
    /// crossbows, raise a slice of the garrison's shortfall
    /// ([`garrison_levy_share`]) and send it to the castle on
    /// [`Mission::JOIN_GARRISON`].
    ///
    /// The missile-stock test is the interesting gate: it is
    /// `weapons[4] + weapons[0]`, **bows and crossbows only**, and it is read
    /// once per county rather than once per pass — so a realm that spends its
    /// last bows on the first castle stops garrisoning at the second.
    pub fn run_ai_garrisons(&mut self, realm_id: u8) -> Vec<usize> {
        let mut raised = Vec::new();
        let Some(lord) = self.realms.get(realm_id as usize).map(|r| r.lord) else { return raised };
        let Some(p) = self.tables.ai_personality(lord) else { return raised };
        let (floor, year) = (p.garrison_min_population, self.year);
        for id in 1..=self.county_count.min(MAX_COUNTIES - 1) {
            let c = &self.counties[id];
            if c.owner != realm_id
                || c.castle_type == 0
                || c.castle_degraded == crate::siege::CASTLE_DEGRADED_BUILDING
                || c.castle_ruined
                || c.population < floor
            {
                continue;
            }
            let realm = &self.realms[realm_id as usize];
            if realm.weapons[4] + realm.weapons[0] <= GARRISON_MIN_MISSILE_STOCK {
                continue;
            }
            let sitting = self
                .campaign
                .units
                .get(c.garrison_unit)
                .map_or(0, |u| u.men);
            let gap = crate::industry::garrison_cap(&self.tables, c.castle_type) - sitting;
            if gap <= GARRISON_GAP_MIN {
                continue;
            }
            let Some(want) = garrison_levy_share(gap, c.population) else { continue };
            let percent = crate::industry::pct_of(want, c.population);
            let Some(unit) = self.raise_garrison_levy(id as u8, percent, year) else { continue };
            let tile = aim_tile(
                &self.campaign.map,
                &self.counties,
                self.campaign.units.get(unit).map_or((0, 0), |u| u.tile()),
                id as u8,
                Aim::Castle,
            );
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::JOIN_GARRISON;
                u.needs_destination = false;
                u.dest_county = id as u8;
                u.dest = Some(tile);
            }
            raised.push(unit);
        }
        raised
    }

    /// `FUN_004A5389` — the AI's garrison levy, which is **not**
    /// `FUN_004A50AE` and does not equip the same way.
    ///
    /// [`GARRISON_EQUIP_ORDER`] is the difference: a greedy fill in a fixed
    /// priority, where the general raiser round-robins ten at a time.
    fn raise_garrison_levy(&mut self, county: u8, percent: i32, year: i32) -> Option<usize> {
        let realm = self.counties.get(county as usize)?.owner;
        let population = self.counties.get(county as usize)?.population;
        let happiness_cost = self.tables.army_happiness_cost(percent);
        let men = pct(population, percent);
        let mut basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        for troop in GARRISON_EQUIP_ORDER {
            basket.equip(troop, i32::MAX);
        }
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
        )
        .ok()
    }

    /// Step 7's **third** pass — `FUN_0049F431`, hold the frontier or write it
    /// off.
    ///
    /// For every county the realm holds that has **no castle or no garrison**,
    /// compare the hostile men standing in it against the realm's own. When
    /// the enemy is above [`FRONTIER_ENEMY_MIN`] and beats the defenders by
    /// more than [`FRONTIER_DEFICIT`], the realm decides:
    ///
    /// * **hold** — if the enemy force is smaller than the realm's total
    ///   weapon stock plus two thirds of the county's people, *or* the county
    ///   is the muster county. A levy of [`FRONTIER_LEVY_PCT`] goes up, **armed**
    ///   from the realm's stores, on [`Mission::HOLD_HOME`].
    /// * **write it off** — the same levy goes up **carrying nothing at all**,
    ///   with its [`crate::unit::Unit::home_county`] set to the *muster* county so it walks
    ///   away rather than standing and dying; the county is taxed at the
    ///   lord's [`crate::tables::AI_PERSONALITY_ABANDON_TAX_RATE`], its whole
    ///   workforce is thrown at industry, and its larder is shipped out.
    ///
    /// **The two branches differ by one argument** — the equip mode — and that
    /// single bit is the whole difference between a defence and an evacuation.
    /// A county below [`FRONTIER_MIN_POPULATION`] people forces the comparison
    /// threshold to zero and is therefore **always** written off.
    ///
    /// The larder is the seam; see [`Evacuation`].
    pub fn run_ai_frontier(&mut self, realm_id: u8) -> (Vec<usize>, Vec<Evacuation>) {
        let (mut raised, mut evacuations) = (Vec::new(), Vec::new());
        let Some(lord) = self.realms.get(realm_id as usize).map(|r| r.lord) else {
            return (raised, evacuations);
        };
        let Some(p) = self.tables.ai_personality(lord) else { return (raised, evacuations) };
        let abandon_tax_rate = p.abandon_tax_rate;
        let year = self.year;
        for id in 1..=self.county_count.min(MAX_COUNTIES - 1) {
            let c = &self.counties[id];
            if c.owner != realm_id || (c.castle_type != 0 && c.garrison_unit != 0) {
                continue;
            }
            let enemy = men_in_county(&self.campaign.units, &self.realms, id as u8, realm_id, true);
            let mine = men_in_county(&self.campaign.units, &self.realms, id as u8, realm_id, false);
            if enemy <= FRONTIER_ENEMY_MIN || mine >= enemy - FRONTIER_DEFICIT {
                continue;
            }
            let population = self.counties[id].population;
            let muster = self.realms[realm_id as usize].muster_county;
            let mut threshold = self.realms[realm_id as usize].weapons_total() + (population * 2) / 3;
            if population < FRONTIER_MIN_POPULATION {
                threshold = 0;
            }
            let hold = enemy < threshold || muster == id as u8;
            let unit = self.raise_frontier_levy(id as u8, hold, year);
            if let Some(unit) = unit {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.mission = Mission::HOLD_HOME;
                    if !hold {
                        u.home_county = muster;
                    }
                }
                raised.push(unit);
            }
            if hold {
                continue;
            }
            let c = &mut self.counties[id];
            c.tax_rate = abandon_tax_rate;
            c.industry_share = 100;
            if worth_evacuating(c) {
                evacuations.push(Evacuation {
                    from: id as u8,
                    to: muster,
                    grain: c.grain,
                    herd: c.herd,
                });
            }
        }
        (raised, evacuations)
    }

    /// `FUN_004A50AE` with the frontier pass's arguments: 50 % of the county,
    /// a population floor of [`FRONTIER_LEVY_MIN_POPULATION`], and the equip
    /// mode that decides whether this is a defence or a retreat.
    fn raise_frontier_levy(&mut self, county: u8, armed: bool, year: i32) -> Option<usize> {
        let c = self.counties.get(county as usize)?;
        if c.population < FRONTIER_LEVY_MIN_POPULATION {
            return None;
        }
        let (realm, population) = (c.owner, c.population);
        let happiness_cost = self.tables.army_happiness_cost(FRONTIER_LEVY_PCT);
        let men = pct(population, FRONTIER_LEVY_PCT);
        let mut basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        if armed {
            basket.auto_equip();
        }
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
        )
        .ok()
    }

    /// AI step 9 — `FUN_0049F977`, **raise the main army and point it at
    /// somebody**.
    ///
    /// The shape, which is more interesting than any one of its numbers:
    ///
    /// 1. An **ally's request** outranks everything. If one stands and still
    ///    holds ([`ally_request_still_stands`]), that county is the target and
    ///    the lord's population floor is halved.
    /// 2. Otherwise, with a **declared war target**, the floor is halved and
    ///    the realm looks for somewhere every turn.
    /// 3. Otherwise the realm counts to its lord's
    ///    [`crate::tables::AI_PERSONALITY_MUSTER_PATIENCE`] first, and only
    ///    then looks. Two attempts: **the threat's counties**, and failing
    ///    that anybody's. Nothing found means nothing raised.
    /// 4. If the muster county has more people than the floor, levy
    ///    [`crate::tables::AI_PERSONALITY_MUSTER_PCT`] of it — gated on the
    ///    realm's weapon stock unless [`emergency_weapons`] fires. If it has
    ///    **fewer**, no levy at all: an existing idle army of at least
    ///    [`DIVERT_MIN_MEN`] is diverted instead.
    /// 5. Whatever came out is aimed by [`Kingdom::aim_army`].
    ///
    /// Point 4 is the one worth reading twice. A realm whose best county is
    /// too small to conscript **does not stop making war** — it re-tasks the
    /// army it already has. That is why a cornered AI keeps coming.
    ///
    /// Returns the army it raised or diverted.
    pub fn run_ai_raise_army(&mut self, realm_id: u8) -> Option<usize> {
        let lord = self.realms.get(realm_id as usize)?.lord;
        let p = *self.tables.ai_personality(lord)?;
        let mut floor = p.help_population_floor;

        let target_county = self.realms[realm_id as usize].target_county;
        if target_county == 0 {
            if self.realms[realm_id as usize].war_target == 0 {
                let r = &mut self.realms[realm_id as usize];
                r.muster_timer = r.muster_timer.saturating_add(1);
                if (r.muster_timer as i32) < p.muster_patience {
                    return None;
                }
                r.muster_timer = 0;
            } else {
                floor /= 2;
            }
        } else {
            let kept = ally_request_still_stands(&self.counties, &self.realms, target_county, realm_id);
            self.realms[realm_id as usize].target_county = kept;
        }

        let muster = self.realms[realm_id as usize].muster_county;
        if self.realms[realm_id as usize].target_county == 0 {
            let threat = pick_threat(&self.realms, realm_id);
            self.realms[realm_id as usize].threat_realm = threat;
            if !self.pick_attack_county(realm_id, muster, true, 0)
                && !self.pick_attack_county(realm_id, muster, false, 0)
            {
                return None;
            }
        } else {
            let t = self.realms[realm_id as usize].target_county;
            self.realms[realm_id as usize].attack_county = t;
            floor /= 2;
        }

        let population = self.counties.get(muster as usize).map_or(0, |c| c.population);
        let year = self.year;
        let unit = if population > floor {
            let emergency = emergency_weapons(&mut self.realms[realm_id as usize], year);
            if !emergency && self.realms[realm_id as usize].weapons_total() < p.muster_arms {
                return None;
            }
            self.raise_muster_levy(muster, p.muster_pct, year)?
        } else {
            let target = self.realms[realm_id as usize].attack_county;
            self.pick_army_to_divert(realm_id, target)?
        };

        self.aim_army(realm_id, unit);
        self.realms[realm_id as usize].target_county = 0;
        Some(unit)
    }

    /// `FUN_004A50AE(muster, musterPct, 100, 1)` — the main levy, auto-equipped
    /// from the realm's stores.
    fn raise_muster_levy(&mut self, county: u8, percent: i32, year: i32) -> Option<usize> {
        let c = self.counties.get(county as usize)?;
        if c.population < FRONTIER_LEVY_MIN_POPULATION {
            return None;
        }
        let (realm, population) = (c.owner, c.population);
        let happiness_cost = self.tables.army_happiness_cost(percent);
        let men = pct(population, percent);
        let mut basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        basket.auto_equip();
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
        )
        .ok()
    }

    /// `FUN_004A03F2` — pick the county to march on, into realm `+0x4B`.
    ///
    /// `prefer_threat` restricts the search to [`crate::realm::Realm::threat_realm`]'s
    /// counties; `owner_filter` restricts it to one realm's and — the part
    /// that matters — **skips the diplomacy and adjacency gates entirely**.
    /// Step 10 uses the filter, which is why a raid can be sent at a county
    /// nowhere near the raider's border, and at an ally's.
    ///
    /// Returns whether anything was found.
    pub fn pick_attack_county(
        &mut self,
        realm_id: u8,
        from: u8,
        prefer_threat: bool,
        owner_filter: u8,
    ) -> bool {
        let mut best = TARGET_SCORE_CEILING;
        let mut found = 0u8;
        let threat = self.realms[realm_id as usize].threat_realm;
        for id in 1..=self.county_count.min(MAX_COUNTIES - 1) {
            let owner = self.counties[id].owner;
            let eligible = if owner_filter != 0 {
                owner == owner_filter
            } else {
                (!prefer_threat || threat == owner)
                    && action_allowed(&mut self.realms, realm_id, owner)
                    && county_borders_realm(&self.counties, id as u8, realm_id)
            };
            if !eligible {
                continue;
            }
            let Some(from_county) = self.counties.get(from as usize) else { continue };
            let score = target_score(from_county, &self.counties[id], true);
            if score < best {
                best = score;
                found = id as u8;
            }
        }
        self.realms[realm_id as usize].attack_county = found;
        found != 0
    }

    /// `FUN_004A0917` — the best **existing** army to send instead of raising
    /// one.
    ///
    /// Only an army with no orders (`needs_destination`), out of any garrison
    /// and any siege, and of at least [`DIVERT_MIN_MEN`] men qualifies. It is
    /// scored [`DIVERT_IN_COUNTY`] for already standing in the target county,
    /// [`DIVERT_NEXT_DOOR`] for standing next door, nothing otherwise, plus an
    /// eighth of its [`crate::unit::Unit::strength_score`] — so **position dominates
    /// strength**: an eighth of the strongest plausible army is worth far less
    /// than the 200 that being adjacent is worth.
    pub fn pick_army_to_divert(&self, realm_id: u8, county: u8) -> Option<usize> {
        let mut best = 0;
        let mut chosen = None;
        for (id, u) in self.campaign.units.iter() {
            if u.owner != realm_id
                || u.garrison_county != 0
                || u.besieging_county != 0
                || !u.needs_destination
                || u.men < DIVERT_MIN_MEN
            {
                continue;
            }
            let mut score = if u.county == county {
                DIVERT_IN_COUNTY
            } else if self
                .counties
                .get(u.county as usize)
                .is_some_and(|c| c.neighbours().contains(&county))
            {
                DIVERT_NEXT_DOOR
            } else {
                0
            };
            score += u.strength_score() / 8;
            if score > best {
                best = score;
                chosen = Some(id);
            }
        }
        chosen
    }

    /// `FUN_0049FDA5` — give the raised or diverted army its mission and its
    /// destination tile.
    ///
    /// A plain campaign is [`Mission::SEEK_ENEMY`]. An **ally's request**
    /// against a county the ally itself holds is [`Mission::ASSIST_ALLY`] —
    /// relief rather than conquest — and against anybody else's it is an
    /// ordinary attack. The tile is [`aim_for_county`]'s choice, so a
    /// garrisoned castle is approached at the castle and a siege begins.
    pub fn aim_army(&mut self, realm_id: u8, unit: usize) {
        let (target, request, ally) = {
            let r = &self.realms[realm_id as usize];
            (r.attack_county, r.target_county, r.ally)
        };
        let target_owner = self.counties.get(target as usize).map_or(0, |c| c.owner);
        let aim = aim_for_county(&self.counties, target);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, aim);
        let Some(u) = self.campaign.units.get_mut(unit) else { return };
        if request == 0 {
            u.mission = Mission::SEEK_ENEMY;
        } else {
            u.mission_county = request;
            u.mission =
                if target_owner == ally { Mission::ASSIST_ALLY } else { Mission::SEEK_ENEMY };
        }
        u.needs_destination = false;
        u.dest_county = target;
        u.dest = Some(tile);
    }

    /// AI step 10 — `FUN_004A0015`, **the raid**.
    ///
    /// One unit, every [`crate::tables::AI_PERSONALITY_RAID_INTERVAL`] turns,
    /// out of the muster county, aimed at the standing crops of a county
    /// belonging to the realm this one thinks worst of. It is
    /// [`RAID_MEN`]-ish men carrying **nothing** — `FUN_004A5003` never opens
    /// the armoury — so it cannot fight and is not meant to: the damage is
    /// [`crate::movement::destroy_field`], done on the way in.
    ///
    /// Three gates, all on the muster county: more than
    /// [`RAID_MIN_POPULATION`] people, more than [`RAID_MIN_HAPPINESS`]
    /// happiness, and a rival below [`RAID_STANDING_THRESHOLD`] to send it at.
    ///
    /// **The cooldown is only loaded on success.** A realm that wants to raid
    /// and cannot — no victim, an unhappy muster county — tries again every
    /// single turn.
    pub fn run_ai_raid(&mut self, realm_id: u8) -> Option<usize> {
        if self.realms.get(realm_id as usize)?.raid_timer != 0 {
            self.realms[realm_id as usize].raid_timer -= 1;
            return None;
        }
        let victim = pick_raid_victim(&self.realms, realm_id);
        if victim == 0 {
            return None;
        }
        let muster = self.realms[realm_id as usize].muster_county;
        if !self.pick_attack_county(realm_id, muster, true, victim) {
            return None;
        }
        let c = self.counties.get(muster as usize)?;
        if c.population <= RAID_MIN_POPULATION || c.happiness <= RAID_MIN_HAPPINESS {
            return None;
        }
        let year = self.year;
        let unit = self.raise_raiding_party(muster, year)?;
        let target = self.realms[realm_id as usize].attack_county;
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, Aim::StandingCrop);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = Mission::RAID;
            u.needs_destination = false;
            u.dest_county = target;
            u.dest = Some(tile);
        }
        let lord = self.realms[realm_id as usize].lord;
        if let Some(p) = self.tables.ai_personality(lord) {
            self.realms[realm_id as usize].raid_timer = p.raid_interval.clamp(0, 255) as u8;
        }
        Some(unit)
    }

    /// `FUN_004A5003` — [`RAID_MEN`] men, unequipped.
    ///
    /// The percentage is `PctOf(50, population)` and the men are
    /// `Pct(population, that)`, so the party is fifty men rounded by integer
    /// percent: 48 out of a county of 400, 50 out of 500, 45 out of 900. The
    /// happiness the county is charged is the table entry for that same
    /// percentage, so a **big** county pays almost nothing for a raid and a
    /// small one pays a lot.
    fn raise_raiding_party(&mut self, county: u8, year: i32) -> Option<usize> {
        let c = self.counties.get(county as usize)?;
        let (realm, population) = (c.owner, c.population);
        let percent = crate::industry::pct_of(RAID_MEN, population);
        let happiness_cost = self.tables.army_happiness_cost(percent);
        let men = pct(population, percent);
        let basket = LevyBasket::seed(self.realms.get(realm as usize)?, men);
        let Kingdom { tables, counties, realms, campaign, .. } = self;
        levy::create_army(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            &basket,
            Muster { realm, county, happiness_cost, year },
        )
        .ok()
    }
}

// ---------------------------------------------------------------------------
// Step 11 — run every army's mission and re-path it
// ---------------------------------------------------------------------------

/// A garrison [`Mission::GARRISON`] turned out of a castle whose county has
/// changed hands, and — where it had one — the besieger that was waiting for
/// it.
///
/// `FUN_00437535` starts the battle itself. This crate reports it, for the
/// same reason [`crate::ai::taunt`] returns its letters rather than showing
/// them: a battle is not this crate's to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eviction {
    pub unit: usize,
    pub county: u8,
    /// The besieger's slot, if the garrison was under siege when it was
    /// turned out.
    pub besieger: Option<usize>,
    /// True when there was nowhere to put the garrison and it was destroyed.
    pub destroyed: bool,
}

/// What [`Kingdom::run_ai_move_armies`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MoveReport {
    /// The armies that were given a fresh path this step.
    pub marching: Vec<usize>,
    /// Garrisons turned out of castles in counties the realm has lost.
    pub evictions: Vec<Eviction>,
    /// Armies disbanded because [`Mission::JOIN_GARRISON`] found no castle
    /// with room for them.
    pub disbanded: Vec<usize>,
}

/// `FUN_004673B8` — the nearest **enemy** army anywhere on the map, by
/// Chebyshev distance, as `(distance, slot)`. Distance is 1000 when there is
/// none.
///
/// Its filter is `owner != 0 && owner != mine && kind == 1 && !garrisoned`,
/// and it makes **no diplomacy check at all** — so an *allied* army is a
/// candidate here and is thrown out afterwards by
/// [`Mission::SEEK_ENEMY`]'s own test. The consequence is not cosmetic: a
/// nearer ally **masks** a slightly farther enemy, and the intercept is simply
/// skipped that turn. `[D]`
pub fn nearest_enemy_army(units: &Units, unit: usize) -> (i32, Option<usize>) {
    let Some(me) = units.get(unit) else { return (1000, None) };
    let mut best = 1000;
    let mut found = None;
    for (id, u) in units.iter() {
        if u.owner == 0 || u.owner == me.owner || u.kind != UnitKind::Army || u.garrison_county != 0
        {
            continue;
        }
        let d = chebyshev(me.tile(), u.tile());
        if d < best {
            best = d;
            found = Some(id);
        }
    }
    (best, found)
}

/// `FUN_00467532` — the nearest army **in one county** that
/// [`action_allowed`] lets this one attack.
///
/// It takes `&mut` realms because `Diplo_ActionAllowed` is not a predicate:
/// every allied unit it steps over bumps the searcher's own grudge. This
/// function is called once per turn per army on missions 3 and 6, over every
/// unit slot, so **an AI hemmed in by its ally corrodes the alliance simply by
/// looking around**. Whether that is intended is not established;
/// [`action_allowed`] carries the note.
pub fn nearest_attackable_army_in_county(
    units: &Units,
    realms: &mut [Realm; MAX_REALMS],
    unit: usize,
    county: u8,
) -> (i32, Option<usize>) {
    let Some(me) = units.get(unit) else { return (1000, None) };
    let (mine, my_tile) = (me.owner, me.tile());
    let mut best = 1000;
    let mut found = None;
    for (id, u) in units.iter() {
        if u.owner == 0 || u.kind != UnitKind::Army {
            continue;
        }
        if !action_allowed(realms, mine, u.owner) {
            continue;
        }
        if u.garrison_county != 0 || u.county != county {
            continue;
        }
        let d = chebyshev(my_tile, u.tile());
        if d < best {
            best = d;
            found = Some(id);
        }
    }
    (best, found)
}

/// `FUN_004A0649` — the target an army on [`Mission::SEEK_ENEMY`] picks for
/// itself when its standing order has run out.
///
/// The same scoring as [`Kingdom::pick_attack_county`] with the **narrow**
/// grain ladder ([`target_score`]), and the same two gates: the county must be
/// one [`action_allowed`] permits and must border the realm.
pub fn pick_next_target(
    counties: &[County; MAX_COUNTIES],
    county_count: usize,
    realms: &mut [Realm; MAX_REALMS],
    realm_id: u8,
    from: u8,
) -> u8 {
    let mut best = TARGET_SCORE_CEILING;
    let mut found = 0u8;
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        let owner = counties[id].owner;
        if owner == realm_id
            || !action_allowed(realms, realm_id, owner)
            || !county_borders_realm(counties, id as u8, realm_id)
        {
            continue;
        }
        let Some(from_county) = counties.get(from as usize) else { continue };
        let score = target_score(from_county, &counties[id], false);
        if score < best {
            best = score;
            found = id as u8;
        }
    }
    found
}

/// `FUN_004A07F2` — the **lowest-numbered** county of this realm whose castle
/// has room for `men`.
///
/// Lowest-numbered, not nearest: an army that cannot get into the castle it
/// was raised for walks to the realm's first castle instead, however far that
/// is.
pub fn first_castle_with_room(
    t: &Tables,
    counties: &[County; MAX_COUNTIES],
    units: &Units,
    county_count: usize,
    realm: u8,
    men: i32,
) -> u8 {
    for id in 1..=county_count.min(MAX_COUNTIES - 1) {
        let c = &counties[id];
        if c.owner != realm
            || c.castle_type == 0
            || c.castle_degraded == crate::siege::CASTLE_DEGRADED_BUILDING
            || c.castle_ruined
        {
            continue;
        }
        let sitting = units.get(c.garrison_unit).map_or(0, |u| u.men);
        if men <= crate::industry::garrison_cap(t, c.castle_type) - sitting {
            return id as u8;
        }
    }
    0
}

impl Kingdom {
    /// AI step 11 — `FUN_004A5667`, which this crate names `Ai_AdvanceArmies`.
    ///
    /// ```c
    /// for (i = 1; i <= 150; i++)
    ///   if (units[i].kind == 1 && units[i].owner == realm && mission_handler(i) != 0) {
    ///       units[i].castlePassThrough = 0;          /* +0x14E */
    ///       Move_FloodFill(0, units[i].x, units[i].y, 0);
    ///       if (Move_ExtractPath(0, units[i].destX, units[i].destY))
    ///           { Path_CopyToUnit(0, i); units[i].moving = 1; }
    ///   }
    /// ```
    ///
    /// It is the same shape as `PeasantMob_AdvanceAll`, which is already
    /// implemented as [`Kingdom::begin_unit_phase`]'s phase-5 arm — the AI's
    /// armies are the mobs' mechanism with a six-way mission dispatch in front
    /// of it.
    ///
    /// **The routing is `Direct`, not `PreferRoads`.** `Unit_OrderMove` — the
    /// player's path — tries roads first and falls back; this call passes
    /// mode 0 outright. So the claim in [`crate::movement::Routing`] that *"AI
    /// armies prefer roads and the player's do not"* is true of the
    /// *order-move* entry point and **false of the AI's own driver**, which is
    /// the one that actually moves AI armies every turn. Both readings are of
    /// the binary; they are about two different functions.
    ///
    /// Unit `+0x14E`, cleared before each re-path, is a *pass through the
    /// castle site* flag: `Unit_TryEnterTile` masks off the settlement bit
    /// while it is set, which is what stops a garrison just turned out of a
    /// castle immediately walking back into it. This crate has no such field
    /// and no path that sets one, so there is nothing to clear. Named rather
    /// than omitted. `[D]`
    pub fn run_ai_move_armies(&mut self, realm_id: u8) -> MoveReport {
        let mut report = MoveReport::default();
        let ids: Vec<usize> = self
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.kind == UnitKind::Army && u.owner == realm_id)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            if !self.run_mission(id, &mut report) {
                continue;
            }
            let Some(dest) = self.campaign.units.get(id).and_then(|u| u.dest) else { continue };
            if crate::movement::order_move(
                &self.campaign.map,
                &mut self.campaign.units,
                id,
                dest,
                crate::movement::Routing::Direct,
            )
            .is_some()
            {
                report.marching.push(id);
            }
        }
        report
    }

    /// `FUN_004A57AC` — dispatch one army's mission.
    ///
    /// Returns whether the unit wants a fresh path. **An unrecognised mission
    /// byte is rewritten to [`Mission::SEEK_ENEMY`] and the unit is not moved
    /// this turn** — so a freshly spawned army, whose byte is 0, loses exactly
    /// one turn and then behaves as an attacker.
    fn run_mission(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(mission) = self.campaign.units.get(unit).map(|u| u.mission) else { return false };
        match mission {
            Mission::RAID => self.mission_raid(unit),
            Mission::ASSIST_ALLY => self.mission_assist_ally(unit),
            Mission::SEEK_ENEMY => self.mission_seek_enemy(unit),
            Mission::HOLD_HOME => self.mission_hold_home(unit),
            Mission::JOIN_GARRISON => self.mission_join_garrison(unit, report),
            Mission::GARRISON => self.mission_garrison(unit, report),
            _ => {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.mission = Mission::SEEK_ENEMY;
                }
                false
            }
        }
    }

    /// [`Mission::RAID`] — `FUN_004A58F4`. Re-aim at the target county's
    /// nearest standing crop, every turn, unless the unit is camped outside a
    /// castle.
    ///
    /// Re-aiming *every* turn is what makes a raid destructive out of
    /// proportion to its size: the party is fifty men who cannot fight, but it
    /// walks a fresh line to the nearest surviving crop each season and
    /// tramples everything it crosses.
    fn mission_raid(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (from, target) = (u.tile(), u.dest_county);
        let tile = aim_tile(&self.campaign.map, &self.counties, from, target, Aim::StandingCrop);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.dest = Some(tile);
            u.needs_destination = false;
        }
        true
    }

    /// [`Mission::ASSIST_ALLY`] — `FUN_004A599D`.
    ///
    /// Re-validate the ally's request; a request that no longer stands demotes
    /// the unit to [`Mission::SEEK_ENEMY`] and **runs that handler in the same
    /// tick**, so no turn is lost. Otherwise close on the nearest enemy in the
    /// ally's county, but only within [`ASSIST_ALLY_RADIUS`] — an army too far
    /// away does nothing at all this turn rather than starting the walk, which
    /// is a real difference from [`Mission::HOLD_HOME`].
    fn mission_assist_ally(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, mut county) = (u.owner, u.mission_county);
        if county != 0 {
            county = ally_request_still_stands(&self.counties, &self.realms, county, owner);
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission_county = county;
            }
        }
        if county == 0 {
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::SEEK_ENEMY;
            }
            return self.mission_seek_enemy(unit);
        }
        let (d, other) = nearest_attackable_army_in_county(
            &self.campaign.units,
            &mut self.realms,
            unit,
            county,
        );
        let Some(other) = other.filter(|_| d < ASSIST_ALLY_RADIUS) else { return false };
        let tile = self.campaign.units.get(other).map(|u| u.tile());
        if let (Some(tile), Some(u)) = (tile, self.campaign.units.get_mut(unit)) {
            u.needs_destination = false;
            u.dest = Some(tile);
            return true;
        }
        false
    }

    /// [`Mission::SEEK_ENEMY`] — `FUN_004A5B1F`, the default war mission and
    /// the largest of the six.
    ///
    /// Four things in order:
    ///
    /// 1. **Intercept.** An enemy army within [`SEEK_ENEMY_RADIUS`] that is
    ///    not this realm's ally, and that is either *not itself on this
    ///    mission* or is to the east. The `x` test is a tie-break that stops
    ///    two attacking armies chasing each other for ever: whichever is
    ///    further west gives chase. `[I]` on the intent, `[D]` on the code.
    /// 2. **Keep the standing order** if the county it names is still somebody
    ///    else's and still borders this realm.
    /// 3. Otherwise, if the army is standing somewhere it cannot attack —
    ///    its own county, or one that does not touch its realm — **pick a new
    ///    target** and go.
    /// 4. Otherwise it is standing in a hostile county that borders its realm:
    ///    **attack where it stands**, unless that county belongs to its ally,
    ///    in which case pick another.
    ///
    /// **Two defects are reproduced.** In arm 4 the unit's
    /// [`crate::unit::Unit::dest_county`] is *not* updated, so it walks into the county it
    /// is in while its stored order still names the county arm 2 rejected; and
    /// if the ally case finds nothing, the aim is taken at **county 0**, which
    /// falls through to county 0's anchor. Both are in the decompilation and
    /// neither is tidied here.
    ///
    /// A third is *not* reproduced because it cannot be: the original calls
    /// `FUN_00467F2E` between arms 3 and 4 and **discards the result** — the
    /// two branches it was meant to choose between are identical. A dead call
    /// with no side effect is nothing to reproduce.
    fn mission_seek_enemy(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, my_x, order, here) = (u.owner, u.x, u.dest_county, u.county);
        let ally = self.realms.get(owner as usize).map_or(0, |r| r.ally);

        // 1 — intercept.
        let (d, other) = nearest_enemy_army(&self.campaign.units, unit);
        if d < SEEK_ENEMY_RADIUS {
            if let Some(o) = other.and_then(|o| self.campaign.units.get(o)) {
                if (o.mission != Mission::SEEK_ENEMY || my_x < o.x) && ally != o.owner {
                    let tile = o.tile();
                    if let Some(u) = self.campaign.units.get_mut(unit) {
                        u.needs_destination = false;
                        u.dest = Some(tile);
                    }
                    return true;
                }
            }
        }

        // 2 — the standing order still stands.
        if order != 0
            && self.counties.get(order as usize).is_some_and(|c| c.owner != owner)
            && county_borders_realm(&self.counties, order, owner)
        {
            self.aim_at_county(unit, order);
            return true;
        }

        // 3 — nowhere to attack from here: choose again.
        let here_owner = self.counties.get(here as usize).map_or(0, |c| c.owner);
        if here_owner == owner || !county_borders_realm(&self.counties, here, owner) {
            let next = pick_next_target(
                &self.counties,
                self.county_count,
                &mut self.realms,
                owner,
                here,
            );
            if next == 0 {
                return false;
            }
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.dest_county = next;
            }
            self.aim_at_county(unit, next);
            return true;
        }

        // 4 — attack where it stands. `dest_county` is deliberately left alone.
        let mut target = here;
        if ally == here_owner {
            target = pick_next_target(
                &self.counties,
                self.county_count,
                &mut self.realms,
                owner,
                here,
            );
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.dest_county = target;
            }
        }
        self.aim_at_county(unit, target);
        true
    }

    /// [`Mission::HOLD_HOME`] — `FUN_004A5F0A`.
    ///
    /// The same intercept test as [`Mission::SEEK_ENEMY`] but restricted to
    /// [`crate::unit::Unit::home_county`] and with five times the radius
    /// ([`HOLD_HOME_RADIUS`]) — and, unlike the ally mission, **it always
    /// returns something to do**: nothing to intercept means walk back to the
    /// county it is posted to.
    ///
    /// "Home county" is not always where the army was raised: step 7's
    /// abandonment branch overwrites it with the realm's muster county, which
    /// is what turns a written-off county's levy into a retreat.
    fn mission_hold_home(&mut self, unit: usize) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        if u.besieging_county != 0 {
            return false;
        }
        let (owner, my_x, home) = (u.owner, u.x, u.home_county);
        let ally = self.realms.get(owner as usize).map_or(0, |r| r.ally);
        let (d, other) =
            nearest_attackable_army_in_county(&self.campaign.units, &mut self.realms, unit, home);
        if d < HOLD_HOME_RADIUS {
            if let Some(o) = other.and_then(|o| self.campaign.units.get(o)) {
                if (o.mission != Mission::SEEK_ENEMY || my_x < o.x) && ally != o.owner {
                    let tile = o.tile();
                    if let Some(u) = self.campaign.units.get_mut(unit) {
                        u.needs_destination = false;
                        u.dest = Some(tile);
                    }
                    return true;
                }
            }
        }
        self.aim_at_county(unit, home);
        true
    }

    /// [`Mission::JOIN_GARRISON`] — `FUN_004A6270`.
    ///
    /// Re-check every turn that the castle this army was sent to is still
    /// worth walking to: it must still have room for **all** of the army, be
    /// standing, not be under construction, and still belong to the realm. Any
    /// of those failing sends it to
    /// [`first_castle_with_room`] instead, and if there is no such castle the
    /// army is **disbanded** — its men go back into a county and its weapons
    /// back into the armoury.
    ///
    /// **The original returns 1 even after disbanding**, so its caller then
    /// flood-fills from a record `Army_Destroy` has already cleared. Here the
    /// slot is simply empty and the re-path finds nothing to do, which is the
    /// same observable outcome by a route that cannot read freed memory.
    fn mission_join_garrison(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        let (owner, men, target) = (u.owner, u.men, u.dest_county);
        let ok = self.counties.get(target as usize).is_some_and(|c| {
            let sitting = self.campaign.units.get(c.garrison_unit).map_or(0, |g| g.men);
            c.castle_type != 0
                && !c.castle_ruined
                && c.castle_degraded != crate::siege::CASTLE_DEGRADED_BUILDING
                && c.owner == owner
                && men <= crate::industry::garrison_cap(&self.tables, c.castle_type) - sitting
        });
        let county = if ok {
            target
        } else {
            first_castle_with_room(
                &self.tables,
                &self.counties,
                &self.campaign.units,
                self.county_count,
                owner,
                men,
            )
        };
        if county == 0 {
            let Kingdom { tables, counties, realms, campaign, options, .. } = self;
            let _ = crate::divide::disband(
                tables,
                counties,
                realms,
                &mut campaign.units,
                &mut campaign.names,
                &mut campaign.mercenaries,
                unit,
                options.difficulty,
            );
            report.disbanded.push(unit);
            return true;
        }
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, Aim::Castle);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.dest_county = county;
            u.needs_destination = false;
            u.dest = Some(tile);
        }
        true
    }

    /// [`Mission::GARRISON`] — `FUN_004A60B9`, *"should I still be in here?"*
    ///
    /// A garrison whose county still belongs to its realm does **nothing at
    /// all** — the common case, and the reason this handler returns `false`
    /// nearly always. Once the county is lost, the garrison is turned out
    /// ([`Kingdom::evict_garrison`]) and demoted to [`Mission::SEEK_ENEMY`],
    /// and in the one case where the castle is still standing and unruined it
    /// is also sent straight at the **county town** — i.e. it walks out of the
    /// castle to take the county back.
    fn mission_garrison(&mut self, unit: usize, report: &mut MoveReport) -> bool {
        let Some(u) = self.campaign.units.get(unit) else { return false };
        let (owner, county) = (u.owner, u.dest_county);
        let Some(c) = self.counties.get(county as usize) else { return false };
        if c.owner == owner {
            return false;
        }
        let retake = c.castle_type != 0
            && c.castle_degraded != crate::siege::CASTLE_DEGRADED_BUILDING
            && !c.castle_ruined;
        if !retake {
            self.evict_garrison(unit, county, report);
            if let Some(u) = self.campaign.units.get_mut(unit) {
                u.mission = Mission::SEEK_ENEMY;
            }
            return false;
        }
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.mission = Mission::SEEK_ENEMY;
        }
        self.evict_garrison(unit, county, report);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, Aim::Town);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = false;
            u.dest = Some(tile);
            return true;
        }
        false
    }

    /// `FUN_00437535` — turn a garrison out of its castle onto the nearest
    /// free tile, or destroy it if there is nowhere to stand.
    ///
    /// The link is cleared on **both** sides — the unit's
    /// [`crate::unit::Unit::garrison_county`] and the county's
    /// [`crate::county::County::garrison_unit`] — and a besieger waiting outside is reported
    /// rather than fought; see [`Eviction`].
    ///
    /// **`[I]` on the search.** The original calls `Map_FindFreeTileNear`,
    /// whose radius this crate has not read; the box walk here is the one
    /// [`crate::levy::muster_tile`] and [`crate::merchant::find_free_road_tile`]
    /// both use, growing 1, 2, 3 around the unit's own tile and taking any
    /// passable unoccupied ground. The choice of tile is not observable in any
    /// rule; whether one is *found* is, and at radius 3 around a castle it
    /// always is on a real map.
    pub fn evict_garrison(&mut self, unit: usize, county: u8, report: &mut MoveReport) {
        let besieger = self.campaign.units.get(unit).and_then(|u| {
            let b = u.besieged_by as usize;
            if b == 0 {
                None
            } else {
                Some(b)
            }
        });
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let spot = free_tile_near(&self.campaign.map, &self.campaign.units, from);
        if let Some(c) = self.counties.get_mut(county as usize) {
            if c.garrison_unit == unit {
                c.garrison_unit = 0;
            }
        }
        match spot {
            Some((x, y)) => {
                if let Some(u) = self.campaign.units.get_mut(unit) {
                    u.x = x;
                    u.y = y;
                    u.garrison_county = 0;
                    u.besieged_by = 0;
                }
                report.evictions.push(Eviction { unit, county, besieger, destroyed: false });
            }
            None => {
                self.campaign.units.remove(unit);
                report.evictions.push(Eviction { unit, county, besieger, destroyed: true });
            }
        }
    }

    /// `FUN_004A64CA` — aim an army at a county, at its castle when that
    /// castle is held and at its town otherwise. See [`aim_for_county`].
    fn aim_at_county(&mut self, unit: usize, county: u8) {
        let aim = aim_for_county(&self.counties, county);
        let from = self.campaign.units.get(unit).map_or((0, 0), |u| u.tile());
        let tile = aim_tile(&self.campaign.map, &self.counties, from, county, aim);
        if let Some(u) = self.campaign.units.get_mut(unit) {
            u.needs_destination = false;
            u.dest = Some(tile);
        }
    }
}

/// Any free, passable tile within three of `from`. See
/// [`Kingdom::evict_garrison`].
fn free_tile_near(map: &CampaignMap, units: &Units, (ax, ay): (u8, u8)) -> Option<(u8, u8)> {
    for r in 1..=3i32 {
        let (x0, y0) = ((ax as i32 - r).max(0), (ay as i32 - r).max(0));
        let (x1, y1) = ((ax as i32 + r).min(MAP_DIM as i32 - 1), (ay as i32 + r).min(MAP_DIM as i32 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (x, y) = (x as u8, y as u8);
                if units.at(x, y).is_none() && map.flags_at(x, y) & flags::IMPASSABLE == 0 {
                    return Some((x, y));
                }
            }
        }
    }
    None
}
