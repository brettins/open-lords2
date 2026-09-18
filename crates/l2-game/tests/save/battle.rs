#![allow(unused_imports)]
use super::*;
use super::roundtrip::*;

use l2_formats::save::Save;
use l2_game::save;
use l2_game::Game;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM};
use l2_kingdom::tables::Tables;
use l2_kingdom::unit::{Unit, UnitKind, TROOP_TYPES};
use l2_net::{Canonical, Encode};

/// `g_units`, `0x0052F0B0`, stride `0x1A4`, slot 5 — `docs/armies.md` §1, the
/// same record `crates/l2-game/tests/seam/battle.rs` reads.
const UNIT_BASE: u32 = 0x0052_F0B0;
const UNIT_STRIDE: u32 = 0x1A4;
const ATTACKER_SLOT: u32 = 5;
const COUNTY: u8 = 3;

fn troops_of(save: &Save, slot: u32) -> [i32; TROOP_TYPES] {
    let base = UNIT_BASE + slot * UNIT_STRIDE;
    let mut troops = [0i32; TROOP_TYPES];
    for (t, out) in troops.iter_mut().enumerate() {
        *out = save.u16_at(base + 0x16C + t as u32 * 2).expect("a troop count") as i32;
    }
    troops
}

/// The position `battle-before.sav` holds, taken to the field instead of
/// autocalculated — `take_the_field` (`FUN_0043B593` → `Battle_Start`
/// `0x004778A0`).
fn a_fight_from_the_fixture(save: &Save) -> Game {
    let c = save.county(COUNTY as usize).expect("county 3");
    let troops = troops_of(save, ATTACKER_SLOT);

    let mut game = Game::new(0x0BAD_F00D);
    let k = &mut game.kingdom;
    assert!(k.set_county_count(5));
    let mut map = CampaignMap::empty();
    for i in 0..MAP_DIM * MAP_DIM {
        map.county[i] = COUNTY;
        map.flags[i] = flags::ROAD;
    }
    k.campaign.map = map;
    k.counties[COUNTY as usize].owner = 0;
    k.counties[COUNTY as usize].population = c.population;
    k.counties[COUNTY as usize].anchor_x = c.anchor_x;
    k.counties[COUNTY as usize].anchor_y = c.anchor_y;
    for realm in 1..=2usize {
        k.realms[realm].in_play = true;
    }
    k.realms[1].is_human = true;

    // **Lopsided on purpose.** `Battle_UpdateStrengthAdvantage` (`0x0047FC01`)
    // fires every 101st frame and the jitter spans 31, so two equal armies leave
    // `g_aiStrengthAdvantage` at zero and a test on them cannot tell a dropped
    // AI field from a restored one - `docs/decisions.md` C221.
    let spawn = |k: &mut Kingdom, owner: u8, human: bool, share: i32| {
        let mut u = Unit::new(UnitKind::Army, owner, c.anchor_x, c.anchor_y);
        u.owner_is_human = human;
        u.county = COUNTY;
        u.troops = troops.map(|t| t / share);
        u.men = u.troops.iter().sum();
        k.campaign.units.spawn(u).expect("a unit slot")
    };
    let attacker = spawn(k, 1, true, 1);
    let defender = spawn(k, 2, false, 3);

    let runner = l2_game::engagement::begin_fight(k, attacker, defender, None, 0x004D_9870)
        .expect("two armies make a battle");
    game.player = 1;
    game.battle = Some(Box::new(l2_game::battlefield::LiveBattle::new(
        runner, attacker, defender, COUNTY, None, 1, 1,
    )));
    game
}

fn step(game: &mut Game, ticks: u32) {
    let live = game.battle.as_deref_mut().expect("a live battle");
    for _ in 0..ticks {
        live.runner.step();
    }
}

fn digest(game: &Game) -> u64 {
    let mut c = Canonical::hashing();
    game.battle.as_deref().expect("a live battle").runner.encode(&mut c);
    c.finish().hash
}

/// **The digest is the encoder** (`docs/decisions.md` C65), so a field the save
/// drops is a field the lockstep hash cannot see either: run the same ticks on
/// the saved game and on the reloaded one and the two must agree tick for tick.
#[test]
fn a_battle_saved_and_reloaded_runs_the_same_hundred_ticks() {
    let before: Save = l2_testkit::fixture!("battle-before.sav");
    let mut kept = a_fight_from_the_fixture(&before);
    step(&mut kept, 150);
    assert_ne!(
        kept.battle.as_deref().expect("a battle").runner.ai.strength_advantage,
        0,
        "the AI never weighed the sides, so this fixture cannot tell its state apart"
    );

    let bytes = save::encode(&kept);
    let mut back = save::decode(&bytes, Tables::DEFAULT).expect("our own bytes");
    assert_eq!(digest(&back), digest(&kept), "the battle did not survive the round trip");

    for n in 1..=100u32 {
        step(&mut kept, 1);
        step(&mut back, 1);
        assert_eq!(digest(&back), digest(&kept), "the two timelines split at tick {n}");
    }
    assert_eq!(kept.battle.as_deref().expect("a battle").runner.tick, 250);
}

/// A save made on the campaign map and one made with the battle screen open
/// differ in exactly the blocks that hold the battle.
#[test]
fn the_originals_block_table_carries_the_battle_only_when_one_is_running() {
    let on_map: Save = l2_testkit::fixture!("battle-before.sav");
    let in_battle: Save = l2_testkit::fixture!("noorders-start.sav");

    // The four block lengths are the arithmetic that pins every stride this
    // work relies on: `g_battlefield` 80 × 80 × 8, `g_battleMen` 81 × `0x1B0`,
    // the missiles 101 × `0x4C`, `g_battleUnits` 81 × `0x34`.
    let len_of = |va: u32| {
        on_map
            .layout()
            .blocks()
            .iter()
            .find(|b| b.va == va)
            .unwrap_or_else(|| panic!("no save block at {va:#010x}"))
            .len
    };
    assert_eq!(len_of(0x0054_40E0), 80 * 80 * 8, "g_battlefield");
    assert_eq!(len_of(0x0055_4480), 81 * 0x1B0, "g_battleMen");
    assert_eq!(len_of(0x0057_A100), 101 * 0x4C, "the missiles");
    assert_eq!(len_of(0x0056_6520), 81 * 0x34, "g_battleUnits");

    let any_set = |s: &Save, va: u32, len: u32| (0..len).any(|i| s.u8_at(va + i) != Ok(0));
    for (va, len, what) in [
        (0x0054_40E0u32, 80 * 80 * 8u32, "g_battlefield"),
        (0x0055_4480, 81 * 0x1B0, "g_battleMen"),
        (0x0056_6520, 81 * 0x34, "g_battleUnits"),
    ] {
        assert!(!any_set(&on_map, va, len), "{what} is set in a save made on the map");
        assert!(any_set(&in_battle, va, len), "{what} is clear in a save made in a battle");
    }

    // `g_battleArmyA` / `g_battleArmyB` — the two `g_units` indices the battle
    // is fought between, blocks 33 and 34.
    for va in [0x0056_8210u32, 0x0056_8218] {
        assert_eq!(on_map.i32_at(va), Ok(0), "{va:#010x} is set with no battle running");
        assert_ne!(in_battle.i32_at(va), Ok(0), "{va:#010x} is clear inside a battle");
    }
}
