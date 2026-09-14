#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use crate::figure::{Figure, Side, State};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};
    use crate::troop::Troop;

    fn one_unit(troop: Troop, men: u16, count: usize) -> (Units, Vec<Figure>, Vec<(u8, u8)>) {
        let mut units = Units::new();
        let cat = CATEGORY_OF_TROOP[troop.index()];
        let u = units.create(1, false, SIDE_A, cat).unwrap();
        let mut figs = Vec::new();
        let mut pos = Vec::new();
        for i in 0..count {
            let mut f = Figure::new(troop, SIDE_A, men);
            f.unit = u as u16;
            f.owner = 1;
            figs.push(f);
            pos.push((10 + i as u8, 20));
        }
        (units, figs, pos)
    }

    #[test]
    fn slot_zero_is_never_handed_out_so_zero_can_mean_nobody() {
        let mut units = Units::new();
        for _ in 0..MAX_UNITS {
            let i = units.create(1, false, SIDE_A, 2).unwrap();
            assert!((1..=MAX_UNITS).contains(&i));
        }
        assert_eq!(units.create(1, false, SIDE_A, 2), None, "the array does not grow");
        assert_eq!(units.owner_of(0), 0);
    }

    #[test]
    fn a_unit_stands_at_the_centre_of_its_figures_bounding_box() {
        let (mut units, mut figs, mut pos) = one_unit(Troop::Swordsmen, 4, 3);
        // Deliberately lopsided: the centroid and the box centre differ.
        pos[0] = (10, 10);
        pos[1] = (11, 10);
        pos[2] = (20, 30);
        units.rebuild_from_figures(&mut figs);
        units.recentre(1, &figs, &pos);
        assert_eq!((units.get(1).x, units.get(1).y), (15, 20), "box centre, not centroid");
    }

    #[test]
    fn an_unordered_unit_takes_its_own_position_as_its_destination() {
        let (mut units, mut figs, pos) = one_unit(Troop::Archers, 4, 2);
        units.rebuild_from_figures(&mut figs);
        assert_eq!((units.get(1).target_x, units.get(1).target_y), (0, 0));
        units.recentre(1, &figs, &pos);
        let u = units.get(1);
        assert_eq!((u.target_x, u.target_y), (u.x, u.y), "seeded from the position");
        // And it is seeded once: moving the unit does not drag the destination.
        let mut pos2 = pos.clone();
        pos2[0] = (40, 40);
        pos2[1] = (41, 40);
        units.recentre(1, &figs, &pos2);
        assert_eq!((units.get(1).target_x, units.get(1).target_y), (10, 20));
    }

    #[test]
    fn the_rebuild_frees_a_unit_whose_last_figure_died() {
        let (mut units, mut figs, _) = one_unit(Troop::Peasants, 1, 2);
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.get(1).figures, 2);
        assert_eq!(units.owner_of(1), 1);
        for f in &mut figs {
            f.take_hits(10_000);
        }
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.owner_of(1), 0, "a wiped-out unit reads as a free slot");
        assert_eq!(units.get(1).figures, 0);
    }

    /// The grudge is fifty **frames**, and the think interval is two hundred.
    /// Rounding it up to "until the next decision" is the mistake
    /// `docs/battle-ai.md` §3.2 warns about, so pin the real number.
    #[test]
    fn the_memory_of_an_attacker_expires_in_fifty_frames() {
        let mut units = Units::new();
        let victim = units.create(1, false, SIDE_A, 3).unwrap();
        let attacker = units.create(2, true, SIDE_B, 3).unwrap();
        let mut figs = vec![
            Figure::new(Troop::Swordsmen, SIDE_A, 4),
            Figure::new(Troop::Swordsmen, SIDE_B, 4),
        ];
        figs[0].unit = victim as u16;
        figs[0].owner = 1;
        figs[1].unit = attacker as u16;
        figs[1].owner = 2;

        figs[0].was_hit = true;
        figs[0].hit_by = Some(1);
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.get(victim).last_attacker, attacker as u16);
        assert_eq!(units.get(victim).hit_memory, HIT_MEMORY);
        assert_eq!(units.get(victim).times_hit, 1);
        assert!(!figs[0].was_hit, "the flag is consumed by the pass that reads it");

        for _ in 0..HIT_MEMORY {
            units.rebuild_from_figures(&mut figs);
        }
        assert_eq!(units.get(victim).hit_memory, 0, "fifty frames, not two hundred");
        // The attacker index survives its own expiry; the handlers gate on the
        // countdown, not on the index.
        assert_eq!(units.get(victim).last_attacker, attacker as u16);
        assert_eq!(units.get(victim).times_hit, 1, "and it only ages once the grudge has");
        units.rebuild_from_figures(&mut figs);
        assert_eq!(units.get(victim).times_hit, 0);
    }

    #[test]
    fn a_figure_in_melee_marks_its_whole_unit() {
        let (mut units, mut figs, _) = one_unit(Troop::Macemen, 4, 3);
        units.rebuild_from_figures(&mut figs);
        assert!(!units.get(1).in_melee);
        figs[2].state = State::Melee;
        units.rebuild_from_figures(&mut figs);
        assert!(units.get(1).in_melee);
        figs[2].state = State::Idle;
        units.rebuild_from_figures(&mut figs);
        assert!(!units.get(1).in_melee, "and it is cleared, not latched");
    }

    #[test]
    fn pct_of_answers_zero_rather_than_dividing_by_zero() {
        assert_eq!(pct_of(500, 250), 200);
        assert_eq!(pct_of(0, 250), 0);
        assert_eq!(pct_of(500, 0), 0, "an army with no enemy left");
    }

    #[test]
    fn the_category_ladder_matches_the_documented_eleven_way_assignment() {
        assert_eq!(CATEGORY_OF_TROOP[Troop::Peasants.index()], 2);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Crossbowmen.index()], 1);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Archers.index()], 1);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Macemen.index()], 3);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Swordsmen.index()], 3);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Pikemen.index()], 2);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Knights.index()], 4);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Catapults.index()], 5);
        assert_eq!(CATEGORY_OF_TROOP[Troop::SiegeTowers.index()], 6);
        assert_eq!(CATEGORY_OF_TROOP[Troop::BatteringRams.index()], 7);
        assert_eq!(CATEGORY_OF_TROOP[Troop::Oil.index()], 8);
    }
}

