//! `tools/oracle/tables.ps1` *does* read these tables out of the executable —
//! and prints them to a console, where nothing compares them to anything. The
//! whole apparatus existed and was not wired to the suite. `docs/decisions.md`
//! C10, C12, C17 and C20 are four instances of one mechanism: **nothing
//! re-checks.** This is a re-check.
//!
//! `Lords2.exe` has no ASLR and a fixed image base of `0x400000`, so a virtual
//! address is a constant and the bytes behind it come straight off disk — no
//! process, no window, nothing a screen lock can spoil (C16).
//!
//! | table | address | shape |
//! |---|---|---|
//! | `g_troopBattleStats` | `0x004D96D0` | 11 x 5 `i32` |
//! | `g_formationTypePriority` | `0x004D98C8` | 11 `i32` |
//! | `g_missileStats` | `0x004D97B0` | 4 x 5 `i32` |
//!
//! Width matters and is easy to get wrong; `tools/oracle/tables.ps1`'s own
//! comment records reading `g_meleeAttackTable` as `u32` and getting 262149,
//! which is `0x00040005` — two small numbers in a trenchcoat.

use l2_sim::formation::{FOOTPRINT, MAX_FIGURES_PER_UNIT, ROW_MAX, TYPE_PRIORITY, WEAPON_CLASS};
use l2_sim::missile::WeaponClass;
use l2_sim::troop::{Troop, ALL_TROOPS};
use l2_testkit::pe::Table;

const TROOP_BATTLE_STATS: u32 = 0x004D_96D0;
const FORMATION_TYPE_PRIORITY: u32 = 0x004D_98C8;
const MISSILE_STATS: u32 = 0x004D_97B0;

#[test]
fn the_four_unit_geometry_tables_are_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();
    let t = Table::at(&exe, TROOP_BATTLE_STATS);
    const COLS: usize = 5;

    let mut compared = 0;
    for troop in ALL_TROOPS {
        let row = troop.index();
        let read = |col: usize| t.i32_at(row * COLS + col);

        assert_eq!(MAX_FIGURES_PER_UNIT[row] as i32, read(0), "{troop:?} maxFigures");
        assert_eq!(FOOTPRINT[row], read(1), "{troop:?} footprint");
        assert_eq!(ROW_MAX[row], read(2), "{troop:?} rowMax");
        assert_eq!(WEAPON_CLASS[row] as i32, read(3), "{troop:?} weaponClass");
        assert_eq!(l2_sim::move_delay(troop) as i32, read(4), "{troop:?} moveDelay");
        compared += 5;
    }
    assert_eq!(compared, 55, "eleven troop types, five columns");

    assert_eq!(t.i32_at(COLS + 3), 2, "row 1 carries a crossbow");
    assert_eq!(t.i32_at(5 * COLS + 3), 1, "row 5 carries a bow");
    assert_eq!(t.i32_at(7 * COLS + 3), 3, "row 7 is the catapult");
    assert_eq!(Troop::Crossbowmen.index(), 1);
    assert_eq!(Troop::Archers.index(), 5);
    assert_eq!(Troop::Catapults.index(), 7);

    eprintln!("g_troopBattleStats: {compared} values match the executable");
}

#[test]
fn the_formation_priority_table_is_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();
    let t = Table::at(&exe, FORMATION_TYPE_PRIORITY);
    let read = t.i32s(11);
    assert_eq!(read, TYPE_PRIORITY.to_vec(), "g_formationTypePriority");

    let mut sorted = read.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 11, "two troop types share a priority");
    eprintln!("g_formationTypePriority: 11 values match the executable");
}

#[test]
fn the_missile_stats_are_the_bytes_in_the_executable() {
    let exe = l2_testkit::executable!();
    let t = Table::at(&exe, MISSILE_STATS);
    const COLS: usize = 5;

    for class in [WeaponClass::Bow, WeaponClass::Crossbow, WeaponClass::Catapult] {
        let row = class as usize;
        let ours = class.stats();
        assert_eq!(ours.range as i32 * 8, t.i32_at(row * COLS), "{class:?} range, in eighths");
        assert_eq!(ours.reload as i32, t.i32_at(row * COLS + 1), "{class:?} reload");
        assert_eq!(ours.damage as i32, t.i32_at(row * COLS + 3), "{class:?} damage");
    }

    for troop in ALL_TROOPS {
        let has_weapon = WEAPON_CLASS[troop.index()] != 0;
        assert_eq!(
            WeaponClass::for_troop(troop).is_some(),
            has_weapon,
            "{troop:?} disagrees with its weaponClass column"
        );
    }
    eprintln!("g_missileStats: bow, crossbow and catapult match the executable");
}

#[test]
fn the_addresses_are_specific_and_not_a_field_of_zeros() {
    let exe = l2_testkit::executable!();
    let stats = Table::at(&exe, TROOP_BATTLE_STATS);
    let rows: Vec<Vec<i32>> = (0..11).map(|r| (0..5).map(|c| stats.i32_at(r * 5 + c)).collect()).collect();

    assert!(rows.iter().any(|r| r != &rows[0]), "every row read the same");
    assert!(rows.iter().flatten().any(|&v| v != 0), "the whole table read zero");

    let shifted = Table::at(&exe, TROOP_BATTLE_STATS - 4);
    let shifted_first: Vec<i32> = shifted.i32s(5);
    assert_ne!(shifted_first, rows[0], "the table has no distinguishing alignment");
}
