#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use l2_sim::castle::{self, CastleSheets};
use l2_sim::siege::{
    self, DOCK_WALL_ELEVATION, FLAG_DRAWBRIDGE, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY,
    SURFACE_RAMPART_WALK, SURFACE_WATER,
};
use l2_sim::terrain::DIM;
use l2_sim::{BattleRunner, Muster, Troop};

/// `Deploy_SlotForUnitSiege` (`0x004816F9`) maps a troop type to slot 0, 1, 4
/// or 8 of the garrison's twelve and to nothing else. Every shipped layout
/// fills exactly those four and leaves the other eight at zero — which nothing
/// in this decode arranges for, so it is the check that the 2 x 2 marker
/// blocks and the `buf[+2] - 0x40` index are being read the way `Marker_DeploySlot`
/// reads them.
#[test]
fn the_garrison_slots_the_layouts_fill_are_the_four_the_dispatcher_reaches() {
    let s = sheets!();
    for level in 0..=4u8 {
        let t = castle::tables(s.get(level).unwrap());
        let filled: Vec<usize> = (0..12).filter(|&i| t.deploy_side0[i] != (0, 0)).collect();
        assert_eq!(filled, vec![0, 1, 4, 8], "level {level}");
        assert!(t.deploy_side4.iter().all(|&(x, y)| x > 0 && y >= 60), "level {level}");
        assert_eq!(t.deploy_side4.iter().filter(|&&p| p != (0, 0)).count(), 12);
    }
}

/// The wall slots, the approach lanes and the castle's reference cell — every
/// one of them a `[I]` in [`l2_sim::AiField`] until this read them.
#[test]
fn the_structure_layer_fills_the_tables_the_ai_reads() {
    let s = sheets!();
    for level in 0..=4u8 {
        let sheet = s.get(level).unwrap();
        let f = castle::build(level, sheet);
        let t = castle::tables(sheet);
        let ai = castle::ai_field(&f, level, &t);
        assert!(ai.wall_slot[0][0] != (0, 0), "level {level}: no wall slots");
        assert!(ai.wall_slot[1][0] != (0, 0), "level {level}: no inner wall slots");
        assert!(ai.castle_ref != (0, 0), "level {level}: no reference cell");
        assert!(ai.staging.iter().all(|&p| p != (0, 0)), "level {level}: four lanes");
        assert!(ai.castle_approach[4].iter().all(|&p| p == (0, 0)), "level {level}");
        assert!(ai.castle_approach[2].iter().all(|&p| p == ai.castle_ref), "level {level}");
        for p in ai.wall_slot.iter().flatten().chain(ai.castle_approach.iter().flatten()) {
            assert!((0..DIM as i16).contains(&p.0) && (0..DIM as i16).contains(&p.1), "{p:?}");
        }
    }
}

/// Boiling oil wants elevation 2 under the pot (`Oil_FindPourTarget`) and a
/// tower wants elevation 2 in front of it (`FUN_00491492`). Both are counted by
/// [`l2_sim::Cues`], so this runs the battle and reads the counters
/// asserting on geometry.
#[test]
fn oil_pours_and_a_tower_docks_in_a_siege_of_a_real_castle() {
    let s = sheets!();
    let attacker = [
        (Troop::Peasants, 240u32),
        (Troop::Archers, 80),
        (Troop::Swordsmen, 80),
        (Troop::Knights, 40),
        (Troop::Catapults, 2),
        (Troop::SiegeTowers, 4),
        (Troop::BatteringRams, 1),
    ];
    let defender = [
        (Troop::Archers, 500u32),
        (Troop::Crossbowmen, 200),
        (Troop::Swordsmen, 40),
        (Troop::Pikemen, 40),
        (Troop::Knights, 20),
        (Troop::Oil, 6),
    ];
    let mut poured = 0;
    let mut docked = 0;
    for level in 0..=4u8 {
        let sheet = s.get(level).unwrap();
        let mut r = BattleRunner::deploy_siege_on_sheet(
            castle::build(level, sheet),
            &castle::tables(sheet),
            0x0C_A571_E0u64.wrapping_mul(level as u64 + 1),
            Muster { troops: &attacker, owner: 1, human: false },
            Muster { troops: &defender, owner: 2, human: false },
            level,
        );
        for _ in 0..60_000 {
            if r.conclusion().is_some() {
                break;
            }
            r.step();
        }
        poured += r.sim.cues.oil_poured();
        docked += r.sim.cues.towers_docked();
        eprintln!(
            "level {level}: oil {} towers {} frames {}",
            r.sim.cues.oil_poured(),
            r.sim.cues.towers_docked(),
            r.tick
        );
    }
    assert!(poured > 0, "no pot of oil was ever poured in five real sieges");
    assert!(docked > 0, "no siege tower ever docked in five real sieges");
}

/// `Battlefield_PlaceMoatCell` zeroes `g_siegeApproachScore`
/// ([`siege::approach_score_at_build`]), and `UnitOrder_SiegeAttTower`
/// (`0x0048DDC7`) moves a tower at the castle only on
/// `(orders > 0x3C && approach > 6) || approach > 10`. So a tower may only
/// reach a wall on a **dry** castle, and the docks C203 measured — 1, 0, 1, 0,
/// 0 — are exactly the dry levels. Elevation is not the reason: level 3 has
/// **256** cells at [`DOCK_WALL_ELEVATION`], more than level 2's 134. `[V]`
#[test]
fn only_a_dry_castle_opens_the_approach_a_siege_tower_needs() {
    let s = sheets!();
    let mut wet = 0;
    for level in 0..=4u8 {
        let field = castle::build(level, s.get(level).unwrap());
        let moat = field.cells.iter().any(|c| c.surface == SURFACE_WATER);
        let score = siege::approach_score_at_build(&field);
        assert_eq!(moat, score == 0, "level {level}");
        // C203's moat levels, restated from the file.
        assert_eq!(moat, matches!(level, 1 | 3 | 4), "level {level}");
        wet += moat as i32;
        let high = field.cells.iter().filter(|c| c.elevation == DOCK_WALL_ELEVATION).count();
        assert!(high > 100, "level {level}: {high} cells a tower could dock against");
    }
    assert_eq!(wet, 3, "three of the five castles ship a ditch");
}

