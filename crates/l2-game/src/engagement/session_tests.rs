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

/// An open field's marker tables are what the campaign keeps for the next castle field.
/// Ablation: drop the `battle_markers` write-back in `begin_fight_in`.
#[test]
fn the_campaign_keeps_the_last_open_fields_marker_tables() {
    let (mut k, a, d) = two_armies();
    let r = begin_fight(&mut k, a, d, None, 7).unwrap();
    assert_ne!(r.session().markers, Default::default());
    assert_eq!(k.campaign.battle_markers, r.session().markers);
}

/// The tables are `.bss`, zeroed once per process: a new game and a skirmish read the last
/// battle's. Ablation: empty `carry_battle_markers`.
#[test]
fn a_new_game_and_a_skirmish_read_the_last_battles_marker_tables() {
    let markers = ([[(9, 70); 4], [(40, 5); 4]], [[[(12, 34); 3]; 2], [[(56, 78); 3]; 2]]);
    let mut fresh = crate::Game::new(2);
    crate::setup::carry_battle_markers(&mut fresh, (markers, (0, 0), 0));
    assert_eq!(crate::setup::skirmish_session(&fresh).markers, markers);
}

/// `Battle_Start` 00470000.c:3104-3118 steps the selectors from process globals, and
/// `g_battleCounty` is one too. Ablation: drop `lane, rally` from `skirmish_session`.
#[test]
fn a_new_game_and_a_skirmish_carry_the_selectors_and_the_battle_county() {
    let mut fresh = crate::Game::new(2);
    crate::setup::carry_battle_markers(&mut fresh, (Default::default(), (3, 1), 17));
    let s = crate::setup::skirmish_session(&fresh);
    assert_eq!((s.lane, s.rally), (3, 1));
    assert_eq!(fresh.kingdom.campaign.battle_county, 17);
}

/// Siege_RestoreCastleDamage 0x004787A4: `castleDegraded != 2` leaves the fresh scores, 2
/// restores the county's. Ablation: `Some(SiegeScars::default())` on the else arm.
#[test]
fn a_first_assault_keeps_the_fresh_scores_and_a_repeat_takes_the_countys() {
    let (mut k, a, d) = two_armies();
    let mut r = begin_fight_in(&mut k, a, d, Some(1), 7, false).unwrap();
    let fresh = r.castle_damage();
    k.counties[5].siege_scars.approach_score = 9;
    restore_last_siege(&mut k, 5, &mut r);
    assert_eq!(r.castle_damage(), fresh);
    assert_eq!(k.counties[5].siege_scars.approach_score, 0, "the else arm zeroes the county's");
    k.counties[5].castle_degraded = l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED;
    k.counties[5].siege_scars.approach_score = 9;
    restore_last_siege(&mut k, 5, &mut r);
    assert_eq!(r.castle_damage().approach_score, 9);
}

/// Every campaign battle writes `g_battleCounty`. Ablation: drop the write in `resolve_battle`.
#[test]
fn a_campaign_battle_leaves_its_county_for_the_next_skirmish() {
    let (mut k, a, d) = two_armies();
    resolve(&mut k, Attack::Battle { attacker: a, defender: d }, 12, Answer::TakeTheField, 7);
    assert_eq!(k.campaign.battle_county, 12);
}
