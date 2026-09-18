#![allow(unused_imports)]
use super::*;
use super::ignite_part::*;
use super::woodland::*;
use super::burn_part::*;
use crate::figure::{Figure, State};
use crate::missile::{Missile, Missiles, CLASS_FIRE, CLASS_OIL, SUB_CELL};
use crate::terrain::{Battlefield, DIM};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};
    use crate::runner::blank_field;
    use crate::Troop;

    #[test]
    fn a_frame_in_fire_costs_three_six_nine_or_twelve_hits_and_an_engine_less() {
        for (class, man, engine) in [(0u8, 3u16, 1u16), (1, 3, 1), (2, 6, 3), (3, 9, 5), (4, 12, 7), (8, 12, 7)] {
            let mut f = Figure::new(Troop::Peasants, SIDE_A, 4);
            burn(&mut f, class, false);
            assert_eq!(f.hits, man, "a man at class {class}");
            let mut h = Figure::new(Troop::Peasants, SIDE_A, 4);
            h.owner_is_human = true;
            burn(&mut h, class, false);
            assert_eq!(h.hits, man + 1, "a human's man at class {class}");
            let mut e = Figure::new(Troop::SiegeTowers, SIDE_B, 4);
            burn(&mut e, class, true);
            assert_eq!(e.hits, engine, "an engine at class {class}");
            e.owner_is_human = true;
            burn(&mut e, class, true);
            assert_eq!(e.hits, 2 * engine + 2, "a human's engine at class {class}");
        }
    }

    #[test]
    fn a_man_in_fire_dies_every_thirty_four_frames_and_the_figure_once() {
        let mut f = Figure::new(Troop::Archers, SIDE_B, 2);
        let mut died = 0;
        for frame in 1..=68 {
            if burn(&mut f, 0, false) {
                died += 1;
            }
            if frame == 33 {
                assert_eq!((f.men, f.hits), (2, 99));
            }
            if frame == 34 {
                assert_eq!((f.men, f.hits), (1, 2), "the remainder is carried");
            }
        }
        assert_eq!(f.men, 0);
        assert_eq!(f.state, State::Dead);
        assert_eq!(died, 1, "the death is reported once");
        assert!(!burn(&mut f, 0, false), "a corpse does not burn");
    }

    /// **Oil burns at a man's threshold**, because `+0x194` is set for 7, 8
    /// and 9 and not for 10 — although our troop table gives oil 160.
    #[test]
    fn a_pot_of_oil_burns_as_a_man_does_not_as_an_engine() {
        assert!(!is_engine(Troop::Oil));
        assert!(is_engine(Troop::Catapults) && is_engine(Troop::SiegeTowers) && is_engine(Troop::BatteringRams));
        assert_eq!(Troop::Oil.hits_per_casualty(), 160, "the table's number, which the burn ignores");
        let mut pot = Figure::new(Troop::Oil, SIDE_A, 4);
        for _ in 0..34 {
            burn(&mut pot, 0, is_engine(Troop::Oil));
        }
        assert_eq!(pot.men, 3, "100 hits, not 160");
    }

    #[test]
    fn a_fire_puts_back_the_ground_except_a_bridge_and_a_wood() {
        let mut field = blank_field();
        let mut ms = Missiles::new();
        let at = |x: usize, y: usize| y * DIM + x;
        field.cells[at(10, 10)].surface = 4;
        field.cells[at(11, 10)].surface = SURFACE_BRIDGE;
        field.cells[at(11, 10)].elevation = 2;
        field.cells[at(11, 10)].flags = 0x10;
        field.cells[at(11, 10)].gfx = 150;

        ignite(&mut field, &mut ms, 10, 10, 0x78);
        ignite(&mut field, &mut ms, 11, 10, 0);
        assert_eq!(field.cells[at(10, 10)].surface, SURFACE_BURNING);
        let bridge = field.cells[at(11, 10)];
        assert_eq!(
            (bridge.surface, bridge.flags, bridge.elevation, bridge.gfx),
            (SURFACE_BURNING, 0, 0, 0),
            "a bridge is flattened and cleared the moment it catches"
        );
        let (a, b) = (*ms.get(1), *ms.get(2));
        assert_eq!((a.class, a.ttl, a.saved_surface, a.owner), (CLASS_FIRE, 520, 4, 1));
        assert_eq!((b.ttl, b.saved_surface), (640, SURFACE_BRIDGE));
        assert_eq!((a.sub_steps, a.range_ticks), (0, 20_000), "a fire does not fly");

        put_out(&mut field, &a);
        put_out(&mut field, &b);
        assert_eq!(field.cells[at(10, 10)].surface, 4, "the rampart comes back");
        assert_eq!(field.cells[at(11, 10)].surface, SURFACE_BURNT_BRIDGE, "the bridge does not");

        let mut wood = blank_field();
        wood.cells[at(3, 4)].surface = SURFACE_WOODLAND;
        let cell = ignite_woodland(&mut wood, &mut ms, 3, 4).unwrap();
        let fire = *ms.iter().find(|(_, m)| m.cell_x == 3 && m.cell_y == 4).unwrap().1;
        assert_eq!(fire.ttl, 640 - 10 * 7, "0x280 - 10 * ((x + y) & 0x1F)");
        assert_eq!(wood.cells[cell].surface, SURFACE_WOOD_CATCHING);
        put_out(&mut wood, &fire);
        assert_eq!(wood.cells[cell].surface, 0, "a burnt wood is surface 0");
    }

    /// The durations are pinned as the literals `FUN_0048551D` produces: 520,
    /// then `0x280 − (0x78 − 7k)`. Ablation: drop the `c` accumulation and every
    /// spread cell reads 520.
    #[test]
    fn a_bridge_burns_five_cells_either_way_and_stops() {
        let mut field = blank_field();
        let mut ms = Missiles::new();
        for y in 30..=45usize {
            field.cells[y * DIM + 40].surface = SURFACE_BRIDGE;
        }
        field.cells[38 * DIM + 41].surface = SURFACE_BURNT_BRIDGE;
        field.cells[38 * DIM + 41].gfx = 0x5A;

        bridge_fire(&mut field, &mut ms, 40, 38);

        let burning: Vec<usize> =
            (30..=45).filter(|&y| field.cells[y * DIM + 40].surface == SURFACE_BURNING).collect();
        assert_eq!(burning, (33..=43).collect::<Vec<_>>(), "five either way of row 38");
        assert_eq!(field.cells[32 * DIM + 40].surface, SURFACE_BRIDGE);
        assert_eq!(field.cells[44 * DIM + 40].surface, SURFACE_BRIDGE);
        assert_eq!(field.cells[38 * DIM + 41].gfx, 0x0A, "scorched, FUN_00485B47");

        let life = |y: i16| ms.iter().find(|(_, m)| m.cell_y == y && m.cell_x == 40).unwrap().1.ttl;
        assert_eq!(life(38), 520);
        assert_eq!(life(37), 520);
        assert_eq!(life(39), 527);
        assert_eq!(life(36), 534);
        assert_eq!(life(40), 541);
        assert_eq!(life(33), 576);
        assert_eq!(life(43), 583);
        assert_eq!(ms.live(), 11);
    }

    #[test]
    fn a_wood_fire_floods_the_wood_one_ring_a_frame() {
        let mut field = blank_field();
        let mut ms = Missiles::new();
        for x in 10..=20usize {
            field.cells[10 * DIM + x].surface = SURFACE_WOODLAND;
        }
        ignite_woodland(&mut field, &mut ms, 15, 10).unwrap();
        let mut spreading = true;
        let row = |f: &Battlefield| (10..=20).map(|x| f.cells[10 * DIM + x].surface).collect::<Vec<_>>();

        spread_woodland(&mut field, &mut ms, &mut spreading);
        let r = row(&field);
        assert_eq!(r[5], SURFACE_WOOD_BURNING);
        assert_eq!((r[4], r[6]), (SURFACE_WOOD_CATCHING, SURFACE_WOOD_CATCHING));
        assert_eq!((r[3], r[7]), (SURFACE_WOODLAND, SURFACE_WOODLAND), "one ring, not two");
        assert!(spreading);

        for _ in 0..5 {
            spread_woodland(&mut field, &mut ms, &mut spreading);
        }
        assert!(row(&field).iter().all(|&s| s == SURFACE_WOOD_BURNING));
        spread_woodland(&mut field, &mut ms, &mut spreading);
        assert!(!spreading, "a frame that turns nothing from catching to burning ends it");
    }

    /// The pour's two tables, from `BattleUnit_Order` and `FUN_0047A814`.
    #[test]
    fn an_order_downhill_pours_and_the_pot_turns_along_the_longer_axis() {
        assert!(order_pours(4, 1) && order_pours(4, 3) && !order_pours(4, 4));
        assert!(order_pours(5, 3) && !order_pours(5, 4), "the bailey pours only below the rampart");
        assert!(order_pours(6, 5) && !order_pours(6, 6));
        assert!(!order_pours(1, 0), "a pot on the field never pours from an order");
        assert_eq!(pour_facing((10, 10), (10, 20)), 4);
        assert_eq!(pour_facing((10, 10), (10, 0)), 0);
        assert_eq!(pour_facing((10, 10), (15, 15)), 2, "a tie goes to x");
        assert_eq!(pour_facing((10, 10), (5, 12)), 6);
        let m = oil_record(2, 7, (10, 10), (10, 20));
        assert_eq!((m.class, m.sub_steps, m.range_ticks, m.power, m.launch_elevation), (7, 16, 16, 0, 4));
        assert_eq!((m.shooter, m.dir, m.ttl), (7, 4, 0));
    }
}

