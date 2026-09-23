use super::*;

fn siege(level: u8) -> BattleRunner {
    let a = [(Troop::Swordsmen, 200u32), (Troop::Catapults, 1)];
    let b = [(Troop::Pikemen, 100u32)];
    BattleRunner::deploy_siege(
        crate::siege::our_castle(level),
        DEFAULT_SEED,
        Muster { troops: &a, owner: 1, human: false, mercenaries: None },
        Muster { troops: &b, owner: 2, human: true, mercenaries: None },
        level,
    )
}

/// `Battle_CheckOutcome` (`0x00477DFC`, `00470000.c:3344-3347`):
/// `g_castleEntered` credits `g_battleArmyA`, side 4.
#[test]
fn a_break_in_is_won_by_army_a() {
    let mut r = siege(4);
    r.siege.broke_in = true;
    r.ai.breach_score = 1;
    assert_eq!(r.conclusion(), Some(Conclusion { winner: SIDE_B, cause: End::BrokeIn }));
}

/// `:3349-3369` runs after the break-in arm, so with no breach and no engines
/// at a large castle the garrison is credited in the same frame. Ablation:
/// test `broke_in` first and this reads `BrokeIn`.
#[test]
fn a_failed_assault_outranks_a_break_in_in_the_same_frame() {
    let mut r = siege(4);
    r.siege.broke_in = true;
    r.ai.breach_score = 0;
    r.ai.siege_engine_count = 0;
    assert_eq!(r.conclusion(), Some(Conclusion { winner: SIDE_A, cause: End::AssaultFailed }));
    r.siege.castle_level = 0;
    assert_eq!(r.conclusion().map(|c| c.cause), Some(End::BrokeIn), "`local_8 == 0`: no end");
}
