#![allow(unused_imports)]
use super::*;
use super::births::*;
use super::super::*;

fn linked(happiness: [i32; 3], unowned: bool) -> Vec<County> {
    let mut counties = vec![County::new(); 4];
    for id in 1..=3 {
        counties[id].population = 1000;
        counties[id].happiness = happiness[id - 1];
        counties[id].owner = if unowned { 0 } else { 1 };
    }
    counties[1].add_neighbour(2);
    counties[2].add_neighbour(1);
    counties[2].add_neighbour(3);
    counties[3].add_neighbour(2);
    counties
}

#[test]
fn people_move_towards_the_happier_neighbour_and_the_cap_bites() {
    let mut c = linked([20, 90, 90], false);
    migrate_all(&mut c, 3, Q);
    assert_eq!(c[1].emigrants, MIGRATION_CAP);
    assert_eq!(c[1].emigrant_destination, 2);
    assert_eq!(c[2].immigrants, MIGRATION_CAP);
    assert_eq!(c[2].emigrants, 0, "nobody leaves the happiest county");
}

#[test]
fn a_perfectly_happy_county_never_loses_anyone() {
    assert_eq!(movers(10_000, 100, 100, false), 0);
    let mut c = linked([100, 100, 100], false);
    migrate_all(&mut c, 3, Q);
    for id in 1..=3 {
        assert_eq!(c[id].emigrants, 0);
    }
}

#[test]
fn an_unowned_county_sheds_people_at_half_the_rate() {
    let owned = movers(1000, 20, 90, false);
    let unowned = movers(1000, 20, 90, true);
    assert_eq!(unowned, owned / 2);
}

#[test]
fn nobody_moves_towards_an_unhappier_neighbour() {
    let mut c = linked([90, 20, 20], false);
    migrate_all(&mut c, 3, Q);
    assert_eq!(c[1].emigrants, 0, "the happiest county loses nobody");
    assert_eq!(c[3].emigrants, 0, "3's only neighbour is no happier than it is");
    assert_eq!(c[2].emigrants, MIGRATION_CAP, "2 empties towards 1");
    assert_eq!(c[2].emigrant_destination, 1);
    assert_eq!(c[1].immigrants, c[2].emigrants);
    assert_eq!(c[3].immigrants, 0);
}

#[test]
fn the_inflow_list_repeats_one_source_into_every_free_slot() {
    let mut c = linked([20, 90, 90], false);
    migrate_all(&mut c, 3, Q);
    assert_eq!(c[2].inflow_sources, [1u8; MAX_INFLOW_SOURCES]);
    assert_eq!(c[2].largest_inflow, MIGRATION_CAP);
    assert_eq!(c[2].largest_inflow_source, 1);
}

#[test]
fn migration_gives_the_same_answer_every_time() {
    let a = {
        let mut c = linked([10, 40, 90], false);
        migrate_all(&mut c, 3, Q);
        c
    };
    for _ in 0..8 {
        let mut b = linked([10, 40, 90], false);
        migrate_all(&mut b, 3, Q);
        assert_eq!(a, b);
    }
}

#[test]
fn migration_moves_nobody_until_the_population_pass_runs() {
    let mut c = linked([20, 90, 90], false);
    migrate_all(&mut c, 3, Q);
    assert_eq!(c[1].population, 1000, "still there");
    update_all(T, &mut c, 3, Season::Summer, Q);
    assert!(c[1].population < 1000 + c[1].births);
    assert_eq!(c[2].population, 1000 + c[2].births - c[2].deaths + MIGRATION_CAP);
}

