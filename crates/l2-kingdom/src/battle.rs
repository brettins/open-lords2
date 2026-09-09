//! **The campaign half of a battle**: choosing how it is settled, settling it
//! by arithmetic when nobody is watching, and applying the result.
//!
//! [`crate::conquest`] stops at `Attack::Battle { attacker, defender }` and
//! says the caller hands the pair to `l2-sim`. This is what happens on either
//! side of that hand-off, and none of it needs the real-time simulation:
//!
//! ```text
//! Army_AttackCounty                 crate::conquest — a defender is settled
//! FUN_004A6A30       §1  choose     autocalc, prompt, or straight to a report
//! FUN_004AAD07       §2  autocalc   strength, a ratio, a survival percentage
//! …or l2-sim runs a real battle and writes the survivors back…
//! Battle_ReturnToCampaign §3        the county, the moves, the loser
//! Defence_Disband    §4             the levy goes home
//! ```
//!
//! **`docs/armies.md` §7 described `Battle_ReturnToCampaign` with the winner
//! and the loser exchanged**, and the document says so at the top of the
//! section. This module is written from the function, not from the name: see
//! [`Verdict`].
//!
//! # Why this is in `l2-kingdom` and not beside the simulation
//!
//! Every rule here reads and writes campaign records — units, counties, realms.
//! The autocalc in particular is a *complete alternative* to the battle
//! simulation that never mentions a figure, a cell or a tick, and it is the
//! path the original takes for every battle no human is in. Putting it here
//! means the campaign can fight a war with `l2-sim` absent, which is exactly
//! what an AI-versus-AI turn is.
//!
//! `l2-game` owns the other path: raising the two records into `l2-sim`,
//! running it, and writing the survivors back into
//! [`Unit::troops`](crate::unit::Unit::troops) before calling
//! [`return_to_campaign`].

use crate::county::{County, MAX_COUNTIES};
use crate::math::pct;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{ArmyNames, Unit, Units, TROOP_TYPES};

// ---------------------------------------------------------------- §1 the gate

/// How a battle between two campaign units is going to be settled —
/// `FUN_004A6A30` (`0x004A6A30`) and the one `if` at every one of its three
/// callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    /// **Neither side is a human's.** Screen `0x12` is never pushed and no
    /// report is drawn: the autocalc runs and the turn carries on. This is the
    /// gate's own return value of 0.
    Silently,
    /// A human is in it, but `Fight humans only?` is on and the *other* side is
    /// the AI's. The autocalc runs and the player is shown the result on screen
    /// `0x13` — a battle they are told about rather than asked about.
    Reported,
    /// Screen `0x12`: *"A Battle is to be fought. Will you take the field?"*
    /// Taking the field is the real simulation; declining is
    /// [`Settlement::Reported`] by another route, because `FUN_0043B622` runs
    /// the same autocalc.
    Prompt,
}

/// `g_optFightHumansOnly` (`0x0053F284`) — the advanced option *"Fight humans
/// only?"*, **stored inverted**: the byte is 0 when the option displays *Yes*.
///
/// So `fight_humans_only == false` here means the player asked to fight only
/// other people, and a battle against an AI is auto-resolved. The inversion is
/// the original's, and `docs/symbols.md` records it; this function takes the
/// byte, not the sense, so the two never drift apart.
pub const FIGHT_HUMANS_ONLY_DEFAULT: u8 = 1;

/// `FUN_004A6A30` — which of the three ways this battle is settled.
///
/// ```c
/// if (A.ownerIsHuman == 0 && B.ownerIsHuman == 0) return 0;   /* autocalc, silent   */
/// bothHuman = A.ownerIsHuman != 0 && B.ownerIsHuman != 0;
/// ...
/// if (g_optFightHumansOnly == 0 && !bothHuman) -> autocalc, screen 0x13
/// else                                        -> screen 0x12, ask
/// ```
///
/// Note what is **not** here: no distance from the view, no army-size cutoff,
/// no separate "quick battle" toggle. The mid-battle *"Autocalc battle?"*
/// button is a fourth entry and re-runs [`auto_resolve`] from the counts as
/// they stand; it is not a fifth outcome. `[V]` — one function, three callers,
/// all reading the same two expressions.
pub fn settlement(units: &Units, a: usize, b: usize, fight_humans_only_byte: u8) -> Settlement {
    let human = |id: usize| units.get(id).is_some_and(|u| u.owner_is_human);
    let (a_human, b_human) = (human(a), human(b));
    if !a_human && !b_human {
        return Settlement::Silently;
    }
    if fight_humans_only_byte == 0 && !(a_human && b_human) {
        return Settlement::Reported;
    }
    Settlement::Prompt
}

// ------------------------------------------------------------ §2 the autocalc

/// `Table_Lookup(ratio, DAT_004DE710, 10, 100)` — **how much of the winner
/// survives**, as a percentage, by how far ahead of the loser it was.
///
/// Read out of `Lords2.exe` at `0x004DE710`, ten `(threshold, value)` pairs.
/// `[V]` — the thresholds ascend strictly and the eleventh pair in memory is
/// `(100, 200)`, a different table starting, which is what fixes the count at
/// ten.
///
/// **The shape of it is the interesting part.** An evenly matched fight —
/// ratio under 110 — leaves the winner **ten per cent of its army**. Mutual
/// annihilation is the *default* outcome of a close battle, and the survival
/// curve only becomes generous once one side is roughly triple the other. A
/// player who auto-resolves a fair fight has effectively traded both armies.
pub const SURVIVAL_LADDER: [(i32, i32); 10] = [
    (110, 10),
    (130, 20),
    (160, 30),
    (180, 40),
    (220, 50),
    (270, 65),
    (360, 80),
    (500, 90),
    (700, 95),
    (900, 98),
];

/// The ladder's default: past 900 % the winner loses nobody at all.
pub const SURVIVAL_DEFAULT: i32 = 100;

/// The percentage of the winner that survives, for a strength ratio in per
/// cent. The ratio is always ≥ 100 because it is the larger score over the
/// smaller.
///
/// ```
/// # use l2_kingdom::battle::survival_percent;
/// assert_eq!(survival_percent(100), 10, "an even fight destroys both armies");
/// assert_eq!(survival_percent(112), 20);
/// assert_eq!(survival_percent(1000), 100, "a tenfold advantage is free");
/// ```
pub fn survival_percent(ratio: i32) -> i32 {
    SURVIVAL_LADDER
        .iter()
        .find(|&&(threshold, _)| ratio < threshold)
        .map_or(SURVIVAL_DEFAULT, |&(_, value)| value)
}

/// What a castle multiplies the **defender's** strength by in the autocalc, by
/// castle level 0 … 4 — 160 %, 200 %, 250 %, 320 %, 400 %.
///
/// Sieges are out of scope, so nothing in this crate passes a level yet; the
/// table is here because it is half of [`auto_resolve`]'s arithmetic and
/// leaving it out would make the function look symmetric when it is not.
pub const CASTLE_STRENGTH_PERCENT: [i32; 5] = [160, 200, 250, 320, 400];

/// Who won, and by how much — [`auto_resolve`]'s answer, and the argument
/// [`return_to_campaign`] takes.
///
/// > ### The name in the binary is inverted, and this type exists to stop that
/// > spreading.
/// >
/// > `g_battleLoser` (`0x0057C924`) holds the **winner**. Three independent
/// > sites agree against the name: the autocalc assigns it the side whose
/// > troops it then *keeps*; `Battle_ReturnToCampaign`'s
/// > `g_battleLoser == g_battleArmyA` branch hands the county to A and destroys
/// > B; and `FUN_00478419` maps `g_localPlayer == g_units[g_battleLoser].owner`
/// > to `L2.eng` group 82's *"won"* string. Implemented on the name, you
/// > destroy the winner and hand the county to the corpse.
/// >
/// > So this type has no field called `loser` next to a field called `winner`
/// > and no way to fill them in the wrong order: [`Verdict::a_won`] and
/// > [`Verdict::b_won`] are the only constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    /// The attacker — `g_battleArmyA`. At every call site in the original this
    /// is the unit that moved.
    pub attacker: usize,
    /// The defender — `g_battleArmyB`. At every call site this is the garrison,
    /// the army found at the town, or the defence just levied.
    pub defender: usize,
    /// True when the attacker won.
    pub attacker_won: bool,
    /// The percentage of itself the winner kept. Only the autocalc produces
    /// one; a fought battle counts the survivors instead and leaves this
    /// `None`.
    pub survival_percent: Option<i32>,
}

impl Verdict {
    pub fn a_won(attacker: usize, defender: usize) -> Verdict {
        Verdict { attacker, defender, attacker_won: true, survival_percent: None }
    }

    pub fn b_won(attacker: usize, defender: usize) -> Verdict {
        Verdict { attacker, defender, attacker_won: false, survival_percent: None }
    }

    pub fn winner(&self) -> usize {
        if self.attacker_won {
            self.attacker
        } else {
            self.defender
        }
    }

    pub fn loser(&self) -> usize {
        if self.attacker_won {
            self.defender
        } else {
            self.attacker
        }
    }
}

/// `FUN_004AAD07` (`0x004AAD07`) — **the autocalc**, the whole of a battle
/// nobody watches.
///
/// ```c
/// sA = Army_StrengthScore(A);  sB = Army_StrengthScore(B);
/// if (siege) sB = Pct(sB, castleBonus[level]);
/// ratio = (sA < sB) ? PctOf(sB, sA) : PctOf(sA, sB);   /* bigger * 100 / smaller */
/// p = Table_Lookup(ratio, survivalLadder, 10, 100);
/// winner = (sA < sB) ? B : A;                          /* a tie goes to A */
/// for t in 0..7: winner.troops[t] = Pct(winner.troops[t], p);
/// winner.merc.men = Pct(winner.merc.men, p);
/// winner.men = Σ winner.troops + winner.merc.men;
/// loser.troops[..] = 0;  release the loser's band;  loser.men = 0;
/// ```
///
/// **A tie goes to the attacker**, because the test is `sA < sB` and nothing
/// else. And the castle bonus is applied to B alone — B is the defender at
/// every one of the three call sites, so the asymmetry is not a bug in the
/// reading.
///
/// The loser is *emptied* here and *destroyed* in [`return_to_campaign`]; the
/// two are separate because a real battle empties it by killing men instead.
///
/// `castle_level` is `Some(level)` for a siege. Sieges are out of scope and
/// nothing in this crate passes one yet.
pub fn auto_resolve(
    units: &mut Units,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
) -> Option<Verdict> {
    let strength_a = units.get(attacker)?.strength_score();
    let mut strength_b = units.get(defender)?.strength_score();
    if let Some(level) = castle_level {
        let bonus = CASTLE_STRENGTH_PERCENT
            [(level as usize).min(CASTLE_STRENGTH_PERCENT.len() - 1)];
        strength_b = pct(strength_b, bonus);
    }

    // `PctOf(a, b) = a * 100 / b` — the larger over the smaller, so the ratio
    // never falls below 100 and the ladder is only ever read from one side.
    let attacker_won = strength_a >= strength_b;
    let (bigger, smaller) =
        if attacker_won { (strength_a, strength_b) } else { (strength_b, strength_a) };
    let ratio = if smaller == 0 { 0 } else { bigger * 100 / smaller };
    let percent = survival_percent(ratio);

    let mut verdict = if attacker_won {
        Verdict::a_won(attacker, defender)
    } else {
        Verdict::b_won(attacker, defender)
    };
    verdict.survival_percent = Some(percent);

    if let Some(w) = units.get_mut(verdict.winner()) {
        apply_survival(w, percent);
    }
    if let Some(l) = units.get_mut(verdict.loser()) {
        l.troops = [0; TROOP_TYPES];
        l.mercenaries = None;
        l.men = 0;
    }
    Some(verdict)
}

/// Keep `percent` of every count, then rebuild the total from what is left —
/// the original recomputes `+0x168` by summing rather than scaling it, so the
/// total and the counts cannot drift apart however the rounding falls.
fn apply_survival(unit: &mut Unit, percent: i32) {
    for t in 0..TROOP_TYPES {
        unit.troops[t] = pct(unit.troops[t], percent);
    }
    if let Some(band) = unit.mercenaries.as_mut() {
        band.men = pct(band.men as i32, percent).clamp(0, u8::MAX as i32) as u8;
    }
    unit.men = unit.troops.iter().sum::<i32>() + unit.mercenary_men();
}

// ----------------------------------------------- §2a what the banner says

/// `g_battleOutcome` (`0x00553FB8`) — which of `L2.eng` group 82's **seven**
/// heading/body pairs `Screen_BattleOutcome` (screen `0x2B`) shows.
///
/// A battle does not end two ways. `FUN_00478419` (`0x00478419`) sorts it into
/// six by two questions — was it a siege, and is the local player the winner —
/// and the seventh is the pair for a battle the local player was in neither
/// side of.
///
/// **The four siege arms each read as one sentence, and that is the check on
/// the mapping.** A is the besieger and B the garrison, so: the local player
/// won as A took the castle (*siege won*); won as B beat the besieger off
/// (*siege lifted*); lost as B and the castle fell (*castle lost*); lost as A
/// and was driven off (*siege lost*). Four arms, four different things, none of
/// them interchangeable — a wrong mapping would produce at least one sentence
/// that made no sense.
///
/// It is also a **fourth independent site confirming that `g_battleLoser`
/// holds the winner**: `DAT_005440CC` is `g_units[g_battleLoser].owner`, and
/// this function maps `g_localPlayer == DAT_005440CC` onto the *won* pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Outcome {
    /// Pair 0/1 — a field battle the local player won.
    Won = 0,
    /// Pair 2/3 — a field battle the local player lost.
    Lost = 1,
    /// Pair 4/5 — the local player besieged a castle and took it.
    SiegeWon = 2,
    /// Pair 6/7 — the local player besieged a castle and was driven off.
    SiegeLost = 3,
    /// Pair 8/9 — the local player held the castle and the siege was lifted.
    SiegeLifted = 4,
    /// Pair 10/11 — the local player held the castle and lost it.
    CastleLost = 5,
    /// Pair 12/13, *"The conflict is over."* — a battle between two other
    /// realms, which the local player is only told about.
    Bystander = 6,
}

impl Outcome {
    /// The index into `L2.eng` group 82: the heading is at `2n` and the body at
    /// `2n + 1`.
    pub fn pair(self) -> usize {
        self as usize
    }
}

/// `FUN_00478419` — which banner this battle draws for `local_player`.
///
/// `winner_owner` is the realm of the side that held the field. A local player
/// who owns neither army gets [`Outcome::Bystander`]; the original reaches that
/// pair by a different route (`DAT_005533D0 == 0` never brings screen `0x2B`
/// up at all) and the pair exists for it. `[D]` on the seventh, `[V]` on the
/// six.
pub fn outcome(
    verdict: Verdict,
    is_siege: bool,
    local_player: u8,
    winner_owner: u8,
    loser_owner: u8,
) -> Outcome {
    if local_player != winner_owner && local_player != loser_owner {
        return Outcome::Bystander;
    }
    let local_won = local_player == winner_owner;
    match (is_siege, local_won, verdict.attacker_won) {
        (false, true, _) => Outcome::Won,
        (false, false, _) => Outcome::Lost,
        (true, true, true) => Outcome::SiegeWon,
        (true, true, false) => Outcome::SiegeLifted,
        (true, false, true) => Outcome::CastleLost,
        (true, false, false) => Outcome::SiegeLost,
    }
}

// ------------------------------------------------- §3 back onto the campaign

/// What [`return_to_campaign`] did, for a caller that has messages, sounds or a
/// diplomacy layer to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Aftermath {
    /// The county changed hands, and to whom.
    pub county_taken_by: Option<u8>,
    /// The loser's slot was emptied.
    pub loser_destroyed: bool,
    /// The loser **survived**, and all that happened to it was that its siege
    /// was lifted. See [`return_to_campaign`]'s loser branch: two separate
    /// rules reach this, and only one of them is the 50-men one.
    pub loser_siege_lifted: bool,
    /// `Diplo_Offend(loserOwner, winnerOwner, 20)` — the loser's realm resents
    /// the winner's. `None` when the loser was ownerless.
    pub offence: Option<(u8, u8)>,
    /// The moves the winner was charged.
    pub winner_moves_used: i32,
    /// Men [`withdraw_casualties`] took off the loser on its way out, or `None`
    /// when this battle was not ended by a withdrawal.
    ///
    /// It is reported rather than merely done because it is charged **before**
    /// the loser is destroyed, so a caller diffing the unit array afterwards
    /// cannot tell a retreat's losses from the whole army's.
    pub withdrawal_casualties: Option<i32>,
    /// The realm that lost, **captured before its record was emptied** — the
    /// argument `Realm_RecountStrength` (`0x0049B42B`) takes at the bottom of
    /// both of `Battle_ReturnToCampaign`'s branches.
    ///
    /// That call is one of only four places a realm can be eliminated, and it
    /// is the one that fires the instant a realm's last army dies rather than
    /// waiting for its own turn to come round. It is reported here rather than
    /// made here because [`crate::victory::recount_strength`] needs the county
    /// count and the local player, and neither is a battle rule.
    pub loser_owner: u8,
}

/// The diplomatic hit the loser's realm takes against the winner's.
pub const BATTLE_OFFENCE: i32 = 20;

/// The moves a **winning attacker** is charged on top of the eight
/// [`crate::conquest::attack_county`] already took — so taking a county in a
/// fight costs sixteen of fifteen, and the army is finished for the season
/// either way.
pub const WINNER_MOVE_COST: i32 = 8;

/// The moves a **winning AI defender** is charged. A winning *human* defender
/// pays nothing at all.
pub const AI_DEFENDER_MOVE_COST: i32 = 7;

/// `Battle_ReturnToCampaign` (`0x004AB383`) — apply a settled battle to the
/// campaign.
///
/// The two branches are **not** mirror images and the differences are the
/// content of the function:
///
/// | | attacker wins | defender wins |
/// |---|---|---|
/// | county changes hands | yes, if the defender was a garrison **or** carried a defence mark | **never** |
/// | county's garrison link cleared | if the defender was a garrison | if the *attacker* was a garrison |
/// | winner's moves | `+8`, then an **AI** is set to `allowance − 1` | an **AI** pays `+7`; a human pays nothing |
/// | loser | destroyed | destroyed |
/// | diplomacy | −20 from the loser's realm | −20 from the loser's realm |
///
/// **`unit.owner_is_human == false` is the AI**, which `docs/armies.md` §7 had
/// the other way round before it was corrected; the moves column above is the
/// shape that correction produces.
///
/// The county cannot change hands to a defender because `g_battleArmyB` is the
/// defender at all three of the original's call sites: there is no
/// `County_ChangeOwner` anywhere in the B-wins branch. A defender that wins
/// keeps a county it already had — or, for a neutral county's levy, keeps it
/// neutral.
///
/// > **Corrects `docs/armies.md` §7 and `docs/symbols.md` a third time.** Both
/// > say the loser survives with its siege merely lifted when it has ≥ 50 men
/// > *"under autocalc"*. The gate is `DAT_0056D5C8`, which
/// > [`auto_resolve`]'s first statement **clears**; it is raised in exactly one
/// > place, `UnitOrder_SiegeAttKnight`, when an all-knight AI besieger gives up
/// > on an unbreached wall. So it is a *siege-withdrawal* rule and under
/// > autocalc the loser is always destroyed. That path is unreachable from
/// > this crate and is deliberately not modelled here. `[V]` — the write and
/// > the three clears are the only four sites the flag has. See correction C31.
///
/// > ### ⚠ And C31 stopped one branch too early. **A besieger that loses but
/// > still has men is not destroyed either, and that rule has no flag on it.**
/// >
/// > The loser branch is two nested tests, not one:
/// >
/// > ```c
/// > if (loser.besiegingCounty == 0 || loser.menTotal == 0) {
/// >     if (withdrawal) {
/// >         if (loser.menTotal < 50) { message 0x120; Army_Destroy(loser); }
/// >         else                       loser.besiegingCounty = 0;
/// >     } else Army_Destroy(loser);
/// > } else loser.besiegingCounty = 0;      /* <- the outer else */
/// > ```
/// >
/// > C31 read the inner test — the 50-men rule, gated on the withdrawal flag —
/// > and concluded that *"under autocalc the loser is always destroyed"*. That
/// > conclusion is true, but for a different reason than the one given: under
/// > autocalc the loser's men are set to **zero**, so the outer test passes and
/// > the inner one runs. In a **fought** siege the loser can walk off the field
/// > with men, and then the outer `else` fires: **the besieging army survives
/// > and its siege is merely lifted.** That is the rule a repulsed assault
/// > needs, it is reachable from `l2-sim` and from nowhere else, and both
/// > `docs/armies.md` §7 and C31 are silent about it. See correction C38.
///
/// # The Readme calls the withdrawal a *retreat*
///
/// The shipped `Readme.txt`'s *Retreats (pg82)*: *"Armies that retreat will
/// suffer some casualties. Any army that would have less than 50 men after
/// retreating is eliminated instead."* That is the inner branch in English, and
/// it is the game's own documentation of a rule the code reaches from **one**
/// place — `UnitOrder_SiegeAttKnight`, an all-knight AI besieger giving up on
/// an unbreached wall. The errata describe the rule as general and the shipped
/// binary makes it specific; where they differ the code is what shipped, and
/// `withdrawal` is a parameter here so a caller with a retreat of its own can
/// reach the branch honestly.
#[allow(clippy::too_many_arguments)]
pub fn return_to_campaign(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    verdict: Verdict,
    county: u8,
    is_siege: bool,
    withdrawal: bool,
    difficulty: u8,
) -> Aftermath {
    let mut out = Aftermath::default();
    let (winner, loser) = (verdict.winner(), verdict.loser());
    let Some(loser_unit) = units.get(loser).cloned() else { return out };
    let Some(winner_unit) = units.get(winner).cloned() else { return out };

    if verdict.attacker_won {
        // The defender was the castle's garrison: the castle is no longer held,
        // and the county goes with it.
        if loser_unit.garrison_county != 0 {
            if let Some(c) = counties.get_mut(county as usize) {
                c.garrison_unit = 0;
            }
            crate::conquest::change_owner(
                counties, realms, units, winner_unit.owner, county, difficulty,
            );
            out.county_taken_by = Some(winner_unit.owner);
        }
        // …or it was raised to defend the county, which is the same conclusion
        // by the other route. The original really does call `County_ChangeOwner`
        // twice when both hold; the second is a no-op on an already-flipped
        // county, and so is this.
        if loser_unit.defence_mark != 0 {
            crate::conquest::change_owner(
                counties, realms, units, winner_unit.owner, county, difficulty,
            );
            out.county_taken_by = Some(winner_unit.owner);
        }
    } else if loser_unit.garrison_county != 0 {
        // A garrison that sallied out and lost stops being the castle's
        // garrison — but the county does **not** change hands, because the
        // winner is the side that already held it.
        if let Some(c) = counties.get_mut(county as usize) {
            c.garrison_unit = 0;
        }
    }

    if let Some(w) = units.get_mut(winner) {
        if is_siege {
            // A siege that ended is a siege link that has to go, whichever
            // side won. The original clears the *besieger's* `+0x199` in the
            // A-wins branch and the *garrison's* `+0x19A` in the B-wins one —
            // the winner's own half in each case.
            w.besieging_county = 0;
            w.besieged_by = 0;
        }
        if verdict.attacker_won {
            w.besieged_by = 0;
            w.moves_used += WINNER_MOVE_COST;
            if !w.owner_is_human {
                // A winning AI is left with exactly one move, whatever it spent
                // getting here. A winning human keeps everything but the eight.
                w.moves_used = w.move_allowance - 1;
            }
        } else if !w.owner_is_human {
            w.moves_used += AI_DEFENDER_MOVE_COST;
        }
        out.winner_moves_used = w.moves_used;
    }
    // `if (!isSiege) Siege_RecomputeBuildTime(winner)`. The winner lost men, so
    // a siege it is *itself* laying somewhere else now needs a different number
    // of seasons. On the siege path the link has just been cleared instead.
    if !is_siege {
        crate::siege::recompute_build_time(units, winner);
    }

    // `Army_ClearBattleSlots` (`0x004AA89F`) zeroes the four battle-only troop
    // slots on both sides. `Unit::troops` is the seven campaign counts and has
    // no room for them, so there is nothing to clear — the slots only exist
    // once sieges do.

    // `Diplo_Offend(loser.owner, winner.owner, 20)`. The original guards on
    // `loser.owner != 0`, which an **ownerless militia's 6 passes** — it then
    // indexes a five-realm table with 6. We refuse instead of reproducing an
    // out-of-bounds write: a county's own people have no realm to hold a
    // grudge with.
    if loser_unit.owner != 0 && (loser_unit.owner as usize) < MAX_REALMS {
        out.offence = Some((loser_unit.owner, winner_unit.owner));
    }
    out.loser_owner = loser_unit.owner;

    // **`Army_WithdrawCasualties` (`0x004AD8CC`) — the price of leaving the
    // field, and it is charged *before* anything reads the loser's total.**
    //
    // `if (g_battleWithdrawal == 1) Army_WithdrawCasualties(loser);` sits above
    // the whole loser branch in both arms of the original. Half of every troop
    // count, and a count under eleven is wiped outright; the total is rebuilt
    // by summing, so the two tests below read the *withdrawn* army and not the
    // one that walked on.
    let withdrawn_men = if withdrawal {
        Some(withdraw_casualties(t, units, realms, loser, difficulty))
    } else {
        None
    };
    out.withdrawal_casualties = withdrawn_men.map(|left| loser_unit.men - left);
    let loser_men = withdrawn_men.unwrap_or(loser_unit.men);

    // **The loser branch, both rules.** See the correction above the function.
    let still_besieging = loser_unit.besieging_county != 0 && loser_men != 0;
    let survives = still_besieging || (withdrawal && loser_men >= WITHDRAWAL_SURVIVAL_MEN);
    if survives {
        if let Some(l) = units.get_mut(loser) {
            l.besieging_county = 0;
        }
        // The garrison's half of the link goes with it, or the pair is left
        // half-connected for `Siege_StartPhase` to find next turn.
        if let Some(g) = counties.get(county as usize).map(|c| c.garrison_unit) {
            if let Some(g) = units.get_mut(g) {
                if g.besieged_by == loser as u8 {
                    g.besieged_by = 0;
                }
            }
        }
        out.loser_siege_lifted = true;
    } else {
        crate::unit::destroy(t, units, realms, names, loser, difficulty);
        out.loser_destroyed = true;
    }
    out
}

/// The men a **withdrawing** army needs to survive its withdrawal — the
/// Readme's *Retreats (pg82)* number, and `Battle_ReturnToCampaign`'s
/// `menTotal < 0x32`. Under it, message `0x120` (`L2.eng` group 288) and the
/// army is destroyed.
///
/// **It is measured after [`withdraw_casualties`] has run**, which is what the
/// Readme's *"any army that would have less than 50 men **after** retreating"*
/// says and what this crate used to get wrong: the test read the total the army
/// walked onto the field with, so an army of 80 survived at 80 where the
/// original halves it to 40 and destroys it.
pub const WITHDRAWAL_SURVIVAL_MEN: i32 = 50;

/// The count below which a troop line is wiped outright rather than halved —
/// `Army_WithdrawCasualties`' `if (n < 0xB) n = 0;`.
///
/// Ten men do not retreat in good order; eleven lose five. It is the same shape
/// as [`crate::unit::desert`]'s *"only where the count exceeds ten"* guard, and
/// the two are the only places in the campaign that treat a small troop line
/// differently from a large one.
pub const WITHDRAWAL_WIPE_BELOW: i32 = 11;

/// **`Army_WithdrawCasualties` (`FUN_004AD8CC`, `0x004AD8CC`) — what leaving the
/// field costs.**
///
/// ```c
/// total = 0;
/// for (t = 0; t < 7; t++) {
///     n = troops[t];
///     if (n < 0xB) n = 0; else n = n / 2;
///     troops[t] = n;  total += n;
/// }
/// menTotal = total;
/// menTotal += mercMen;          /* the band is NOT halved */
/// pathLen  = 0;                 /* +0x1C  */
/// moving   = 0;                 /* +0x14C */
/// Wages_ForUnit(unit);
/// ```
///
/// Four things a reimplementation gets wrong by default, and all four are in
/// those nine lines:
///
/// * **The mercenary band does not lose a man.** It is added to the rebuilt
///   total and never scaled — unlike the autocalc, which scales it with
///   everything else. A retreating army of hirelings retreats intact.
/// * **A line under eleven is wiped, not halved.** Six knights become none; six
///   knights and six peasants become nobody at all.
/// * **The total is rebuilt by summing**, so it cannot drift from the counts
///   however the integer division falls — the same discipline
///   [`auto_resolve`] uses.
/// * **The army stops where it stands.** The path is thrown away and the move
///   state cleared, so a retreat cancels the order that walked into the battle
///   rather than resuming it. That is the half of this function that is not a
///   casualty rule at all, and it is why the army is not carried on into a
///   second fight on the same turn.
///
/// Returns the men left, which is the number
/// [`WITHDRAWAL_SURVIVAL_MEN`] is tested against.
///
/// > **The only thing in `Lords2.exe` that reaches this is a siege.**
/// > `g_battleWithdrawal` (`0x0056D5C8`) has exactly one writer —
/// > `UnitOrder_SiegeAttKnight` (`0x0048D9CE`), an AI besieger whose whole
/// > force is knights facing an unbreached wall — and three clearers, one of
/// > which is [`auto_resolve`]'s first statement. **The Retreat button is not
/// > one of them**: `FUN_0043BA29`'s confirm reaches `FUN_0043BE65`, which is
/// > the autocalc and the return, so a player who "retreats" has auto-resolved
/// > the battle and, if the ladder says he lost, watched his army destroyed
/// > rather than withdrawn. See this module's `retreat` note.
pub fn withdraw_casualties(
    t: &Tables,
    units: &mut Units,
    realms: &mut [Realm; MAX_REALMS],
    id: usize,
    difficulty: u8,
) -> i32 {
    let Some(unit) = units.get_mut(id) else { return 0 };
    let mut total = 0;
    for slot in unit.troops.iter_mut() {
        *slot = if *slot < WITHDRAWAL_WIPE_BELOW { 0 } else { *slot / 2 };
        total += *slot;
    }
    unit.men = total + unit.mercenary_men();
    unit.path.clear();
    unit.moving = false;
    let (owner, left) = (unit.owner, unit.men);
    crate::unit::refresh_wages(t, units, realms, owner, difficulty);
    left
}

// ------------------------------------------------------ §4 the levy goes home

/// `Defence_Disband` (`0x004ABA5A`) — **the temporary defence is undone and its
/// survivors walk back into the county.**
///
/// ```c
/// if (unit.defenceMark == 0) return;
/// if (unit.defenceMark < 2) {
///     county[unit.homeCounty].population += unit.menTotal;
///     county[unit.homeCounty].popArmy    += unit.menTotal;
///     Army_Destroy(unit);
/// } else {
///     unit.defenceMark = 0;
/// }
/// ```
///
/// **Every survivor, not a fraction**, and into the population *and* the
/// panel's *"Army"* line, exactly as the levy debited both. Mark 2 — an army
/// that already existed and was pressed into defending — is not disbanded at
/// all; it only loses the mark.
///
/// This runs **after** [`return_to_campaign`], which is what makes it correct
/// in both directions: a defence that lost has already been destroyed and this
/// finds nothing, and a defence that won is still standing with its survivors
/// in [`Unit::men`](crate::unit::Unit::men).
///
/// Returns the men returned to the county.
///
/// > `[V]` **against the battle fixture triple, and it closes exactly.**
/// > County 3 holds 728 people in `battle-before.sav`; a 25 % militia of 182 is
/// > levied and it holds 546 in `battle-during.sav`; the defence wins with 36
/// > men and it holds 582 in `battle-after.sav`. `546 + 36 = 582`.
pub fn disband_defence(
    t: &Tables,
    counties: &mut [County; MAX_COUNTIES],
    realms: &mut [Realm; MAX_REALMS],
    units: &mut Units,
    names: &mut ArmyNames,
    id: usize,
    difficulty: u8,
) -> i32 {
    let Some(unit) = units.get(id) else { return 0 };
    match unit.defence_mark {
        0 => 0,
        1 => {
            let (home, men) = (unit.home_county, unit.men);
            if let Some(c) = counties.get_mut(home as usize) {
                c.population += men;
                c.army += men;
            }
            crate::unit::destroy(t, units, realms, names, id, difficulty);
            men
        }
        _ => {
            if let Some(u) = units.get_mut(id) {
                u.defence_mark = 0;
            }
            0
        }
    }
}

/// `Armies_ReturnDefences` (`0x004AD39A`) — the turn-end backstop.
///
/// Sweeps slots 1 … 150, runs [`disband_defence`] on **every** army, and then
/// destroys any whose county byte is above 16 — an army standing outside the
/// map's county set. The comma operator in the original's condition is what
/// makes the disband unconditional and the destroy selective, and it is easy to
/// read the other way:
///
/// ```c
/// if ((g_units[i].kind == 1) && (Defence_Disband(i), 0x10 < g_units[i].county))
///     Army_Destroy(i);
/// ```
///
/// Called first thing in the wage pass, so a defence that somehow outlived its
/// battle is gone before anyone is paid for it.
pub const STRAY_COUNTY_LIMIT: u8 = 16;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{TroopType, UnitKind};

    const T: &Tables = &Tables::DEFAULT;

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for i in 1..4 {
            counties[i].population = 500;
            counties[i].happiness = 80;
        }
        realms[1].in_play = true;
        realms[1].is_human = true;
        realms[2].in_play = true;
        (counties, realms)
    }

    fn army(units: &mut Units, owner: u8, human: bool, troops: &[(TroopType, i32)]) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, 10, 10);
        u.owner_is_human = human;
        for &(t, n) in troops {
            u.troops[t.index()] = n;
        }
        u.men = u.troops.iter().sum();
        units.spawn(u).expect("a slot")
    }

    // --- §1 the gate -------------------------------------------------------

    #[test]
    fn two_ai_armies_never_reach_a_screen() {
        let mut units = Units::new();
        let a = army(&mut units, 2, false, &[(TroopType::Peasant, 100)]);
        let b = army(&mut units, 3, false, &[(TroopType::Peasant, 100)]);
        // The option cannot make an AI-versus-AI battle interactive.
        for byte in [0, 1] {
            assert_eq!(settlement(&units, a, b, byte), Settlement::Silently);
        }
    }

    #[test]
    fn the_option_decides_whether_a_human_is_asked_and_two_humans_always_are() {
        let mut units = Units::new();
        let human = army(&mut units, 1, true, &[(TroopType::Peasant, 100)]);
        let ai = army(&mut units, 2, false, &[(TroopType::Peasant, 100)]);
        let other = army(&mut units, 3, true, &[(TroopType::Peasant, 100)]);

        // The byte is stored inverted: 0 is the option displaying *Yes*.
        assert_eq!(settlement(&units, human, ai, 0), Settlement::Reported);
        assert_eq!(settlement(&units, human, ai, 1), Settlement::Prompt);
        // …and it never applies when both sides are people.
        assert_eq!(settlement(&units, human, other, 0), Settlement::Prompt);
    }

    // --- §2 the autocalc ---------------------------------------------------

    /// The ladder's first rung, which is the one that decides most battles.
    #[test]
    fn an_even_fight_leaves_the_winner_a_tenth_of_its_army() {
        assert_eq!(survival_percent(100), 10);
        assert_eq!(survival_percent(109), 10);
        assert_eq!(survival_percent(110), 20, "the threshold is exclusive");
        assert_eq!(survival_percent(899), 98);
        assert_eq!(survival_percent(900), SURVIVAL_DEFAULT);
    }

    #[test]
    fn a_tie_goes_to_the_attacker() {
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 100)]);
        let b = army(&mut units, 2, false, &[(TroopType::Peasant, 100)]);
        let v = auto_resolve(&mut units, a, b, None).unwrap();
        assert!(v.attacker_won, "equal scores, and the test is `sA < sB`");
        assert_eq!(v.winner(), a);
        assert_eq!(v.loser(), b);
        // Ratio 100 is the first rung: the winner keeps a tenth.
        assert_eq!(units.get(a).unwrap().men, 10);
        assert_eq!(units.get(b).unwrap().men, 0);
    }

    /// The total is rebuilt by summing, not by scaling, so it can never drift
    /// from the counts however the rounding falls.
    #[test]
    fn the_winners_total_is_the_sum_of_its_surviving_counts() {
        let mut units = Units::new();
        let a = army(
            &mut units,
            1,
            true,
            &[(TroopType::Knight, 100), (TroopType::Archer, 33), (TroopType::Peasant, 7)],
        );
        let b = army(&mut units, 2, false, &[(TroopType::Peasant, 3)]);
        let v = auto_resolve(&mut units, a, b, None).unwrap();
        assert!(v.attacker_won);
        let w = units.get(a).unwrap();
        assert_eq!(w.men, w.troops.iter().sum::<i32>());
        assert_eq!(v.survival_percent, Some(SURVIVAL_DEFAULT), "an overwhelming advantage");
        assert_eq!(w.troops[TroopType::Archer.index()], 33, "and it costs nobody");
    }

    #[test]
    fn the_castle_bonus_can_turn_a_losing_defence_into_a_winning_one() {
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Swordsman, 100)]);
        let b = army(&mut units, 2, false, &[(TroopType::Swordsman, 90)]);
        let open = auto_resolve(&mut units.clone(), a, b, None).unwrap();
        assert!(open.attacker_won, "in the open the bigger army wins");
        let sieged = auto_resolve(&mut units, a, b, Some(0)).unwrap();
        assert!(!sieged.attacker_won, "160% of 90 beats 100");
    }

    // --- §2a the banner ----------------------------------------------------

    /// The six arms, each of which has to read as a different sentence. A is
    /// the besieger and B the garrison, so swapping any pair produces a banner
    /// that says the opposite of what happened.
    #[test]
    fn the_four_siege_banners_each_say_a_different_thing() {
        let a_won = Verdict::a_won(1, 2);
        let b_won = Verdict::b_won(1, 2);
        // Realm 1 besieges realm 2's castle.
        let (besieger, garrison) = (1, 2);

        // The player is the besieger.
        assert_eq!(outcome(a_won, true, besieger, besieger, garrison), Outcome::SiegeWon);
        assert_eq!(outcome(b_won, true, besieger, garrison, besieger), Outcome::SiegeLost);
        // The player holds the castle.
        assert_eq!(outcome(b_won, true, garrison, garrison, besieger), Outcome::SiegeLifted);
        assert_eq!(outcome(a_won, true, garrison, besieger, garrison), Outcome::CastleLost);
        // A field battle has only two.
        assert_eq!(outcome(a_won, false, 1, 1, 2), Outcome::Won);
        assert_eq!(outcome(a_won, false, 2, 1, 2), Outcome::Lost);
        // And the seventh pair is for somebody else's war.
        assert_eq!(outcome(a_won, false, 4, 1, 2), Outcome::Bystander);
    }

    /// The pair indices are the string offsets `Screen_BattleOutcome` reads, so
    /// they are the one thing here that must not drift.
    #[test]
    fn the_seven_pairs_are_numbered_nought_to_six() {
        let all = [
            Outcome::Won,
            Outcome::Lost,
            Outcome::SiegeWon,
            Outcome::SiegeLost,
            Outcome::SiegeLifted,
            Outcome::CastleLost,
            Outcome::Bystander,
        ];
        for (i, o) in all.iter().enumerate() {
            assert_eq!(o.pair(), i);
        }
    }

    // --- §3 the return -----------------------------------------------------

    #[test]
    fn a_winning_attacker_takes_the_county_from_a_marked_defence() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Knight, 100)]);
        let d = army(&mut units, 6, false, &[(TroopType::Peasant, 50)]);
        units.get_mut(d).unwrap().defence_mark = 1;
        counties[2].owner = 0;
        units.get_mut(a).unwrap().moves_used = 8;

        let v = Verdict::a_won(a, d);
        let after =
            return_to_campaign(T, &mut counties, &mut realms, &mut units, &mut names, v, 2, false, false, 1);

        assert_eq!(after.county_taken_by, Some(1));
        assert_eq!(counties[2].owner, 1, "the county changed hands");
        assert!(after.loser_destroyed);
        assert!(units.get(d).is_none(), "the loser is gone");
        assert_eq!(
            units.get(a).unwrap().moves_used,
            16,
            "eight from the attack and eight more from winning"
        );
        assert_eq!(after.offence, None, "an ownerless militia has no realm to resent with");
    }

    /// The half `docs/armies.md` §7 inverted, stated as a test: **a defender
    /// that wins never takes the county**, because there is no
    /// `County_ChangeOwner` in that branch at all.
    #[test]
    fn a_winning_defender_leaves_the_county_exactly_where_it_was() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 50)]);
        let d = army(&mut units, 6, false, &[(TroopType::Knight, 100)]);
        units.get_mut(d).unwrap().defence_mark = 1;
        counties[2].owner = 0;

        let v = Verdict::b_won(a, d);
        let after =
            return_to_campaign(T, &mut counties, &mut realms, &mut units, &mut names, v, 2, false, false, 1);

        assert_eq!(after.county_taken_by, None);
        assert_eq!(counties[2].owner, 0, "still neutral");
        assert!(units.get(a).is_none(), "the attacker is destroyed");
        assert!(units.get(d).is_some(), "the defence is still standing");
        assert_eq!(after.offence, Some((1, 6)), "the attacker's realm resents the militia");
    }

    /// The move charge is asymmetric in both axes: who won, and whether the
    /// winner is a person.
    #[test]
    fn the_move_charge_is_asymmetric_and_the_ai_is_the_one_that_pays() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();

        // A winning AI attacker is left with exactly one move.
        let mut units = Units::new();
        let a = army(&mut units, 2, false, &[(TroopType::Knight, 100)]);
        let d = army(&mut units, 1, true, &[(TroopType::Peasant, 10)]);
        units.get_mut(a).unwrap().moves_used = 8;
        return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::a_won(a, d), 2, false, false, 1,
        );
        let w = units.get(a).unwrap();
        assert_eq!(w.moves_left(), 1, "a winning AI is finished for the season");

        // A winning human defender pays nothing whatsoever.
        let mut units = Units::new();
        let a = army(&mut units, 2, false, &[(TroopType::Peasant, 10)]);
        let d = army(&mut units, 1, true, &[(TroopType::Knight, 100)]);
        return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, false, false, 1,
        );
        assert_eq!(units.get(d).unwrap().moves_used, 0);

        // A winning AI defender pays seven — and seven, not eight.
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 10)]);
        let d = army(&mut units, 2, false, &[(TroopType::Knight, 100)]);
        return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, false, false, 1,
        );
        assert_eq!(units.get(d).unwrap().moves_used, AI_DEFENDER_MOVE_COST);
    }

    #[test]
    fn an_unmarked_defender_that_loses_does_not_hand_over_its_county() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Knight, 100)]);
        // A field battle: the loser is neither a garrison nor a county defence.
        let d = army(&mut units, 2, false, &[(TroopType::Peasant, 10)]);
        counties[2].owner = 2;

        let after = return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::a_won(a, d), 2, false, false, 1,
        );
        assert_eq!(after.county_taken_by, None);
        assert_eq!(counties[2].owner, 2, "beating an army in the field takes no land");
    }

    /// **A repulsed assault does not destroy the besieging army.**
    ///
    /// The rule C31 stopped one branch short of: a loser that is still linked
    /// as a besieger and still has men keeps its men and only loses the siege.
    /// It is unreachable under the autocalc — which zeroes the loser's men —
    /// and reachable from a fought battle, which is why it never showed up.
    #[test]
    fn a_besieger_that_loses_with_men_left_keeps_them_and_only_loses_the_siege() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 200)]);
        let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
        units.get_mut(a).unwrap().besieging_county = 2;
        units.get_mut(d).unwrap().garrison_county = 2;
        units.get_mut(d).unwrap().besieged_by = a as u8;
        counties[2].owner = 2;
        counties[2].garrison_unit = d;

        let after = return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, true, false, 1,
        );
        assert!(!after.loser_destroyed, "the assault failed; the army did not");
        assert!(after.loser_siege_lifted);
        assert_eq!(units.get(a).unwrap().men, 200, "it keeps every man it walked off with");
        assert_eq!(units.get(a).unwrap().besieging_county, 0, "and loses only the siege");
        assert_eq!(units.get(d).unwrap().besieged_by, 0, "both halves of the link");
        assert_eq!(counties[2].owner, 2, "the castle held");

        // …and the same battle with the besieger wiped out destroys it, which
        // is the outer test's other arm and the autocalc's only arm.
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 200)]);
        let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
        units.get_mut(a).unwrap().besieging_county = 2;
        units.get_mut(a).unwrap().men = 0;
        units.get_mut(a).unwrap().troops = [0; TROOP_TYPES];
        let after = return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, true, false, 1,
        );
        assert!(after.loser_destroyed);
        assert!(units.get(a).is_none());
    }

    /// The **withdrawal** rule, which is a different rule at the same site —
    /// the Readme's *Retreats (pg82)* and `menTotal < 50`.
    ///
    /// > **Corrected, and it moved the threshold by a factor of two.** This
    /// > test used to read `[(50, true), (49, false)]` — fifty men walked off
    /// > the field and fifty men arrived. They do not:
    /// > `Army_WithdrawCasualties` runs *above* the whole loser branch and
    /// > halves every troop line first, so the fifty the original tests is the
    /// > fifty *left*, and an army needs a **hundred** to make it. That is the
    /// > Readme's own wording — *"any army that would have less than 50 men
    /// > **after** retreating is eliminated instead"* — and this file had the
    /// > constant right and the order wrong. `docs/decisions.md`
    /// > C71.
    #[test]
    fn a_withdrawing_army_needs_a_hundred_men_to_walk_off_with_fifty() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        for (men, survives, left) in [(100, true, 50), (99, false, 49), (50, false, 25)] {
            let mut units = Units::new();
            let a = army(&mut units, 1, true, &[(TroopType::Peasant, men)]);
            let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
            // Not besieging any more — so only the withdrawal branch is left.
            let after = return_to_campaign(
                T, &mut counties, &mut realms, &mut units, &mut names,
                Verdict::b_won(a, d), 2, false, true, 1,
            );
            assert_eq!(units.get(a).is_some(), survives, "{men} men");
            assert_eq!(after.loser_siege_lifted, survives);
            // Charged either way: the casualties are taken before the army is
            // told whether it is going to live.
            assert_eq!(after.withdrawal_casualties, Some(men - left), "{men} men");
            if survives {
                assert_eq!(units.get(a).unwrap().men, left);
            }
        }
    }

    /// **`Army_WithdrawCasualties` on its own**, and the three details a
    /// reimplementation gets wrong.
    #[test]
    fn a_retreat_halves_every_line_wipes_the_small_ones_and_spares_the_band() {
        let (_counties, mut realms) = world();
        let mut units = Units::new();
        let a = army(
            &mut units,
            1,
            true,
            &[
                (TroopType::Peasant, 101),
                (TroopType::Knight, 11),
                (TroopType::Archer, 10),
                (TroopType::Pikeman, 1),
            ],
        );
        units.get_mut(a).unwrap().mercenaries = Some(crate::unit::Mercenaries {
            troop: TroopType::Swordsman,
            men: 40,
            band: 1,
        });
        let before = units.get(a).unwrap().men;
        assert_eq!(before, 123, "the band is not part of `troops`");

        let left = withdraw_casualties(T, &mut units, &mut realms, a, 1);
        let u = units.get(a).unwrap();
        assert_eq!(u.troops[TroopType::Peasant.index()], 50, "101 / 2, truncating");
        assert_eq!(u.troops[TroopType::Knight.index()], 5, "eleven is the first line that halves");
        assert_eq!(u.troops[TroopType::Archer.index()], 0, "ten is wiped outright");
        assert_eq!(u.troops[TroopType::Pikeman.index()], 0);
        assert_eq!(u.mercenary_men(), 40, "the band walks off whole");
        assert_eq!(left, 55 + 40);
        assert_eq!(u.men, u.troops.iter().sum::<i32>() + u.mercenary_men(), "summed, not scaled");
        assert!(!u.moving, "and it stops where it stands rather than resuming its order");
        assert!(u.path.is_empty());
    }

    /// An army every one of whose lines is under eleven is **annihilated by
    /// retreating** — the outer test then reads zero men and the inner one
    /// never gets a chance to spare it.
    #[test]
    fn an_army_of_small_lines_does_not_survive_a_retreat_at_all() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = army(
            &mut units,
            1,
            true,
            &[(TroopType::Knight, 10), (TroopType::Swordsman, 10), (TroopType::Maceman, 10)],
        );
        let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
        units.get_mut(a).unwrap().besieging_county = 2;
        let after = return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, true, true, 1,
        );
        assert_eq!(after.withdrawal_casualties, Some(30));
        assert!(after.loser_destroyed, "thirty men, none of them in a line of eleven");
        assert!(units.get(a).is_none());
    }

    /// And without the flag, fifty men buys nothing — which is the half of
    /// C31 that stands.
    #[test]
    fn without_a_withdrawal_a_loser_with_men_is_still_destroyed() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 500)]);
        let d = army(&mut units, 2, false, &[(TroopType::Archer, 100)]);
        let after = return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, false, false, 1,
        );
        assert!(after.loser_destroyed);
        assert!(units.get(a).is_none());
    }

    // --- §4 the disband ----------------------------------------------------

    #[test]
    fn a_levied_defence_walks_its_survivors_back_into_the_county() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let d = army(&mut units, 6, false, &[(TroopType::Peasant, 24), (TroopType::Archer, 12)]);
        {
            let u = units.get_mut(d).unwrap();
            u.defence_mark = 1;
            u.home_county = 3;
        }
        counties[3].population = 546;

        let back = disband_defence(T, &mut counties, &mut realms, &mut units, &mut names, d, 1);
        assert_eq!(back, 36);
        assert_eq!(counties[3].population, 582);
        assert!(units.get(d).is_none(), "a levied defence does not survive the battle it won");
    }

    #[test]
    fn an_existing_army_pressed_into_defending_only_loses_the_mark() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let d = army(&mut units, 2, false, &[(TroopType::Knight, 40)]);
        {
            let u = units.get_mut(d).unwrap();
            u.defence_mark = 2;
            u.home_county = 3;
        }
        counties[3].population = 546;

        let back = disband_defence(T, &mut counties, &mut realms, &mut units, &mut names, d, 1);
        assert_eq!(back, 0);
        assert_eq!(counties[3].population, 546, "nobody goes home; it was never a levy");
        assert_eq!(units.get(d).unwrap().defence_mark, 0);
        assert_eq!(units.get(d).unwrap().men, 40, "and the army is untouched");
    }

    #[test]
    fn disbanding_an_army_that_never_defended_anything_does_nothing() {
        let (mut counties, mut realms) = world();
        let mut names = ArmyNames::new();
        let mut units = Units::new();
        let u = army(&mut units, 1, true, &[(TroopType::Peasant, 100)]);
        counties[3].population = 546;
        assert_eq!(disband_defence(T, &mut counties, &mut realms, &mut units, &mut names, u, 1), 0);
        assert_eq!(counties[3].population, 546);
        assert!(units.get(u).is_some());
    }

    /// **The fixture battle, by arithmetic alone.** `battle-before.sav`'s two
    /// armies, resolved the way the original resolves a battle the player does
    /// not fight — and the survivor count it produces is the one the saved game
    /// holds. `crates/l2-game/tests/seam.rs` runs the same battle from the
    /// bytes; this is the arithmetic on its own, so a broken fixture and a
    /// broken ladder fail in different files.
    #[test]
    fn the_fixture_battle_resolves_to_the_thirty_six_men_the_save_holds() {
        let mut units = Units::new();
        let a = army(
            &mut units,
            1,
            true,
            &[(TroopType::Peasant, 128), (TroopType::Swordsman, 25), (TroopType::Archer, 25)],
        );
        let d = army(&mut units, 6, false, &[(TroopType::Peasant, 122), (TroopType::Archer, 60)]);

        // 128*2 + 25*13 + 25*13 = 906, +20; 122*2 + 60*13 = 1024, +20.
        assert_eq!(units.get(a).unwrap().strength_score(), 926);
        assert_eq!(units.get(d).unwrap().strength_score(), 1044);

        let v = auto_resolve(&mut units, a, d, None).unwrap();
        assert!(!v.attacker_won, "the player lost that battle, and the save says so");
        // 1044 * 100 / 926 = 112, and 112 is the ladder's second rung.
        assert_eq!(v.survival_percent, Some(20));

        let survivor = units.get(d).unwrap();
        assert_eq!(survivor.troops[TroopType::Peasant.index()], 24);
        assert_eq!(survivor.troops[TroopType::Archer.index()], 12);
        assert_eq!(survivor.men, 36, "county 3 goes 546 -> 582 across the save pair");
        assert_eq!(units.get(a).unwrap().men, 0);
    }
}
