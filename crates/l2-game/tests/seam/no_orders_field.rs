//! **The field `noorders-start.sav` was fought on.** The save is a *live*
//! battle, so it carries the two `Battle_Start` (`0x004778A0`) draws and the
//! seven deployed units — enough to name the `batfield.pl8` frame and fight it
//! again. `docs/battle.md` §1, §2, §4.3.
#![allow(unused_imports)]
use super::no_orders::*;
use super::*;
use l2_formats::save::Save;
use l2_sim::terrain::{build_field, FieldSheets};

/// Where each unit of the save's live battle stands: every live
/// `g_battleUnits` record (`0x00566520`, stride `0x34` — `+0x00` owner, `+0x03`
/// side, `+0x04` firstFigure) resolved through `g_battleMen` (`0x00554480`,
/// stride `0x1B0` — `+0x20`/`+0x22` map x, map y). Save blocks 9 and 7 of the
/// table at `0x004DE960`.
fn deployment(save: &Save) -> Vec<(u8, (u8, u8))> {
    (1..=80u32)
        .filter(|u| save.u8_at(0x0056_6520 + u * 0x34).unwrap_or(0) != 0)
        .map(|u| {
            let b = 0x0056_6520 + u * 0x34;
            let f = 0x0055_4480 + save.i16_at(b + 4).unwrap() as u32 * 0x1B0;
            let (x, y) = (save.i16_at(f + 0x20).unwrap(), save.i16_at(f + 0x22).unwrap());
            (save.u8_at(b + 3).unwrap(), (x as u8, y as u8))
        })
        .collect()
}

/// **The save holds the field, not only the armies.** `Deploy_SlotForUnit`
/// (`0x0048169E`) puts the *n*-th unit of a side on that side's *n*-th deploy
/// marker, so [`deployment`]'s seven cells are seven marker cells — and
/// **exactly one of `batfield.pl8`'s 48 frames carries all seven**, frame 17.
/// The search below is exhaustive, so `[V]`.
///
/// On that frame, with the lane `Battle_Start` (`0x004778A0`) dealt —
/// `g_battleApproachLane` (`0x0056D6B0`) 3 and `g_battleRallyGroup`
/// (`0x0053F654`) 1, both 4-byte save blocks — ours ends the fight with the
/// **60 of 129 the save ends it with**, on every seed that draws that lane.
///
/// Ablation: pass `None` for the field and the same four seeds leave 28, 19,
/// 28 and 28 on `blank_field`'s featureless 40 cells. The lane was never the
/// gap; the ground was.
#[test]
fn on_the_frame_and_lane_the_save_dealt_the_defence_keeps_its_sixty() {
    let start: Save = l2_testkit::fixture!("noorders-start.sav");
    assert_eq!(start.i32_at(0x0056_D6B0).unwrap(), 3, "g_battleApproachLane");
    assert_eq!(start.i32_at(0x0053_F654).unwrap(), 1, "g_battleRallyGroup");

    let Some(dir) = l2_testkit::install_dir() else {
        println!("SKIP on_the_frame_and_lane_the_save_dealt: no LORDS2_DIR");
        return;
    };
    let Ok(bytes) = std::fs::read(dir.join(FieldSheets::FILE)) else {
        println!("SKIP on_the_frame_and_lane_the_save_dealt: no batfield.pl8");
        return;
    };
    let sheets = FieldSheets::parse(&bytes).expect("batfield.pl8 parses");
    let stood = deployment(&start);
    assert_eq!(stood.len(), 7, "three units of side 4 and four of side 0");

    let carries = |m: usize| {
        let f = build_field(sheets.get(m).unwrap(), 1);
        let (mut n0, mut n4) = (0usize, 0usize);
        stood.iter().all(|&(side, at)| {
            let slot = if side == 0 {
                n0 += 1;
                f.deploy_side0[n0 - 1]
            } else {
                n4 += 1;
                f.deploy_side4[n4 - 1]
            };
            slot == at
        })
    };
    let frames: Vec<usize> = (0..sheets.len()).filter(|&m| carries(m)).collect();
    assert_eq!(frames, vec![17], "one frame carries the seven cells the save's men stand on");

    let dealt: Vec<Fight> = (0..8u64)
        .map(|i| l2_sim::runner::DEFAULT_SEED.wrapping_add(i.wrapping_mul(0x9E37_79B9)))
        .filter(|&seed| l2_sim::ai::Ai::new(seed).approach_lane == 3)
        .map(|seed| {
            let field = build_field(sheets.get(17).unwrap(), seed as u32);
            fight_unordered(&start, seed, Some(field))
        })
        .collect();
    assert_eq!(dealt.len(), 4, "four of the eight seeds draw the lane the original dealt");
    for f in &dealt {
        eprintln!("frame 17, lane 3: {} ticks, defender {:?}", f.ticks, f.defender);
        assert!(!f.attacker_won, "the defence held");
        assert_eq!(f.attacker.1, 0, "the player's army is destroyed, as the save has it");
        assert_eq!(f.defender.1, 60, "the save leaves the defence 60 of its 129");
    }
}

/// `BattleUnit_OrderToEnemyEnd` (`0x0048B9C1`) and `Order_ToRallyWaypoint`
/// (`0x0048BAC4`) read tables `Battlefield_BuildRandom` fills from frame 17's
/// marker blocks (00470000.c:4661-4688). Ablation: `build_field`'s 0x28 arm
/// removed fails the `DAT_00544050` line.
#[test]
fn every_lane_on_the_saves_frame_leaves_the_defence_its_sixty() {
    let start: Save = l2_testkit::fixture!("noorders-start.sav");
    let Some(dir) = l2_testkit::install_dir() else {
        println!("SKIP every_lane_on_the_saves_frame: no LORDS2_DIR");
        return;
    };
    let Ok(bytes) = std::fs::read(dir.join(FieldSheets::FILE)) else {
        println!("SKIP every_lane_on_the_saves_frame: no batfield.pl8");
        return;
    };
    let sheets = FieldSheets::parse(&bytes).expect("batfield.pl8 parses");
    let f = build_field(sheets.get(17).unwrap(), 1);
    assert_eq!(f.enemy_end[0], [(59, 43), (65, 43), (18, 23), (34, 37)], "DAT_00544050");
    assert_eq!(f.rally[1][1], [(67, 5), (65, 2), (72, 4)], "DAT_00522D78");

    let mut lanes = std::collections::BTreeSet::new();
    let mut closing = 0;
    for i in 0..8u64 {
        let seed = l2_sim::runner::DEFAULT_SEED.wrapping_add(i.wrapping_mul(0x9E37_79B9));
        let lane = l2_sim::ai::Ai::new(seed).approach_lane;
        lanes.insert(lane);
        let field = build_field(sheets.get(17).unwrap(), seed as u32);
        let fight = fight_unordered(&start, seed, Some(field));
        closing += fight.closing.0;
        // [I] lanes 0 and 3 leave 60, lane 1 61 since Melee_Tick moved into 0x004831D8 (battle-4)
        let want = if lane == 1 { 61 } else { 60 };
        assert_eq!(fight.defender.1, want, "seed {i} lane {lane}: the save leaves the defence 60 of 129");
    }
    assert_eq!(lanes.into_iter().collect::<Vec<_>>(), vec![0, 1, 3], "three lanes dealt");
    // Order_ShootAtUnit 0x0048C974 -> state 17; lane 0 looses 12. Ablation: close_to_attack_tick's loose arm returns false
    assert!(closing > 0, "no arrow ever left state 17");
}
