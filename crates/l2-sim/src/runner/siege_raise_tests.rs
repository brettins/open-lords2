use super::*;

fn garrison(band: Option<(Troop, u32)>) -> BattleRunner {
    let a = [(Troop::Swordsmen, 200u32)];
    let b = [
        (Troop::Pikemen, 40u32),
        (Troop::Archers, 200),
        (Troop::Knights, 20),
        (Troop::Crossbowmen, 100),
        (Troop::Oil, 2),
    ];
    BattleRunner::deploy_siege(
        crate::siege::our_castle(2),
        DEFAULT_SEED,
        Muster { troops: &a, owner: 1, human: false, mercenaries: None },
        Muster { troops: &b, owner: 2, human: true, mercenaries: band },
        2,
    )
}

fn units_of(r: &BattleRunner, side: Side) -> Vec<(Troop, usize, u8)> {
    let mut out: Vec<(Troop, usize, u8)> = Vec::new();
    let mut last = 0u16;
    for f in r.sim.figures.iter().filter(|f| f.side == side && f.is_alive()) {
        if f.unit != last {
            out.push((f.troop, 0, r.units.get(f.unit as usize).category));
            last = f.unit;
        }
        out.last_mut().unwrap().1 += 1;
    }
    out
}

/// `Battle_RaiseSideSiege` (`0x004801F8`): `g_raiseOrderSiege` order, archer and
/// crossbow units of at most 3 figures (`00480000.c:50-52`). Ablation: drop the
/// `at.siege => 3` arm and the archer units grow.
#[test]
fn the_garrison_is_raised_in_the_siege_order_with_missile_units_of_three() {
    let r = garrison(None);
    let units = units_of(&r, SIDE_A);
    let order: Vec<Troop> = units.iter().map(|u| u.0).fold(Vec::new(), |mut v, t| {
        if v.last() != Some(&t) {
            v.push(t);
        }
        v
    });
    let want = [Troop::Oil, Troop::Archers, Troop::Crossbowmen, Troop::Knights, Troop::Pikemen];
    assert_eq!(order, want);
    for &(t, figures, _) in &units {
        if matches!(t, Troop::Archers | Troop::Crossbowmen) {
            assert!(figures <= 3, "{t:?} unit of {figures}");
        }
    }
    let mpf = r.men_per_figure[0] as usize;
    let archer_units = units.iter().filter(|u| u.0 == Troop::Archers).count();
    assert_eq!(archer_units, 200usize.div_ceil(3 * mpf));
}

/// `BattleUnit_Create` (`0x00480662`, `00480000.c:199-208`): side 0's first
/// missile unit is category 9, the next two 10, the rest 1. Ablation: `< 2`
/// for `< 3` and the third missile unit reads 1.
#[test]
fn the_first_missile_unit_is_9_and_the_next_two_10() {
    let r = garrison(None);
    let cats: Vec<u8> = units_of(&r, SIDE_A)
        .iter()
        .filter(|u| matches!(u.0, Troop::Archers | Troop::Crossbowmen))
        .map(|u| u.2)
        .collect();
    assert!(cats.len() > 3);
    assert_eq!(&cats[..4], &[9, 10, 10, 1]);
}

/// The band is raised by the same rules (`00480000.c:73-99`), flagged.
#[test]
fn a_garrison_band_of_archers_is_capped_at_three() {
    let r = garrison(Some((Troop::Archers, 400)));
    let band: Vec<usize> = units_of(&r, SIDE_A)
        .iter()
        .filter(|u| u.0 == Troop::Archers)
        .map(|u| u.1)
        .collect();
    assert!(band.iter().all(|&n| n <= 3));
    assert!(r.sim.figures.iter().any(|f| f.side == SIDE_A && f.mercenary));
}

/// `Deploy_SlotForUnitSiege` (`0x004816F9`, `00480000.c:484-527`). Ablation:
/// map archers to slot 0 unconditionally and the first assertion fails.
#[test]
fn the_siege_slots_follow_the_troop_type() {
    let mut r = garrison(None);
    let mut oil = 0;
    let slot = |r: &BattleRunner, n: usize| r.field.deploy_side0[n];
    r.wall_missile_latch = 0;
    assert_eq!(r.deploy_slot_siege(Troop::Archers, &mut oil), slot(&r, 1));
    r.wall_missile_latch = 1;
    assert_eq!(r.deploy_slot_siege(Troop::Crossbowmen, &mut oil), slot(&r, 0));
    assert_eq!(r.deploy_slot_siege(Troop::Knights, &mut oil), slot(&r, 8));
    assert_eq!(r.deploy_slot_siege(Troop::Peasants, &mut oil), slot(&r, 8));
    assert_eq!(r.deploy_slot_siege(Troop::Pikemen, &mut oil), slot(&r, 4));
    for k in 0..16 {
        r.ai_field.wall_slot[1][k] = if k < 2 { (10 + k as i16, 20) } else { (0, 0) };
    }
    let pots: Vec<(u8, u8)> = (0..4).map(|_| r.deploy_slot_siege(Troop::Oil, &mut oil)).collect();
    assert_eq!(pots, vec![(10, 20), (11, 20), (10, 20), (11, 20)]);
}

fn in_session(field: crate::terrain::Battlefield, level: Option<u8>, session: Session) -> BattleRunner {
    let a = [(Troop::Swordsmen, 200u32)];
    let b = [(Troop::Archers, 200u32)];
    let a = Muster { troops: &a, owner: 1, human: false, mercenaries: None };
    let b = Muster { troops: &b, owner: 2, human: true, mercenaries: None };
    BattleRunner::deploy_in_session(field, None, DEFAULT_SEED, a, b, level, session)
}

/// Battlefield_BuildCastle 0x0047C4BA writes no DAT_00544050/g_rallyWaypoints: the siege
/// orders to the last field's. Ablation: drop the castle arm in `deploy_prepared`.
#[test]
fn a_castle_field_orders_to_the_last_open_fields_marker_tables() {
    let markers = ([[(11, 12); 4], [(13, 14); 4]], [[[(21, 22); 3]; 2], [[(23, 24); 3]; 2]]);
    let session = Session { markers, ..Session::default() };
    let r = in_session(crate::siege::our_castle(2), Some(2), session);
    assert_eq!(r.ai_field.enemy_end[1][3], (13, 14), "DAT_00544070");
    assert_eq!(r.ai_field.rally[0][1][2], (21, 22), "g_rallyWaypoints");
    assert_eq!(r.session().markers, markers);
    let first = in_session(crate::siege::our_castle(2), Some(2), Session::default());
    assert_eq!(first.session().markers, Default::default(), ".bss at process start");
}

/// An open field's own tables replace the carried ones. Ablation: carry them on the field too.
#[test]
fn an_open_field_hands_on_its_own_marker_tables() {
    let markers = ([[(11, 12); 4], [(13, 14); 4]], [[[(21, 22); 3]; 2], [[(23, 24); 3]; 2]]);
    let field = crate::siege::our_castle(2);
    let own = (field.enemy_end, field.rally);
    let r = in_session(field, None, Session { markers, ..Session::default() });
    assert_eq!(r.session().markers, own);
}
