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
    begin_fight_in(kingdom, attacker, defender, castle_level, seed, false)
}

/// `multiplayer` is `g_multiplayer`, which picks `Battle_Start`'s arm for the
/// two selectors.
pub fn begin_fight_in(
    kingdom: &mut Kingdom,
    attacker: usize,
    defender: usize,
    castle_level: Option<u8>,
    seed: u64,
    multiplayer: bool,
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

    // **The playlist walk, before the field is built.** In
    // `Battlefield_BuildRandom` (`0x0047AAA3`) the read and the bump of
    // `DAT_005653F8` are the first two statements of the non-skirmish branch;
    // a siege takes `Battlefield_BuildCastle` and never reaches them, so this
    // only steps on the open field.
    let frame = match castle_level {
        Some(_) => 0,
        None => kingdom.campaign.field_playlist.take_next(),
    };

    let (a_band, d_band) = (band_of(kingdom, attacker), band_of(kingdom, defender));
    let a = Muster { troops: &a_troops, owner: a_owner, human: a_human, mercenaries: a_band };
    let d = Muster { troops: &d_troops, owner: d_owner, human: d_human, mercenaries: d_band };
    let (lane, rally) = kingdom.campaign.battle_selectors;
    let session = l2_sim::Session { multiplayer, lane: lane as usize, rally: rally as usize };
    let mut runner = match castle_level {
        // **`Battlefield_BuildCastle` (`0x0047C4BA`), from the layout file the
        // player's own install ships.** `crate::castle` holds the two 6,400-byte
        // layers of each castle, because the original's builder reaches for
// them through a global.
        Some(level) => match crate::castle::sheet(level) {
            Some(sheet) => {
                let field = l2_sim::castle::build(level, sheet);
                let tables = l2_sim::castle::tables(sheet);
                BattleRunner::deploy_in_session(field, Some(&tables), seed, a, d, Some(level), session)
            }
            None => {
                let field = l2_sim::siege::our_castle(level);
                BattleRunner::deploy_in_session(field, None, seed, a, d, Some(level), session)
            }
        },
        // **The open field.** `Battle_Start` (`0x004778A0`) calls
        // `Battlefield_BuildRandom` (`0x0047AAA3`) here, which reads one
        // `batfield.pl8` frame as an 80 x 80 terrain plane —
        // `l2_sim::terrain::build_field`. `crate::batfield` is the process
        // global the original reads it out of, and it falls back to
        // [`blank_field`] on a checkout with no install.
        None => {
            let field = crate::batfield::field_frame(frame, seed);
            BattleRunner::deploy_in_session(field, None, seed, a, d, None, session)
        }
    };
    // `Battle_Start` `00470000.c:3059-3072` writes the pair at the start, so an
    // autocalc or a retreat after it still leaves them stepped.
    let s = runner.session();
    kingdom.campaign.battle_selectors = (s.lane as u8, s.rally as u8);

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

/// **The stand-in points again, at the men and not at the cell.** C186's charge
/// is one order, and one order runs out: a closing archer stops at bow range
/// and `movStraff` holds it (`BattleMan_StateCloseToAttack`, `0x00484BF9`), so
/// the unit re-forms on it and the last enemy remnant — cautious, out of its
/// own charge radius — never comes. The opening order stays the enemy's home
/// so that no fixture's first frame moves; this one follows the survivors, as a
/// player's clicks do. `[I]`, C186.
pub(super) fn point_again_for_the_absent_player(
    kingdom: &Kingdom,
    runner: &mut BattleRunner,
    attacker: usize,
    defender: usize,
) {
    let human = |id: usize| kingdom.campaign.units.get(id).is_some_and(|u| u.owner_is_human);
    for (side, id) in [(SIDE_B, attacker), (SIDE_A, defender)] {
        if !human(id) {
            continue;
        }
        // **Nothing in the binary re-orders a human unit.** The whole order
        // table sits inside `Battle_UpdateAllUnits`'s `if (humanControlled ==
        // 0)` (`0x00489401`, `00480000.c:3640-3654`); only the recentre and the
        // 500-frame reform at `:3655-3662` run outside it, and the reform reads
        // the destination this stand-in has to write.
        //
        // At the nearest enemy **unit**, per unit of ours: that is what the
        // handlers behind that guard point at. `UnitOrder_FieldMissile`
        // (`0x0048A9D2`, `:4294`) and `UnitOrder_FieldMelee` (`0x0048B02B`,
        // `:4502`) both take `Enemy_NearestUnit`'s unit; none of them orders at
        // a centre of mass.
        let mine: Vec<usize> =
            runner.units.live().filter(|&u| runner.units.get(u).side == side).collect();
        for u in mine {
            let (mx, my) = (runner.units.get(u).x, runner.units.get(u).y);
            let at = runner
                .units
                .live()
                .filter(|&e| runner.units.get(e).side != side)
                .min_by_key(|&e| {
                    let t = runner.units.get(e);
                    (t.x - mx).abs() as i32 + (t.y - my).abs() as i32
                })
                .map(|e| {
                    let t = runner.units.get(e);
                    (t.x.clamp(0, 79) as u8, t.y.clamp(0, 79) as u8)
                });
            if let Some(at) = at {
                runner.order_unit(u, at.0, at.1);
            }
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
    write_back(kingdom, attacker, runner.survivors(SIDE_B), runner.band_survivors(SIDE_B));
    write_back(kingdom, defender, runner.survivors(SIDE_A), runner.band_survivors(SIDE_A));

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

