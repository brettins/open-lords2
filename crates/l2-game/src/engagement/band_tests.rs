use super::*;
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind};

/// The player's autosave, army 8: 200 pikemen of band 2 beside a levy.
fn kingdom_with_band(levy: i32, band: u8, foes: i32) -> (Kingdom, usize, usize) {
    let mut k = Kingdom::new(1);
    k.counties[3].owner = 0;
    k.counties[3].population = 546;
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;

    let mut a = Unit::new(UnitKind::Army, 1, 33, 17);
    a.owner_is_human = true;
    a.troops[TroopType::Pikeman.index()] = levy;
    a.mercenaries = Some(Mercenaries { band: 2, troop: TroopType::Pikeman, men: band });
    a.men = levy + band as i32;
    a.moves_used = 8;
    let ai = k.campaign.units.spawn(a).unwrap();

    let mut d = Unit::new(UnitKind::Army, l2_kingdom::levy::OWNERLESS, 36, 17);
    d.home_county = 3;
    d.defence_mark = l2_kingdom::conquest::RAISED;
    d.move_allowance = 0;
    d.troops[TroopType::Peasant.index()] = foes;
    d.men = foes;
    let di = k.campaign.units.spawn(d).unwrap();
    (k, ai, di)
}

/// `Battle_WriteBackCasualties` (`0x0047F474`): a flagged figure's men go to
/// `mercMen`, the rest to its troop slot. Nobody has died yet, so the record
/// comes back as it went in. Ablation: `begin_fight` passes `None` for the
/// attacker's band — the band is not raised and reads 0.
#[test]
fn the_band_comes_back_to_the_band_and_not_to_its_troop() {
    let (mut k, a, d) = kingdom_with_band(100, 200, 50);
    let runner = begin_fight(&mut k, a, d, None, l2_sim::runner::DEFAULT_SEED).unwrap();
    conclude_fight(&mut k, a, d, runner);
    let u = k.campaign.units.get(a).unwrap();
    assert_eq!(u.troops[TroopType::Pikeman.index()], 100);
    assert_eq!(u.mercenaries, Some(Mercenaries { band: 2, troop: TroopType::Pikeman, men: 200 }));
    assert_eq!(u.men, 300);
}

/// A fought battle: survivors of the band return to it, so `UnitPanel_Draw`
/// (`0x0041B19D`) — which prints *"No mercenaries in the army."* on
/// `mercMen == 0` — prints the band instead. Ablation: `write_back`'s
/// `m.men = band as u8` removed and the band reads 0 after the fight.
#[test]
fn a_band_that_survives_a_fought_battle_is_still_in_the_army() {
    let (mut k, a, d) = kingdom_with_band(100, 200, 40);
    let attack = Attack::Battle { attacker: a, defender: d };
    let report = resolve(&mut k, attack, 3, Answer::TakeTheField, l2_sim::runner::DEFAULT_SEED)
        .expect("a battle");
    assert!(matches!(report.resolution, Resolution::Fought { .. }), "{:?}", report.resolution);
    assert!(report.verdict.attacker_won);
    let u = k.campaign.units.get(a).unwrap();
    let band = u.mercenaries.expect("the band id is never cleared");
    assert_eq!((band.band, band.troop), (2, TroopType::Pikeman));
    assert!(band.men > 0 && band.men <= 200, "band {}", band.men);
    assert!(u.troops[TroopType::Pikeman.index()] <= 100, "the band is not folded into the levy");
    assert_eq!(u.men, u.troops.iter().sum::<i32>() + band.men as i32);
}
