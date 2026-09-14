#![allow(unused_imports)]
use super::*;
use super::drawbridge::*;
use l2_sim::siege;
use l2_sim::terrain::{id, DIM};
use l2_sim::{BattleRunner, Muster, Troop};

/// **A moat cell filled in is one point of approach damage**, and it costs the
/// county work and no materials at all.
///
/// The **four** loads are the finding, and it comes out of
/// two numbers that were never put beside each other: the fill counter lives in
/// the cell's own **terrain** byte, which `Battlefield_BuildCastle` has already
/// written the water id — 11 — into, and `g_moatFillSteps` is 15.
#[test]
fn filling_a_moat_cell_scores_the_approach_and_bills_only_the_digging() {
    let mut r = siege_battle(2, 5);
    let cell = r
        .field
        .cells
        .iter()
        .position(|c| c.surface == siege::SURFACE_WATER)
        .expect("a Norman keep has a moat");
    assert_eq!(r.field.cells[cell].terrain, id::WATER);
    assert_eq!(id::WATER, 11);
    assert_eq!(siege::MOAT_FILL_STEPS, 15, "so four loads, not fifteen");

    let score = siege::fill_moat_cell(&mut r.field, &mut r.siege, cell);
    assert_eq!(r.siege.moat_filled, 1, "DAT_0057A0D8");
    assert_eq!(r.siege.wall_damage, 0, "and not a stick of wood is owed");
    assert_eq!(r.field.cells[cell].surface, siege::SURFACE_FILLED);
    assert_eq!(r.field.cells[cell].flags, 0, "it is walkable now");

    // One per orthogonal neighbour that is castle ground, rampart or breach —
    // so a cell out in open water scores nothing and one against the wall
    // scores up to four, which is what makes the fill work inward.
    let want = siege::orthogonal_neighbours(cell)
        .filter(|&n| {
            matches!(
                r.field.cells[n].surface,
                siege::SURFACE_GROUND | siege::SURFACE_BAILEY | siege::SURFACE_RAMPART_WALK
            )
        })
        .count() as i32;
    assert_eq!(score, want);
}


/// **A figure really shovels**, over the frames the original charges it, and
/// the ditch is water until the last load goes in.
///
/// This is the state-9 handler, and what
/// it drives is the whole chain the moat needed and did not have: a unit
/// ordered at the water, every one of its figures in state 9,
/// `BattleMan_Step`'s state-9 arm latching the cell that stopped them, four
/// loads at 101 frames each, and `FUN_0047DD86` at the end of it.
///
/// **Why only the human side is run.** The `ownerIsHuman` asymmetry is real —
/// `BattleMan_StateFillMoat` picks `100` for a human's man and `0x50` for an
/// AI's, then tests `threshold < counter`, so the two are 101 and 81 frames a
/// load, and four loads is 404 frames against 324 — but it cannot be *measured*
/// by racing two battles: an AI unit is re-ordered by its own handler every 200
/// frames, so the order this test gives is not the order it is still following
/// a load later. The constants are asserted instead, and the integration run is
/// the human one, where the player's order stands until he gives another.
#[test]
fn a_man_ordered_onto_the_moat_shovels_it_full_over_four_loads() {
    // 101 and 81, from the `threshold < counter` test.
    assert_eq!(siege::MOAT_TICKS_PER_LOAD_HUMAN, 100);
    assert_eq!(siege::MOAT_TICKS_PER_LOAD_AI, 0x50);
    let loads = u32::from(siege::MOAT_FILL_STEPS - id::WATER);
    assert_eq!(loads, 4, "the counter starts at the water id, 11, and runs to 15");

    let attacker = [(Troop::Peasants, 200u32)];
    let defender = [(Troop::Archers, 40u32)];
    let mut r = BattleRunner::deploy_siege(
        siege::our_castle(2),
        17,
        Muster { troops: &attacker, owner: 1, human: true },
        Muster { troops: &defender, owner: 2, human: false },
        2,
    );
    let moat = r
        .field
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.surface == siege::SURFACE_WATER)
        .map(|(i, _)| i)
        .max_by_key(|i| i / DIM)
        .expect("a moat");
    let (mx, my) = ((moat % DIM) as u8, (moat / DIM) as u8);
    r.order_side(l2_sim::SIDE_B, mx, my);

    // **Every figure of the unit is in state 9**, not only the ones whose slot
    // happens to be wet — that is `DAT_00553FE4` being the *unit's* destination
    //, and it is what a check on the slot could never
    // produce, because no slot is ever chosen on water.
    let filling = r
        .fighters
        .iter()
        .filter(|f| r.sim.figures[f.sim].state == l2_sim::figure::State::FillingMoat)
        .count();
    assert!(filling > 1, "only {filling} figure(s) went to the ditch");

    let mut ticks = 0;
    while r.siege.moat_filled == 0 && ticks < 6_000 {
        r.step();
        ticks += 1;
    }
    eprintln!("{filling} men at the ditch ({mx},{my}); the first cell was full after {ticks}");
    assert!(r.siege.moat_filled > 0, "the ditch was never filled");
    assert!(
        ticks > loads * u32::from(siege::MOAT_TICKS_PER_LOAD_HUMAN),
        "filled in {ticks} frames, faster than four loads of \
         {} — the shovelling is not being charged",
        siege::MOAT_TICKS_PER_LOAD_HUMAN,
    );
    assert!(
        r.field.cells.iter().any(|c| c.surface == siege::SURFACE_FILLED),
        "a cell of ditch became ground",
    );

    // **And they go and fill the next one.** `FUN_004926FB` retargets a figure
    // that has finished a cell at the nearest water within nineteen, so a unit
    // sent at the ditch digs its way along it; only when there is none left
    // within reach does it drop back to state 5 and become soldiers again.
    let one = r.siege.moat_filled;
    for _ in 0..3_000 {
        r.step();
    }
    assert!(
        r.siege.moat_filled > one,
        "the men stopped at the first cell: {} filled after three thousand more frames",
        r.siege.moat_filled,
    );
}

/// **The two accumulators are separate and are billed differently**, which is
/// the whole of `docs/bugs.md` B69: the wall is repaired in the castle's own
/// material and the ditch is only ever dug out again.
#[test]
fn the_moat_and_the_wall_are_two_accumulators_and_only_one_costs_material() {
    let mut r = siege_battle(3, 9);
    // Nothing has happened yet, so the bookkeeper would do nothing at all —
    // which is exactly the state a siege settled by the autocalc ends in, and
    // why a calculated assault leaves a castle unmarked.
    assert!(!r.castle_damage().any());

    let cells: Vec<usize> = r
        .field
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.surface == siege::SURFACE_WATER)
        .map(|(i, _)| i)
        .take(3)
        .collect();
    for c in cells {
        siege::fill_moat_cell(&mut r.field, &mut r.siege, c);
    }
    let d = r.castle_damage();
    assert_eq!(d.moat_filled, 3);
    assert_eq!(d.wall_damage, 0);
    assert!(d.any(), "and the bookkeeper now has something to bill");
}

/// The round trip: what a siege leaves is what the next assault on the same
/// castle picks up — `FUN_004787A4`, the last statement but one of
/// `Battlefield_BuildCastle`.
/// `Battle_Start` had just written.
#[test]
fn a_castle_carries_its_scars_into_the_next_assault() {
    let mut first = siege_battle(3, 13);
    assert!(first.lower_drawbridge());
    let cell = first
        .field
        .cells
        .iter()
        .position(|c| c.surface == siege::SURFACE_WATER)
        .expect("a moat");
    siege::fill_moat_cell(&mut first.field, &mut first.siege, cell);
    let left = first.castle_damage();
    assert!(left.any() && left.gate_open);

    let mut second = siege_battle(3, 13);
    assert!(!second.siege.gate_breached, "a fresh battle starts with the gate shut");
    second.restore_castle_damage(left);
    assert!(second.siege.gate_breached, "and the last siege's open gate comes back");
    assert_eq!(second.siege.moat_filled, left.moat_filled);
    assert_eq!(second.ai.approach_score, left.approach_score);
}

