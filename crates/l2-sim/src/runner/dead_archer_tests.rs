use super::*;
use super::blank_field;
use crate::missile::apply_hit;

fn fight() -> BattleRunner {
    BattleRunner::deploy_armies(
        blank_field(),
        0x5EED,
        Army { troops: &[(Troop::Archers, 4)], owner: 1, human: false },
        Army { troops: &[(Troop::Pikemen, 4)], owner: 2, human: true },
    )
}

/// Battle_UpdateAllMen 00480000.c:873 dispatches by state: a body ordered to
/// state 17 (BNEW-dead-archer-shoots) no longer runs BattleMan_StateDead, so
/// `+0x173` stops and the slot is never freed. Ablation: `is_corpse` as `!is_alive`.
#[test]
fn a_body_that_stood_up_stops_counting_down() {
    let mut r = fight();
    let sim = r.fighters[0].sim;
    r.sim.figures[sim].men = 0;
    r.sim.figures[sim].state = State::Dead;
    r.step();
    let counted = r.fighters[0].corpse;
    r.sim.figures[sim].state = State::Shooting;
    for _ in 0..200 {
        r.step();
    }
    assert_eq!(r.fighters[0].corpse, counted);
    assert_ne!(r.sim.figures[sim].owner, 0, "the slot was freed");
}

/// Missile_Step 00490000.c:1443 `men < 1` sits outside the casualty threshold:
/// any hit puts him back in state 2. Ablation: the early return in `apply_hit`.
#[test]
fn any_hit_lays_him_down_again() {
    let mut r = fight();
    let sim = r.fighters[0].sim;
    let f = &mut r.sim.figures[sim];
    f.men = 0;
    f.state = State::Shooting;
    assert_eq!(apply_hit(f, 1), 0);
    assert_eq!(f.state, State::Dead);
}
