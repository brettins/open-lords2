#![allow(unused_imports)]
use super::*;
use super::walker::*;
use super::screen::*;
use l2_kingdom::levy::{self, LevyRefusal};
use l2_kingdom::tables::WEAPON_TYPE_COUNT;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::widget;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_racks_are_one_row_across_the_bottom_of_the_screen() {
        let mut xs: Vec<i32> = RACKS.iter().map(|r| r.1).collect();
        for &(_, _, sy, _, ny) in &RACKS {
            assert_eq!(sy, 396, "every rack sprite sits on one line");
            assert_eq!(ny, 450, "and every number under it");
        }
        xs.sort_unstable();
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] >= 70, "racks {pair:?} would overlap");
        }
        assert!(xs[7] + 76 <= 640, "the last rack runs off the screen");

        let mut by_x: Vec<(i32, usize)> = RACKS.iter().map(|r| (r.1, r.0)).collect();
        by_x.sort_unstable();
        assert_eq!(by_x.iter().map(|p| p.1).collect::<Vec<_>>(), vec![6, 7, 8, 9, 10, 11, 12, 13]);
    }

    #[test]
    fn the_totals_rack_is_dead_code_in_the_original() {
        assert_eq!(RACKS_DRAWN, 7, "FUN_004181EB's guard is `if (6 < i) return`");
        assert_eq!(RACKS.len(), 8, "the table has eight records all the same");
        assert!(RACKS_DRAWN < RACKS.len(), "the last record is never drawn");
    }

    #[test]
    fn each_rack_hotspot_covers_the_rack_it_opens() {
        let mut seen: Vec<u8> = RACK_HOTSPOTS.iter().map(|h| h.4).collect();
        seen.sort_unstable();
        assert_eq!(seen, vec![1, 2, 3, 4, 5, 6], "six weapon types, once each");
        for &(x0, y0, x1, y1, troop) in &RACK_HOTSPOTS {
            let (_, sx, sy, ..) = RACKS[troop as usize];
            assert!(x0 <= sx && sx < x1, "hotspot {x0}..{x1} misses rack {troop} at x {sx}");
            assert!(y0 <= sy && sy < y1, "hotspot {y0}..{y1} misses rack {troop} at y {sy}");
            assert!(x1 <= 640 && y1 <= 480, "hotspot for {troop} runs off the screen");
        }
    }

    #[test]
    fn the_three_buttons_are_disjoint_and_hold_their_own_labels() {
        let boxes = [CREATE_BOX, CHANGE_BOX, CANCEL_BOX];
        for (i, a) in boxes.iter().enumerate() {
            assert!(a.x + a.w <= 640 && a.y + a.h <= 480, "button {i} is off screen");
            assert!(a.x <= LABEL_X && LABEL_X < a.x + a.w, "button {i}'s label starts outside it");
            assert!(a.y <= LABEL_Y[i] && LABEL_Y[i] < a.y + a.h, "button {i}'s label is not in it");
            for (j, b) in boxes.iter().enumerate().skip(i + 1) {
                assert!(
                    a.y + a.h <= b.y || b.y + b.h <= a.y,
                    "buttons {i} and {j} overlap: a click would take the first",
                );
            }
            for &(hx0, hy0, hx1, hy1, t) in &RACK_HOTSPOTS {
                assert!(
                    a.x + a.w <= hx0 || hx1 <= a.x || a.y + a.h <= hy0 || hy1 <= a.y,
                    "button {i} overlaps rack {t}",
                );
            }
        }
        assert_eq!(LABEL_X + LABEL_W - 640, 2, "the painter's column overhangs by two");
        assert_eq!(CREATE_BOX.x + CREATE_BOX.w, 634, "and the hotspot stops short of it");
    }

    #[test]
    fn the_four_rack_buttons_are_inside_the_window_and_clear_of_the_ok() {
        let w = rack_window();
        for i in 0..4 {
            let b = button_box(i);
            assert!(w.contains(b.x, b.y), "button {i} starts outside the window");
            assert!(w.contains(b.x + b.w - 1, b.y + b.h - 1), "button {i} runs off it");
            assert!(
                b.x + b.w <= RACK_OK.x
                    || RACK_OK.x + RACK_OK.w <= b.x
                    || b.y + b.h <= RACK_OK.y
                    || RACK_OK.y + RACK_OK.h <= b.y,
                "button {i} overlaps the close button",
            );
            if i > 0 {
                assert!(button_box(i - 1).x + BUTTON_DIM <= b.x, "buttons {i} and {} overlap", i - 1);
            }
        }
        assert_eq!(BUTTON_FRAMES[0], 68, "record 0 carries the plus picture");
        assert_eq!(BUTTON_FRAMES[1], 66, "record 1 carries the minus");
        assert_eq!(BUTTONS[0], Button::EquipOne, "and record 0's body is the plus");
    }

    #[test]
    fn the_weapons_on_the_walls_are_six_and_in_slot_order() {
        for (slot, &(frame, x, y)) in WALL.iter().enumerate() {
            assert_eq!(frame, slot, "wall record {slot} draws frame {frame}");
            assert!((0..640).contains(&x) && (0..480).contains(&y), "wall {slot} is off screen");
        }
    }

    #[test]
    fn a_realm_with_no_banner_gets_the_same_sheet_as_realm_one() {
        assert_eq!(items_sheet(0), items_sheet(1));
        assert_eq!(items_sheet(1), "Arm_it_r.pl8");
        assert_eq!(items_sheet(9), items_sheet(5), "out of range clamps to the last");
        let mut distinct: Vec<&str> = ITEM_SHEETS.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 5, "five colours");
    }
}

