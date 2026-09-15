#![allow(unused_imports)]
use super::*;

use l2_formats::save::Save;
use l2_kingdom::battle::{self, CASTLE_STRENGTH_PERCENT};
use l2_kingdom::siege::{self, Engine, ENGINE_WORK};
use l2_kingdom::unit::{Unit, UnitKind, Units, TROOP_TYPES};

#[test]
fn one_catapult_and_forty_three_men_reproduce_every_snapshot_of_the_build() {
    let steps: [(&str, i32, i32, u8); 4] = [
        ("siege-safeturn.sav", 86, 43, 3),
        ("siege-old_turn.sav", 129, 64, 2),
        ("siege-lastturn.sav", 172, 86, 1),
        ("siege-sieging.sav", 200, 100, 0),
    ];

    let total = ENGINE_WORK[Engine::Catapult.index()];
    let mut previous_work = 43; // the season before the earliest snapshot
    for (name, work, percent, seasons) in steps {
        let save: Save = l2_testkit::fixture!(name);
        let a = unit(&save, BESIEGER);

        assert_eq!(a.kind, 1, "{name}: the besieger is an army");
        assert_eq!(a.men, 43, "{name}: 43 men — the fixture fingerprint");
        assert_eq!(
            a.besieging_county, BESIEGED_COUNTY as u8,
            "{name}: it is siege-linked to county 4"
        );
        // **The two fields are not the same field.** A besieger is *outside*
        // the castle, so `+0x198` is zero and `+0x199` is the county.
        assert_eq!(a.garrison_county, 0, "{name}: a besieger is not a garrison");

        assert_eq!(a.engines[0].0, 1, "{name}: one catapult ordered");
        assert_eq!((a.engines[1].0, a.engines[2].0), (0, 0), "{name}: nothing else");

        assert_eq!(a.engines[0].2, work, "{name}: man-seasons of work done");
        assert_eq!(a.engines[0].1, percent, "{name}: percent complete");
        assert_eq!(a.seasons_left, seasons, "{name}: seasons the screen prints");

        assert_eq!(total, 200, "a catapult costs 200 man-seasons");
        assert_eq!(
            work - previous_work,
            43.min(total - previous_work),
            "{name}: a season adds the army's men, capped at what is left"
        );
        assert_eq!(a.engines[0].1, work * 100 / total, "{name}: percent is work*100/total");
        assert_eq!(
            a.seasons_left as i32,
            (total - work).div_euclid(43) + i32::from((total - work) % 43 != 0),
            "{name}: seasons are ceil(remaining / men)"
        );
        previous_work = work;
    }
}

#[test]
fn our_build_tick_walks_the_same_four_records() {
    let save: Save = l2_testkit::fixture!("siege-safeturn.sav");
    let a = unit(&save, BESIEGER);
    assert_eq!(a.men, 43);

    let mut units = Units::new();
    let mut army = Unit::new(UnitKind::Army, 1, 46, 41);
    army.owner_is_human = true;
    army.men = 43;
    let id = units.spawn(army).unwrap();
    units.get_mut(id).unwrap().besieging_county = BESIEGED_COUNTY as u8;
    siege::order_engine(&mut units, id, Engine::Catapult, 1);

    assert_eq!(units.get(id).unwrap().siege_seasons_left, 5);

    let expected = [(43, 21, 4), (86, 43, 3), (129, 64, 2), (172, 86, 1), (200, 100, 0)];
    for (season, (work, percent, seasons)) in expected.into_iter().enumerate() {
        let ready = siege::build_tick(&mut units, id);
        let u = units.get(id).unwrap();
        assert_eq!(u.engines[0].work_done as i32, work, "season {season}");
        assert_eq!(u.engines[0].percent as i32, percent, "season {season}");
        assert_eq!(u.siege_seasons_left, seasons, "season {season}");
        assert_eq!(ready, seasons == 0, "season {season}: ready exactly at zero");
    }
}

#[test]
fn the_staged_battle_carries_one_catapult_and_one_pot_of_oil() {
    let save: Save = l2_testkit::fixture!("siege-sieging.sav");
    let a = unit(&save, BESIEGER);
    let d = unit(&save, GARRISON);

    assert_eq!(castle_type(&save), 1, "a wooden palisade");
    let level = castle_type(&save) - 1;
    assert_eq!(level, 0);

    let staged = siege::prepare_besieger(&{
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        for (e, record) in u.engines.iter_mut().enumerate() {
            record.ordered = a.engines[e].0 as i16;
        }
        u
    });
    assert_eq!(staged.counts()[..3], [a.troops[7], a.troops[8], a.troops[9]]);
    assert_eq!(a.troops[7], 1, "the one catapult it built");

    assert_eq!(d.troops[10], siege::prepare_garrison(level).oil);
    assert_eq!(d.troops[10], 1);
    assert_eq!(a.troops[10], 0, "an attacker never gets oil");
}

#[test]
fn the_castle_bonus_for_a_palisade_is_the_only_one_that_reproduces_the_aftermath() {
    let before: Save = l2_testkit::fixture!("siege-sieging.sav");
    let after: Save = l2_testkit::fixture!("siege-aftersie.sav");

    let a = unit(&before, BESIEGER);
    let d = unit(&before, GARRISON);
    let survivor = unit(&after, GARRISON);

    assert_eq!(a.men, 43);
    assert_eq!(d.men, 149);
    assert_eq!(d.besieged_by, BESIEGER as u8, "the pair is linked before");
    assert_eq!(survivor.men, 133, "and the garrison came out at 133");
    assert_eq!(survivor.besieged_by, 0, "with the siege link gone");
    assert_eq!(garrison_slot(&after), GARRISON as i32, "and still holding its castle");
    assert_eq!(
        unit(&after, BESIEGER).owner,
        0,
        "the besieger's slot is free — the loser was destroyed"
    );

    let expected: Vec<i32> = survivor.troops[..TROOP_TYPES].to_vec();

    let mut agreed = Vec::new();
    for level in 0..CASTLE_STRENGTH_PERCENT.len() as u8 {
        let mut units = Units::new();
        let attacker = {
            let mut u = Unit::new(UnitKind::Army, 1, 46, 41);
            u.owner_is_human = true;
            u.troops = core::array::from_fn(|t| a.troops[t]);
            u.men = a.men;
            units.spawn(u).unwrap()
        };
        let defender = {
            let mut u = Unit::new(UnitKind::Army, 2, 45, 42);
            u.troops = core::array::from_fn(|t| d.troops[t]);
            u.men = d.men;
            u.garrison_county = BESIEGED_COUNTY as u8;
            units.spawn(u).unwrap()
        };
        let verdict =
            battle::auto_resolve(&mut units, attacker, defender, Some(level)).unwrap();
        let left = units.get(defender).unwrap();
        if !verdict.attacker_won && left.troops.to_vec() == expected && left.men == survivor.men {
            agreed.push(level);
        }
    }

    assert_eq!(
        agreed,
        vec![0],
        "exactly one castle level reproduces 4/20/125 -> 3/18/112 and 149 -> 133"
    );
    assert_eq!(CASTLE_STRENGTH_PERCENT[0], 160);
}

#[test]
fn the_fixture_position_drives_our_siege_from_the_guard_to_the_assault() {
    let save: Save = l2_testkit::fixture!("siege-safeturn.sav");
    let a = unit(&save, BESIEGER);
    let d = unit(&save, GARRISON);

    let mut counties: [l2_kingdom::County; l2_kingdom::MAX_COUNTIES] =
        core::array::from_fn(|_| l2_kingdom::County::new());
    let mut realms: [l2_kingdom::Realm; l2_kingdom::MAX_REALMS] =
        core::array::from_fn(|_| l2_kingdom::Realm::new());
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[2].in_play = true;
    realms[2].lord = 1;

    let mut units = Units::new();
    let mut garrison = Unit::new(UnitKind::Army, d.owner, 45, 42);
    garrison.troops = core::array::from_fn(|t| d.troops[t]);
    garrison.men = d.men;
    garrison.garrison_county = BESIEGED_COUNTY as u8;
    let g = units.spawn(garrison).unwrap();

    counties[BESIEGED_COUNTY as usize].owner = d.owner;
    counties[BESIEGED_COUNTY as usize].castle_type = castle_type(&save);
    counties[BESIEGED_COUNTY as usize].garrison_unit = g;

    let mut army = Unit::new(UnitKind::Army, a.owner, 46, 41);
    army.owner_is_human = true;
    army.troops = core::array::from_fn(|t| a.troops[t]);
    army.men = a.men;
    let id = units.spawn(army).unwrap();

    assert!(
        !l2_kingdom::conquest::can_be_entered(&counties, &units, BESIEGED_COUNTY as u8, a.owner),
        "a castle and a garrison: this is a siege"
    );

    siege::begin_siege(
        &l2_kingdom::tables::Tables::DEFAULT,
        &counties,
        &realms,
        &mut units,
        id,
        BESIEGED_COUNTY as u8,
        1,
    )
    .expect("the guard passes");
    assert!(siege::garrison_is_besieged(&counties, &units, BESIEGED_COUNTY as u8));

    assert_eq!(siege::assault_castle_level(&counties[BESIEGED_COUNTY as usize]), 0);
    assert!(siege::can_assault(0, 0), "a palisade can be stormed bare-handed");

    siege::order_engine(&mut units, id, Engine::Catapult, 1);
    let mut seasons = 0;
    let ready = loop {
        seasons += 1;
        assert!(seasons < 20, "the build must terminate");
        let mut cursor = siege::start_phase(&counties, &mut units);
        if let Some(u) = siege::tick_phase(&mut cursor, &mut units) {
            break u;
        }
    };
    assert_eq!(ready, id);
    assert_eq!(seasons, 5, "five seasons, which is what the player saw");

    match siege::assault(&counties, &mut units, ready) {
        siege::Assault::Battle { attacker, defender, castle_level } => {
            assert_eq!((attacker, defender, castle_level), (id, g, 0));
        }
        other => panic!("expected a battle, got {other:?}"),
    }
}

