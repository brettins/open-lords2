//! **The garrison's drawbridge, and the two accumulators that bill the
//! repair.**
//!
//! ```text
//! cargo test -p l2-sim --test drawbridge
//! ```
//!
//! Three things the crate could not do before, all of them reachable from a
//! battle a player is watching:
//!
//! * `FUN_00496B9F` — **lower the drawbridge**. The battlefield's third button
//!   called a once-per-battle latch and nothing else; this is the routine.
//! * `FUN_0047DD86` — **fill a moat cell in**, which is the only writer of
//!   `DAT_0057A0D8` in the whole binary.
//! * the pair `DAT_0057A0D8` / `DAT_0056D648`, which is everything
//!   `Siege_RecordCastleDamage` (`0x004784CA`) bills a castle's repair from.
//!
//! Needs no game install. The castle is [`l2_sim::siege::our_castle`] — **ours,
//! not the original's**, and its own documentation says why. What is *not* ours
//! is every number asserted below: the 7 × 4 patch, the twenty-eight frames,
//! the four score arms and the two accumulators are all read out of
//! `Lords2.exe`.

use l2_sim::siege;
use l2_sim::terrain::{id, DIM};
use l2_sim::{BattleRunner, Muster, Troop};

/// A besieger against a garrison at a given castle level, both sides driven by
/// the AI so that nothing here depends on an order being issued.
fn siege_battle(level: u8, seed: u64) -> BattleRunner {
    let attacker = [
        (Troop::Peasants, 240u32),
        (Troop::Swordsmen, 80),
        (Troop::Catapults, 2),
        (Troop::BatteringRams, 1),
    ];
    let defender = [(Troop::Archers, 300u32), (Troop::Swordsmen, 40), (Troop::Oil, 3)];
    BattleRunner::deploy_siege(
        siege::our_castle(level),
        seed,
        Muster { troops: &attacker, owner: 1, human: false },
        Muster { troops: &defender, owner: 2, human: false },
        level,
    )
}

// ---------------------------------------------------------------------------
// The drawbridge — `FUN_00496B9F`
// ---------------------------------------------------------------------------

/// **The garrison lowers its own drawbridge**, and what that costs it.
///
/// `FUN_00496B9F` sets exactly the globals the twenty-thousandth ram hit sets:
/// `_DAT_00569588`, and four on each of the two progress scores. So opening
/// your own gate to sally hands the besieger's AI the same signal it would have
/// got from breaking the gate down itself — which is the mechanical reason the
/// shipped `Readme.txt` says *"drawbridges can not be closed once they have
/// been opened."*
#[test]
fn lowering_the_drawbridge_opens_a_way_through_and_tells_the_besieger_so() {
    let mut r = siege_battle(3, 7);
    assert!(r.has_drawbridge(), "a stone castle has one");
    assert!(!r.siege.gate_breached, "and it is shut to begin with");
    let (breach, approach) = (r.ai.breach_score, r.ai.approach_score);

    let bridge_cells =
        r.field.cells.iter().filter(|c| c.flags & siege::FLAG_DRAWBRIDGE != 0).count();
    assert!(bridge_cells > 0);

    assert!(r.lower_drawbridge(), "the bridge comes down");

    assert!(r.siege.drawbridge_down, "DAT_0052AF9C, the latch");
    assert!(r.siege.gate_breached, "_DAT_00569588, the gate is open");
    assert_eq!(r.ai.breach_score - breach, siege::GATE_BREACH_SCORE);
    assert_eq!(r.ai.approach_score - approach, siege::GATE_BREACH_SCORE);
    assert_eq!(
        r.field.cells.iter().filter(|c| c.flags & siege::FLAG_DRAWBRIDGE != 0).count(),
        0,
        "every cell the patch covered had its flags cleared",
    );

    // Once, and once only.
    assert!(!r.lower_drawbridge(), "a drawbridge cannot be raised again");
}

/// **The patch is 7 × 4 and the frames are the ones in the executable**, laid
/// down in the patch's own row-major order.
///
/// A table exercised at one input is `docs/decisions.md` C26's shape, so this
/// reads all twenty-eight cells back off the field. The **shape** is what makes
/// the *reading* checkable rather than merely the copying: fifteen cells carry
/// the filler frame 197 and the other thirteen trace a diagonal in frames
/// 192…198 into the corner nearest the gate. Twenty-eight `i32`s misread as
/// bytes, or bytes misread as `i32`s, would have no shape at all — they would
/// be uniform or they would be noise.
#[test]
fn the_drawbridge_patch_is_seven_by_four_and_drawn_with_the_executables_frames() {
    let mut r = siege_battle(4, 11);
    let anchor = r
        .field
        .cells
        .iter()
        .position(|c| c.flags & siege::FLAG_DRAWBRIDGE != 0)
        .expect("a royal castle has a drawbridge");
    assert!(r.lower_drawbridge());

    let (ax, ay) = (anchor % DIM, anchor / DIM);
    for row in 0..siege::DRAWBRIDGE_ROWS {
        for col in 0..siege::DRAWBRIDGE_COLS {
            let cell = &r.field.cells[(ay + row) * DIM + ax + col];
            assert_eq!(
                cell.gfx,
                siege::DRAWBRIDGE_FRAMES[row * siege::DRAWBRIDGE_COLS + col],
                "frame at row {row} column {col}",
            );
            assert_eq!(cell.surface, siege::SURFACE_GROUND, "row {row} column {col}");
            assert_eq!(cell.flags, 0, "row {row} column {col}");
        }
    }
    assert_eq!((siege::DRAWBRIDGE_ROWS, siege::DRAWBRIDGE_COLS), (7, 4));
    let filler = siege::DRAWBRIDGE_FRAMES.iter().filter(|&&f| f == 197).count();
    assert_eq!(filler, 15, "the filler frame");
    assert_eq!(
        siege::DRAWBRIDGE_FRAMES.iter().filter(|&&f| f != 197).count(),
        13,
        "and thirteen cells that are the bridge itself",
    );
    assert!(
        siege::DRAWBRIDGE_FRAMES.iter().all(|&f| (192..=198).contains(&f)),
        "every frame is in the castle tileset's 192…198 run",
    );
}

/// **A palisade has nothing to lower**, and the button is not spent trying.
///
/// The original's latch is set *inside* the search's `if`, so a castle with no
/// `0x40` cell leaves `DAT_0052AF9C` clear. It matters because the level-3
/// guard and the search are two separate tests — the first is the button's and
/// the second is the routine's — and only the first is what the shipped
/// `Readme.txt` describes.
#[test]
fn a_castle_with_no_drawbridge_does_not_spend_the_latch() {
    for level in 0..3u8 {
        let mut r = siege_battle(level, 3);
        assert!(!r.has_drawbridge(), "level {level} has no drawbridge");
        assert!(!r.lower_drawbridge(), "level {level}");
        assert!(!r.siege.drawbridge_down, "level {level}: the latch is untouched");
        assert!(!r.siege.gate_breached, "level {level}: and the gate is still shut");
    }
}

/// The bridge is a **way through**, not just a picture.
///
/// `Cell_TryEnter` refuses a `0x40` cell to **both** sides — it is the one
/// castle flag that is not a side test — so a raised drawbridge is a shut gate
/// to the garrison as well. Lowering it leaves plain ground behind, and
/// `strike_castle` then has nothing to catch, which is what lets a sallying
/// garrison walk out.
#[test]
fn the_lowered_bridge_is_a_way_through_where_the_raised_one_was_a_wall() {
    let mut r = siege_battle(3, 21);
    let bridge: Vec<usize> = r
        .field
        .cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.flags & siege::FLAG_DRAWBRIDGE != 0)
        .map(|(i, _)| i)
        .collect();
    assert!(!bridge.is_empty(), "a stone castle has a drawbridge");
    assert!(r.lower_drawbridge());
    for cell in bridge {
        let c = r.field.cells[cell];
        assert!(!c.impassable(), "cell {cell} is still blocked");
        assert_eq!(c.flags & (siege::FLAG_WALL | siege::FLAG_DRAWBRIDGE), 0);
        assert_eq!(c.surface, siege::SURFACE_GROUND);
    }
}

// ---------------------------------------------------------------------------
// The two accumulators
// ---------------------------------------------------------------------------

/// **A moat cell filled in is one point of approach damage**, and it costs the
/// county work and no materials at all.
///
/// The **four** loads rather than fifteen are the finding, and it comes out of
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
                siege::SURFACE_GROUND | siege::SURFACE_RAMPART | siege::SURFACE_BREACH
            )
        })
        .count() as i32;
    assert_eq!(score, want);
}


/// **A figure really shovels**, over the frames the original charges it, and
/// the ditch is water until the last load goes in.
///
/// This is the state-9 handler rather than the routine underneath it, and what
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
    // 101 and 81, from the `threshold < counter` test rather than `<=`.
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
    // rather than the figure's, and it is what a check on the slot could never
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
/// `Battlefield_BuildCastle`, which is why it overwrites the fresh scores
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
