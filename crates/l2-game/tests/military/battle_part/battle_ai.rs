#![allow(unused_imports)]
use super::*;
use super::hires_and_sieges::*;
use super::prompts_and_settling::*;
use super::tactical_controls::*;
use super::watched_battles::*;
use super::*;
use super::raising::*;
use super::marching::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

/// **A battle a player watches and gives the headless path's own order in
/// produces exactly the battle nobody watches.**
///
/// This is the assertion that lets orders be added safely: a player's orders
/// change what a player *can do*, and must not change what the same inputs
/// produce. The two paths share [`l2_game::engagement::begin_fight`] and
/// [`l2_game::engagement::conclude_fight`]; what differs is who supplies the
/// ticks,
/// battle is over every hundredth tick and the played one every tick.
///
/// **The order is now the player's and is issued here.** It used to be inside
/// `begin_fight`, so "giving no orders" and "giving the headless order" were the
/// same thing and the test could not tell them apart. `Battle_Start` orders
/// nobody,
/// headless path uses for an absent player has to be issued explicitly on this
/// side for the two to be comparable at all.
#[test]
fn watching_a_battle_and_giving_the_same_order_reproduces_the_headless_verdict() {
    // The headless path: `end_turn`, whose policy answers Decline — so force
    // the fought path by resolving the same pair directly.
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let county = g.kingdom.campaign.units.get(defender).map_or(0, |u| u.county);
    let seed = 0x5EED_BEEF;
    let mut headless = g.kingdom.clone();
    let mut runner = l2_game::engagement::begin_fight(&mut headless, attacker, defender, None, seed)
        .expect("two armies");
    // `engagement::fight` supplies this for an absent player; this loop is
    // `fight`'s body written out, so it has to supply it too.
    {
        let enemy = runner.home(l2_sim::runner::other_side(l2_sim::SIDE_B));
        runner.order_side(l2_sim::SIDE_B, enemy.0, enemy.1);
    }
    while runner.tick < 12_000 && runner.conclusion().is_none() {
        runner.run(100);
    }
    let (want_verdict, _) =
        l2_game::engagement::conclude_fight(&mut headless, attacker, defender, runner);

    // The watched path: the same two armies, the same seed, one tick at a time.
    let mut watched = g.kingdom.clone();
    let mut runner =
        l2_game::engagement::begin_fight(&mut watched, attacker, defender, None, seed)
            .expect("two armies");
    // The click the headless path makes on the player's behalf, made here by
    // hand: every unit of the human side at the enemy's marker.
    {
        let enemy = runner.home(l2_sim::runner::other_side(l2_sim::SIDE_B));
        runner.order_side(l2_sim::SIDE_B, enemy.0, enemy.1);
    }
    let mut live = l2_game::battlefield::LiveBattle::new(runner, attacker, defender, county, None, 1, 1);
    live.paused = false;
    for _ in 0..12_000 {
        live.tick();
        if live.conclusion.is_some() {
            break;
        }
    }
    runner = live.runner;
    let (got_verdict, _) =
        l2_game::engagement::conclude_fight(&mut watched, attacker, defender, runner);

    assert_eq!(got_verdict, want_verdict, "a watched battle changed its own outcome");
    for id in [attacker, defender] {
        assert_eq!(
            headless.campaign.units.get(id).map(|u| u.troops),
            watched.campaign.units.get(id).map(|u| u.troops),
            "unit {id} came off the field with different men",
        );
    }
    let _ = &mut g;
}

/// **A raised battle orders nobody**, because `Battle_Start` (`0x004778A0`)
/// orders nobody.
///
/// The player's report: *"my men in battle started moving before I clicked"*,
/// and *"units don't advance on their own"*. Both are right,
/// the original's own arithmetic:
///
/// * `Battle_Start` runs `Battlefield_Build*`, `Battle_InitArmies`,
///   `FUN_00480F8B`, `Battle_UpdateAllMen` and
/// `Battle_UpdateStrengthAdvantage`.
///   `Battle_RaiseSide` (`0x0047FEA7`) → `BattleUnit_Create` (`0x00480662`)
///   gives each figure `tg x`/`tg y` equal to the cell it stands on.
/// * `Battle_UpdateAllUnits` (`0x00489401`) runs no order handler for a unit
///   whose `humanControlled` byte is set, so nothing writes a destination for
///   the player's units either.
/// * It is not that an unordered unit is inert: `BattleMan_FireMissile`
///   (`0x004804xx`, the state-5 arm calling `Missile_FindTarget`) carries **no**
///   human guard, so an unordered archer stands where it is and shoots whatever
///   comes inside its range. Standing is the behaviour; helplessness is not.
///
/// `Battle_Start` also seeds `DAT_0053F238 = 0xFFFFFFFF`,
/// paused —
/// men moved without him.
#[test]
fn a_raised_battle_gives_the_players_own_units_no_orders() {
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let mut runner =
        l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 0x5EED_BEEF)
            .expect("two armies");

    // The attacker is the human's and `Battle_InitArmies` raises it as army A,
    // side 4. Nothing may have written a destination for any of its units.
    let human: Vec<usize> = (1..=l2_sim::unit::MAX_UNITS)
        .filter(|&u| runner.units.get(u).is_live() && runner.units.get(u).human)
        .collect();
    assert!(!human.is_empty(), "the player's army must raise at least one unit");
    for u in &human {
        let unit = runner.units.get(*u);
        assert_eq!(
            (unit.target_x, unit.target_y),
            (unit.x, unit.y),
            "unit {u} was given a destination before anybody clicked",
        );
    }
    for (i, f) in runner.fighters.iter().enumerate() {
        if runner.sim.figures[f.sim].owner_is_human {
            assert_eq!(
                f.target,
                (f.x, f.y),
                "figure {i} was sent somewhere before anybody clicked",
            );
        }
    }

    // And it holds when the clock runs: two hundred ticks with no input leaves
    // every one of the player's men on the cell it deployed on.
    let before: Vec<(u8, u8)> = runner
        .fighters
        .iter()
        .map(|f| (f.x, f.y))
        .collect();
    runner.run(200);
    for (i, f) in runner.fighters.iter().enumerate() {
        if runner.sim.figures[f.sim].owner_is_human {
            assert_eq!(
                (f.x, f.y),
                before[i],
                "the player's figure {i} walked off on its own",
            );
        }
    }
}

/// The other half of the same claim, from the AI's side: the handler guard
/// selects **AI** units, not the player's.
///
/// `Battle_UpdateAllUnits`: `if (g_battleUnits[cur].humanControlled == 0)` — the
/// handler runs for everyone *except* the human. The player reported *"it might
/// be that enemy ai is getting applied to my units"*; this is the measurement
/// that says which side our dispatch picks.
#[test]
fn the_battle_ai_thinks_for_the_enemys_units_and_not_the_players() {
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let mut runner =
        l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 0x5EED_BEEF)
            .expect("two armies");
    runner.run(400);

    let mut ai_thought = 0;
    for u in 1..=l2_sim::unit::MAX_UNITS {
        let unit = runner.units.get(u);
        if !unit.is_live() {
            continue;
        }
        if unit.human {
            assert_eq!(unit.orders, 0, "a handler ran for the player's unit {u}");
            assert_eq!(unit.think, 0, "the player's unit {u} was given think time");
        } else if unit.orders > 0 {
            ai_thought += 1;
        }
    }
    assert!(ai_thought > 0, "no enemy unit thought at all in four hundred ticks");
}

/// **And an outnumbered AI holds its ground**, which is the other half of what
/// the player saw: *"I didn't see the enemy's units moving."*
///
/// He was right,
/// `Battle_UpdateStrengthAdvantage` (`0x0047FC01`) at roughly −50 against a
/// threshold of 5 (`g_aiAggressionThreshold`, `0x0057C8B4`), so every field
/// handler takes its **cautious** branch. With no attacker in `hit_memory` and
/// no enemy unit inside the charge radius — 8 cells for `UnitOrder_FieldFoot`,
/// 9 for `UnitOrder_FieldMelee`, against forty cells of field — the only
/// movement left in that branch is `Order_ToRallyWaypoint`.
///
/// And a rally waypoint is the side's **own** deployment marker. `[V]`:
/// `Battlefield_BuildRandom` writes all three of a side's waypoints from the
/// tile it just found the marker on — `g_rallyWaypoints[i] = g_foundTileX`,
/// both rally groups, in the terrain-`0x14` and terrain-`0x1E` arms.
/// cautious AI orders itself to stand where it is,
/// to it. That is the game, not a stall.
#[test]
fn an_outnumbered_ai_holds_its_ground_and_does_not_advance() {
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let mut runner =
        l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 0x5EED_BEEF)
            .expect("two armies");
    let ai: Vec<usize> = (1..=l2_sim::unit::MAX_UNITS)
        .filter(|&u| runner.units.get(u).is_live() && !runner.units.get(u).human)
        .collect();
    assert!(!ai.is_empty(), "the defence must raise at least one unit");
    // Its own marker, which is also all three of its rally waypoints.
    let (hx, hy) = runner.home(l2_sim::SIDE_A);
    let (px, py) = runner.home(l2_sim::SIDE_B);

    runner.run(4_000);
    assert!(runner.ai.strength_advantage < 5, "the AI must be the weaker side here");
    for u in ai {
        let unit = runner.units.get(u);
        assert!(unit.orders > 0, "AI unit {u} never thought");
        // It closes up **on its own marker** — the rally waypoint — and goes no
        // further. A unit that had marched on the enemy would be tens of cells
        // down the field, not six.
        assert!(
            (unit.x - i16::from(hx)).abs() <= 6 && (unit.y - i16::from(hy)).abs() <= 6,
            "AI unit {u} left its own marker ({hx}, {hy}) for ({}, {})",
            unit.x,
            unit.y,
        );
    }
    // And the player's end of the field is a long way off,
// marker" is a real claim.
    assert!(
        (i16::from(py) - i16::from(hy)).abs() > 30,
        "the two markers must be far enough apart for this to mean anything",
    );
    let _ = px;
}

