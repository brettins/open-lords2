#![allow(unused_imports)]
use super::*;
use super::breaches_and_assaults::*;
use super::wall_damage::*;
use super::outcomes::*;
use super::player_tactics::*;
use l2_sim::ai::{handler_for, TABLE_FIELD, TABLE_SIEGE_ATT, TABLE_SIEGE_DEF};
use l2_sim::runner::{ASSAULT_REPEATS_BELOW_LEVEL, ASSAULT_REPEAT_SCORE};
use l2_sim::siege::{
    self, SiegeState, FLAG_KEEP, FLAG_WALL, SURFACE_BAILEY, SURFACE_RAMPART_WALK, SURFACE_WALL,
};
use l2_sim::{BattleRunner, End, Muster, Troop, SIDE_A, SIDE_B};

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

