#![allow(unused_imports)]
use super::*;



/// This is the property `docs/netcode.md` D-3 needs from an event stream a
/// listener reads — nothing it records may reach a decision — and it is
/// checked on the running battle
/// uses. Ablation, by insertion: make
/// [`BattleRunner::step`] skip its missile sweep while
/// `self.sim.cues.missile_hits(WeaponClass::Bow) > 0` and this goes red on
/// the tick after the first hit.
#[test]
fn a_battle_whose_cues_are_wiped_every_tick_is_the_same_battle() {
    let build = || {
        BattleRunner::deploy(
            blank_field(),
            &[(Troop::Archers, 6), (Troop::Swordsmen, 4)],
            &[(Troop::Crossbowmen, 6), (Troop::Macemen, 4)],
        )
    };
    let (mut heard, mut wiped) = (build(), build());
    for t in 0..6_000u32 {
        if t % 300 == 0 {
            for r in [&mut heard, &mut wiped] {
                r.order_side(SIDE_A, 40, 40);
                r.order_side(SIDE_B, 40, 40);
            }
        }
        wiped.sim.cues = crate::cue::Cues::default();
        heard.step();
        wiped.step();
        let mut same = wiped.clone();
        same.sim.cues = heard.sim.cues;
        assert_eq!(heard, same, "the cues changed the battle at tick {}", heard.tick);
    }
    let c = heard.sim.cues;
    let shots = c.loosed(WeaponClass::Bow) + c.loosed(WeaponClass::Crossbow);
    let hits = c.missile_hits(WeaponClass::Bow) + c.missile_hits(WeaponClass::Crossbow);
    let melee: u32 = crate::ALL_TROOPS.iter().map(|&t| c.melee_casualties(t)).sum();
    assert!(shots > 0 && hits > 0, "the battle must actually shoot and hit: {c:?}");
    assert!(melee > 0, "and come to blows: {c:?}");
}

#[test]
fn a_weakened_archer_shoots_for_less() {
    let full = crate::missile::band_scaled(WeaponClass::Bow.stats().damage, 0);
    let hurt = crate::missile::band_scaled(WeaponClass::Bow.stats().damage, 3);
    assert_eq!(full, 50);
    assert_eq!(hurt, 25, "half at the bottom band");
}

