use super::*;

fn with_band() -> BattleRunner {
    let a = [(Troop::Pikemen, 100u32)];
    let b = [(Troop::Peasants, 200u32)];
    BattleRunner::deploy_muster(
        blank_field(),
        DEFAULT_SEED,
        Muster { troops: &a, owner: 1, human: false, mercenaries: Some((Troop::Pikemen, 200)) },
        Muster { troops: &b, owner: 2, human: true, mercenaries: None },
    )
}

/// `Battle_RaiseSide` (`0x0047FEA7`): the band is raised after the eleven, in
/// units of its own, every man with `mercenary = 1` (`BattleUnit_Create`
/// `0x00480662`, `00480000.c:260-264`). Ablation: raise the band with `false`
/// in `raise_men` and the band's count reads 0.
#[test]
fn the_band_is_raised_last_in_units_of_its_own() {
    let r = with_band();
    assert_eq!(r.men(SIDE_B), 300);
    assert_eq!(r.survivors(SIDE_B)[Troop::Pikemen.index()], 100, "the levy, not the band");
    assert_eq!(r.band_survivors(SIDE_B), 200);
    assert_eq!(r.band_survivors(SIDE_A), 0);
    let side_b: Vec<&crate::Figure> = r.sim.figures.iter().filter(|f| f.side == SIDE_B).collect();
    let first_merc = side_b.iter().position(|f| f.mercenary).expect("a band figure");
    assert!(side_b[first_merc..].iter().all(|f| f.mercenary), "the band comes after the levy");
    let levy_units: Vec<u16> = side_b[..first_merc].iter().map(|f| f.unit).collect();
    assert!(side_b[first_merc..].iter().all(|f| !levy_units.contains(&f.unit)));
}

/// `Battle_WriteBackCasualties` (`0x0047F474`) reads the flag, and the flag
/// travels with a saved battle.
#[test]
fn the_flag_survives_a_save() {
    use l2_net::{Canonical, Decode, Encode, Reader};
    let f = with_band().sim.figures.iter().find(|f| f.mercenary).copied().unwrap();
    let mut out = Canonical::recording();
    f.encode(&mut out);
    let bytes = out.finish().bytes.expect("a recording encoder keeps its bytes");
    let back = crate::Figure::decode(&mut Reader::new(&bytes)).unwrap();
    assert!(back.mercenary);
}
