//! `BattleUnit_Order` (`0x00479E90`), the one order path for AI and player.

use super::*;
use crate::ai::World;

fn one(troop: Troop, human: bool, at: (u8, u8), surface: Option<u8>) -> (BattleRunner, usize, usize) {
    let mut field = blank_field();
    if let Some(s) = surface {
        field.cells[cell(at.0, at.1)].surface = s;
    }
    let mut r = BattleRunner::empty(field, DEFAULT_SEED);
    let i = stand(&mut r, troop, SIDE_A, 1, human, at);
    let unit = r.units.create(1, human, SIDE_A, 1).unwrap();
    let sim = r.fighters[i].sim;
    r.sim.figures[sim].unit = unit as u16;
    r.settle();
    (r, unit, i)
}

/// 00470000.c:4405 `reTarg = 0x14`; `BattleUnit_Reform`'s one caller is `Battle_UpdateAllUnits`
/// 00480000.c:3656. Ablation: `self.reform_unit(unit)` back at the end of `order_full`.
#[test]
fn a_players_order_sends_the_men_twenty_frames_after_the_click() {
    let (mut r, unit, man) = one(Troop::Swordsmen, true, (40, 34), None);
    r.order_full(unit, 40, 60, None, false, Formation::Keep);
    assert_eq!(r.fighters[man].target, (40, 34), "no destination on the click");
    let sent = (1..=40).find(|_| {
        r.step();
        r.fighters[man].target != (40, 34)
    });
    assert_eq!(sent, Some(20));
}

/// `FUN_0047a814` on every order, 00470000.c:4393: an AI pot re-ordered to the cell it already
/// holds pours. Ablation: pour only an `Ai::orders` entry whose target changed.
#[test]
fn an_ai_pot_reordered_to_the_cell_it_holds_still_pours() {
    let (mut r, unit, pot) = one(Troop::Oil, false, (40, 40), Some(4));
    r.units.get_mut(unit).target_x = 40;
    r.units.get_mut(unit).target_y = 42;
    {
        let BattleRunner { units, sim, positions, ai_field, ai, .. } = &mut r;
        let mut w = World { units, figures: &mut sim.figures, positions, field: ai_field, ai };
        w.set_target(unit, 40, 42);
    }
    r.step();
    assert!(!r.is_alive(pot), "the pot is spent by its pour");
    assert_eq!(r.sim.cues.oil_poured(), 1);
}

/// `targeted` is a byte: `+ 2` wraps (00490000.c:2043) and is read `(char)` (:2031).
/// Ablations: `saturating_add(2)` in `retarget`; `targeted as i32` in `chase_target`.
#[test]
fn a_man_targeted_past_127_draws_chasers_and_wraps_to_zero() {
    let (mut r, _, me) = one(Troop::Swordsmen, false, (40, 34), None);
    let _near = stand(&mut r, Troop::Swordsmen, SIDE_B, 2, true, (40, 36));
    let far = stand(&mut r, Troop::Swordsmen, SIDE_B, 2, true, (40, 37));
    let far_sim = r.fighters[far].sim;
    r.sim.figures[far_sim].targeted = 254;
    r.sim.figures[r.fighters[me].sim].state = crate::figure::State::Chasing;
    r.retarget(me);
    assert_eq!(r.sim.figures[r.fighters[me].sim].target, Some(far_sim), "3 + (char)254 = 1 beats 2");
    assert_eq!(r.sim.figures[far_sim].targeted, 0, "254 + 2 wraps");
}

/// The pour loop tests `owner != 0`, not state (00470000.c:4389), and the rebuild counts a corpse
/// (00480000.c:3533) until BattleMan_StateDead frees it (:1337). Ablations: `!is_alive()` in
/// `pour_on_order`; `!is_alive()` in `rebuild_from_figures`; the `owner = 0` in `step`.
#[test]
fn a_spent_pot_pours_again_and_its_unit_lasts_until_the_slot_is_freed() {
    let (mut r, unit, _pot) = one(Troop::Oil, true, (40, 40), Some(4));
    r.order_unit(unit, 40, 42);
    assert_eq!(r.sim.cues.oil_poured(), 1);
    r.step();
    assert!(r.units.get(unit).is_live(), "the spent pot's unit is still in the list");
    r.order_unit(unit, 40, 36);
    assert_eq!(r.sim.cues.oil_poured(), 2, "a spent pot pours again");
    let freed = (2..=120).find(|_| {
        r.step();
        !r.units.get(unit).is_live()
    });
    assert_eq!(freed, Some(82), "80 corpse frames, then the next frame's rebuild");
}
