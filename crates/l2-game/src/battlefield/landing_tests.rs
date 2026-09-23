//! A network seat's cry and camera read the op's applied result, not the
//! selection standing when it was queued. `docs/netcode.md` §4.

use super::*;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::Troop;

const ALL: BattleOp = BattleOp::Pick { lo: (0, 0), hi: (254, 254), commit: true };

fn seat() -> LiveBattle {
    let field = l2_sim::terrain::build(&vec![0u8; l2_sim::terrain::CELLS], 1);
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: &[(Troop::Peasants, 20u16)], owner: 2, human: true },
        Army { troops: &[(Troop::Archers, 20u16)], owner: 1, human: true },
    );
    let mut b = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    b.paused = false;
    b.deferred = true;
    b
}

fn land(b: &mut LiveBattle) {
    for op in std::mem::take(&mut b.outbox) {
        b.apply(1, op);
    }
}

/// Ablation: `self.cry(class)` in `LiveBattle::landed` (no cry), or
/// `self.landed(&op)` in `apply`.
#[test]
fn a_queued_pick_cries_with_the_troop_it_picked_when_it_lands() {
    let mut b = seat();
    b.act_then(ALL, Follow::Cry { class: cry::SELECTED, if_held: false });
    assert!(b.cries.is_empty(), "the cry waits for the pick to land");
    land(&mut b);
    let archers = Troop::Archers.index() as u8;
    assert_eq!(b.cries, vec![Cry { troop: archers, class: cry::SELECTED }]);
    assert!(b.awaiting.is_empty());
}

/// Ablation: `Follow::Look => self.look_at_selection()` in `landed`, or the
/// `if !self.deferred` in `recall_group` (the camera jumps before the landing).
#[test]
fn a_recalled_group_moves_the_camera_when_it_lands() {
    let mut b = seat();
    b.apply(1, ALL);
    b.look_at_selection();
    let target = b.cam;
    assert!(b.store_group(b'1'));
    b.apply(1, BattleOp::Clear);
    let away = (target.0 ^ 16, target.1 ^ 16);
    b.cam = away;
    assert!(b.recall_group(b'1'));
    assert_eq!(b.cam, away, "no jump before the recall lands");
    land(&mut b);
    assert_eq!(b.cam, target, "FUN_0043C910's jump, on the recalled group");
}
