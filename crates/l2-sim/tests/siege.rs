//! **A siege that actually runs**, and the fourteen order handlers it reaches.
//!
//! `docs/battle-ai.md` has held all seventeen handlers since it was written and
//! `crates/l2-sim/src/ai.rs` has implemented all eighteen slots. **Fourteen of
//! them had never been dispatched even once**, because nothing in this crate
//! could produce a siege: no castle, no `is_siege`, and no way for a figure to
//! attack a wall.
//!
//! Needs no game install. The castle is
//! [`l2_sim::siege::our_castle`] — **ours, not the original's**, and its own
//! documentation says why.

use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

/// A besieger with everything, against a garrison with everything, at a given
/// castle level. Both sides AI-controlled so that every unit is dispatched.
fn siege_battle(level: u8, seed: u64) -> BattleRunner {
    let attacker = [
        (Troop::Peasants, 240u32),
        (Troop::Archers, 80),
        (Troop::Swordsmen, 80),
        (Troop::Knights, 40),
        (Troop::Catapults, 2),
        (Troop::SiegeTowers, 2),
        (Troop::BatteringRams, 1),
    ];
    // Three missile groups, because the first two are latched to the two wall
    // categories and the third is what reaches `UnitOrder_SiegeDefMissile`.
    let defender = [
        (Troop::Archers, 500u32),
        (Troop::Crossbowmen, 200),
        (Troop::Swordsmen, 40),
        (Troop::Pikemen, 40),
        (Troop::Knights, 20),
        (Troop::Oil, 3),
    ];
    BattleRunner::deploy_siege(
        siege::our_castle(level),
        seed,
        Muster { troops: &attacker, owner: 1, human: false },
        Muster { troops: &defender, owner: 2, human: false },
        level,
    )
}

/// **The headline: how many of the seventeen a running siege reaches.**
///
/// A field battle can reach three. Sieges are the other fourteen, and this
/// enumerates them by running one and recording which slot every live unit
/// dispatched to.
#[test]
fn a_running_siege_reaches_the_fourteen_handlers_a_field_battle_cannot() {
    let mut seen: Vec<&'static str> = Vec::new();
    for level in 0..=4u8 {
        let mut r = siege_battle(level, 0x51_E6E_u64.wrapping_mul(level as u64 + 1));
        r.run(600);
        for u in 1..=l2_sim::MAX_UNITS {
            let unit = r.units.get(u);
            if !unit.is_live() {
                continue;
            }
            if let Some(slot) = handler_for(r.ai.is_siege, unit.side, unit.category) {
                if slot.name != "UnitOrder_None" && !seen.contains(&slot.name) {
                    seen.push(slot.name);
                }
            }
        }
    }
    seen.sort_unstable();

    // The three a field battle already had are *not* in this list: the siege
    // tables hold entirely different functions at every category.
    for field in TABLE_FIELD.iter() {
        assert!(
            !seen.contains(&field.name) || field.name == "UnitOrder_None",
            "{} is a field handler and should not appear in a siege",
            field.name
        );
    }

    let all_siege: Vec<&'static str> = {
        let mut v: Vec<&'static str> = TABLE_SIEGE_ATT
            .iter()
            .chain(TABLE_SIEGE_DEF.iter())
            .map(|s| s.name)
            .filter(|n| *n != "UnitOrder_None")
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    assert_eq!(all_siege.len(), 14, "fourteen of the seventeen are siege-only");
    assert_eq!(seen, all_siege, "and a running siege reaches every one of them");
}

/// The two dispatch categories that exist **only** in a siege, and only for the
/// garrison — the one-shot latches in `BattleUnit_Create`.
#[test]
fn the_garrisons_first_two_missile_units_take_the_two_wall_categories() {
    let r = siege_battle(4, 7);
    let mut wall: Vec<u8> = (1..=l2_sim::MAX_UNITS)
        .map(|u| r.units.get(u))
        .filter(|u| u.is_live() && u.side == SIDE_A && u.category >= 9)
        .map(|u| u.category)
        .collect();
    wall.sort_unstable();
    assert_eq!(wall, vec![9, 10], "exactly one of each, and only for the defender");

    // The besieger's missile units keep category 1 whatever it raises.
    assert!(
        (1..=l2_sim::MAX_UNITS)
            .map(|u| r.units.get(u))
            .filter(|u| u.is_live() && u.side == SIDE_B)
            .all(|u| u.category < 9),
        "the latches are the defender's"
    );

    // And in a field battle neither latch fires at all.
    let field = BattleRunner::deploy_muster(
        l2_sim::runner::blank_field(),
        7,
        Muster { troops: &[(Troop::Archers, 100u32)], owner: 1, human: false },
        Muster { troops: &[(Troop::Archers, 100u32)], owner: 2, human: false },
    );
    assert!((1..=l2_sim::MAX_UNITS)
        .map(|u| field.units.get(u))
        .filter(|u| u.is_live())
        .all(|u| u.category < 9));
}

/// **A wall comes down**, and a ram is twenty times faster at it than a man.
#[test]
fn a_battering_ram_opens_the_gate_and_the_breach_reaches_the_order_layer() {
    let mut s = SiegeState::castle(4);
    let mut frames = 0;
    while !s.gate_breached {
        siege::strike_wall(&mut s, siege::SURFACE_GROUND, true);
        frames += 1;
    }
    assert_eq!(frames, 1_000, "one ram, one thousand frames");

    // And in a running battle. The besieger is a player's here — an AI unit
    // marches to the approach points its script names, and the player's is the
    // side that actually walks its men into a wall.
    let field = siege::our_castle(1);
    let wall_cells = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    assert!(wall_cells > 0);
    let gate = field
        .cells
        .iter()
        .position(|c| c.flags & FLAG_KEEP != 0)
        .expect("a way in");
    let (gx, gy) = ((gate % 80) as u8, (gate / 80) as u8);
    let mut r = BattleRunner::deploy_siege(
        field,
        11,
        Muster {
            troops: &[(Troop::BatteringRams, 2u32), (Troop::Peasants, 200)],
            owner: 1,
            human: true,
        },
        Muster { troops: &[(Troop::Archers, 40u32)], owner: 2, human: false },
        1,
    );
    r.order_side(SIDE_B, gx, gy);
    for _ in 0..80 {
        r.run(200);
        r.order_side(SIDE_B, gx, gy);
        if r.siege.gate_hits > 0 || r.siege.ramparts_breached > 0 {
            break;
        }
    }
    assert!(
        r.siege.gate_hits > 0 || r.siege.ramparts_breached > 0,
        "the besiegers reached the wall and hit it"
    );
}

/// **The way in ends the siege without a man of the garrison being killed.**
///
/// `DAT_00553F3C`, which `Battle_CheckOutcome` tests before either men counter
/// and which no document had before today.
#[test]
fn reaching_the_keep_wins_the_siege_outright() {
    let mut r = siege_battle(0, 3);
    assert!(r.conclusion().is_none());
    let garrison_before = r.men_of_side(SIDE_A);
    r.siege.broke_in = true;
    let c = r.conclusion().expect("a siege ends when somebody gets in");
    assert_eq!(c.winner, SIDE_B, "the besieger");
    assert_eq!(c.cause, End::BrokeIn);
    assert_eq!(r.men_of_side(SIDE_A), garrison_before, "and the garrison is untouched");

    // The cell is real, not a flag somebody has to set by hand.
    assert_eq!(
        siege::our_castle(0).cells.iter().filter(|c| c.flags & FLAG_KEEP != 0).count(),
        1
    );
}

/// **Assault repulsed, repeat** — the one arm of the outcome test that changes
/// something rather than ending the battle, and the level that separates it
/// from *the besieger loses*.
#[test]
fn a_small_castle_repeats_a_failed_assault_and_a_large_one_ends_it() {
    // A besieger with no engines at all is the position both arms test, and
    // the engine count is recounted from the figures every frame — so this is
    // reached by bringing none rather than by setting a counter.
    let bare = |level: u8| {
        BattleRunner::deploy_siege(
            siege::our_castle(level),
            5,
            Muster { troops: &[(Troop::Peasants, 200u32)], owner: 1, human: false },
            Muster { troops: &[(Troop::Archers, 60u32)], owner: 2, human: false },
            level,
        )
    };

    // Level 2: the scores reset and the fight goes on.
    let mut small = bare(2);
    small.run(1);
    assert_eq!(small.ai.siege_engine_count, 0, "it brought none");
    assert!(small.conclusion().is_none(), "below level 3 the assault repeats");
    assert_eq!(small.ai.breach_score, ASSAULT_REPEAT_SCORE);
    assert_eq!(small.ai.approach_score, ASSAULT_REPEAT_SCORE);

    // Level 3: the same position is a defeat, and the *campaign* gate refuses
    // to start it for the same reason and at the same level.
    let mut large = bare(3);
    large.run(1);
    let c = large.conclusion().expect("at level 3 there is no second chance");
    assert_eq!(c.winner, SIDE_A, "the garrison");
    assert_eq!(c.cause, End::AssaultFailed);
    assert_eq!(ASSAULT_REPEATS_BELOW_LEVEL, 3);
}

/// A siege still ends the two ways a field battle does, and the men counters
/// still ignore the engines.
#[test]
fn a_siege_still_ends_on_annihilation_and_engines_are_worth_no_men() {
    let mut r = siege_battle(0, 9);
    let before = r.men_of_side(SIDE_B);
    r.run(1);
    // Two catapults, two towers and a ram, and not one of them counts.
    let engines: u32 = r
        .fighters
        .iter()
        .filter(|f| f.troop.index() >= 7 && r.is_alive(0))
        .map(|f| f.troop.index() as u32)
        .sum();
    assert!(engines > 0, "the besieger brought engines");
    assert_eq!(r.men_of_side(SIDE_B), before, "and they are worth no men");
}

/// Determinism, which the whole crate rests on: a siege is a simulation like
/// any other and two runs of it must be bit-identical.
#[test]
fn two_runs_of_the_same_siege_stay_identical() {
    let (mut a, mut b) = (siege_battle(4, 42), siege_battle(4, 42));
    for _ in 0..20 {
        a.run(100);
        b.run(100);
        assert_eq!(a.siege, b.siege, "diverged at tick {}", a.tick);
        assert_eq!(a.sim.figures, b.sim.figures, "diverged at tick {}", a.tick);
    }
}

/// **The four surfaces a siege turns on, and the two that were the wrong way
/// round.**
///
/// This test used to assert `SURFACE_BREACH == 4` and `SURFACE_RAMPART == 5`
/// and it was wrong about the first: nothing in `Lords2.exe` writes surface 4
/// outside the castle build's flood classifier, and a breach leaves **5**
/// (`Wall_Smash`) or **9** (`Wall_Collapse`). `docs/decisions.md`
/// `CNEW-surfaces`.
#[test]
fn a_smashed_wall_joins_the_bailey_and_a_collapsed_one_does_not() {
    assert_eq!(SURFACE_WALL, 8, "an intact wall, the cell that carries 0x20");
    assert_eq!(SURFACE_BAILEY, 5, "the bailey — and what Wall_Smash leaves");
    assert_eq!(SURFACE_RAMPART_WALK, 4, "what Siege_FindCellSurface4 hunts");
    assert_eq!(siege::SURFACE_COLLAPSED, 9, "what Wall_Collapse leaves");

    // The accumulator is chosen by where the attacker *stands*, and 5 is the
    // one that picks the rampart's 5,000.
    let mut s = SiegeState::castle(2);
    for _ in 0..siege::RAMPART_HITS - 1 {
        siege::strike_wall(&mut s, SURFACE_BAILEY, false);
    }
    assert_eq!(
        siege::strike_wall(&mut s, SURFACE_BAILEY, false),
        siege::WallBlow::RampartBreached
    );
    // Standing on the rampart walk is *not* standing on a 5, so it feeds the
    // gate — which is the whole content of the correction.
    let mut g = SiegeState::castle(2);
    siege::strike_wall(&mut g, SURFACE_RAMPART_WALK, false);
    assert_eq!(g.gate_hits, 1);
    assert_eq!(g.rampart_hits, 0);
}

/// **`Wall_Smash` opens a nine-cell-wide hole, and `Wall_Collapse` opens one
/// cell and leaves a different mark.** They are two routines, not one.
#[test]
fn a_breach_is_nine_cells_wide_and_a_collapse_is_one() {
    let mut field = siege::our_castle(2);
    let wall_before = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();

    // The south wall of `our_castle(2)` runs along y = 34 through x = 40.
    let opened = siege::smash_walls(&mut field, 40, 34, siege::SMASH_RADIUS);
    assert_eq!(opened, 9, "a 9 x 9 centred on one wall cell meets nine of them");
    let wall_after = field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    assert_eq!(wall_before - wall_after, 9);
    for x in 36..=44u32 {
        let c = &field.cells[34 * 80 + x as usize];
        assert_eq!(c.flags & FLAG_WALL, 0, "({x}, 34) is open");
        assert_eq!(c.surface, SURFACE_BAILEY, "and joins the bailey");
        assert_eq!(
            c.elevation,
            siege::WALL_ELEVATION,
            "and keeps the wall's own height — Wall_Smash does not flatten"
        );
    }

    // A collapse is one cell, at ground level, on surface 9, and it bills the
    // orthogonal neighbours still at 5.
    let mut field = siege::our_castle(2);
    let mut s = SiegeState::castle(2);
    let cell = 34 * 80 + 40;
    let billed = siege::collapse_wall(&mut field, &mut s, cell);
    assert_eq!(field.cells[cell].surface, siege::SURFACE_COLLAPSED);
    assert_eq!(field.cells[cell].elevation, siege::BREACH_ELEVATION);
    assert_eq!(field.cells[cell].flags & FLAG_WALL, 0);
    assert_eq!(field.cells[cell - 80].flags & FLAG_WALL, 0, "north of it is the bailey");
    assert_eq!(billed, 1, "one neighbour — the bailey cell inside it");
    assert_eq!(s.wall_damage, 1, "and the bill is the same count");
    assert_eq!(
        field.cells[cell + 1].flags & FLAG_WALL,
        FLAG_WALL,
        "the wall either side of it still stands"
    );
}

// ---------------------------------------------------------------------------
// The headline: a besieger who can win
// ---------------------------------------------------------------------------

/// **848 men against a garrison of two figures, at every castle level.**
///
/// This is the exact position the branch started from, and it is written as a
/// *long* battle on purpose. The four defects it found were all invisible to a
/// 600-frame test — the whole suite's previous longest siege — because every
/// one of them is an order nobody is ever given rather than an order that goes
/// wrong:
///
/// | | |
/// |---|---|
/// | a breach was **one cell wide** | `Wall_Smash` (`FUN_0049694F`) opens a 9 × 9 |
/// | the approach score started at **0** | `Battlefield_BuildCastle` writes 500, and 0 only when there is a ditch |
/// | the wall stood at **elevation 2** | the builder's structure code 8 writes 1, and a step of 2 cannot be climbed |
/// | the rampart walk sat **against the wall** | `Wall_Collapse` scores only bailey neighbours, so a catapult earned nothing |
///
/// Before them the besieger stood in the field for 400,000 frames. After them
/// every level ends, and the *cause* differs by level, which is the sign that
/// the three different ways in are all live.
#[test]
fn a_besieger_with_eight_hundred_men_takes_a_castle_held_by_two() {
    for level in 0..=4u8 {
        let mut r = storming_party(level);
        assert_eq!(r.men_of_side(SIDE_B), 848, "the besieger, level {level}");
        assert!(r.living(SIDE_A) <= 2, "a garrison of at most two figures");

        let mut ticks = 0;
        // `g_attackersOnWall` — side-4 figures standing on surface 5, which
        // after `Wall_Smash` is the breach and the bailey behind it. It going
        // positive *is* "the assault was pressed home", and it is recounted
        // from the figures' own cells every frame rather than set by anything
        // this test can reach.
        let mut got_inside = 0;
        let end = loop {
            r.run(500);
            ticks += 500;
            got_inside = got_inside.max(r.ai.attackers_on_wall);
            if let Some(c) = r.conclusion() {
                break c;
            }
            assert!(
                ticks < 200_000,
                "level {level}: 848 men still outside after {ticks} frames — \
                 approach {} breach {} gate {} ramparts {} engines {} inside {got_inside}",
                r.ai.approach_score,
                r.ai.breach_score,
                r.siege.gate_breached,
                r.siege.ramparts_breached,
                r.ai.siege_engine_count,
            );
        };
        assert_eq!(end.winner, SIDE_B, "level {level}: the besieger takes it");
        assert!(
            got_inside > 0 || end.cause == End::BrokeIn,
            "level {level}: the besieger won without a man ever getting in — \
             {:?} after {ticks} frames",
            end.cause
        );
        println!(
            "level {level}: {:?} at {ticks} frames, inside {got_inside}, \
             gate {} ramparts {} breach {} wall damage {} moat {}",
            end.cause,
            r.siege.gate_breached,
            r.siege.ramparts_breached,
            r.ai.breach_score,
            r.siege.wall_damage,
            r.siege.moat_filled,
        );
    }
}

/// And the other direction, because a besieger that always wins is not a siege
/// either: **the same 848 men lose to a garrison that outnumbers them.**
///
/// The pair is the real assertion. One of them alone passes for a model that
/// has stopped simulating.
#[test]
fn the_same_besieger_is_thrown_off_a_castle_held_in_strength() {
    let mut r = BattleRunner::deploy_siege(
        siege::our_castle(4),
        99,
        Muster { troops: STORMING_PARTY, owner: 1, human: false },
        Muster { troops: &[(Troop::Archers, 2_000u32)], owner: 2, human: false },
        4,
    );
    let mut ticks = 0;
    let end = loop {
        r.run(500);
        ticks += 500;
        if let Some(c) = r.conclusion() {
            break c;
        }
        assert!(ticks < 200_000, "neither side could finish it");
    };
    assert_eq!(end.winner, SIDE_A, "the garrison holds");
}

/// The besieger of the tests above: 848 men and four engines, and **not one
/// bowman**.
///
/// That is deliberate and it is the second thing this file learned tonight.
/// The first draft gave the besieger 148 archers, and at three of the five
/// castle levels it won with `breach_score` 0, `wall_damage` 0 and the wall
/// untouched — 148 archers simply shot two garrison figures off the rampart
/// from the open field, and the test that was supposed to prove an assault
/// proved a shooting match. `docs/agents.md`'s *"a check that passes for an
/// accidental reason looks exactly like one that passes"*: it went green for a
/// reason unrelated to anything the branch changed, and would have gone on
/// doing so with the whole assault path deleted.
///
/// With no missile troop at all the only ways to win are the three
/// `Battle_CheckOutcome` actually offers a besieger, and every one of them
/// requires getting through the wall.
const STORMING_PARTY: &[(Troop, u32)] = &[
    (Troop::Peasants, 448),
    (Troop::Swordsmen, 300),
    (Troop::Knights, 100),
    (Troop::Catapults, 2),
    (Troop::BatteringRams, 2),
];

fn storming_party(level: u8) -> BattleRunner {
    BattleRunner::deploy_siege(
        siege::our_castle(level),
        99,
        Muster { troops: STORMING_PARTY, owner: 1, human: false },
        Muster { troops: &[(Troop::Archers, 8u32)], owner: 2, human: false },
        level,
    )
}

/// **The approach score's two starting values, and the moat is what chooses.**
///
/// `Battlefield_BuildCastle` writes 500; the raster's moat byte `0xEE` hands
/// each ditch cell to `FUN_0047DCCE`, whose first statement puts it back to 0.
/// So a dry castle opens with the approach already *done* and a moated one
/// opens with everything to do — which is exactly the shape of
/// `Order_ToBreachOrStaging`'s three arms.
#[test]
fn a_dry_castle_opens_at_five_hundred_and_a_moated_one_at_zero() {
    for level in 0..=4u8 {
        let r = storming_party(level);
        let moated = r.field.cells.iter().any(|c| c.surface == siege::SURFACE_WATER);
        assert_eq!(moated, level >= 2, "level {level}");
        assert_eq!(
            r.ai.approach_score,
            if moated { 0 } else { siege::APPROACH_SCORE_START },
            "level {level}"
        );
    }
}

/// **A player's assault: a ram opens the gate, and the Charge button carries
/// it.**
///
/// The other half of the headline, and the one that actually exercises
/// `Wall_Smash`. The AI besieger above wins through cells a *catapult*
/// collapsed one at a time; this one drives the 20,000-hit gate accumulator
/// with rams, which is the path that opens a nine-cell hole and leaves it at
/// the wall's own height.
///
/// Everything the player does here is a value: [`BattleRunner::order_side`] and
/// [`BattleRunner::charge_all`], the same two calls the battlefield screen
/// makes. Nothing sets a flag by hand.
#[test]
fn a_player_rams_the_gate_open_and_charges_through_the_breach() {
    let level = 1; // dry, so the men reach the wall without shovelling
    let field = siege::our_castle(level);
    let keep = field.cells.iter().position(|c| c.flags & FLAG_KEEP != 0).expect("a way in");
    let (kx, ky) = ((keep % 80) as u8, (keep / 80) as u8);
    let mut r = BattleRunner::deploy_siege(
        field,
        21,
        Muster {
            troops: &[(Troop::BatteringRams, 2u32), (Troop::Swordsmen, 600)],
            owner: 1,
            human: true,
        },
        Muster { troops: &[(Troop::Archers, 40u32)], owner: 2, human: false },
        level,
    );

    let wall_before = r.field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    let mut ticks = 0;
    let mut inside = 0;
    let end = loop {
        r.order_side(SIDE_B, kx, ky);
        r.charge_all(1);
        r.run(500);
        ticks += 500;
        inside = inside.max(r.ai.attackers_on_wall);
        if let Some(c) = r.conclusion() {
            break c;
        }
        assert!(
            ticks < 60_000,
            "the gate is {} after {ticks} frames, {} men inside, {} wall cells left",
            if r.siege.gate_breached { "open" } else { "shut" },
            inside,
            r.field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count(),
        );
    };

    assert!(r.siege.gate_breached, "two rams open a gate");
    let opened = wall_before - r.field.cells.iter().filter(|c| c.flags & FLAG_WALL != 0).count();
    assert!(
        opened >= 5,
        "the gate breach opens a 9 x 9 of wall, not one cell — {opened} came down"
    );
    assert!(inside > 0, "and the charge carried men through it");
    assert_eq!(end.winner, SIDE_B);
}
