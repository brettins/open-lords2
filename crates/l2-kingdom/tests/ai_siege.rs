//! **An AI lord besieges a castle and orders its engines**, travelled from the
//! step onto the castle tile rather than by setting a field.
//!
//! ```text
//! cargo test -p l2-kingdom --test ai_siege
//! ```
//!
//! Needs no game install and no fixture.
//!
//! # Why this file exists
//!
//! The hand-off that opened this branch said: *"No AI in this workspace orders
//! siege engines — `siege::order_engine` has three callers and all three are
//! the player's, so an AI besieging a level-3 castle can never assault at
//! all."*
//!
//! **The premise is true and the conclusion is false, and the gap between them
//! is worth more than the feature.** `order_engine` is the *siege screen's `+`
//! and `-` buttons* (`0x0043B681` / `0x0043B741`), and it is right that no AI
//! touches it — the original's AI does not press buttons either. The AI's
//! ordering path is a different function, and it is four calls above:
//!
//! ```text
//! Unit_ReachCastleBuilding  0x004686A0   an army walks onto a castle tile
//!   Army_BeginSiege         0x004A7CA2   four guards, no else
//!     Siege_Link            0x004A7E0A   the two back-pointers
//!       Siege_Prepare       0x004A7EB5   clear the records, and for an AI, order
//! ```
//!
//! All four are implemented, and `Siege_Prepare` has been correct — including
//! the `+0xA0` doctrine byte — since it was written. What was missing was
//! **anything that travelled the road**: every siege test in the workspace
//! staged its besieger by hand and therefore had to order the engines by hand
//! too, which is `docs/agents.md`'s *"a field is only tested if something a
//! test reads was written by something the game runs"*, exactly.
//!
//! # What the AI orders, and it is per lord rather than per castle
//!
//! `Siege_Prepare`'s only input beyond the castle is `g_aiPersonality[lord-1]
//! +0xA0`, read out of `Lords2.exe` as **8, 9, 7, 7** for the four lords —
//! three distinct values, and exactly the three the function tests. Two towers
//! always; four for doctrine 8; a ram for 9; three catapults for 7, plus a ram
//! against a stone or royal castle after season 2.

use l2_kingdom::conquest::{self, CastleArrival};
use l2_kingdom::county::County;
use l2_kingdom::map::CampaignMap;
use l2_kingdom::realm::Realm;
use l2_kingdom::siege::{self, Engine};
use l2_kingdom::tables::Tables;
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::{MAX_COUNTIES, MAX_REALMS};

const CASTLE_COUNTY: u8 = 3;

/// Realm 1 is a human's, realm 2 is an AI lord's; county 3 holds realm 1's
/// castle of `castle_type` with a garrison in it, and realm 2 has an army
/// standing beside it.
fn about_to_besiege(
    castle_type: u8,
    lord: u8,
) -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units, usize) {
    let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
    let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
    realms[1].in_play = true;
    realms[1].is_human = true;
    realms[2].in_play = true;
    realms[2].lord = lord;

    let mut units = Units::new();
    let mut garrison = Unit::new(UnitKind::Army, 1, 40, 40);
    garrison.men = 200;
    garrison.garrison_county = CASTLE_COUNTY;
    let g = units.spawn(garrison).unwrap();

    counties[CASTLE_COUNTY as usize].owner = 1;
    counties[CASTLE_COUNTY as usize].castle_type = castle_type;
    counties[CASTLE_COUNTY as usize].garrison_unit = g;

    // The AI's army, on the tile next door with its men counted.
    let mut army = Unit::new(UnitKind::Army, 2, 41, 40);
    army.owner_is_human = false;
    army.men = 400;
    let a = units.spawn(army).unwrap();
    (counties, realms, units, a)
}

/// **The AI walks onto the castle and its engines are ordered**, by the same
/// chain a real turn runs.
#[test]
fn an_ai_army_that_reaches_a_castle_lays_siege_and_orders_its_engines() {
    // (lord, personality +0xA0, then catapults / towers / rams)
    for (lord, doctrine, want) in [
        (1u8, 8i32, (0i16, 4i16, 0i16)),
        (2, 9, (0, 2, 1)),
        (3, 7, (3, 2, 0)),
        (4, 7, (3, 2, 0)),
    ] {
        let (mut counties, realms, mut units, army) = about_to_besiege(2, lord);
        assert_eq!(
            siege::siege_doctrine(&Tables::DEFAULT, lord),
            Some(doctrine),
            "the personality byte for lord {lord}"
        );

        let arrival = conquest::reach_castle_building(
            &Tables::DEFAULT,
            &CampaignMap::empty(),
            &mut counties,
            &realms,
            &mut units,
            army,
            CASTLE_COUNTY,
            1,
        );
        assert!(matches!(arrival, CastleArrival::Siege(Ok(()))), "lord {lord}: {arrival:?}");

        let u = units.get(army).unwrap();
        assert_eq!(u.besieging_county, CASTLE_COUNTY, "lord {lord} is besieging");
        let got = (
            u.engines[Engine::Catapult.index()].ordered,
            u.engines[Engine::SiegeTower.index()].ordered,
            u.engines[Engine::BatteringRam.index()].ordered,
        );
        assert_eq!(got, want, "lord {lord}, doctrine {doctrine}");
        assert!(
            u.siege_seasons_left > 0,
            "lord {lord} has a build to wait out, not an assault it can launch today"
        );
    }
}

/// **A besieging AI builds its way to an assault on a stone castle**, which is
/// the question the hand-off actually asked. A level-3 castle refuses an
/// assault with no engines; this one arrives with two towers and three
/// catapults because nobody pressed a button.
#[test]
fn an_ai_besieging_a_stone_castle_builds_its_way_to_an_assault() {
    let (mut counties, realms, mut units, army) = about_to_besiege(4, 3);
    let arrival = conquest::reach_castle_building(
        &Tables::DEFAULT,
        &CampaignMap::empty(),
        &mut counties,
        &realms,
        &mut units,
        army,
        CASTLE_COUNTY,
        3,
    );
    assert!(matches!(arrival, CastleArrival::Siege(Ok(()))));

    let level = siege::assault_castle_level(&counties[CASTLE_COUNTY as usize]);
    assert!(level >= 3, "a stone castle");
    assert!(
        !siege::can_assault(level, 0),
        "and with nothing built the assault is refused — which is the position an \
         AI that never ordered anything would be stuck in for ever"
    );

    let ordered: i32 =
        units.get(army).unwrap().engines.iter().map(|e| e.ordered as i32).sum();
    assert!(ordered > 0, "the AI ordered engines at the moment it laid the siege");

    // Turn phase 2, a season at a time, exactly as `Siege_TickPhase` pumps it.
    let mut seasons = 0;
    let ready = loop {
        seasons += 1;
        assert!(seasons < 60, "the build must terminate: {ordered} engines over 400 men");
        let mut cursor = siege::start_phase(&counties, &mut units);
        if let Some(u) = siege::tick_phase(&mut cursor, &mut units) {
            break u;
        }
    };
    assert_eq!(ready, army);
    let built: i32 =
        units.get(army).unwrap().engines.iter().map(|e| e.ordered as i32).sum();
    assert!(
        siege::can_assault(level, built),
        "with {built} engines a stone castle can be assaulted"
    );
}
