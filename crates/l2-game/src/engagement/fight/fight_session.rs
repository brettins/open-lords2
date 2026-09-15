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

/// This is `Battle_Start` (`0x004778A0`) minus the screen: the two musters, the
/// battlefield, and the one order a human side always issues on the first frame
/// because `Battle_UpdateAllUnits` runs no handler for it.
pub fn begin_fight(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
    seed: u64,
) -> Option<BattleRunner> {
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
        // **The open field.** `Battle_Start` (`0x004778A0`) calls
        // `Battlefield_BuildRandom` (`0x0047AAA3`) here, which reads one
        // `batfield.pl8` frame as an 80 x 80 terrain plane —
        // `l2_sim::terrain::build_field`. `crate::batfield` is the process
        // global the original reads it out of, and it falls back to
        // [`blank_field`] on a checkout with no install.
        None => BattleRunner::deploy_muster(crate::batfield::field(seed), seed, a, d),
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

/// It is deliberately **not** in [`begin_fight`]. `Battle_Start` (`0x004778A0`)
/// issues no order to anybody — it builds the field, calls `Battle_InitArmies`,
/// `FUN_00480F8B`, `Battle_UpdateAllMen` and `Battle_UpdateStrengthAdvantage`,
/// and stops — and a player who watches a battle must get that and not this.
///
/// Putting the stand-in here is what lets a watched battle open the way the
/// original's does while a headless one still resolves. `docs/decisions.md`
/// C186.
pub(super) fn charge_for_the_absent_player(
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

    let (winner_side, resolution) = match conclusion {
        Some(c) => (c.winner, Resolution::Fought { ticks: runner.tick, cause: c.cause }),
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

