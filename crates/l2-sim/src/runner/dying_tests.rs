//! **A living man must never be drawn dead** — the player's 2026-09-14 report,
//! "a dead sprite but he was still alive".

use super::*;
use super::blank_field;

fn fight(seed: u64) -> BattleRunner {
    BattleRunner::deploy_armies(
        blank_field(),
        seed,
        Army { troops: &[(Troop::Swordsmen, 6), (Troop::Archers, 4)], owner: 1, human: false },
        Army { troops: &[(Troop::Pikemen, 6), (Troop::Peasants, 4)], owner: 2, human: true },
    )
}

/// `Motion::Dying` is state 2's pose — `BattleMan_StateDead` (`0x004830E9`),
/// `Anim_CollapseA2` (`0x00487CE4`) then `Anim_DyingA2` (`0x00487908`). Only
/// `BattleMan_StateFillMoat` (`0x00483FE1`) plays it on a living man, and there
/// is no moat on a field battlefield.
#[test]
fn no_living_figure_is_drawn_dying_in_a_field_battle() {
    for seed in [0x5EED, 1, 2, 7, 0xC0FFEE] {
        let mut r = fight(seed);
        let mut corpses = 0;
        for tick in 0..12_000u32 {
            r.step();
            for i in 0..r.fighters.len() {
                assert!(
                    !(r.is_alive(i) && r.fighters[i].anim == Motion::Dying),
                    "seed {seed} tick {tick}: figure {i} alive with the corpse pose"
                );
            }
            corpses = r.fighters.iter().filter(|f| f.anim == Motion::Dying).count();
            if r.is_decided() {
                break;
            }
        }
        assert!(corpses > 0, "seed {seed}: nobody died, the invariant proved nothing");
    }
}

/// **The moat filler is the one living man who plays `Anim_Dying`** —
/// `BattleMan_StateFillMoat` (`0x00483FE1`) is its only caller — and he must
/// wear [`Motion::Shovelling`], not the corpse pose.
///
/// **Ablation**: give him `Motion::Dying`, as this did, and two things follow
/// at once. He is drawn in the frames every dead man wears, and `step`'s corpse
/// arm counts `+0x173` out under him, so [`BattleRunner::corpse_gone`] stops
/// the renderer collecting a man who is alive and working.
#[test]
fn a_man_shovelling_earth_into_the_moat_is_alive_and_not_a_corpse() {
    let mut r = fight(0x5EED);
    let sim = r.fighters[0].sim;
    let cell = r.fighters[0].y as usize * DIM + r.fighters[0].x as usize + 1;
    r.field.cells[cell].surface = crate::siege::SURFACE_WATER;
    r.field.cells[cell].terrain = 11;
    r.sim.figures[sim].state = State::FillingMoat;
    r.fighters[0].moat_cell = Some(cell as u32);
    for _ in 0..200 {
        assert!(r.fill_moat_tick(0), "he stopped shovelling");
        assert!(r.is_alive(0));
        assert_eq!(r.fighters[0].anim, Motion::Shovelling);
        assert!(!r.corpse_gone(0), "a living man was cleared away as a body");
    }
    // `animPhase += 1` clamped at 0x5F, which is what walks the three frames.
    assert!(r.fighters[0].phase <= 0x5F);
}

/// **`Anim_DyingA2` has no knight arm.** `00480000.c:3004-3006` returns for
/// every `troopType` the band misses — of 0 … 6 that is 6 alone — *before*
/// `animPhase += 1` and *before* `facingDrawn = dirc`, so a shovelling knight
/// holds both.
///
/// **Ablation**: step them anyway and the knight walks a band drawn for men on
/// foot, his drawn facing following `dirc` while his frame does not.
#[test]
fn a_shovelling_knight_holds_his_phase_and_his_drawn_facing() {
    let mut r = BattleRunner::deploy_armies(
        blank_field(),
        0x5EED,
        Army { troops: &[(Troop::Knights, 6)], owner: 1, human: false },
        Army { troops: &[(Troop::Peasants, 6)], owner: 2, human: true },
    );
    let i = r.fighters.iter().position(|f| f.troop == Troop::Knights).unwrap();
    r.fighters[i].phase = 7;
    r.fighters[i].facing = 3;
    r.fighters[i].facing_drawn = 5;
    for _ in 0..200 {
        r.shovel(i);
        assert_eq!(r.fighters[i].anim, Motion::Shovelling);
        assert_eq!(r.fighters[i].phase, 7, "the knight's phase was stepped");
        assert_eq!(r.fighters[i].facing_drawn, 5, "the knight's drawn facing moved");
    }
}
