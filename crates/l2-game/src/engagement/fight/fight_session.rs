#![allow(unused_imports)]
use super::*;
use super::resolution::*;
use super::battle_runner::*;
use super::*;
use super::report::*;
use super::siege::*;
use super::tests::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// **Raise the battle and stop**, so that somebody else can supply the ticks.
///
/// This is `Battle_Start` (`0x004778A0`) minus the screen: the two musters, the
/// battlefield, and the one order a human side always issues on the first frame
/// because `Battle_UpdateAllUnits` runs no handler for it.
///
/// A battle a player watches and a battle nobody watches begin **here, in the
/// same call, with the same seed**, which is what makes it possible to assert
/// that giving no orders reproduces the headless verdict exactly.
pub fn begin_fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
    seed: u64,
) -> Option<BattleRunner> {
    // `Army_PrepareForBattle` — the four battle-only troop slots, produced here
// because the original zeroes them again the moment the
    // battle is over (`Army_ClearBattleSlots`).
    let (a_extra, d_extra) = match castle_level {
        Some(level) => (
            l2_kingdom::siege::prepare_besieger(kingdom.campaign.units.get(attacker)?),
            l2_kingdom::siege::prepare_garrison(level),
        ),
        None => Default::default(),
    };
    let a_troops = muster_with(kingdom, attacker, a_extra)?;
    let d_troops = muster_with(kingdom, defender, d_extra)?;
    let (a_owner, a_human) = {
        let u = kingdom.campaign.units.get(attacker)?;
        (u.owner, u.owner_is_human)
    };
    let (d_owner, d_human) = {
        let u = kingdom.campaign.units.get(defender)?;
        (u.owner, u.owner_is_human)
    };

    let a = Muster { troops: &a_troops, owner: a_owner, human: a_human };
    let d = Muster { troops: &d_troops, owner: d_owner, human: d_human };
    let mut runner = match castle_level {
        // **`Battlefield_BuildCastle` (`0x0047C4BA`), from the layout file the
        // player's own install ships.** `crate::castle` holds the two 6,400-byte
        // layers of each castle, because the original's builder reaches for
// them through a global.
        //
        // Without the install, and the stand-in ring
        // `l2_sim::siege::our_castle` takes over — a castle whose wall stands
        // one high, which is exactly what boiling oil and a siege tower's dock
        // cannot happen against.
        Some(level) => match crate::castle::sheet(level) {
            Some(sheet) => {
                let field = l2_sim::castle::build(level, sheet);
                let tables = l2_sim::castle::tables(sheet);
                BattleRunner::deploy_siege_on_sheet(field, &tables, seed, a, d, level)
            }
            None => BattleRunner::deploy_siege(
                l2_sim::siege::our_castle(level),
                seed,
                a,
                d,
                level,
            ),
        },
        None => BattleRunner::deploy_muster(blank_field(), seed, a, d),
    };

    // **What the last siege on this castle left.** `FUN_004787A4` is the last
    // statement but one of `Battlefield_BuildCastle`, so it runs *after* the
// fresh scores and overwrites them — so this sits below
// `deploy_siege`.
    if castle_level.is_some() {
        let besieged = kingdom.campaign.units.get(attacker)?.besieging_county;
        if let Some(c) = kingdom.counties.get_mut(besieged as usize) {
            let scars = l2_kingdom::siege::scars_for_assault(c);
            runner.restore_castle_damage(from_scars(scars));
        }
    }

    Some(runner)
}

/// **Nobody is at the keyboard**, so somebody has to be the player.
///
/// This exists only for [`fight`], the headless path, and it has no counterpart
/// in the original: every `Answer::TakeTheField` there is a person on screen
/// `0x29`. `Battle_UpdateAllUnits` runs no handler for a human unit, so a
/// headless battle with a human side would stand where it deployed until the
/// other side walked into it and then be cut down without ever having advanced
/// — which is a stand-in for an absent player, not a battle.
///
/// It is deliberately **not** in [`begin_fight`]. `Battle_Start` (`0x004778A0`)
/// issues no order to anybody — it builds the field, calls `Battle_InitArmies`,
/// `FUN_00480F8B`, `Battle_UpdateAllMen` and `Battle_UpdateStrengthAdvantage`,
/// and stops — and a player who watches a battle must get that and not this.
/// Putting the stand-in here is what lets a watched battle open the way the
/// original's does while a headless one still resolves. `docs/decisions.md`
/// C186.
fn charge_for_the_absent_player(
    kingdom: &Kingdom,
    runner: &mut BattleRunner,
    attacker: usize,
    defender: usize,
) {
    let human = |id: usize| kingdom.campaign.units.get(id).is_some_and(|u| u.owner_is_human);
    for (side, id) in [(SIDE_B, attacker), (SIDE_A, defender)] {
        if human(id) {
            let enemy = runner.home(l2_sim::runner::other_side(side));
            runner.order_side(side, enemy.0, enemy.1);
        }
    }
}

/// **The tail of a fought battle**: the write-back and the verdict.
///
/// `FUN_0047F474` rebuilds both campaign records from the figures still
/// standing, and the winner is the side the conclusion names. The stall arm is
/// ours — the original has no clock — and it is a separate [`Resolution`]
/// variant so that nobody can mistake an invented winner for a won battle.
pub fn conclude_fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    runner: BattleRunner,
) -> (Verdict, Resolution) {
    let conclusion = runner.conclusion();

    // `FUN_0047F474` — the write-back. Both sides, all eleven slots, rebuilt
    // from what is still standing.
    write_back(kingdom, attacker, runner.survivors(SIDE_B));
    write_back(kingdom, defender, runner.survivors(SIDE_A));

    // Army A is side 4 and army B is side 0 — `Battle_InitArmies`.
    let (winner_side, resolution) = match conclusion {
        Some(c) => (c.winner, Resolution::Fought { ticks: runner.tick, cause: c.cause }),
        // Our stall, not the original's. The larger force holds the field; the
// variant says the number was invented.
        None => {
            let side = if runner.men_of_side(SIDE_B) > runner.men_of_side(SIDE_A) {
                SIDE_B
            } else {
                SIDE_A
            };
            (side, Resolution::Stalled { ticks: runner.tick })
        }
    };
    let verdict = if winner_side == SIDE_B {
        Verdict::a_won(attacker, defender)
    } else {
        Verdict::b_won(attacker, defender)
    };
    (verdict, resolution)
}

