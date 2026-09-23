use super::*;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

fn two_armies() -> (Kingdom, usize, usize) {
    let mut k = Kingdom::new(1);
    k.realms[1].in_play = true;
    let mut a = Unit::new(UnitKind::Army, 1, 33, 17);
    a.troops[TroopType::Pikeman.index()] = 100;
    a.men = 100;
    let ai = k.campaign.units.spawn(a).unwrap();
    let mut d = Unit::new(UnitKind::Army, l2_kingdom::levy::OWNERLESS, 36, 17);
    d.troops[TroopType::Peasant.index()] = 50;
    d.men = 50;
    let di = k.campaign.units.spawn(d).unwrap();
    (k, ai, di)
}

/// `Battle_Start` (`0x004778A0`, `00470000.c:3104-3118`), multiplayer arm: the
/// lane steps 0..3 and the group 0..1 from the last battle's, which the
/// campaign carries. Ablation: drop the write-back in `begin_fight_in` and the
/// second battle opens at (1, 1) again.
#[test]
fn a_network_game_steps_the_selectors_from_the_last_battle() {
    let (mut k, a, d) = two_armies();
    k.campaign.battle_selectors = (3, 1);
    let seed = l2_sim::runner::DEFAULT_SEED;
    let r = begin_fight_in(&mut k, a, d, None, seed, true).unwrap();
    assert_eq!((r.session().lane, r.session().rally), (0, 0));
    assert_eq!(k.campaign.battle_selectors, (0, 0));
    begin_fight_in(&mut k, a, d, None, seed, true).unwrap();
    begin_fight_in(&mut k, a, d, None, seed, true).unwrap();
    assert_eq!(k.campaign.battle_selectors, (2, 0));
}

/// The single-player arm draws, and what it drew is what the campaign keeps.
#[test]
fn a_solo_game_keeps_the_draw() {
    let (mut k, a, d) = two_armies();
    k.campaign.battle_selectors = (3, 1);
    let r = begin_fight(&mut k, a, d, None, 7).unwrap();
    let s = r.session();
    assert_eq!(s.lane & 1, s.rally, "one value, masked twice");
    assert_eq!(k.campaign.battle_selectors, (s.lane as u8, s.rally as u8));
}
