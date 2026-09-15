#![allow(unused_imports)]
use super::*;
use super::partition::*;
use super::secession::*;
use crate::county::County;
use crate::realm::MAX_REALMS;

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(n: usize, owner: u8) -> Vec<County> {
        let mut counties: Vec<County> = (0..=n).map(|_| County::new()).collect();
        for id in 1..=n {
            counties[id].owner = owner;
            counties[id].population = 100;
            if id > 1 {
                counties[id].add_neighbour(id as u8 - 1);
                counties[id - 1].add_neighbour(id as u8);
            }
        }
        counties
    }

    #[test]
    fn a_connected_realm_is_one_block_and_loses_nothing() {
        let counties = chain(5, 1);
        let blocks = build_blocks(&counties, 5);
        assert_eq!(blocks.count(), 1);
        assert_eq!(blocks.0[0].len(), 5);
        assert_eq!(blocks.0[0].population, 500, "the block's key is the population sum");
        let strength = [0u8, 3, 0, 0, 0, 0];
        let cut = minor_blocks(&blocks, &strength);
        assert_eq!(cut[0].blocks, 1);
        assert!(cut[0].counties.is_empty());
    }

    #[test]
    fn cutting_the_bridge_county_costs_the_realm_the_far_half() {
        let mut counties = chain(5, 1);
        counties[3].owner = 2; // an enemy takes the middle
        let blocks = build_blocks(&counties, 5);
        assert_eq!(blocks.count(), 3, "1-2, 4-5, and the captured 3");
        let cut = minor_blocks(&blocks, &[0, 3, 3, 0, 0, 0]);
        let realm1 = cut.iter().find(|s| s.realm == 1).unwrap();
        assert_eq!(realm1.blocks, 2);
        assert_eq!(realm1.counties.len(), 2);
    }

    #[test]
    fn an_equal_population_hands_the_realm_the_higher_numbered_block() {
        let mut blocks = Blocks::empty();
        blocks.0[0] = Block { population: 400, owner: 1, members: { let mut m = [0; MAX_BLOCK_MEMBERS]; m[0] = 1; m } };
        blocks.0[1] = Block { population: 400, owner: 1, members: { let mut m = [0; MAX_BLOCK_MEMBERS]; m[0] = 2; m } };
        let cut = minor_blocks(&blocks, &[0, 3, 0, 0, 0, 0]);
        assert_eq!(cut[0].kept, 1, "the later of two equal blocks");
        assert_eq!(cut[0].counties, vec![1]);
    }

    #[test]
    fn the_most_populous_block_is_kept_not_the_one_with_most_counties() {
        let mut counties: Vec<County> = (0..=3).map(|_| County::new()).collect();
        for c in counties.iter_mut().take(4).skip(1) {
            c.owner = 1;
        }
        counties[1].population = 900; // alone
        counties[2].population = 100; // 2 - 3, adjacent
        counties[3].population = 100;
        counties[2].add_neighbour(3);
        counties[3].add_neighbour(2);
        let blocks = build_blocks(&counties, 3);
        assert_eq!(blocks.count(), 2);
        let cut = minor_blocks(&blocks, &[0, 3, 0, 0, 0, 0]);
        assert_eq!(cut[0].counties, vec![2, 3], "two counties lost to one bigger one");
    }

    #[test]
    fn an_eliminated_realm_and_the_neutral_counties_are_both_left_alone() {
        let mut counties = chain(4, 0);
        counties[1].owner = 2;
        counties[3].owner = 2; // realm 2 is split, but eliminated
        let blocks = build_blocks(&counties, 4);
        assert_eq!(blocks.count(), 2);
        assert!(minor_blocks(&blocks, &[0; MAX_REALMS]).is_empty(), "strength 0 everywhere");
        let cut = minor_blocks(&blocks, &[0, 0, 4, 0, 0, 0]);
        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].realm, 2);
        assert_eq!(cut[0].counties.len(), 1);
    }

    #[test]
    fn a_bridge_discovered_last_still_ends_as_one_block() {
        let mut counties: Vec<County> = (0..=3).map(|_| County::new()).collect();
        for c in counties.iter_mut().take(4).skip(1) {
            c.owner = 1;
            c.population = 10;
        }
        counties[1].add_neighbour(2);
        counties[2].add_neighbour(1);
        counties[2].add_neighbour(3);
        counties[3].add_neighbour(2);
        let blocks = build_blocks(&counties, 3);
        assert_eq!(blocks.count(), 1, "one block, whatever order they were offered in");
        assert_eq!(blocks.0[0].len(), 3);
    }

    #[test]
    fn contiguity_is_the_neighbour_list_and_nothing_else() {
        let mut counties: Vec<County> = (0..=2).map(|_| County::new()).collect();
        counties[1].owner = 1;
        counties[2].owner = 1;
        let blocks = build_blocks(&counties, 2);
        assert_eq!(blocks.count(), 2, "no neighbour entry, no contiguity");
    }

    #[test]
    fn the_partition_is_a_partition() {
        let mut counties = chain(10, 1);
        for id in [4usize, 7] {
            counties[id].owner = 2;
        }
        counties[9].owner = 0;
        let blocks = build_blocks(&counties, 10);
        let mut seen: Vec<u8> = Vec::new();
        for block in blocks.0.iter().filter(|b| b.owner != 0) {
            for id in block.members() {
                assert_eq!(counties[id as usize].owner, block.owner, "no mixed block");
                assert!(!seen.contains(&id), "county {id} is in two blocks");
                seen.push(id);
            }
        }
        seen.sort_unstable();
        let owned: Vec<u8> =
            (1..=10u8).filter(|&id| counties[id as usize].owner != 0).collect();
        assert_eq!(seen, owned, "every owned county, exactly once");
    }
}

