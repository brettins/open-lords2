#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::screen::*;
use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eight_rows_and_both_totals_sit_inside_the_rows_well() {
        let (well, w) = (rows_well(), window());
        for row in 0..=MERC_ROW {
            let y = row_y(row);
            assert!(y >= well.y && y < well.y + well.h, "row {row} at {y} is outside the well");
            for b in [parent_button(row), daughter_button(row)] {
                assert!(w.contains(b.x, b.y), "row {row}'s button is off the window");
                assert!(w.contains(b.x + b.w - 1, b.y + b.h - 1));
            }
        }
        assert_eq!(parent_button(0).y, well.y - 8, "the first row's button overhangs the well");
        assert_eq!(totals_y(false), row_y(MERC_ROW));
        assert_eq!(totals_y(true) - row_y(MERC_ROW), 0x24);
        assert!(totals_y(true) < well.y + well.h, "the totals run off the well");
    }

    #[test]
    fn the_row_buttons_are_the_widget_tables_own_geometry() {
        for row in 0..=MERC_ROW {
            let (p, d) = (parent_button(row), daughter_button(row));
            assert_eq!((p.x, p.w, p.h), (256, 24, 24), "row {row} parent");
            assert_eq!((d.x, d.w, d.h), (288, 24, 24), "row {row} daughter");
            assert_eq!(p.y, row as i32 * 0x20 + 0x78, "row {row} y");
            assert_eq!(d.y, p.y);
            assert!(PARENT_NUMBER_X < p.x, "row {row}: the button covers its number");
            assert!(d.x + d.w <= DAUGHTER_ICON_X, "row {row}: the button covers the icon");
        }
    }

    #[test]
    fn no_two_hotspots_on_this_screen_overlap() {
        let mut boxes: Vec<(String, Rect)> = vec![
            ("tick".into(), SPLIT_TICK),
            ("cross".into(), SPLIT_CROSS),
            ("ok".into(), OK),
        ];
        for row in 0..=MERC_ROW {
            boxes.push((format!("parent {row}"), parent_button(row)));
            boxes.push((format!("daughter {row}"), daughter_button(row)));
        }
        for (i, (an, a)) in boxes.iter().enumerate() {
            assert!(
                a.x >= 0 && a.y >= 0 && a.x + a.w <= 640 && a.y + a.h <= 480,
                "{an} {a:?} is off the screen",
            );
            for (bn, b) in boxes.iter().skip(i + 1) {
                let hit = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!hit, "{an} {a:?} overlaps {bn} {b:?}");
            }
        }
    }
}

