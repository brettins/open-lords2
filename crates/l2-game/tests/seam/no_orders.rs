#![allow(unused_imports)]
use super::*;
use super::battle::*;
use super::conquest_part::*;
use super::realm::*;
use l2_formats::save::Save;
use l2_game::engagement::{self, Answer, Resolution};
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM};
use l2_kingdom::unit::{TroopType, Unit, UnitKind, TROOP_TYPES};
use l2_sim::{Troop, SIDE_A, SIDE_B};

/// County 1 of `noorders-start.sav` — neutral, and the one the two 2026-09-17
/// pairs are both fought over.
const HELD: u8 = 1;
/// `g_units` slot 5 is the player's army in all four saves, slot 6 the levy.
const ARMY: usize = 5;
const LEVY: usize = 6;

/// The rosters the save holds, straight off `g_units` — `docs/armies.md` §1,
/// `+0x168` men and `+0x16C…` the eleven troop words.
fn roster(save: &Save, slot: usize) -> ([i32; TROOP_TYPES], i32) {
    let u = save.unit(slot).expect("a unit slot");
    let mut troops = [0i32; TROOP_TYPES];
    for (out, n) in troops.iter_mut().zip(u.troops.iter()) {
        *out = (*n).max(0) as i32;
    }
    (troops, u.men)
}

/// Both armies of the save, in a kingdom with nothing else in it, so that
/// [`engagement::begin_fight`] musters exactly what the player saw.
fn the_position(save: &Save) -> (Kingdom, usize, usize) {
    let c = save.county(HELD as usize).expect("county 1");
    assert_eq!(c.owner, 0, "county 1 is neutral when the battle opens");

    let mut k = Kingdom::new(1);
    k.county_count = 4;
    k.campaign.map = {
        let mut m = CampaignMap::empty();
        for i in 0..MAP_DIM * MAP_DIM {
            m.county[i] = HELD;
            m.flags[i] = flags::ROAD;
        }
        m
    };
    k.counties[HELD as usize].owner = 0;
    k.counties[HELD as usize].population = c.population;
    k.counties[HELD as usize].happiness = c.happiness as i32;
    k.counties[HELD as usize].anchor_x = c.anchor_x;
    k.counties[HELD as usize].anchor_y = c.anchor_y;
    k.realms[1].in_play = true;
    k.realms[1].is_human = true;
    k.options.difficulty = 0;

    let (a_troops, a_men) = roster(save, ARMY);
    let mut a = Unit::new(UnitKind::Army, 1, c.anchor_x, c.anchor_y);
    a.owner_is_human = true;
    a.county = HELD;
    a.troops = a_troops;
    a.men = a_men;
    let attacker = k.campaign.units.spawn(a).expect("a slot");

    let (d_troops, d_men) = roster(save, LEVY);
    let mut d = Unit::new(UnitKind::Army, 6, c.anchor_x, c.anchor_y);
    d.owner_is_human = false;
    d.county = HELD;
    d.troops = d_troops;
    d.men = d_men;
    let defender = k.campaign.units.spawn(d).expect("a slot");
    (k, attacker, defender)
}

/// The same roster as a muster: `Battle_InitArmies` (`0x0047EFEE`) raises a
/// side from `g_units` `+0x16C + t*2`, and the campaign record carries seven of
/// the eleven troop words.
fn muster(save: &Save, slot: usize) -> Vec<(Troop, u32)> {
    let (troops, _) = roster(save, slot);
    (0..TROOP_TYPES)
        .filter(|&t| troops[t] > 0)
        .map(|t| (l2_sim::ALL_TROOPS[t], troops[t] as u32))
        .collect()
}

pub struct Fight {
    pub attacker: (i32, i32),
    pub defender: (i32, i32),
    pub attacker_won: bool,
    pub ticks: u32,
    /// Closing loosings by class before the first melee casualty of the
    /// battle — state 17's own shot, `BattleMan_StateCloseToAttack`.
    closing_before_contact: (u32, u32),
    standing_before_contact: (u32, u32),
    pub closing: (u32, u32),
    standing: (u32, u32),
}

/// **No order at all.** [`engagement::fight`] issues one on the human side
/// through `charge_for_the_absent_player`; the player of `noorders-start.sav`
/// issued none, so the loop is driven here without it. Everything else is
/// `fight`'s: `begin_fight`, run to a conclusion, `conclude_fight`.
///
/// `on` stands in for the field `begin_fight` builds. Nothing publishes
/// `batfield.pl8` into this test binary, so `batfield::field_frame` falls back
/// to [`l2_sim::runner::blank_field`] and `None` is that blank ground.
pub fn fight_unordered(save: &Save, seed: u64, on: Option<l2_sim::Battlefield>) -> Fight {
    let (mut k, attacker, defender) = the_position(save);
    let before = |k: &Kingdom, id: usize| k.campaign.units.get(id).map_or(0, |u| u.men);
    let (a0, d0) = (before(&k, attacker), before(&k, defender));
    let (a, d) = (muster(save, ARMY), muster(save, LEVY));
    let mut runner = match on {
        Some(field) => l2_sim::runner::BattleRunner::deploy_muster(
            field,
            seed,
            l2_sim::runner::Muster { troops: &a, owner: 1, human: true, mercenaries: None },
            l2_sim::runner::Muster { troops: &d, owner: 6, human: false, mercenaries: None },
        ),
        None => engagement::begin_fight(&mut k, attacker, defender, None, seed).expect("a field"),
    };

    let (mut closing, mut standing) = ((0, 0), (0, 0));
    let mut contact = false;
    while runner.tick < engagement::MAX_TICKS {
        runner.run(1);
        if !contact {
            let c = runner.sim.cues;
            let melee: u32 = l2_sim::ALL_TROOPS.iter().map(|&t| c.melee_casualties(t)).sum();
            if melee > 0 {
                contact = true;
            } else {
                closing = (
                    c.closing_loosed(l2_sim::WeaponClass::Bow),
                    c.closing_loosed(l2_sim::WeaponClass::Crossbow),
                );
                standing = (
                    c.loosed(l2_sim::WeaponClass::Bow),
                    c.loosed(l2_sim::WeaponClass::Crossbow),
                );
            }
        }
        if runner.conclusion().is_some() {
            break;
        }
    }
    let ticks = runner.tick;
    let end = runner.sim.cues;
    let by_class = |f: &dyn Fn(l2_sim::WeaponClass) -> u32| {
        (f(l2_sim::WeaponClass::Bow), f(l2_sim::WeaponClass::Crossbow))
    };
    let whole_closing = by_class(&|w| end.closing_loosed(w));
    let whole_standing = by_class(&|w| end.loosed(w));
    let (verdict, _) = engagement::conclude_fight(&mut k, attacker, defender, runner);
    Fight {
        attacker: (a0, before(&k, attacker)),
        defender: (d0, before(&k, defender)),
        attacker_won: verdict.attacker_won,
        ticks,
        closing_before_contact: closing,
        standing_before_contact: standing,
        closing: whole_closing,
        standing: whole_standing,
    }
}

/// **The player gave no order and lost.** `noorders-start.sav` is the battle
/// screen the frame it opened; `noorders-end.sav` is the campaign map after it.
/// Between them the save says, byte for byte:
///
/// | | start | end |
/// |---|---|---|
/// | the player's army, `g_units` slot 5 `+0x168` | 52 men — 2 peasants, 25 swordsmen, 25 archers | the slot is free |
/// | the levy, slot 6 | 129 — 69 peasants, 60 archers | the slot is free |
/// | county 1 `owner` | 0 | 0 — not taken |
/// | county 1 `population` | 388 | 448, the 60 survivors returned |
/// | realm 1 `strength` | 4 | 3 |
///
/// **So the defence held, and it held with 60 of its 129.** `[V]`
///
/// Ours is asked the same question with the same absence of orders, over eight
/// seeds. Only the verdict is asserted, because it is the one number the save
/// fixes that ours reaches; the men left and the closing shots are printed.
#[test]
fn the_position_the_player_gave_no_order_in_is_held_by_the_defence() {
    let start: Save = l2_testkit::fixture!("noorders-start.sav");
    let end: Save = l2_testkit::fixture!("noorders-end.sav");

    // The save's own numbers, read here so the table above cannot rot.
    let (a_troops, a_men) = roster(&start, ARMY);
    let (d_troops, d_men) = roster(&start, LEVY);
    assert_eq!((a_men, a_troops[0], a_troops[3], a_troops[5]), (52, 2, 25, 25));
    assert_eq!((d_men, d_troops[0], d_troops[5]), (129, 69, 60));
    assert!(start.unit(ARMY).unwrap().owner_is_human, "slot 5 is the player's");
    assert_eq!(start.unit(LEVY).unwrap().owner, 6, "slot 6 is a county's levy");
    assert_eq!(end.unit(ARMY).unwrap().owner, 0, "the player's army is gone");
    assert_eq!(end.unit(LEVY).unwrap().owner, 0, "and the levy was dissolved");
    let (c0, c1) = (start.county(1).unwrap(), end.county(1).unwrap());
    assert_eq!((c0.owner, c1.owner), (0, 0), "the county was not taken");
    assert_eq!(c1.population - c0.population, 60, "60 of the 129 went home");
    assert_eq!((start.realm(1).unwrap().strength, end.realm(1).unwrap().strength), (4, 3));

    const SEEDS: u64 = 8;
    let fights: Vec<Fight> = (0..SEEDS)
        .map(|i| {
            fight_unordered(
                &start,
                l2_sim::runner::DEFAULT_SEED.wrapping_add(i.wrapping_mul(0x9E37_79B9)),
                None,
            )
        })
        .collect();
    for (i, f) in fights.iter().enumerate() {
        eprintln!(
            "seed {i}: {} ticks, attacker {:?}, defender {:?}, {} held; \
             loosed [bow, crossbow] state 17 {:?} of which {:?} before first contact, \
             state 5 {:?} of which {:?}",
            f.ticks,
            f.attacker,
            f.defender,
            if f.attacker_won { "the player" } else { "the defence" },
            f.closing,
            f.closing_before_contact,
            f.standing,
            f.standing_before_contact,
        );
    }
    let held = fights.iter().filter(|f| !f.attacker_won).count();
    eprintln!(
        "the defence held {held} of {SEEDS}; the save leaves it 60 of 129, ours {:?}",
        fights.iter().map(|f| f.defender.1).collect::<Vec<_>>()
    );

    // **The two the save fixes.**
    assert_eq!(
        held as u64, SEEDS,
        "the defence held the field in the save, unordered, 129 against 52; ours held {held}",
    );
    assert!(
        fights.iter().all(|f| f.attacker.1 == 0),
        "the save has the player's army destroyed — slot 5 free, realm 1 strength 4 to 3",
    );
    // state 17 in a real battle: no_orders_field, on the save's frame 17 (battle-4)
}
