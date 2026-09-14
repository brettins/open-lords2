//! **The campaign half of a battle**: choosing how it is settled, settling it
//! by arithmetic when nobody is watching, and applying the result.
//!
//! [`crate::conquest`] stops at `Attack::Battle { attacker, defender }` and
//! says the caller hands the pair to `l2-sim`. This is what happens on either
//! side of that hand-off:
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

mod settlement;
pub use settlement::*;
mod campaign;
pub use campaign::*;

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
/// `0x13` — a battle they are told about.
    Reported,
    /// Screen `0x12`: *"A Battle is to be fought. Will you take the field?"*
    /// Taking the field is the real simulation;
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
/// It is reported because it is charged **before**
    /// the loser is destroyed,
    /// cannot tell a retreat's losses from the whole army's.
    pub withdrawal_casualties: Option<i32>,
    /// The realm that lost, **captured before its record was emptied** — the
    /// argument `Realm_RecountStrength` (`0x0049B42B`) takes at the bottom of
    /// both of `Battle_ReturnToCampaign`'s branches.
    ///
    /// That call is one of only four places a realm can be eliminated, and it
/// is the one that fires the instant a realm's last army dies
/// It is reported here
    /// made here because [`crate::victory::recount_strength`] needs the county
    /// count and the local player, and neither is a battle rule.
    pub loser_owner: u8,
    /// **What each `County_ChangeOwner` call knew**, in call order, for the
    /// letters it posts. Two slots because the attacker-wins branch calls it
    /// once for a beaten garrison and once for a defence mark — and, when a
    /// loser was both, really does call it twice and post two letters. See
    /// [`crate::conquest::Capture`].
    pub captures: [Option<crate::conquest::Capture>; 2],
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

/// The count below which a troop line is wiped outright —
/// `Army_WithdrawCasualties`' `if (n < 0xB) n = 0;`.
///
/// Ten men do not retreat in good order; eleven lose five. It is the same shape
/// as [`crate::unit::desert`]'s *"only where the count exceeds ten"* guard, and
/// the two are the only places in the campaign that treat a small troop line
/// differently from a large one.
pub const WITHDRAWAL_WIPE_BELOW: i32 = 11;

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
/// Called first thing in the wage pass,
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
            return_to_campaign(T, &mut counties, &mut realms, &mut units, &mut names, v, 2, false, false, 1, &crate::map::CampaignMap::empty(), &mut crate::explore::Explored::new(), crate::conquest::Restore::NEUTRAL);

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
    /// that wins never takes the county**,
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
            return_to_campaign(T, &mut counties, &mut realms, &mut units, &mut names, v, 2, false, false, 1, &crate::map::CampaignMap::empty(), &mut crate::explore::Explored::new(), crate::conquest::Restore::NEUTRAL);

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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
        );
        assert_eq!(units.get(d).unwrap().moves_used, 0);

        // A winning AI defender pays seven — and seven, not eight.
        let mut units = Units::new();
        let a = army(&mut units, 1, true, &[(TroopType::Peasant, 10)]);
        let d = army(&mut units, 2, false, &[(TroopType::Knight, 100)]);
        return_to_campaign(
            T, &mut counties, &mut realms, &mut units, &mut names,
            Verdict::b_won(a, d), 2, false, false, 1,
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
        );
        assert_eq!(after.county_taken_by, None);
        assert_eq!(counties[2].owner, 2, "beating an army in the field takes no land");
    }

    /// **A repulsed assault does not destroy the besieging army.**
    ///
    /// The rule C31 stopped one branch short of: a loser that is still linked
    /// as a besieger and still has men keeps its men and only loses the siege.
    /// It is unreachable under the autocalc — which zeroes the loser's men —
/// and reachable from a fought battle, so it never showed up.
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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
        );
        assert!(!after.loser_destroyed, "the assault failed; the army did not");
        assert!(after.loser_siege_lifted);
        assert_eq!(units.get(a).unwrap().men, 200, "it keeps every man it walked off with");
        assert_eq!(units.get(a).unwrap().besieging_county, 0, "and loses only the siege");
        assert_eq!(units.get(d).unwrap().besieged_by, 0, "both halves of the link");
        assert_eq!(counties[2].owner, 2, "the castle held");

        // …and the same battle with the besieger wiped out destroys it,
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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
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
                &crate::map::CampaignMap::empty(),
                &mut crate::explore::Explored::new(),
            crate::conquest::Restore::NEUTRAL,
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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
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
            &crate::map::CampaignMap::empty(),
            &mut crate::explore::Explored::new(),
        crate::conquest::Restore::NEUTRAL,
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
    /// bytes; this is the arithmetic on its own,
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

