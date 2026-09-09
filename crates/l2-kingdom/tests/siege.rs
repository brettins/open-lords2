//! **The siege model against the bytes of a real siege.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" LORDS2_DIR="F:\games\Lords of the Realm II" \
//!   cargo test -p l2-kingdom --test siege
//! ```
//!
//! # What these five files are
//!
//! One siege, caught at five moments. `docs/armies.md` §4 was written entirely
//! from the decompiler because **there was no save with a castle under siege**;
//! there is now, and every number in this file comes out of it rather than out
//! of the document.
//!
//! | file | what it holds |
//! |---|---|
//! | `siege-safeturn.sav` | the catapult 43 % built, 3 seasons to go |
//! | `siege-old_turn.sav` | 64 %, 2 seasons |
//! | `siege-lastturn.sav` | 86 %, 1 season |
//! | `siege-sieging.sav` | 100 %, 0 seasons, **and the battle already staged** |
//! | `siege-aftersie.sav` | the assault resolved: the besieger gone, the garrison at 133 |
//!
//! The position: county 4 holds a **palisade** (`castleType` 1) garrisoned by
//! an AI army of **149**, and a human army of **43** is camped beside it
//! building one catapult.
//!
//! # The four things this settles that code alone could not
//!
//! 1. **The build model.** `work_done` climbs by exactly the besieger's 43 men
//!    a season — 86, 129, 172, 200 — its percentage is `work * 100 / 200`
//!    truncated, and `+0x19C` is `ceil(remaining / 43)` at every step. So
//!    `g_siegeEngineWork[0]` really is **200** and the catapult really is
//!    record 0.
//! 2. **`+0x198` and `+0x199` are different fields.** The besieger carries
//!    `garrison_county = 0` and `besieging_county = 4`; the garrison carries
//!    `garrison_county = 4` and `besieged_by = 5`. Nothing here was ever
//!    checked against a position where the two could be told apart.
//! 3. **`Army_PrepareForBattle`.** `siege-sieging.sav` was taken with the
//!    battle staged: the besieger's troop slot **7** holds the one catapult it
//!    built, and the garrison's slot **10** holds **one** oil — which is
//!    `OIL_BY_CASTLE_LEVEL[0]` for a palisade, the first arm of a five-way
//!    switch nobody had ever seen fire.
//! 4. **The castle bonus, uniquely.** The autocalc reproduces the aftermath to
//!    the man *only* at level 0. See
//!    [`the_castle_bonus_for_a_palisade_is_the_only_one_that_reproduces_the_aftermath`].

use l2_formats::save::Save;
use l2_kingdom::battle::{self, CASTLE_STRENGTH_PERCENT};
use l2_kingdom::siege::{self, Engine, ENGINE_WORK};
use l2_kingdom::unit::{Unit, UnitKind, Units, TROOP_TYPES};

/// `g_units`, `0x0052F0B0`, stride `0x1A4` — `docs/armies.md` §1.
const UNIT_BASE: u32 = 0x0052_F0B0;
const UNIT_STRIDE: u32 = 0x1A4;

/// The besieging army and the garrison, by slot, in every one of the five
/// files. Named rather than searched for: a fixture is a file name plus a
/// fingerprint (`docs/environment.md`), and these two slots are part of the
/// fingerprint.
const BESIEGER: u32 = 5;
const GARRISON: u32 = 4;
const BESIEGED_COUNTY: u32 = 4;

fn w16(save: &Save, at: u32) -> i32 {
    let lo = save.u8_at(at).unwrap_or(0) as i32;
    let hi = save.u8_at(at + 1).unwrap_or(0) as i32;
    lo | (hi << 8)
}

/// One unit slot, as the fields a siege reads.
struct SavedUnit {
    owner: u8,
    kind: u8,
    men: i32,
    garrison_county: u8,
    besieging_county: u8,
    besieged_by: u8,
    seasons_left: u8,
    /// `+0x182 + e*6`, as `(ordered, percent, work_done)`.
    engines: [(i32, i32, i32); 3],
    /// All **eleven** `+0x16C` counts — the four battle-only slots included,
    /// which is the point of reading them here.
    troops: [i32; 11],
}

fn unit(save: &Save, slot: u32) -> SavedUnit {
    let b = UNIT_BASE + slot * UNIT_STRIDE;
    SavedUnit {
        owner: save.u8_at(b).unwrap(),
        kind: save.u8_at(b + 0x08).unwrap(),
        men: save.i32_at(b + 0x168).unwrap(),
        garrison_county: save.u8_at(b + 0x198).unwrap(),
        besieging_county: save.u8_at(b + 0x199).unwrap(),
        besieged_by: save.u8_at(b + 0x19A).unwrap(),
        seasons_left: save.u8_at(b + 0x19C).unwrap(),
        engines: core::array::from_fn(|e| {
            let r = b + 0x182 + e as u32 * 6;
            (w16(save, r), w16(save, r + 2), w16(save, r + 4))
        }),
        troops: core::array::from_fn(|t| w16(save, b + 0x16C + t as u32 * 2)),
    }
}

fn castle_type(save: &Save) -> u8 {
    let cb = l2_formats::save::COUNTY_BASE
        + BESIEGED_COUNTY * l2_formats::save::COUNTY_STRIDE as u32;
    save.u8_at(cb + 0x1C0).unwrap()
}

fn garrison_slot(save: &Save) -> i32 {
    let cb = l2_formats::save::COUNTY_BASE
        + BESIEGED_COUNTY * l2_formats::save::COUNTY_STRIDE as u32;
    save.i32_at(cb + 0x1BC).unwrap()
}

/// **The build model, step by step, across four snapshots.**
///
/// The four are the same siege on four consecutive turns, and every number in
/// [`siege::build_tick`] and [`siege::recompute_build_time`] has to be right
/// for all twelve of the stored values to come out.
#[test]
fn one_catapult_and_forty_three_men_reproduce_every_snapshot_of_the_build() {
    // Turn order, earliest first, with what the record should hold.
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

        // Record 0 is the catapult, and it is the only one ordered.
        assert_eq!(a.engines[0].0, 1, "{name}: one catapult ordered");
        assert_eq!((a.engines[1].0, a.engines[2].0), (0, 0), "{name}: nothing else");

        assert_eq!(a.engines[0].2, work, "{name}: man-seasons of work done");
        assert_eq!(a.engines[0].1, percent, "{name}: percent complete");
        assert_eq!(a.seasons_left, seasons, "{name}: seasons the screen prints");

        // The three arithmetic claims, each stated against the stored bytes
        // rather than against each other.
        assert_eq!(total, 200, "a catapult costs 200 man-seasons");
        // A season adds the army's men — capped at what the record still
        // needs, which is why the last step is 28 and not 43. Work is never
        // banked past the engine it was building.
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

/// And the same four snapshots driven through **our** [`siege::build_tick`],
/// from an order placed at zero, must land on the same four records.
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

    // Ordering it says five seasons — which is the number the player reported
    // seeing on the screen, arrived at from the other end.
    assert_eq!(units.get(id).unwrap().siege_seasons_left, 5);

    // Five seasons, and the middle four are the four saved snapshots.
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

/// **`Army_PrepareForBattle`, caught in the act.**
///
/// `siege-sieging.sav` was saved with the battle already staged — the
/// *"Will you take the field?"* prompt up — so the four battle-only troop slots
/// are filled, and they are the only place in any fixture where they ever are.
#[test]
fn the_staged_battle_carries_one_catapult_and_one_pot_of_oil() {
    let save: Save = l2_testkit::fixture!("siege-sieging.sav");
    let a = unit(&save, BESIEGER);
    let d = unit(&save, GARRISON);

    assert_eq!(castle_type(&save), 1, "a wooden palisade");
    let level = castle_type(&save) - 1;
    assert_eq!(level, 0);

    // The besieger's engines became troops 7, 8, 9.
    let staged = siege::prepare_besieger(&{
        let mut u = Unit::new(UnitKind::Army, 1, 0, 0);
        for (e, record) in u.engines.iter_mut().enumerate() {
            record.ordered = a.engines[e].0 as i16;
        }
        u
    });
    assert_eq!(staged.counts()[..3], [a.troops[7], a.troops[8], a.troops[9]]);
    assert_eq!(a.troops[7], 1, "the one catapult it built");

    // The garrison's oil is the castle level's row, and only the garrison has
    // any. `OIL_BY_CASTLE_LEVEL[0]` is 1, and this is the first time that arm
    // has been seen fire.
    assert_eq!(d.troops[10], siege::prepare_garrison(level).oil);
    assert_eq!(d.troops[10], 1);
    assert_eq!(a.troops[10], 0, "an attacker never gets oil");
}

/// **The castle bonus, pinned by the aftermath, and only at level 0.**
///
/// 43 attackers against a 149-man garrison in a palisade. The player declined
/// the prompt, so `Battle_Decline` ran the autocalc; `siege-aftersie.sav` holds
/// what it left behind. Run [`battle::auto_resolve`] on the two records and the
/// five surviving counts have to match — and they do at level 0 and at no other
/// level, which is what makes this a measurement of
/// `CASTLE_STRENGTH_PERCENT[0]` rather than a check that the arithmetic runs.
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

/// The whole campaign chain, from the guard to the assault, run over the
/// fixture's own position.
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

    // The county cannot be walked into — that is what forces the siege, and
    // the game's own Readme says it in English under *Capturing Counties*.
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

    // A palisade is level 0, so the assault needs no engines at all — but the
    // player built one anyway, and the game let him.
    assert_eq!(siege::assault_castle_level(&counties[BESIEGED_COUNTY as usize]), 0);
    assert!(siege::can_assault(0, 0), "a palisade can be stormed bare-handed");

    siege::order_engine(&mut units, id, Engine::Catapult, 1);
    // One turn phase 2 a season: seed the cursor, pump it, and see whether it
    // yielded an army whose engines came in.
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
